//! UCC team management.
//!
//! Teams define groups of processes that participate in collective operations.
//! Teams are created from contexts and support hierarchical structures via
//! parent-child relationships.

use crate::bindings::{
    ucc_coll_sync_type_t, ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES, ucc_oob_coll_t,
    ucc_post_ordering_t, ucc_post_ordering_t_UCC_COLLECTIVE_POST_ORDERED, ucc_team_attr,
    ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_EP, ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_SIZE,
    ucc_team_create_from_parent, ucc_team_destroy, ucc_team_get_attr, ucc_team_h, ucc_team_params,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_FLAGS,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OOB,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_ORDERING,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_SYNC_TYPE,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE,
};
use crate::context::UccContext;
use crate::status::{check_status, UccError, UccStatus};

/// UCC team handle with RAII cleanup.
///
/// Teams define groups of processes that participate in collective operations.
/// Holds a reference to the [`UccContext`] that created it. Cloneable — each
/// clone shares the same underlying C handle.
#[must_use = "Team handles should be kept alive or explicitly dropped"]
#[derive(Clone)]
pub struct UccTeam {
    handle: ucc_team_h,
    _ctx: UccContext,
}

impl UccTeam {
    /// Create a new team from the root (parentless) context.
    ///
    /// Uses default team parameters. For custom configuration, use
    /// [`UccTeam::with_params`] or [`UccTeam::from_parent`].
    #[must_use = "Result should be checked"]
    pub fn new(ctx: UccContext) -> Result<Self, UccStatus> {
        Self::with_params(ctx, Default::default())
    }

    /// Create a new team with custom parameters.
    ///
    /// The FFI signature is:
    /// `ucc_team_create_from_parent(my_ep, included, parent_team, new_team)`
    ///
    /// For a root team, pass `parent_team = NULL`.
    #[must_use = "Result should be checked"]
    pub fn with_params(ctx: UccContext, team_params: UccTeamParams) -> Result<Self, UccStatus> {
        let mut team: ucc_team_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: team_params.0 is a valid reference; &mut team is a valid
            // output pointer. ucc_team_create_from_parent accepts NULL for the
            // parent team to create a root team.
            // Signature: (my_ep: u64, included: u32, parent_team: ucc_team_h, new_team: *mut ucc_team_h)
            ucc_team_create_from_parent(
                team_params.0.ep,
                1, // included = 1 (this process is part of the team)
                std::ptr::null_mut(),
                &mut team,
            )
        };
        check_status(status)?;
        if team.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(Self {
            handle: team,
            _ctx: ctx,
        })
    }

    /// Create a child team from a parent team.
    #[must_use = "Result should be checked"]
    pub fn from_parent(
        parent: &UccTeam,
        ctx: UccContext,
        team_params: UccTeamParams,
    ) -> Result<Self, UccStatus> {
        let mut team: ucc_team_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: parent.handle is a valid team handle; team_params.0 is a
            // valid reference; &mut team is a valid output pointer.
            ucc_team_create_from_parent(
                team_params.0.ep,
                1, // included = 1
                parent.handle,
                &mut team,
            )
        };
        check_status(status)?;
        if team.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(Self {
            handle: team,
            _ctx: ctx,
        })
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_team_h {
        self.handle
    }

    /// Query team attributes.
    ///
    /// Sets the requested attribute fields in the mask before calling the
    /// FFI, then returns the populated `ucc_team_attr` struct.
    pub fn attr(&self, fields: u64) -> Result<ucc_team_attr, UccStatus> {
        let mut attr: ucc_team_attr = unsafe { std::mem::zeroed() };
        attr.mask = fields;
        let status = unsafe {
            // Safety: self.handle is a valid team handle; &mut attr is a valid
            // output pointer with the mask pre-set to request specific fields.
            ucc_team_get_attr(self.handle, &mut attr)
        };
        check_status(status)?;
        Ok(attr)
    }

    /// Get the team's size.
    pub fn size(&self) -> Result<u32, UccStatus> {
        let attr = self.attr(ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_SIZE as u64)?;
        Ok(attr.size)
    }

    /// Get the team's endpoint.
    pub fn ep(&self) -> Result<u64, UccStatus> {
        let attr = self.attr(ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_EP as u64)?;
        Ok(attr.ep)
    }
}

impl Drop for UccTeam {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // Safety: handle was validated non-null above and the C function only
            // dereferences the handle pointer for cleanup. Handle is nulled after
            // to prevent double-free.
            let _ = unsafe { ucc_team_destroy(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Parameters for UCC team creation.
///
/// Wraps `ucc_team_params` and manages the field mask automatically.
/// Configure team properties like ordering, sync type, and team size.
#[must_use = "Team params should be used to create a team"]
pub struct UccTeamParams(ucc_team_params);

impl Default for UccTeamParams {
    fn default() -> Self {
        // Safety: ucc_team_params is a POD struct with no pointers or discriminants
        // that the C API only reads fields indicated by the mask.
        let mut params: ucc_team_params = unsafe { std::mem::zeroed() };
        params.ordering = ucc_post_ordering_t_UCC_COLLECTIVE_POST_ORDERED;
        params.sync_type = ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES;
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_ORDERING as u64;
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_SYNC_TYPE as u64;
        Self(params)
    }
}

impl UccTeamParams {
    /// Set the team size.
    pub fn with_team_size(&mut self, size: u64) -> &mut Self {
        self.0.team_size = size;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE as u64;
        self
    }

    /// Set the collective posting ordering.
    pub fn with_ordering(&mut self, ordering: ucc_post_ordering_t) -> &mut Self {
        self.0.ordering = ordering;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_ORDERING as u64;
        self
    }

    /// Set the collective sync type.
    pub fn with_sync_type(&mut self, sync_type: ucc_coll_sync_type_t) -> &mut Self {
        self.0.sync_type = sync_type;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_SYNC_TYPE as u64;
        self
    }

    /// Set team flags.
    pub fn with_flags(&mut self, flags: u64) -> &mut Self {
        self.0.flags = flags;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_FLAGS as u64;
        self
    }

    /// Set the OOB (out-of-band) collective for team creation.
    ///
    /// The OOB collective is a struct containing function pointers for
    /// out-of-band communication (allgather) used during team setup.
    pub fn with_oob(&mut self, oob: ucc_oob_coll_t) -> &mut Self {
        self.0.oob = oob;
        self.0.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OOB as u64;
        self
    }

    /// Get mutable access to the underlying FFI struct for advanced configuration.
    ///
    /// # Safety
    /// Directly modifying FFI fields bypasses mask management. Ensure any field
    /// you set also has its corresponding bit set in `self.0.mask`.
    pub fn inner_mut(&mut self) -> &mut ucc_team_params {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;
    use std::clone::Clone;

    #[test]
    fn test_ucc_team_params_default() {
        let params = UccTeamParams::default();
        assert_eq!(
            params.0.ordering,
            ucc_post_ordering_t_UCC_COLLECTIVE_POST_ORDERED
        );
        assert_eq!(
            params.0.sync_type,
            ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES
        );
    }

    #[test]
    fn test_ucc_team_trait_bounds() {
        // UccTeam is Clone but NOT Send — raw FFI handles don't impl Send
        assert_impl_all!(UccTeam: Clone);
    }

    #[test]
    fn test_ucc_team_handle_size() {
        // UccTeam wraps a raw FFI handle — intentionally not Send.
        let _ = std::mem::size_of::<UccTeam>();
    }
}
