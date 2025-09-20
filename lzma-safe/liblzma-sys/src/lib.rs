//! Low-level FFI bindings for liblzma (XZ Utils).
//!
//! This crate provides raw FFI bindings to the liblzma C library.
//! For a safe, idiomatic Rust wrapper, use the `lzma-safe` crate.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

// Include the generated bindings when the `bindgen` feature is enabled.
#[cfg(feature = "bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// When the `bindgen` feature is disabled, use pre-generated bindings.
// This is typically used for pre-generated bindings or when bindgen is not available.
#[cfg(not(feature = "bindgen"))]
include!("lzma_bindings.rs");
