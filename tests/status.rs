//! Acceptance checks for read-only folder classification, freshness, and local service observations.

use omni_knowledge as fixture_library;
#[path = "support/retained.rs"]
mod retained;
use retained::legacy_init;

use fs2::FileExt;
use omni_knowledge::embedding::EmbeddingConfig;
use omni_knowledge::status::{FolderState, ServiceState, StatusProbe};
use omni_knowledge::{CONFIG_FILE, IndexOptions, KnowledgeBase, SearchOptions};
use std::fs::{self, File, FileTimes};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Creates an explicit v1 status-component fixture without a provider connection.
fn library(indexed: bool) -> (tempfile::TempDir, KnowledgeBase) {
    let directory = tempfile::tempdir().unwrap();
    let mut kb = legacy_init(directory.path(), "Status fixture").unwrap();
    kb.config.embedding = None;
    fs::write(
        kb.root.join(CONFIG_FILE),
        serde_json::to_vec(&kb.config).unwrap(),
    )
    .unwrap();
    if indexed {
        kb.index(&IndexOptions::default()).unwrap();
    }
    (directory, kb)
}

/// Returns the selected generation for direct fault injection into disposable derived state.
fn generation(kb: &KnowledgeBase) -> PathBuf {
    let name: String =
        serde_json::from_slice(&fs::read(kb.root.join(".omni/current.json")).unwrap()).unwrap();
    kb.root.join(".omni/indexes").join(name)
}

/// Resolve only the actual tuple published by this disposable fixture, not a legacy fake index.
fn kernel_reference(kb: &KnowledgeBase) -> serde_json::Value {
    serde_json::from_slice(&fs::read(generation(kb).join("tantivy/kernel-reference.json")).unwrap())
        .unwrap()
}
/// Locate fixture derived data only through the actual account-held cache directory identity.
fn native_generation(kb: &KnowledgeBase) -> PathBuf {
    use sha2::{Digest, Sha256};
    let reference = kernel_reference(kb);
    let selected = knowledge_core::admission::Account::current()
        .unwrap()
        .generated_directory()
        .unwrap()
        .metadata()
        .unwrap();
    let suffix = format!(
        "omni-knowledge-derived-{}",
        rustix::process::geteuid().as_raw()
    );
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("OMNI_CACHE_DIR") {
        candidates.push(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join("Library/Caches").join(&suffix));
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
            candidates.push(PathBuf::from(path).join(&suffix));
        }
        if let Some(home) = std::env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join(".cache").join(&suffix));
        }
    }
    candidates.push(std::env::temp_dir().join(suffix));
    let cache = candidates
        .into_iter()
        .find(|path| {
            fs::metadata(path).is_ok_and(|metadata| {
                (metadata.dev(), metadata.ino()) == (selected.dev(), selected.ino())
            })
        })
        .expect("fixture cache candidate must match the actual account-held directory");
    cache
        .join(reference["corpus"].as_str().unwrap())
        .join("kernel")
        .join(hex::encode(Sha256::digest(b"shard-0")))
        .join(reference["generations"]["shard-0"].as_str().unwrap())
}

