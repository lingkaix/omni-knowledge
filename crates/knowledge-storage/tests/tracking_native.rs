//! Actual source reconciliation/replay/polling/migration checks, not source-text or mocked-filesystem oracles.
use knowledge_core::{
    admission::{Account, Configuration, Decision, Holding, Origin, Scheduling},
    backend::Budget,
    jobs::Reservation,
};
use knowledge_local::{
    CurrentAdmission, LocalMount, LocalReadPolicy, LocalityPremise, MonitorPolicy, ReadDecision,
    TrackingMode,
};
use knowledge_storage::*;
use std::{
    fs::{self, File},
    sync::{Mutex, MutexGuard, OnceLock},
    time::Duration,
};
const M: u64 = 1024 * 1024;
/// Serialize only this process-owned real fixture account.
static SERIAL: Mutex<()> = Mutex::new(());
/// Native selected cache remains alive while the actual process account holds its directory.
static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
/// Explicit admitted native fixture owner; declaration data is not permission.
#[derive(Clone)]
struct Owner;
impl WritePolicy for Owner {
    /// Actual fixture admission, with no remote/transport or raw-original write grant.
    fn current(&self) -> Result<WriteContext, Error> {
        Ok(WriteContext {
            library_id: "tracking-fixture".into(),
            revision: 1,
            text_root: "pages".into(),
            writable: true,
            raw_original_writes: false,
            outside_scope: vec![],
        })
    }
}
impl LocalReadPolicy for Owner {
    /// Ordinary selected fixture root with actual runtime-internal exclusions.
    fn decide(&self) -> ReadDecision {
        ReadDecision::Admit(CurrentAdmission {
            library_id: "tracking-fixture".into(),
            mount_id: None,
            revision: 1,
            locality: LocalityPremise::OrdinaryLocal,
            outside_scope: vec![
                knowledge_core::data::RelativePath::new("journal").unwrap(),
                knowledge_core::data::RelativePath::new(".omni").unwrap(),
            ],
        })
    }
}
/// Own exact native disposable resources; no synthetic verified original is needed for metadata scanning.
struct Fixture {
    /// Serialization lifetime.
    _serial: MutexGuard<'static, ()>,
    /// Actual canonical parent, including macOS /var alias correction.
    parent: tempfile::TempDir,
    /// Real process budget token retained through every native/result lifetime.
    holding: Holding,
    /// Actual one account.
    account: &'static Account,
}
impl Fixture {
    /// Bind native private roots and reserve before metadata/hash/candidate allocation.
    fn new() -> Self {
        let serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().canonicalize().unwrap();
        let cache = CACHE.get_or_init(|| {
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::Builder::new()
                .prefix("tracking-cache-")
                .tempdir_in(&tmp)
                .unwrap();
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
            dir
        });
        let account = match Account::current() {
            Ok(a) => a,
            Err(_) => Account::start(
                File::open(cache.path()).unwrap(),
                Configuration {
                    memory_bytes: Some(512 * M),
                    storage_bytes: Some(1024 * M),
                },
                Scheduling {
                    max_pending: 128,
                    max_records: 4096,
                    max_descriptor_bytes: 4 * M as usize,
                    per_mount: 1,
                },
            )
            .unwrap(),
        };
        let parent = tempfile::Builder::new()
            .prefix("tracking-home-")
            .tempdir_in(tmp)
            .unwrap();
        fs::write(parent.path().join("omni-knowledge.json"),b"{\"version\":2,\"library_id\":\"tracking-fixture\",\"name\":\"tracking\",\"text_root\":\"pages\",\"mounts\":[],\"convention_version\":\"0.2\",\"exposure\":{\"mode\":\"off\",\"sensitive\":false,\"excluded_paths\":[],\"transports\":[]}}").unwrap();
        fs::create_dir(parent.path().join("pages")).unwrap();
        let holding = match account
            .reserve(
                Reservation {
                    memory_bytes: 32 * M,
                    temporary_bytes: 0,
                },
                Origin::BeforeVerification,
            )
            .unwrap()
        {
            Decision::Ready(h) => h,
            _ => panic!("fixture contention"),
        };
        Self {
            _serial: serial,
            parent,
            holding,
            account,
        }
    }
    /// Selected actual native home.
    fn root(&self) -> &std::path::Path {
        self.parent.path()
    }
    /// Real native writer bound to current owner.
    fn store(&self) -> Store<Owner> {
        Store::open(self.root(), "tracking-fixture", "tracking-writer", Owner).unwrap()
    }
    /// Real native read backend, not a fake metadata source.
    fn mount(&self) -> LocalMount<Owner> {
        LocalMount::open(self.root(), "tracking-fixture", None, Owner).unwrap()
    }
    /// Whole actual per-operation work plan, with separately retained real resources.
    fn preflight(&self) -> Preflight<'_> {
        Preflight {
            holding: &self.holding,
            budget: Budget {
                max_files: 10000,
                max_bytes: 16 * M,
                max_millis: 60000,
                max_memory_bytes: 32 * M,
            },
        }
    }
    /// Perform actual selected source verification/reconciliation.
    fn scan(&self) -> Reconciled {
        self.store()
            .reconcile_local(
                &self.mount(),
                ReconcileOptions {
                    verify_bytes: true,
                    schedule: Schedule::StartupPeriodic,
                },
                &self.preflight(),
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    /// Native methods are synchronous; settle only after their file/owned buffer lifetimes ended.
    fn drop(&mut self) {
        let _ = self.holding.finish();
    }
}
/// Real equal-content files stay distinct; rename/replay/reopen preserves qualified asset continuity.
#[test]
fn native_reconcile_replay_rename_duplicate_and_delete() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"same bytes").unwrap();
    fs::write(f.root().join("pages/b.md"), b"same bytes").unwrap();
    let first = f.scan();
    assert!(first.complete && first.last_complete.is_some());
    assert_eq!(first.verified, 2);
    let store = f.store();
    let replay = store.replay_tracking(0, &f.preflight()).unwrap();
    assert!(replay.complete && replay.issue.is_none());
    let a = replay
        .observations
        .iter()
        .find(|o| o.stat.path.as_str() == "pages/a.md")
        .unwrap();
    let b = replay
        .observations
        .iter()
        .find(|o| o.stat.path.as_str() == "pages/b.md")
        .unwrap();
    assert_ne!(a.source_id, b.source_id);
    assert_eq!(
        a.version.as_ref().unwrap().digest,
        b.version.as_ref().unwrap().digest
    );
    let aid = a.source_id.clone();
    let bid = b.source_id.clone();
    assert!(
        replay
            .checkpoints
            .iter()
            .any(|c| c.complete && c.observed_at.contains('T'))
    );
    // Evict only this fixture's known qualified GENERATED projection; visible source/journal data stays intact.
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(f.root()).unwrap();
    let namespace = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                (meta.dev(), meta.ino()),
                f.mount().context(),
                "tracking-writer"
            ))
            .unwrap()
        )
    );
    fs::remove_dir_all(
        CACHE
            .get()
            .unwrap()
            .path()
            .join("inventory-v1")
            .join(namespace),
    )
    .unwrap();
    assert!(f.scan().complete);
    let rebuilt = f.store().replay_tracking(0, &f.preflight()).unwrap();
    assert_eq!(
        rebuilt
            .observations
            .iter()
            .rev()
            .find(|o| o.stat.path.as_str() == "pages/a.md")
            .unwrap()
            .source_id,
        aid
    );
    fs::rename(f.root().join("pages/a.md"), f.root().join("pages/moved.md")).unwrap();
    fs::remove_file(f.root().join("pages/b.md")).unwrap();
    assert!(f.scan().complete);
    let replay = f.store().replay_tracking(0, &f.preflight()).unwrap();
    assert!(replay.complete);
    let moved = replay
        .observations
        .iter()
        .rev()
        .find(|o| o.stat.path.as_str() == "pages/moved.md")
        .unwrap();
    assert_eq!(moved.source_id, aid);
    assert_eq!(moved.change, ChangeKind::Renamed);
    assert!(
        replay
            .observations
            .iter()
            .any(|o| o.source_id == bid && o.change == ChangeKind::Deleted)
    );
    assert!(
        !replay
            .observations
            .iter()
            .any(|o| o.source_id == aid && o.change == ChangeKind::Deleted)
    );
}
/// A bounded partial scan and lost root never manufacture deletion or replace a prior complete checkpoint.
#[test]
fn partial_and_unavailable_never_emit_false_tombstones() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"source").unwrap();
    assert!(f.scan().complete);
    fs::remove_file(f.root().join("pages/a.md")).unwrap();
    let pre = Preflight {
        holding: &f.holding,
        budget: Budget {
            max_files: 1,
            ..f.preflight().budget
        },
    };
    let report = f
        .store()
        .reconcile_local(
            &f.mount(),
            ReconcileOptions {
                verify_bytes: false,
                schedule: Schedule::NightlyManual,
            },
            &pre,
        )
        .unwrap();
    assert!(!report.complete && report.last_complete.is_none());
    let replay = f.store().replay_tracking(0, &f.preflight()).unwrap();
    assert!(
        !replay
            .observations
            .iter()
            .any(|o| o.change == ChangeKind::Deleted)
    );
    assert!(replay.checkpoints.iter().any(|c| c.complete));
    assert!(!replay.checkpoints.last().unwrap().complete);
    let old = fs::read(f.root().join("journal/tracking-writer.jsonl")).unwrap();
    let detached = f.root().with_extension("detached");
    let store = f.store();
    let mount = f.mount();
    fs::rename(f.root(), &detached).unwrap();
    assert!(
        store
            .reconcile_local(
                &mount,
                ReconcileOptions {
                    verify_bytes: false,
                    schedule: Schedule::Manual
                },
                &f.preflight()
            )
            .is_err()
    );
    assert_eq!(
        fs::read(detached.join("journal/tracking-writer.jsonl")).unwrap(),
        old
    );
    fs::rename(detached, f.root()).unwrap();
}
/// Disposable projections cannot override a new visible writer-stream incarnation at the same native root.
/// Storage's rebuildable-state rule requires current visible evidence, not an old generated anchor.
#[test]
fn stale_generated_projection_does_not_corrupt_a_fresh_visible_stream() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"same bytes").unwrap();
    assert!(f.scan().complete);
    let before = f.store().replay_tracking(0, &f.preflight()).unwrap();
    let old_id = before.observations[0].source_id.clone();
    let old_journal = fs::read(f.root().join("journal/tracking-writer.jsonl")).unwrap();
    let archive = tempfile::Builder::new()
        .prefix("retained-journal-")
        .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
        .unwrap();
    // Retain the actual old stream outside the selected home, leaving its generated projection behind.
    // This reproduces the namespace collision from native root-inode reuse without forging metadata.
    fs::rename(f.root().join("journal"), archive.path().join("journal")).unwrap();
    assert!(f.scan().complete);
    let current = f.store().replay_tracking(0, &f.preflight()).unwrap();
    assert!(current.complete && current.issue.is_none());
    assert_ne!(
        current.observations[0].source_id, old_id,
        "generated cache became asset authority"
    );
    assert_eq!(
        fs::read(archive.path().join("journal/tracking-writer.jsonl")).unwrap(),
        old_journal
    );
    let id = current.observations[0].source_id.clone();
    assert!(f.scan().complete);
    let reopened = f.store().replay_tracking(0, &f.preflight()).unwrap();
    assert!(reopened.observations.iter().all(|o| o.source_id == id));
}

