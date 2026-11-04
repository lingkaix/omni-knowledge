//! Bounded syntax/wire admission without domain execution or raw input diagnostics.

use crate::dto::{
    Command, OperationState, PackageFormat, PlanSelection, Request, Response, Result as WireResult,
    SearchRequest,
};
use serde_json::error::Category;
use std::{
    fmt,
    io::{self, Write},
};

/// The only supported service wire version.
pub const WIRE_VERSION: u64 = 1;
/// Maximum bytes in one supplied control payload, checked before JSON parsing.
pub const MAX_CONTROL_FRAME_BYTES: usize = 1_048_576;

/// A fixed, input-free codec refusal. No source error or command bytes are retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    /// The input or encoded response exceeds one MiB; no partial output is returned.
    FrameTooLarge,
    /// The supplied payload is not UTF-8.
    InvalidUtf8,
    /// The payload is incomplete or not a single valid JSON text.
    InvalidJson,
    /// Required fields, tags, scalars, or explicit wire bounds are invalid.
    InvalidWireData,
    /// A fully decoded envelope does not select wire version 1.
    UnsupportedVersion,
}

impl fmt::Display for CodecError {
    /// Write a fixed diagnostic, never serde's input-bearing error message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FrameTooLarge => "control frame exceeds the byte limit",
            Self::InvalidUtf8 => "control frame is not UTF-8",
            Self::InvalidJson => "invalid JSON control frame",
            Self::InvalidWireData => "invalid service wire data",
            Self::UnsupportedVersion => "unsupported service wire version",
        })
    }
}
impl std::error::Error for CodecError {}

/// Decode one complete UTF-8 JSON request, retaining its strings unchanged.
///
/// The byte bound is checked before UTF-8 or JSON parsing. Missing nullable fields,
/// unknown command/nested fields, unknown operations/tags, unsafe integers, invalid
/// paths, and explicit command bounds are refused. A supplied trailing delimiter
/// counts toward the bound; no stream splitting, I/O, or authority check occurs.
///
/// # Errors
/// Returns an input-free [`CodecError`] for oversized, malformed, unsupported, or
/// invalid wire data. Successful decoding is not domain validation or admission.
pub fn decode_request(bytes: &[u8]) -> std::result::Result<Request, CodecError> {
    if bytes.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(CodecError::FrameTooLarge);
    }
    std::str::from_utf8(bytes).map_err(|_| CodecError::InvalidUtf8)?;
    let request: Request =
        serde_json::from_slice(bytes).map_err(|error| match error.classify() {
            Category::Syntax | Category::Eof => CodecError::InvalidJson,
            Category::Data | Category::Io => CodecError::InvalidWireData,
        })?;
    if request.wire_version != WIRE_VERSION {
        return Err(CodecError::UnsupportedVersion);
    }
    validate_command(&request.command)?;
    Ok(request)
}

/// Encode one compact UTF-8 JSON response without a newline or transport wrapper.
///
/// Checked numeric scalars cannot silently serialize NaN/Infinity as null. Output
/// allocation is bounded during serialization, not merely checked afterward. The
/// function checks version and explicit response wire constraints, but cannot
/// establish whether source facts, exposure, provenance, or effects are truthful.
///
/// # Errors
/// Returns an input-free [`CodecError`] for unsupported versions, invalid response
/// wire data, or an oversized result. It never returns a partial response.
pub fn encode_response(response: &Response) -> std::result::Result<Vec<u8>, CodecError> {
    if response.wire_version != WIRE_VERSION {
        return Err(CodecError::UnsupportedVersion);
    }
    validate_result(&response.result)?;
    let mut writer = FrameWriter {
        bytes: Vec::new(),
        exceeded: false,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(if writer.exceeded {
            CodecError::FrameTooLarge
        } else {
            CodecError::InvalidWireData
        });
    }
    Ok(writer.bytes)
}

/// Check the owner-defined inclusive range without choosing defaults.
fn in_range(value: u64, maximum: u64) -> std::result::Result<(), CodecError> {
    if (1..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(CodecError::InvalidWireData)
    }
}

/// Check request-only bounds; strings and identity remain unchanged.
fn validate_command(command: &Command) -> std::result::Result<(), CodecError> {
    match command {
        Command::Browse { limit, .. } | Command::HistoryList { limit, .. } => {
            in_range(limit.get(), 1_000)
        }
        Command::ReadRange { length, .. }
        | Command::HistoryDiff { length, .. }
        | Command::ProposalDiff { length, .. } => in_range(length.get(), 524_288),
        Command::ReadRepresentation { max_lines, .. } => in_range(max_lines.get(), 20_000),
        Command::Query { query } => validate_query(query),
        Command::SyncSchedule {
            enabled: true,
            interval_minutes,
            ..
        } if interval_minutes.get() == 0 => Err(CodecError::InvalidWireData),
        Command::EpubPlan { page_paths, .. } if page_paths.is_empty() => {
            Err(CodecError::InvalidWireData)
        }
        _ => Ok(()),
    }
}

/// Check finite query bounds without method execution, normalization, or capability checks.
fn validate_query(query: &SearchRequest) -> std::result::Result<(), CodecError> {
    in_range(query.limit.get(), 10_000)?;
    if !query.keywords.is_empty() && query.methods.is_empty() {
        return Err(CodecError::InvalidWireData);
    }
    Ok(())
}

/// Check explicit wire-only result invariants, not domain truth or admission.
fn validate_result(result: &WireResult) -> std::result::Result<(), CodecError> {
    match result {
        WireResult::Operation { state, outcome, .. } => {
            if *state == OperationState::Pending && outcome.is_some() {
                return Err(CodecError::InvalidWireData);
            }
            if let Some(outcome) = outcome {
                if matches!(outcome.as_ref(), WireResult::Operation { .. }) {
                    return Err(CodecError::InvalidWireData);
                }
                validate_result(outcome)?;
            }
        }
        WireResult::Search { search } => {
            validate_query(&search.applied)?;
            if search
                .groups
                .iter()
                .flat_map(|group| &group.files)
                .flat_map(|file| &file.evidence)
                .any(|evidence| evidence.snippet.chars().count() > 500)
            {
                return Err(CodecError::InvalidWireData);
            }
        }
        WireResult::PackagePlan { plan } => {
            let valid = matches!(
                (&plan.format, &plan.selection),
                (PackageFormat::Epub, PlanSelection::Epub(_))
                    | (
                        PackageFormat::NativeZip | PackageFormat::OpenkitV1,
                        PlanSelection::Package(_)
                    )
            );
            if !valid {
                return Err(CodecError::InvalidWireData);
            }
        }
        WireResult::Diff { unified_diff, .. } if unified_diff.len() > 524_288 => {
            return Err(CodecError::InvalidWireData);
        }
        _ => {}
    }
    Ok(())
}

/// A private output buffer that refuses growth past the control-frame bound.
struct FrameWriter {
    /// Only complete successful serialization is returned to a caller.
    bytes: Vec<u8>,
    /// Distinguishes an output budget refusal from a scalar serialization failure.
    exceeded: bool,
}
impl Write for FrameWriter {
    /// Append a complete serializer fragment or refuse it without partial growth.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_CONTROL_FRAME_BYTES - self.bytes.len() {
            self.exceeded = true;
            return Err(io::Error::other("control frame exceeds the byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    /// No external buffered resource exists to flush.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
