//! UCC collective operation builder and execution.
//!
//! Provides a type-safe builder for initializing and posting collective
//! operations (allreduce, broadcast, allgather, reduce, barrier, etc.).

use crate::bindings::{
    ucc_coll_args, ucc_coll_callback, ucc_coll_id_t, ucc_coll_req_h, ucc_coll_type_t,
    ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHER, ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE,
    ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALL, ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALLV,
    ucc_coll_type_t_UCC_COLL_TYPE_BARRIER, ucc_coll_type_t_UCC_COLL_TYPE_BCAST,
    ucc_coll_type_t_UCC_COLL_TYPE_FANIN, ucc_coll_type_t_UCC_COLL_TYPE_FANOUT,
    ucc_coll_type_t_UCC_COLL_TYPE_GATHER, ucc_coll_type_t_UCC_COLL_TYPE_GATHERV,
    ucc_coll_type_t_UCC_COLL_TYPE_REDUCE, ucc_coll_type_t_UCC_COLL_TYPE_REDUCE_SCATTER,
    ucc_coll_type_t_UCC_COLL_TYPE_SCATTER, ucc_coll_type_t_UCC_COLL_TYPE_SCATTERV,
    ucc_collective_finalize, ucc_collective_init, ucc_collective_init_and_post,
    ucc_collective_post, ucc_collective_triggered_post, ucc_datatype_t,
    ucc_error_type_t_UCC_ERR_TYPE_LOCAL, ucc_memory_type_UCC_MEMORY_TYPE_HOST, ucc_reduction_op_t,
    ucc_reduction_op_t_UCC_OP_BAND, ucc_reduction_op_t_UCC_OP_BOR, ucc_reduction_op_t_UCC_OP_BXOR,
    ucc_reduction_op_t_UCC_OP_LAND, ucc_reduction_op_t_UCC_OP_LOR, ucc_reduction_op_t_UCC_OP_LXOR,
    ucc_reduction_op_t_UCC_OP_MAX, ucc_reduction_op_t_UCC_OP_MIN, ucc_reduction_op_t_UCC_OP_PROD,
    ucc_reduction_op_t_UCC_OP_SUM,
};
use crate::memory::UccMemHandle;
use crate::status::{check_status, UccError, UccStatus};
use crate::team::UccTeam;

/// Collective type enum for safe API usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum UccCollectiveType {
    Allreduce = ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE as u32,
    Reduce = ucc_coll_type_t_UCC_COLL_TYPE_REDUCE as u32,
    Broadcast = ucc_coll_type_t_UCC_COLL_TYPE_BCAST as u32,
    Allgather = ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHER as u32,
    ReduceScatter = ucc_coll_type_t_UCC_COLL_TYPE_REDUCE_SCATTER as u32,
    Gather = ucc_coll_type_t_UCC_COLL_TYPE_GATHER as u32,
    GatherV = ucc_coll_type_t_UCC_COLL_TYPE_GATHERV as u32,
    Scatter = ucc_coll_type_t_UCC_COLL_TYPE_SCATTER as u32,
    ScatterV = ucc_coll_type_t_UCC_COLL_TYPE_SCATTERV as u32,
    Alltoall = ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALL as u32,
    AlltoallV = ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALLV as u32,
    Barrier = ucc_coll_type_t_UCC_COLL_TYPE_BARRIER as u32,
    FanIn = ucc_coll_type_t_UCC_COLL_TYPE_FANIN as u32,
    FanOut = ucc_coll_type_t_UCC_COLL_TYPE_FANOUT as u32,
}

impl UccCollectiveType {
    /// Convert to the raw FFI type.
    pub fn as_raw(self) -> ucc_coll_type_t {
        self as ucc_coll_type_t
    }
}

/// Reduction operation enum for safe API usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum UccReductionOp {
    Sum = ucc_reduction_op_t_UCC_OP_SUM as u32,
    Prod = ucc_reduction_op_t_UCC_OP_PROD as u32,
    Min = ucc_reduction_op_t_UCC_OP_MIN as u32,
    Max = ucc_reduction_op_t_UCC_OP_MAX as u32,
    Land = ucc_reduction_op_t_UCC_OP_LAND as u32,
    Lor = ucc_reduction_op_t_UCC_OP_LOR as u32,
    Lxor = ucc_reduction_op_t_UCC_OP_LXOR as u32,
    Band = ucc_reduction_op_t_UCC_OP_BAND as u32,
    Bor = ucc_reduction_op_t_UCC_OP_BOR as u32,
    Bxor = ucc_reduction_op_t_UCC_OP_BXOR as u32,
}

