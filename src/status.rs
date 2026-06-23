//! UCC status codes and error handling.
//!
//! Wraps `ucc_status_t` with idiomatic Rust error handling, following the
//! pmix-rs pattern of a two-tier type system:
//!
//! * [`UccError`] — exhaustive enum covering every standard UCC status code.
//!   Use this when you want compile-time discrimination between error variants.
//! * [`UccStatus`] — supertype that wraps [`UccError`] plus an `Unknown(i32)`
//!   catch-all for future library extensions or user-defined codes.
//!
//! Convert between raw `ucc_status_t` and [`UccStatus`] with
//! [`UccStatus::from_raw`] / [`UccStatus::to_raw`].
//!
//! # Example
//!
//! ```
//! use ucc_rs::{UccError, UccStatus};
//!
//! assert_eq!(UccStatus::from_raw(0), UccStatus::Known(UccError::Ok));
//! assert_eq!(UccStatus::from_raw(1), UccStatus::Known(UccError::InProgress));
//! assert!(matches!(UccStatus::from_raw(-999), UccStatus::Unknown(_)));
//! ```

use crate::bindings::ucc_status_t;

// ─────────────────────────────────────────────────────────────────────────────
// UccError — known, standard UCC status codes
// ─────────────────────────────────────────────────────────────────────────────

/// Safe Rust representation of `ucc_status_t`.
///
/// `ucc_status_t` is an `i32` where:
/// * `0`            → [`Ok`][UccError::Ok]
/// * positive       → informational / progress codes
/// * negative       → error codes
///
/// All values defined in the UCC API are represented. Unknown raw values
/// produced by future library versions are captured in
/// [`UccStatus::Unknown`].
///
/// The enum is `#[non_exhaustive]` so that adding new variants in a future
/// library version is a non-breaking change for downstream crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
#[non_exhaustive]
pub enum UccError {
    // ── Success ────────────────────────────────────────────────────────────
    /// `UCC_OK` (0) — operation completed successfully.
    Ok = 0,

    // ── Informational / progress ───────────────────────────────────────────
    /// `UCC_INPROGRESS` (1) — operation is still in progress; poll again.
    InProgress = 1,

    /// `UCC_OPERATION_INITIALIZED` (2) — operation has been initialized but
    /// not yet started.
    OperationInitialized = 2,

    // ── Error codes ────────────────────────────────────────────────────────
    /// `UCC_ERR_NOT_SUPPORTED` (-1) — the requested feature or API is not
    /// supported by this UCC build.
    ErrNotSupported = -1,

    /// `UCC_ERR_NOT_IMPLEMENTED` (-2) — the requested feature exists in the
    /// API but has not been implemented yet.
    ErrNotImplemented = -2,

    /// `UCC_ERR_INVALID_PARAM` (-3) — one or more parameters are out of range
    /// or inconsistent.
    ErrInvalidParam = -3,

    /// `UCC_ERR_NO_MEMORY` (-4) — memory allocation failed.
    ErrNoMemory = -4,

    /// `UCC_ERR_NO_RESOURCE` (-5) — a system resource was exhausted.
    ErrNoResource = -5,

    /// `UCC_ERR_NO_MESSAGE` (-6) — no message was available when one was
    /// expected.
    ErrNoMessage = -6,

    /// `UCC_ERR_NOT_FOUND` (-7) — the requested item does not exist.
    ErrNotFound = -7,

    /// `UCC_ERR_TIMED_OUT` (-8) — operation exceeded its timeout.
    ErrTimedOut = -8,

    /// `UCC_ERR_IO_ERROR` (-9) — I/O error during operation.
    ErrIoError = -9,

    // ── Sentinel ───────────────────────────────────────────────────────────
    /// `UCC_ERR_LAST` (-100) — sentinel marking the end of the standard
    /// error range. Values more negative than this are user-defined.
    ErrLast = -100,
}

impl UccError {
    /// Convert a raw `ucc_status_t` (`i32`) to a `UccError`, or `None` if the
    /// value is not a known standard code.
    ///
    /// This is O(1) — it matches against a table of all known discriminants.
    pub fn from_raw(code: i32) -> Option<Self> {
        Some(match code {
            0 => Self::Ok,
            1 => Self::InProgress,
            2 => Self::OperationInitialized,
            -1 => Self::ErrNotSupported,
            -2 => Self::ErrNotImplemented,
            -3 => Self::ErrInvalidParam,
            -4 => Self::ErrNoMemory,
            -5 => Self::ErrNoResource,
            -6 => Self::ErrNoMessage,
            -7 => Self::ErrNotFound,
            -8 => Self::ErrTimedOut,
            -9 => Self::ErrIoError,
            -100 => Self::ErrLast,
            _ => return None,
        })
    }

    /// Return the raw `i32` value.
    pub fn to_raw(self) -> i32 {
        self as i32
    }

