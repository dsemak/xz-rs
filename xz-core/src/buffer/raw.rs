//! Raw buffer implementation with custom memory management.

use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::Arc;

use crate::error::Error;
use crate::error::Result;

use super::allocator::{Allocator, GlobalAllocator};
use super::deallocator::{noop_deallocator, vec_deallocator, Deallocator};

/// A memory buffer with custom allocation and deallocation strategies.
///
/// [`Buffer`] provides a safe wrapper around raw memory that was allocated by
/// a custom allocator. It ensures proper cleanup through RAII and supports
/// zero-copy operations for efficient memory management in XZ operations.
pub struct Buffer {
    /// Non-null pointer to the allocated memory region.
    ptr: NonNull<u8>,
    /// Total capacity of the buffer in bytes.
    capacity: usize,
    /// Strategy for deallocating the memory when the buffer is dropped.
    deallocator: Arc<dyn Deallocator>,
}

impl Buffer {
    /// Creates a buffer handle from raw memory components.
    ///
    /// # Parameters
    ///
    /// * `ptr` - Non-null pointer to the allocated memory
    /// * `capacity` - Size of the allocation in bytes
    /// * `deallocator` - Strategy for releasing the memory when dropped
    ///
    /// # Safety
    ///
    /// The caller must guarantee that:
    ///
    /// - `ptr` points to a valid allocation of at least `capacity` bytes
    /// - The memory was allocated by a method compatible with `deallocator`
    /// - The allocation follows Rust's aliasing rules for the buffer's lifetime
    /// - No other code will access or deallocate this memory
    pub unsafe fn from_raw_parts(
        ptr: NonNull<u8>,
        capacity: usize,
        deallocator: Arc<dyn Deallocator>,
    ) -> Self {
        Self {
            ptr,
            capacity,
            deallocator,
        }
    }

    /// Allocates a buffer with the specified capacity using a custom allocator.
    ///
    /// # Parameters
    ///
    /// * `allocator` - The allocator to use for memory allocation
    /// * `capacity` - Number of bytes to allocate
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if allocation fails.
    ///
    /// # Returns
    ///
    /// A new [`Buffer`] with the requested capacity, or an error if allocation fails.
    pub fn with_allocator<A: Allocator>(allocator: &A, capacity: usize) -> Result<Self> {
        allocator.allocate(capacity)
    }

    /// Allocates a buffer using the global system allocator.
    ///
    /// # Parameters
    ///
    /// * `capacity` - Number of bytes to allocate
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if allocation fails.
    ///
    /// # Returns
    ///
    /// A new [`Buffer`] with the requested capacity, or an error if allocation fails.
    pub fn new(capacity: usize) -> Result<Self> {
        Self::with_allocator(&GlobalAllocator, capacity)
    }

    /// Returns the buffer's capacity in bytes.
    ///
    /// This represents the total amount of memory allocated for the buffer,
    /// which may be larger than the amount of data currently stored.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns an immutable slice view of the entire buffer.
    ///
    /// The slice spans the full capacity of the buffer. Callers should be
    /// aware that the buffer may contain uninitialized memory.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: The buffer was constructed with a valid pointer and capacity,
        // and we maintain exclusive access through Rust's ownership system.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.capacity) }
    }

    /// Returns a mutable slice view of the entire buffer.
    ///
    /// The slice spans the full capacity of the buffer. Callers should be
    /// aware that the buffer may contain uninitialized memory.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: The buffer was constructed with a valid pointer and capacity,
        // and we maintain exclusive access through Rust's ownership system.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity) }
    }

    /// Converts a [`Vec<u8>`] into a [`Buffer`] without copying data.
    ///
    /// This operation transfers ownership of the vector's memory to the buffer.
    /// The vector's length information is lost, and the buffer will have a
    /// capacity equal to the vector's capacity.
    ///
    /// # Parameters
    ///
    /// * `vec` - The vector to convert.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if allocation fails.
    ///
    /// # Returns
    ///
    /// A [`Buffer`] backed by the vector's memory allocation.
    ///
    /// # Safety
    ///
    /// After conversion, the caller must ensure that only the initialized
    /// portion of the buffer (up to the original vector's length) is read
    /// until additional initialization occurs.
    pub unsafe fn from_vec(vec: Vec<u8>) -> Result<Self> {
        // Handle zero-capacity vectors without allocation
        if vec.capacity() == 0 {
            return Ok(Self::default());
        }

        // Extract raw parts from the vector without dropping it
        let mut vec = ManuallyDrop::new(vec);
        let ptr = NonNull::new(vec.as_mut_ptr()).ok_or_else(|| Error::AllocationFailed {
            capacity: vec.capacity(),
        })?;

        // Create deallocator that can properly release Vec-allocated memory
        let deallocator = Arc::new(vec_deallocator);

        Ok(unsafe { Self::from_raw_parts(ptr, vec.capacity(), deallocator) })
    }
}

impl Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl Default for Buffer {
    /// Creates an empty buffer with zero capacity.
    ///
    /// The default buffer uses a dangling pointer and no-op deallocator,
    /// making it safe to drop without performing any actual deallocation.
    fn default() -> Self {
        let deallocator = Arc::new(noop_deallocator);
        // SAFETY: Zero-capacity buffer with dangling pointer is safe because
        // no memory access will occur and the no-op deallocator won't attempt
        // to free any memory.
        unsafe { Self::from_raw_parts(NonNull::dangling(), 0, deallocator) }
    }
}

// SAFETY: Buffer can be safely sent between threads because:
//
// - The contained pointer is owned exclusively by this buffer
// - The deallocator is required to be Send + Sync
// - No shared mutable state exists
unsafe impl Send for Buffer {}

// SAFETY: Buffer can be safely shared between threads because:
//
// - Immutable access to the buffer data is thread-safe
// - The deallocator is required to be Send + Sync
// - The buffer maintains exclusive ownership of its memory
unsafe impl Sync for Buffer {}

impl Drop for Buffer {
    /// Releases the buffer's memory using the associated deallocator.
    ///
    /// This ensures that memory allocated by custom allocators is properly
    /// released according to their specific requirements.
    fn drop(&mut self) {
        self.deallocator.deallocate(self.ptr, self.capacity);
    }
}
