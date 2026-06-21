//! UCC collective operations -- non-blocking API.
//!
//! All collectives return a [`UccCollRequest`] that must be polled via
//! [`UccCollRequest::test`] (which drives context progress internally) or
//! waited on via [`UccCollRequest::wait`].

use crate::bindings::{
    ucc_coll_args_field_UCC_COLL_ARGS_FIELD_FLAGS,
    ucc_coll_args_flags_t,
    ucc_coll_args_flags_t_UCC_COLL_ARGS_FLAG_IN_PLACE,
    ucc_coll_args_t,
    ucc_coll_req_h,
    ucc_coll_type_t,
    ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHER,
    ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHERV,
    ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE,
    ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALL,
    ucc_coll_type_t_UCC_COLL_TYPE_BARRIER,
    ucc_coll_type_t_UCC_COLL_TYPE_BCAST,
    ucc_coll_type_t_UCC_COLL_TYPE_FANIN,
    ucc_coll_type_t_UCC_COLL_TYPE_FANOUT,
    ucc_coll_type_t_UCC_COLL_TYPE_GATHER,
    ucc_coll_type_t_UCC_COLL_TYPE_REDUCE,
    ucc_coll_type_t_UCC_COLL_TYPE_REDUCE_SCATTER,
    ucc_coll_type_t_UCC_COLL_TYPE_SCATTER,
    ucc_collective_finalize,
    ucc_collective_init,
    ucc_collective_post,
    ucc_count_t,
    ucc_datatype_t,
    ucc_memory_type_UCC_MEMORY_TYPE_HOST,
    ucc_reduction_op_t,
    ucc_reduction_op_t_UCC_OP_AVG,
    ucc_reduction_op_t_UCC_OP_BAND,
    ucc_reduction_op_t_UCC_OP_BOR,
    ucc_reduction_op_t_UCC_OP_BXOR,
    ucc_reduction_op_t_UCC_OP_LAND,
    ucc_reduction_op_t_UCC_OP_LOR,
    ucc_reduction_op_t_UCC_OP_LXOR,
    ucc_reduction_op_t_UCC_OP_MAX,
    ucc_reduction_op_t_UCC_OP_MAXLOC,
    ucc_reduction_op_t_UCC_OP_MIN,
    ucc_reduction_op_t_UCC_OP_MINLOC,
    ucc_reduction_op_t_UCC_OP_PROD,
    ucc_reduction_op_t_UCC_OP_SUM,
};
use crate::status::{check_status, UccStatus};
use crate::team::UccTeam;

/// Reduction operations for collective operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReductionOp {
    Sum = ucc_reduction_op_t_UCC_OP_SUM as isize,
    Prod = ucc_reduction_op_t_UCC_OP_PROD as isize,
    Max = ucc_reduction_op_t_UCC_OP_MAX as isize,
    Min = ucc_reduction_op_t_UCC_OP_MIN as isize,
    Land = ucc_reduction_op_t_UCC_OP_LAND as isize,
    Lor = ucc_reduction_op_t_UCC_OP_LOR as isize,
    Lxor = ucc_reduction_op_t_UCC_OP_LXOR as isize,
    Band = ucc_reduction_op_t_UCC_OP_BAND as isize,
    Bor = ucc_reduction_op_t_UCC_OP_BOR as isize,
    Bxor = ucc_reduction_op_t_UCC_OP_BXOR as isize,
    Maxloc = ucc_reduction_op_t_UCC_OP_MAXLOC as isize,
    Minloc = ucc_reduction_op_t_UCC_OP_MINLOC as isize,
    Avg = ucc_reduction_op_t_UCC_OP_AVG as isize,
}

impl ReductionOp {
    fn as_raw(self) -> ucc_reduction_op_t {
        self as ucc_reduction_op_t
    }
}

/// Built-in UCC datatypes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Uchar = 1,
    Char = 2,
    Short = 3,
    Int = 4,
    Long = 5,
    Llong = 6,
    Ushort = 7,
    Uint = 8,
    Ulong = 9,
    Ullong = 10,
    Float = 11,
    Double = 12,
    Longlong = 13,
    Ldouble = 14,
    Bool = 15,
    Int8_t = 16,
    Int16_t = 17,
    Int32_t = 18,
    Int64_t = 19,
    Uint8_t = 20,
    Uint16_t = 21,
    Uint32_t = 22,
    Uint64_t = 23,
}

impl DataType {
    fn as_raw(self) -> ucc_datatype_t {
        self as ucc_datatype_t
    }
}

/// UCC collective request handle with RAII cleanup.
pub struct UccCollRequest {
    handle: ucc_coll_req_h,
    _team: UccTeam,
}

