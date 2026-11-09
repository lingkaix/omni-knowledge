//! Source-action preflight tests; no test changes the host pasteboard, Finder, or Trash.

use omni_knowledge as fixture_library;
#[path = "support/retained.rs"]
mod retained;
use retained::legacy_init;

use anyhow::Result;
use fs2::FileExt;
use omni_knowledge::{CONFIG_FILE, KnowledgeBase, file_actions};
use std::fs::{self, OpenOptions};

/// Find a declared path-keyed sidecar using the storage contract's SHA-256 identity.
fn sidecar(kb: &KnowledgeBase, relative: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    kb.root.join(format!(
        ".omni/metadata/{}.json",
        hex::encode(Sha256::digest(relative.as_bytes()))
    ))
}

/// Resolve an owned Unicode filename without changing its bytes or sidecar metadata.
#[test]
fn source_paths_preserve_owned_files() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Actions")?;
    let relative = "raw/研究 notes #1.bin";
    fs::write(kb.root.join(relative), [0, 1, 255])?;
    kb.set_metadata(relative, &["research".into()], Some("Keep this note"))?;
    let path = file_actions::source_path(&kb.root, relative)?;
    assert_eq!(path, kb.root.join(relative));
    assert_eq!(fs::read(&path)?, [0, 1, 255]);
    assert_eq!(kb.metadata(relative)?.description, "Keep this note");
    Ok(())
}

/// A removed child marker must not redirect explicit source actions to a same-named parent file.
#[test]
fn explicit_roots_refuse_ancestor_rebinding_after_marker_removal() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let parent = legacy_init(directory.path(), "Parent")?;
    let child = legacy_init(&parent.root.join("child"), "Child")?;
    let relative = "raw/source.txt";
    fs::write(parent.root.join(relative), "parent source")?;
    fs::write(child.root.join(relative), "child source")?;
    parent.set_metadata(relative, &["parent".into()], Some("parent notes"))?;
    child.set_metadata(relative, &["child".into()], Some("child notes"))?;
    fs::create_dir(parent.root.join("destination"))?;
    fs::create_dir(child.root.join("destination"))?;
    let external_directory = tempfile::tempdir()?;
    let external = external_directory.path().join("external.bin");
    fs::write(&external, [0, 1, 255])?;
    let parent_marker = fs::read(parent.root.join(CONFIG_FILE))?;
    let parent_metadata = fs::read(sidecar(&parent, relative))?;
    let child_metadata = fs::read(sidecar(&child, relative))?;
    let metadata_count = fs::read_dir(parent.root.join(".omni/metadata"))?.count();
    fs::remove_file(child.root.join(CONFIG_FILE))?;
    assert_eq!(KnowledgeBase::open(&child.root)?.root, parent.root);
    assert!(file_actions::source_path(&child.root, relative).is_err());
    assert!(file_actions::directory_children(&child.root, "").is_err());
    assert!(file_actions::directory_children(&child.root, "raw").is_err());
    assert!(file_actions::copy_into(&child.root, &external, "destination").is_err());
    assert!(file_actions::move_within(&child.root, relative, "destination").is_err());
    assert_eq!(
        fs::read_to_string(parent.root.join(relative))?,
        "parent source"
    );
    assert_eq!(
        fs::read_to_string(child.root.join(relative))?,
        "child source"
    );
    assert!(
        fs::read_dir(parent.root.join("destination"))?
            .next()
            .is_none()
    );
    assert!(
        fs::read_dir(child.root.join("destination"))?
            .next()
            .is_none()
    );
    assert_eq!(fs::read(&external)?, [0, 1, 255]);
    assert_eq!(fs::read(parent.root.join(CONFIG_FILE))?, parent_marker);
    assert_eq!(fs::read(sidecar(&parent, relative))?, parent_metadata);
    assert_eq!(fs::read(sidecar(&child, relative))?, child_metadata);
    assert_eq!(
        fs::read_dir(parent.root.join(".omni/metadata"))?.count(),
        metadata_count
    );
    Ok(())
}