impl UccReductionOp {
    /// Convert to the raw FFI type.
    pub fn as_raw(self) -> ucc_reduction_op_t {
        self as ucc_reduction_op_t
    }
}

/// Builder for UCC collective operations.
///
/// Provides a type-safe, ergonomic API for constructing collective
/// operations. Use the builder pattern to set all required fields,
/// then call `init()` or `init_and_post()` to execute.
///
/// # Example
///
/// ```no_run
/// # use ucc::{lib_init::UccLib, context::UccContext, team::UccTeam, memory::UccMemHandle, collective::{CollectiveBuilder, UccCollectiveType, UccReductionOp}};
/// # let lib = UccLib::init().unwrap();
/// # let ctx = UccContext::new(lib.clone()).unwrap();
/// # let team = UccTeam::new(ctx.clone()).unwrap();
/// # let buf = UccMemHandle::map_slice(&ctx, &[0u8; 1024]).unwrap();
/// let coll = CollectiveBuilder::new(UccCollectiveType::Allreduce)
///     .with_src(&buf)
///     .with_dst(&buf)
///     .with_count(256)
///     .with_reduction_op(UccReductionOp::Sum)
///     .init(&team)
///     .unwrap();
/// ```
pub struct CollectiveBuilder {
    coll_type: UccCollectiveType,
    src: Option<usize>, // UccMemHandle address as usize — avoids Clone requirement
    dst: Option<usize>,
    count: Option<u64>,
    datatype: Option<u32>,
    reduction_op: Option<UccReductionOp>,
    root: Option<u64>,
    tag: Option<u16>,
    flags: u64,
}

impl CollectiveBuilder {
    /// Create a new collective builder with the specified collective type.
    pub fn new(coll_type: UccCollectiveType) -> Self {
        Self {
            coll_type,
            src: None,
            dst: None,
            count: None,
            datatype: None,
            reduction_op: None,
            root: None,
            tag: None,
            flags: 0,
        }
    }

    /// Set the source buffer.
    pub fn with_src(mut self, src: &UccMemHandle) -> Self {
        self.src = Some(src.handle() as usize);
        self
    }

    /// Set the destination buffer.
    pub fn with_dst(mut self, dst: &UccMemHandle) -> Self {
        self.dst = Some(dst.handle() as usize);
        self
    }

    /// Set the element count.
    pub fn with_count(mut self, count: u64) -> Self {
        self.count = Some(count);
        self
    }

    /// Set the data type (as raw u32).
    pub fn with_dtype(mut self, dt: u32) -> Self {
        self.datatype = Some(dt);
        self
    }

    /// Set the reduction operation.
    pub fn with_reduction_op(mut self, op: UccReductionOp) -> Self {
        self.reduction_op = Some(op);
        self
    }

    /// Set the root rank for broadcast/reduce/gather operations.
    pub fn with_root(mut self, root: u64) -> Self {
        self.root = Some(root);
        self
    }

    /// Set the collective tag for ordering.
    pub fn with_tag(mut self, tag: u16) -> Self {
        self.tag = Some(tag);
        self
    }

    /// Set collective flags.
    pub fn with_flags(mut self, flags: u64) -> Self {
        self.flags = flags;
        self
    }

    /// Initialize the collective operation (without posting).
    ///
    /// Returns a [`UccCollective`] that can be posted later via `post()`.
    #[must_use = "Result should be checked"]
    pub fn init(self, team: &UccTeam) -> Result<UccCollective, UccStatus> {
        let mut args = self.build_args()?;
        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: args is a valid mutable reference, coll_req is a valid output pointer,
            // team.handle() is a valid team handle.
            // ucc_collective_init takes *mut ucc_coll_args (mutable ref)
            ucc_collective_init(&mut args, &mut coll_req, team.handle())
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollective {
            request: coll_req,
            coll_type: self.coll_type,
            _team: team.clone(),
        })
    }

    /// Initialize and post the collective in a single call.
    ///
    /// Convenience method that calls `ucc_collective_init_and_post` —
    /// combines initialization and posting into one FFI call.
    #[must_use = "Result should be checked"]
    pub fn init_and_post(self, team: &UccTeam) -> Result<UccCollective, UccStatus> {
        let mut args = self.build_args()?;
        let mut coll_req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            // Safety: args is a valid mutable reference, coll_req is a valid output pointer,
            // team.handle() is a valid team handle.
            ucc_collective_init_and_post(&mut args, &mut coll_req, team.handle())
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollective {
            request: coll_req,
            coll_type: self.coll_type,
            _team: team.clone(),
        })
    }

    /// Build the internal `ucc_coll_args` struct from builder state.
    fn build_args(&self) -> Result<ucc_coll_args, UccStatus> {
        let src = self.src.expect("Source buffer must be set");
        let dst = self.dst.expect("Destination buffer must be set");
        let count = self.count.unwrap_or(1);
        let datatype = self.datatype.unwrap_or(4); // default to u32

        // Safety: ucc_coll_args is a POD struct; we initialize all fields explicitly.
        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = self.coll_type.as_raw();
        // Access the union's `info` field (ucc_coll_buffer_info)
        args.src.info.buffer = src as *mut std::os::raw::c_void;
        args.dst.info.buffer = dst as *mut std::os::raw::c_void;
        args.src.info.count = count;
        args.src.info.datatype = datatype as ucc_datatype_t;
        args.dst.info.count = count;
        args.dst.info.datatype = datatype as ucc_datatype_t;
        args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        args.op = self
            .reduction_op
            .map(|op| op.as_raw())
            .unwrap_or(ucc_reduction_op_t_UCC_OP_SUM);
        args.root = self.root.unwrap_or(0);
        args.tag = self.tag.unwrap_or(0) as ucc_coll_id_t;
        args.flags = self.flags;
        args.error_type = ucc_error_type_t_UCC_ERR_TYPE_LOCAL;
        args.cb = ucc_coll_callback {
            cb: None,
            data: std::ptr::null_mut(),
        };
        args.timeout = 0.0;
        Ok(args)
    }
}