/// Actual polling is an explicit reduced-capability choice and performs bounded metadata, never fake events.
#[test]
fn explicit_budgeted_polling_and_native_capacity_fallback() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"source").unwrap();
    let mount = f.mount();
    let baseline = f.account.status().unwrap().reserved.memory_bytes;
    let budget = Budget {
        max_memory_bytes: M,
        ..f.preflight().budget
    };
    let mut monitor = mount
        .monitor(
            budget,
            MonitorPolicy {
                allow_poll_fallback: true,
                polling_only: true,
                poll_interval: Duration::from_secs(60),
            },
        )
        .unwrap();
    let s = monitor.poll().unwrap();
    assert_eq!(s.mode, TrackingMode::Polling);
    assert!(s.hints.reconciliation_required && s.hints.coverage_complete);
    assert!(!s.durable_event_replay);
    assert!(s.native_failure.is_none());
    assert!(monitor.poll().unwrap().hints.reconciliation_required);
    assert!(f.scan().complete);
    assert!(
        !monitor
            .acknowledge_reconciliation(s.hints.epoch)
            .unwrap()
            .hints
            .reconciliation_required
    );
    assert!(!monitor.poll().unwrap().hints.reconciliation_required);
    drop(monitor);
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, baseline);
    let mut actual = mount
        .monitor(
            Budget {
                max_memory_bytes: 16 * M,
                ..budget
            },
            MonitorPolicy {
                allow_poll_fallback: true,
                polling_only: false,
                poll_interval: Duration::from_secs(60),
            },
        )
        .unwrap();
    let status = actual.poll().unwrap();
    if status.mode == TrackingMode::Polling {
        assert!(status.native_failure.is_some());
    }
    assert!(status.hints.reconciliation_required);
    drop(actual);
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, baseline);
}
/// Actual legacy inventory preserves unknown bytes and reports conflicting authored trees without migration.
#[test]
fn migration_inventory_is_readonly_and_conflict_preserving() {
    let f = Fixture::new();
    fs::create_dir_all(f.root().join(".omni/metadata")).unwrap();
    fs::create_dir_all(f.root().join("wiki/sources")).unwrap();
    fs::write(
        f.root().join(".omni/metadata/source.json"),
        b"{\"tags\":[\"Authored\"],\"unknown\":1e2}",
    )
    .unwrap();
    fs::write(
        f.root().join(".omni/unknown.bin"),
        b"unknown original state",
    )
    .unwrap();
    let before = fs::read(f.root().join(".omni/metadata/source.json")).unwrap();
    let result = f.store().migration_inventory(&f.preflight()).unwrap();
    assert!(result.complete && result.conflicting_text_roots);
    assert!(
        result
            .items
            .iter()
            .any(|i| i.kind == MigrationKind::LegacyAnnotation)
    );
    assert!(
        result
            .items
            .iter()
            .any(|i| i.kind == MigrationKind::UnknownLegacyState)
    );
    assert_eq!(
        fs::read(f.root().join(".omni/metadata/source.json")).unwrap(),
        before
    );
    assert_eq!(
        fs::read(f.root().join(".omni/unknown.bin")).unwrap(),
        b"unknown original state"
    );
}
/// Actual malformed tail is reported at its original replay offset while the valid prefix remains visible.
#[test]
fn replay_retains_valid_prefix_and_reports_truncated_tail() {
    use std::io::Write;
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"source").unwrap();
    f.scan();
    let path = f.root().join("journal/tracking-writer.jsonl");
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(b"{truncated").unwrap();
    file.sync_all().unwrap();
    drop(file);
    let before = fs::read(&path).unwrap();
    let replay = f.store().replay_tracking(0, &f.preflight()).unwrap();
    assert!(!replay.complete && replay.issue == Some(Error::Recovery));
    assert_eq!(replay.observations.len(), 1);
    assert_eq!(fs::read(path).unwrap(), before);
}

