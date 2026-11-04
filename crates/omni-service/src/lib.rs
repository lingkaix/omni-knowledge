//! Version-1 public service data and a finite JSON codec.
//!
//! The accepted `docs/specs/service.md` owns this boundary. These types describe
//! data, not admitted filesystem handles, caller identity, or capabilities.
//! Decoding an owner-only command does not authorize it. No operation handler,
//! backend, transport, daemon, or exposure implementation exists in this crate.
//!
//! Use [`decode_request`] for one complete control payload and [`encode_response`]
//! for one response. Neither function reads a file, logs input, executes commands,
//! or adds a transport delimiter. Transport framing and domain admission remain
//! the caller's responsibility. Serde traits alone do not enforce envelope bounds
//! or command bounds; use the codec at the boundary.
//!
//! ```
//! use omni_service::{Command, decode_request};
//! let request = decode_request(
//!     br#"{"wire_version":1,"request_id":"example","command":{"op":"library_list"}}"#,
//! )?;
//! assert!(matches!(request.command, Command::LibraryList { .. }));
//! # Ok::<(), omni_service::CodecError>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod codec;
pub mod dto;
pub mod scalar;

pub use codec::{
    CodecError, MAX_CONTROL_FRAME_BYTES, WIRE_VERSION, decode_request, encode_response,
};
pub use dto::*;
pub use scalar::*;
