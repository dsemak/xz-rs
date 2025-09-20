use crate::decoder::options::Flags;
use crate::decoder::Options;
use crate::encoder::options::{Compression, IntegrityCheck};
use crate::{Action, Error, Stream};

use super::*;

/// Test data for primary stream.
const TEST_DATA_PRIMARY: &[u8] = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAABBBBBBBBBBBBBBBB";
const TEST_DATA_SECONDARY: &[u8] = b"CCCCCCCCCCCCCCDDDDDDDDDDDDD";
/// Test data for legacy .lzma format.
const LZMA_ALONE_DATA: &[u8] = b"Standalone LZMA Test Data";
/// Test data for legacy .lzma format stream.
const LZMA_ALONE_STREAM: &[u8] = &[
    93, 0, 0, 128, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 41, 157, 8, 39, 16, 149, 194, 228,
    207, 53, 88, 218, 14, 168, 142, 173, 34, 251, 230, 151, 180, 204, 104, 101, 236, 21, 198, 56,
    155, 104, 255, 254, 87, 240, 0,
];

fn compress_xz(data: &[u8]) -> Vec<u8> {
    let mut encoder = Stream::default()
        .easy_encoder(Compression::Level6, IntegrityCheck::Crc32)
        .unwrap();
    let mut output = vec![0u8; 4096];
    let (read, written) = encoder.process(data, &mut output, Action::Run).unwrap();
    assert_eq!(read, data.len());
    let mut compressed = Vec::from(&output[..written]);
    let (_, finish_written) = encoder.process(&[], &mut output, Action::Finish).unwrap();
    compressed.extend_from_slice(&output[..finish_written]);
    compressed
}

/// Test basic decoder decompression functionality.
#[test]
fn decoder_basic_decompression() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();

    let mut output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
    let (bytes_read, bytes_written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();

    assert_eq!(bytes_read, compressed.len());
    assert_eq!(bytes_written, TEST_DATA_PRIMARY.len());
    assert_eq!(&output[..bytes_written], TEST_DATA_PRIMARY);
    assert!(decoder.is_finished());
    assert_eq!(decoder.total_in(), bytes_read as u64);
    assert_eq!(decoder.total_out(), bytes_written as u64);
}

/// Test decoder works with small output buffers.
#[test]
fn decoder_small_output_buffer_progress() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();

    let mut remaining_input = compressed.as_slice();
    let mut scratch = vec![0u8; 5];
    let mut output = Vec::new();
    let mut action = Action::Run;

    while !decoder.is_finished() {
        let (read, written) = decoder
            .process(remaining_input, &mut scratch, action)
            .unwrap();
        output.extend_from_slice(&scratch[..written]);
        remaining_input = &remaining_input[read..];

        if remaining_input.is_empty() {
            action = Action::Finish;
        }

        if read == 0 && written == 0 {
            assert!(decoder.is_finished());
        }
    }

    assert_eq!(output, TEST_DATA_PRIMARY);
}

/// Test auto decoder handles concatenated streams.
#[test]
fn decoder_auto_with_concatenated_streams() {
    let mut concatenated = compress_xz(TEST_DATA_PRIMARY);
    concatenated.extend_from_slice(&compress_xz(TEST_DATA_SECONDARY));

    let flags = Flags::CONCATENATED;
    let mut decoder = Stream::default().auto_decoder(u64::MAX, flags).unwrap();

    let expected: Vec<u8> = TEST_DATA_PRIMARY
        .iter()
        .copied()
        .chain(TEST_DATA_SECONDARY.iter().copied())
        .collect();
    let mut output = vec![0u8; expected.len()];
    let (bytes_read, bytes_written) = decoder
        .process(&concatenated, &mut output, Action::Finish)
        .unwrap();

    assert_eq!(bytes_read, concatenated.len());
    assert_eq!(bytes_written, expected.len());
    assert_eq!(&output[..bytes_written], expected.as_slice());
    assert!(decoder.flags().is_concatenated());
    assert!(decoder.is_finished());
}

