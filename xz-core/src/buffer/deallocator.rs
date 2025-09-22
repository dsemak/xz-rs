//! Buffer deallocation strategies for custom memory management.

use std::ptr::NonNull;

/// Trait defining deallocation logic for buffers produced by a custom allocator.
pub trait Deallocator: Send + Sync {
    /// Deallocates a buffer at the specified pointer with the given capacity.
    ///
    /// # Parameters
    ///
    /// * `ptr` - Non-null pointer to the buffer to deallocate
    /// * `capacity` - Original capacity of the buffer in bytes
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    ///
    /// - The pointer was allocated by the corresponding allocator
    /// - The capacity matches the original allocation size
    /// - The pointer is not used after this call
    fn deallocate(&self, ptr: NonNull<u8>, capacity: usize);
}

/// Blanket implementation allowing closures to act as deallocators.
impl<T> Deallocator for T
where
    T: Fn(NonNull<u8>, usize) + Send + Sync,
{
    fn deallocate(&self, ptr: NonNull<u8>, capacity: usize) {
        self(ptr, capacity);
    }
}

/// Deallocator that reconstructs and drops a `Vec<u8>` to release memory.
///
/// # Parameters
///
/// * `ptr` - Non-null pointer to the buffer to deallocate.
/// * `capacity` - Original capacity of the buffer in bytes.
///
/// # Safety
///
/// The pointer must have been allocated by `Vec<u8>` with the specified capacity.
pub fn vec_deallocator(ptr: NonNull<u8>, capacity: usize) {
    // SAFETY: The caller guarantees that ptr was allocated by Vec<u8> with
    // the specified capacity. Reconstructing with zero length ensures no
    // invalid memory access while preserving the allocation metadata.
    unsafe {
        let _ = Vec::from_raw_parts(ptr.as_ptr(), 0, capacity);
    }
}

/// No-operation deallocator that performs no cleanup.
pub fn noop_deallocator(_ptr: NonNull<u8>, _capacity: usize) {
    // Intentionally empty - no deallocation performed
}

/// Type alias for trait objects implementing the `Deallocator` trait.
pub type DeallocatorFn = dyn Deallocator;
