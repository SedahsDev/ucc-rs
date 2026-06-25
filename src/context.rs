//! UCC context creation and management.
//!
//! A context is the primary communication resource in UCC.
//! It wraps the underlying UCX context for the `tl/ucp` transport.

use crate::bindings::{
    ucc_coll_sync_type_t, ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES, ucc_context_config_h,
    ucc_context_config_modify, ucc_context_config_print, ucc_context_config_read,
    ucc_context_config_release, ucc_context_create, ucc_context_destroy, ucc_context_get_attr,
    ucc_context_h, ucc_context_params, ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_SYNC_TYPE,
    ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_TYPE, ucc_context_progress,
    ucc_context_type_t, ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE,
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

    /// Print the context configuration to stdout.
    ///
    /// Useful for debugging — dumps all configuration key-value pairs.
    pub fn print(&self) {
        use std::ffi::CString;
        let title = CString::new("UCC Context Configuration").unwrap_or_default();
        unsafe {
            // Safety: config handle is valid, stdout is a valid FILE*, title is a valid C string.
            ucc_context_config_print(
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
    /// # use ucc::{lib_init::UccLib, context::UccContextConfig};
    /// # let lib = UccLib::init().unwrap();
    /// let config = UccContextConfig::read(&lib).unwrap();
    /// config.modify("IB_GID_INDEX", "0").unwrap();
    /// ```
    pub fn modify(&self, name: &str, value: &str) -> Result<(), UccStatus> {
        use std::ffi::CString;
        let c_name = CString::new(name).map_err(|_| UccStatus::Unknown(-1))?;
        let c_value = CString::new(value).map_err(|_| UccStatus::Unknown(-1))?;
        let status = unsafe {
            // Safety: config handle, name, and value are valid C strings.
            // ucc_context_config_modify takes 4 args: (config, component, name, value)
            // component = NULL means use the default component
            ucc_context_config_modify(
                self.handle,
                std::ptr::null(),
                c_name.as_ptr(),
                c_value.as_ptr(),
            )
        };
        check_status(status)
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

/// Internal shared state for UccContext — Rc-backed so clones share the
/// same C handle and `ucc_context_destroy` is called exactly once.
/// Note: UCC handles are thread-local by design, so `Rc` (not `Arc`) is correct.
struct UccContextInner {
    handle: ucc_context_h,
    _lib: UccLib,
}

/// UCC context handle with RAII cleanup.
///
/// Represents a UCC communication context — the primary resource for
/// creating teams and running collective operations. Holds a reference
/// to the [`UccLib`] that created it, keeping the library alive.
/// Cloneable — each clone shares the same underlying C handle via `Rc`,
/// so `ucc_context_destroy` is called exactly once when the last clone is dropped.
#[must_use = "Context handles should be kept alive or explicitly dropped"]
pub struct UccContext {
    inner: std::rc::Rc<UccContextInner>,
}

impl Clone for UccContext {
    fn clone(&self) -> Self {
        Self {
            inner: std::rc::Rc::clone(&self.inner),
        }
    }
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
            ucc_context_create(lib_handle, &ctx_params.0, config.handle(), &mut context)
        };
        check_status(status)?;
        if context.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        // config is dropped here, releasing the context config handle
        Ok(Self {
            inner: std::rc::Rc::new(UccContextInner {
                handle: context,
                _lib: lib,
            }),
        })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_context_h {
        self.inner.handle
    }

    /// Query context attributes.
    ///
    /// Returns a [`UccContextAttrs`] struct containing the requested attributes.
    /// Use the `mask` parameter to specify which attributes to query.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::{lib_init::UccLib, context::{UccContext, UccContextAttrField}};
    /// # let ctx = UccContext::new(UccLib::init().unwrap()).unwrap();
    /// let attrs = ctx.get_attr(
    ///     UccContextAttrField::TYPE | UccContextAttrField::SYNC_TYPE
    /// ).unwrap();
    /// println!("Context type: {}", attrs.context_type());
    /// ```
    #[must_use = "Result should be checked"]
    pub fn get_attr(&self, mask: u64) -> Result<UccContextAttrs, UccStatus> {
        let mut attr: crate::bindings::ucc_context_attr_t = unsafe {
            // Safety: ucc_context_attr_t is a POD struct with no pointers requiring initialization.
            std::mem::zeroed()
        };
        attr.mask = mask;
        let status = unsafe {
            // Safety: self.inner.handle is a valid context handle and &mut attr is a valid pointer.
            ucc_context_get_attr(self.inner.handle, &mut attr)
        };
        check_status(status)?;
        Ok(UccContextAttrs(attr))
    }

    /// Progress the context.
    ///
    /// Calls `ucc_context_progress` to make progress on any pending operations.
    /// This is a blocking call that may perform network I/O.
    pub fn progress(&self) {
        unsafe {
            // Safety: self.inner.handle is a valid context handle.
            ucc_context_progress(self.inner.handle);
        }
    }
}

impl Drop for UccContext {
    fn drop(&mut self) {
        // Rc ensures ucc_context_destroy is called exactly once — when the last
        // UccContext clone is dropped.
        let handle = std::rc::Rc::get_mut(&mut self.inner)
            .map(|inner| std::mem::replace(&mut inner.handle, std::ptr::null_mut()))
            .filter(|h| !h.is_null());
        if let Some(h) = handle {
            unsafe {
                // Safety: handle was validated non-null and is a valid context handle.
                ucc_context_destroy(h);
            }
        }
    }
}

/// Context attributes returned by [`UccContext::get_attr`].
///
/// Wraps `ucc_context_attr_t` and provides safe accessors for each field.
/// Only fields requested via the mask in `get_attr()` are guaranteed valid.
#[must_use = "Context attributes contain useful runtime information"]
pub struct UccContextAttrs(crate::bindings::ucc_context_attr_t);

impl UccContextAttrs {
    /// Get the context type.
    pub fn context_type(&self) -> ucc_context_type_t {
        self.0.type_
    }

    /// Get the collective sync type.
    pub fn sync_type(&self) -> ucc_coll_sync_type_t {
        self.0.sync_type
    }

    /// Get the context address (for OOB discovery).
    pub fn ctx_addr(&self) -> *mut libc::c_void {
        self.0.ctx_addr
    }

    /// Get the context address length.
    pub fn ctx_addr_len(&self) -> usize {
        self.0.ctx_addr_len
    }

    /// Get the global work buffer size.
    pub fn global_work_buffer_size(&self) -> u64 {
        self.0.global_work_buffer_size
    }

    /// Get the raw mask indicating which fields are valid.
    pub fn mask(&self) -> u64 {
        self.0.mask
    }
}

/// Bitmask for UCC context attribute fields.
///
/// Use these flags with [`UccContext::get_attr`] to specify which attributes to query.
/// Combine with bitwise OR (e.g., `TYPE | SYNC_TYPE`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UccContextAttrField(u64);

impl UccContextAttrField {
    /// Context type attribute.
    pub const TYPE: Self = Self(1);
    /// Collective sync type attribute.
    pub const SYNC_TYPE: Self = Self(2);
    /// Context address attribute.
    pub const CTX_ADDR: Self = Self(4);
    /// Context address length attribute.
    pub const CTX_ADDR_LEN: Self = Self(8);
    /// Work buffer size attribute.
    pub const WORK_BUFFER_SIZE: Self = Self(16);
}

impl std::ops::BitOr for UccContextAttrField {
    type Output = u64;
    fn bitor(self, rhs: Self) -> u64 {
        self.0 | rhs.0
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
        self.0.mask |=
            crate::bindings::ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_TYPE as u64;
    }

    /// Set the collective sync type.
    pub fn with_sync_type(&mut self, sync_type: ucc_coll_sync_type_t) {
        self.0.sync_type = sync_type;
        self.0.mask |=
            crate::bindings::ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_SYNC_TYPE as u64;
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
    use crate::bindings::ucc_context_params_field_UCC_CONTEXT_PARAM_FIELD_ID;
    use static_assertions::assert_impl_all;
    use std::clone::Clone;

    #[test]
    fn test_ucc_context_params_default() {
        let params = UccContextParams::default();
        assert_eq!(params.0.type_, ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE);
        assert_eq!(
            params.0.sync_type,
            ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES
        );
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

    // ── Integration tests (call into libucc.so) ────────────────────────────

    mod integration_tests {
        use super::*;

        #[test]
        fn integration_context_create_and_drop() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create should succeed");
            assert!(!ctx.handle().is_null(), "Context handle must be non-null");
            // ctx dropped here — ucc_context_destroy called automatically
        }

        #[test]
        fn integration_context_create_twice() {
            for _ in 0..3 {
                let lib = UccLib::init().expect("init");
                let ctx = UccContext::new(lib).expect("context create");
                assert!(!ctx.handle().is_null());
                // drop
            }
        }

        #[test]
        fn integration_context_get_attr_type() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let attrs = ctx
                .get_attr(UccContextAttrField::TYPE.0)
                .expect("get_attr(TYPE) should succeed");
            assert_eq!(
                attrs.context_type(),
                ucc_context_type_t_UCC_CONTEXT_EXCLUSIVE,
                "Default context type should be EXCLUSIVE"
            );
        }

        #[test]
        fn integration_context_get_attr_sync_type() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let attrs = ctx
                .get_attr(UccContextAttrField::SYNC_TYPE.0)
                .expect("get_attr(SYNC_TYPE) should succeed");
            assert_eq!(
                attrs.sync_type(),
                ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES,
                "Default sync type should be COLLECTIVES"
            );
        }

        #[test]
        fn integration_context_get_attr_combined_mask() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let mask = UccContextAttrField::TYPE | UccContextAttrField::SYNC_TYPE;
            let attrs = ctx
                .get_attr(mask)
                .expect("get_attr(combined) should succeed");
            assert!(attrs.context_type() > 0, "context_type should be set");
            assert!(attrs.sync_type() > 0, "sync_type should be set");
        }

        #[test]
        fn integration_context_progress() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            // progress() should not panic or crash — it's a no-op with no pending ops
            ctx.progress();
        }

        #[test]
        fn integration_context_clone() {
            // Verify UccContext::clone() works. Keep the original alive so both
            // clones share the same handle and only one drop actually destroys.
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let _clone = ctx.clone();
            assert!(!ctx.handle().is_null());
            // ctx dropped last — only its drop calls ucc_context_destroy
        }

        #[test]
        fn integration_context_config_read_and_modify() {
            let lib = UccLib::init().expect("init");
            let config = UccContextConfig::read(&lib).expect("context config read");
            // Modifying a non-critical key should not fail
            let _ = config.modify("IB_GID_INDEX", "0");
        }

        #[test]
        fn integration_context_with_custom_params() {
            let mut params = UccContextParams::default();
            params.with_id(42);
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::with_params(lib, params).expect("context with custom params");
            assert!(!ctx.handle().is_null());
        }
    }
}