impl UccCollRequest {
    /// Test if the collective operation has completed.
    /// Returns true if complete, false if still in progress.
    pub fn test(&mut self) -> Result<bool, UccStatus> {
        if self.handle.is_null() {
            return Ok(true);
        }
        let status = unsafe { ucc_collective_post(self.handle) };
        if status == 1 {
            // UCC_INPROGRESS
            Ok(false)
        } else if status == 0 {
            // UCC_OK
            self.handle = std::ptr::null_mut();
            Ok(true)
        } else {
            Err(UccStatus(status))
        }
    }

    /// Wait for the collective operation to complete.
    pub fn wait(&mut self) -> Result<(), UccStatus> {
        while !self.test()? {}
        Ok(())
    }

    /// Get a raw handle for use with lower-level APIs.
    pub fn handle(&self) -> ucc_coll_req_h {
        self.handle
    }
}

impl Drop for UccCollRequest {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ucc_collective_finalize(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Build and post a collective operation.
pub struct CollectiveBuilder<'a> {
    team: &'a UccTeam,
    coll_type: ucc_coll_type_t,
    src_buffer: *mut std::os::raw::c_void,
    dst_buffer: *mut std::os::raw::c_void,
    count: ucc_count_t,
    datatype: ucc_datatype_t,
    reduction_op: ucc_reduction_op_t,
    root: u64,
    flags: ucc_coll_args_flags_t,
}

impl<'a> CollectiveBuilder<'a> {
    fn new(team: &'a UccTeam) -> Self {
        Self {
            team,
            coll_type: 0,
            src_buffer: std::ptr::null_mut(),
            dst_buffer: std::ptr::null_mut(),
            count: 0,
            datatype: DataType::Int.as_raw(),
            reduction_op: ReductionOp::Sum.as_raw(),
            root: 0,
            flags: 0,
        }
    }

    /// Set the collective type.
    pub fn coll_type(mut self, coll_type: ucc_coll_type_t) -> Self {
        self.coll_type = coll_type;
        self
    }

    /// Set the source buffer.
    pub unsafe fn src_buffer(mut self, buffer: *mut std::os::raw::c_void) -> Self {
        self.src_buffer = buffer;
        self
    }

    /// Set the destination buffer.
    pub unsafe fn dst_buffer(mut self, buffer: *mut std::os::raw::c_void) -> Self {
        self.dst_buffer = buffer;
        self
    }

    /// Set the element count.
    pub fn count(mut self, count: ucc_count_t) -> Self {
        self.count = count;
        self
    }

    /// Set the datatype.
    pub fn datatype(mut self, dt: DataType) -> Self {
        self.datatype = dt.as_raw();
        self
    }

    /// Set the reduction operation.
    pub fn reduction_op(mut self, op: ReductionOp) -> Self {
        self.reduction_op = op.as_raw();
        self
    }

    /// Set the root rank.
    pub fn root(mut self, root: u64) -> Self {
        self.root = root;
        self
    }

    /// Set the in-place flag.
    pub fn inplace(mut self) -> Self {
        self.flags = ucc_coll_args_flags_t_UCC_COLL_ARGS_FLAG_IN_PLACE;
        self
    }

