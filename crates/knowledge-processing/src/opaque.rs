//! Real parser subprocesses borrow admitted input and use prepaid capture/result lifetimes.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::extract::Extraction;
use crate::funding::{Credits, DIAGNOSTIC_CAP, Funding};
use crate::worker;

/// Bounded JSON transport for the existing extraction envelope and escaped text.
pub(crate) fn transport_cap(max_bytes: usize) -> u64 {
    (max_bytes as u64).saturating_mul(4).saturating_add(65_536)
}

/// Concurrent output, diagnostics, and bounded native watchdog scratch space.
pub(crate) fn capture_memory(cap: u64) -> u64 {
    // A realloc may temporarily retain both old and replacement buffers.
    cap.saturating_mul(2)
        .saturating_add(DIAGNOSTIC_CAP)
        .saturating_add(1024 * 1024)
}

/// Run the real mature parser with result allocation already covered by `credits.output`.
pub(crate) fn parse_in_worker(
    mode: &str,
    bytes: &[u8],
    max_bytes: usize,
    credits: &mut Credits,
    funding: &Funding<'_>,
) -> Result<Extraction> {
    let mut command = Command::new(worker::parser_worker_path()?);
    command.args(["parse", mode, &max_bytes.to_string()]);
    let mut capture = worker::run(
        command,
        Some(bytes),
        transport_cap(max_bytes),
        Duration::from_secs(120),
        credits,
        funding,
        None,
    )?;
    // Do not overlap geometric capture slack with the decoded result/scratch.
    // Both old and replacement buffers remain prepaid during this compaction.
    capture.bytes.shrink_to_fit();
    // The transport guard stays in credits until this actual capture buffer is destroyed.
    serde_json::from_slice(&capture.bytes).context("Parser worker returned invalid extraction")
}

/// Run the real EPUB parser. The output allowance covers the returned vector capacity, not only its length.
pub(crate) fn epub_text_in_worker(
    bytes: &[u8],
    max_bytes: usize,
    credits: &mut Credits,
    funding: &Funding<'_>,
) -> Result<String> {
    let mut command = Command::new(worker::parser_worker_path()?);
    command.args(["epub-text", &max_bytes.to_string()]);
    let capture = worker::run(
        command,
        Some(bytes),
        transport_cap(max_bytes),
        Duration::from_secs(120),
        credits,
        funding,
        None,
    )?;
    String::from_utf8(capture.bytes).context("EPUB worker returned non-UTF-8 text")
}

/// Parse untrusted transcript JSON in the watched process, not an unbounded parent serde Value tree.
pub(crate) fn transcript_in_worker(
    bytes: &[u8],
    max_bytes: usize,
    credits: &mut Credits,
    funding: &Funding<'_>,
) -> Result<Vec<crate::media::TranscriptSegment>> {
    let mut command = Command::new(worker::parser_worker_path()?);
    command.arg("transcript");
    let capture = worker::run(
        command,
        Some(bytes),
        transport_cap(max_bytes),
        Duration::from_secs(120),
        credits,
        funding,
        None,
    )?;
    serde_json::from_slice(&capture.bytes).context("Transcript worker returned invalid segments")
}

/// Run actual EPUB generation under the prepaid kernel extent ceiling for its one staging inode.
pub(crate) fn export_in_worker(
    root: &Path,
    output: &Path,
    title: &str,
    include_media: bool,
    credits: &mut Credits,
    funding: &Funding<'_>,
    file_limit: u64,
) -> Result<usize> {
    let mut command = Command::new(worker::parser_worker_path()?);
    command.args([
        "export-epub",
        &root.display().to_string(),
        &output.display().to_string(),
        title,
        if include_media { "1" } else { "0" },
    ]);
    let capture = worker::run(
        command,
        None,
        1024,
        Duration::from_secs(300),
        credits,
        funding,
        Some(file_limit),
    )?;
    std::str::from_utf8(&capture.bytes)?
        .trim()
        .parse()
        .context("EPUB export worker returned a non-numeric count")
}
