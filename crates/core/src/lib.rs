//! Shared domain primitives for Veil: stable error codes and core enums.
//!
//! `ErrorCode` values are part of the contract with the UI — each serializes to a
//! dotted string that maps to an i18n key (`errors.<code>`) and to the TypeScript
//! `AppErrorCode` union in `@veil/shared`.

pub mod android_device;
pub mod error;
pub mod fingerprint;
pub mod types;

pub use error::{AppError, AppResult, ErrorCode};
pub use fingerprint::{
    derived_work_area, to_engine_args, validate, work_area_inset, Fingerprint, FlagDialect,
    LaunchOptions, NoiseMode, WebrtcPolicy,
};
pub use types::{ProfileOs, ProfileStatus};