    /// `true` for success and informational codes (>= 0).
    pub fn is_success(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// `true` for in-progress / not-yet-complete codes.
    pub fn is_inprogress(self) -> bool {
        matches!(self, Self::InProgress | Self::OperationInitialized)
    }

    /// `true` for error codes (< 0).
    pub fn is_error(self) -> bool {
        !self.is_success() && !self.is_inprogress()
    }
}

impl std::fmt::Display for UccError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "UCC_OK"),
            Self::InProgress => write!(f, "UCC_INPROGRESS"),
            Self::OperationInitialized => write!(f, "UCC_OPERATION_INITIALIZED"),
            Self::ErrNotSupported => write!(f, "UCC_ERR_NOT_SUPPORTED"),
            Self::ErrNotImplemented => write!(f, "UCC_ERR_NOT_IMPLEMENTED"),
            Self::ErrInvalidParam => write!(f, "UCC_ERR_INVALID_PARAM"),
            Self::ErrNoMemory => write!(f, "UCC_ERR_NO_MEMORY"),
            Self::ErrNoResource => write!(f, "UCC_ERR_NO_RESOURCE"),
            Self::ErrNoMessage => write!(f, "UCC_ERR_NO_MESSAGE"),
            Self::ErrNotFound => write!(f, "UCC_ERR_NOT_FOUND"),
            Self::ErrTimedOut => write!(f, "UCC_ERR_TIMED_OUT"),
            Self::ErrIoError => write!(f, "UCC_ERR_IO_ERROR"),
            Self::ErrLast => write!(f, "UCC_ERR_LAST"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UccStatus — supertype: Known(UccError) | Unknown(i32)
// ─────────────────────────────────────────────────────────────────────────────

/// The complete value space of `ucc_status_t`.
///
/// [`UccError`] covers every constant defined in the current UCC API.
/// `UccStatus::Unknown` captures any raw value that does not correspond to a
/// known constant — typically a user-defined code or a future library
/// extension not yet reflected in this crate.
///
/// Use [`UccStatus::from_raw`] to convert a `ucc_status_t` received from C
/// and [`UccStatus::to_raw`] to convert back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UccStatus {
    /// A known, standard UCC status code.
    Known(UccError),
    /// An unrecognised or user-defined status code.
    Unknown(i32),
}

impl UccStatus {
    /// Convert a raw `ucc_status_t` (`i32`) into a `UccStatus`.
    ///
    /// ```
    /// use ucc_rs::{UccError, UccStatus};
    ///
    /// assert_eq!(UccStatus::from_raw(0), UccStatus::Known(UccError::Ok));
    /// assert_eq!(UccStatus::from_raw(1), UccStatus::Known(UccError::InProgress));
    /// assert!(matches!(UccStatus::from_raw(-999), UccStatus::Unknown(_)));
    /// ```
    pub fn from_raw(code: i32) -> Self {
        match UccError::from_raw(code) {
            Some(e) => Self::Known(e),
            None => Self::Unknown(code),
        }
    }

    /// Return the raw `i32` value.
    pub fn to_raw(self) -> i32 {
        match self {
            Self::Known(e) => e as i32,
            Self::Unknown(v) => v,
        }
    }

    /// `true` for `UCC_OK` (exact success).
    pub fn is_ok(self) -> bool {
        match self {
            Self::Known(e) => e.is_success(),
            Self::Unknown(v) => v == 0,
        }
    }

    /// `true` for in-progress / not-yet-complete codes.
    pub fn is_inprogress(self) -> bool {
        match self {
            Self::Known(e) => e.is_inprogress(),
            Self::Unknown(v) => v > 0,
        }
    }

    /// `true` for all negative codes (errors).
    pub fn is_error(self) -> bool {
        !self.is_ok() && !self.is_inprogress()
    }

    /// Return the inner `UccError` if the code is known.
    pub fn known(self) -> Option<UccError> {
        match self {
            Self::Known(e) => Some(e),
            _ => None,
        }
    }
}

impl std::fmt::Display for UccStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Known(e) => e.fmt(f),
            Self::Unknown(v) => write!(f, "ucc_status_t({v}) [unknown/user-defined]"),
        }
    }
}

impl std::error::Error for UccStatus {}

impl From<UccError> for UccStatus {
    fn from(e: UccError) -> Self {
        Self::Known(e)
    }
}

impl From<ucc_status_t> for UccStatus {
    fn from(raw: ucc_status_t) -> Self {
        Self::from_raw(raw as i32)
    }
}