/// Actual hard links never pick an annotation owner by equal bytes/inode after alias ambiguity.
#[test]
fn hard_link_ambiguity_does_not_transplant_source_identity() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"same object").unwrap();
    fs::hard_link(f.root().join("pages/a.md"), f.root().join("pages/b.md")).unwrap();
    f.scan();
    let first = f.store().replay_tracking(0, &f.preflight()).unwrap();
    let ids = first
        .observations
        .iter()
        .map(|o| o.source_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(first.observations.iter().all(|o| o.identity_ambiguous));
    fs::remove_file(f.root().join("pages/a.md")).unwrap();
    fs::rename(f.root().join("pages/b.md"), f.root().join("pages/moved.md")).unwrap();
    f.scan();
    let replay = f.store().replay_tracking(0, &f.preflight()).unwrap();
    let new = replay
        .observations
        .iter()
        .rev()
        .find(|o| o.stat.path.as_str() == "pages/moved.md")
        .unwrap();
    assert!(!ids.contains(&new.source_id));
    assert!(new.identity_ambiguous);
}
/// Real FSEvents source binding, confirmed runtime exit and event delivery; compiled/run only by native macOS CI.
#[cfg(target_os = "macos")]
#[test]
fn actual_fsevents_native_delivery_and_resource_settlement() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"before").unwrap();
    let mount = f.mount();
    let before = f.account.status().unwrap().reserved.memory_bytes;
    let mut monitor = mount
        .monitor(
            Budget {
                max_memory_bytes: 16 * M,
                ..f.preflight().budget
            },
            MonitorPolicy {
                allow_poll_fallback: false,
                polling_only: false,
                poll_interval: Duration::from_secs(60),
            },
        )
        .unwrap();
    let initial = monitor.poll().unwrap();
    assert_eq!(initial.mode, TrackingMode::Fsevents);
    assert!(initial.hints.reconciliation_required);
    assert!(f.scan().complete);
    monitor
        .acknowledge_reconciliation(initial.hints.epoch)
        .unwrap();
    fs::write(f.root().join("pages/a.md"), b"after").unwrap();
    let start = std::time::Instant::now();
    let mut delivered = false;
    while start.elapsed() < Duration::from_secs(5) {
        let status = monitor.poll().unwrap();
        if status.hints.epoch > initial.hints.epoch {
            assert!(status.hints.reconciliation_required);
            delivered = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        delivered,
        "native FSEvents did not deliver the actual source change"
    );
    drop(monitor);
    assert_eq!(f.account.status().unwrap().reserved.memory_bytes, before);
}

