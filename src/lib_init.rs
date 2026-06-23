//! UCC library initialization and finalization.
//!
//! Manages the lifetime of the UCC library handle (`ucc_lib_h`).
//! Also manages the UCC config handle (`ucc_lib_config_h`) which must
//! be created via `ucc_lib_config_read` before `ucc_init_version`.

use crate::bindings::{
    ucc_finalize, ucc_init_version, ucc_lib_config_h, ucc_lib_config_read,
    ucc_lib_config_release, ucc_lib_h, ucc_lib_params,
    ucc_thread_mode_t_UCC_THREAD_SINGLE,
    ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_THREAD_MODE,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_COLL_TYPES,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_SYNC_TYPE,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_REDUCTION_TYPES,
};
use crate::status::{check_status, UccStatus};

/// UCC library config handle with RAII cleanup.
pub struct UccLibConfig {
    handle: ucc_lib_config_h,
}

impl UccLibConfig {
    /// Read the UCC library configuration from environment variables.
    pub fn read() -> Result<Self, UccStatus> {
        let mut config: ucc_lib_config_h = std::ptr::null_mut();
        let status = unsafe {
            ucc_lib_config_read(std::ptr::null(), std::ptr::null(), &mut config)
        };
        check_status(status)?;
        Ok(Self { handle: config })
    }

    /// Get the raw config handle.
    pub fn handle(&self) -> ucc_lib_config_h {
        self.handle
    }
}

impl Drop for UccLibConfig {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ucc_lib_config_release(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// UCC library handle with RAII cleanup.
#[derive(Clone)]
pub struct UccLib {
    handle: ucc_lib_h,
}

impl UccLib {
    /// Initialize the UCC library with default parameters.
    pub fn init() -> Result<Self, UccStatus> {
        Self::with_params(Default::default())
    }

    /// Initialize the UCC library with custom parameters.
    pub fn with_params(lib_params: UccLibParams) -> Result<Self, UccStatus> {
        // Must create a config handle before initializing the library.
        // ucc_init_version dereferences the config pointer internally,
        // so passing NULL causes a segfault.
        let config = UccLibConfig::read()?;
        let mut lib: ucc_lib_h = std::ptr::null_mut();
        let status = unsafe {
            ucc_init_version(
                1,
                9,
                &lib_params.0,
                config.handle(),
                &mut lib,
            )
        };
        check_status(status)?;
        // config is dropped here, releasing the config handle
        Ok(Self { handle: lib })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_lib_h {
        self.handle
    }
}

impl Drop for UccLib {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ucc_finalize(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Parameters for UCC library initialization.
pub struct UccLibParams(ucc_lib_params);

impl Default for UccLibParams {
    fn default() -> Self {
        let mut params: ucc_lib_params = unsafe { std::mem::zeroed() };
        params.thread_mode = ucc_thread_mode_t_UCC_THREAD_SINGLE;
        params.sync_type = ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES;
        params.mask = ucc_lib_params_field_UCC_LIB_PARAM_FIELD_THREAD_MODE as u64
            | ucc_lib_params_field_UCC_LIB_PARAM_FIELD_COLL_TYPES as u64
            | ucc_lib_params_field_UCC_LIB_PARAM_FIELD_SYNC_TYPE as u64
            | ucc_lib_params_field_UCC_LIB_PARAM_FIELD_REDUCTION_TYPES as u64;
        // Request all collective and reduction types
        params.coll_types = u64::MAX;
        params.reduction_types = u64::MAX;
        Self(params)
    }
}

impl UccLibParams {
    /// Access the inner params for advanced configuration.
    pub fn inner_mut(&mut self) -> &mut ucc_lib_params {
        &mut self.0
    }
}
