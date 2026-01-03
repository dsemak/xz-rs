use crate::decoder::options::Flags;
use crate::encoder::options::{Compression, IntegrityCheck, Lzma1Options, Options};
use crate::{Action, Error, Stream};

use super::*;

const TEST_DATA: &[u8] = b"The quick brown fox jumps over the lazy dog";

fn encode_all(encoder: &mut Encoder, data: &[u8]) -> Vec<u8> {
    let mut output = vec![0u8; 4096];
    let mut compressed = Vec::new();
    let mut remaining_data = data;

    // Process all input data
    while !remaining_data.is_empty() {
        let (read, written) = encoder
            .process(remaining_data, &mut output, Action::Run)
            .unwrap();

        if read == 0 && written == 0 {
            // No progress at all; avoid spinning forever
            break;
        }

        remaining_data = &remaining_data[read..];
        compressed.extend_from_slice(&output[..written]);
    }

    // Finish the stream, allowing liblzma to emit remaining blocks across calls.
    while !encoder.is_finished() {
        let (_, written_finish) = encoder.process(&[], &mut output, Action::Finish).unwrap();

        if written_finish == 0 {
            // Nothing emitted yet; try again until liblzma signals completion.
            continue;
        }

        compressed.extend_from_slice(&output[..written_finish]);
    }

    compressed
}

fn encode_all_alone(encoder: &mut AloneEncoder, data: &[u8]) -> Vec<u8> {
    let mut output = vec![0u8; 4096];
    let mut compressed = Vec::new();
    let mut remaining_data = data;

    while !remaining_data.is_empty() {
        let (read, written) = encoder
            .process(remaining_data, &mut output, Action::Run)
            .unwrap();

        if read == 0 && written == 0 {
            break;
        }

        remaining_data = &remaining_data[read..];
        compressed.extend_from_slice(&output[..written]);
    }

    while !encoder.is_finished() {
        let (_, written_finish) = encoder.process(&[], &mut output, Action::Finish).unwrap();
        if written_finish == 0 {
            continue;
        }
        compressed.extend_from_slice(&output[..written_finish]);
    }

    compressed
}