/// Refuse source actions on traversal, markers, internal files, folders, and child-owned sources.
#[test]
fn source_actions_reject_unowned_targets() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Actions")?;
    fs::create_dir_all(kb.root.join(".git"))?;
    fs::write(kb.root.join(".git/config"), "private")?;
    fs::create_dir_all(kb.root.join("raw/.omni"))?;
    fs::write(kb.root.join("raw/.omni/hidden"), "private")?;
    let child = legacy_init(&kb.root.join("child"), "Child")?;
    fs::write(child.root.join("raw/source.txt"), "child")?;
    for relative in [
        "",
        "../outside",
        "/tmp/outside",
        CONFIG_FILE,
        ".git/config",
        ".omni/write.lock",
        "raw/.omni/hidden",
        "raw",
        "child/raw/source.txt",
        "child/omni-knowledge.json",
        "raw/missing.txt",
    ] {
        assert!(
            file_actions::source_path(&kb.root, relative).is_err(),
            "{relative}"
        );
    }
    assert!(kb.root.join(CONFIG_FILE).is_file());
    assert_eq!(
        fs::read_to_string(child.root.join("raw/source.txt"))?,
        "child"
    );
    Ok(())
}

/// Reject a symlink even when its target is an ordinary file inside the selected root.
#[cfg(unix)]
#[test]
fn source_actions_reject_symlink_components() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Actions")?;
    fs::write(kb.root.join("raw/source.txt"), "safe")?;
    std::os::unix::fs::symlink(kb.root.join("raw/source.txt"), kb.root.join("raw/link.txt"))?;
    std::os::unix::fs::symlink(kb.root.join("raw"), kb.root.join("alias"))?;
    assert!(file_actions::source_path(&kb.root, "raw/link.txt").is_err());
    assert!(file_actions::source_path(&kb.root, "alias/source.txt").is_err());
    assert_eq!(fs::read_to_string(kb.root.join("raw/source.txt"))?, "safe");
    Ok(())
}

/// A competing writer refuses transfers before modifying either private fixture location.
#[test]
fn transfers_refuse_an_active_writer_without_touching_source() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Actions")?;
    fs::write(kb.root.join("raw/source.txt"), "safe")?;
    kb.set_metadata("raw/source.txt", &["preserve".into()], Some("notes"))?;
    fs::create_dir(kb.root.join("destination"))?;
    let gate = OpenOptions::new()
        .read(true)
        .write(true)
        .open(kb.root.join(".omni/write.lock"))?;
    gate.lock_exclusive()?;
    let error = file_actions::copy_into(&kb.root, &kb.root.join("raw/source.txt"), "destination")
        .unwrap_err();
    assert!(format!("{error:#}").contains("another library writer"));
    let error = file_actions::move_within(&kb.root, "raw/source.txt", "destination").unwrap_err();
    assert!(format!("{error:#}").contains("another library writer"));
    assert_eq!(fs::read_to_string(kb.root.join("raw/source.txt"))?, "safe");
    assert_eq!(kb.metadata("raw/source.txt")?.description, "notes");
    Ok(())
}

/// Live browsing includes empty folders and more than the search limit, without entering child roots.
#[test]
fn live_directory_browsing_is_complete_and_scoped() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Browse")?;
    fs::create_dir(kb.root.join("empty"))?;
    fs::create_dir(kb.root.join(".git"))?;
    let child = legacy_init(&kb.root.join("child"), "Child")?;
    let original_raw_children = fs::read_dir(kb.root.join("raw"))?.count();
    for index in 0..1100 {
        fs::write(
            kb.root.join(format!("raw/item-{index:04}.bin")),
            [index as u8],
        )?;
    }
    fs::write(kb.root.join("raw/中文.html"), "content")?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(kb.root.join("raw/中文.html"), kb.root.join("raw/link"))?;
    let root = file_actions::directory_children(&kb.root, "")?;
    assert!(
        root.iter()
            .any(|entry| entry.relative == "empty" && entry.is_directory && !entry.is_nested_root)
    );
    assert!(
        root.iter()
            .any(|entry| entry.relative == "child" && entry.is_nested_root)
    );
    assert!(
        !root
            .iter()
            .any(|entry| matches!(entry.relative.as_str(), ".omni" | ".git" | CONFIG_FILE))
    );
    assert!(file_actions::directory_children(&kb.root, "empty")?.is_empty());
    assert_eq!(
        file_actions::directory_children(&kb.root, "raw")?.len(),
        original_raw_children + 1101
    );
    assert!(file_actions::directory_children(&kb.root, "child").is_err());
    assert!(file_actions::directory_children(&kb.root, "child/raw").is_err());
    assert!(child.root.join(CONFIG_FILE).is_file());
    Ok(())
}

