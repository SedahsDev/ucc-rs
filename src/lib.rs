// ucc-rs: Safe Rust bindings for UCC (Unified Collective Communication)
//
// UCC provides native collective operations built on top of UCX.
// This crate provides both FFI bindings (auto-generated via bindgen) and
// safe Rust wrappers with RAII lifetime management.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

// Auto-generated FFI bindings (written to src/bindings.rs by build.rs)
#[path = "bindings.rs"]
mod bindings;
pub use bindings::*;

// Safe wrapper modules
pub mod collective;
pub mod context;
pub mod event_engine;
pub mod lib_init;
pub mod memory;
pub mod status;
pub mod team;

// Re-export commonly used types for convenience
pub use bindings::{
    ucc_coll_sync_type_t, ucc_coll_type_t, ucc_post_ordering_t, ucc_reduction_op_t,
    ucc_thread_mode_t, ucc_status_t,
};