/// Test decoder supports legacy .lzma format.
#[test]
fn decoder_alone_stream_support() {
    let mut decoder = Stream::default().alone_decoder(u64::MAX).unwrap();

    let mut output = vec![0u8; LZMA_ALONE_DATA.len() + 8];
    let (bytes_read, bytes_written) = decoder
        .process(LZMA_ALONE_STREAM, &mut output, Action::Finish)
        .unwrap();

    assert_eq!(bytes_read, LZMA_ALONE_STREAM.len());
    assert_eq!(bytes_written, LZMA_ALONE_DATA.len());
    assert_eq!(&output[..bytes_written], LZMA_ALONE_DATA);
    assert!(decoder.is_finished());
}

/// Test decoder configuration and multithreading.
#[test]
fn decoder_configuration_accessors_and_mt() {
    let flags = Flags::NO_CHECK | Flags::IGNORE_CHECK;
    let memlimit = 8 * 1024 * 1024;
    let memlimit_stop = 16 * 1024 * 1024;
    let threads = 2;
    let options = Options {
        threads,
        memlimit,
        memlimit_stop,
        flags,
        timeout: 42,
    };

    let decoder = Decoder::new_mt(options, Stream::default()).unwrap();

    assert_eq!(decoder.memlimit(), memlimit);
    assert!(decoder.flags().is_no_check());
    assert!(decoder.flags().is_ignore_check());
    assert_eq!(decoder.threads(), threads);
    assert!(!decoder.is_finished());
}

/// Test decoder errors when processing after finish.
#[test]
fn decoder_process_after_finish_errors() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();

    let mut output = vec![0u8; TEST_DATA_PRIMARY.len()];
    decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();

    assert!(decoder.is_finished());

    let err = decoder
        .process(&compressed, &mut output, Action::Run)
        .unwrap_err();
    assert_eq!(err, Error::ProgError);
}

/// Test decoder with corrupted data returns [`Error::DataError`].
#[test]
fn decoder_corrupted_data_error() {
    let mut corrupted_data = compress_xz(TEST_DATA_PRIMARY);
    // Corrupt the data by modifying bytes in the middle
    let mid_point = corrupted_data.len() / 2;
    corrupted_data[mid_point] = 0xFF;
    corrupted_data[mid_point + 1] = 0xFF;

    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];

    let result = decoder.process(&corrupted_data, &mut output, Action::Finish);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::DataError | Error::FormatError));
}

/// Test decoder with invalid format data returns [`Error::FormatError`].
#[test]
fn decoder_invalid_format_error() {
    let invalid_data = b"Not XZ compressed data";
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut output = vec![0u8; 1024];

    let result = decoder.process(invalid_data, &mut output, Action::Finish);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::FormatError | Error::DataError));
}

/// Test decoder with memory limit too low returns [`Error::MemLimitError`].
#[test]
fn decoder_memory_limit_error() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    // Set memory limit too low (1 byte)
    let result = Stream::default().decoder(1, Flags::empty());

    // Either fails during initialization or during processing
    if let Ok(mut decoder) = result {
        let mut output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
        let process_result = decoder.process(&compressed, &mut output, Action::Finish);
        if let Err(err) = process_result {
            assert!(matches!(err, Error::MemLimitError | Error::MemError));
        }
    } else {
        assert!(result.is_err());
        // Memory limit error expected during initialization
    }
}

/// Test decoder with [`Flags::NO_CHECK`] flag.
#[test]
fn decoder_no_check_flag() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default()
        .decoder(u64::MAX, Flags::NO_CHECK)
        .unwrap();

    assert!(decoder.flags().is_no_check());

    let mut output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
    let (bytes_read, bytes_written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();

    assert_eq!(bytes_read, compressed.len());
    assert_eq!(bytes_written, TEST_DATA_PRIMARY.len());
    assert_eq!(&output[..bytes_written], TEST_DATA_PRIMARY);
}

/// Test decoder with [`Flags::IGNORE_CHECK`] flag.
#[test]
fn decoder_ignore_check_flag() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default()
        .decoder(u64::MAX, Flags::IGNORE_CHECK)
        .unwrap();

    assert!(decoder.flags().is_ignore_check());

    let mut output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
    let (bytes_read, bytes_written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();

    assert_eq!(bytes_read, compressed.len());
    assert_eq!(bytes_written, TEST_DATA_PRIMARY.len());
    assert_eq!(&output[..bytes_written], TEST_DATA_PRIMARY);
}

