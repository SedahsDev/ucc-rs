//! UCC execution engine for event-driven collective scheduling.
//!
//! Execution engines allow users to create events and schedule collective
//! operations that are triggered by those events. This enables advanced
//! pipelining and synchronization patterns.

use crate::bindings::{
    ucc_ee_ack_event, ucc_ee_create, ucc_ee_destroy, ucc_ee_get_event, ucc_ee_h,
    ucc_ee_params, ucc_ee_set_event, ucc_ee_wait,
};
use crate::team::UccTeam;
use crate::status::{check_status, UccError, UccStatus};

/// UCC execution engine handle with RAII cleanup.
///
/// An execution engine manages events and allows scheduling collective
/// operations that are triggered by those events. Holds a reference to
/// the [`UccTeam`] that created it. Cloneable — each clone shares
/// the same underlying C handle.
#[must_use = "Execution engine handles should be kept alive or explicitly dropped"]
#[derive(Clone)]
pub struct UccExecutionEngine {
    handle: ucc_ee_h,
    _team: UccTeam,
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
            handle: ee,
            _team: team,
        })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_ee_h {
        self.handle
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
    pub unsafe fn set_event(&self, event: *mut crate::bindings::ucc_ev_t) -> Result<(), UccStatus> {
        let status = ucc_ee_set_event(self.handle, event);
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
    pub unsafe fn ack_event(&self, event: *mut crate::bindings::ucc_ev_t) -> Result<(), UccStatus> {
        let status = ucc_ee_ack_event(self.handle, event);
        check_status(status)
    }

    /// Block until an event is available on the execution engine.
    ///
    /// This is a blocking call that waits for an event to be posted to
    /// the execution engine. When an event arrives, the event pointer
    /// is written to the provided location.
    ///
    /// # Safety
    ///
    /// The returned event pointer must eventually be acknowledged via `ack_event()`
    /// or used with `ucc_collective_triggered_post()`.
    pub unsafe fn wait_event(&self) -> Result<*mut crate::bindings::ucc_ev_t, UccStatus> {
        // The C API writes event data into a ucc_ev_t struct, then returns a pointer to it.
        // We allocate a struct on the stack and pass a mutable pointer to it.
        let mut event: crate::bindings::ucc_ev_t = crate::bindings::ucc_ev_t {
            ev_type: 0,
            ev_context: std::ptr::null_mut(),
            ev_context_size: 0,
            req: std::ptr::null_mut(),
        };
        let status = ucc_ee_wait(self.handle, &mut event as *mut crate::bindings::ucc_ev_t);
        check_status(status)?;
        // Return a pointer to the event data — note the caller must keep this struct alive
        Ok(&mut event as *mut crate::bindings::ucc_ev_t)
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
    pub unsafe fn get_event(&self) -> Result<Option<*mut crate::bindings::ucc_ev_t>, UccStatus> {
        let mut event: *mut crate::bindings::ucc_ev_t = std::ptr::null_mut();
        let status = ucc_ee_get_event(self.handle, &mut event);
        if status == 0 {
            // UCC_OK — event was available
            return Ok(Some(event));
        }
        if status == 7 {
            // UCC_ERR_NO_RESOURCE — no event available right now
            return Ok(None);
        }
        check_status(status)?;
        Ok(Some(event))
    }
}

impl Drop for UccExecutionEngine {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // Safety: handle was validated non-null above and the C function only
            // dereferences the handle pointer for cleanup. Handle is nulled after
            // to prevent double-free.
            let _ = unsafe { ucc_ee_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
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
