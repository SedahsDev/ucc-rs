//! UCC execution engine for event-driven collective scheduling.
//!
//! Execution engines allow users to create events and schedule collective
//! operations that are triggered by those events. This enables advanced
//! pipelining and synchronization patterns.

use crate::bindings::{
    ucc_ee_ack_event, ucc_ee_create, ucc_ee_destroy, ucc_ee_get_event, ucc_ee_h, ucc_ee_params,
    ucc_ee_set_event, ucc_status_t_UCC_ERR_NO_RESOURCE, ucc_status_t_UCC_OK,
};
use crate::status::{check_status, UccError, UccStatus};
use crate::team::UccTeam;
use std::os::raw::c_void;

/// Native wrapper for UCC event type (ucc_ev_t).
///
/// Used for triggered collective operations. This is a newtype wrapper
/// around the raw FFI type to prevent direct FFI exposure in public APIs.
///
/// An event pointer returned by the execution engine.
///
/// The storage is owned by UCC and remains valid until the event is
/// acknowledged. Dropping this wrapper does not acknowledge or free it.
#[repr(transparent)]
pub struct UccEvent {
    pub(crate) inner: *mut crate::bindings::ucc_ev_t,
}

impl UccEvent {
    /// Create a new UccEvent from a raw pointer.
    ///
    /// # Safety
    /// `raw` must be a valid pointer to a ucc_ev_t structure.
    pub(crate) unsafe fn from_raw(raw: *mut crate::bindings::ucc_ev_t) -> Self {
        Self { inner: raw }
    }

    /// Get the raw pointer.
    ///
    /// # Safety
    /// The returned pointer must only be used in FFI calls where UCC expects ucc_ev_t.
    pub(crate) fn as_raw(&self) -> *mut crate::bindings::ucc_ev_t {
        self.inner
    }
}

/// UCC execution engine handle with RAII cleanup.
///
/// An execution engine manages events and allows scheduling collective
/// operations that are triggered by those events. Holds a reference to
/// the [`UccTeam`] that created it. Cloneable — each clone shares
/// the same underlying C handle.
struct UccExecutionEngineInner {
    handle: ucc_ee_h,
}

#[must_use = "Execution engine handles should be kept alive or explicitly dropped"]
pub struct UccExecutionEngine {
    inner: std::rc::Rc<UccExecutionEngineInner>,
    _team: UccTeam,
}

impl Clone for UccExecutionEngine {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _team: self._team.clone(),
        }
    }
}

impl UccExecutionEngine {
    /// Create a new execution engine.
    #[must_use = "Result should be checked"]
    pub fn new(team: UccTeam) -> Result<Self, UccStatus> {
        Self::with_params(team, Default::default())
    }