/// Check a UCC status code and return an error if it's not OK.
///
/// Returns `Ok(())` for `UCC_OK` and `Err(UccStatus)` for anything else
/// (including `UCC_INPROGRESS` and error codes).
pub fn check_status(status: ucc_status_t) -> Result<(), UccStatus> {
    let ucc_status = UccStatus::from_raw(status as i32);
    if ucc_status.is_ok() {
        Ok(())
    } else {
        Err(ucc_status)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ucc_error_from_raw_known() {
        assert_eq!(UccError::from_raw(0), Some(UccError::Ok));
        assert_eq!(UccError::from_raw(1), Some(UccError::InProgress));
        assert_eq!(UccError::from_raw(-1), Some(UccError::ErrNotSupported));
        assert_eq!(UccError::from_raw(-5), Some(UccError::ErrNoResource));
        assert_eq!(UccError::from_raw(-100), Some(UccError::ErrLast));
    }

    #[test]
    fn test_ucc_error_from_raw_unknown() {
        assert_eq!(UccError::from_raw(-999), None);
        assert_eq!(UccError::from_raw(42), None);
    }

    #[test]
    fn test_ucc_error_to_raw_roundtrip() {
        for &variant in &[
            UccError::Ok,
            UccError::InProgress,
            UccError::ErrNotSupported,
            UccError::ErrNoResource,
            UccError::ErrLast,
        ] {
            assert_eq!(UccError::from_raw(variant.to_raw()), Some(variant));
        }
    }

    #[test]
    fn test_ucc_status_from_raw_known() {
        assert_eq!(UccStatus::from_raw(0), UccStatus::Known(UccError::Ok));
        assert_eq!(
            UccStatus::from_raw(1),
            UccStatus::Known(UccError::InProgress)
        );
        assert_eq!(
            UccStatus::from_raw(-3),
            UccStatus::Known(UccError::ErrInvalidParam)
        );
    }

    #[test]
    fn test_ucc_status_from_raw_unknown() {
        assert!(matches!(UccStatus::from_raw(-999), UccStatus::Unknown(_)));
        assert!(matches!(UccStatus::from_raw(42), UccStatus::Unknown(_)));
    }

    #[test]
    fn test_ucc_status_is_ok() {
        assert!(UccStatus::from_raw(0).is_ok());
        assert!(!UccStatus::from_raw(1).is_ok());
        assert!(!UccStatus::from_raw(-1).is_ok());
        assert!(!UccStatus::from_raw(-999).is_ok());
    }

    #[test]
    fn test_ucc_status_is_inprogress() {
        assert!(!UccStatus::from_raw(0).is_inprogress());
        assert!(UccStatus::from_raw(1).is_inprogress());
        assert!(UccStatus::from_raw(2).is_inprogress());
        assert!(!UccStatus::from_raw(-1).is_inprogress());
    }

    #[test]
    fn test_ucc_status_is_error() {
        assert!(!UccStatus::from_raw(0).is_error());
        assert!(!UccStatus::from_raw(1).is_error());
        assert!(UccStatus::from_raw(-1).is_error());
        assert!(UccStatus::from_raw(-999).is_error());
    }

    #[test]
    fn test_ucc_status_known() {
        assert!(UccStatus::from_raw(0).known().is_some());
        assert!(UccStatus::from_raw(-1).known().is_some());
        assert!(UccStatus::from_raw(-999).known().is_none());
    }

    #[test]
    fn test_ucc_status_display_known() {
        assert_eq!(format!("{}", UccStatus::from_raw(0)), "UCC_OK");
        assert_eq!(format!("{}", UccStatus::from_raw(1)), "UCC_INPROGRESS");
        assert_eq!(
            format!("{}", UccStatus::from_raw(-3)),
            "UCC_ERR_INVALID_PARAM"
        );
    }

    #[test]
    fn test_ucc_status_display_unknown() {
        assert_eq!(
            format!("{}", UccStatus::from_raw(-999)),
            "ucc_status_t(-999) [unknown/user-defined]"
        );
    }

    #[test]
    fn test_check_status_ok() {
        assert!(check_status(0).is_ok());
    }

    #[test]
    fn test_check_status_error() {
        let err = check_status(-1).unwrap_err();
        assert_eq!(err, UccStatus::Known(UccError::ErrNotSupported));
    }

    #[test]
    fn test_check_status_inprogress() {
        let err = check_status(1).unwrap_err();
        assert_eq!(err, UccStatus::Known(UccError::InProgress));
    }

    #[test]
    fn test_from_ucc_error() {
        let status: UccStatus = UccError::Ok.into();
        assert_eq!(status, UccStatus::Known(UccError::Ok));
    }

    #[test]
    fn test_ucc_error_display() {
        assert_eq!(format!("{}", UccError::Ok), "UCC_OK");
        assert_eq!(format!("{}", UccError::ErrNoMemory), "UCC_ERR_NO_MEMORY");
        assert_eq!(format!("{}", UccError::ErrLast), "UCC_ERR_LAST");
    }

    #[test]
    fn test_ucc_error_classification() {
        assert!(UccError::Ok.is_success());
        assert!(!UccError::Ok.is_inprogress());
        assert!(!UccError::Ok.is_error());

        assert!(!UccError::InProgress.is_success());
        assert!(UccError::InProgress.is_inprogress());
        assert!(!UccError::InProgress.is_error());

        assert!(!UccError::ErrNotSupported.is_success());
        assert!(!UccError::ErrNotSupported.is_inprogress());
        assert!(UccError::ErrNotSupported.is_error());
    }
}