/// Internal copies keep original bytes and notes while creating the new editable identity.
#[test]
fn copy_preserves_source_and_authored_metadata() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Copy")?;
    fs::write(kb.root.join("raw/资料.bin"), [0, 7, 255])?;
    kb.set_metadata("raw/资料.bin", &["研究".into()], Some("authored note"))?;
    fs::create_dir(kb.root.join("copies"))?;
    kb.index(&Default::default())?;
    let pointer = fs::read(kb.root.join(".omni/current.json"))?;
    let original_metadata = fs::read(sidecar(&kb, "raw/资料.bin"))?;
    let receipt = file_actions::copy_into(&kb.root, &kb.root.join("raw/资料.bin"), "copies")?;
    assert_eq!(receipt.relative, "copies/资料.bin");
    assert_eq!(fs::read(&receipt.source)?, [0, 7, 255]);
    assert_eq!(fs::read(&receipt.destination)?, [0, 7, 255]);
    let metadata = kb.metadata(&receipt.relative)?;
    assert_eq!(metadata.tags, ["研究"]);
    assert_eq!(metadata.description, "authored note");
    assert_eq!(fs::read(sidecar(&kb, "raw/资料.bin"))?, original_metadata);
    assert_eq!(fs::read(kb.root.join(".omni/current.json"))?, pointer);
    Ok(())
}

/// External files copy into the selected folder without inheriting another library's authored notes.
#[test]
fn external_copy_has_explicit_empty_metadata() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Copy")?;
    let outside = tempfile::tempdir()?;
    let other = legacy_init(outside.path(), "Other")?;
    let source = other.root.join("raw/other.bin");
    fs::write(&source, [4, 3, 2, 1])?;
    other.set_metadata(
        "raw/other.bin",
        &["private-to-other-root".into()],
        Some("other note"),
    )?;
    let receipt = file_actions::copy_into(&kb.root, &source, "")?;
    assert_eq!(receipt.relative, "other.bin");
    assert_eq!(fs::read(&source)?, [4, 3, 2, 1]);
    assert_eq!(fs::read(&receipt.destination)?, [4, 3, 2, 1]);
    let metadata = kb.metadata(&receipt.relative)?;
    assert!(metadata.tags.is_empty());
    assert!(metadata.description.is_empty());
    assert_eq!(other.metadata("raw/other.bin")?.description, "other note");
    Ok(())
}

/// Existing files and orphan sidecars are collisions that preserve both parties' authored state.
#[test]
fn transfers_refuse_file_and_metadata_collisions() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Collision")?;
    fs::write(kb.root.join("raw/source.txt"), "source")?;
    kb.set_metadata("raw/source.txt", &["original".into()], Some("keep"))?;
    fs::create_dir(kb.root.join("destination"))?;
    fs::write(kb.root.join("destination/source.txt"), "unrelated")?;
    kb.set_metadata(
        "destination/source.txt",
        &["destination".into()],
        Some("unrelated notes"),
    )?;
    let original = fs::read(sidecar(&kb, "raw/source.txt"))?;
    let unrelated = fs::read(sidecar(&kb, "destination/source.txt"))?;
    assert!(
        file_actions::copy_into(&kb.root, &kb.root.join("raw/source.txt"), "destination").is_err()
    );
    assert!(file_actions::move_within(&kb.root, "raw/source.txt", "destination").is_err());
    assert_eq!(
        fs::read_to_string(kb.root.join("destination/source.txt"))?,
        "unrelated"
    );
    fs::remove_file(kb.root.join("destination/source.txt"))?;
    assert!(
        file_actions::copy_into(&kb.root, &kb.root.join("raw/source.txt"), "destination").is_err()
    );
    assert!(file_actions::move_within(&kb.root, "raw/source.txt", "destination").is_err());
    assert!(!kb.root.join("destination/source.txt").exists());
    assert_eq!(
        fs::read_to_string(kb.root.join("raw/source.txt"))?,
        "source"
    );
    assert_eq!(fs::read(sidecar(&kb, "raw/source.txt"))?, original);
    assert_eq!(fs::read(sidecar(&kb, "destination/source.txt"))?, unrelated);
    Ok(())
}

