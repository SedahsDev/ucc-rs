//! UCC team management.
//!
//! Teams define groups of processes that participate in collective operations.
//! Teams are created from contexts and support hierarchical structures via
//! parent-child relationships.

use crate::bindings::{
    ucc_coll_sync_type_t, ucc_coll_sync_type_t_UCC_SYNC_COLLECTIVES, ucc_context_h,
    ucc_ep_range_type_t_UCC_COLLECTIVE_EP_RANGE_CONTIG, ucc_oob_coll_t, ucc_post_ordering_t,
    ucc_post_ordering_t_UCC_COLLECTIVE_POST_ORDERED, ucc_status_t_UCC_OK, ucc_team_attr,
    ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_EP, ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_SIZE,
    ucc_team_create_post, ucc_team_create_test, ucc_team_destroy, ucc_team_get_attr, ucc_team_h,
    ucc_team_params, ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP_RANGE,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_FLAGS,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_OOB,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_ORDERING,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_SYNC_TYPE,
    ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE,
};
use crate::collective::{DataType, ReductionOp, UccCollectiveRequest};
use crate::context::UccContext;
use crate::status::{check_status, UccError, UccStatus};

/// Internal shared state for UccTeam — Rc-backed so clones share the
/// same C handle and `ucc_team_destroy` is called exactly once.
/// Note: UCC handles are thread-local by design, so `Rc` (not `Arc`) is correct.
struct UccTeamInner {
    handle: ucc_team_h,
    _ctx: UccContext,
}

/// UCC team handle with RAII cleanup.
///
/// Teams define groups of processes that participate in collective operations.
/// Holds a reference to the [`UccContext`] that created it. Cloneable — each
/// clone shares the same underlying C handle via `Rc`,
/// so `ucc_team_destroy` is called exactly once when the last clone is dropped.
#[must_use = "Team handles should be kept alive or explicitly dropped"]
pub struct UccTeam {
    inner: std::rc::Rc<UccTeamInner>,
}

impl Clone for UccTeam {
    fn clone(&self) -> Self {
        Self {
            inner: std::rc::Rc::clone(&self.inner),
        }
    }
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
    /// Uses the two-phase UCC team creation API:
    /// 1. `ucc_team_create_post` — posts the team creation request
    /// 2. `ucc_team_create_test` — polls until the team is ready
    ///
    /// For a root team, the context array contains a single context handle.
    #[must_use = "Result should be checked"]
    pub fn with_params(ctx: UccContext, team_params: UccTeamParams) -> Result<Self, UccStatus> {
        let mut team: ucc_team_h = std::ptr::null_mut();
        let ctx_handle = ctx.handle();
        let params_ptr: *const ucc_team_params = &team_params.0;

        // Phase 1: Post team creation
        let status = unsafe {
            // Safety: ctx_handle is a valid context handle; params_ptr is valid for the
            // lifetime of team_params; &mut team is a valid output pointer.
            // We pass &ctx_handle as *mut ucc_context_h because the API expects an array
            // of context handles, and we have a single context.
            let ctx_ptr = &ctx_handle as *const ucc_context_h as *mut ucc_context_h;
            ucc_team_create_post(ctx_ptr, 1, params_ptr, &mut team)
        };
        check_status(status)?;
        if team.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }

        // Phase 2: Test/wait for team creation to complete
        let mut iterations = 0;
        loop {
            let test_status = unsafe {
                // Safety: team is a valid handle from ucc_team_create_post
                ucc_team_create_test(team)
            };
            if test_status == ucc_status_t_UCC_OK {
                break;
            }
            // Progress the context while waiting
            ctx.progress();
            iterations += 1;
            if iterations > 10000 {
                unsafe {
                    ucc_team_destroy(team);
                }
                return Err(UccStatus::Known(UccError::ErrTimedOut));
            }
        }

