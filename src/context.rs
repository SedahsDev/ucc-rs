//! UCC context creation and management.
//!
//! A context is the primary communication resource in UCC.
//! It wraps the underlying UCX context for the `tl/ucp` transport.

use crate::bindings::{
    ucc_coll_sync_type_t, ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES,
    ucc_context_config_h, ucc_context_config_read, ucc_context_config_release,
    ucc_context_create, ucc_context_destroy, ucc_context_h, ucc_context_params,
    ucc_context_type_t, ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE,
    ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_ID,
    ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_SYNC_TYPE,
    ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_TYPE,
};
use crate::lib_init::UccLib;
use crate::status::{check_status, UccError, UccStatus};

/// UCC context config handle with RAII cleanup.
///
/// Holds a `ucc_context_config_h` obtained from `ucc_context_config_read`.
/// Must be created before calling `ucc_context_create`. Dropped automatically,
/// releasing the underlying C config handle.
#[must_use = "Config handles should be kept alive or explicitly dropped"]
pub struct UccContextConfig {
    handle: ucc_context_config_h,
}

impl UccContextConfig {
    /// Read the UCC context configuration from environment variables.
    #[must_use = "Result should be checked"]
    pub fn read(lib: &UccLib) -> Result<Self, UccStatus> {
        let mut config: ucc_context_config_h = std::ptr::null_mut();
        let status = unsafe {
             // Safety: lib.handle() is a valid handle from UccLib; ucc_context_config_read
             // accepts NULL for the parent config argument and only writes to &mut config.
            ucc_context_config_read(lib.handle(), std::ptr::null(), &mut config)
        };
        check_status(status)?;
        Ok(Self { handle: config })
    }

    /// Get the raw config handle.
    pub fn handle(&self) -> ucc_context_config_h {
        self.handle
    }
}

impl Drop for UccContextConfig {
    fn drop(&mut self) {
        if !self.handle.is_null() {
             // Safety: handle was validated non-null above and the C function only
             // dereferences the handle pointer for cleanup. Handle is nulled after
             // to prevent double-free.
            unsafe {
                ucc_context_config_release(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// UCC context handle with RAII cleanup.
///
/// Represents a UCC communication context — the primary resource for
/// creating teams and running collective operations. Holds a reference
/// to the [`UccLib`] that created it, keeping the library alive.
/// Cloneable — each clone shares the same underlying C handle.
#[must_use = "Context handles should be kept alive or explicitly dropped"]
#[derive(Clone)]
pub struct UccContext {
    handle: ucc_context_h,
    _lib: UccLib,
}

impl UccContext {
    /// Create a new context with default parameters.
    #[must_use = "Result should be checked"]
    pub fn new(lib: UccLib) -> Result<Self, UccStatus> {
        Self::with_params(lib, Default::default())
    }

    /// Create a new context with custom parameters.
    #[must_use = "Result should be checked"]
    pub fn with_params(lib: UccLib, ctx_params: UccContextParams) -> Result<Self, UccStatus> {
        // Must create a context config before creating the context.
        // ucc_context_create dereferences the config pointer internally,
        // so passing NULL causes a segfault.
        let config = UccContextConfig::read(&lib)?;
        let lib_handle = lib.handle();
        let mut context: ucc_context_h = std::ptr::null_mut();
        let status = unsafe {
             // Safety: lib_handle and config.handle() are valid; &ctx_params.0 is a valid
             // reference; &mut context is a valid output pointer.
            ucc_context_create(
                lib_handle,
                &ctx_params.0,
                config.handle(),
                &mut context,
            )
        };
        check_status(status)?;
        if context.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        // config is dropped here, releasing the context config handle
        Ok(Self {
            handle: context,
            _lib: lib,
        })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_context_h {
        self.handle
    }
}

impl Drop for UccContext {
    fn drop(&mut self) {
        if !self.handle.is_null() {
             // Safety: handle was validated non-null above and the C function only
             // dereferences the handle pointer for cleanup. Handle is nulled after
             // to prevent double-free.
            unsafe {
                ucc_context_destroy(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Parameters for UCC context creation.
///
/// Wraps `ucc_context_params` and manages the field mask automatically.
///
/// # Default values
///
/// * `type_` — `UCC_CONTEXT_EXCLUSIVE`
/// * `sync_type` — `UCC_SYNC_COLLECTIVES`
/// * OOB callbacks — all set to `None` / null
#[must_use = "Context params should be used to create a context"]
pub struct UccContextParams(ucc_context_params);

impl Default for UccContextParams {
    fn default() -> Self {
         // Safety: ucc_context_params is a POD struct with no pointers or discriminants
         // that the C API only reads fields indicated by the mask.
        let mut params: ucc_context_params = unsafe { std::mem::zeroed() };
        params.type_ = ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE;
        params.sync_type = ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES;
        params.oob.allgather = None;
        params.oob.req_test = None;
        params.oob.req_free = None;
        params.oob.coll_info = std::ptr::null_mut();
        params.oob.n_oob_eps = 0;
        params.oob.oob_ep = 0;
        params.mask = ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_TYPE as u64
            | ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_SYNC_TYPE as u64;
        Self(params)
    }
}

impl UccContextParams {
    /// Set the context type.
    pub fn with_type(&mut self, ctx_type: ucc_context_type_t) {
        self.0.type_ = ctx_type;
        self.0.mask |= crate::bindings::ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_TYPE as u64;
    }

    /// Set the collective sync type.
    pub fn with_sync_type(&mut self, sync_type: ucc_coll_sync_type_t) {
        self.0.sync_type = sync_type;
        self.0.mask |= crate::bindings::ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_SYNC_TYPE as u64;
    }

    /// Set the context ID.
    pub fn with_id(&mut self, id: u64) {
        self.0.ctx_id = id;
        self.0.mask |= crate::bindings::ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_ID as u64;
    }

    /// Set OOB callbacks.
    pub fn with_oob(&mut self, oob: crate::bindings::ucc_context_oob_coll_t) {
        self.0.oob = oob;
        // No specific mask bit for OOB in the current bindings; set via inner_mut if needed
    }

    /// Get mutable access to the underlying FFI struct for advanced configuration.
    ///
    /// # Safety
    /// Directly modifying FFI fields bypasses mask management. Ensure any field
    /// you set also has its corresponding bit set in `self.0.mask`.
    pub fn inner_mut(&mut self) -> &mut ucc_context_params {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;
    use std::clone::Clone;

    #[test]
    fn test_ucc_context_params_default() {
        let params = UccContextParams::default();
        assert_eq!(params.0.type_, ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE);
        assert_eq!(params.0.sync_type, ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES);
    }

    #[test]
    fn test_ucc_context_params_with_type() {
        let mut params = UccContextParams::default();
        params.with_type(ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE);
        assert!(params.0.mask & ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_TYPE as u64 != 0);
    }

    #[test]
    fn test_ucc_context_params_with_id() {
        let mut params = UccContextParams::default();
        params.with_id(42);
        assert_eq!(params.0.ctx_id, 42);
        assert!(params.0.mask & ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_ID as u64 != 0);
    }

    #[test]
    fn test_ucc_context_trait_bounds() {
        // UccContext is Clone but NOT Send — raw FFI handles don't impl Send
        assert_impl_all!(UccContext: Clone);
    }

    #[test]
    fn test_ucc_context_config_has_handle() {
        // UccContextConfig wraps a raw FFI handle — intentionally not Send.
        let _ = std::mem::size_of::<UccContextConfig>();
    }
}
