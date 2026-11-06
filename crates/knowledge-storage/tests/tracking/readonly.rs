//! Existing visible evidence is readable only under a matching actual native read owner.
use super::*;
use std::os::unix::fs::MetadataExt;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
struct Reader {
    calls: Arc<AtomicUsize>,
    admitted_calls: usize,
}
impl WritePolicy for Reader {
    fn current(&self) -> Result<WriteContext, Error> {
        let mut c = Owner.current()?;
        c.writable = false;
        Ok(c)
    }
}
impl LocalReadPolicy for Reader {
    fn decide(&self) -> ReadDecision {
        if self.calls.fetch_add(1, Ordering::SeqCst) >= self.admitted_calls {
            ReadDecision::Deny
        } else {
            Owner.decide()
        }
    }
}
impl Reader {
    fn new(admitted_calls: usize) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            admitted_calls,
        }
    }
}
struct ChangingScope(AtomicUsize);
impl LocalReadPolicy for ChangingScope {
    fn decide(&self) -> ReadDecision {
        let ReadDecision::Admit(mut actual) = Owner.decide() else {
            unreachable!()
        };
        actual.revision = self.0.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        ReadDecision::Admit(actual)
    }
}
fn tree(root: &std::path::Path) -> Vec<(String, u64, u64, i64, i64, String)> {
    fn walk(
        root: &std::path::Path,
        at: &std::path::Path,
        out: &mut Vec<(String, u64, u64, i64, i64, String)>,
    ) {
        for entry in fs::read_dir(at).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            let m = fs::symlink_metadata(&p).unwrap();
            let hash = if m.is_file() {
                use sha2::Digest;
                format!("{:x}", sha2::Sha256::digest(fs::read(&p).unwrap()))
            } else {
                String::new()
            };
            out.push((
                p.strip_prefix(root).unwrap().to_str().unwrap().into(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                hash,
            ));
            if m.is_dir() {
                walk(root, &p, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}
#[test]
fn actual_readonly_owner_replays_without_writer_grant_or_mutation() {
    let f = Fixture::new();
    fs::write(f.root().join("pages/a.md"), b"actual readonly evidence").unwrap();
    let scanned = f.scan();
    assert!(scanned.complete);
    let reader = Reader::new(usize::MAX);
    let store = Store::from_selected_directory(
        f.root(),
        File::open(f.root()).unwrap(),
        "tracking-fixture",
        "tracking-writer",
        reader.clone(),
    )
    .unwrap();
    let mount = LocalMount::from_selected_directory(
        f.root(),
        File::open(f.root()).unwrap(),
        "tracking-fixture",
        None,
        reader,
    )
    .unwrap();
    let before = tree(f.root());
    assert_eq!(
        store.replay_tracking(0, &f.preflight()).err().unwrap(),
        Error::Context,
        "no fabricated writable context for the existing write-gated API"
    );
    let replay = store
        .replay_tracking_readonly(&mount, 0, &f.preflight())
        .unwrap();
    assert!(replay.complete && replay.issue.is_none());
    assert!(
        replay
            .observations
            .iter()
            .any(|o| o.stat.path.as_str() == "pages/a.md")
    );
    #[cfg(target_os = "linux")]
    {
        // Owning-thread kernel counters measure the REAL synchronous readonly adapter, including BufReader
        // prefetch, not merely its logical next_offset. The one /proc snapshot read itself
        // is subtracted exactly; no native backend/provider is mocked or interposed.
        fn rchar() -> (u64, u64) {
            let data = fs::read_to_string("/proc/thread-self/io").unwrap();
            let value = data
                .lines()
                .find_map(|line| line.strip_prefix("rchar: "))
                .unwrap()
                .parse()
                .unwrap();
            (value, data.len() as u64)
        }
        let mut one = f.preflight();
        one.budget.max_bytes = 1;
        let (before_bytes, snapshot_bytes) = rchar();
        let tiny = store.replay_tracking_readonly(&mount, 0, &one).unwrap();
        let (after_bytes, _) = rchar();
        let actual = after_bytes - before_bytes - snapshot_bytes;
        assert!(
            actual <= 1,
            "actual one-byte readonly metadata call read {actual} native bytes; logical cursor {} is not physical IO admission",
            tiny.next_offset
        );
        assert_eq!(tiny.next_offset, 0);
        assert_eq!(tiny.issue, Some(Error::Budget));
        assert_eq!(
            tiny.read_bytes, actual,
            "reported native read spend must include real buffering"
        );
        let (before_bytes, snapshot_bytes) = rchar();
        let tail = store
            .replay_tracking_readonly(&mount, replay.next_offset, &one)
            .unwrap();
        let (after_bytes, _) = rchar();
        let actual = after_bytes - before_bytes - snapshot_bytes;
        assert!(
            actual <= 1,
            "resumed predecessor validation read {actual} native bytes beyond one-byte admission"
        );
        assert!(!tail.complete);
        assert_eq!(tail.next_offset, replay.next_offset);
        assert_eq!(tail.issue, Some(Error::Budget));
        assert_eq!(tail.read_bytes, actual);
    }
    let mut limited = f.preflight();
    limited.budget.max_files = 1;
    let first = store.replay_tracking_readonly(&mount, 0, &limited).unwrap();
    assert!(!first.complete && first.next_offset > 0);
    assert!(
        store
            .replay_tracking_readonly(&mount, first.next_offset, &f.preflight())
            .unwrap()
            .complete
    );
    assert_eq!(
        tree(f.root()),
        before,
        "read path must not append, create or replace any visible source/journal node"
    );
    let changing = LocalMount::from_selected_directory(
        f.root(),
        File::open(f.root()).unwrap(),
        "tracking-fixture",
        None,
        ChangingScope(AtomicUsize::new(0)),
    )
    .unwrap();
    assert!(
        store
            .replay_tracking_readonly(&changing, 0, &f.preflight())
            .is_err(),
        "a changed actual admitted read scope is not the same read context merely because permission remains Admit"
    );
    let denied = LocalMount::from_selected_directory(
        f.root(),
        File::open(f.root()).unwrap(),
        "tracking-fixture",
        None,
        Reader::new(0),
    )
    .unwrap();
    assert!(
        store
            .replay_tracking_readonly(&denied, 0, &f.preflight())
            .is_err()
    );
    let revoked = LocalMount::from_selected_directory(
        f.root(),
        File::open(f.root()).unwrap(),
        "tracking-fixture",
        None,
        Reader::new(3),
    )
    .unwrap();
    assert!(
        store
            .replay_tracking_readonly(&revoked, 0, &f.preflight())
            .is_err(),
        "initial native read decision is not a cached grant through replay"
    );
    let other = tempfile::tempdir_in(f.root().parent().unwrap()).unwrap();
    fs::write(
        other.path().join("omni-knowledge.json"),
        fs::read(f.root().join("omni-knowledge.json")).unwrap(),
    )
    .unwrap();
    fs::create_dir(other.path().join("pages")).unwrap();
    let foreign = LocalMount::from_selected_directory(
        other.path(),
        File::open(other.path()).unwrap(),
        "tracking-fixture",
        None,
        Reader::new(usize::MAX),
    )
    .unwrap();
    assert_eq!(
        store
            .replay_tracking_readonly(&foreign, 0, &f.preflight())
            .err()
            .unwrap(),
        Error::Context,
        "same labels on an unrelated actual selected directory cannot authorize this Store"
    );
    let empty_store = Store::from_selected_directory(
        other.path(),
        File::open(other.path()).unwrap(),
        "tracking-fixture",
        "tracking-writer",
        Reader::new(usize::MAX),
    )
    .unwrap();
    let empty_before = tree(other.path());
    assert!(
        empty_store
            .replay_tracking_readonly(&foreign, 0, &f.preflight())
            .is_err()
    );
    assert_eq!(
        tree(other.path()),
        empty_before,
        "readonly startup must not create a missing journal/lane"
    );
    assert_eq!(tree(f.root()), before);
}
