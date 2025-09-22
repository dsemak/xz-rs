//! Unit tests covering buffer allocation and memory management behaviors.

use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use crate::error::Result;

use super::{Allocator, Buffer};

#[test]
/// Verify that Buffer can allocate memory using the global system allocator.
fn buffer_can_allocate_with_global_allocator() {
    let buf = Buffer::new(1024).unwrap();
    assert_eq!(buf.capacity(), 1024);
}

#[test]
/// Test that zero-capacity allocation returns an empty but valid buffer.
fn global_allocator_zero_capacity_returns_empty() {
    let buf = Buffer::new(0).unwrap();
    assert_eq!(buf.capacity(), 0);
    assert_eq!(buf.as_slice(), &[]);
}

#[test]
/// Verify that custom deallocators are properly called when buffers are dropped.
fn custom_deallocator_is_invoked_on_drop() {
    struct Pool {
        released: Mutex<usize>,
    }

    impl Pool {
        fn new() -> Self {
            Self {
                released: Mutex::new(0),
            }
        }
    }

    impl Allocator for Arc<Pool> {
        fn allocate(&self, capacity: usize) -> Result<Buffer> {
            if capacity == 0 {
                return Ok(Buffer::default());
            }

            let data = vec![0u8; capacity];

            let mut data = ManuallyDrop::new(data);
            let ptr = NonNull::new(data.as_mut_ptr()).unwrap();
            let len = data.capacity();

            let pool = Arc::clone(self);
            let deallocator: Arc<dyn super::Deallocator> =
                Arc::new(move |ptr: NonNull<u8>, len: usize| {
                    super::deallocator::vec_deallocator(ptr, len);
                    let mut released = pool.released.lock().unwrap();
                    *released += len;
                });

            let buffer = unsafe { Buffer::from_raw_parts(ptr, len, deallocator) };
            Ok(buffer)
        }
    }

    let pool = Arc::new(Pool::new());
    {
        let _buf = Buffer::with_allocator(&pool, 512).unwrap();
    }

    assert_eq!(*pool.released.lock().unwrap(), 512);
}

#[test]
/// Test that converting Vec to Buffer preserves the original capacity.
fn buffer_from_vec_preserves_capacity() {
    let vec = vec![1, 2, 3, 4, 5];
    let capacity = vec.capacity();

    let buffer = unsafe { Buffer::from_vec(vec).unwrap() };
    assert_eq!(buffer.capacity(), capacity);
    assert!(buffer.capacity() >= 5);
}

#[test]
/// Test that empty Vec creates a default (zero-capacity) buffer.
fn buffer_from_empty_vec_creates_default() {
    let vec = Vec::new();
    let buffer = unsafe { Buffer::from_vec(vec).unwrap() };
    assert_eq!(buffer.capacity(), 0);
    assert_eq!(buffer.as_slice(), &[]);
}

#[test]
/// Test Vec with reserved capacity converts correctly to Buffer.
fn buffer_from_vec_with_reserved_capacity() {
    let mut vec = Vec::with_capacity(100);
    vec.extend_from_slice(&[1, 2, 3]);
    let expected_capacity = vec.capacity();

    let buffer = unsafe { Buffer::from_vec(vec).unwrap() };
    assert_eq!(buffer.capacity(), expected_capacity);
    // Note: Only first 3 bytes are guaranteed to be initialized
    assert_eq!(buffer[0], 1);
    assert_eq!(buffer[1], 2);
    assert_eq!(buffer[2], 3);
}

#[test]
/// Test that Vec with zero capacity creates an empty buffer.
fn buffer_from_vec_zero_capacity() {
    let vec = Vec::with_capacity(0);
    let buffer = unsafe { Buffer::from_vec(vec).unwrap() };
    assert_eq!(buffer.capacity(), 0);
    assert_eq!(buffer.as_slice(), &[]);
}

#[test]
/// Test that default Buffer has zero capacity and empty slice.
fn buffer_default_has_zero_capacity() {
    let buffer = Buffer::default();
    assert_eq!(buffer.capacity(), 0);
    assert_eq!(buffer.as_slice(), &[]);
}

