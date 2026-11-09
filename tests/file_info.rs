//! Selected-file facts regressions; fixtures never invoke native viewers or read a full search snapshot.

use omni_knowledge as fixture_library;
#[path = "support/retained.rs"]
mod retained;
use retained::legacy_init;

use anyhow::Result;
use fs2::FileExt;
use omni_knowledge::{CONFIG_FILE, KnowledgeBase, file_info};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// Locate one authored sidecar without asking the index for source records.
fn sidecar(kb: &KnowledgeBase, relative: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    kb.root.join(format!(
        ".omni/metadata/{}.json",
        hex::encode(Sha256::digest(relative.as_bytes()))
    ))
}

/// Return live facts and only editable metadata for an unindexed Unicode file without changing it.
#[test]
fn live_facts_do_not_require_index_or_parse_source() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "File facts")?;
    let relative = "raw/资料 + #1.jpg";
    let source = kb.root.join(relative);
    let bytes = [0, 1, 255];
    fs::write(&source, bytes)?;
    kb.set_metadata(
        relative,
        &["photo".into(), "研究".into()],
        Some("Original notes"),
    )?;
    let marker = fs::read(kb.root.join(CONFIG_FILE))?;
    let authored = fs::read(sidecar(&kb, relative))?;
    let reader_gate = fs::read(kb.root.join(".omni/read.lock")).ok();
    let info = file_info::source_info(&kb.root, relative)?;
    assert_eq!(info.root, kb.root);
    assert_eq!(info.relative, relative);
    assert_eq!(info.path, source);
    assert_eq!(info.media_type, "image/jpeg");
    assert_eq!(info.size, 3);
    assert_eq!(
        info.modified_ms,
        Some(
            fs::metadata(&source)?
                .modified()?
                .duration_since(UNIX_EPOCH)?
                .as_millis() as u64
        )
    );
    assert_eq!(info.tags, ["photo", "研究"]);
    assert_eq!(info.description, "Original notes");
    assert_eq!(fs::read(&source)?, bytes);
    assert_eq!(fs::read(kb.root.join(CONFIG_FILE))?, marker);
    assert_eq!(fs::read(sidecar(&kb, relative))?, authored);
    assert!(!kb.root.join(".omni/current.json").exists());
    assert_eq!(fs::read(kb.root.join(".omni/read.lock")).ok(), reader_gate);
    Ok(())
}

/// Large invalid vector state and a busy index writer cannot delay or invalidate selected-file facts.
#[test]
fn live_facts_ignore_derived_state_and_active_writer() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "File facts")?;
    let relative = "raw/source.bin";
    fs::write(kb.root.join(relative), [0, 255])?;
    fs::write(kb.root.join(".omni/current.json"), "not a JSON pointer")?;
    let generation = kb.root.join(".omni/indexes/gen-poison");
    fs::create_dir_all(&generation)?;
    let manifest_path = generation.join("manifest.json");
    let mut manifest = File::create(&manifest_path)?;
    manifest.write_all(br#"{"version":999,"records":[],"vectors":""#)?;
    let vector_placeholder = [b'x'; 8192];
    for _ in 0..64 {
        manifest.write_all(&vector_placeholder)?;
    }
    manifest.write_all(br#""}"#)?;
    manifest.sync_all()?;
    let manifest_length = fs::metadata(&manifest_path)?.len();
    let gate = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(kb.root.join(".omni/write.lock"))?;
    gate.lock_exclusive()?;
    assert!(kb.list().is_err());
    let info = file_info::source_info(&kb.root, relative)?;
    assert_eq!(info.size, 2);
    assert!(info.tags.is_empty());
    assert!(info.description.is_empty());
    assert_eq!(fs::metadata(manifest_path)?.len(), manifest_length);
    assert_eq!(
        fs::read(kb.root.join(".omni/current.json"))?,
        b"not a JSON pointer"
    );
    assert!(!kb.root.join(".omni/read.lock").exists());
    Ok(())
}

/// Fresh source stats and sidecars replace previous facts, and aliases keep the validated canonical identity.
#[test]
fn source_updates_and_aliases_use_current_authored_metadata() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "File facts")?;
    let relative = "raw/source.md";
    fs::write(
        kb.root.join(relative),
        "---\ntags: [front-matter]\n---\n# Source",
    )?;
    kb.set_metadata(relative, &["editable".into()], Some("Before"))?;
    let before = file_info::source_info(&kb.root, relative)?;
    assert_eq!(before.tags, ["editable"]);
    fs::write(kb.root.join(relative), "Changed live file")?;
    kb.set_metadata(relative, &["changed".into()], Some("After"))?;
    let after = file_info::source_info(&kb.root, "raw/./source.md")?;
    assert_eq!(after.relative, relative);
    assert_eq!(after.path, kb.root.join(relative));
    assert_eq!(after.size, 17);
    assert_ne!(after.size, before.size);
    assert_eq!(after.tags, ["changed"]);
    assert_eq!(after.description, "After");
    Ok(())
}

/// Reject non-source paths, nested owners, symlinks, and removed exact-root markers.
#[test]
fn selected_facts_enforce_source_and_root_ownership() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let parent = legacy_init(directory.path(), "Parent")?;
    let child = legacy_init(&parent.root.join("child"), "Child")?;
    fs::write(parent.root.join("raw/source.txt"), "parent")?;
    fs::write(child.root.join("raw/source.txt"), "child")?;
    fs::create_dir(parent.root.join(".git"))?;
    fs::write(parent.root.join(".git/private"), "private")?;
    for relative in [
        "../outside",
        CONFIG_FILE,
        ".git/private",
        ".omni/write.lock",
        "raw",
        "child/raw/source.txt",
        "raw/missing",
    ] {
        assert!(
            file_info::source_info(&parent.root, relative).is_err(),
            "{relative}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            parent.root.join("raw/source.txt"),
            parent.root.join("alias"),
        )?;
        assert!(file_info::source_info(&parent.root, "alias").is_err());
    }
    fs::remove_file(child.root.join(CONFIG_FILE))?;
    assert!(file_info::source_info(&child.root, "raw/source.txt").is_err());
    assert_eq!(
        fs::read_to_string(parent.root.join("raw/source.txt"))?,
        "parent"
    );
    assert_eq!(
        fs::read_to_string(child.root.join("raw/source.txt"))?,
        "child"
    );
    Ok(())
}

/// Report corrupt authored metadata explicitly without substituting indexed or empty notes.
#[test]
fn malformed_authored_metadata_has_a_contextual_error() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "File facts")?;
    let relative = "raw/source.bin";
    fs::write(kb.root.join(relative), [0, 1])?;
    let path = sidecar(&kb, relative);
    fs::write(
        &path,
        br#"{"version":2,"path":"raw/source.bin","tags":["keep"],"description":"notes"}"#,
    )?;
    let original = fs::read(&path)?;
    let error = file_info::source_info(&kb.root, relative).unwrap_err();
    assert!(format!("{error:#}").contains("editable metadata"));
    assert_eq!(fs::read(path)?, original);
    assert_eq!(fs::read(kb.root.join(relative))?, [0, 1]);
    Ok(())
}
