//! Infrastructure for providing custom allocators to liblzma.

use std::os::raw::c_void;
use std::sync::Arc;

/// Trait for custom memory allocators compatible with liblzma.
pub trait Allocator: Send + Sync + 'static {
    /// Allocate a block of memory for `nmemb` elements of `size` bytes each.
    ///
    /// Returns a pointer to the allocated memory, or null on failure or overflow.
    fn alloc(&self, nmemb: usize, size: usize) -> *mut c_void;

    /// Free a previously allocated memory block.
    ///
    /// # Safety
    ///
    /// The pointer must have been returned by `alloc` and not already freed.
    unsafe fn free(&self, ptr: *mut c_void);
}

/// Standard allocator using `libc::malloc`/`free`.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdAllocator;

impl Allocator for StdAllocator {
    fn alloc(&self, nmemb: usize, size: usize) -> *mut c_void {
        // Return null if multiplication overflows or allocation fails.
        match nmemb.checked_mul(size) {
            Some(total) if total > 0 => unsafe { libc::malloc(total) },
            _ => std::ptr::null_mut(),
        }
    }

    unsafe fn free(&self, ptr: *mut c_void) {
        // Free only non-null pointers, as required by C standard.
        if !ptr.is_null() {
            libc::free(ptr);
        }
    }
}

/// RAII wrapper for a liblzma-compatible allocator.
pub struct LzmaAllocator {
    /// The C allocator structure passed to liblzma.
    inner: liblzma_sys::lzma_allocator,
    /// Keep the allocator alive. None for default allocator.
    _allocator_box: Option<Box<Arc<dyn Allocator>>>,
}

impl Default for LzmaAllocator {
    fn default() -> Self {
        Self {
            inner: liblzma_sys::lzma_allocator {
                alloc: None, // Use liblzma's default malloc
                free: None,  // Use liblzma's default free
                opaque: std::ptr::null_mut(),
            },
            _allocator_box: None,
        }
    }
}

impl LzmaAllocator {
    /// Construct a new `LzmaAllocator` from a custom Rust allocator.
    ///
    /// The allocator will be boxed and kept alive for the lifetime of this wrapper.
    /// The returned structure can be passed to liblzma.
    pub fn from_allocator(allocator: Arc<dyn Allocator>) -> Self {
        // Box the Arc to keep it alive and get a stable pointer
        let boxed_allocator = Box::new(allocator);
        let opaque_ptr =
            std::ptr::from_ref::<Arc<dyn Allocator>>(Box::as_ref(&boxed_allocator)) as *mut c_void;

        Self {
            inner: liblzma_sys::lzma_allocator {
                alloc: Some(c_alloc_wrapper),
                free: Some(c_free_wrapper),
                opaque: opaque_ptr,
            },
            _allocator_box: Some(boxed_allocator),
        }
    }

    /// Return a const pointer to the wrapped allocator.
    pub fn as_ptr(&self) -> *const liblzma_sys::lzma_allocator {
        std::ptr::from_ref(&self.inner)
    }

    /// Return a mutable pointer to the wrapped allocator.
    pub fn as_mut_ptr(&mut self) -> *mut liblzma_sys::lzma_allocator {
        std::ptr::from_mut(&mut self.inner)
    }
}

impl Clone for LzmaAllocator {
    fn clone(&self) -> Self {
        match &self._allocator_box {
            None => {
                // Default allocator - just create a new one
                Self::default()
            }
            Some(boxed_arc) => {
                // Clone the Arc to share the allocator
                let allocator_arc = Arc::clone(boxed_arc.as_ref());
                Self::from_allocator(allocator_arc)
            }
        }
    }
}

impl Drop for LzmaAllocator {
    fn drop(&mut self) {
        // The Box<Arc<dyn Allocator>> will be dropped automatically,
        // which is safe since we control the lifetime
    }
}

/// C-compatible allocation wrapper for liblzma.
extern "C" fn c_alloc_wrapper(opaque: *mut c_void, nmemb: usize, size: usize) -> *mut c_void {
    if opaque.is_null() {
        return std::ptr::null_mut();
    }

    // Safety: opaque is a valid pointer to a Box<Arc<dyn Allocator>>
    // The Box is kept alive by LzmaAllocator, so this is safe to dereference.
    let allocator_arc = unsafe { &*(opaque as *const Arc<dyn Allocator>) };
    allocator_arc.alloc(nmemb, size)
}

