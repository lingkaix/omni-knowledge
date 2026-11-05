//! Mature extraction, EPUB, media, and local speech adapters.
//!
//! The host admits every effect on the one process account. This crate does not
//! start that account, mint a source digest, or keep a second pool.
//! Read [the crate guide](../README.md) before calling the admitted entry points.

pub mod admission;
pub mod epub;
pub mod extract;
pub mod media;
pub mod runtime;
pub mod worker;

pub use admission::{AdmitError, Bound, Charge, SharedAccount, reservation};
pub use worker::WorkerFailure;