/// Independent actual libraries must not compete for one content-only mutable kernel pointer.
#[test]
fn independent_empty_libraries_never_share_the_mutable_kernel_writer() {
    let (_a, first) = library(false);
    let (_b, second) = library(false);
    let (_c, mut copied) = library(false);
    copied.config.library_id = first.config.library_id.clone();
    fs::write(
        copied.root.join(CONFIG_FILE),
        serde_json::to_vec(&copied.config).unwrap(),
    )
    .unwrap();
    for root in [&first.root, &second.root, &copied.root] {
        for path in [
            "index.md",
            "log.md",
            "AGENTS.md",
            ".gitignore",
            "raw/llm-wiki.source.txt",
            "raw/open-knowledge-format.source.txt",
            "raw/OKF-LICENSE.txt",
            "wiki/index.md",
            "wiki/sources/llm-wiki.md",
            "wiki/sources/open-knowledge-format.md",
        ] {
            fs::remove_file(root.join(path)).unwrap();
        }
    }
    first.index(&IndexOptions::default()).unwrap();
    let work = match knowledge_retrieval::Work::reserve(
        knowledge_core::jobs::Reservation {
            memory_bytes: 65536,
            temporary_bytes: 0,
        },
        knowledge_core::admission::Origin::BeforeVerification,
    )
    .unwrap()
    {
        knowledge_core::admission::Decision::Ready(work) => work,
        _ => panic!("fixture lock admission"),
    };
    let generation = native_generation(&first);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(generation.parent().unwrap().join("LOCK"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    // Hold the ACTUAL first kernel's writer FD, not a boolean/mock 'busy' response.
    // The second source tree/marker is independently selected, despite identical empty content.
    let result = second.index(&IndexOptions::default());
    let copied_result = copied.index(&IndexOptions::default());
    drop(lock);
    result.expect("an unrelated native library cannot inherit the first library's active writer");
    copied_result.expect(
        "a separately rebuilt same-lineage home cannot inherit another selected root's writer",
    );
    assert_ne!(
        kernel_reference(&first)["corpus"],
        kernel_reference(&second)["corpus"]
    );
    assert_ne!(
        kernel_reference(&first)["corpus"],
        kernel_reference(&copied)["corpus"]
    );
    drop(work);
}

/// Advances fixture metadata deterministically without sleeping for timestamp precision.
fn touch_after(path: &Path, indexed_at: u64) {
    File::open(path)
        .unwrap()
        .set_times(
            FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_millis(indexed_at + 1000)),
        )
        .unwrap();
}

/// Distinguishes absent, invalid, and uninitialized selections without writing a root marker.
#[test]
fn arbitrary_folder_selection_is_classified_without_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let probe = StatusProbe::default();
    assert_eq!(
        probe.inspect(Path::new("")).folder.state,
        FolderState::NotSelected
    );
    assert_eq!(
        probe
            .inspect(&directory.path().join("missing"))
            .folder
            .state,
        FolderState::Missing
    );
    fs::write(directory.path().join("file.bin"), [0, 255]).unwrap();
    assert_eq!(
        probe
            .inspect(&directory.path().join("file.bin"))
            .folder
            .state,
        FolderState::NotDirectory
    );
    assert_eq!(
        probe.inspect(directory.path()).folder.state,
        FolderState::Uninitialized
    );
    assert!(!directory.path().join(CONFIG_FILE).exists());
    assert!(!directory.path().join(".omni").exists());
}

/// Reports a missing index, then observes a published index without changing its pointer or sources.
#[test]
fn missing_index_and_current_metadata_have_distinct_read_only_states() {
    let (_directory, kb) = library(false);
    let probe = StatusProbe::default();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    assert!(!kb.root.join(".omni/read.lock").exists());
    kb.index(&IndexOptions::default()).unwrap();
    let pointer = fs::read(kb.root.join(".omni/current.json")).unwrap();
    let marker = fs::read(kb.root.join(CONFIG_FILE)).unwrap();
    let source = fs::read(kb.root.join("index.md")).unwrap();
    let reference = kernel_reference(&kb);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(native_generation(&kb).join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["machinery"].as_str().unwrap(),
        knowledge_retrieval::consumer_machinery(reference["consumer"].as_str().unwrap()).unwrap(),
        "the actual written schema/machinery is the shared kernel recipe"
    );
    let report = probe.inspect(&kb.root);
    assert_eq!(
        report.folder.state,
        FolderState::Current,
        "{}",
        report.folder.detail
    );
    assert!(report.folder.detail.contains("hashes were not checked"));
    assert_eq!(report.provider.state, ServiceState::NotConfigured);
    assert_eq!(
        fs::read(kb.root.join(".omni/current.json")).unwrap(),
        pointer
    );
    assert_eq!(fs::read(kb.root.join(CONFIG_FILE)).unwrap(), marker);
    assert_eq!(fs::read(kb.root.join("index.md")).unwrap(), source);
}