/// Storage 125: actual native IDs cannot precede durable complete inventory; restart performs real catch-up.
#[cfg(target_os = "macos")]
#[test]
fn native_cursor_follows_inventory_and_restart_never_assumes_continuity() {
    let f = Fixture::new();
    // replay_tracking requires a durable inventory. A cold store correctly returns Missing;
    // the initial enumeration still must not invent an FSEvents cursor.
    assert!(f.scan().complete);
    let mount = f.mount();
    let store = f.store();
    let watch_budget = Budget {
        max_memory_bytes: 16 * M,
        ..f.preflight().budget
    };
    let mut watch = mount.watch(watch_budget).unwrap();
    assert!(watch.poll().unwrap().reconciliation_required);
    fs::write(f.root().join("pages/native.md"), b"actual native source").unwrap();
    let start = std::time::Instant::now();
    while watch.native_cursor().unwrap().is_none() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "native event ID not delivered"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        store
            .replay_tracking(0, &f.preflight())
            .unwrap()
            .event_cursors
            .is_empty()
    );
    let partial = store
        .reconcile_watched(
            &mut watch,
            ReconcileOptions {
                verify_bytes: false,
                schedule: Schedule::StartupPeriodic,
            },
            &Preflight {
                budget: Budget {
                    max_files: 1,
                    ..f.preflight().budget
                },
                ..f.preflight()
            },
        )
        .unwrap();
    assert!(!partial.complete);
    assert!(
        store
            .replay_tracking(0, &f.preflight())
            .unwrap()
            .event_cursors
            .is_empty()
    );
    std::thread::sleep(Duration::from_millis(250));
    let complete = store
        .reconcile_watched(
            &mut watch,
            ReconcileOptions {
                verify_bytes: true,
                schedule: Schedule::StartupPeriodic,
            },
            &f.preflight(),
        )
        .unwrap();
    assert!(complete.complete);
    let replay = store.replay_tracking(0, &f.preflight()).unwrap();
    let cursor = replay
        .event_cursors
        .last()
        .expect("native cursor after durable inventory");
    assert_eq!(Some(&cursor.scan), complete.last_complete.as_ref());
    assert!(
        replay
            .checkpoints
            .iter()
            .any(|c| c.scan == cursor.scan && c.complete)
    );
    let lines = fs::read_to_string(f.root().join("journal/tracking-writer.jsonl")).unwrap();
    let records: Vec<serde_json::Value> = lines
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let inventory_index = records
        .iter()
        .position(|r| r["checkpoint"]["scan"] == cursor.scan)
        .unwrap();
    let cursor_index = records
        .iter()
        .position(|r| r["event_cursor"]["scan"] == cursor.scan)
        .unwrap();
    assert!(
        inventory_index < cursor_index,
        "event cursor preceded corresponding inventory durability"
    );
    watch.close().unwrap();
    drop(store);
    fs::remove_file(f.root().join("pages/native.md")).unwrap();
    let mut restarted = mount.watch(watch_budget).unwrap();
    assert!(restarted.poll().unwrap().reconciliation_required);
    // A newly delivered native ID is possible even immediately after startup, but it cannot acknowledge old inventory.
    let _ = restarted.native_cursor().unwrap();
    assert!(
        restarted.poll().unwrap().reconciliation_required,
        "restart invented native continuity"
    );
    let store = f.store();
    let caught_up = store
        .reconcile_watched(
            &mut restarted,
            ReconcileOptions {
                verify_bytes: true,
                schedule: Schedule::StartupPeriodic,
            },
            &f.preflight(),
        )
        .unwrap();
    assert!(caught_up.complete);
    assert!(
        store
            .replay_tracking(0, &f.preflight())
            .unwrap()
            .observations
            .iter()
            .any(|o| o.stat.path.as_str() == "pages/native.md" && o.change == ChangeKind::Deleted)
    );
    restarted.close().unwrap();

    // Real native work loss must lead to bounded inventory/known-descendant reconciliation, never cursor advancement.
    fs::write(f.root().join("pages/gap-old.md"), b"known descendant").unwrap();
    assert!(f.scan().complete);
    let cursor_count = store
        .replay_tracking(0, &f.preflight())
        .unwrap()
        .event_cursors
        .len();
    let mut limited = mount
        .watch(Budget {
            max_files: 2,
            ..watch_budget
        })
        .unwrap();
    fs::remove_file(f.root().join("pages/gap-old.md")).unwrap();
    for i in 0..8 {
        fs::write(
            f.root().join(format!("pages/storm-{i}.md")),
            b"actual native hint",
        )
        .unwrap();
    }
    let start = std::time::Instant::now();
    loop {
        match limited.poll() {
            Err(_) => break,
            Ok(h) if !h.coverage_complete => {
                assert!(h.reconciliation_required);
                break;
            }
            _ => (),
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "native work loss was not exposed"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Ok(h) = limited.acknowledge_reconciliation(0) {
        assert!(h.reconciliation_required);
    }
    let reconciled = store
        .reconcile_local(
            &mount,
            ReconcileOptions {
                verify_bytes: true,
                schedule: Schedule::StartupPeriodic,
            },
            &f.preflight(),
        )
        .unwrap();
    assert!(reconciled.complete);
    let replay = store.replay_tracking(0, &f.preflight()).unwrap();
    assert_eq!(replay.event_cursors.len(), cursor_count);
    assert!(
        replay
            .observations
            .iter()
            .any(|o| o.stat.path.as_str() == "pages/gap-old.md" && o.change == ChangeKind::Deleted)
    );
    assert!(
        replay
            .observations
            .iter()
            .any(|o| o.stat.path.as_str() == "pages/storm-7.md" && o.change == ChangeKind::Created)
    );
    limited.close().unwrap();
}

/// Sequential native writers preserve source IDs from visible evidence, not equal content.
#[test]
fn writer_transition_keeps_identity_and_separates_equal_bytes() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"same").unwrap();
    fs::write(f.root().join("pages/b.md"), b"same").unwrap();
    f.scan();
    let first = f.store().replay_tracking(0, &f.preflight()).unwrap();
    let a = first
        .observations
        .iter()
        .find(|o| o.stat.path.as_str() == "pages/a.md")
        .unwrap();
    let b = first
        .observations
        .iter()
        .find(|o| o.stat.path.as_str() == "pages/b.md")
        .unwrap();
    let next = Store::open(f.root(), "tracking-fixture", "writer-next", Owner).unwrap();
    next.reconcile_local(
        &f.mount(),
        ReconcileOptions {
            verify_bytes: true,
            schedule: Schedule::Manual,
        },
        &f.preflight(),
    )
    .unwrap();
    let replay = next.replay_tracking(0, &f.preflight()).unwrap();
    assert_eq!(
        replay
            .observations
            .iter()
            .find(|o| o.stat.path.as_str() == "pages/a.md")
            .unwrap()
            .source_id,
        a.source_id
    );
    assert_eq!(
        replay
            .observations
            .iter()
            .find(|o| o.stat.path.as_str() == "pages/b.md")
            .unwrap()
            .source_id,
        b.source_id
    );
    assert_ne!(a.source_id, b.source_id);
    fs::rename(f.root().join("pages/a.md"), f.root().join("pages/moved.md")).unwrap();
    next.reconcile_local(
        &f.mount(),
        ReconcileOptions {
            verify_bytes: true,
            schedule: Schedule::Manual,
        },
        &f.preflight(),
    )
    .unwrap();
    f.scan();
    let replay = f.store().replay_tracking(0, &f.preflight()).unwrap();
    let moved = replay
        .observations
        .iter()
        .rev()
        .find(|o| o.stat.path.as_str() == "pages/moved.md")
        .unwrap();
    assert_eq!(moved.source_id, a.source_id);
    assert!(
        !replay
            .observations
            .iter()
            .any(|o| o.source_id == a.source_id && o.change == ChangeKind::Deleted)
    );
}

