//! UCC status codes and error handling.
//!
//! Wraps `ucc_status_t` with idiomatic Rust error handling.

use crate::bindings::{
    ucc_status_t,
    // constified enum constants
    ucc_status_t_UCC_ERR_INVALID_PARAM,
    ucc_status_t_UCC_ERR_NO_MEMORY,
    ucc_status_t_UCC_ERR_NO_MESSAGE,
    ucc_status_t_UCC_ERR_NO_RESOURCE,
    ucc_status_t_UCC_ERR_NOT_FOUND,
    ucc_status_t_UCC_ERR_NOT_IMPLEMENTED,
    ucc_status_t_UCC_ERR_NOT_SUPPORTED,
    ucc_status_t_UCC_ERR_TIMED_OUT,
    ucc_status_t_UCC_INPROGRESS,
    ucc_status_t_UCC_OK,
};

/// UCC status code wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UccStatus(pub ucc_status_t);

impl UccStatus {
    /// Check if this is a success status.
    pub fn is_ok(&self) -> bool {
        self.0 == ucc_status_t_UCC_OK
    }

    /// Check if this is an in-progress status.
    pub fn is_inprogress(&self) -> bool {
        self.0 == ucc_status_t_UCC_INPROGRESS
    }

    /// Check if this is an error status.
    pub fn is_error(&self) -> bool {
        self.0 < 0
    }
}

impl std::fmt::Display for UccStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ucc_status_t_UCC_OK => write!(f, "UCC_OK"),
            ucc_status_t_UCC_INPROGRESS => write!(f, "UCC_INPROGRESS"),
            ucc_status_t_UCC_ERR_NOT_SUPPORTED => write!(f, "UCC_ERR_NOT_SUPPORTED"),
            ucc_status_t_UCC_ERR_NOT_IMPLEMENTED => write!(f, "UCC_ERR_NOT_IMPLEMENTED"),
            ucc_status_t_UCC_ERR_INVALID_PARAM => write!(f, "UCC_ERR_INVALID_PARAM"),
            ucc_status_t_UCC_ERR_NO_MEMORY => write!(f, "UCC_ERR_NO_MEMORY"),
            ucc_status_t_UCC_ERR_NO_MESSAGE => write!(f, "UCC_ERR_NO_MESSAGE"),
            ucc_status_t_UCC_ERR_NOT_FOUND => write!(f, "UCC_ERR_NOT_FOUND"),
            ucc_status_t_UCC_ERR_TIMED_OUT => write!(f, "UCC_ERR_TIMED_OUT"),
            ucc_status_t_UCC_ERR_NO_RESOURCE => write!(f, "UCC_ERR_NO_RESOURCE"),
            _ => write!(f, "UCC_ERROR({})", self.0),
        }
    }
}

impl std::error::Error for UccStatus {}

/// Check a UCC status code and return an error if it's not OK.
pub fn check_status(status: ucc_status_t) -> Result<(), UccStatus> {
    if status == ucc_status_t_UCC_OK {
        Ok(())
    } else {
        Err(UccStatus(status))
    }
}