#[test]
/// Verify that default buffer can be safely dropped without panicking.
fn buffer_default_can_be_dropped_safely() {
    let buffer = Buffer::default();
    drop(buffer); // Should not panic
}

#[test]
/// Test slice operations on empty buffer work correctly.
fn empty_buffer_slice_operations() {
    let buffer = Buffer::default();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);

    // Test iterator
    assert_eq!(buffer.iter().count(), 0);
}

#[test]
/// Test that Buffer provides slice access through Deref/DerefMut traits.
fn buffer_deref_provides_slice_access() {
    let mut buf = Buffer::new(10).unwrap();

    // Test Deref
    let slice: &[u8] = &buf;
    assert_eq!(slice.len(), 10);

    // Test DerefMut
    let slice_mut: &mut [u8] = &mut buf;
    slice_mut[0] = 42;
    slice_mut[9] = 99;

    assert_eq!(buf[0], 42);
    assert_eq!(buf[9], 99);
}

#[test]
/// Verify that standard slice methods work on Buffer through Deref.
fn buffer_slice_methods_work() {
    let mut buf = Buffer::new(5).unwrap();
    buf[0] = 1;
    buf[1] = 2;
    buf[2] = 3;
    buf[3] = 4;
    buf[4] = 5;

    // Test slice operations through Deref
    assert_eq!(buf.len(), 5);
    assert_eq!(&buf[1..4], &[2, 3, 4]);
    assert_eq!(buf.first(), Some(&1));
    assert_eq!(buf.last(), Some(&5));
}

#[test]
/// Test mutable slice operations and modifications.
fn buffer_mutable_slice_operations() {
    let mut buf = Buffer::new(3).unwrap();

    // Test as_mut_slice
    {
        let slice = buf.as_mut_slice();
        slice[0] = 10;
        slice[1] = 20;
        slice[2] = 30;
    }

    assert_eq!(buf.as_slice(), &[10, 20, 30]);

    // Test through DerefMut
    buf.fill(255);
    assert_eq!(buf.as_slice(), &[255, 255, 255]);
}

#[test]
/// Compile-time test that Buffer implements Send trait.
fn buffer_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Buffer>();
}

#[test]
/// Compile-time test that Buffer implements Sync trait.
fn buffer_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<Buffer>();
}

#[test]
/// Test that Buffer can be moved between threads safely.
fn buffer_can_be_sent_between_threads() {
    use std::thread;

    let mut buf = Buffer::new(100).unwrap();
    buf[0] = 42;

    let handle = thread::spawn(move || {
        assert_eq!(buf[0], 42);
        buf.capacity()
    });

    let capacity = handle.join().unwrap();
    assert_eq!(capacity, 100);
}

#[test]
/// Test that Buffer can be shared between threads using Arc.
fn buffer_can_be_shared_between_threads() {
    use std::sync::Arc;
    use std::thread;

    let buf = Arc::new(Buffer::new(10).unwrap());
    let buf_clone = Arc::clone(&buf);

    let handle = thread::spawn(move || buf_clone.capacity());

    let capacity1 = buf.capacity();
    let capacity2 = handle.join().unwrap();

    assert_eq!(capacity1, capacity2);
    assert_eq!(capacity1, 10);
}

#[test]
/// Test that extremely large allocation requests are handled gracefully.
fn global_allocator_handles_large_allocation() {
    // Test allocation that might fail on some systems
    let result = Buffer::new(usize::MAX);
    assert!(result.is_err());

    // Just verify it's an error, don't check the specific message
    // as it may vary between systems and allocator implementations
}

#[test]
/// Test that allocation overflow is properly handled and returns error.
fn allocation_overflow_is_handled() {
    use super::GlobalAllocator;

    // Try to allocate more than possible
    let result = Buffer::with_allocator(&GlobalAllocator, usize::MAX);
    assert!(result.is_err());
}

