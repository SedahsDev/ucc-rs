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
    ucc_reduction_op_t_UCC_OP_SUM, ucc_status_t_UCC_ERR_NO_MESSAGE, ucc_status_t_UCC_OK,
};
use crate::status::{check_status, UccError, UccStatus};
use crate::team::UccTeam;

/// Collective type enum for safe API usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum UccCollectiveType {
    Allreduce = ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE,
    Reduce = ucc_coll_type_t_UCC_COLL_TYPE_REDUCE,
    Broadcast = ucc_coll_type_t_UCC_COLL_TYPE_BCAST,
    Allgather = ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHER,
    ReduceScatter = ucc_coll_type_t_UCC_COLL_TYPE_REDUCE_SCATTER,
    Gather = ucc_coll_type_t_UCC_COLL_TYPE_GATHER,
    GatherV = ucc_coll_type_t_UCC_COLL_TYPE_GATHERV,
    Scatter = ucc_coll_type_t_UCC_COLL_TYPE_SCATTER,
    ScatterV = ucc_coll_type_t_UCC_COLL_TYPE_SCATTERV,
    Alltoall = ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALL,
    AlltoallV = ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALLV,
    Barrier = ucc_coll_type_t_UCC_COLL_TYPE_BARRIER,
    FanIn = ucc_coll_type_t_UCC_COLL_TYPE_FANIN,
    FanOut = ucc_coll_type_t_UCC_COLL_TYPE_FANOUT,
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
    Sum = ucc_reduction_op_t_UCC_OP_SUM,
    Prod = ucc_reduction_op_t_UCC_OP_PROD,
    Min = ucc_reduction_op_t_UCC_OP_MIN,
    Max = ucc_reduction_op_t_UCC_OP_MAX,
    Land = ucc_reduction_op_t_UCC_OP_LAND,
    Lor = ucc_reduction_op_t_UCC_OP_LOR,
    Lxor = ucc_reduction_op_t_UCC_OP_LXOR,
    Band = ucc_reduction_op_t_UCC_OP_BAND,
    Bor = ucc_reduction_op_t_UCC_OP_BOR,
    Bxor = ucc_reduction_op_t_UCC_OP_BXOR,
}

impl UccReductionOp {
    /// Convert to the raw FFI type.
    pub fn as_raw(self) -> ucc_reduction_op_t {
        self as ucc_reduction_op_t
    }
}

/// Alias for backward compatibility with osu-rs import paths.
pub type ReductionOp = UccReductionOp;

/// UCC datatype enum for safe API usage.
///
/// Maps directly to the `UCC_DT_*` predefined datatypes from the UCC C API.
/// These are simple integer values (0–17) used in collective operations
/// to specify element types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
#[allow(non_upper_case_globals)]
pub enum DataType {
    /// 8-bit signed integer (int8_t)
    Int8 = 0,
    /// 16-bit signed integer (int16_t)
    Int16 = 1,
    /// 32-bit signed integer (int32_t)
    Int32 = 2,
    /// 64-bit signed integer (int64_t)
    Int64 = 3,
    /// 128-bit signed integer
    Int128 = 4,
    /// 8-bit unsigned integer (uint8_t / unsigned char)
    Uint8 = 5,
    /// 16-bit unsigned integer (uint16_t)
    Uint16 = 6,
    /// 32-bit unsigned integer (uint32_t)
    Uint32 = 7,
    /// 64-bit unsigned integer (uint64_t)
    Uint64 = 8,
    /// 128-bit unsigned integer
    Uint128 = 9,
    /// 16-bit float (half precision)
    Float16 = 10,
    /// 32-bit float (single precision)
    Float32 = 11,
    /// 64-bit float (double precision)
    Float64 = 12,
    /// 16-bit brain float (bfloat16)
    BFloat16 = 13,
    /// 128-bit float (quad precision)
    Float128 = 14,
    /// 32-bit complex float (two single-precision floats)
    Float32Complex = 15,
    /// 64-bit complex float (two double-precision floats)
    Float64Complex = 16,
    /// 128-bit complex float (two quad-precision floats)
    Float128Complex = 17,
}

impl DataType {
    /// Alias for unsigned char — commonly used in byte-level benchmarks.
    #[allow(non_upper_case_globals)]
    pub const Uchar: Self = Self::Uint8;

    /// Convert to the raw FFI type (ucc_datatype_t = u64).
    pub fn as_raw(self) -> ucc_datatype_t {
        self as ucc_datatype_t
    }