/// Detects new, changed, and removed sources and clears the warning after explicit reconciliation.
#[test]
fn source_changes_need_reindexing_until_explicit_reconciliation() {
    let (_directory, kb) = library(false);
    fs::write(kb.root.join("raw/removed.bin"), [1, 2]).unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    fs::remove_file(kb.root.join("raw/removed.bin")).unwrap();
    fs::write(kb.root.join("raw/added.bin"), [0, 255, 3]).unwrap();
    fs::write(
        kb.root.join("index.md"),
        "Modified source with a different length",
    )
    .unwrap();
    let probe = StatusProbe::default();
    let report = probe.inspect(&kb.root);
    assert_eq!(report.folder.state, FolderState::Stale);
    assert_eq!(
        (
            report.folder.added,
            report.folder.modified,
            report.folder.removed
        ),
        (1, 1, 1)
    );
    kb.index(&IndexOptions::default()).unwrap();
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
}

/// Detects external sidecar edits and removals using file and parent-directory stamps.
#[test]
fn authored_metadata_changes_need_reindexing() {
    let (_directory, kb) = library(false);
    kb.set_metadata("index.md", &["initial".into()], Some("Authored notes"))
        .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let probe = StatusProbe::default();
    let before = probe.inspect(&kb.root);
    let sidecar = fs::read_dir(kb.root.join(".omni/metadata"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    touch_after(&sidecar, before.folder.indexed_at.unwrap());
    assert!(probe.inspect(&kb.root).folder.metadata_changed);
    fs::remove_file(sidecar).unwrap();
    touch_after(
        &kb.root.join(".omni/metadata"),
        before.folder.indexed_at.unwrap(),
    );
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Stale);
}

/// Keeps parent freshness independent of child libraries, internal files, and source symlinks.
#[test]
fn child_roots_and_excluded_paths_do_not_dirty_the_parent() {
    let (_directory, parent) = library(false);
    let child = legacy_init(&parent.root.join("child"), "Child").unwrap();
    parent.index(&IndexOptions::default()).unwrap();
    fs::write(child.root.join("new.txt"), "Child-owned change").unwrap();
    fs::create_dir(parent.root.join(".git")).unwrap();
    fs::write(parent.root.join(".git/private-state"), "Excluded").unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), parent.root.join("outside-link")).unwrap();
    let report = StatusProbe::default().inspect(&parent.root.join("wiki"));
    assert_eq!(report.folder.root.as_deref(), Some(parent.root.as_path()));
    assert_eq!(
        report.folder.state,
        FolderState::Current,
        "{}",
        report.folder.detail
    );
}

/// A dangling marker still isolates all descendants from parent freshness and is invalid when selected directly.
#[cfg(unix)]
#[test]
fn dangling_child_boundary_keeps_parent_status_current() {
    let (_directory, parent) = library(false);
    let child = parent.root.join("marked-child");
    fs::create_dir_all(child.join("grandchild")).unwrap();
    fs::write(child.join("grandchild/note.txt"), "child source").unwrap();
    std::os::unix::fs::symlink(
        parent.root.join("missing-child-config"),
        child.join(CONFIG_FILE),
    )
    .unwrap();
    parent.index(&IndexOptions::default()).unwrap();
    let pointer = fs::read(parent.root.join(".omni/current.json")).unwrap();
    let marker = fs::read(parent.root.join(CONFIG_FILE)).unwrap();
    let probe = StatusProbe::default();
    assert_eq!(
        probe.inspect(&parent.root).folder.state,
        FolderState::Current
    );
    fs::write(child.join("grandchild/note.txt"), "changed child source").unwrap();
    fs::write(child.join("grandchild/added.txt"), "new child source").unwrap();
    let status = probe.inspect(&parent.root).folder;
    assert_eq!(status.state, FolderState::Current, "{}", status.detail);
    assert_eq!((status.added, status.modified, status.removed), (0, 0, 0));
    assert_eq!(
        probe.inspect(&child.join("grandchild")).folder.state,
        FolderState::Unavailable
    );
    assert_eq!(
        fs::read(parent.root.join(".omni/current.json")).unwrap(),
        pointer
    );
    assert_eq!(fs::read(parent.root.join(CONFIG_FILE)).unwrap(), marker);
}

