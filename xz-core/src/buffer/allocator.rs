//! Buffer allocation strategies backed by Rust's global allocator.

use std::alloc::{self, Layout};
use std::ptr::NonNull;
use std::sync::Arc;

use crate::error::{Error, Result};

use super::{deallocator::Deallocator, raw::Buffer};

/// Trait for allocating scratch buffers used in XZ compression/decompression operations.
pub trait Allocator: Send + Sync {
    /// Allocates a buffer with the specified capacity.
    ///
    /// # Parameters
    ///
    /// * `capacity` - The number of bytes to allocate
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if allocation fails.
    ///
    /// # Returns
    ///
    /// A [`Buffer`] instance with the requested capacity, or an error if allocation fails.
    /// For zero capacity, returns an empty buffer without performing allocation.
    fn allocate(&self, capacity: usize) -> Result<Buffer>;
}

/// Standard memory allocator implementation using Rust's global allocator.
#[derive(Clone, Copy, Default)]
pub struct GlobalAllocator;

/// Deallocator that remembers the original layout used for allocation.
struct LayoutDeallocator {
    layout: Layout,
}

impl Deallocator for LayoutDeallocator {
    fn deallocate(&self, ptr: NonNull<u8>, _capacity: usize) {
        // SAFETY: The pointer was allocated with this exact layout, and we're
        // deallocating it with the same layout. The capacity parameter is
        // ignored since the layout already contains the size information.
        unsafe {
            alloc::dealloc(ptr.as_ptr(), self.layout);
        }
    }
}

impl Allocator for GlobalAllocator {
    fn allocate(&self, capacity: usize) -> Result<Buffer> {
        // Handle zero-capacity allocation without calling the allocator
        if capacity == 0 {
            return Ok(Buffer::default());
        }

        // Create layout for byte array allocation, handling potential overflow
        let layout =
            Layout::array::<u8>(capacity).map_err(|_| Error::AllocationFailed { capacity })?;

        // Perform the actual allocation using the global allocator
        let ptr = unsafe { alloc::alloc(layout) };
        let Some(ptr) = NonNull::new(ptr) else {
            // If allocation returns null, trigger the global allocation error handler
            alloc::handle_alloc_error(layout);
        };

        // Zero-initialize the buffer to provide consistent behavior with Vec-based
        // buffers and prevent information leakage from previous allocations
        unsafe {
            ptr.as_ptr().write_bytes(0, capacity);
        }

        // Create a deallocator that remembers the layout for proper cleanup
        let deallocator = Arc::new(LayoutDeallocator { layout }) as Arc<dyn Deallocator>;

        // SAFETY: We just allocated this pointer with the specified capacity,
        // and we're providing a valid deallocator that will clean it up properly
        let buffer = unsafe { Buffer::from_raw_parts(ptr, capacity, deallocator) };
        Ok(buffer)
    }
}