    /// Size in bytes for this datatype.
    pub fn size_in_bytes(self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 => 1,
            Self::Int16 | Self::Uint16 | Self::Float16 | Self::BFloat16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Int64 | Self::Uint64 | Self::Float64 => 8,
            Self::Int128 | Self::Uint128 | Self::Float128 => 16,
            Self::Float32Complex => 8,
            Self::Float64Complex => 16,
            Self::Float128Complex => 32,
        }
    }
}

/// Non-blocking collective request handle.
///
/// Returned by convenience methods like [`UccTeam::allreduce`].
/// Use [`UccCollectiveRequest::test`] to poll for completion.
///
/// Dropping an incomplete request intentionally leaks its UCC request rather
/// than finalizing it while it is in flight, which would be undefined behavior.
pub struct UccCollectiveRequest {
    pub(crate) request: ucc_coll_req_h,
    pub(crate) completed: std::cell::Cell<bool>,
}

impl UccCollectiveRequest {
    /// Test whether the collective operation has completed.
    ///
    /// Returns `Ok(true)` if the operation is complete, `Ok(false)` if
    /// still in progress, or `Err` if the operation failed.
    ///
    /// This is a Rust equivalent of the C `ucc_collective_test()` inline
    /// function — reads `request->status` directly.
    #[allow(non_upper_case_globals)]
    pub fn test(&self) -> Result<bool, UccStatus> {
        let status = unsafe {
            // Safety: self.request is a valid handle from ucc_collective_init_and_post.
            // ucc_collective_test is just `return request->status;`
            (*self.request).status
        };
        match status {
            ucc_status_t_UCC_OK => {
                self.completed.set(true);
                Ok(true)
            }
            ucc_status_t_UCC_ERR_NO_MESSAGE => Ok(false), // still in progress
            _ => Err(check_status(status)
                .err()
                .unwrap_or(UccStatus::Known(UccError::ErrNoResource))),
        }
    }
}

impl Drop for UccCollectiveRequest {
    fn drop(&mut self) {
        if !self.request.is_null() && self.completed.get() {
            // Safety: request is a completed, valid handle.
            let _ = unsafe { ucc_collective_finalize(self.request) };
            self.request = std::ptr::null_mut();
        } else if !self.request.is_null() {
            eprintln!(
                "dropping an incomplete UCC collective request; leaking it to avoid undefined behavior"
            );
            debug_assert!(
                self.completed.get(),
                "incomplete UCC collective request was dropped"
            );
        }
    }
}

/// Builder for UCC collective operations.
///
/// Provides a type-safe, ergonomic API for constructing collective
/// operations. Use the builder pattern to set all required fields,
/// then call `init()` or `init_and_post()` to execute.
///
/// Buffer pointers come from borrowed slices. Lifetime `'a` ties the
/// builder to those buffers for construction; callers must also keep the
/// buffers alive until the collective request is finalized (UCC does not
/// copy payload data).
///
/// # Example
///
/// ```no_run
/// # use ucc::{lib_init::UccLib, context::UccContext, team::UccTeam, collective::{CollectiveBuilder, UccCollectiveType, UccReductionOp, DataType}};
/// # let lib = UccLib::init().unwrap();
/// # let ctx = UccContext::new(lib.clone()).unwrap();
/// # let team = UccTeam::new(ctx.clone()).unwrap();
/// let mut buf = vec![1u8; 256];
/// let coll = CollectiveBuilder::new(UccCollectiveType::Allreduce)
///     .with_inplace(&mut buf)
///     .with_count(256)
///     .with_dtype(DataType::Uchar.as_raw() as u32)
///     .with_reduction_op(UccReductionOp::Sum)
///     .init(&team)
///     .unwrap();
/// ```
pub struct CollectiveBuilder<'a> {
    coll_type: UccCollectiveType,
    /// Host buffer pointer for `src.info.buffer` (live until coll completes).
    src: Option<*mut std::os::raw::c_void>,
    /// Host buffer pointer for `dst.info.buffer` (live until coll completes).
    dst: Option<*mut std::os::raw::c_void>,
    count: Option<u64>,
    datatype: Option<u32>,
    reduction_op: Option<UccReductionOp>,
    root: Option<u64>,
    tag: Option<u16>,
    flags: u64,
    _bufs: std::marker::PhantomData<&'a mut ()>,
}