    /// Post the collective operation and return a request handle.
    pub fn post(self) -> Result<UccCollRequest, UccStatus> {
        let mut coll_args: ucc_coll_args_t = unsafe { std::mem::zeroed() };

        coll_args.coll_type = self.coll_type;
        // src/dst are unions with .info field for simple buffer info
        coll_args.src.info.buffer = self.src_buffer;
        coll_args.src.info.datatype = self.datatype;
        coll_args.src.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        coll_args.dst.info.buffer = self.dst_buffer;
        coll_args.dst.info.datatype = self.datatype;
        coll_args.dst.info.mem_type = ucc_memory_type_UCC_MEMORY_TYPE_HOST;
        coll_args.op = self.reduction_op;
        coll_args.root = self.root;
        coll_args.flags = self.flags as u64;
        coll_args.global_work_buffer = std::ptr::null_mut();
        coll_args.cb = unsafe { std::mem::zeroed() };
        coll_args.timeout = 0.0;
        coll_args.mask = ucc_coll_args_field_UCC_COLL_ARGS_FIELD_FLAGS as u64;

        let mut req: ucc_coll_req_h = std::ptr::null_mut();
        let status = unsafe {
            ucc_collective_init(&mut coll_args, &mut req, self.team.handle())
        };
        check_status(status)?;

        if req.is_null() {
            return Err(UccStatus(-5)); // UCC_ERR_NO_RESOURCE
        }

        let status = unsafe { ucc_collective_post(req) };
        check_status(status)?;

        Ok(UccCollRequest {
            handle: req,
            _team: self.team.clone(),
        })
    }
}

// Convenience methods on UccTeam

impl UccTeam {
    /// Start a barrier collective.
    pub fn barrier(&self) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_BARRIER)
            .post()
    }

    /// Broadcast a buffer to all team members (safe slice version).
    pub fn bcast(&self, buffer: &mut [u8], datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.bcast_raw(buffer.as_ptr() as *mut std::os::raw::c_void, buffer.len() as ucc_count_t, datatype, root) }
    }

    /// Broadcast a buffer to all team members (raw pointer version).
    pub unsafe fn bcast_raw(&self, buffer: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_BCAST)
            .src_buffer(buffer)
            .dst_buffer(buffer)
            .count(count)
            .datatype(datatype)
            .root(root)
            .post()
    }

    /// Allreduce a buffer (safe slice version).
    pub fn allreduce(&self, buffer: &mut [u8], datatype: DataType, op: ReductionOp) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.allreduce_raw(buffer.as_ptr() as *mut std::os::raw::c_void, buffer.len() as ucc_count_t, datatype, op) }
    }

    /// Allreduce a buffer (raw pointer version).
    pub unsafe fn allreduce_raw(&self, buffer: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, op: ReductionOp) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_ALLREDUCE)
            .src_buffer(buffer)
            .dst_buffer(buffer)
            .count(count)
            .datatype(datatype)
            .reduction_op(op)
            .post()
    }

    /// Reduce from src into dst (safe slice version).
    pub fn reduce(&self, src: &[u8], dst: &mut [u8], datatype: DataType, op: ReductionOp, root: u64) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.reduce_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype, op, root) }
    }

    /// Reduce from src into dst (raw pointer version).
    pub unsafe fn reduce_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, op: ReductionOp, root: u64) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_REDUCE)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .reduction_op(op)
            .root(root)
            .post()
    }

    /// Allgather from src into dst (safe slice version).
    pub fn allgather(&self, src: &[u8], dst: &mut [u8], datatype: DataType) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.allgather_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype) }
    }

    /// Allgather from src into dst (raw pointer version).
    pub unsafe fn allgather_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHER)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .post()
    }

    /// Gather from src into dst (safe slice version).
    pub fn gather(&self, src: &[u8], dst: &mut [u8], datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.gather_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype, root) }
    }

    /// Gather from src into dst (raw pointer version).
    pub unsafe fn gather_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_GATHER)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .root(root)
            .post()
    }

    /// Allgatherv collective (raw pointer version — no safe variant due to variable-count API).
    pub unsafe fn allgatherv_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, recvcounts: *const ucc_count_t, datatype: DataType) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_ALLGATHERV)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(recvcounts as ucc_count_t)
            .datatype(datatype)
            .post()
    }

    /// Scatter from src into dst (safe slice version).
    pub fn scatter(&self, src: &[u8], dst: &mut [u8], datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.scatter_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype, root) }
    }

    /// Scatter from src into dst (raw pointer version).
    pub unsafe fn scatter_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_SCATTER)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .root(root)
            .post()
    }

    /// Reduce-scatter from src into dst (safe slice version).
    pub fn reduce_scatter(&self, src: &[u8], dst: &mut [u8], datatype: DataType, op: ReductionOp) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.reduce_scatter_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype, op) }
    }

    /// Reduce-scatter from src into dst (raw pointer version).
    pub unsafe fn reduce_scatter_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, op: ReductionOp) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_REDUCE_SCATTER)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .reduction_op(op)
            .post()
    }

    /// Alltoall from src into dst (safe slice version).
    pub fn alltoall(&self, src: &[u8], dst: &mut [u8], datatype: DataType) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.alltoall_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype) }
    }

    /// Alltoall from src into dst (raw pointer version).
    pub unsafe fn alltoall_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_ALLTOALL)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .post()
    }

    /// Fanin from src into dst (safe slice version).
    pub fn fanin(&self, src: &[u8], dst: &mut [u8], datatype: DataType, op: ReductionOp, root: u64) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.fanin_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype, op, root) }
    }

    /// Fanin from src into dst (raw pointer version).
    pub unsafe fn fanin_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, op: ReductionOp, root: u64) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_FANIN)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .reduction_op(op)
            .root(root)
            .post()
    }

    /// Fanout from src into dst (safe slice version).
    pub fn fanout(&self, src: &[u8], dst: &mut [u8], datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        unsafe { self.fanout_raw(src.as_ptr() as *mut std::os::raw::c_void, dst.as_ptr() as *mut std::os::raw::c_void, src.len() as ucc_count_t, datatype, root) }
    }

    /// Fanout from src into dst (raw pointer version).
    pub unsafe fn fanout_raw(&self, src: *mut std::os::raw::c_void, dst: *mut std::os::raw::c_void, count: ucc_count_t, datatype: DataType, root: u64) -> Result<UccCollRequest, UccStatus> {
        CollectiveBuilder::new(self)
            .coll_type(ucc_coll_type_t_UCC_COLL_TYPE_FANOUT)
            .src_buffer(src)
            .dst_buffer(dst)
            .count(count)
            .datatype(datatype)
            .root(root)
            .post()
    }
}
