//! Real public codec demonstration, not a backend or a service executable.
//!
//! Reads at most one bounded synthetic request and emits an explicit unsupported
//! response fixture. No command is executed and no library state is fabricated.

use omni_service::{
    ErrorCode, MAX_CONTROL_FRAME_BYTES, Response, Result as WireResult, ServiceError, WIRE_VERSION,
    decode_request, encode_response,
};
use std::io::{self, Read, Write};

/// Cross a real external JSON producer/consumer boundary using only the public API.
///
/// Returns fixed codec/I/O errors; no command arguments are printed on failure.
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take((MAX_CONTROL_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let request = decode_request(&bytes)?;
    let response = Response {
        wire_version: WIRE_VERSION,
        request_id: request.request_id,
        result: WireResult::Error {
            error: ServiceError {
                code: ErrorCode::Unsupported,
                message: "Codec demonstration; no command was executed.".into(),
                retryable: false,
                operation_id: None,
                library_id: None,
                source_id: None,
                job_id: None,
                method: None,
                recovery_paths: Vec::new(),
            },
        },
    };
    io::stdout()
        .lock()
        .write_all(&encode_response(&response)?)?;
    Ok(())
}