/// Destination traversal, internals, child roots, and directories masquerading as sources have no effects.
#[test]
fn transfers_refuse_unowned_sources_and_destinations() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Scope")?;
    fs::write(kb.root.join("raw/source.txt"), "source")?;
    let child = legacy_init(&kb.root.join("child"), "Child")?;
    fs::write(child.root.join("raw/child.txt"), "child")?;
    for target in [
        "../escape",
        "/tmp",
        ".omni",
        ".git",
        "missing",
        "child",
        "child/raw",
        CONFIG_FILE,
    ] {
        assert!(
            file_actions::copy_into(&kb.root, &kb.root.join("raw/source.txt"), target).is_err(),
            "{target}"
        );
        assert!(
            file_actions::move_within(&kb.root, "raw/source.txt", target).is_err(),
            "{target}"
        );
    }
    assert!(file_actions::copy_into(&kb.root, &child.root.join("raw/child.txt"), "raw").is_err());
    assert!(file_actions::copy_into(&kb.root, &kb.root.join(CONFIG_FILE), "raw").is_err());
    assert!(file_actions::copy_into(&kb.root, &kb.root.join("raw"), "").is_err());
    assert_eq!(
        fs::read_to_string(kb.root.join("raw/source.txt"))?,
        "source"
    );
    assert_eq!(
        fs::read_to_string(child.root.join("raw/child.txt"))?,
        "child"
    );
    Ok(())
}

/// A dangling nested marker still marks a visible boundary and prevents source effects below it.
#[cfg(unix)]
#[test]
fn malformed_nested_marker_still_blocks_parent_actions() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Boundary")?;
    fs::create_dir(kb.root.join("marked"))?;
    fs::write(kb.root.join("marked/source.txt"), "preserved")?;
    fs::create_dir(kb.root.join("marked/subfolder"))?;
    fs::write(kb.root.join("raw/owned.txt"), "owned")?;
    std::os::unix::fs::symlink(
        kb.root.join("missing-config"),
        kb.root.join("marked").join(CONFIG_FILE),
    )?;
    let root = file_actions::directory_children(&kb.root, "")?;
    assert!(
        root.iter()
            .any(|entry| entry.relative == "marked" && entry.is_nested_root)
    );
    assert!(file_actions::directory_children(&kb.root, "marked").is_err());
    assert!(file_actions::directory_children(&kb.root, "marked/subfolder").is_err());
    assert!(file_actions::source_path(&kb.root, "marked/source.txt").is_err());
    assert!(file_actions::copy_into(&kb.root, &kb.root.join("marked/source.txt"), "raw").is_err());
    assert!(
        file_actions::copy_into(&kb.root, &kb.root.join("raw/owned.txt"), "marked/subfolder")
            .is_err()
    );
    assert!(file_actions::move_within(&kb.root, "raw/owned.txt", "marked/subfolder").is_err());
    assert_eq!(fs::read_to_string(kb.root.join("raw/owned.txt"))?, "owned");
    assert!(!kb.root.join("marked/subfolder/owned.txt").exists());
    assert_eq!(
        fs::read_to_string(kb.root.join("marked/source.txt"))?,
        "preserved"
    );
    Ok(())
}

/// Kernel-exclusive movement preserves bytes, migrates editable fields, and retains the old sidecar.
#[cfg(target_os = "macos")]
#[test]
fn move_migrates_metadata_without_reindexing() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let kb = legacy_init(directory.path(), "Move")?;
    fs::write(kb.root.join("raw/source.txt"), "source")?;
    kb.set_metadata("raw/source.txt", &["keep".into()], Some("authored notes"))?;
    fs::create_dir(kb.root.join("destination"))?;
    kb.index(&Default::default())?;
    let pointer = fs::read(kb.root.join(".omni/current.json"))?;
    let old = fs::read(sidecar(&kb, "raw/source.txt"))?;
    let receipt = file_actions::move_within(&kb.root, "raw/./source.txt", "destination")?;
    assert_eq!(receipt.relative, "destination/source.txt");
    assert!(!receipt.source.exists());
    assert_eq!(fs::read_to_string(&receipt.destination)?, "source");
    let metadata = kb.metadata(&receipt.relative)?;
    assert_eq!(metadata.tags, ["keep"]);
    assert_eq!(metadata.description, "authored notes");
    assert_eq!(fs::read(sidecar(&kb, "raw/source.txt"))?, old);
    assert_eq!(fs::read(kb.root.join(".omni/current.json"))?, pointer);
    Ok(())
}
