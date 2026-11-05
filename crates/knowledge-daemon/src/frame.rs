//! One newline-delimited control frame, capped at the service codec's 1 MiB bound.
//!
//! The retained selected-root MCP server still uses its historical 4 MiB ceiling.
//! This reader is the versioned service transport, not a silent reclassification of that ceiling.

use std::io::{self, BufRead};

/// Read one LF-terminated frame.
///
/// `Ok(None)` is clean EOF before any byte. An oversized or non-UTF-8 frame is an error
/// and the caller must not decode a prefix. A final frame without LF is returned when
/// it fits, so a half-close after a complete payload still reaches the codec.
pub fn read_frame(input: &mut impl BufRead, max_bytes: usize) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    let terminated = loop {
        let available = match input.fill_buf() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if available.is_empty() {
            break false;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.unwrap_or(available.len());
        if bytes.len().saturating_add(count) > max_bytes.saturating_add(1) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "control frame exceeds 1 MiB",
            ));
        }
        bytes.extend_from_slice(&available[..count]);
        if bytes.len() > max_bytes && bytes.last() != Some(&b'\r') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "control frame exceeds 1 MiB",
            ));
        }
        input.consume(count + usize::from(newline.is_some()));
        if newline.is_some() {
            break true;
        }
    };
    if terminated && bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "control frame exceeds 1 MiB",
        ));
    }
    if bytes.is_empty() && !terminated {
        return Ok(None);
    }
    Ok(Some(bytes))
}