        Ok(Self {
            inner: std::rc::Rc::new(UccTeamInner {
                handle: team,
                _ctx: ctx,
            }),
        })
    }

    /// Create a child team from a parent team.
    ///
    /// NOTE: UCC 1.9.x uses `ucc_team_create_post`/`ucc_team_create_test` for
    /// all team creation. The concept of "parent teams" is managed internally by
    /// UCC based on context hierarchy. This method creates a team using the same
    /// context as the parent — true hierarchical team splitting requires
    /// `ucc_team_create_from_parent` which is not available in this UCC version.
    #[must_use = "Result should be checked"]
    pub fn from_parent(
        _parent: &UccTeam,
        ctx: UccContext,
        team_params: UccTeamParams,
    ) -> Result<Self, UccStatus> {
        // Delegate to with_params — parent team concept is managed by UCC internally
        Self::with_params(ctx, team_params)
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_team_h {
        self.inner.handle
    }

    /// Query team attributes.
    ///
    /// Sets the requested attribute fields in the mask before calling the
    /// FFI, then returns the populated `ucc_team_attr` struct.
    pub fn attr(&self, fields: u64) -> Result<ucc_team_attr, UccStatus> {
        let mut attr: ucc_team_attr = unsafe { std::mem::zeroed() };
        attr.mask = fields;
        let status = unsafe {
            // Safety: self.inner.handle is a valid team handle; &mut attr is a valid
            // output pointer with the mask pre-set to request specific fields.
            ucc_team_get_attr(self.inner.handle, &mut attr)
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

    /// Perform a non-blocking allreduce collective operation.
    ///
    /// This is a convenience method that initializes and posts an allreduce
    /// collective in one step. The operation is in-place — the send buffer
    /// is modified to contain the reduced result.
    ///
    /// # Arguments
    ///
    /// * `buf` — Send/receive buffer (mutable slice). Modified in place.
    /// * `datatype` — Element data type (e.g., `DataType::Uchar`).
    /// * `op` — Reduction operation (e.g., `ReductionOp::Sum`).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::{lib_init::UccLib, context::UccContext, team::UccTeam, collective::{DataType, ReductionOp}};
    /// # let lib = UccLib::init().unwrap();
    /// # let ctx = UccContext::new(lib).unwrap();
    /// # let team = UccTeam::new(ctx).unwrap();
    /// let mut buf = vec![1u8; 1024];
    /// let mut req = team.allreduce(&mut buf, DataType::Uchar, ReductionOp::Sum).unwrap();
    /// while !req.test().unwrap_or(false) {
    ///     // progress...
    /// }
    /// ```
    #[must_use = "Result should be checked"]
    pub fn allreduce(
        &self,
        buf: &mut [u8],
        datatype: DataType,
        op: ReductionOp,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (buf.len() / datatype.size_in_bytes()) as u64;
        let buf_ptr = buf.as_mut_ptr() as *mut std::os::raw::c_void;

        // Safety: ucc_coll_args is a POD struct; we initialize all fields explicitly.
        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE;
        args.src.info.buffer = buf_ptr;
        args.dst.info.buffer = buf_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.op = op.as_raw();
        args.root = 0;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: args is a valid mutable reference; coll_req is a valid output pointer;
            // self.inner.handle is a valid team handle.
            ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle)
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Perform a non-blocking barrier collective operation.
    ///
    /// Synchronizes all ranks in the team. No data is transferred —
    /// this is a synchronization point only.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::{lib_init::UccLib, context::UccContext, team::UccTeam};
    /// # let lib = UccLib::init().unwrap();
    /// # let ctx = UccContext::new(lib).unwrap();
    /// # let team = UccTeam::new(ctx).unwrap();
    /// let mut req = team.barrier().unwrap();
    /// while !req.test().unwrap_or(false) {
    ///     // progress...
    /// }
    /// ```
    #[must_use = "Result should be checked"]
    pub fn barrier(&self) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_BARRIER, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        // UCC requires non-null buffer pointers, so we use a dummy 1-byte buffer.
        let mut dummy = [0u8; 1];
        let buf_ptr = dummy.as_mut_ptr() as *mut std::os::raw::c_void;

        // Safety: ucc_coll_args is a POD struct; we initialize all fields explicitly.
        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_BARRIER;
        args.src.info.buffer = buf_ptr;
        args.dst.info.buffer = buf_ptr;
        args.src.info.count = 0;
        args.src.info.datatype = DataType::Uchar.as_raw();
        args.dst.info.count = 0;
        args.dst.info.datatype = DataType::Uchar.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.op = 0;
        args.root = 0;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: args is a valid mutable reference; coll_req is a valid output pointer;
            // self.inner.handle is a valid team handle.
            ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle)
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Perform a non-blocking allgather collective operation.
    ///
    /// Each rank contributes its `sendbuf` data, and all ranks receive
    /// the concatenated contributions from every rank into `recvbuf`.
    /// The layout in `recvbuf` is contiguous: rank 0's data first, then
    /// rank 1's, and so on.
    ///
    /// # Arguments
    ///
    /// * `sendbuf` — Send buffer containing this rank's contribution.
    /// * `recvbuf` — Receive buffer that will hold all ranks' contributions.
    /// * `datatype` — Element data type (e.g., `DataType::Uchar`).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::{lib_init::UccLib, context::UccContext, team::UccTeam, collective::DataType};
    /// # let lib = UccLib::init().unwrap();
    /// # let ctx = UccContext::new(lib).unwrap();
    /// # let team = UccTeam::new(ctx).unwrap();
    /// let sendbuf = vec![42u8; 256];
    /// let recv_size = 256 * team.size().unwrap() as usize;
    /// let mut recvbuf = vec![0u8; recv_size];
    /// let mut req = team.allgather(&sendbuf, &mut recvbuf, DataType::Uchar).unwrap();
    /// while !req.test().unwrap_or(false) {
    ///     // progress...
    /// }
    /// ```
    #[must_use = "Result should be checked"]
    pub fn allgather(
        &self,
        sendbuf: &[u8],
        recvbuf: &mut [u8],
        datatype: DataType,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHER, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (sendbuf.len() / datatype.size_in_bytes()) as u64;
        let src_ptr = sendbuf.as_ptr() as *mut std::os::raw::c_void;
        let dst_ptr = recvbuf.as_mut_ptr() as *mut std::os::raw::c_void;

        // Safety: ucc_coll_args is a POD struct; we initialize all fields explicitly.
        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHER;
        args.src.info.buffer = src_ptr;
        args.dst.info.buffer = dst_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.op = 0;
        args.root = 0;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: args is a valid mutable reference; coll_req is a valid output pointer;
            // self.inner.handle is a valid team handle.
            ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle)
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Perform a non-blocking broadcast collective operation.
    ///
    /// The root rank sends data to all other ranks. This is an in-place
    /// operation — the same buffer is used for both send and receive.
    ///
    /// # Arguments
    ///
    /// * `buf` — Send/receive buffer. On the root, it contains the data to
    ///   broadcast. On non-roots, it receives the broadcast data.
    /// * `datatype` — Element data type (e.g., `DataType::Uchar`).
    /// * `root` — Rank of the root process that sends the data.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::{lib_init::UccLib, context::UccContext, team::UccTeam, collective::DataType};
    /// # let lib = UccLib::init().unwrap();
    /// # let ctx = UccContext::new(lib).unwrap();
    /// # let team = UccTeam::new(ctx).unwrap();
    /// let mut buf = vec![0u8; 1024];
    /// let mut req = team.bcast(&mut buf, DataType::Uchar, 0).unwrap();
    /// while !req.test().unwrap_or(false) {
    ///     // progress...
    /// }
    /// ```
    #[must_use = "Result should be checked"]
    pub fn bcast(
        &self,
        buf: &mut [u8],
        datatype: DataType,
        root: u32,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_BCAST, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (buf.len() / datatype.size_in_bytes()) as u64;
        let buf_ptr = buf.as_mut_ptr() as *mut std::os::raw::c_void;

        // Safety: ucc_coll_args is a POD struct; we initialize all fields explicitly.
        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_BCAST;
        args.src.info.buffer = buf_ptr;
        args.dst.info.buffer = buf_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.op = 0;
        args.root = root as u64;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: args is a valid mutable reference; coll_req is a valid output pointer;
            // self.inner.handle is a valid team handle.
            ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle)
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Perform a non-blocking reduce collective operation.
    ///
    /// All ranks send data, and the root rank receives the reduced result.
    /// Only the root's receive buffer is populated after completion.
    ///
    /// # Arguments
    ///
    /// * `sendbuf` — Send buffer containing this rank's data.
    /// * `recvbuf` — Receive buffer for the reduced result (only valid on root).
    /// * `datatype` — Element data type (e.g., `DataType::Uchar`).
    /// * `op` — Reduction operation (e.g., `ReductionOp::Sum`).
    /// * `root` — Rank of the root process that receives the result.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ucc::{lib_init::UccLib, context::UccContext, team::UccTeam, collective::{DataType, ReductionOp}};
    /// # let lib = UccLib::init().unwrap();
    /// # let ctx = UccContext::new(lib).unwrap();
    /// # let team = UccTeam::new(ctx).unwrap();
    /// let sendbuf = vec![1u8; 1024];
    /// let mut recvbuf = vec![0u8; 1024];
    /// let mut req = team.reduce(&sendbuf, &mut recvbuf, DataType::Uchar, ReductionOp::Sum, 0).unwrap();
    /// while !req.test().unwrap_or(false) {
    ///     // progress...
    /// }
    /// ```
    #[must_use = "Result should be checked"]
    pub fn reduce(
        &self,
        sendbuf: &[u8],
        recvbuf: &mut [u8],
        datatype: DataType,
        op: ReductionOp,
        root: u32,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_REDUCE, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (sendbuf.len() / datatype.size_in_bytes()) as u64;
        let src_ptr = sendbuf.as_ptr() as *mut std::os::raw::c_void;
        let dst_ptr = recvbuf.as_mut_ptr() as *mut std::os::raw::c_void;

        // Safety: ucc_coll_args is a POD struct; we initialize all fields explicitly.
        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_REDUCE;
        args.src.info.buffer = src_ptr;
        args.dst.info.buffer = dst_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.op = op.as_raw();
        args.root = root as u64;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: args is a valid mutable reference; coll_req is a valid output pointer;
            // self.inner.handle is a valid team handle.
            ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle)
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Non-blocking gather: all ranks send, root receives.
    #[must_use = "Result should be checked"]
    pub fn gather(
        &self,
        sendbuf: &[u8],
        recvbuf: &mut [u8],
        datatype: DataType,
        root: u32,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_GATHER, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (sendbuf.len() / datatype.size_in_bytes().max(1)) as u64;
        let src_ptr = sendbuf.as_ptr() as *mut std::os::raw::c_void;
        let dst_ptr = recvbuf.as_mut_ptr() as *mut std::os::raw::c_void;

        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_GATHER;
        args.src.info.buffer = src_ptr;
        args.dst.info.buffer = dst_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.root = root as u64;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status =
            unsafe { ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle) };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Non-blocking scatter: root sends, all ranks receive.
    #[must_use = "Result should be checked"]
    pub fn scatter(
        &self,
        sendbuf: &[u8],
        recvbuf: &mut [u8],
        datatype: DataType,
        root: u32,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_SCATTER, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (recvbuf.len() / datatype.size_in_bytes().max(1)) as u64;
        let src_ptr = sendbuf.as_ptr() as *mut std::os::raw::c_void;
        let dst_ptr = recvbuf.as_mut_ptr() as *mut std::os::raw::c_void;

        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_SCATTER;
        args.src.info.buffer = src_ptr;
        args.dst.info.buffer = dst_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.root = root as u64;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status =
            unsafe { ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle) };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Non-blocking alltoall.
    #[must_use = "Result should be checked"]
    pub fn alltoall(
        &self,
        sendbuf: &[u8],
        recvbuf: &mut [u8],
        datatype: DataType,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALL, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (sendbuf.len() / datatype.size_in_bytes().max(1)) as u64;
        let src_ptr = sendbuf.as_ptr() as *mut std::os::raw::c_void;
        let dst_ptr = recvbuf.as_mut_ptr() as *mut std::os::raw::c_void;

        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALL;
        args.src.info.buffer = src_ptr;
        args.dst.info.buffer = dst_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status =
            unsafe { ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle) };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }

    /// Non-blocking reduce-scatter.
    #[must_use = "Result should be checked"]
    pub fn reduce_scatter(
        &self,
        sendbuf: &[u8],
        recvbuf: &mut [u8],
        datatype: DataType,
        op: ReductionOp,
    ) -> Result<UccCollectiveRequest, UccStatus> {
        use crate::bindings::{
            ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h,
            ucc_coll_type_t_UCC_COLL_TYPE_REDUCE_SCATTER, ucc_collective_init_and_post,
            ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST,
        };

        let count = (recvbuf.len() / datatype.size_in_bytes().max(1)) as u64;
        let src_ptr = sendbuf.as_ptr() as *mut std::os::raw::c_void;
        let dst_ptr = recvbuf.as_mut_ptr() as *mut std::os::raw::c_void;

        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = ucc_coll_type_t_UCC_COLL_TYPE_REDUCE_SCATTER;
        args.src.info.buffer = src_ptr;
        args.dst.info.buffer = dst_ptr;
        args.src.info.count = count;
        args.src.info.datatype = datatype.as_raw();
        args.dst.info.count = count;
        args.dst.info.datatype = datatype.as_raw();
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.op = op.as_raw();
        args.tag = 0 as ucc_coll_id_t;
        args.flags = 0;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;

        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status =
            unsafe { ucc_collective_init_and_post(&mut args, &mut coll_req, self.inner.handle) };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollectiveRequest { request: coll_req })
    }
}