/// Test basic encoder round-trip compression and decompression.
#[test]
fn easy_encoder_round_trip() {
    let mut encoder = Stream::default()
        .easy_encoder(Compression::Level6, IntegrityCheck::Crc32)
        .unwrap();

    let compressed = encode_all(&mut encoder, TEST_DATA);
    assert!(encoder.is_finished());
    assert_eq!(encoder.total_in(), TEST_DATA.len() as u64);
    assert!(encoder.total_out() > 0);

    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();

    let mut output = vec![0u8; TEST_DATA.len() * 2];
    let (read, written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();

    assert_eq!(read, compressed.len());
    assert_eq!(written, TEST_DATA.len());
    assert_eq!(&output[..written], TEST_DATA);
    assert!(decoder.is_finished());
}

/// Test `.lzma` encoder round-trip via `.lzma` decoder.
#[test]
fn alone_encoder_round_trip() {
    let options = Lzma1Options::from_preset(Compression::Level6).unwrap();
    let mut encoder = Stream::default().alone_encoder(options).unwrap();

    let compressed = encode_all_alone(&mut encoder, TEST_DATA);
    assert!(encoder.is_finished());
    assert_eq!(encoder.total_in(), TEST_DATA.len() as u64);
    assert!(encoder.total_out() > 0);

    let mut decoder = Stream::default().alone_decoder(u64::MAX).unwrap();
    let mut output = vec![0u8; TEST_DATA.len() * 2];
    let (_, written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();
    assert_eq!(written, TEST_DATA.len());
    assert_eq!(&output[..written], TEST_DATA);
}

/// Test `.lzma` encoder rejects unsupported actions.
#[test]
fn alone_encoder_rejects_flush_actions() {
    let options = Lzma1Options::from_preset(Compression::Level1).unwrap();
    let mut encoder = Stream::default().alone_encoder(options).unwrap();
    let mut output = vec![0u8; 128];

    let err = encoder
        .process(TEST_DATA, &mut output, Action::SyncFlush)
        .unwrap_err();
    assert_eq!(err, Error::ProgError);
}

/// Test encoder state tracking and finish behavior.
#[test]
fn encoder_tracks_totals_and_finish_behavior() {
    let stream = Stream::default();
    let mut encoder = Encoder::new(Compression::Level5, IntegrityCheck::None, stream).unwrap();

    let mut buffer = vec![0u8; 2048];
    let (bytes_read, first_written) = encoder
        .process(TEST_DATA, &mut buffer, Action::Run)
        .unwrap();
    assert_eq!(bytes_read, TEST_DATA.len());
    assert!(!encoder.is_finished());
    assert_eq!(encoder.total_in(), bytes_read as u64);
    assert_eq!(encoder.total_out(), first_written as u64);

    let (_, second_written) = encoder.process(&[], &mut buffer, Action::Finish).unwrap();
    assert!(encoder.is_finished());
    assert_eq!(encoder.total_in(), TEST_DATA.len() as u64);
    assert_eq!(encoder.total_out(), (first_written + second_written) as u64);

    let err = encoder
        .process(&[], &mut buffer, Action::Finish)
        .unwrap_err();
    assert_eq!(err, Error::ProgError);

    let (post_read, post_written) = encoder
        .process(TEST_DATA, &mut buffer, Action::Run)
        .unwrap();
    assert_eq!(post_read, 0);
    assert_eq!(post_written, 0);
}

/// Test multithreaded encoder produces valid compressed stream.
#[test]
fn multithreaded_encoder_produces_valid_stream() {
    let options = Options {
        level: Compression::Level3,
        check: IntegrityCheck::Crc32,
        threads: 2,
        block_size: 64 * 1024,
        timeout: 10,
        filters: Vec::new(),
    };

    let stream = Stream::default();
    let mut encoder = Encoder::new_mt(options, stream).unwrap();

    let compressed = encode_all(&mut encoder, TEST_DATA);
    assert!(encoder.is_finished());
    assert_eq!(encoder.threads(), 2);
    assert_eq!(encoder.check(), IntegrityCheck::Crc32);

    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut output = vec![0u8; TEST_DATA.len() * 2];
    let (_, written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();
    assert_eq!(written, TEST_DATA.len());
    assert_eq!(&output[..written], TEST_DATA);
}

/// Test zero threads are promoted to one for multithreaded encoder.
#[test]
fn multithreaded_encoder_from_stream_promotes_zero_threads() {
    let mut encoder = Stream::default()
        .multithreaded_encoder(Compression::Level4, IntegrityCheck::Crc32, 0)
        .unwrap();
    assert_eq!(encoder.threads(), 1);

    let compressed = encode_all(&mut encoder, TEST_DATA);
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut output = vec![0u8; TEST_DATA.len() * 2];
    let (_, written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();
    assert_eq!(written, TEST_DATA.len());
    assert_eq!(&output[..written], TEST_DATA);
}

/// Test all compression levels produce valid output.
#[test]
fn all_compression_levels_work() {
    let levels = [
        Compression::Level0,
        Compression::Level1,
        Compression::Level2,
        Compression::Level3,
        Compression::Level4,
        Compression::Level5,
        Compression::Level6,
        Compression::Level7,
        Compression::Level8,
        Compression::Level9,
    ];

    for level in levels {
        let mut encoder = Stream::default()
            .easy_encoder(level, IntegrityCheck::Crc32)
            .unwrap();
        assert_eq!(encoder.compression_level(), level);

        let compressed = encode_all(&mut encoder, TEST_DATA);
        assert!(!compressed.is_empty());
        assert!(encoder.is_finished());

        // Verify decompression works
        let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
        let mut output = vec![0u8; TEST_DATA.len() * 2];
        let (_, written) = decoder
            .process(&compressed, &mut output, Action::Finish)
            .unwrap();
        assert_eq!(written, TEST_DATA.len());
        assert_eq!(&output[..written], TEST_DATA);
    }
}

/// Test all integrity check types work correctly.
#[test]
fn all_integrity_checks_work() {
    let checks = [
        IntegrityCheck::None,
        IntegrityCheck::Crc32,
        IntegrityCheck::Crc64,
        IntegrityCheck::Sha256,
    ];

    for check in checks {
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level3, check)
            .unwrap();
        assert_eq!(encoder.check(), check);

        let compressed = encode_all(&mut encoder, TEST_DATA);
        assert!(!compressed.is_empty());
        assert!(encoder.is_finished());

        // Verify decompression works
        let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
        let mut output = vec![0u8; TEST_DATA.len() * 2];
        let (_, written) = decoder
            .process(&compressed, &mut output, Action::Finish)
            .unwrap();
        assert_eq!(written, TEST_DATA.len());
        assert_eq!(&output[..written], TEST_DATA);
    }
}

/// Test error handling for insufficient output buffer.
#[test]
fn encoder_handles_insufficient_output_buffer() {
    let mut encoder = Stream::default()
        .easy_encoder(Compression::Level1, IntegrityCheck::None)
        .unwrap();

    // Try with a very small output buffer
    let mut small_buffer = vec![0u8; 1];
    let result = encoder.process(TEST_DATA, &mut small_buffer, Action::Run);

    // Should succeed but process only partial data
    assert!(result.is_ok());
    let (bytes_read, bytes_written) = result.unwrap();
    assert!(bytes_read <= TEST_DATA.len());
    assert!(bytes_written <= small_buffer.len());
}

/// Test encoder behavior with empty input data.
#[test]
fn encoder_handles_empty_input() {
    let mut encoder = Stream::default()
        .easy_encoder(Compression::Level1, IntegrityCheck::Crc32)
        .unwrap();

    let empty_data = b"";
    let mut output = vec![0u8; 1024];

    // Process empty data
    let (read, written) = encoder
        .process(empty_data, &mut output, Action::Run)
        .unwrap();
    assert_eq!(read, 0);

    // Finish the stream
    let (_, written_finish) = encoder
        .process(&[], &mut output[written..], Action::Finish)
        .unwrap();

    assert!(encoder.is_finished());
    assert!(written_finish > 0); // Should still have header/footer

    // Verify the compressed empty stream can be decompressed
    let compressed = &output[..written + written_finish];
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut decompressed = vec![0u8; 100];
    let (_, decompressed_len) = decoder
        .process(compressed, &mut decompressed, Action::Finish)
        .unwrap();
    assert_eq!(decompressed_len, 0);
}

/// Test different Action types during encoding.
#[test]
fn encoder_handles_different_actions() {
    // Test Action::SyncFlush
    {
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level2, IntegrityCheck::Crc32)
            .unwrap();

        let mut output = vec![0u8; 4096];
        let (read, _written) = encoder
            .process(&[], &mut output, Action::SyncFlush)
            .unwrap();
        assert_eq!(read, 0);
        // SyncFlush should succeed without error
    }

    // Test Action::FullFlush
    {
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level2, IntegrityCheck::Crc32)
            .unwrap();

        let mut output = vec![0u8; 4096];
        let (read, _written) = encoder
            .process(&[], &mut output, Action::FullFlush)
            .unwrap();
        assert_eq!(read, 0);
        // FullFlush should succeed without error
    }

    // Test normal encoding with Action::Run and Action::Finish
    {
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level2, IntegrityCheck::Crc32)
            .unwrap();

        let compressed = encode_all(&mut encoder, TEST_DATA);
        assert!(encoder.is_finished());
        assert_eq!(encoder.total_in(), TEST_DATA.len() as u64);

        // Verify the compressed stream can be decompressed
        let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
        let mut decompressed = vec![0u8; TEST_DATA.len() * 2];
        let (_, decompressed_len) = decoder
            .process(&compressed, &mut decompressed, Action::Finish)
            .unwrap();
        assert_eq!(decompressed_len, TEST_DATA.len());
        assert_eq!(&decompressed[..decompressed_len], TEST_DATA);
    }
}

