//! Portable metadata behavior, not filesystem, authorization, or application orchestration.
//!
//! Read [the crate guide](../README.md) before integrating a backend or persisting a candidate.
//! Manifest editing is pure and preserves opaque authored fields; backend data is never a grant.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod backend;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod cache;
pub mod data;
pub mod jobs;
pub mod manifest;
