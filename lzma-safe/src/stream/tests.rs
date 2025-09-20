use std::os::raw::c_void;

use crate::decoder::options::Flags;
use crate::encoder::options::{Compression, IntegrityCheck};

use super::*;

#[derive(Default)]
struct DummyAllocator;

impl Allocator for DummyAllocator {
    fn alloc(&self, _nmemb: usize, _size: usize) -> *mut c_void {
        std::ptr::null_mut()
    }

    unsafe fn free(&self, _ptr: *mut c_void) {}
}

/// Test buffer helper methods update lengths correctly.
#[test]
fn next_buffer_helpers_update_lengths() {
    let mut stream = Stream::default();
    assert_eq!(stream.avail_in(), 0);
    assert_eq!(stream.avail_out(), 0);

    let input = b"abcd";
    stream.set_next_input(input);
    assert_eq!(stream.avail_in(), input.len());

    let mut out = [0u8; 16];
    stream.set_next_out(&mut out);
    assert_eq!(stream.avail_out(), out.len());

    let mut empty: [u8; 0] = [];
    stream.set_next_input(&empty);
    assert_eq!(stream.avail_in(), 0);
    stream.set_next_out(&mut empty);
    assert_eq!(stream.avail_out(), 0);
}

/// Test multithreaded decoder sets thread count correctly.
#[test]
fn multithreaded_decoder_from_stream_sets_threads() {
    let decoder = Stream::default()
        .mt_decoder(u64::MAX, Flags::empty(), 4)
        .expect("failed to create mt decoder");
    assert_eq!(decoder.threads(), 4);
}

/// Test custom allocator is properly stored in stream.
#[test]
fn custom_allocator_is_stored_in_stream() {
    let allocator = std::sync::Arc::new(DummyAllocator);
    let stream = Stream::with_allocator(Some(allocator));

    assert!(stream.allocator.is_some());
    assert!(!stream.inner.allocator.is_null());
}

/// Test stream initialization with default values.
#[test]
fn stream_initialization_defaults() {
    let stream = Stream::default();

    assert_eq!(stream.avail_in(), 0);
    assert_eq!(stream.avail_out(), 0);
    assert_eq!(stream.total_in(), 0);
    assert_eq!(stream.total_out(), 0);
    assert!(stream.allocator.is_none());
    assert!(stream.inner.allocator.is_null());
    assert!(stream.inner.next_in.is_null());
    assert!(stream.inner.next_out.is_null());
    assert!(stream.inner.internal.is_null());
}

/// Test stream creation with allocator works correctly.
#[test]
fn stream_with_allocator_creation() {
    let stream = Stream::with_allocator(None);

    assert_eq!(stream.avail_in(), 0);
    assert_eq!(stream.avail_out(), 0);
    assert!(stream.allocator.is_none());

    // Test with allocator
    let allocator = std::sync::Arc::new(DummyAllocator);
    let stream_with_alloc = Stream::with_allocator(Some(allocator));

    assert!(stream_with_alloc.allocator.is_some());
    assert!(!stream_with_alloc.inner.allocator.is_null());
}

/// Test buffer state management during processing.
#[test]
fn buffer_state_management() {
    let mut stream = Stream::default();

    // Test initial state
    assert_eq!(stream.avail_in(), 0);
    assert_eq!(stream.avail_out(), 0);

    // Set input buffer
    let input = b"test input data for compression";
    stream.set_next_input(input);
    assert_eq!(stream.avail_in(), input.len());
    assert_eq!(stream.inner.next_in, input.as_ptr());

    // Set output buffer
    let mut output = vec![0u8; 1024];
    stream.set_next_out(&mut output);
    assert_eq!(stream.avail_out(), output.len());
    assert_eq!(stream.inner.next_out, output.as_mut_ptr());

    // Test with empty buffers
    let empty: &[u8] = &[];
    stream.set_next_input(empty);
    assert_eq!(stream.avail_in(), 0);
    assert!(stream.inner.next_in.is_null());

    let empty_out: &mut [u8] = &mut [];
    stream.set_next_out(empty_out);
    assert_eq!(stream.avail_out(), 0);
    assert!(stream.inner.next_out.is_null());
}

/// Test encoder creation from stream.
#[test]
fn encoder_creation_from_stream() {
    let stream = Stream::default();
    let encoder = stream.easy_encoder(Compression::default(), IntegrityCheck::Crc64);

    assert!(encoder.is_ok());
}

/// Test decoder creation from stream.
#[test]
fn decoder_creation_from_stream() {
    let stream = Stream::default();
    let decoder = stream.decoder(u64::MAX, Flags::empty());

    assert!(decoder.is_ok());
}

/// Test auto decoder creation from stream.
#[test]
fn auto_decoder_creation_from_stream() {
    let stream = Stream::default();
    let decoder = stream.auto_decoder(u64::MAX, Flags::empty());

    assert!(decoder.is_ok());
}

/// Test alone decoder creation from stream.
#[test]
fn alone_decoder_creation_from_stream() {
    let stream = Stream::default();
    let decoder = stream.alone_decoder(u64::MAX);

    assert!(decoder.is_ok());
}

/// Test multithreaded encoder creation with different thread counts.
#[test]
fn multithreaded_encoder_creation() {
    // Test with explicit thread count
    let stream = Stream::default();
    let encoder = stream.multithreaded_encoder(Compression::default(), IntegrityCheck::Crc64, 4);
    assert!(encoder.is_ok());

    // Test with zero threads (should default to 1)
    let stream2 = Stream::default();
    let encoder2 = stream2.multithreaded_encoder(Compression::default(), IntegrityCheck::Crc64, 0);
    assert!(encoder2.is_ok());
}

/// Test stream with different memory limits for decoders.
#[test]
fn decoder_with_memory_limits() {
    // Test with reasonable memory limit
    let stream1 = Stream::default();
    let decoder1 = stream1.decoder(1024 * 1024, Flags::empty());
    assert!(decoder1.is_ok());

    // Test with very low memory limit (might fail depending on implementation)
    let stream2 = Stream::default();
    let _decoder2 = stream2.decoder(1024, Flags::empty());
    // Note: This might fail or succeed depending on liblzma implementation

    // Test with maximum memory limit
    let stream3 = Stream::default();
    let decoder3 = stream3.decoder(u64::MAX, Flags::empty());
    assert!(decoder3.is_ok());
}

/// Test stream behavior with large buffers.
#[test]
fn large_buffer_handling() {
    let mut stream = Stream::default();

    // Test with large input buffer
    let large_input = vec![0u8; 1024 * 1024]; // 1MB
    stream.set_next_input(&large_input);
    assert_eq!(stream.avail_in(), large_input.len());

    // Test with large output buffer
    let mut large_output = vec![0u8; 2 * 1024 * 1024]; // 2MB
    stream.set_next_out(&mut large_output);
    assert_eq!(stream.avail_out(), large_output.len());
}

/// Test stream state consistency after multiple buffer operations.
#[test]
fn stream_state_consistency() {
    let mut stream = Stream::default();

    // Multiple buffer set operations
    for i in 0..10 {
        let input = vec![u8::try_from(i).unwrap(); i * 10 + 1];
        stream.set_next_input(&input);
        assert_eq!(stream.avail_in(), input.len());

        let mut output = vec![0u8; i * 20 + 10];
        stream.set_next_out(&mut output);
        assert_eq!(stream.avail_out(), output.len());

        // Verify total counters remain consistent
        assert_eq!(stream.total_in(), 0); // No actual processing happened
        assert_eq!(stream.total_out(), 0);
    }
}