/// C-compatible free wrapper for liblzma.
extern "C" fn c_free_wrapper(opaque: *mut c_void, ptr: *mut c_void) {
    if opaque.is_null() {
        return;
    }

    // Safety: opaque is a valid pointer to a Box<Arc<dyn Allocator>>
    // The Box is kept alive by LzmaAllocator, so this is safe to dereference.
    let allocator_arc = unsafe { &*(opaque as *const Arc<dyn Allocator>) };
    unsafe { allocator_arc.free(ptr) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test allocator that tracks allocation statistics
    #[derive(Debug)]
    struct CountingAllocator {
        alloc_count: AtomicUsize,
        free_count: AtomicUsize,
    }

    impl CountingAllocator {
        fn new() -> Self {
            Self {
                alloc_count: AtomicUsize::new(0),
                free_count: AtomicUsize::new(0),
            }
        }

        fn alloc_count(&self) -> usize {
            self.alloc_count.load(Ordering::Relaxed)
        }

        fn free_count(&self) -> usize {
            self.free_count.load(Ordering::Relaxed)
        }
    }

    impl Allocator for CountingAllocator {
        fn alloc(&self, nmemb: usize, size: usize) -> *mut c_void {
            self.alloc_count.fetch_add(1, Ordering::Relaxed);
            match nmemb.checked_mul(size) {
                Some(total) if total > 0 => unsafe { libc::malloc(total) },
                _ => std::ptr::null_mut(),
            }
        }

        unsafe fn free(&self, ptr: *mut c_void) {
            if !ptr.is_null() {
                self.free_count.fetch_add(1, Ordering::Relaxed);
                libc::free(ptr);
            }
        }
    }

    /// Test standard allocator basic functionality.
    #[test]
    fn test_std_allocator() {
        let allocator = StdAllocator;

        // Test normal allocation
        let ptr = allocator.alloc(1, 100);
        assert!(!ptr.is_null());
        unsafe { allocator.free(ptr) };

        // Test overflow protection
        let ptr = allocator.alloc(usize::MAX, 2);
        assert!(ptr.is_null());

        // Test zero size
        let ptr = allocator.alloc(0, 100);
        assert!(ptr.is_null());
    }

    /// Test custom allocator wrapper functionality.
    #[test]
    fn test_custom_allocator() {
        let counting_allocator = Arc::new(CountingAllocator::new());
        let lzma_allocator = LzmaAllocator::from_allocator(counting_allocator.clone());

        // Simulate liblzma calling our allocator
        let ptr =
            unsafe { (lzma_allocator.inner.alloc.unwrap())(lzma_allocator.inner.opaque, 1, 100) };

        assert!(!ptr.is_null());
        assert_eq!(counting_allocator.alloc_count(), 1);

        unsafe {
            (lzma_allocator.inner.free.unwrap())(lzma_allocator.inner.opaque, ptr);
        }

        assert_eq!(counting_allocator.free_count(), 1);
    }

    /// Test default [`LzmaAllocator`] functionality.
    #[test]
    fn test_default_lzma_allocator() {
        let allocator = LzmaAllocator::default();

        // Default allocator should have null function pointers
        assert!(allocator.inner.alloc.is_none());
        assert!(allocator.inner.free.is_none());
        assert!(allocator.inner.opaque.is_null());
    }

    /// Test [`StdAllocator`] free with null pointer.
    #[test]
    fn test_std_allocator_null_free() {
        let allocator = StdAllocator;

        // Should be safe to free null pointer
        unsafe { allocator.free(std::ptr::null_mut()) };
    }

    /// Test C wrapper functions with null opaque pointer.
    #[test]
    fn test_c_wrappers_null_opaque() {
        // Test alloc wrapper with null opaque
        let result = c_alloc_wrapper(std::ptr::null_mut(), 1, 100);
        assert!(result.is_null());

        // Test free wrapper with null opaque - should not crash
        c_free_wrapper(std::ptr::null_mut(), std::ptr::null_mut());
    }

    /// Test allocator with zero nmemb but non-zero size.
    #[test]
    fn test_allocator_zero_nmemb() {
        let allocator = StdAllocator;

        // Zero nmemb should return null
        let ptr = allocator.alloc(0, 100);
        assert!(ptr.is_null());

        // Test with custom allocator too
        let counting_allocator = Arc::new(CountingAllocator::new());
        let ptr = counting_allocator.alloc(0, 100);
        assert!(ptr.is_null());
        assert_eq!(counting_allocator.alloc_count(), 1); // Should still increment counter
    }

    /// Test [`LzmaAllocator`] pointer methods.
    #[test]
    fn test_lzma_allocator_pointers() {
        let mut allocator = LzmaAllocator::default();

        // Test const pointer
        let const_ptr = allocator.as_ptr();
        assert!(!const_ptr.is_null());

        // Test mutable pointer
        let mut_ptr = allocator.as_mut_ptr();
        assert!(!mut_ptr.is_null());

        // Pointers should point to the same location
        assert_eq!(const_ptr.cast::<u8>(), mut_ptr as *const u8);
    }

    /// Test custom allocator edge cases.
    #[test]
    fn test_custom_allocator_edge_cases() {
        let counting_allocator = Arc::new(CountingAllocator::new());
        let lzma_allocator = LzmaAllocator::from_allocator(counting_allocator.clone());

        // Test overflow case through C wrapper
        let ptr = unsafe {
            (lzma_allocator.inner.alloc.unwrap())(lzma_allocator.inner.opaque, usize::MAX, 2)
        };
        assert!(ptr.is_null());
        assert_eq!(counting_allocator.alloc_count(), 1); // Should still increment

        // Test zero size case through C wrapper
        let ptr =
            unsafe { (lzma_allocator.inner.alloc.unwrap())(lzma_allocator.inner.opaque, 1, 0) };
        assert!(ptr.is_null());
        assert_eq!(counting_allocator.alloc_count(), 2); // Should increment again

        // Test free with null pointer through C wrapper
        unsafe {
            (lzma_allocator.inner.free.unwrap())(lzma_allocator.inner.opaque, std::ptr::null_mut());
        }
        // free_count should not increment for null pointer
        assert_eq!(counting_allocator.free_count(), 0);
    }

    /// Test that [`LzmaAllocator`] properly manages allocator lifetime.
    #[test]
    fn test_allocator_lifetime() {
        let counting_allocator = Arc::new(CountingAllocator::new());
        let weak_ref = Arc::downgrade(&counting_allocator);

        // Create allocator in a scope
        {
            let _lzma_allocator = LzmaAllocator::from_allocator(counting_allocator);
            // Allocator should still be alive
            assert!(weak_ref.upgrade().is_some());
        }
        // After scope, the boxed Arc should be dropped, but original Arc might still exist
        // This test mainly ensures no crashes occur during cleanup
    }
}
