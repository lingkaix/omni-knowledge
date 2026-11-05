//! Codec responses for transport refusals.
//!
//! These are framing results. They are not domain success and they do not echo input.

use omni_service::{CodecError, ErrorCode, Id, Response, Result as WireResult, ServiceError};

/// Build a typed error envelope. Identifiers in the error stay null.
pub fn error_response(request_id: Id, code: ErrorCode, message: &str, retryable: bool) -> Response {
    Response {
        wire_version: 1,
        request_id,
        result: WireResult::Error {
            error: ServiceError {
                code,
                message: message.to_owned(),
                retryable,
                operation_id: None,
                library_id: None,
                source_id: None,
                job_id: None,
                method: None,
                recovery_paths: Vec::new(),
            },
        },
    }
}

/// Map a codec failure to the service error code used at the HTTP boundary.
pub fn codec_code(error: CodecError) -> ErrorCode {
    match error {
        CodecError::UnsupportedVersion => ErrorCode::UnsupportedVersion,
        CodecError::FrameTooLarge => ErrorCode::BudgetExhausted,
        CodecError::InvalidUtf8 | CodecError::InvalidJson | CodecError::InvalidWireData => {
            ErrorCode::InvalidRequest
        }
    }
}
