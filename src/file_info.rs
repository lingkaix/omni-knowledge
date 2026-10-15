//! Live selected-file attributes and authored metadata, independent of derived search state.
//! This module never reads source contents, an index pointer, or a snapshot manifest.

use crate::file_actions;
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Current attributes and editable metadata for one contained regular source.
/// Indexed status and SHA-256 are deliberately absent; these facts do not establish index membership or freshness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    /// Canonical exact knowledge root whose marker was validated for this observation.
    pub root: PathBuf,
    /// Canonical root-relative UTF-8 identity, also used to select the authored sidecar.
    pub relative: String,
    /// Canonical absolute path of the validated owned source.
    pub path: PathBuf,
    /// Extension-based MIME guess; unrecognized types use application/octet-stream.
    pub media_type: String,
    /// Live source size in bytes, without reading its contents.
    pub size: u64,
    /// Live modification timestamp in Unix milliseconds, absent when unavailable or out of range.
    pub modified_ms: Option<u64>,
    /// Editable exact tags from the authored sidecar; front matter and indexed tags are not merged.
    pub tags: Vec<String>,
    /// Editable description from the authored sidecar, empty when no sidecar exists.
    pub description: String,
}

/// Read one source's current filesystem attributes and editable metadata without consulting an index.
/// `root` must remain the exact opened library and `relative` must identify its owned regular source.
/// Derives the normalized metadata identity only after source validation; aliases such as `raw/./file` retain their original sidecar.
/// Returns an error for missing roots or sources, protected paths, symlinks, child ownership, unreadable metadata, or invalid sidecars.
/// Performs no writes, content extraction, source hashing, lock waits, or index publication; callers should run filesystem work off the UI thread.
pub fn source_info(root: &Path, relative: &str) -> Result<SourceInfo> {
    let kb = file_actions::open_exact_root(root)?;
    let path = file_actions::source_path(&kb.root, relative)
        .with_context(|| format!("Cannot resolve selected source {relative}"))?;
    let relative = path
        .strip_prefix(&kb.root)
        .context("Selected source is outside its exact knowledge root")?
        .to_str()
        .context("Selected source identity is not UTF-8")?
        .to_owned();
    let stat = fs::symlink_metadata(&path)
        .with_context(|| format!("Cannot inspect selected source {}", path.display()))?;
    ensure!(
        stat.is_file() && !stat.file_type().is_symlink(),
        "Selected source changed to a nonregular file"
    );
    let metadata = kb
        .metadata(&relative)
        .with_context(|| format!("Cannot read editable metadata for {relative}"))?;
    let modified_ms = stat
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| duration.as_millis().try_into().ok());
    Ok(SourceInfo {
        root: kb.root,
        relative,
        media_type: mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string(),
        path,
        size: stat.len(),
        modified_ms,
        tags: metadata.tags,
        description: metadata.description,
    })
}
