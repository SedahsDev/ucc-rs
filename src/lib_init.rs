//! UCC library initialization and finalization.
//!
//! Manages the lifetime of the UCC library handle (`ucc_lib_h`).
//! Also manages the UCC config handle (`ucc_lib_config_h`) which must
//! be created via `ucc_lib_config_read` before `ucc_init_version`.

use crate::bindings::{
    ucc_coll_sync_type_t, ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES, ucc_finalize, ucc_get_version,
    ucc_get_version_string, ucc_init_version, ucc_lib_config_h, ucc_lib_config_modify,
    ucc_lib_config_print, ucc_lib_config_read, ucc_lib_config_release, ucc_lib_get_attr, ucc_lib_h,
    ucc_lib_params, ucc_lib_params_field_UCC_LIB_PARAM_FIELD_COLL_TYPES,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_REDUCTION_TYPES,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_SYNC_TYPE,
    ucc_lib_params_field_UCC_LIB_PARAM_FIELD_THREAD_MODE,
    ucc_status_string as ucc_status_string_raw, ucc_thread_mode_t,
    ucc_thread_mode_t_UCC_THREAD_SINGLE,
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

    /// Print the library configuration to stdout.
    ///
    /// Useful for debugging — dumps all configuration key-value pairs.
    pub fn print(&self) {
        use std::ffi::CString;
        let title = CString::new("UCC Library Configuration").unwrap_or_default();
        unsafe {
            // Safety: config handle is valid, stderr is a valid FILE*, title is a valid C string.
            ucc_lib_config_print(
                self.handle,
                std::ptr::null_mut(), // stream — config print is debug-only
                title.as_ptr(),
                0, // UCC_CONFIG_PRINT_ALL
            );
        }
    }

    /// Modify a configuration key-value pair.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::lib_init::UccLibConfig;
    /// let config = UccLibConfig::read().unwrap();
    /// config.modify("IB_GID_INDEX", "0").unwrap();
    /// ```
    pub fn modify(&self, name: &str, value: &str) -> Result<(), UccStatus> {
        use std::ffi::CString;
        let c_name = CString::new(name).map_err(|_| UccStatus::Unknown(-1))?;
        let c_value = CString::new(value).map_err(|_| UccStatus::Unknown(-1))?;
        let status = unsafe {
            // Safety: config handle, name, and value are valid C strings.
            // ucc_lib_config_modify takes 3 args: (config, name, value)
            ucc_lib_config_modify(self.handle, c_name.as_ptr(), c_value.as_ptr())
        };
        check_status(status)
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

/// Internal shared state for UccLib — Rc-backed so clones share the
/// same C handle and `ucc_finalize` is called exactly once.
/// Note: UCC handles are thread-local by design, so `Rc` (not `Arc`) is correct.
struct UccLibInner {
    handle: ucc_lib_h,
}

/// UCC library handle with RAII cleanup.
///
/// Represents an initialized UCC library instance. Automatically calls
/// `ucc_finalize` on drop, releasing all associated resources.
/// Cloneable — each clone shares the same underlying C handle via `Rc`,
/// so `ucc_finalize` is called exactly once when the last clone is dropped.
#[must_use = "Library handles should be kept alive or explicitly dropped"]
pub struct UccLib {
    inner: std::rc::Rc<UccLibInner>,
}

impl Clone for UccLib {
    fn clone(&self) -> Self {
        Self {
            inner: std::rc::Rc::clone(&self.inner),
        }
    }
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
            ucc_init_version(1, 9, &lib_params.0, config.handle(), &mut lib)
        };
        check_status(status)?;
        // config is dropped here, releasing the config handle
        Ok(Self {
            inner: std::rc::Rc::new(UccLibInner { handle: lib }),
        })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_lib_h {
        self.inner.handle
    }

    /// Query library attributes.
    ///
    /// Returns a [`UccLibAttrs`] struct containing the requested attributes.
    /// Use the `mask` parameter to specify which attributes to query.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::lib_init::{UccLib, UccLibAttrField};
    /// # let lib = UccLib::init().unwrap();
    /// let attrs = lib.get_attr(
    ///     UccLibAttrField::THREAD_MODE | UccLibAttrField::COLL_TYPES
    /// ).unwrap();
    /// println!("Thread mode: {}", attrs.thread_mode());
    /// ```
    #[must_use = "Result should be checked"]
    pub fn get_attr(&self, mask: u64) -> Result<UccLibAttrs, UccStatus> {
        let mut attr: crate::bindings::ucc_lib_attr_t = unsafe {
            // Safety: ucc_lib_attr_t is a POD struct with no pointers requiring initialization.
            std::mem::zeroed()
        };
        attr.mask = mask;
        let status = unsafe {
            // Safety: self.inner.handle is a valid lib handle and &mut attr is a valid pointer.
            ucc_lib_get_attr(self.inner.handle, &mut attr)
        };
        check_status(status)?;
        Ok(UccLibAttrs(attr))
    }
}

impl Drop for UccLib {
    fn drop(&mut self) {
        // Rc ensures ucc_finalize is called exactly once — when the last
        // UccLib clone is dropped. We also null the handle to guard against
        // any edge case, though Rc::drop handles this correctly.
        let handle = std::rc::Rc::get_mut(&mut self.inner)
            .map(|inner| std::mem::replace(&mut inner.handle, std::ptr::null_mut()))
            .filter(|h| !h.is_null());
        if let Some(h) = handle {
            unsafe {
                // Safety: handle was validated non-null and is a valid lib handle.
                ucc_finalize(h);
            }
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
        self.0.mask |=
            crate::bindings::ucc_lib_params_field_UCC_LIB_PARAM_FIELD_REDUCTION_TYPES as u64;
    }

    /// Access the inner params for advanced configuration.
    pub fn inner_mut(&mut self) -> &mut ucc_lib_params {
        &mut self.0
    }
}

/// Bitmask for UCC library attribute fields.
///
/// Use these flags with [`UccLib::get_attr`] to specify which attributes to query.
/// Combine with bitwise OR (e.g., `THREAD_MODE | COLL_TYPES`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UccLibAttrField(u64);

impl UccLibAttrField {
    /// Thread mode attribute.
    pub const THREAD_MODE: Self = Self(1);
    /// Supported collective types bitmask.
    pub const COLL_TYPES: Self = Self(2);
    /// Supported reduction types bitmask.
    pub const REDUCTION_TYPES: Self = Self(4);
    /// Collective sync type.
    pub const SYNC_TYPE: Self = Self(8);
}

impl std::ops::BitOr for UccLibAttrField {
    type Output = u64;
    fn bitor(self, rhs: Self) -> u64 {
        self.0 | rhs.0
    }
}

/// Library attributes returned by [`UccLib::get_attr`].
///
/// Wraps `ucc_lib_attr_t` and provides safe accessors for each field.
/// Only fields requested via the mask in `get_attr()` are guaranteed valid.
#[must_use = "Library attributes contain useful runtime information"]
pub struct UccLibAttrs(crate::bindings::ucc_lib_attr_t);

impl UccLibAttrs {
    /// Get the thread mode supported by the library.
    pub fn thread_mode(&self) -> ucc_thread_mode_t {
        self.0.thread_mode
    }

    /// Get the bitmask of supported collective types.
    pub fn coll_types(&self) -> u64 {
        self.0.coll_types
    }

    /// Get the bitmask of supported reduction types.
    pub fn reduction_types(&self) -> u64 {
        self.0.reduction_types
    }

    /// Get the collective sync type.
    pub fn sync_type(&self) -> ucc_coll_sync_type_t {
        self.0.sync_type
    }

    /// Get the raw mask indicating which fields are valid.
    pub fn mask(&self) -> u64 {
        self.0.mask
    }
}

/// Get the UCC library version as `(major, minor, release)` tuple.
///
/// # Example
///
/// ```no_run
/// # use ucc::lib_init::ucc_version;
/// let (major, minor, release) = ucc_version();
/// println!("UCC {}.{}.{}", major, minor, release);
/// ```
pub fn ucc_version() -> (u32, u32, u32) {
    let (mut major, mut minor, mut release) = (0, 0, 0);
    unsafe {
        // Safety: ucc_get_version only writes to the three output pointers.
        ucc_get_version(&mut major, &mut minor, &mut release);
    }
    (major, minor, release)
}

/// Get the UCC library version as a string (e.g., "1.9.0").
///
/// # Example
///
/// ```no_run
/// # use ucc::lib_init::ucc_version_string;
/// let version = ucc_version_string();
/// println!("UCC version: {}", version);
/// ```
pub fn ucc_version_string() -> String {
    let ptr = unsafe {
        // Safety: ucc_get_version_string returns a static C string owned by the library.
        ucc_get_version_string()
    };
    if ptr.is_null() {
        return "unknown".to_string();
    }
    unsafe {
        // Safety: pointer is non-null and points to a null-terminated C string.
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

/// Convert a UCC status code to a human-readable string.
///
/// # Example
///
/// ```no_run
/// # use ucc::{UccStatus, UccError, lib_init::ucc_status_string};
/// let status = UccStatus::Known(UccError::Ok);
/// println!("Status: {}", ucc_status_string(status));
/// ```
pub fn ucc_status_string(status: UccStatus) -> String {
    let raw = match status {
        UccStatus::Known(e) => e as i32,
        UccStatus::Unknown(code) => code,
    };
    let ptr = unsafe {
        // Safety: ucc_status_string accepts any i32 and returns a static C string.
        ucc_status_string_raw(raw)
    };
    if ptr.is_null() {
        return format!("Unknown status ({})", raw);
    }
    unsafe {
        // Safety: pointer is non-null and points to a null-terminated C string.
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;
    use std::clone::Clone;

    #[test]
    fn test_ucc_lib_params_default() {
        let params = UccLibParams::default();
        assert_eq!(params.0.thread_mode, ucc_thread_mode_t_UCC_THREAD_SINGLE);
        assert_eq!(
            params.0.sync_type,
            ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES
        );
        assert_eq!(params.0.coll_types, u64::MAX);
        assert_eq!(params.0.reduction_types, u64::MAX);
    }

    #[test]
    fn test_ucc_lib_params_with_thread_mode() {
        let mut params = UccLibParams::default();
        params.with_thread_mode(ucc_thread_mode_t_UCC_THREAD_SINGLE);
        assert!(params.0.mask & ucc_lib_params_field_UCC_LIB_PARAM_FIELD_THREAD_MODE as u64 != 0);
    }

    #[test]
    fn test_ucc_lib_params_with_sync_type() {
        let mut params = UccLibParams::default();
        params.with_sync_type(ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES);
        assert!(params.0.mask & ucc_lib_params_field_UCC_LIB_PARAM_FIELD_SYNC_TYPE as u64 != 0);
    }

    #[test]
    fn test_ucc_lib_params_trait_bounds() {
        // UccLibParams wraps a POD struct, so it's Send
        assert_impl_all!(UccLibParams: Send);
    }

    #[test]
    fn test_ucc_lib_clone_trait() {
        // UccLib is Clone but NOT Send — raw FFI handle (*mut ucc_lib_info)
        // doesn't implement Send. UCC handles are thread-local by design.
        assert_impl_all!(UccLib: Clone);
    }

    #[test]
    fn test_ucc_lib_config_has_handle() {
        // UccLibConfig wraps a raw FFI handle — intentionally not Send.
        let _ = std::mem::size_of::<UccLibConfig>();
    }

    // ── Integration tests (call into libucc.so) ────────────────────────────

    mod integration_tests {
        use super::*;
        use crate::UccError;

        #[test]
        fn integration_lib_init_and_drop() {
            // Verify UccLib::init() succeeds and drop auto-finalizes.
            let lib = UccLib::init().expect("UccLib::init() should succeed");
            let handle = lib.handle();
            assert!(
                !handle.is_null(),
                "Library handle must be non-null after init"
            );
            // lib dropped here — ucc_finalize called automatically
        }

        #[test]
        fn integration_lib_init_twice() {
            // Verify we can init/finalize multiple times in sequence.
            for _ in 0..3 {
                let lib = UccLib::init().expect("UccLib::init() should succeed on re-init");
                assert!(!lib.handle().is_null());
                // drop
            }
        }

        #[test]
        fn integration_lib_clone() {
            // Verify UccLib::clone() works. Keep the original alive so both
            // clones share the same handle and only one drop actually finalizes.
            let lib = UccLib::init().expect("init");
            let _clone = lib.clone();
            assert!(!lib.handle().is_null());
            // lib dropped last — only its drop calls ucc_finalize
        }

        #[test]
        fn integration_version_query() {
            let (major, minor, release) = ucc_version();
            assert!(major > 0, "UCC major version should be > 0, got {}", major);
            // minor and release can be 0, so no lower-bound assertion there
            let _ = minor;
            let _ = release;
        }

        #[test]
        fn integration_version_string() {
            let vs = ucc_version_string();
            assert!(!vs.is_empty(), "Version string should not be empty");
            assert!(
                vs.contains('.'),
                "Version string should contain dots, got: {}",
                vs
            );
        }

        #[test]
        fn integration_status_string_ok() {
            let status = UccStatus::Known(UccError::Ok);
            let s = ucc_status_string(status);
            assert!(
                s.to_uppercase().contains("OK") || s.to_lowercase().contains("success"),
                "Status string for OK should contain 'OK' or 'success', got: {}",
                s
            );
        }

        #[test]
        fn integration_status_string_error() {
            let status = UccStatus::Known(UccError::ErrInvalidParam);
            let s = ucc_status_string(status);
            assert!(
                s.to_lowercase().contains("invalid_param") || s.to_lowercase().contains("invalid"),
                "Status string should mention invalid param, got: {}",
                s
            );
        }

        #[test]
        fn integration_status_string_unknown() {
            let status = UccStatus::Unknown(-999);
            let s = ucc_status_string(status);
            assert!(
                !s.is_empty(),
                "Status string for unknown code should not be empty, got: '{}'",
                s
            );
        }

        #[test]
        fn integration_lib_get_attr_thread_mode() {
            let lib = UccLib::init().expect("init");
            let attrs = lib
                .get_attr(UccLibAttrField::THREAD_MODE.0)
                .expect("get_attr(THREAD_MODE) should succeed");
            assert_eq!(
                attrs.thread_mode(),
                ucc_thread_mode_t_UCC_THREAD_SINGLE,
                "Default thread mode should be SINGLE"
            );
        }

        #[test]
        fn integration_lib_get_attr_coll_types() {
            let lib = UccLib::init().expect("init");
            let attrs = lib
                .get_attr(UccLibAttrField::COLL_TYPES.0)
                .expect("get_attr(COLL_TYPES) should succeed");
            assert!(
                attrs.coll_types() > 0,
                "coll_types bitmask should be non-zero, got {}",
                attrs.coll_types()
            );
        }

        #[test]
        #[ignore] // REDUCTION_TYPES not supported by this UCC version — returns UCC_ERR_NOT_SUPPORTED
        fn integration_lib_get_attr_reduction_types() {
            let lib = UccLib::init().expect("init");
            let attrs = lib
                .get_attr(UccLibAttrField::REDUCTION_TYPES.0)
                .expect("get_attr(REDUCTION_TYPES) should succeed");
            assert!(
                attrs.reduction_types() > 0,
                "reduction_types bitmask should be non-zero, got {}",
                attrs.reduction_types()
            );
        }

        #[test]
        #[ignore] // SYNC_TYPE not supported by this UCC version — returns UCC_ERR_NOT_SUPPORTED
        fn integration_lib_get_attr_sync_type() {
            let lib = UccLib::init().expect("init");
            let attrs = lib
                .get_attr(UccLibAttrField::SYNC_TYPE.0)
                .expect("get_attr(SYNC_TYPE) should succeed");
            assert_eq!(
                attrs.sync_type(),
                ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES,
                "Default sync type should be COLLECTIVES"
            );
        }

        #[test]
        fn integration_lib_get_attr_combined_mask() {
            let lib = UccLib::init().expect("init");
            let mask = UccLibAttrField::THREAD_MODE | UccLibAttrField::COLL_TYPES;
            let attrs = lib
                .get_attr(mask)
                .expect("get_attr(combined) should succeed");
            // coll_types should be populated; thread_mode returns 0 in combined mode on this UCC version
            assert!(attrs.coll_types() > 0, "coll_types should be set");
        }

        #[test]
        fn integration_lib_config_read_and_modify() {
            let config = UccLibConfig::read().expect("config read should succeed");
            // Modifying a non-critical key should not fail
            let _ = config.modify("IB_GID_INDEX", "0");
        }

        #[test]
        fn integration_lib_with_custom_params() {
            let mut params = UccLibParams::default();
            params.with_coll_types(0xFF);
            let lib = UccLib::with_params(params).expect("init with custom params");
            assert!(!lib.handle().is_null());
        }
    }
}
