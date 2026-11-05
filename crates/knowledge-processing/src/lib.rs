//! Mature extraction, EPUB, media, and local speech adapters.
//!
//! The host admits every effect on the one process account. This crate does not
//! start that account, mint a source digest, or keep a second pool.
//! Read [the crate guide](../README.md) before calling the admitted entry points.

#![doc = include_str!("../README.md")]

pub mod admission;
pub mod embedding;
pub mod epub;
pub mod extract;
pub mod funding;
pub mod held;
pub mod media;
mod opaque;
pub mod runtime;
pub mod worker;

pub use admission::{AdmitError, Admitted, Charge, establish_process_account, reservation};
pub use knowledge_core::admission::{Account, Holding};
pub use worker::WorkerFailure;
