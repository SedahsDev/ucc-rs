//! UCC Execution Engine (EE) for triggered/async collective operations.
//!
//! The Execution Engine provides an event-driven model for collective
//! operations. Instead of polling, the caller sets up an event via
//! [`UccExecutionEngine::set_collective_post`] or
//! [`UccExecutionEngine::set_collective_complete`], and retrieves
//! notifications via [`UccExecutionEngine::get_event`].
//!
//! Events are wrapped in [`UccEvent`], which exposes the event type, context
//! pointer, and convenience methods like [`UccEvent::is_collective_complete`].

use crate::bindings::{
    ucc_ee_create, ucc_ee_destroy, ucc_ee_get_event, ucc_ee_h, ucc_ee_params_t,
    ucc_ee_set_event, ucc_ev_t,
    // constified enum constants
    ucc_event_type_UCC_EVENT_COLLECTIVE_COMPLETE,
    ucc_event_type_UCC_EVENT_COLLECTIVE_POST,
};
use crate::team::UccTeam;
use crate::status::{check_status, UccError, UccStatus};

/// UCC Execution Engine handle with RAII cleanup.
///
/// Manages the event-driven execution engine for triggered/async collective
/// operations. Holds a reference to the [`UccTeam`] it was created from,
/// keeping the team alive. Automatically calls `ucc_ee_destroy` on drop.
pub struct UccExecutionEngine {
    handle: ucc_ee_h,
    _team: UccTeam,
}

impl UccExecutionEngine {
    /// Create a new execution engine.
    pub fn new(team: UccTeam) -> Result<Self, UccStatus> {
        let team_handle = team.handle();
        let mut ee: ucc_ee_h = std::ptr::null_mut();
        let ee_params: ucc_ee_params_t = unsafe { std::mem::zeroed() };
        let status = unsafe { ucc_ee_create(team_handle, &ee_params, &mut ee) };
        check_status(status)?;
        if ee.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(Self {
            handle: ee,
            _team: team,
        })
    }

    /// Set an event on the execution engine.
    pub unsafe fn set_event(&self, ev_type: u32, ev_context: *mut std::os::raw::c_void) -> Result<(), UccStatus> {
        let mut ev: ucc_ev_t = unsafe { std::mem::zeroed() };
        ev.ev_type = ev_type;
        ev.ev_context = ev_context;
        ev.ev_context_size = 0;
        let status = unsafe { ucc_ee_set_event(self.handle, &mut ev) };
        check_status(status)
    }

    /// Set a collective post event.
    pub fn set_collective_post(&self) -> Result<(), UccStatus> {
        unsafe { self.set_event(ucc_event_type_UCC_EVENT_COLLECTIVE_POST, std::ptr::null_mut()) }
    }

    /// Set a collective complete event.
    pub fn set_collective_complete(&self) -> Result<(), UccStatus> {
        unsafe { self.set_event(ucc_event_type_UCC_EVENT_COLLECTIVE_COMPLETE, std::ptr::null_mut()) }
    }

    /// Get an event from the execution engine's event queue.
    pub fn get_event(&self) -> Result<UccEvent, UccStatus> {
        let mut ev: *mut ucc_ev_t = std::ptr::null_mut();
        let status = unsafe { ucc_ee_get_event(self.handle, &mut ev) };
        check_status(status)?;
        let raw = unsafe { &*ev };
        Ok(UccEvent {
            ev_type: raw.ev_type,
            ev_context: raw.ev_context,
            ev_context_size: raw.ev_context_size as u64,
        })
    }

    /// Get the handle.
    pub fn handle(&self) -> ucc_ee_h {
        self.handle
    }
}

/// Safe wrapper around a UCC event (`ucc_ev_t`).
///
/// Represents an event retrieved from the execution engine's event queue.
/// Provides convenience methods to check the event type, such as
/// [`is_collective_complete`](Self::is_collective_complete) and
/// [`is_collective_post`](Self::is_collective_post).
pub struct UccEvent {
    ev_type: u32,
    ev_context: *mut std::os::raw::c_void,
    ev_context_size: u64,
}

impl UccEvent {
    /// Get the event type.
    pub fn event_type(&self) -> u32 {
        self.ev_type
    }

    /// Get the event context pointer.
    pub fn context(&self) -> *mut std::os::raw::c_void {
        self.ev_context
    }

    /// Get the event context size.
    pub fn context_size(&self) -> u64 {
        self.ev_context_size
    }

    /// Check if this is a collective complete event.
    pub fn is_collective_complete(&self) -> bool {
        self.ev_type == crate::bindings::ucc_event_type_UCC_EVENT_COLLECTIVE_COMPLETE
    }

    /// Check if this is a collective post event.
    pub fn is_collective_post(&self) -> bool {
        self.ev_type == crate::bindings::ucc_event_type_UCC_EVENT_COLLECTIVE_POST
    }
}

impl Drop for UccExecutionEngine {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ucc_ee_destroy(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}