/// An initialized collective operation ready to be posted.
///
/// Holds the collective request handle until the operation
/// completes. Drop finalizes the collective request.
#[must_use = "Collective operations should be posted and waited on"]
pub struct UccCollective {
    request: ucc_coll_req_h,
    coll_type: UccCollectiveType,
    _team: UccTeam,
}

impl UccCollective {
    /// Post the collective operation for asynchronous execution.
    ///
    /// Returns immediately — the collective runs asynchronously.
    /// Use `request()` to get the raw handle for completion testing.
    pub fn post(&self) -> Result<(), UccStatus> {
        let status = unsafe {
            // Safety: self.request is a valid collective request handle from ucc_collective_init.
            ucc_collective_post(self.request)
        };
        check_status(status)
    }

    /// Triggered post — post the collective with an execution engine event.
    ///
    /// Posts the collective operation to be triggered by the specified
    /// event from the execution engine. The collective will execute
    /// when the event is signaled.
    ///
    /// # Safety
    ///
    /// The event pointer must be valid and obtained from the execution engine
    /// (e.g., via `UccExecutionEngine::wait_event()`).
    pub unsafe fn triggered_post(
        &self,
        ee_handle: crate::bindings::ucc_ee_h,
        event: *mut crate::bindings::ucc_ev_t,
    ) -> Result<(), UccStatus> {
        let status = ucc_collective_triggered_post(ee_handle, event);
        check_status(status)
    }

    /// Finalize the collective request, releasing resources.
    pub fn finalize(&mut self) -> Result<(), UccStatus> {
        if self.request.is_null() {
            return Ok(());
        }
        let status = unsafe {
            // Safety: self.request is a valid collective request handle.
            ucc_collective_finalize(self.request)
        };
        self.request = std::ptr::null_mut();
        check_status(status)
    }

    /// Get the raw request handle for testing completion.
    pub fn request(&self) -> ucc_coll_req_h {
        self.request
    }

    /// Get the collective type.
    pub fn coll_type(&self) -> UccCollectiveType {
        self.coll_type
    }
}

impl Drop for UccCollective {
    fn drop(&mut self) {
        if !self.request.is_null() {
            // Safety: request is a valid handle from ucc_collective_init.
            // We ignore the status here — cleanup on drop should not panic.
            let _ = unsafe { ucc_collective_finalize(self.request) };
            self.request = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collective_type_as_raw() {
        assert_eq!(
            UccCollectiveType::Allreduce.as_raw(),
            ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE
        );
        assert_eq!(
            UccCollectiveType::Broadcast.as_raw(),
            ucc_coll_type_t_UCC_COLL_TYPE_BCAST
        );
    }

    #[test]
    fn test_reduction_op_as_raw() {
        assert_eq!(UccReductionOp::Sum.as_raw(), ucc_reduction_op_t_UCC_OP_SUM);
        assert_eq!(UccReductionOp::Max.as_raw(), ucc_reduction_op_t_UCC_OP_MAX);
    }

    #[test]
    fn test_collective_builder_creation() {
        let builder = CollectiveBuilder::new(UccCollectiveType::Allreduce);
        let _ = builder
            .with_count(100)
            .with_reduction_op(UccReductionOp::Sum)
            .with_root(0);
        // Builder created successfully — actual init requires live UCC resources
    }
}
