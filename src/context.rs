//! UCC context creation and management.
//!
//! A context is the primary communication resource in UCC.
//! It wraps the underlying UCX context for the `tl/ucp` transport.

use crate::bindings::{
    ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES, ucc_context_config_h,
    ucc_context_config_read, ucc_context_config_release, ucc_context_create,
    ucc_context_destroy, ucc_context_h, ucc_context_params, ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_ID,
    ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_SYNC_TYPE,
    ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_TYPE, ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE,
};
use crate::lib_init::UccLib;
use crate::status::{check_status, UccError, UccStatus};

/// UCC context config handle with RAII cleanup.
pub struct UccContextConfig {
    handle: ucc_context_config_h,
}

impl UccContextConfig {
    /// Read the UCC context configuration from environment variables.
    pub fn read(lib: &UccLib) -> Result<Self, UccStatus> {
        let mut config: ucc_context_config_h = std::ptr::null_mut();
        let status = unsafe {
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
            unsafe {
                ucc_context_config_release(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// UCC context handle with RAII cleanup.
#[derive(Clone)]
pub struct UccContext {
    handle: ucc_context_h,
    _lib: UccLib,
}

impl UccContext {
    /// Create a new context with default parameters.
    pub fn new(lib: UccLib) -> Result<Self, UccStatus> {
        Self::with_params(lib, Default::default())
    }

    /// Create a new context with custom parameters.
    pub fn with_params(lib: UccLib, ctx_params: UccContextParams) -> Result<Self, UccStatus> {
        // Must create a context config before creating the context.
        // ucc_context_create dereferences the config pointer internally,
        // so passing NULL causes a segfault.
        let config = UccContextConfig::read(&lib)?;
        let lib_handle = lib.handle();
        let mut context: ucc_context_h = std::ptr::null_mut();
        let status = unsafe {
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
            unsafe {
                ucc_context_destroy(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Parameters for UCC context creation.
pub struct UccContextParams(ucc_context_params);

impl Default for UccContextParams {
    fn default() -> Self {
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
    /// Set the context ID.
    pub fn with_id(&mut self, id: u64) {
        self.0.ctx_id = id;
        self.0.mask |= ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_ID as u64;
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
