//! ucc-rs — Safe Rust bindings for the Unified Collective Communication (UCC) library.
//!
//! This crate provides idiomatic Rust wrappers around the UCC C API with:
//! * RAII resource management (auto-cleanup on drop)
//! * Type-safe enums for status codes, collectives, and datatypes
//! * Builder patterns for collective operations
//!
//! # Quick Start
//!
//! ```no_run
//! use ucc_rs::UccLib;
//!
//! let lib = UccLib::init().unwrap();
//! // lib is automatically finalized when dropped
//! ```

// Auto-generated FFI bindings from ucc.h
mod bindings;

// Re-export status types at crate root for ergonomic access
pub use status::{check_status, UccError, UccStatus};

// Core modules
pub mod collective;
pub mod context;
pub mod event_engine;
pub mod lib_init;
pub mod memory;
pub mod status;
pub mod team;
