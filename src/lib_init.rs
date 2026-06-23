//! UCC library initialization and finalization.
//!
//! Manages the lifetime of the UCC library handle (`ucc_lib_h`).
//! Also manages the UCC config handle (`ucc_lib_config_h`) which must
//! be created via `ucc_lib_config_read` before `ucc_init_version`.

use crate::bindings::{
    ucc_coll_sync_type_t, ucc_finalize, ucc_init_version, ucc_lib_config_h,
    ucc_lib_config_read, ucc_lib_config_release, ucc_lib_h, ucc_lib_params,
    ucc_thread_mode_t,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_COLL_TYPES,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_REDUCTION_TYPES,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_SYNC_TYPE,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_THREAD_MODE,
    ucc_thread_mode_t_UCC_THREAD_SINGLE,
    ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES,
};
use crate::status::{check_status, UccStatus};

/// UCC library config handle with RAII cleanup.
///
/// Holds a `ucc_lib_config_h` obtained from `ucc_lib_config_read`. The config
/// must be created before calling `ucc_init_version`. Dropped automatically,
/// releasing the underlying C config handle.
#[must_use = "Config handles should be kept alive or explicitly dropped"]
pub struct UccLibConfig {
    handle: ucc_lib_config_h,
}

impl UccLibConfig {
 /// Read the UCC library configuration from environment variables.
    #[must_use = "Result should be checked"]
    pub fn read() -> Result<Self, UccStatus> {
        let mut config: ucc_lib_config_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: ucc_lib_config_read only writes to the config output pointer
            // and accepts NULL for the first two arguments (lib and parent config).
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
            // Safety: handle was validated non-null above and the C function only
            // dereferences the handle pointer for cleanup. Handle is nulled after
            // to prevent double-free.
            unsafe {
                ucc_lib_config_release(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// UCC library handle with RAII cleanup.
///
/// Represents an initialized UCC library instance. Automatically calls
/// `ucc_finalize` on drop, releasing all associated resources.
/// Cloneable — each clone shares the same underlying C handle.
#[must_use = "Library handles should be kept alive or explicitly dropped"]
#[derive(Clone)]
pub struct UccLib {
    handle: ucc_lib_h,
}

impl UccLib {
    /// Initialize the UCC library with default parameters.
    #[must_use = "Result should be checked"]
    pub fn init() -> Result<Self, UccStatus> {
        Self::with_params(Default::default())
    }

    /// Initialize the UCC library with custom parameters.
    #[must_use = "Result should be checked"]
    pub fn with_params(lib_params: UccLibParams) -> Result<Self, UccStatus> {
        // Must create a config handle before initializing the library.
        // ucc_init_version dereferences the config pointer internally,
        // so passing NULL causes a segfault.
        let config = UccLibConfig::read()?;
        let mut lib: ucc_lib_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: lib_params and config are valid references; &mut lib is a
            // valid output pointer. ucc_init_version only writes to this pointer
            // on success.
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
            // Safety: handle was validated non-null above and the C function only
            // dereferences the handle pointer for cleanup. Handle is nulled after
            // to prevent double-free.
            unsafe {
                ucc_finalize(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Parameters for UCC library initialization.
///
/// Wraps `ucc_lib_params` and manages the field mask automatically.
///
/// # Default values
///
/// * `thread_mode` — `UCC_THREAD_SINGLE`
/// * `sync_type` — `UCC_SYNC_COLLECTIVES`
/// * `coll_types` — all types enabled (`u64::MAX`)
/// * `reduction_types` — all types enabled (`u64::MAX`)
#[must_use = "Library params should be used to initialize the library"]
pub struct UccLibParams(ucc_lib_params);

impl Default for UccLibParams {
    fn default() -> Self {
        // Safety: ucc_lib_params is a POD struct with no pointers or discriminants
        // that the C API only reads fields indicated by the mask.
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
    /// Set the thread mode.
    pub fn with_thread_mode(&mut self, mode: ucc_thread_mode_t) {
        self.0.thread_mode = mode;
        self.0.mask |= crate::bindings::ucc_lib_params_field_UCC_LIB_PARAM_FIELD_THREAD_MODE as u64;
    }

    /// Set the collective sync type.
    pub fn with_sync_type(&mut self, sync_type: ucc_coll_sync_type_t) {
        self.0.sync_type = sync_type;
        self.0.mask |= crate::bindings::ucc_lib_params_field_UCC_LIB_PARAM_FIELD_SYNC_TYPE as u64;
    }

    /// Set the collective types bitmask.
    pub fn with_coll_types(&mut self, coll_types: u64) {
        self.0.coll_types = coll_types;
        self.0.mask |= crate::bindings::ucc_lib_params_field_UCC_LIB_PARAM_FIELD_COLL_TYPES as u64;
    }

    /// Set the reduction types bitmask.
    pub fn with_reduction_types(&mut self, reduction_types: u64) {
        self.0.reduction_types = reduction_types;
        self.0.mask |= crate::bindings::ucc_lib_params_field_UCC_LIB_PARAM_FIELD_REDUCTION_TYPES as u64;
    }

    /// Access the inner params for advanced configuration.
    pub fn inner_mut(&mut self) -> &mut ucc_lib_params {
        &mut self.0
    }
}
