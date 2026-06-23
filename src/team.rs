//! UCC team creation and management.
//!
//! A team is a group of processes that participate in collective operations.
//! Teams are created within a context and can be nested (subteams).

use crate::bindings::{
    ucc_context_h, ucc_context_oob_coll_t,
    ucc_count_t, ucc_team_attr_t,
    ucc_team_h,
    ucc_team_create_post, ucc_team_create_test, ucc_team_destroy, ucc_team_get_attr,
    ucc_team_params,
    // constified enum constants
    ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES,
    ucc_post_ordering_t_UCC_COLLECTIVE_POST_ORDERED,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_ORDERING,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OUTSTANDING_COLLS,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP_RANGE,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_SYNC_TYPE,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OOB,
    ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_EP,
    ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_SIZE,
};
use crate::context::UccContext;
use crate::status::{check_status, UccError, UccStatus};

/// UCC team handle with RAII cleanup.
pub struct UccTeam {
    pub(crate) handle: ucc_team_h,
    pub(crate) _context: UccContext,
}

impl UccTeam {
    /// Create a new team with default parameters.
    pub fn new(context: UccContext, ep: ucc_count_t, size: ucc_count_t) -> Result<Self, UccStatus> {
        let mut params = UccTeamParams::default();
        params.with_ep(ep);
        params.with_size(size);
        Self::with_params(context, params)
    }

    /// Create a new team with custom parameters (two-phase: post + test).
    pub fn with_params(context: UccContext, team_params: UccTeamParams) -> Result<Self, UccStatus> {
        let mut ctx_handle = context.handle();
        let contexts: *mut ucc_context_h = &mut ctx_handle;
        let mut team: ucc_team_h = std::ptr::null_mut();
        let status = unsafe {
            ucc_team_create_post(contexts, 1, &team_params.0, &mut team)
        };
        check_status(status)?;
        if team.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        // Two-phase: test until team creation completes
        loop {
            let status = unsafe { ucc_team_create_test(team) };
            if status == 0 {
                // UCC_OK
                break;
            }
            // UCC_INPROGRESS = 1, continue polling
        }
        Ok(Self {
            handle: team,
            _context: context,
        })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_team_h {
        self.handle
    }

    /// Get the endpoint rank for this team.
    pub fn ep(&self) -> Result<u64, UccStatus> {
        let mut attr: ucc_team_attr_t = unsafe { std::mem::zeroed() };
        attr.mask = ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_EP as u64;
        let status = unsafe { ucc_team_get_attr(self.handle, &mut attr) };
        check_status(status)?;
        Ok(attr.ep)
    }

    /// Get the team size.
    pub fn size(&self) -> Result<u32, UccStatus> {
        let mut attr: ucc_team_attr_t = unsafe { std::mem::zeroed() };
        attr.mask = ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_SIZE as u64;
        let status = unsafe { ucc_team_get_attr(self.handle, &mut attr) };
        check_status(status)?;
        Ok(attr.size)
    }
}

impl Drop for UccTeam {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ucc_team_destroy(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

impl Clone for UccTeam {
    fn clone(&self) -> Self {
        UccTeam {
            handle: self.handle,
            _context: self._context.clone(),
        }
    }
}

/// Parameters for UCC team creation.
pub struct UccTeamParams(ucc_team_params);

impl Default for UccTeamParams {
    fn default() -> Self {
        let mut params: ucc_team_params = unsafe { std::mem::zeroed() };
        params.ordering = ucc_post_ordering_t_UCC_COLLECTIVE_POST_ORDERED;
        params.outstanding_colls = 1;
        params.ep = 0;
        params.ep_range = 0; // UCC_COLLECTIVE_EP_RANGE_CONTIG
        params.team_size = 1;
        params.sync_type = ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES;
        params.oob.allgather = None;
        params.oob.req_test = None;
        params.oob.req_free = None;
        params.oob.coll_info = std::ptr::null_mut();
        params.oob.n_oob_eps = 0;
        params.oob.oob_ep = 0;
        params.mask = ucc_team_params_field_UCC_TEAM_PARAM_FIELD_ORDERING as u64
            | ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OUTSTANDING_COLLS as u64
            | ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP as u64
            | ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP_RANGE as u64
            | ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE as u64
            | ucc_team_params_field_UCC_TEAM_PARAM_FIELD_SYNC_TYPE as u64;
        Self(params)
    }
}

impl UccTeamParams {
    /// Set the endpoint rank.
    pub fn with_ep(&mut self, ep: ucc_count_t) {
        self.0.ep = ep;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP as u64;
    }

    /// Set the team size.
    pub fn with_size(&mut self, size: ucc_count_t) {
        self.0.team_size = size;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE as u64;
    }

    /// Set the OOB collective functions.
    pub fn with_oob(&mut self, oob: ucc_context_oob_coll_t) {
        self.0.oob = oob;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OOB as u64;
    }

    /// Set the outstanding collectives limit.
    pub fn with_outstanding_colls(&mut self, count: ucc_count_t) {
        self.0.outstanding_colls = count;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OUTSTANDING_COLLS as u64;
    }

    /// Access the inner params for advanced configuration.
    pub fn inner_mut(&mut self) -> &mut ucc_team_params {
        &mut self.0
    }
}