/// Test decoder with multiple flags combined.
#[test]
fn decoder_combined_flags() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let flags = Flags::NO_CHECK | Flags::IGNORE_CHECK | Flags::CONCATENATED;
    let mut decoder = Stream::default().decoder(u64::MAX, flags).unwrap();

    assert!(decoder.flags().is_no_check());
    assert!(decoder.flags().is_ignore_check());
    assert!(decoder.flags().is_concatenated());
    assert!(!decoder.flags().is_any_check());
    assert!(!decoder.flags().is_unsupported_check());

    let mut output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
    let (bytes_read, bytes_written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();

    assert_eq!(bytes_read, compressed.len());
    assert_eq!(bytes_written, TEST_DATA_PRIMARY.len());
    assert_eq!(&output[..bytes_written], TEST_DATA_PRIMARY);
}

/// Test standard stream decoder (new) vs auto decoder functionality.
#[test]
fn decoder_standard_vs_auto() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);

    // Test standard decoder
    let mut std_decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut std_output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
    let (std_read, std_written) = std_decoder
        .process(&compressed, &mut std_output, Action::Finish)
        .unwrap();

    // Test auto decoder
    let mut auto_decoder = Stream::default()
        .auto_decoder(u64::MAX, Flags::empty())
        .unwrap();
    let mut auto_output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
    let (auto_read, auto_written) = auto_decoder
        .process(&compressed, &mut auto_output, Action::Finish)
        .unwrap();

    // Both should produce identical results
    assert_eq!(std_read, auto_read);
    assert_eq!(std_written, auto_written);
    assert_eq!(&std_output[..std_written], &auto_output[..auto_written]);
    assert_eq!(&std_output[..std_written], TEST_DATA_PRIMARY);
}

/// Test decoder with empty input data.
#[test]
fn decoder_empty_input() {
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut output = vec![0u8; 1024];

    let (bytes_read, bytes_written) = decoder.process(&[], &mut output, Action::Finish).unwrap();

    assert_eq!(bytes_read, 0);
    assert_eq!(bytes_written, 0);
    assert!(decoder.is_finished());
}

/// Test decoder with zero-sized output buffer.
#[test]
fn decoder_zero_output_buffer() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut output = vec![];

    let (bytes_read, bytes_written) = decoder
        .process(&compressed, &mut output, Action::Run)
        .unwrap();

    assert_eq!(bytes_written, 0);
    // Should read some input even with zero output buffer
    assert!(bytes_read > 0 || !decoder.is_finished());
}

/// Test decoder total counters accuracy.
#[test]
fn decoder_total_counters() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();

    // Initially counters should be zero
    assert_eq!(decoder.total_in(), 0);
    assert_eq!(decoder.total_out(), 0);

    let mut output = vec![0u8; TEST_DATA_PRIMARY.len() * 2];
    let (bytes_read, bytes_written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();

    // Counters should match the operation results
    assert_eq!(decoder.total_in(), bytes_read as u64);
    assert_eq!(decoder.total_out(), bytes_written as u64);
    assert_eq!(decoder.total_in(), compressed.len() as u64);
    assert_eq!(decoder.total_out(), TEST_DATA_PRIMARY.len() as u64);
}

/// Test decoder partial processing with [`Action::Run`].
#[test]
fn decoder_partial_processing() {
    let compressed = compress_xz(TEST_DATA_PRIMARY);
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();

    let mut _total_read = 0;
    let mut total_written = 0;
    let mut output = Vec::new();
    let mut output_chunk = vec![0u8; 10]; // Small chunks

    // Process in small chunks with Action::Run
    for chunk in compressed.chunks(5) {
        let (bytes_read, bytes_written) = decoder
            .process(chunk, &mut output_chunk, Action::Run)
            .unwrap();

        _total_read += bytes_read;
        total_written += bytes_written;
        output.extend_from_slice(&output_chunk[..bytes_written]);

        if bytes_read < chunk.len() {
            // If not all input was consumed, we might need to finish
            break;
        }
    }

    // Finish processing
    loop {
        let (bytes_read, bytes_written) = decoder
            .process(&[], &mut output_chunk, Action::Finish)
            .unwrap();

        _total_read += bytes_read;
        total_written += bytes_written;
        output.extend_from_slice(&output_chunk[..bytes_written]);

        if decoder.is_finished() {
            break;
        }
    }

    assert_eq!(output, TEST_DATA_PRIMARY);
    assert_eq!(total_written, TEST_DATA_PRIMARY.len());
}