/// A page boundary cannot hide a duplicate sequence or treat damaged EOF as complete.
#[test]
fn resumed_replay_validates_predecessor_at_page_boundary() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"a").unwrap();
    f.scan();
    let page = Preflight {
        holding: &f.holding,
        budget: Budget {
            max_files: 1,
            ..f.preflight().budget
        },
    };
    let first = f.store().replay_tracking(0, &page).unwrap();
    assert!(!first.complete && first.issue.is_none());
    let file = f.root().join("journal/tracking-writer.jsonl");
    let original = fs::read(&file).unwrap();
    let start = first.next_offset as usize;
    let end = start + original[start..].iter().position(|b| *b == b'\n').unwrap() + 1;
    let previous_start = original[..start - 1]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |n| n + 1);
    let previous: serde_json::Value =
        serde_json::from_slice(&original[previous_start..start]).unwrap();
    let mut next: serde_json::Value = serde_json::from_slice(&original[start..end]).unwrap();
    next["sequence"] = previous["sequence"].clone();
    let mut corrupt = original[..start].to_vec();
    corrupt.extend(serde_json::to_vec(&next).unwrap());
    corrupt.push(b'\n');
    corrupt.extend(&original[end..]);
    fs::write(&file, &corrupt).unwrap();
    let resumed = f.store().replay_tracking(first.next_offset, &page).unwrap();
    assert_eq!(resumed.issue, Some(Error::Recovery));
    assert!(!resumed.complete);
    assert_eq!(resumed.next_offset, first.next_offset);
    assert_eq!(fs::read(file).unwrap(), corrupt);
}