impl Drop for UccTeam {
    fn drop(&mut self) {
        // Rc ensures ucc_team_destroy is called exactly once — when the last
        // UccTeam clone is dropped.
        let handle = std::rc::Rc::get_mut(&mut self.inner)
            .map(|inner| std::mem::replace(&mut inner.handle, std::ptr::null_mut()))
            .filter(|h| !h.is_null());
        if let Some(h) = handle {
            unsafe {
                // Safety: handle was validated non-null and is a valid team handle.
                ucc_team_destroy(h);
            }
        }
    }
}

/// Parameters for UCC team creation.
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
        // UCC requires either EP+EP_RANGE or OOB to be set for team creation.
        // Default to EP=0 + CONTIG range (single-process mode).
        params.ep = 0;
        params.ep_range = ucc_ep_range_type_t_UCC_COLLECTIVE_EP_RANGE_CONTIG;
        params.team_size = 1; // Default single-process team
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_ORDERING as u64;
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_SYNC_TYPE as u64;
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_FLAGS as u64;
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE as u64;
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP as u64;
        params.mask |= ucc_team_params_field_UCC_TEAM_PARAM_FIELD_EP_RANGE as u64;
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

    // ── Integration tests (call into libucc.so) ────────────────────────────

    mod integration_tests {
        use super::*;
        use crate::context::UccContext;
        use crate::lib_init::UccLib;

        /// Verify team params can be configured without creating a team.
        /// This tests the params builder pattern.
        #[test]
        fn integration_team_params_configuration() {
            let mut params = UccTeamParams::default();
            params.with_team_size(4);
            params.with_flags(0x10);
            assert_eq!(params.0.team_size, 4);
            assert_eq!(params.0.flags, 0x10);
            // Verify mask includes the fields we set
            assert!(
                params.0.mask & ucc_team_params_field_UCC_TEAM_PARAM_FIELD_TEAM_SIZE as u64 != 0
            );
            assert!(params.0.mask & ucc_team_params_field_UCC_TEAM_PARAM_FIELD_FLAGS as u64 != 0);
        }

        /// Integration test: create a team using the correct ucc_team_create_post API.
        /// This verifies the segfault fix — previously ucc_team_create_from_parent
        /// was called but doesn't exist in the UCC library.
        #[test]
        fn integration_team_create_and_destroy() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context");
            let team = UccTeam::new(ctx).expect("team create");
            // Team should be valid
            assert!(!team.handle().is_null(), "Team handle must be non-null");
            let size = team.size().expect("team size");
            assert_eq!(size, 1, "single-process team should have size 1");
            // Drop is automatic via RAII
        }

        /// Integration test: create team with custom params
        #[test]
        fn integration_team_create_with_params() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let mut params = UccTeamParams::default();
            params.with_team_size(1);
            let team = UccTeam::with_params(ctx, params).expect("team create with params");
            assert!(!team.handle().is_null());
        }

        /// Integration test: verify team size query works
        #[test]
        fn integration_team_size() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx).expect("team create");
            let size = team.size().expect("team size query");
            assert!(size > 0, "Team size should be > 0, got {}", size);
        }

        /// Integration test: barrier collective posts and completes
        ///
        /// #[ignore] — UCC `ucc_collective_init_and_post` returns
        /// `UCC_ERR_NOT_IMPLEMENTED` on a single-process team without
        /// multi-process OOB configuration. Requires a DVM (prrte) for
        /// multi-rank execution.
        #[test]
        #[ignore]
        fn integration_barrier() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            let req = team.barrier().expect("barrier post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Barrier should complete");
        }

        /// Integration test: allgather collective posts and completes
        ///
        /// #[ignore] — UCC `ucc_collective_init_and_post` returns
        /// `UCC_ERR_NOT_IMPLEMENTED` on a single-process team without
        /// multi-process OOB configuration. Requires a DVM (prrte).
        #[test]
        #[ignore]
        fn integration_allgather() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            let sendbuf = [42u8; 64];
            let mut recvbuf = vec![0u8; 64];
            let req = team
                .allgather(&sendbuf, &mut recvbuf, DataType::Uchar)
                .expect("allgather post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Allgather should complete");
        }

        /// Integration test: bcast collective posts and completes
        ///
        /// #[ignore] — UCC `ucc_collective_init_and_post` returns
        /// `UCC_ERR_NOT_IMPLEMENTED` on a single-process team without
        /// multi-process OOB configuration. Requires a DVM (prrte).
        #[test]
        #[ignore]
        fn integration_bcast() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            let mut buf = vec![42u8; 128];
            let req = team
                .bcast(&mut buf, DataType::Uchar, 0)
                .expect("bcast post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Bcast should complete");
        }

        /// Integration test: reduce collective posts and completes
        ///
        /// #[ignore] — UCC `ucc_collective_init_and_post` returns
        /// `UCC_ERR_NOT_IMPLEMENTED` on a single-process team without
        /// multi-process OOB configuration. Requires a DVM (prrte).
        #[test]
        #[ignore]
        fn integration_reduce() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            let sendbuf = vec![1u8; 64];
            let mut recvbuf = vec![0u8; 64];
            let req = team
                .reduce(&sendbuf, &mut recvbuf, DataType::Uchar, ReductionOp::Sum, 0)
                .expect("reduce post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Reduce should complete");
        }

        /// Integration test: allreduce convenience method posts and completes
        ///
        /// #[ignore] — UCC `ucc_collective_init_and_post` returns
        /// `UCC_ERR_NOT_IMPLEMENTED` on a single-process team without
        /// multi-process OOB configuration. Requires a DVM (prrte).
        #[test]
        #[ignore]
        fn integration_allreduce() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            let mut buf = vec![1u8; 128];
            let req = team
                .allreduce(&mut buf, DataType::Uchar, ReductionOp::Sum)
                .expect("allreduce post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Allreduce should complete");
        }

        /// Integration test: team EP query returns valid endpoint
        #[test]
        fn integration_team_ep() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx).expect("team create");
            let ep = team.ep().expect("team ep query");
            // Default EP is 0 for single-process team
            assert_eq!(ep, 0, "Single-process team EP should be 0");
        }

        /// Integration test: team raw attr query works with SIZE mask
        #[test]
        fn integration_team_attr_raw() {
            use crate::bindings::ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_SIZE;
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx).expect("team create");
            let attr = team
                .attr(ucc_team_attr_field_UCC_TEAM_ATTR_FIELD_SIZE as u64)
                .expect("team attr query");
            assert_eq!(attr.size, 1, "Raw attr size should be 1");
        }

        /// Integration test: team clone shares the same handle (RAII correctness)
        #[test]
        fn integration_team_clone_shares_handle() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx).expect("team create");
            let original_handle = team.handle();
            let clone = team.clone();
            // Both clones must share the same underlying C handle
            assert_eq!(
                clone.handle(),
                original_handle,
                "Cloned team must share the same C handle"
            );
            // Dropping the clone must NOT invalidate the original
            drop(clone);
            assert!(
                !team.handle().is_null(),
                "Original team handle must remain valid after clone drop"
            );
            assert_eq!(team.size().expect("size"), 1);
        }

        /// Integration test: multiple team clones — only last drop destroys
        #[test]
        fn integration_team_multiple_clones() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx).expect("team create");
            let handle = team.handle();
            let _c1 = team.clone();
            let _c2 = team.clone();
            let _c3 = team.clone();
            // All 4 clones (original + 3) share one handle
            assert_eq!(_c1.handle(), handle);
            assert_eq!(_c2.handle(), handle);
            assert_eq!(_c3.handle(), handle);
            // Drop clones one at a time — handle stays alive
            drop(_c3);
            assert!(!team.handle().is_null());
            drop(_c2);
            assert!(!team.handle().is_null());
            drop(_c1);
            assert!(
                !team.handle().is_null(),
                "Handle must survive until last clone drops"
            );
            // team is last — destroy happens on its drop
        }

        /// Integration test: allreduce with Int32 datatype (128 bytes = 32 x i32)
        /// Verifies the convenience method handles non-byte datatypes correctly.
        ///
        /// #[ignore] — Requires DVM for completion.
        #[test]
        #[ignore]
        fn integration_allreduce_int32() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            // 32 i32 elements = 128 bytes; allreduce takes &mut [u8] and the
            // datatype tells UCC how to interpret the buffer.
            let mut buf = vec![1u8; 128];
            let req = team
                .allreduce(&mut buf, DataType::Int32, ReductionOp::Sum)
                .expect("allreduce Int32 post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Allreduce Int32 should complete");
        }

        /// Integration test: bcast with non-zero root
        ///
        /// #[ignore] — Requires DVM for completion.
        #[test]
        #[ignore]
        fn integration_bcast_root1() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            let mut buf = vec![42u8; 128];
            // Root=1 on single-process team — tests parameter passing
            let req = team
                .bcast(&mut buf, DataType::Uchar, 1)
                .expect("bcast root=1 post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Bcast root=1 should complete");
        }

        /// Integration test: reduce with different ops (Max, Min, Prod)
        ///
        /// #[ignore] — Requires DVM for completion.
        #[test]
        #[ignore]
        fn integration_reduce_max() {
            let lib = UccLib::init().expect("ucc_init");
            let ctx = UccContext::new(lib).expect("context create");
            let team = UccTeam::new(ctx.clone()).expect("team create");
            let sendbuf = vec![7u8; 64];
            let mut recvbuf = vec![0u8; 64];
            let req = team
                .reduce(&sendbuf, &mut recvbuf, DataType::Uchar, ReductionOp::Max, 0)
                .expect("reduce Max post");
            let mut iterations = 0;
            while !req.test().unwrap_or(false) {
                ctx.progress();
                iterations += 1;
                if iterations > 10000 {
                    break;
                }
            }
            assert!(iterations < 10000, "Reduce Max should complete");
        }
    }
}
