mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::{
    benchmark_targets, locate_cargo_binary, locate_system_binary, prepare_compress_fixture,
    prepare_decode_file_fixture, prepare_decode_stdout_fixture, run_checked, CodecFormat,
    DatasetKind, MIB,
};
use criterion::measurement::WallTime;
use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkGroup, BenchmarkId, Criterion,
    Throughput,
};

const SIZES: &[usize] = &[MIB, 8 * MIB, 32 * MIB];
const LEVELS: &[u32] = &[1, 6, 9];
const DATASETS: &[DatasetKind] = &[
    DatasetKind::Textual,
    DatasetKind::Mixed,
    DatasetKind::Binary,
];

fn size_label(size: usize) -> String {
    format!("{}MiB", size / MIB)
}

fn case_label(dataset: DatasetKind, size: usize, level: u32) -> String {
    format!("{}/{}/level-{level}", dataset.label(), size_label(size))
}

fn prepare_group<'a>(c: &'a mut Criterion, name: String) -> BenchmarkGroup<'a, WallTime> {
    c.benchmark_group(name)
}

struct CompressCase<'a> {
    binary_name: &'a str,
    format: CodecFormat,
    target: &'a common::targets::BenchmarkTarget,
    dataset: DatasetKind,
    level: u32,
    bench_case: &'a str,
    data: &'a [u8],
}

struct DecodeCase<'a> {
    binary_name: &'a str,
    format: CodecFormat,
    target: &'a common::targets::BenchmarkTarget,
    dataset: DatasetKind,
    level: u32,
    bench_case: &'a str,
    data: &'a [u8],
    compressor_path: &'a Path,
}

fn register_compress_case(group: &mut BenchmarkGroup<'_, WallTime>, case: CompressCase<'_>) {
    let CompressCase {
        binary_name,
        format,
        target,
        dataset,
        level,
        bench_case,
        data,
    } = case;

    group.bench_with_input(
        BenchmarkId::new(target.label, bench_case),
        data,
        |b, input_data| {
            b.iter_batched_ref(
                || prepare_compress_fixture(format, dataset, input_data),
                |fixture| {
                    let mut command = Command::new(&target.binary_path);
                    command
                        .arg(format!("-{level}"))
                        .arg("-k")
                        .arg("-f")
                        .arg("-T1")
                        .arg(&fixture.input_path);
                    run_checked(&mut command, binary_name);

                    let compressed_size = fs::metadata(&fixture.compressed_path)
                        .expect("compressed output is missing")
                        .len();
                    black_box(compressed_size);
                },
                BatchSize::PerIteration,
            );
        },
    );
}

fn register_decode_file_case(group: &mut BenchmarkGroup<'_, WallTime>, case: DecodeCase<'_>) {
    let DecodeCase {
        binary_name,
        format,
        target,
        dataset,
        level,
        bench_case,
        data,
        compressor_path,
    } = case;

    group.bench_with_input(
        BenchmarkId::new(target.label, bench_case),
        data,
        |b, input_data| {
            b.iter_batched_ref(
                || prepare_decode_file_fixture(format, dataset, input_data, compressor_path, level),
                |fixture| {
                    let mut command = Command::new(&target.binary_path);
                    command.arg("-k").arg("-f").arg(&fixture.compressed_path);
                    run_checked(&mut command, binary_name);

                    let decoded =
                        fs::read(&fixture.output_path).expect("decoded output is missing");
                    assert_eq!(decoded, fixture.expected_data, "decoded bytes differ");
                    black_box(decoded.len());
                },
                BatchSize::PerIteration,
            );
        },
    );
}

fn register_decode_stdout_case(group: &mut BenchmarkGroup<'_, WallTime>, case: DecodeCase<'_>) {
    let DecodeCase {
        binary_name,
        format,
        target,
        dataset,
        level,
        bench_case,
        data,
        compressor_path,
    } = case;

    group.bench_with_input(
        BenchmarkId::new(target.label, bench_case),
        data,
        |b, input_data| {
            b.iter_batched_ref(
                || {
                    prepare_decode_stdout_fixture(
                        format,
                        dataset,
                        input_data,
                        compressor_path,
                        level,
                    )
                },
                |fixture| {
                    let mut command = Command::new(&target.binary_path);
                    command.arg(&fixture.compressed_path);
                    let output = run_checked(&mut command, binary_name);

                    assert_eq!(
                        output.stdout, fixture.expected_data,
                        "decoded stdout differs"
                    );
                    black_box(output.stdout.len());
                },
                BatchSize::PerIteration,
            );
        },
    );
}

fn bench_compress_alias(c: &mut Criterion, binary_name: &str, format: CodecFormat) {
    let targets = benchmark_targets(binary_name);
    let mut group = prepare_group(c, format!("{binary_name}/compress"));

    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));

        for &dataset in DATASETS {
            let data = dataset.build(size);

            for &level in LEVELS {
                let bench_case = case_label(dataset, size, level);

                for target in &targets {
                    register_compress_case(&mut group, CompressCase {
                        binary_name,
                        format,
                        target,
                        dataset,
                        level,
                        bench_case: &bench_case,
                        data: &data,
                    });
                }
            }
        }
    }

    group.finish();
}

fn bench_decode_file_alias(c: &mut Criterion, binary_name: &str, format: CodecFormat) {
    let targets = benchmark_targets(binary_name);
    let compressor_path = locate_system_binary(format.compressor_binary())
        .unwrap_or_else(|| locate_cargo_binary(format.compressor_binary()));
    let mut group = prepare_group(c, format!("{binary_name}/decode_file"));

    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));

        for &dataset in DATASETS {
            let data = dataset.build(size);

            for &level in LEVELS {
                let bench_case = case_label(dataset, size, level);

                for target in &targets {
                    register_decode_file_case(&mut group, DecodeCase {
                        binary_name,
                        format,
                        target,
                        dataset,
                        level,
                        bench_case: &bench_case,
                        data: &data,
                        compressor_path: &compressor_path,
                    });
                }
            }
        }
    }

    group.finish();
}

fn bench_decode_stdout_alias(c: &mut Criterion, binary_name: &str, format: CodecFormat) {
    let targets = benchmark_targets(binary_name);
    let compressor_path = locate_system_binary(format.compressor_binary())
        .unwrap_or_else(|| locate_cargo_binary(format.compressor_binary()));
    let mut group = prepare_group(c, format!("{binary_name}/decode_stdout"));

    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));

        for &dataset in DATASETS {
            let data = dataset.build(size);

            for &level in LEVELS {
                let bench_case = case_label(dataset, size, level);

                for target in &targets {
                    register_decode_stdout_case(&mut group, DecodeCase {
                        binary_name,
                        format,
                        target,
                        dataset,
                        level,
                        bench_case: &bench_case,
                        data: &data,
                        compressor_path: &compressor_path,
                    });
                }
            }
        }
    }

    group.finish();
}

fn criterion_benchmarks(c: &mut Criterion) {
    bench_compress_alias(c, "xz", CodecFormat::Xz);
    bench_decode_file_alias(c, "unxz", CodecFormat::Xz);
    bench_decode_stdout_alias(c, "xzcat", CodecFormat::Xz);

    bench_compress_alias(c, "lzma", CodecFormat::Lzma);
    bench_decode_file_alias(c, "unlzma", CodecFormat::Lzma);
    bench_decode_stdout_alias(c, "lzcat", CodecFormat::Lzma);
}

criterion_group! {
    name = cli_vs_upstream_benches;
    config = Criterion::default();
    targets = criterion_benchmarks
}
criterion_main!(cli_vs_upstream_benches);