/// Test encoder with large data input.
#[test]
fn encoder_handles_large_data() {
    let large_data = vec![42u8; 1024 * 1024]; // 1MB of data
    let mut encoder = Stream::default()
        .easy_encoder(Compression::Level1, IntegrityCheck::Crc32)
        .unwrap();

    let compressed = encode_all(&mut encoder, &large_data);
    assert!(!compressed.is_empty());
    assert!(encoder.is_finished());
    assert_eq!(encoder.total_in(), large_data.len() as u64);

    // Verify decompression
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut decompressed = vec![0u8; large_data.len() + 1024];
    let (_, decompressed_len) = decoder
        .process(&compressed, &mut decompressed, Action::Finish)
        .unwrap();
    assert_eq!(decompressed_len, large_data.len());
    assert_eq!(&decompressed[..decompressed_len], &large_data[..]);
}

/// Test encoder options builder methods.
#[test]
fn encoder_options_builder_methods() {
    let options = Options::default()
        .with_level(Compression::Level5)
        .with_check(IntegrityCheck::Sha256)
        .with_threads(4)
        .with_block_size(128 * 1024)
        .with_timeout(500);

    let stream = Stream::default();
    let mut encoder = Encoder::new_mt(options, stream).unwrap();

    assert_eq!(encoder.compression_level(), Compression::Level5);
    assert_eq!(encoder.check(), IntegrityCheck::Sha256);
    assert_eq!(encoder.threads(), 4);

    let compressed = encode_all(&mut encoder, TEST_DATA);
    assert!(!compressed.is_empty());

    // Verify decompression works
    let mut decoder = Stream::default().decoder(u64::MAX, Flags::empty()).unwrap();
    let mut output = vec![0u8; TEST_DATA.len() * 2];
    let (_, written) = decoder
        .process(&compressed, &mut output, Action::Finish)
        .unwrap();
    assert_eq!(written, TEST_DATA.len());
    assert_eq!(&output[..written], TEST_DATA);
}

