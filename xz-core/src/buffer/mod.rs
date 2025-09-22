//! Memory buffer utilities with customizable allocation strategies.

mod allocator;
mod deallocator;
mod raw;

#[cfg(test)]
mod tests;

pub use allocator::{Allocator, GlobalAllocator};
pub use deallocator::{Deallocator, DeallocatorFn};
pub use raw::Buffer;