/// Already observed source deletions remain stale when a later metadata inspection cannot complete.
#[cfg(unix)]
#[test]
fn known_source_removal_remains_stale_after_metadata_scan_failure() {
    let (_directory, kb) = library(false);
    fs::write(kb.root.join("raw/removed.txt"), "remove this source").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let metadata_root = kb.root.join(".omni/metadata");
    let stamp = fs::metadata(&metadata_root).unwrap().modified().unwrap();
    std::os::unix::fs::symlink(
        kb.root.join("missing-metadata-target"),
        metadata_root.join("bad-link"),
    )
    .unwrap();
    File::open(&metadata_root)
        .unwrap()
        .set_times(FileTimes::new().set_modified(stamp))
        .unwrap();
    fs::remove_file(kb.root.join("raw/removed.txt")).unwrap();
    let report = StatusProbe::default().inspect(&kb.root);
    assert_eq!(report.folder.removed, 1);
    assert!(!report.folder.metadata_changed);
    assert_eq!(report.folder.state, FolderState::Stale);
    assert!(report.folder.detail.contains("incomplete"));
}

/// Rejects broken lexical state even when a previously cached manifest remains unchanged.
#[test]
fn absent_lexical_metadata_is_not_reported_as_current() {
    let (_directory, kb) = library(true);
    let probe = StatusProbe::default();
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
    let metadata = native_generation(&kb).join("index/meta.json");
    fs::remove_file(&metadata).unwrap();
    assert!(
        kb.search(&SearchOptions {
            query: "knowledge".into(),
            ..Default::default()
        })
        .is_err()
    );
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    assert!(
        !metadata.exists(),
        "read-only status must not repair native files"
    );
}

/// Detects missing referenced segment data without allocating source texts or vectors.
#[test]
fn absent_lexical_segment_requires_rebuilding() {
    let (_directory, kb) = library(true);
    let path = fs::read_dir(native_generation(&kb).join("index"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "term")
        })
        .unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(
        StatusProbe::default().inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
}

/// A valid cached source summary cannot hide a changed consuming-build or shard reference.
#[test]
fn native_reference_binding_and_every_named_shard_are_checked() {
    let (_directory, kb) = library(true);
    let probe = StatusProbe::default();
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
    let path = generation(&kb).join("tantivy/kernel-reference.json");
    let original = fs::read(&path).unwrap();
    let mut reference = kernel_reference(&kb);
    reference["consumer"] = serde_json::json!("0".repeat(64));
    fs::write(&path, serde_json::to_vec(&reference).unwrap()).unwrap();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    fs::write(&path, &original).unwrap();
    reference = kernel_reference(&kb);
    reference["generations"]["shard-1"] = serde_json::json!("0".repeat(64));
    fs::write(&path, serde_json::to_vec(&reference).unwrap()).unwrap();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    fs::write(&path, &original).unwrap();
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
    fs::remove_file(&path).unwrap();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    assert!(!path.exists(), "inspection cannot republish a reference");
}

/// Root CURRENT alone cannot certify a native missing/mismatched CURRENT or corrupted manifest.
#[test]
fn native_current_and_manifest_integrity_are_independent_of_summary_cache() {
    let (_directory, kb) = library(true);
    let probe = StatusProbe::default();
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
    let native = native_generation(&kb);
    let current = native.parent().unwrap().join("CURRENT");
    let pointer = fs::read(&current).unwrap();
    fs::remove_file(&current).unwrap();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    assert!(!current.exists());
    fs::write(&current, "0".repeat(64)).unwrap();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    fs::write(&current, pointer).unwrap();
    let manifest = native.join("manifest.json");
    let mut bytes = fs::read(&manifest).unwrap();
    bytes.push(b' '); // Still valid JSON, but NOT the selected immutable generation's digest.
    fs::write(manifest, bytes).unwrap();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
}