#[test]
/// Verify that zero-size allocation succeeds and creates valid empty buffer.
fn zero_allocation_succeeds() {
    let buf = Buffer::new(0).unwrap();
    assert_eq!(buf.capacity(), 0);
    assert!(buf.is_empty());

    // Should be safe to drop
    drop(buf);
}

#[test]
/// Test that multiple sequential allocations work correctly.
fn multiple_allocations_work() {
    let buffers: Vec<_> = (0..10).map(|i| Buffer::new(i * 100).unwrap()).collect();

    for (i, buf) in buffers.iter().enumerate() {
        assert_eq!(buf.capacity(), i * 100);
    }
}

#[test]
/// Test that `vec_deallocator` properly releases Vec-allocated memory.
fn vec_deallocator_works() {
    use super::deallocator::vec_deallocator;
    use std::mem::ManuallyDrop;
    use std::ptr::NonNull;

    let vec = vec![1, 2, 3, 4, 5];
    let capacity = vec.capacity();
    let mut vec = ManuallyDrop::new(vec);
    let ptr = NonNull::new(vec.as_mut_ptr()).unwrap();

    // This should not panic or leak memory
    vec_deallocator(ptr, capacity);
}

#[test]
/// Test that `noop_deallocator` safely does nothing.
fn noop_deallocator_does_nothing() {
    use super::deallocator::noop_deallocator;
    use std::ptr::NonNull;

    // Should not panic even with invalid pointer
    noop_deallocator(NonNull::dangling(), 1000);
}

#[test]
/// Test that custom closure-based deallocators work correctly.
fn custom_closure_deallocator() {
    use std::ptr::NonNull;
    use std::sync::{Arc, Mutex};

    let counter = Arc::new(Mutex::new(0));
    let counter_clone = Arc::clone(&counter);

    let deallocator: Arc<dyn super::Deallocator> =
        Arc::new(move |_ptr: NonNull<u8>, size: usize| {
            let mut count = counter_clone.lock().unwrap();
            *count += size;
        });

    // Test the deallocator
    deallocator.deallocate(NonNull::dangling(), 100);
    assert_eq!(*counter.lock().unwrap(), 100);

    deallocator.deallocate(NonNull::dangling(), 50);
    assert_eq!(*counter.lock().unwrap(), 150);
}

#[test]
/// Test allocation and access of reasonably large buffer (1MB).
fn large_buffer_allocation() {
    // Test allocation of reasonably large buffer (1MB)
    let size = 1024 * 1024;
    let buf = Buffer::new(size).unwrap();
    assert_eq!(buf.capacity(), size);

    // Test that we can write to the entire buffer
    let slice = buf.as_slice();
    assert_eq!(slice.len(), size);
}

#[test]
/// Stress test with multiple large buffer allocations.
fn stress_test_multiple_large_buffers() {
    // Test multiple large allocations
    let sizes = [1024, 4096, 16384, 65536];
    let mut buffers = Vec::new();

    for &size in &sizes {
        let buf = Buffer::new(size).unwrap();
        assert_eq!(buf.capacity(), size);
        buffers.push(buf);
    }

    // All buffers should still be valid
    for (i, buf) in buffers.iter().enumerate() {
        assert_eq!(buf.capacity(), sizes[i]);
    }
}

#[test]
/// Test that buffer data integrity is maintained across operations.
fn buffer_data_integrity() {
    let mut buf = Buffer::new(1000).unwrap();

    // Fill with pattern
    for (i, byte) in buf.iter_mut().enumerate() {
        *byte = u8::try_from(i % 256).unwrap();
    }

    // Verify pattern
    for (i, &byte) in buf.iter().enumerate() {
        assert_eq!(byte, u8::try_from(i % 256).unwrap());
    }
}

#[test]
/// Test that `GlobalAllocator` zero-initializes memory.
fn buffer_zero_initialization() {
    // GlobalAllocator should zero-initialize memory
    let buf = Buffer::new(100).unwrap();

    // All bytes should be zero
    for &byte in buf.iter() {
        assert_eq!(byte, 0);
    }
}