impl<'a> CollectiveBuilder<'a> {
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
            _bufs: std::marker::PhantomData,
        }
    }

    /// Set the source buffer from an immutable byte slice.
    ///
    /// The memory must remain valid until the collective request is finalized.
    pub fn with_src(mut self, src: &'a [u8]) -> Self {
        self.src = Some(src.as_ptr() as *mut std::os::raw::c_void);
        self
    }

    /// Set the destination buffer from a mutable byte slice.
    ///
    /// The memory must remain valid until the collective request is finalized.
    pub fn with_dst(mut self, dst: &'a mut [u8]) -> Self {
        self.dst = Some(dst.as_mut_ptr() as *mut std::os::raw::c_void);
        self
    }

    /// Set both source and destination to the same in-place buffer.
    pub fn with_inplace(mut self, buf: &'a mut [u8]) -> Self {
        let ptr = buf.as_mut_ptr() as *mut std::os::raw::c_void;
        self.src = Some(ptr);
        self.dst = Some(ptr);
        self
    }

    /// Set a raw source pointer (advanced).
    ///
    /// # Safety
    /// `src` must point to a valid host buffer that remains live until the
    /// collective request is finalized.
    pub unsafe fn with_src_ptr(mut self, src: *mut std::os::raw::c_void) -> Self {
        self.src = Some(src);
        self
    }

    /// Set a raw destination pointer (advanced).
    ///
    /// # Safety
    /// `dst` must point to a valid host buffer that remains live until the
    /// collective request is finalized.
    pub unsafe fn with_dst_ptr(mut self, dst: *mut std::os::raw::c_void) -> Self {
        self.dst = Some(dst);
        self
    }

    /// Set the element count.
    pub fn with_count(mut self, count: u64) -> Self {
        self.count = Some(count);
        self
    }

    /// Set the data type (as raw u32 / UCC datatype ordinal).
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
            // SAFETY: args is fully initialized POD; team handle is valid.
            ucc_collective_init(&mut args, &mut coll_req, team.handle())
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollective {
            request: coll_req,
            coll_type: self.coll_type,
            completed: std::cell::Cell::new(false),
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
            // SAFETY: args is fully initialized POD; team handle is valid.
            ucc_collective_init_and_post(&mut args, &mut coll_req, team.handle())
        };
        check_status(status)?;
        if coll_req.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(UccCollective {
            request: coll_req,
            coll_type: self.coll_type,
            completed: std::cell::Cell::new(false),
            _team: team.clone(),
        })
    }

    /// Build the internal `ucc_coll_args` struct from builder state.
    fn build_args(&self) -> Result<ucc_coll_args, UccStatus> {
        let src = self
            .src
            .ok_or(UccStatus::Known(UccError::ErrInvalidParam))?;
        let dst = self
            .dst
            .ok_or(UccStatus::Known(UccError::ErrInvalidParam))?;
        let count = self.count.unwrap_or(1);
        let datatype = self.datatype.unwrap_or(DataType::Uint8.as_raw() as u32);

        // SAFETY: ucc_coll_args is a POD struct; we initialize all fields explicitly.
        let mut args: ucc_coll_args = unsafe { std::mem::zeroed() };
        args.coll_type = self.coll_type.as_raw();
        args.src.info.buffer = src;
        args.dst.info.buffer = dst;
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
/// Holds the collective request handle until the operation completes.
/// Use [`Self::test`] to record completion. Dropping an incomplete collective
/// leaks its UCC request rather than finalizing it while in flight (UB).
#[must_use = "Collective operations should be posted and waited on"]
pub struct UccCollective {
    request: ucc_coll_req_h,
    coll_type: UccCollectiveType,
    completed: std::cell::Cell<bool>,
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

    /// Test whether this collective has completed and track completion for drop.
    #[allow(non_upper_case_globals)]
    pub fn test(&self) -> Result<bool, UccStatus> {
        let status = unsafe { (*self.request).status };
        match status {
            ucc_status_t_UCC_OK => {
                self.completed.set(true);
                Ok(true)
            }
            ucc_status_t_UCC_ERR_NO_MESSAGE => Ok(false),
            _ => Err(check_status(status)
                .err()
                .unwrap_or(UccStatus::Known(UccError::ErrNoResource))),
        }
    }

    /// Get the collective type.
    pub fn coll_type(&self) -> UccCollectiveType {
        self.coll_type
    }
}

impl Drop for UccCollective {
    fn drop(&mut self) {
        if !self.request.is_null() && self.completed.get() {
            // Safety: request is a completed, valid handle.
            let _ = unsafe { ucc_collective_finalize(self.request) };
            self.request = std::ptr::null_mut();
        } else if !self.request.is_null() {
            eprintln!(
                "dropping an incomplete UCC collective; leaking it to avoid undefined behavior"
            );
            debug_assert!(
                self.completed.get(),
                "incomplete UCC collective was dropped"
            );
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
