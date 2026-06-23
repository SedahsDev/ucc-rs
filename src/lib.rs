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
//!
//! # Module Overview
//!
//! * [`lib_init`] — Initialize and finalize the UCC library (`UccLib`).
//! * [`context`] — Create and manage communication contexts (`UccContext`).
//! * [`team`] — Create and manage teams of processes (`UccTeam`).
//! * [`collective`] — Run collective operations (barrier, bcast, allreduce, etc.).
//! * [`memory`] — Map host memory for use in collectives (`UccMemHandle`).
//! * [`event_engine`] — Event-driven execution engine for triggered collectives.
//! * [`status`] — Status codes and error handling (`UccError`, `UccStatus`).

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