    /// Create a new execution engine with custom parameters.
    #[must_use = "Result should be checked"]
    pub fn with_params(
        team: UccTeam,
        ee_params: UccExecutionEngineParams,
    ) -> Result<Self, UccStatus> {
        let mut ee: ucc_ee_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: team.handle() is a valid team handle; &ee_params.0 is a valid
            // reference; &mut ee is a valid output pointer.
            // ucc_ee_create takes (team, params, ee) — team-based, not context-based.
            ucc_ee_create(team.handle(), &ee_params.0, &mut ee)
        };
        check_status(UccStatus::from_raw(status))?;
        if ee.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(Self {
            inner: std::rc::Rc::new(UccExecutionEngineInner { handle: ee }),
            _team: team,
        })
    }

    /// Get the raw handle.
    ///
    /// # Safety
    /// The returned handle must only be used in FFI calls where UCC expects ucc_ee_h.
    pub(crate) fn as_raw(&self) -> crate::bindings::ucc_ee_h {
        self.inner.handle
    }

    /// Get a raw handle for use with lower-level APIs.
    pub(crate) fn handle(&self) -> ucc_ee_h {
        self.as_raw()
    }

    /// Set a UCC-owned event.
    pub fn set_event(&self, event: &UccEvent) -> Result<(), UccStatus> {
        unsafe { self.set_event_raw(event.as_ptr()) }
    }

    /// Set an event on the execution engine.
    ///
    /// Signals that an event has occurred, potentially triggering
    /// scheduled collective operations.
    ///
    /// # Safety
    ///
    /// The event pointer must be valid and was obtained from the execution engine
    /// or created by the user. The event is consumed by this call.
    pub(crate) unsafe fn set_event_raw(
        &self,
        event: *mut crate::bindings::ucc_ev_t,
    ) -> Result<(), UccStatus> {
        let status = ucc_ee_set_event(self.handle(), event);
        check_status(UccStatus::from_raw(status))
    }

    /// Acknowledge an event from the execution engine.
    ///
    /// Tells the execution engine that the user has finished processing
    /// an event. The event structure will be released by the execution engine.
    ///
    /// # Safety
    ///
    /// The event pointer must be valid and was obtained from the execution engine
    /// (e.g., via a callback or `wait_event()`).
    pub fn ack_event(&self, event: &UccEvent) -> Result<(), UccStatus> {
        unsafe { self.ack_event_raw(event.as_ptr()) }
    }

    /// Acknowledge a raw event pointer. The caller must ensure it came from this EE.
    ///
    /// # Safety
    /// `event` must be a valid event pointer owned by this execution engine.
    pub(crate) unsafe fn ack_event_raw(
        &self,
        event: *mut crate::bindings::ucc_ev_t,
    ) -> Result<(), UccStatus> {
        let status = ucc_ee_ack_event(self.handle(), event);
        check_status(UccStatus::from_raw(status))
    }

    /// Block until an event is available on the execution engine.
    ///
    /// This is an unbounded blocking wrapper around [`Self::get_event`]. It
    /// polls the event queue, progresses the owning context when the queue is
    /// empty, and briefly yields between attempts. The returned event is a
    /// pointer to UCC-owned storage; it must be acknowledged before the
    /// execution engine is destroyed.
    ///
    /// # Safety
    ///
    /// The returned event pointer must eventually be acknowledged via `ack_event()`
    /// or used with `ucc_collective_triggered_post()`.
    pub unsafe fn wait_event(&self) -> Result<UccEvent, UccStatus> {
        loop {
            if let Some(event) = self.get_event()? {
                return Ok(event);
            }
            self._team.progress();
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }

    /// Get an event from the execution engine (non-blocking).
    ///
    /// Returns an event if one is available, otherwise returns `None`.
    /// This is the non-blocking counterpart to `wait_event()`.
    ///
    /// # Safety
    ///
    /// The returned event pointer must eventually be acknowledged via `ack_event()`
    /// or used with `ucc_collective_triggered_post()`.
    #[allow(non_upper_case_globals)]
    pub unsafe fn get_event(&self) -> Result<Option<UccEvent>, UccStatus> {
        let mut event: *mut crate::bindings::ucc_ev_t = std::ptr::null_mut();
        // SAFETY: the EE handle is valid for the lifetime of `self`, and UCC
        // writes only the returned event pointer into this valid output slot.
        // The pointed-to event remains owned by UCC until acknowledgement.
        let status = ucc_ee_get_event(self.handle(), &mut event);
        match status {
            ucc_status_t_UCC_OK if !event.is_null() => Ok(Some(UccEvent::from_raw(event))),
            ucc_status_t_UCC_OK => {
                // A successful call with a null event has no event to return.
                Ok(None)
            }
            ucc_status_t_UCC_ERR_NO_RESOURCE => Ok(None),
            status => Err(check_status(UccStatus::from_raw(status))
                .expect_err("non-OK status must be an error")),
        }
    }
}

impl Drop for UccExecutionEngine {
    fn drop(&mut self) {
        let handle = std::rc::Rc::get_mut(&mut self.inner)
            .map(|inner| std::mem::replace(&mut inner.handle, std::ptr::null_mut()))
            .filter(|h| !h.is_null());
        if let Some(handle) = handle {
            // SAFETY: handle is the valid, uniquely owned execution-engine handle.
            let _ = unsafe { ucc_ee_destroy(handle) };
        }
    }
}

impl UccEvent {
    /// Return the raw event pointer for APIs such as `triggered_post`.
    ///
    /// # Safety
    /// The pointer is only valid while this event is alive and, for borrowed
    /// events, until UCC releases it after acknowledgement.
    pub(crate) fn as_ptr(&self) -> *mut crate::bindings::ucc_ev_t {
        self.as_raw()
    }
}

impl Drop for UccEvent {
    fn drop(&mut self) {}
}

/// Execution engine type, independent of the generated bindings.
///
/// Mirrors `ucc_ee_type_t` so that raw bindgen constants never appear in
/// public signatures. Follows the same shape as the other native enums in
/// this crate ([`crate::UccThreadMode`], [`crate::UccSyncType`]).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UccEeType {
    /// CUDA stream based execution engine (`UCC_EE_CUDA_STREAM`).
    CudaStream = 0,
    /// CPU thread based execution engine (`UCC_EE_CPU_THREAD`).
    #[default]
    CpuThread = 1,
    /// ROCm stream based execution engine (`UCC_EE_ROCM_STREAM`).
    RocmStream = 2,
}

