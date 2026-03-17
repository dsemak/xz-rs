use super::{BinaryType, Fixture};

const UPSTREAM_COMPRESS_ARGS_PREFIX: [&str; 4] = [
    "--memlimit-compress=48MiB",
    "--memlimit-decompress=5MiB",
    "--no-adjust",
    "--threads=1",
];

const UPSTREAM_DECOMPRESS_ARGS: [&str; 5] = [
    "--memlimit-compress=48MiB",
    "--memlimit-decompress=5MiB",
    "--no-adjust",
    "--threads=1",
    "-cd",
];

const UPSTREAM_FILTER_CASES: [&str; 10] = [
    "--filters=delta:dist=1 lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=delta:dist=4 lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=delta:dist=256 lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=x86 lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=powerpc lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=ia64 lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=arm lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=armthumb lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=arm64 lzma2:dict=64KiB,nice=32,mode=fast",
    "--filters=sparc lzma2:dict=64KiB,nice=32,mode=fast",
];

const UPSTREAM_FILTER_CASE_RISCV: &str = "--filters=riscv lzma2:dict=64KiB,nice=32,mode=fast";
const UPSTREAM_LEVEL_CASES: [&str; 4] = ["-1", "-2", "-3", "-4"];

pub fn generated_abc() -> Vec<u8> {
    let mut data = Vec::with_capacity(12_345 * 4);
    for _ in 0..12_345 {
        data.extend_from_slice(b"abc\n");
    }
    data
}

pub fn generated_random() -> Vec<u8> {
    let mut n = 5_u32;
    let mut data = Vec::with_capacity(123_456 * 4);
    for _ in 0..123_456 {
        n = n.wrapping_mul(101_771).wrapping_add(71_777);
        data.extend_from_slice(&n.to_le_bytes());
    }
    data
}

pub fn generated_text() -> Vec<u8> {
    let lorem = [
        "Lorem",
        "ipsum",
        "dolor",
        "sit",
        "amet,",
        "consectetur",
        "adipisicing",
        "elit,",
        "sed",
        "do",
        "eiusmod",
        "tempor",
        "incididunt",
        "ut",
        "labore",
        "et",
        "dolore",
        "magna",
        "aliqua.",
        "Ut",
        "enim",
        "ad",
        "minim",
        "veniam,",
        "quis",
        "nostrud",
        "exercitation",
        "ullamco",
        "laboris",
        "nisi",
        "ut",
        "aliquip",
        "ex",
        "ea",
        "commodo",
        "consequat.",
        "Duis",
        "aute",
        "irure",
        "dolor",
        "in",
        "reprehenderit",
        "in",
        "voluptate",
        "velit",
        "esse",
        "cillum",
        "dolore",
        "eu",
        "fugiat",
        "nulla",
        "pariatur.",
        "Excepteur",
        "sint",
        "occaecat",
        "cupidatat",
        "non",
        "proident,",
        "sunt",
        "in",
        "culpa",
        "qui",
        "officia",
        "deserunt",
        "mollit",
        "anim",
        "id",
        "est",
        "laborum.",
    ];

    let mut out = String::new();
    for (idx, word) in lorem.iter().enumerate() {
        out.push_str(word);
        out.push(' ');
        if idx % 7 == 6 {
            out.push('\n');
        }
    }

    let mut n = 29_u32;
    for _ in 0..500 {
        out.push_str("\n\n");
        for idx in 0..lorem.len() {
            n = n.wrapping_mul(101_771).wrapping_add(71_777);
            out.push_str(lorem[(n as usize) % lorem.len()]);
            out.push(' ');
            if idx % 7 == 6 {
                out.push('\n');
            }
        }
    }

    out.into_bytes()
}

pub async fn assert_generated_roundtrip(file_name: &str, data: &[u8]) {
    let mut fixture = Fixture::with_file(file_name, data);
    let file_path = fixture.path(file_name);

    for case_arg in UPSTREAM_LEVEL_CASES {
        assert_roundtrip_case(&mut fixture, file_name, data, &file_path, &[case_arg]).await;
    }

    for case_arg in UPSTREAM_FILTER_CASES {
        assert_roundtrip_case(&mut fixture, file_name, data, &file_path, &[case_arg]).await;
    }

    assert_roundtrip_case(
        &mut fixture,
        file_name,
        data,
        &file_path,
        &[UPSTREAM_FILTER_CASE_RISCV],
    )
    .await;
}

async fn assert_roundtrip_case(
    fixture: &mut Fixture,
    file_name: &str,
    data: &[u8],
    file_path: &str,
    case_args: &[&str],
) {
    let mut compress_args =
        Vec::with_capacity(UPSTREAM_COMPRESS_ARGS_PREFIX.len() + case_args.len() + 2);
    compress_args.extend(UPSTREAM_COMPRESS_ARGS_PREFIX);
    compress_args.extend(case_args.iter().copied());
    compress_args.push("-c");
    compress_args.push(file_path);

    let compressed = fixture.run_cargo("xz", &compress_args).await;
    assert!(
        compressed.status.success(),
        "compression failed for {file_name} with {:?}: {}",
        case_args,
        compressed.stderr
    );

    let decompressed = fixture
        .run_with_stdin_raw(
            BinaryType::cargo("xz"),
            &UPSTREAM_DECOMPRESS_ARGS,
            &compressed.stdout_raw,
        )
        .await;
    assert!(
        decompressed.status.success(),
        "decompression failed for {file_name} with {:?}: {}",
        case_args,
        decompressed.stderr
    );
    assert_eq!(
        decompressed.stdout_raw, data,
        "roundtrip mismatch for {file_name} with {:?}",
        case_args
    );
}
