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
        check_status(status)?;
        if ee.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(Self {
            inner: std::rc::Rc::new(UccExecutionEngineInner { handle: ee }),
            _team: team,
        })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_ee_h {
        self.inner.handle
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
    pub unsafe fn set_event_raw(
        &self,
        event: *mut crate::bindings::ucc_ev_t,
    ) -> Result<(), UccStatus> {
        let status = ucc_ee_set_event(self.handle(), event);
        check_status(status)
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
    pub unsafe fn ack_event_raw(
        &self,
        event: *mut crate::bindings::ucc_ev_t,
    ) -> Result<(), UccStatus> {
        let status = ucc_ee_ack_event(self.handle(), event);
        check_status(status)
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
            ucc_status_t_UCC_OK if !event.is_null() => Ok(Some(UccEvent { ptr: event })),
            ucc_status_t_UCC_OK => {
                // A successful call with a null event has no event to return.
                Ok(None)
            }
            ucc_status_t_UCC_ERR_NO_RESOURCE => Ok(None),
            status => Err(check_status(status).expect_err("non-OK status must be an error")),
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

/// An event pointer returned by the execution engine.
///
/// The storage is owned by UCC and remains valid until the event is
/// acknowledged. Dropping this wrapper does not acknowledge or free it.
pub struct UccEvent {
    ptr: *mut crate::bindings::ucc_ev_t,
}

impl UccEvent {
    /// Return the raw event pointer for APIs such as `triggered_post`.
    ///
    /// # Safety
    /// The pointer is only valid while this event is alive and, for borrowed
    /// events, until UCC releases it after acknowledgement.
    pub fn as_ptr(&self) -> *mut crate::bindings::ucc_ev_t {
        self.ptr
    }
}

impl Drop for UccEvent {
    fn drop(&mut self) {}
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
    /// Get mutable access to the underlying FFI struct for advanced configuration.
    ///
    /// # Safety
    /// Directly modifying FFI fields bypasses mask management. Ensure any field
    /// you set also has its corresponding bit set in `self.0.mask`.
    pub fn inner_mut(&mut self) -> &mut ucc_ee_params {
        &mut self.0
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
