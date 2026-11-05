//! Actual Apple stream/FD/runtime tests. Linux does not collect or qualify these native observations.
#![cfg(target_os = "macos")]
use knowledge_core::{
    admission::{Account, Configuration, Scheduling},
    backend::Budget,
};
use knowledge_local::{
    CurrentAdmission, LocalMount, LocalReadPolicy, LocalityPremise, ReadDecision, WatchError,
};
use std::{
    fs::{self, File},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};

/// Explicit ordinary-local read owner; no declaration-derived source authorization.
struct Owner;
impl LocalReadPolicy for Owner {
    /// Admit only this test's selected root and library.
    fn decide(&self) -> ReadDecision {
        ReadDecision::Admit(CurrentAdmission {
            library_id: "native-fsevents".into(),
            mount_id: None,
            revision: 1,
            locality: LocalityPremise::OrdinaryLocal,
            outside_scope: vec![],
        })
    }
}
/// Actual disposable roots, removed after all runtime/FD owners are stopped.
struct Fixture(PathBuf);
impl Fixture {
    /// Create canonical private native directories, without relying on the /var pathname alias.
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "fsevents-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(root.join("source")).unwrap();
        fs::create_dir(root.join("cache")).unwrap();
        fs::set_permissions(root.join("cache"), fs::Permissions::from_mode(0o700)).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    /// Remove only this fixture, after its native watches and mount have been dropped.
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Finite real stream plan; no permanent or detached native worker is permitted.
fn budget() -> Budget {
    Budget {
        max_files: 100000,
        max_bytes: 16 * 1024 * 1024,
        max_millis: 15000,
        max_memory_bytes: 16 * 1024 * 1024,
    }
}
/// Observe actual parent descriptors, not just a reservation counter or a stop request.
fn descriptors() -> usize {
    fs::read_dir("/dev/fd").unwrap().count()
}
/// Storage 78/125: actual callbacks, honest startup/quiet, held-root drift, limits and exit-before-settlement.
#[test]
fn actual_stream_callbacks_root_drift_deadline_and_owned_settlement() {
    let f = Fixture::new();
    let account = Account::start(
        File::open(f.0.join("cache")).unwrap(),
        Configuration {
            memory_bytes: Some(128 * 1024 * 1024),
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 8,
            max_records: 32,
            max_descriptor_bytes: 65536,
            per_mount: 1,
        },
    )
    .unwrap();
    let source = f.0.join("source");
    let mount = LocalMount::open(&source, "native-fsevents", None, Owner).unwrap();
    let helper = std::path::Path::new(env!("CARGO_BIN_EXE_omni-fsevents"));
    let base = account.status().unwrap().reserved.memory_bytes;
    let fds = descriptors();
    let mut watch = mount.watch_with_helper(budget(), helper).unwrap();
    assert!(account.status().unwrap().reserved.memory_bytes > base);
    let startup = watch.poll().unwrap();
    assert!(startup.coverage_complete && startup.reconciliation_required);
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        watch.poll().unwrap().reconciliation_required,
        "quiet is not durable inventory"
    );
    fs::write(source.join("changed.txt"), b"actual native event").unwrap();
    let start = Instant::now();
    loop {
        let hints = watch.poll().unwrap();
        if hints.epoch > startup.epoch {
            assert!(hints.reconciliation_required);
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "no actual FSEvents callback delivery"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let cursor = watch
        .native_cursor()
        .unwrap()
        .expect("actual native ID after delivery");
    assert_ne!(cursor.volume, [0; 16]);
    assert_ne!(cursor.event_id, 0);
    assert_eq!(cursor.root, mount.root_identity().unwrap());
    fs::rename(&source, f.0.join("held-source")).unwrap();
    fs::create_dir(&source).unwrap();
    fs::write(source.join("replacement.txt"), b"must not redirect").unwrap();
    assert!(
        watch.poll().is_err(),
        "a pathname replacement cannot redirect a held watch"
    );
    watch.close().unwrap();
    assert_eq!(account.status().unwrap().reserved.memory_bytes, base);
    assert_eq!(descriptors(), fds);
    drop(mount);

    let mount = LocalMount::open(&source, "native-fsevents", None, Owner).unwrap();
    assert!(matches!(
        mount.watch_with_helper(
            Budget {
                max_memory_bytes: 1,
                ..budget()
            },
            helper
        ),
        Err(WatchError::Budget)
    ));
    assert_eq!(account.status().unwrap().reserved.memory_bytes, base);
    let mut watch = mount
        .watch_with_helper(
            Budget {
                max_millis: 750,
                ..budget()
            },
            helper,
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert!(matches!(watch.poll(), Err(WatchError::LeaseExpired)));
    watch.close().unwrap();
    assert_eq!(account.status().unwrap().reserved.memory_bytes, base);

    // A real tiny-work stream and actual write storm cannot establish empty healthy continuity.
    let mut watch = mount
        .watch_with_helper(
            Budget {
                max_files: 2,
                ..budget()
            },
            helper,
        )
        .unwrap();
    for i in 0..32 {
        fs::write(source.join(format!("storm-{i}")), b"hint").unwrap();
    }
    let start = Instant::now();
    loop {
        match watch.poll() {
            Err(_) => break,
            Ok(h) if !h.coverage_complete => {
                assert!(h.reconciliation_required);
                break;
            }
            _ => (),
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "work exhaustion became false continuity"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    watch.close().unwrap();
    assert_eq!(account.status().unwrap().reserved.memory_bytes, base);
    assert_eq!(descriptors(), fds);
}
