//! Small shared utilities for CLI binaries.
//!
//! This module groups helper submodules that are reused across multiple `xz-cli`
//! entrypoints but don't belong to the higher-level CLI orchestration layers.

pub mod argfiles;

pub(crate) mod bytes;
pub(crate) mod math;