impl From<crate::bindings::ucc_ee_type_t> for UccEeType {
    fn from(value: crate::bindings::ucc_ee_type_t) -> Self {
        match value {
            crate::bindings::ucc_ee_type_UCC_EE_CUDA_STREAM => Self::CudaStream,
            crate::bindings::ucc_ee_type_UCC_EE_CPU_THREAD => Self::CpuThread,
            crate::bindings::ucc_ee_type_UCC_EE_ROCM_STREAM => Self::RocmStream,
            _ => Self::CpuThread,
        }
    }
}

impl From<UccEeType> for crate::bindings::ucc_ee_type_t {
    fn from(value: UccEeType) -> crate::bindings::ucc_ee_type_t {
        match value {
            UccEeType::CudaStream => crate::bindings::ucc_ee_type_UCC_EE_CUDA_STREAM,
            UccEeType::CpuThread => crate::bindings::ucc_ee_type_UCC_EE_CPU_THREAD,
            UccEeType::RocmStream => crate::bindings::ucc_ee_type_UCC_EE_ROCM_STREAM,
        }
    }
}

/// Parameters for UCC execution engine creation.
///
/// Wraps `ucc_ee_params` and manages the field mask automatically.
/// Currently the execution engine has no configurable parameters beyond
/// the default, but this struct is provided for future extensibility.
#[must_use = "Execution engine params should be used to create an execution engine"]
pub struct UccExecutionEngineParams(ucc_ee_params);

impl Default for UccExecutionEngineParams {
    fn default() -> Self {
        // Safety: ucc_ee_params is a POD struct with no pointers or discriminants
        // that the C API only reads fields indicated by the mask.
        let params: ucc_ee_params = unsafe { std::mem::zeroed() };
        Self(params)
    }
}

impl UccExecutionEngineParams {
    /// Set the execution engine type.
    pub fn with_type(&mut self, ee_type: UccEeType) -> &mut Self {
        self.0.ee_type = ee_type.into();
        self
    }

    /// Set the execution engine context and its size.
    pub fn with_context(&mut self, ctx: *mut c_void, size: usize) -> &mut Self {
        self.0.ee_context = ctx;
        self.0.ee_context_size = size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;
    use std::clone::Clone;

    #[test]
    fn test_ucc_ee_params_default() {
        let _params = UccExecutionEngineParams::default();
        // Just verify it compiles and doesn't panic
    }

    #[test]
    fn test_ucc_ee_params_typed_setters() {
        let ctx = [0u8; 8];
        let mut params = UccExecutionEngineParams::default();
        params
            .with_type(UccEeType::CpuThread)
            .with_context(ctx.as_ptr() as *mut c_void, ctx.len());
        assert_eq!(
            params.0.ee_type,
            crate::bindings::ucc_ee_type_UCC_EE_CPU_THREAD
        );
        assert_eq!(params.0.ee_context_size, 8);
        assert!(!params.0.ee_context.is_null());
    }

    #[test]
    fn test_ucc_ee_type_roundtrip() {
        let cases = [
            (
                UccEeType::CudaStream,
                crate::bindings::ucc_ee_type_UCC_EE_CUDA_STREAM,
            ),
            (
                UccEeType::CpuThread,
                crate::bindings::ucc_ee_type_UCC_EE_CPU_THREAD,
            ),
            (
                UccEeType::RocmStream,
                crate::bindings::ucc_ee_type_UCC_EE_ROCM_STREAM,
            ),
        ];
        for (native, raw) in cases {
            let converted: crate::bindings::ucc_ee_type_t = native.into();
            assert_eq!(converted, raw);
            assert_eq!(UccEeType::from(raw), native);
        }
        // Unknown values fall back to the CPU-thread engine.
        assert_eq!(UccEeType::from(0xFFFF_0000), UccEeType::CpuThread);
    }

    #[test]
    fn test_ucc_ee_trait_bounds() {
        // UccExecutionEngine is Clone but NOT Send — raw FFI handles don't impl Send
        assert_impl_all!(UccExecutionEngine: Clone);
    }

    #[test]
    fn test_ucc_ee_handle_size() {
        // UccExecutionEngine wraps a raw FFI handle — intentionally not Send.
        let _ = std::mem::size_of::<UccExecutionEngine>();
    }
}
