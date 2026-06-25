//! UCC memory mapping and management.
//!
//! Registers host memory regions with UCC so they can be used in collective
//! operations. Memory is mapped via [`UccMemHandle::map_raw`] (unsafe pointer),
//! [`UccMemHandle::map_slice`] (immutable slice), or [`UccMemHandle::map_slice_mut`]
//! (mutable slice). The returned handle holds the mapping and automatically
//! unmaps the memory on drop.
//!
//! # Safety
//!
//! The underlying memory must remain valid for the entire lifetime of the
//! [`UccMemHandle`]. UCC does not copy the data — it only records the address
//! and length for later use by collective operations.

use crate::bindings::{
    ucc_mem_map,
    ucc_mem_map_mem_h,
    // constified enum constants
    ucc_mem_map_mode_t_UCC_MEM_MAP_MODE_EXPORT,
    ucc_mem_map_params,
    ucc_mem_map_params_t,
    ucc_mem_unmap,
};
use crate::context::UccContext;
use crate::status::{check_status, UccError, UccStatus};

/// Memory handle with RAII cleanup.
///
/// Represents a memory region registered with UCC. The handle holds the
/// mapping and automatically unmaps the memory on drop.
///
/// # Safety
///
/// The underlying memory must remain valid for the entire lifetime of this
/// handle. UCC does not copy the data — it only records the address and
/// length for later use by collective operations.
pub struct UccMemHandle {
    memh: ucc_mem_map_mem_h,
    mapped_len: usize,
}

impl UccMemHandle {
    /// Map memory at a context using a raw pointer and length.
    ///
    /// # Safety
    /// The caller must ensure that `addr` points to a valid memory region
    /// of at least `length` bytes that remains valid for the lifetime of
    /// the returned [`UccMemHandle`].
    pub unsafe fn map_raw(
        context: &UccContext,
        addr: *mut std::os::raw::c_void,
        length: usize,
    ) -> Result<Self, UccStatus> {
        let ctx_handle = context.handle();
        let mut memh: ucc_mem_map_mem_h = std::ptr::null_mut();
        let mut memh_size: usize = 0;

        // Build params with segments
        let mut segments = [ucc_mem_map {
            address: addr,
            len: length,
        }];
        let params: ucc_mem_map_params_t = ucc_mem_map_params {
            segments: segments.as_mut_ptr(),
            n_segments: 1,
        };

        let status = unsafe {
            // Safety: context handle, addr, and mem_params are valid; mem_handle is output pointer.
            ucc_mem_map(
                ctx_handle,
                ucc_mem_map_mode_t_UCC_MEM_MAP_MODE_EXPORT,
                &params,
                &mut memh_size,
                &mut memh,
            )
        };
        check_status(status)?;
        if memh.is_null() {
            return Err(UccStatus::Known(UccError::ErrNoResource));
        }
        Ok(Self {
            memh,
            mapped_len: length,
        })
    }

    /// Safely map a shared byte slice at a context.
    ///
    /// This method does not copy the data — it only registers the slice's
    /// address and length with the C API. The caller must ensure the
    /// underlying memory remains valid for the lifetime of the returned
    /// [`UccMemHandle`].
    pub fn map_slice(context: &UccContext, slice: &[u8]) -> Result<Self, UccStatus> {
        unsafe {
            // Safety: slice pointer is valid for the handle lifetime; caller guarantees memory outlives the handle.
            Self::map_raw(
                context,
                slice.as_ptr() as *mut std::os::raw::c_void,
                slice.len(),
            )
        }
    }

    /// Safely map a mutable byte slice at a context.
    ///
    /// This method does not copy the data — it only registers the slice's
    /// address and length with the C API. The caller must ensure the
    /// underlying memory remains valid for the lifetime of the returned
    /// [`UccMemHandle`].
    pub fn map_slice_mut(context: &UccContext, slice: &mut [u8]) -> Result<Self, UccStatus> {
        unsafe {
            // Safety: slice pointer is valid for the handle lifetime; caller guarantees memory outlives the handle.
            Self::map_raw(
                context,
                slice.as_mut_ptr() as *mut std::os::raw::c_void,
                slice.len(),
            )
        }
    }

    /// Get the raw memory handle.
    pub fn handle(&self) -> ucc_mem_map_mem_h {
        self.memh
    }

    /// Get the length of the mapped memory region.
    pub fn mapped_len(&self) -> usize {
        self.mapped_len
    }
}

impl Drop for UccMemHandle {
    fn drop(&mut self) {
        if !self.memh.is_null() {
            let mut memh = self.memh;
            unsafe {
                // Safety: mem_handle was validated non-null above; safe to unmap in Drop.
                ucc_mem_unmap(&mut memh);
            }
            self.memh = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib_init::UccLib;

    // ── Integration tests (call into libucc.so) ────────────────────────────

    mod integration_tests {
        use super::*;

        #[test]
        fn integration_mem_map_slice() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let data: [u8; 256] = [42; 256];
            let handle = UccMemHandle::map_slice(&ctx, &data).expect("map_slice should succeed");
            assert_eq!(
                handle.mapped_len(),
                256,
                "mapped_len should match slice length"
            );
            assert!(
                !handle.handle().is_null(),
                "memory handle should be non-null"
            );
            // handle dropped here — ucc_mem_unmap called automatically
        }

        #[test]
        fn integration_mem_map_slice_mut() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let mut data = vec![0u8; 512];
            let handle =
                UccMemHandle::map_slice_mut(&ctx, &mut data).expect("map_slice_mut should succeed");
            assert_eq!(
                handle.mapped_len(),
                512,
                "mapped_len should match slice length"
            );
            // handle dropped here
        }

        #[test]
        fn integration_mem_map_multiple_regions() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let data1: [u8; 64] = [1; 64];
            let data2: [u8; 128] = [2; 128];
            let handle1 =
                UccMemHandle::map_slice(&ctx, &data1).expect("map_slice #1 should succeed");
            let handle2 =
                UccMemHandle::map_slice(&ctx, &data2).expect("map_slice #2 should succeed");
            assert_eq!(handle1.mapped_len(), 64);
            assert_eq!(handle2.mapped_len(), 128);
            // Both handles dropped here
        }

        #[test]
        fn integration_mem_map_empty_slice() {
            let lib = UccLib::init().expect("init");
            let ctx = UccContext::new(lib).expect("context create");
            let data: [u8; 0] = [];
            // Mapping zero-length may succeed or fail depending on UCC version;
            // we just verify it doesn't crash either way.
            let result = UccMemHandle::map_slice(&ctx, &data);
            if let Ok(handle) = result {
                assert_eq!(handle.mapped_len(), 0);
            }
        }
    }
}
