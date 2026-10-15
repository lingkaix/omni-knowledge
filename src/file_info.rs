//! Live selected-file attributes and authored metadata, independent of derived search state.
//! This module never reads source contents, an index pointer, or a snapshot manifest.

use crate::file_actions;
use anyhow::{Context, Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// Live filesystem creation timestamp in Unix milliseconds, absent when unavailable or out of range.
    pub created_ms: Option<u64>,
    /// Live modification timestamp in Unix milliseconds, absent when unavailable or out of range.
    pub modified_ms: Option<u64>,
    /// Validated absolute authored-sidecar location for the canonical source identity; the record can be absent.
    pub metadata_path: PathBuf,
    /// True when an existing regular authored sidecar was successfully read during this observation.
    pub metadata_saved: bool,
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
    let metadata_path = kb.metadata_path(&relative)?;
    let metadata_saved = match fs::symlink_metadata(&metadata_path) {
        Ok(sidecar) => {
            ensure!(
                sidecar.is_file() && !sidecar.file_type().is_symlink(),
                "Editable metadata changed to a nonregular file"
            );
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(error).context("Cannot inspect selected source's editable metadata");
        }
    };
    Ok(SourceInfo {
        root: kb.root,
        relative,
        media_type: mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string(),
        path,
        size: stat.len(),
        created_ms: unix_millis(stat.created()),
        modified_ms: unix_millis(stat.modified()),
        metadata_path,
        metadata_saved,
        tags: metadata.tags,
        description: metadata.description,
    })
}

/// Convert a filesystem date to Unix milliseconds; unavailable, pre-epoch, and overflowing dates remain absent.
fn unix_millis(time: std::io::Result<SystemTime>) -> Option<u64> {
    time.ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| duration.as_millis().try_into().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CONFIG_FILE, KnowledgeBase};
    use std::time::Duration;

    /// Read canonical dates and the exact authored sidecar before and after saving without index effects.
    #[test]
    fn live_dates_and_sidecar_location_preserve_authored_identity() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let kb = KnowledgeBase::init(directory.path(), "Inspector facts")?;
        let relative = "raw/资料 + #1.md";
        let source = kb.root.join(relative);
        fs::write(&source, "# Original source\n")?;
        let expected_sidecar = kb.metadata_path(relative)?;
        let marker = fs::read(kb.root.join(CONFIG_FILE))?;
        let read_gate = fs::read(kb.root.join(".omni/read.lock")).ok();

        let empty = source_info(&kb.root, "raw/./资料 + #1.md")?;
        assert_eq!(empty.relative, relative);
        assert_eq!(empty.metadata_path, expected_sidecar);
        assert!(!empty.metadata_path.exists());
        assert!(!empty.metadata_saved);
        assert!(empty.tags.is_empty());
        assert!(empty.description.is_empty());
        assert!(!kb.root.join(".omni/current.json").exists());
        assert_eq!(fs::read(kb.root.join(".omni/read.lock")).ok(), read_gate);

        kb.set_metadata(
            relative,
            &["research".into()],
            Some("Saved inspector notes"),
        )?;
        let authored = fs::read(&expected_sidecar)?;
        let read_gate = fs::read(kb.root.join(".omni/read.lock")).ok();
        let info = source_info(&kb.root, "raw/./资料 + #1.md")?;
        let stat = fs::metadata(&source)?;
        let expected_time = |time: std::io::Result<std::time::SystemTime>| {
            time.ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        };
        assert_eq!(info.path, source);
        assert_eq!(info.metadata_path, expected_sidecar);
        assert!(info.metadata_saved);
        assert_eq!(info.created_ms, expected_time(stat.created()));
        assert_eq!(info.modified_ms, expected_time(stat.modified()));
        #[cfg(target_os = "macos")]
        assert!(info.created_ms.is_some());
        assert_eq!(info.tags, ["research"]);
        assert_eq!(info.description, "Saved inspector notes");
        assert_eq!(fs::read(info.metadata_path)?, authored);
        assert_eq!(fs::read(kb.root.join(CONFIG_FILE))?, marker);
        assert_eq!(fs::read_to_string(source)?, "# Original source\n");
        assert!(!kb.root.join(".omni/current.json").exists());
        assert_eq!(fs::read(kb.root.join(".omni/read.lock")).ok(), read_gate);
        Ok(())
    }

    /// Keep unavailable and pre-epoch filesystem dates absent instead of inventing a usable timestamp.
    #[test]
    fn filesystem_dates_require_a_representable_unix_millisecond_value() {
        assert_eq!(unix_millis(Ok(UNIX_EPOCH)), Some(0));
        assert_eq!(
            unix_millis(Ok(UNIX_EPOCH + Duration::from_millis(1234))),
            Some(1234)
        );
        assert_eq!(unix_millis(Ok(UNIX_EPOCH - Duration::from_millis(1))), None);
        assert_eq!(
            unix_millis(Err(std::io::Error::from(std::io::ErrorKind::Unsupported))),
            None
        );
    }
}