/// Native Tantivy payload/schema identity matters; existence of an old or unrelated index does not.
#[test]
fn native_lexical_commit_payload_must_match_its_generation() {
    let (_directory, kb) = library(true);
    let meta = native_generation(&kb).join("index/meta.json");
    let original = fs::read(&meta).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    value["payload"] = serde_json::json!("0".repeat(64));
    fs::write(&meta, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        StatusProbe::default().inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    // Names/types still exist, but swapped stored field addresses would read the wrong evidence.
    value["schema"].as_array_mut().unwrap().swap(0, 1);
    fs::write(&meta, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        StatusProbe::default().inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
}

/// An existing native reader/publication gate is nonblocking and never created or replaced.
#[test]
fn native_publication_gate_is_checked_without_waiting() {
    let (_directory, kb) = library(true);
    let scope = native_generation(&kb).parent().unwrap().to_path_buf();
    let lock = File::options()
        .read(true)
        .write(true)
        .open(scope.join("LOCK"))
        .unwrap();
    FileExt::lock_exclusive(&lock).unwrap();
    let start = Instant::now();
    let report = StatusProbe::default().inspect(&kb.root);
    FileExt::unlock(&lock).unwrap();
    assert_eq!(
        report.folder.state,
        FolderState::Checking,
        "{}",
        report.folder.detail
    );
    assert!(start.elapsed() < Duration::from_secs(1));
}

/// Shared admission pressure means unknown freshness, not broken state or always-current fallback.
#[test]
fn status_memory_contention_is_checking_and_recovers() {
    // This oracle deliberately drains ALL otherwise-free credit. Unrelated parallel tests
    // can release prior loans (or receive real Waiting), invalidating that pressure premise.
    // Keep ONE account in its own instrument process; preserve every assertion below.
    if std::env::var_os("OMNI_STATUS_PRESSURE_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "status_memory_contention_is_checking_and_recovers",
                "--nocapture",
            ])
            .env("OMNI_STATUS_PRESSURE_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated pressure instrument: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use knowledge_core::{
        admission::{Account, Decision, Origin},
        jobs::Reservation,
    };
    let (_directory, kb) = library(true);
    let probe = StatusProbe::default();
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
    let account = Account::current().unwrap();
    let Decision::Ready(mut held) = account
        .reserve(
            Reservation {
                memory_bytes: account.status().unwrap().memory_available,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    else {
        panic!("owned memory pressure fixture");
    };
    let report = probe.inspect(&kb.root);
    held.finish().unwrap();
    assert_eq!(
        report.folder.state,
        FolderState::Checking,
        "{}",
        report.folder.detail
    );
    assert!(report.folder.detail.contains("admission"));
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
}

/// A valid other corpus with the same path names is not the selected generation's source revision.
#[test]
fn other_native_corpus_cannot_certify_selected_snapshot() {
    let (_directory, kb) = library(true);
    let (_other_directory, other) = library(false);
    fs::write(
        other.root.join("index.md"),
        "A different original source revision\n",
    )
    .unwrap();
    other.index(&IndexOptions::default()).unwrap();
    let reference = fs::read(generation(&other).join("tantivy/kernel-reference.json")).unwrap();
    fs::write(
        generation(&kb).join("tantivy/kernel-reference.json"),
        reference,
    )
    .unwrap();
    let report = StatusProbe::default().inspect(&kb.root);
    assert_eq!(
        report.folder.state,
        FolderState::NeedsIndex,
        "{}",
        report.folder.detail
    );
    assert!(report.folder.detail.contains("source revision"));
}

/// Native source projections are metadata-checked too; a missing one cannot be silently recreated.
#[test]
fn absent_native_projection_requires_rebuilding_without_repair() {
    let (_directory, kb) = library(true);
    let native = native_generation(&kb);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(native.join("manifest.json")).unwrap()).unwrap();
    let relative = manifest["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|record| record["document"]["content"]["path"].as_str())
        .unwrap();
    let path = native.ancestors().nth(3).unwrap().join(relative);
    fs::remove_file(&path).unwrap();
    assert_eq!(
        StatusProbe::default().inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    assert!(!path.exists());
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), &path).unwrap();
    assert_eq!(
        StatusProbe::default().inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
    assert!(
        fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

/// A fresh process cannot silently start Core or create its native cache just to inspect status.
#[test]
fn cold_status_never_initializes_account() {
    if let Some(root) = std::env::var_os("OMNI_STATUS_TEST_ROOT") {
        assert!(knowledge_core::admission::Account::current().is_err());
        let report = StatusProbe::default().inspect(Path::new(&root));
        assert_eq!(
            report.folder.state,
            FolderState::Checking,
            "{}",
            report.folder.detail
        );
        assert!(report.folder.detail.contains("account"));
        assert!(knowledge_core::admission::Account::current().is_err());
        assert!(!Path::new(&std::env::var_os("OMNI_CACHE_DIR").unwrap()).exists());
        return;
    }
    let (_directory, kb) = library(true);
    let cache = kb.root.join("must-not-create-cache");
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cold_status_never_initializes_account",
            "--nocapture",
        ])
        .env("OMNI_STATUS_TEST_ROOT", &kb.root)
        .env("OMNI_CACHE_DIR", &cache)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!cache.exists());
}

/// Retries an active publication gate without blocking behind the external writer.
#[test]
fn publication_gate_is_checked_without_waiting() {
    let (_directory, kb) = library(true);
    let gate = File::options()
        .read(true)
        .write(true)
        .open(kb.root.join(".omni/read.lock"))
        .unwrap();
    FileExt::lock_exclusive(&gate).unwrap();
    let start = Instant::now();
    let report = StatusProbe::default().inspect(&kb.root);
    assert_eq!(report.folder.state, FolderState::Checking);
    assert!(start.elapsed() < Duration::from_secs(1));
    FileExt::unlock(&gate).unwrap();
}

/// Treats a probe size limit as unknown freshness rather than asserting index corruption.
#[test]
fn automatic_scan_size_limit_does_not_force_reindexing() {
    let (_directory, kb) = library(true);
    File::options()
        .write(true)
        .open(generation(&kb).join("manifest.json"))
        .unwrap()
        .set_len(512 * 1024 * 1024 + 1)
        .unwrap();
    let report = StatusProbe::default().inspect(&kb.root);
    assert_eq!(report.folder.state, FolderState::Checking);
    assert!(report.folder.detail.contains("check size limit"));
}

/// Cached derived state cannot remain current after the authoritative marker identity changes.
#[test]
fn cached_manifest_does_not_bypass_current_library_identity() {
    let (_directory, kb) = library(true);
    let probe = StatusProbe::default();
    assert_eq!(probe.inspect(&kb.root).folder.state, FolderState::Current);
    let marker = kb.root.join(CONFIG_FILE);
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    config["library_id"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
    fs::write(marker, serde_json::to_vec(&config).unwrap()).unwrap();
    assert_eq!(
        probe.inspect(&kb.root).folder.state,
        FolderState::NeedsIndex
    );
}

/// Documents the intentional limitation of metadata-based checking for equal-stamp byte changes.
#[test]
fn equal_size_and_timestamp_edits_need_explicit_hash_reconciliation() {
    let (_directory, kb) = library(false);
    let source = kb.root.join("raw/same-stamp.txt");
    fs::write(&source, "alpha").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let modified = fs::metadata(&source).unwrap().modified().unwrap();
    fs::write(&source, "omega").unwrap();
    File::open(&source)
        .unwrap()
        .set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    let report = StatusProbe::default().inspect(&kb.root);
    assert_eq!(report.folder.state, FolderState::Current);
    assert!(report.folder.detail.contains("hashes were not checked"));
    kb.index(&IndexOptions::default()).unwrap();
    assert_eq!(
        kb.search(&SearchOptions {
            query: "omega".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
}

/// Starts a bounded loopback fixture that accepts only two health/model GET requests.
fn provider_fixture(dimension: usize) -> (EmbeddingConfig, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut config = EmbeddingConfig {
        endpoint: format!("http://{}", listener.local_addr().unwrap()),
        dimension: 2,
        model: "fixture".into(),
        package_fingerprint: "a".repeat(64),
        embedding_space: "fixture-space".into(),
        ..Default::default()
    };
    config.timeout_seconds = 2;
    let identity = config.clone();
    let thread = std::thread::spawn(move || {
        let mut paths = vec![];
        let deadline = Instant::now() + Duration::from_secs(5);
        while paths.len() < 2 && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(stream) => stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            // macOS can inherit listener nonblocking mode; accepted requests use bounded blocking reads.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            let mut buffer = [0; 1024];
            while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0 && bytes.len() < 8192);
                bytes.extend_from_slice(&buffer[..count]);
            }
            let request = String::from_utf8(bytes).unwrap();
            let line = request.lines().next().unwrap().to_owned();
            let body = if line == "GET /health HTTP/1.1" {
                serde_json::json!({"status":"ready","package_fingerprint":identity.package_fingerprint,"backend":"fixture"})
            } else {
                assert_eq!(line, "GET /v1/models HTTP/1.1");
                serde_json::json!({"data":[{"id":identity.model,"dimension":dimension,"embedding_space":identity.embedding_space}]})
            };
            paths.push(line);
            let body = body.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
        paths
    });
    (config, thread)
}

/// A health fixture must accept fragmented headers instead of racing the first socket read.
#[test]
fn provider_fixture_accepts_fragmented_requests() {
    let (config, server) = provider_fixture(2);
    let address = config.endpoint.strip_prefix("http://").unwrap();
    for path in ["/health", "/v1/models"] {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost:").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        write!(stream, " localhost\r\n\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    }
    assert_eq!(
        server.join().unwrap(),
        ["GET /health HTTP/1.1", "GET /v1/models HTTP/1.1"]
    );
}

/// Green service state requires matching health/model identity and sends no source payload.
#[test]
fn provider_running_status_requires_validated_identity() {
    for (dimension, expected) in [(2, ServiceState::Available), (3, ServiceState::Unavailable)] {
        let (_directory, mut kb) = library(false);
        let (config, thread) = provider_fixture(dimension);
        kb.config.embedding = Some(config);
        fs::write(
            kb.root.join(CONFIG_FILE),
            serde_json::to_vec(&kb.config).unwrap(),
        )
        .unwrap();
        let report = StatusProbe::default().inspect(&kb.root);
        assert_eq!(
            report.provider.state, expected,
            "{}",
            report.provider.detail
        );
        assert_eq!(
            thread.join().unwrap(),
            ["GET /health HTTP/1.1", "GET /v1/models HTTP/1.1"]
        );
    }
}

/// Invalid provider URLs are rejected before networking and ASR configuration remains independently visible.
#[test]
fn remote_provider_and_incomplete_asr_are_not_reported_available() {
    let (_directory, mut kb) = library(false);
    kb.config.embedding = Some(EmbeddingConfig {
        endpoint: "https://example.com".into(),
        ..Default::default()
    });
    kb.config.whisper_executable = Some(kb.root.join("missing-whisper"));
    fs::write(
        kb.root.join(CONFIG_FILE),
        serde_json::to_vec(&kb.config).unwrap(),
    )
    .unwrap();
    let report = StatusProbe::default().inspect(&kb.root);
    assert_eq!(report.provider.state, ServiceState::Unavailable);
    assert_eq!(report.asr.state, ServiceState::Unavailable);
}