/// Test encoder state after multiple operations.
#[test]
fn encoder_state_consistency() {
    let mut encoder = Stream::default()
        .easy_encoder(Compression::Level3, IntegrityCheck::Crc64)
        .unwrap();

    // Initial state
    assert!(!encoder.is_finished());
    assert_eq!(encoder.total_in(), 0);
    assert_eq!(encoder.total_out(), 0);

    let mut output = vec![0u8; 2048];

    // First chunk
    let chunk1 = &TEST_DATA[..10];
    let (read1, written1) = encoder.process(chunk1, &mut output, Action::Run).unwrap();
    assert_eq!(read1, chunk1.len());
    assert_eq!(encoder.total_in(), chunk1.len() as u64);
    assert_eq!(encoder.total_out(), written1 as u64);
    assert!(!encoder.is_finished());

    // Second chunk
    let chunk2 = &TEST_DATA[10..];
    let (read2, written2) = encoder
        .process(chunk2, &mut output[written1..], Action::Run)
        .unwrap();
    assert_eq!(read2, chunk2.len());
    assert_eq!(encoder.total_in(), TEST_DATA.len() as u64);
    assert_eq!(encoder.total_out(), (written1 + written2) as u64);
    assert!(!encoder.is_finished());

    // Finish
    let (read3, written3) = encoder
        .process(&[], &mut output[written1 + written2..], Action::Finish)
        .unwrap();
    assert_eq!(read3, 0);
    assert_eq!(encoder.total_in(), TEST_DATA.len() as u64);
    assert_eq!(encoder.total_out(), (written1 + written2 + written3) as u64);
    assert!(encoder.is_finished());
}
