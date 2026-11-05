//! Independent observations of the ordinary local mount adapter.
//!
//! Expectations come from the storage, product, and service owners, the
//! knowledge-local crate guide, and the public Rustdoc of this crate and
//! `knowledge_core::backend`. They do not come from adapter output.
//!
//! The caller supplies locality and exclusion. This suite does not claim NAS,
//! cloud, account, or service authorization, and it does not claim that the
//! adapter discovers child libraries or inner mounts.
//!
//! A foreign library or mount is `InvalidRequest`. An offset equal to the
//! verified end is empty EOF; only a larger offset is past the end. Memory
//! reservations include live metadata and the initial listing frame. The
//! content observer admits only the x86-64 Linux syscall ABI.

#![cfg(target_os = "linux")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::ffi::{CString, OsStr};
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const _: () = assert!(
    cfg!(target_arch = "x86_64"),
    "content observer syscall numbers are the x86-64 ABI"
);

use knowledge_core::backend::{
    Availability, BackendError, BackendIdentity, Budget, ByteRange, Capabilities, Cursor,
    Enumeration, EnumerationStop, ExpectedVersion, Freshness, MountBackend, Residency,
    StatSnapshot, Unavailable,
};
use knowledge_core::data::{BackendKind, ContentDigest, RelativePath};
use knowledge_local::{
    CurrentAdmission, LocalError, LocalMount, LocalReadPolicy, LocalityPremise, ReadDecision,
};
use sha2::{Digest, Sha256};

const LIBRARY: &str = "library-1";
const MOUNT: &str = "mount-1";
const HARNESS_BOUND: Duration = Duration::from_secs(10);

static FIXTURES: AtomicU64 = AtomicU64::new(0);
static MEASURING: AtomicBool = AtomicBool::new(false);
static MEASURE_TID: AtomicU64 = AtomicU64::new(0);
static LARGE_COUNT: AtomicU64 = AtomicU64::new(0);
static LARGE_MAX: AtomicU64 = AtomicU64::new(0);
static MEASURE_LOCK: Mutex<()> = Mutex::new(());

/// Counts allocations on the measuring thread only.
///
/// Caller-owned strings are built before the window. A size under 8192 is
/// ignored so the 8192-byte library id is the counted copy.
struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        note_large(layout.size());
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(ptr, layout, new_size) };
        note_large(new_size);
        next
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn note_large(size: usize) {
    if size < 8_192 || !MEASURING.load(Ordering::SeqCst) {
        return;
    }
    let tid = unsafe { syscall(186) } as u64;
    if tid != MEASURE_TID.load(Ordering::SeqCst) {
        return;
    }
    LARGE_COUNT.fetch_add(1, Ordering::SeqCst);
    let size = size as u64;
    let mut current = LARGE_MAX.load(Ordering::SeqCst);
    while size > current {
        match LARGE_MAX.compare_exchange_weak(current, size, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

struct AllocNote {
    count: u64,
    max: u64,
}

struct MeasureStop;

impl Drop for MeasureStop {
    fn drop(&mut self) {
        MEASURING.store(false, Ordering::SeqCst);
    }
}

fn measure<T>(body: impl FnOnce() -> T) -> (T, AllocNote) {
    let _guard = MEASURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    LARGE_COUNT.store(0, Ordering::SeqCst);
    LARGE_MAX.store(0, Ordering::SeqCst);
    MEASURE_TID.store(current_tid(), Ordering::SeqCst);
    MEASURING.store(true, Ordering::SeqCst);
    let _stop = MeasureStop;
    let value = body();
    drop(_stop);
    (
        value,
        AllocNote {
            count: LARGE_COUNT.load(Ordering::SeqCst),
            max: LARGE_MAX.load(Ordering::SeqCst),
        },
    )
}

fn current_tid() -> u64 {
    unsafe { syscall(186) as u64 }
}

/// Published SHA-256 vectors used to admit the locked `sha2` 0.10.9 primitive.
///
/// Storage requires a lowercase SHA-256 content digest. The empty-string and
/// "abc" vectors are fixed public inputs. The adapter's own digest is not the oracle.
#[test]
fn published_sha256_vectors_admit_the_locked_primitive() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

/// `LocalMount::open` binds one absolute directory and does not consult policy.
///
/// The crate guide rejects a relative path, a symlink component, and a
/// non-directory as `InvalidRoot`, and rejects an empty or NUL identity as
/// `InvalidIdentity`. Only `EIO` while binding is `Io`, so a mode `000`
/// directory on this unprivileged fixture is `InvalidRoot`.
#[test]
fn open_binds_only_an_absolute_directory_and_does_not_consult_policy() {
    let fixture = Fixture::new();
    let policy = Caller::new(ReadDecision::Deny);
    let mount = LocalMount::open(fixture.path(), LIBRARY, Some(MOUNT.to_string()), &policy)
        .expect("absolute directory binds");
    assert_eq!(policy.calls(), 0, "open must not consult the read policy");
    drop(mount);

    let relative = LocalMount::open(
        Path::new("relative-root"),
        LIBRARY,
        Some(MOUNT.to_string()),
        Caller::new(ReadDecision::Deny),
    );
    assert_eq!(relative.err(), Some(LocalError::InvalidRoot));

    let missing = fixture.path().join("missing");
    let missing = LocalMount::open(
        &missing,
        LIBRARY,
        Some(MOUNT.to_string()),
        Caller::new(ReadDecision::Deny),
    );
    assert_eq!(missing.err(), Some(LocalError::InvalidRoot));

    let file = fixture.path().join("file");
    fs::write(&file, b"x").unwrap();
    let file = LocalMount::open(
        &file,
        LIBRARY,
        Some(MOUNT.to_string()),
        Caller::new(ReadDecision::Deny),
    );
    assert_eq!(file.err(), Some(LocalError::InvalidRoot));

    let real = fixture.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = fixture.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let link = LocalMount::open(
        &link,
        LIBRARY,
        Some(MOUNT.to_string()),
        Caller::new(ReadDecision::Deny),
    );
    assert_eq!(link.err(), Some(LocalError::InvalidRoot));

    assert_eq!(
        LocalMount::open(
            fixture.path(),
            "",
            Some(MOUNT.to_string()),
            Caller::new(ReadDecision::Deny),
        )
        .err(),
        Some(LocalError::InvalidIdentity)
    );
    assert_eq!(
        LocalMount::open(
            fixture.path(),
            "lib\0rary",
            Some(MOUNT.to_string()),
            Caller::new(ReadDecision::Deny),
        )
        .err(),
        Some(LocalError::InvalidIdentity)
    );
    assert_eq!(
        LocalMount::open(
            fixture.path(),
            LIBRARY,
            Some(String::new()),
            Caller::new(ReadDecision::Deny),
        )
        .err(),
        Some(LocalError::InvalidIdentity)
    );

    let denied = fixture.path().join("denied-root");
    fs::create_dir(&denied).unwrap();
    let _guard = ModeGuard::new(&denied, 0);
    let denied = LocalMount::open(
        &denied,
        LIBRARY,
        Some(MOUNT.to_string()),
        Caller::new(ReadDecision::Deny),
    );
    assert_eq!(denied.err(), Some(LocalError::InvalidRoot));
}

/// Capability flags are adapter facts, not a service grant.
///
/// The crate guide reports range reads only. Stable file identity, watch,
/// conditional write, and residency stay false. `false` residency is not a
/// claim that the directory was classified as NAS or cloud.
#[test]
fn capabilities_report_range_reads_without_stable_identity_watch_write_or_residency() {
    let fixture = Fixture::new();
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let caps = mount.capabilities();
    assert_eq!(
        caps,
        Capabilities {
            range_read: true,
            stable_file_id: false,
            reliable_watch: false,
            conditional_write: false,
            residency: false,
        }
    );
}

/// Stat reports metadata from the filesystem and does not read source bytes.
///
/// Storage forbids treating stat as a hash. The size and timestamps are
/// compared with this process's own metadata, not with a second adapter call.
/// Residency stays `Unknown` on this slice. Linux generation is not observed.
#[test]
fn stat_matches_independent_metadata_without_a_content_read() {
    let fixture = Fixture::new();
    let path = fixture.path().join("note.txt");
    fs::write(&path, b"independent-stat-bytes").unwrap();
    warm(&path);
    let facts = facts_of(&path);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let report = observe(&[&path], move || mount.stat(&rel("note.txt"), room()));
    assert!(
        report.admitted,
        "content observer was not admitted; {}",
        report.admit_reason()
    );
    assert!(
        report.content.is_empty(),
        "stat issued a content syscall: {:?}",
        report.content
    );
    let obs = ok_product(report.value);
    assert_eq!(obs.availability, Availability::Available);
    let snapshot = obs.snapshot.expect("available stat keeps a snapshot");
    assert_snapshot(&snapshot, &facts, "note.txt", Some(MOUNT));
}

/// A real empty directory is a complete listing. A zero ceiling is not.
///
/// Storage forbids turning an offline, denied, or incomplete enumeration into
/// a successful empty directory. This fixture has no NAS disconnect, so offline
/// is not stimulated. Denial is decided before the zero-budget result.
#[test]
fn empty_complete_listing_differs_from_zero_budget_and_denial() {
    let fixture = Fixture::new();
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    assert_complete_names(mount.list(None, room()).expect("empty list"), &[]);

    for budget in [
        Budget {
            max_files: 0,
            ..room()
        },
        Budget {
            max_bytes: 0,
            ..room()
        },
        Budget {
            max_millis: 0,
            ..room()
        },
        Budget {
            max_memory_bytes: 0,
            ..room()
        },
    ] {
        assert_partial(
            mount.list(None, budget).expect("zero ceiling"),
            EnumerationStop::BudgetExhausted,
            &[],
        );
    }

    let denied = bind(fixture.path(), Caller::new(ReadDecision::Deny));
    let zero = Budget {
        max_files: 0,
        ..room()
    };
    assert_eq!(
        denied.list(None, zero).err(),
        Some(BackendError::Unavailable(Unavailable::Denied))
    );
}

/// One file slot cannot complete a three-file directory.
///
/// Directory order is not a contract, so the accepted name only has to be one
/// of the three real children. The root itself is not an entry.
#[test]
fn tight_file_budget_does_not_complete_a_nonempty_directory() {
    let fixture = Fixture::new();
    for name in ["one.txt", "two.txt", "three.txt"] {
        fs::write(fixture.path().join(name), name.as_bytes()).unwrap();
    }
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    assert_complete_names(
        mount.list(None, room()).expect("generous list"),
        &["one.txt", "two.txt", "three.txt"],
    );
    let tight = Budget {
        max_files: 1,
        ..room()
    };
    match mount.list(None, tight).expect("tight list") {
        Enumeration::Complete { .. } => panic!("tight budget was reported complete"),
        Enumeration::Partial {
            entries,
            stopped,
            next_cursor,
            ..
        } => {
            assert_eq!(stopped, EnumerationStop::BudgetExhausted);
            assert!(next_cursor.is_none(), "this adapter issues no cursor");
            assert!(entries.len() <= 1, "max_files bounds returned entries");
            for entry in entries {
                assert!(
                    matches!(entry.path.as_str(), "one.txt" | "two.txt" | "three.txt"),
                    "unexpected entry {}",
                    entry.path.as_str()
                );
            }
        }
    }
}

/// Exclusions use a path boundary, and denial is not an existence answer.
///
/// `hidden` excludes that path and its descendants. It does not exclude
/// `hidden-file`. A missing excluded path stays denied. A missing ordinary
/// path is an observed deletion.
#[test]
fn exclusions_honor_path_boundaries_and_do_not_reveal_existence() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.path().join("hidden")).unwrap();
    let secret = fixture.path().join("hidden").join("note.txt");
    fs::write(&secret, b"secret").unwrap();
    fs::write(fixture.path().join("hidden-file"), b"visible").unwrap();
    fs::write(fixture.path().join("other.txt"), b"other").unwrap();
    warm(&secret);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &["hidden"])),
    );
    assert_complete_names(
        mount.list(None, room()).expect("bounded list"),
        &["hidden-file", "other.txt"],
    );
    let report = observe(&[&secret], move || {
        mount.stat(&rel("hidden/note.txt"), room())
    });
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(
        report.content.is_empty(),
        "excluded stat read the child: {:?}",
        report.content
    );
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Denied))
    );

    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &["hidden"])),
    );
    assert_eq!(
        mount.stat(&rel("hidden/missing.txt"), room()).err(),
        Some(BackendError::Unavailable(Unavailable::Denied))
    );
    let deleted = mount
        .stat(&rel("missing-ordinary.txt"), room())
        .expect("missing ordinary path is an observation");
    assert_eq!(
        deleted.availability,
        Availability::Unavailable(Unavailable::Deleted)
    );
}

/// A nested `omni-knowledge.json` stays visible until the caller excludes it.
///
/// The crate guide says this adapter does not discover child libraries or
/// inner mounts. Visibility here is that non-guarantee, not a detector test.
#[test]
fn child_marker_stays_visible_until_the_caller_excludes_it() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.path().join("nested")).unwrap();
    fs::write(
        fixture.path().join("nested").join("omni-knowledge.json"),
        b"{}\n",
    )
    .unwrap();
    fs::write(fixture.path().join("keep.txt"), b"keep").unwrap();
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    assert_complete_names(
        mount.list(None, room()).expect("list"),
        &["keep.txt", "nested", "nested/omni-knowledge.json"],
    );
}

/// A symlink is not followed and its range read does not read the target.
///
/// The expected version is built before the product window. `Unsupported`
/// with zero content calls on the outside file is the requirement. Hits are
/// not subtracted and `read` stays in the filter.
#[test]
fn symlink_is_unsupported_and_does_not_read_the_target() {
    let fixture = Fixture::new();
    let outside = fixture.path().join("outside.txt");
    fs::write(&outside, b"outside-bytes").unwrap();
    warm(&outside);
    let root = fixture.path().join("root");
    fs::create_dir(&root).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
    let mount = bind(&root, Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])));
    assert_partial(
        mount.list(None, room()).expect("symlink list"),
        EnumerationStop::Unsupported,
        &[],
    );
    let stat = mount
        .stat(&rel("escape"), room())
        .expect("symlink stat is an observation");
    assert_eq!(
        stat.availability,
        Availability::Unavailable(Unavailable::Unsupported)
    );
    let mount = bind(&root, Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])));
    let version = version_for(&outside, "escape", Some(MOUNT));
    let range = ByteRange::new(0, 1).unwrap();
    let report = observe(&[&outside], move || {
        mount.open_range(&version, range, room())
    });
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(
        report.content.is_empty(),
        "symlink range read the outside file: {:?} value {:?}",
        report.content,
        report.value.as_ref().map(|_| ())
    );
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Unsupported))
    );
}

/// A non-UTF-8 name stops the listing before any entry is accepted.
#[test]
fn non_utf8_name_stops_the_listing() {
    let fixture = Fixture::new();
    let raw = fixture.path().join(OsStr::from_bytes(b"bad\xffname"));
    fs::write(&raw, b"x").unwrap();
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    assert_partial(
        mount.list(None, room()).expect("non-utf8 list"),
        EnumerationStop::Unsupported,
        &[],
    );
}

/// A socket is an unsupported node, not a regular source.
#[test]
fn socket_is_unsupported() {
    let fixture = Fixture::new();
    let _listener = UnixListener::bind(fixture.path().join("sock")).unwrap();
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    assert_partial(
        mount.list(None, room()).expect("socket list"),
        EnumerationStop::Unsupported,
        &[],
    );
    let stat = mount
        .stat(&rel("sock"), room())
        .expect("socket stat is an observation");
    assert_eq!(
        stat.availability,
        Availability::Unavailable(Unavailable::Unsupported)
    );
}

/// A fifo is refused without a blocking open.
///
/// The product call runs in a child process. The parent kills that child at
/// the harness bound. The bound is process startup plus the call, not a
/// kernel-call timeout owned by the adapter.
#[test]
fn fifo_is_unsupported_and_does_not_block() {
    if std::env::var_os("OMNI_FIFO_WORKER").is_some() {
        fifo_worker();
        return;
    }
    let fixture = Fixture::new();
    let pipe = fixture.path().join("pipe");
    let c_path = CString::new(pipe.as_os_str().as_bytes()).unwrap();
    let rc = unsafe { mkfifo(c_path.as_ptr(), 0o644) };
    assert_eq!(rc, 0, "mkfifo errno {}", std::io::Error::last_os_error());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("fifo_is_unsupported_and_does_not_block")
        .env("OMNI_FIFO_WORKER", fixture.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > HARNESS_BOUND {
            let _ = child.kill();
            let _ = child.wait();
            panic!("fifo call did not return within the harness bound");
        }
        thread::sleep(Duration::from_millis(20));
    };
    if !status.success() {
        let mut err = String::new();
        let _ = child.stderr.take().unwrap().read_to_string(&mut err);
        panic!("fifo worker failed: {status}\n{err}");
    }
}

/// Replacing the bound directory, or changing the admission revision before
/// delivery, returns `Changed` and delivers nothing.
///
/// Storage binds captured work to the opened root. The crate guide checks the
/// policy again before delivery.
#[test]
fn root_replacement_and_policy_revision_deliver_nothing() {
    let fixture = Fixture::new();
    let root = fixture.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("old.txt"), b"old").unwrap();
    let mount = bind(&root, Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])));
    fs::rename(&root, fixture.path().join("displaced")).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("new.txt"), b"new").unwrap();
    assert_eq!(
        mount.list(None, room()).err(),
        Some(BackendError::Unavailable(Unavailable::Changed))
    );

    let root = fixture.path().join("stable");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("body.txt"), b"abcdefghijklmnopqrstuvwxyz").unwrap();
    let facts = facts_of(&root.join("body.txt"));
    let stable = bind(&root, Caller::new(grant(LIBRARY, Some(MOUNT), 7, &[])));
    let snapshot = current_snapshot(&stable, "body.txt", &facts, Some(MOUNT));
    let mount = bind(&root, Flip::new(admit(LIBRARY, Some(MOUNT), 7, &[])));
    let range = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot,
        },
        ByteRange::new(0, 4).unwrap(),
        room(),
    );
    assert_eq!(
        range.err(),
        Some(BackendError::Unavailable(Unavailable::Changed))
    );
}

/// A later call sees the caller's current exclusion set.
///
/// The same revision with a new `outside_scope` is current policy, not a
/// stale grant. A prefix that does not end on a boundary stays visible.
#[test]
fn current_exclusion_is_applied_on_the_next_call() {
    let fixture = Fixture::new();
    fs::write(fixture.path().join("keep.txt"), b"keep").unwrap();
    fs::write(fixture.path().join("secret.txt"), b"secret").unwrap();
    fs::write(fixture.path().join("secret-file"), b"edge").unwrap();
    let policy = Caller::new(grant(LIBRARY, Some(MOUNT), 4, &[]));
    let mount = bind(fixture.path(), &policy);
    assert_complete_names(
        mount.list(None, room()).expect("first list"),
        &["keep.txt", "secret-file", "secret.txt"],
    );
    policy.set(grant(LIBRARY, Some(MOUNT), 4, &["secret.txt"]));
    assert_complete_names(
        mount.list(None, room()).expect("rechecked list"),
        &["keep.txt", "secret-file"],
    );
    assert_eq!(
        mount.stat(&rel("secret.txt"), room()).err(),
        Some(BackendError::Unavailable(Unavailable::Denied))
    );
}

/// Ordinary-local admission is a caller premise. The other decisions grant nothing.
///
/// `NotEstablished` and `Unestablished` are `Unknown`. `Deny` is `Denied` even
/// when the file exists. `Cancel` on list is a partial stop. `Cancel` on stat
/// and range reads is `Unknown` because `BackendError` has no cancel variant.
/// None of these is a NAS or cloud classification.
#[test]
fn unestablished_denied_and_cancelled_premises_are_not_grants() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body.txt");
    fs::write(&path, b"present").unwrap();
    warm(&path);

    let ordinary = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    assert_complete_names(
        ordinary.list(None, room()).expect("ordinary"),
        &["body.txt"],
    );

    let mount = bind(
        fixture.path(),
        Caller::new(ReadDecision::Admit(CurrentAdmission {
            locality: LocalityPremise::NotEstablished,
            ..admit(LIBRARY, Some(MOUNT), 1, &[])
        })),
    );
    let report = observe(&[&path], move || mount.list(None, room()));
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Unknown))
    );

    let mount = bind(fixture.path(), Caller::new(ReadDecision::Unestablished));
    let report = observe(&[&path], move || mount.stat(&rel("body.txt"), room()));
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Unknown))
    );

    let mount = bind(fixture.path(), Caller::new(ReadDecision::Deny));
    let report = observe(&[&path], move || mount.list(None, room()));
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Denied))
    );

    let mount = bind(fixture.path(), Caller::new(ReadDecision::Cancel));
    assert_partial(
        mount.list(None, room()).expect("cancel list"),
        EnumerationStop::Cancelled,
        &[],
    );
    assert_eq!(
        mount.stat(&rel("body.txt"), room()).err(),
        Some(BackendError::Unavailable(Unavailable::Unknown))
    );
    let facts = facts_of(&path);
    let snapshot = StatSnapshot {
        backend: identity(Some(MOUNT)),
        path: rel("body.txt"),
        size_bytes: facts.size,
        modified_ns: Some(facts.modified_ns),
        changed_ns: Some(facts.changed_ns),
        file_id: None,
        backend_version: None,
        residency: Residency::Unknown,
        freshness: Freshness::Current,
    };
    assert_eq!(
        mount
            .open_range(
                &ExpectedVersion {
                    digest: digest(&facts.digest),
                    snapshot,
                },
                ByteRange::new(0, 1).unwrap(),
                room(),
            )
            .err(),
        Some(BackendError::Unavailable(Unavailable::Unknown))
    );
}

/// A cursor, a digest, and a mismatched admission are not read permission.
///
/// This adapter issues no cursor. A presented cursor is `InvalidRequest`.
/// Policy `Deny` is `Unavailable::Denied` and delivers nothing. A foreign
/// library or mount is also a refusal and non-delivery, and its frozen error
/// is `InvalidRequest`, not `Denied`.
#[test]
fn cursor_digest_and_mismatched_admission_are_not_permission() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body.txt");
    fs::write(&path, b"permission-check").unwrap();
    warm(&path);
    let facts = facts_of(&path);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let cursor = Cursor {
        backend: identity(Some(MOUNT)),
        token: "not-a-grant".to_string(),
    };
    assert_eq!(
        mount.list(Some(&cursor), room()).err(),
        Some(BackendError::InvalidRequest)
    );

    let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
    let mount = bind(fixture.path(), Caller::new(ReadDecision::Deny));
    let report = observe(&[&path], move || {
        mount.open_range(
            &ExpectedVersion {
                digest: digest(&facts.digest),
                snapshot,
            },
            ByteRange::new(0, 4).unwrap(),
            room(),
        )
    });
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Denied))
    );

    let mount = bind(
        fixture.path(),
        Caller::new(grant("other-library", Some(MOUNT), 1, &[])),
    );
    let other_library = mount.list(None, room());
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some("other-mount"), 1, &[])),
    );
    let other_mount = mount.stat(&rel("body.txt"), room());
    assert_eq!(
        other_library.as_ref().err(),
        Some(&BackendError::InvalidRequest),
        "other library {other_library:?}; other mount {other_mount:?}"
    );
    assert_eq!(
        other_mount.as_ref().err(),
        Some(&BackendError::InvalidRequest),
        "other mount {other_mount:?}"
    );
}

/// A mode `000` file still has metadata. Its body read is denied.
///
/// The fixture is owned by this unprivileged uid. The mode is restored on cleanup.
/// Stat uses metadata. The range read must not return bytes.
/// The warm-cache control sits outside the bound root: this uid cannot read a
/// mode `000` file, so that file cannot be the observer's positive control.
#[test]
fn mode_zero_file_denies_the_range_without_claiming_absence() {
    let fixture = Fixture::new();
    let control_root = Fixture::new();
    let control = control_root.path().join("control.txt");
    fs::write(&control, b"observer-control").unwrap();
    let path = fixture.path().join("locked.txt");
    fs::write(&path, b"locked-bytes").unwrap();
    let bytes = fs::read(&path).unwrap();
    let _guard = ModeGuard::new(&path, 0);
    let facts = facts_from_bytes(&path, bytes);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let report = observe(&[&control, &path], move || {
        mount.stat(&rel("locked.txt"), room())
    });
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(report.content.is_empty(), "{:?}", report.content);
    let obs = ok_product(report.value);
    assert_eq!(obs.availability, Availability::Available);
    let snapshot = obs.snapshot.expect("mode 000 still has metadata");
    assert_eq!(snapshot.size_bytes, facts.size);

    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "locked.txt", &facts, Some(MOUNT));
    let report = observe(&[&control, &path], move || {
        mount.open_range(
            &ExpectedVersion {
                digest: digest(&facts.digest),
                snapshot,
            },
            ByteRange::new(0, 4).unwrap(),
            room(),
        )
    });
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Denied))
    );
}

/// An interior range returns exactly those independent bytes after a full SHA-256.
///
/// The digest is computed with the locked primitive from the file the test
/// wrote. A range that runs past EOF keeps the overlap and reports EOF.
/// `progress.bytes_read` counts the bytes actually read to hash the source.
#[test]
fn range_read_returns_the_independent_slice_after_full_sha256() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body.txt");
    let bytes = b"AAAAAAAABBBBBBBBCCCCCCCC";
    fs::write(&path, bytes).unwrap();
    warm(&path);
    let facts = facts_of(&path);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
    let expected_digest = facts.digest.clone();
    let report = observe(&[&path], move || {
        mount.open_range(
            &ExpectedVersion {
                digest: digest(&expected_digest),
                snapshot,
            },
            ByteRange::new(8, 8).unwrap(),
            room(),
        )
    });
    assert!(
        report.admitted,
        "observer missed read and pread or was unresolved: {}",
        report.admit_reason()
    );
    assert!(
        !report.content.is_empty(),
        "successful range read was invisible to the content observer; seen {:?}",
        report.seen
    );
    let read = ok_product(report.value);
    assert_eq!(read.bytes, b"BBBBBBBB");
    assert!(!read.end_of_file);
    assert_eq!(read.version.digest.as_str(), facts.digest);
    assert_eq!(read.version.snapshot.size_bytes, facts.size);
    assert!(read.progress.bytes_read >= facts.size);

    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
    let tail = mount
        .open_range(
            &ExpectedVersion {
                digest: digest(&facts.digest),
                snapshot,
            },
            ByteRange::new(20, 8).unwrap(),
            room(),
        )
        .expect("eof overlap");
    assert_eq!(tail.bytes, b"CCCC");
    assert!(tail.end_of_file);
}

/// A byte change outside the returned range fails the old digest.
///
/// The expected snapshot is the metadata observed after the edit, including a
/// restored modification time. The middle bytes are unchanged, so a range
/// return would hide the edit. Storage requires `Changed` rather than mixed bytes.
#[test]
fn bytes_outside_the_range_fail_the_old_digest() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body.txt");
    fs::write(&path, b"AAAAAAAABBBBBBBBCCCCCCCC").unwrap();
    let before = facts_of(&path);
    let original_mtime = fs::metadata(&path).unwrap().modified().unwrap();
    let mut edited = before.bytes.clone();
    edited[0] = b'Z';
    fs::write(&path, &edited).unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(original_mtime)
        .unwrap();
    let after = facts_of(&path);
    assert_ne!(before.digest, after.digest);
    assert_eq!(&after.bytes[8..16], b"BBBBBBBB");
    assert_eq!(after.size, before.size);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "body.txt", &after, Some(MOUNT));
    let read = mount.open_range(
        &ExpectedVersion {
            digest: digest(&before.digest),
            snapshot,
        },
        ByteRange::new(8, 8).unwrap(),
        room(),
    );
    assert_eq!(
        read.err(),
        Some(BackendError::Unavailable(Unavailable::Changed))
    );
}

/// Equal EOF is an empty read. An offset past that end is `InvalidRequest`.
///
/// Drift wins at both positions. The size-equal call is not the past-end case.
/// Charged progress is not pinned as a physical I/O count.
#[test]
fn offset_past_eof_is_invalid_only_while_the_digest_matches() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body.txt");
    fs::write(&path, b"0123456789").unwrap();
    let facts = facts_of(&path);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let equal = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot: current_snapshot(&mount, "body.txt", &facts, Some(MOUNT)),
        },
        ByteRange::new(facts.size, 1).unwrap(),
        room(),
    );
    let equal = equal.expect("offset equal to the verified end");
    assert!(equal.bytes.is_empty(), "{equal:?}");
    assert!(equal.end_of_file, "{equal:?}");
    assert_eq!(equal.version.digest.as_str(), facts.digest);

    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let past = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot: current_snapshot(&mount, "body.txt", &facts, Some(MOUNT)),
        },
        ByteRange::new(facts.size + 1, 1).unwrap(),
        room(),
    );
    assert_eq!(
        past.as_ref().map(|_| ()).err(),
        Some(&BackendError::InvalidRequest),
        "strictly past end {past:?}"
    );

    let mut edited = facts.bytes.clone();
    edited[0] = b'Z';
    fs::write(&path, &edited).unwrap();
    let after = facts_of(&path);
    assert_eq!(
        after.size, facts.size,
        "the edit keeps both offsets defined"
    );
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let drifted_equal = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot: current_snapshot(&mount, "body.txt", &after, Some(MOUNT)),
        },
        ByteRange::new(after.size, 1).unwrap(),
        room(),
    );
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let drifted_past = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot: current_snapshot(&mount, "body.txt", &after, Some(MOUNT)),
        },
        ByteRange::new(after.size + 1, 1).unwrap(),
        room(),
    );
    assert_eq!(
        drifted_equal.as_ref().map(|_| ()).err(),
        Some(&BackendError::Unavailable(Unavailable::Changed)),
        "drift at EOF {drifted_equal:?}"
    );
    assert_eq!(
        drifted_past.as_ref().map(|_| ()).err(),
        Some(&BackendError::Unavailable(Unavailable::Changed)),
        "drift past EOF {drifted_past:?}"
    );
}

/// Whole-source, metadata, and retained-range budgets are separate premises.
///
/// `max_bytes` equal to the source ignores the path spelling. Seventeen memory
/// bytes are the request plus one hash byte and do not include metadata, so
/// they are a refusal while metadata, the retained range, and the hash buffer
/// are live together. The passing `room` range read is the positive control.
/// These ceilings are reservations, not RSS or a syscall deadline.
#[test]
fn range_budgets_charge_the_whole_source_and_one_hash_byte() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body.txt");
    let bytes: Vec<u8> = (0..64).collect();
    fs::write(&path, &bytes).unwrap();
    warm(&path);
    let facts = facts_of(&path);
    let spell_len = u64::try_from(rel("body.txt").as_str().len()).unwrap();
    assert_eq!(facts.size, 64, "source length precondition");
    assert_eq!(spell_len, 8, "body.txt path spelling precondition");
    let source = facts.size;
    let source_and_path = source + spell_len;
    let cases: [(&str, Budget, u64); 8] = [
        (
            "byte ceiling below the source; memory is the passing room budget",
            Budget {
                max_bytes: source - 1,
                ..room()
            },
            16,
        ),
        (
            "byte ceiling equals the source and ignores the path; memory is room",
            Budget {
                max_bytes: source,
                ..room()
            },
            16,
        ),
        (
            "byte ceiling is one below source plus path; memory is room",
            Budget {
                max_bytes: source_and_path - 1,
                ..room()
            },
            16,
        ),
        (
            "memory is the request plus one hash byte and omits live metadata",
            Budget {
                max_bytes: source_and_path,
                max_memory_bytes: 17,
                ..room()
            },
            16,
        ),
        (
            "memory is below the request plus one hash byte",
            Budget {
                max_memory_bytes: 16,
                ..room()
            },
            16,
        ),
        (
            "zero memory ceiling",
            Budget {
                max_memory_bytes: 0,
                ..room()
            },
            16,
        ),
        (
            "zero file ceiling",
            Budget {
                max_files: 0,
                ..room()
            },
            16,
        ),
        (
            "requested length exceeds a byte ceiling that already covers source plus path",
            Budget {
                max_bytes: source_and_path,
                ..room()
            },
            80,
        ),
    ];
    for (reason, budget, length) in cases {
        let mount = bind(
            fixture.path(),
            Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
        );
        let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
        let expected = ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot,
        };
        let report = observe(&[&path], move || {
            mount.open_range(&expected, ByteRange::new(0, length).unwrap(), budget)
        });
        assert!(report.admitted, "{reason}: {}", report.admit_reason());
        assert!(
            report.content.is_empty(),
            "{reason}: refusal still read the file: {:?}",
            report.content
        );
        assert_eq!(
            report.value.as_ref().err(),
            Some(&BackendError::BudgetExhausted),
            "{reason}"
        );
    }

    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
    let report = observe(&[&path], move || {
        mount.open_range(
            &ExpectedVersion {
                digest: digest(&facts.digest),
                snapshot,
            },
            ByteRange::new(0, 16).unwrap(),
            Budget {
                max_bytes: source_and_path,
                ..room()
            },
        )
    });
    assert!(
        report.admitted,
        "source-plus-path success: {}",
        report.admit_reason()
    );
    assert!(
        !report.content.is_empty(),
        "source-plus-path range read was invisible: {}",
        report.admit_reason()
    );
    let read = ok_product(report.value);
    assert_eq!(read.bytes, &bytes[..16]);
}

/// Room memory still returns the caller-created library id, and the probe sees the copy.
///
/// The id is not an identity-format quota. Policy values are moved from storage
/// allocated before the measured `stat`.
#[test]
fn large_library_id_is_visible_when_memory_can_hold_it() {
    let fixture = Fixture::new();
    let path = fixture.path().join("n");
    fs::write(&path, b"z").unwrap();
    let big = "a".repeat(8_192);
    let policy = StoredAdmit::new(&big, Some(MOUNT), 1, 32);
    let mount = LocalMount::open(fixture.path(), &big, Some(MOUNT.to_string()), &policy)
        .expect("large library id binds");
    assert_eq!(policy.calls(), 0, "open consulted policy");
    let (result, note) = measure(|| mount.stat(&rel("n"), room()));
    let obs = result.expect("room memory admits the 8192-byte library id");
    assert_eq!(obs.availability, Availability::Available);
    let snapshot = obs.snapshot.expect("snapshot");
    assert_eq!(snapshot.backend.library_id, big);
    assert!(
        note.count >= 1,
        "probe gap: room stat did not allocate the 8192-byte library id; count={} max={}",
        note.count,
        note.max
    );
}

/// A 512-byte memory budget must refuse the 8192-byte library id before copying it.
#[test]
fn large_library_id_under_512_bytes_refuses_before_the_copy() {
    let fixture = Fixture::new();
    let path = fixture.path().join("n");
    fs::write(&path, b"z").unwrap();
    let big = "a".repeat(8_192);
    let policy = StoredAdmit::new(&big, Some(MOUNT), 1, 32);
    let mount = LocalMount::open(fixture.path(), &big, Some(MOUNT.to_string()), &policy)
        .expect("large library id binds");
    assert_eq!(policy.calls(), 0, "open consulted policy");
    let budget = Budget {
        max_memory_bytes: 512,
        max_files: 64,
        max_bytes: 1 << 20,
        max_millis: 5_000,
    };
    let (result, note) = measure(|| mount.stat(&rel("n"), budget));
    let shape = match &result {
        Ok(obs) => format!(
            "Ok {:?} id_len={}",
            obs.availability,
            obs.snapshot
                .as_ref()
                .map(|snap| snap.backend.library_id.len())
                .unwrap_or(0)
        ),
        Err(err) => format!("Err {err:?}"),
    };
    assert!(
        matches!(result, Err(BackendError::BudgetExhausted)) && note.count == 0,
        "512-byte budget must refuse before copying the 8192-byte library id; {shape}; large={} max={}",
        note.count,
        note.max
    );
}

/// One positive memory byte is not an empty complete listing.
///
/// The room-budget listing of the same empty directory is the fixture control.
/// A zero ceiling is a separate case.
#[test]
fn empty_list_with_one_byte_of_memory_is_not_complete() {
    let fixture = Fixture::new();
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    assert_complete_names(mount.list(None, room()).expect("room list"), &[]);
    let listed = mount.list(
        None,
        Budget {
            max_memory_bytes: 1,
            ..room()
        },
    );
    match listed {
        Ok(Enumeration::Complete { .. }) => {
            panic!("room control was complete and one positive memory byte also returned Complete");
        }
        Ok(Enumeration::Partial {
            entries,
            stopped,
            next_cursor,
            ..
        }) => {
            assert!(entries.is_empty(), "{entries:?}");
            assert_eq!(stopped, EnumerationStop::BudgetExhausted);
            assert!(next_cursor.is_none());
        }
        Err(BackendError::BudgetExhausted) => {}
        other => panic!("expected a budget refusal, got {other:?}"),
    }
}

/// A descriptor that cannot be resolved is not a quiet miss.
#[test]
fn unresolved_descriptor_is_not_a_quiet_miss() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body");
    fs::write(&path, b"id").unwrap();
    let watched = [watch_id(&path)];
    let tid = u32::try_from(current_tid()).unwrap();
    assert_eq!(classify_fd(tid, 1_000_000, &watched), FdClass::Unresolved);
    assert_eq!(classify_fd(tid, -1, &watched), FdClass::Anonymous);
    let file = File::open(&path).unwrap();
    assert_eq!(classify_fd(tid, file.as_raw_fd(), &watched), FdClass::Hit);
    let other = fixture.path().join("other");
    fs::write(&other, b"no").unwrap();
    let other_file = File::open(&other).unwrap();
    assert_eq!(
        classify_fd(tid, other_file.as_raw_fd(), &watched),
        FdClass::Other
    );
}

#[test]
fn observer_normal_completion_is_quiet() {
    if observer_child_entered("normal") {
        return;
    }
    let outcome = spawn_observer_child("observer_normal_completion_is_quiet", "normal", None);
    assert_child_exit(&outcome, Some(0));
    assert!(outcome.marker.contains("mode=normal"), "{}", outcome.marker);
    assert!(!outcome.marker.contains("OBSERVER_SETTLED"));
    assert_no_late_work(&outcome);
}

#[test]
fn observer_deliberate_content_read_is_visible() {
    if observer_child_entered("content") {
        return;
    }
    let outcome = spawn_observer_child(
        "observer_deliberate_content_read_is_visible",
        "content",
        None,
    );
    assert_child_exit(&outcome, Some(0));
    assert!(
        outcome.marker.contains("mode=content"),
        "{}",
        outcome.marker
    );
    assert!(!outcome.marker.contains("OBSERVER_SETTLED"));
    assert_no_late_work(&outcome);
}

#[test]
fn observer_action_panic_settles_before_unwind() {
    if observer_child_entered("panic") {
        return;
    }
    let outcome =
        spawn_observer_child("observer_action_panic_settles_before_unwind", "panic", None);
    assert!(
        outcome.status.code().is_some() && !outcome.status.success(),
        "panic child status {}",
        outcome.status
    );
    assert!(
        outcome.marker.contains("OBSERVER_SETTLED"),
        "{}",
        outcome.marker
    );
    assert_no_late_work(&outcome);
}

#[test]
fn observer_timeout_exits_without_closing_the_listener() {
    if observer_child_entered("timeout") {
        return;
    }
    let outcome = spawn_observer_child(
        "observer_timeout_exits_without_closing_the_listener",
        "timeout",
        Some(800),
    );
    assert_eq!(outcome.status.code(), Some(97), "{}", outcome.status);
    assert!(
        outcome.marker.contains("mode=timeout"),
        "{}",
        outcome.marker
    );
    assert!(
        !outcome.marker.contains("OBSERVER_SETTLED"),
        "timeout joined in userspace: {}",
        outcome.marker
    );
    let fixture = child_fixture(&outcome);
    assert!(
        fixture.exists(),
        "timeout child ran destructors at {}",
        fixture.display()
    );
    assert!(fixture.join("ready").is_file(), "ready marker missing");
    assert!(!fixture.join("late").exists(), "late fixture work");
    let _ = fs::remove_dir_all(&fixture);
}

fn fifo_worker() {
    let root = PathBuf::from(std::env::var_os("OMNI_FIFO_WORKER").unwrap());
    let mount = bind(&root, Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])));
    assert_partial(
        mount.list(None, room()).expect("fifo list"),
        EnumerationStop::Unsupported,
        &[],
    );
    let stat = mount
        .stat(&rel("pipe"), room())
        .expect("fifo stat is an observation");
    assert_eq!(
        stat.availability,
        Availability::Unavailable(Unavailable::Unsupported)
    );
    let range = mount.open_range(
        &ExpectedVersion {
            digest: digest(&sha256_hex(b"not-the-fifo")),
            snapshot: StatSnapshot {
                backend: identity(Some(MOUNT)),
                path: rel("pipe"),
                size_bytes: 0,
                modified_ns: None,
                changed_ns: None,
                file_id: None,
                backend_version: None,
                residency: Residency::Unknown,
                freshness: Freshness::Unknown,
            },
        },
        ByteRange::new(0, 1).unwrap(),
        room(),
    );
    assert_eq!(
        range.err(),
        Some(BackendError::Unavailable(Unavailable::Unsupported))
    );
}

struct FileFacts {
    bytes: Vec<u8>,
    digest: String,
    size: u64,
    modified_ns: i128,
    changed_ns: i128,
    birth_ns: Option<i128>,
}

fn facts_of(path: &Path) -> FileFacts {
    facts_from_bytes(path, fs::read(path).unwrap())
}

fn facts_from_bytes(path: &Path, bytes: Vec<u8>) -> FileFacts {
    let meta = fs::metadata(path).unwrap();
    let modified_ns = stamp(meta.mtime(), meta.mtime_nsec());
    let changed_ns = stamp(meta.ctime(), meta.ctime_nsec());
    FileFacts {
        digest: sha256_hex(&bytes),
        size: meta.len(),
        birth_ns: birth_ns(path, meta.len(), modified_ns),
        bytes,
        modified_ns,
        changed_ns,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn stamp(secs: i64, nsec: i64) -> i128 {
    i128::from(secs) * 1_000_000_000 + i128::from(nsec)
}

fn birth_ns(path: &Path, size: u64, modified_ns: i128) -> Option<i128> {
    let observed = statx_observed(path)?;
    assert_eq!(observed.size, size, "statx size precondition");
    assert_eq!(observed.mtime_ns, modified_ns, "statx mtime precondition");
    observed.birth_ns
}

struct StatxObserved {
    size: u64,
    mtime_ns: i128,
    birth_ns: Option<i128>,
}

fn statx_observed(path: &Path) -> Option<StatxObserved> {
    let mut buf = std::mem::MaybeUninit::<Statx>::zeroed();
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let rc = unsafe {
        syscall(
            332,
            -100i64,
            c_path.as_ptr(),
            0x100i64,
            0xfffi64,
            buf.as_mut_ptr(),
        )
    };
    if rc < 0 {
        return None;
    }
    let statx = unsafe { buf.assume_init() };
    let mtime_ns = stamp(statx.stx_mtime.tv_sec, i64::from(statx.stx_mtime.tv_nsec));
    let birth_ns = if statx.stx_mask & 0x800 != 0 {
        let birth = stamp(statx.stx_btime.tv_sec, i64::from(statx.stx_btime.tv_nsec));
        (birth != 0).then_some(birth)
    } else {
        None
    };
    Some(StatxObserved {
        size: statx.stx_size,
        mtime_ns,
        birth_ns,
    })
}

#[repr(C)]
struct StatxTimestamp {
    tv_sec: i64,
    tv_nsec: u32,
    pad: i32,
}

#[repr(C)]
struct Statx {
    stx_mask: u32,
    stx_blksize: u32,
    stx_attributes: u64,
    stx_nlink: u32,
    stx_uid: u32,
    stx_gid: u32,
    stx_mode: u16,
    spare0: u16,
    stx_ino: u64,
    stx_size: u64,
    stx_blocks: u64,
    stx_attributes_mask: u64,
    stx_atime: StatxTimestamp,
    stx_btime: StatxTimestamp,
    stx_ctime: StatxTimestamp,
    stx_mtime: StatxTimestamp,
    stx_rdev_major: u32,
    stx_rdev_minor: u32,
    stx_dev_major: u32,
    stx_dev_minor: u32,
    stx_mnt_id: u64,
    spare2: [u64; 13],
}

fn room() -> Budget {
    Budget {
        max_files: 64,
        max_bytes: 1 << 20,
        max_millis: 5_000,
        max_memory_bytes: 1 << 20,
    }
}

fn rel(path: &str) -> RelativePath {
    RelativePath::new(path).expect("precondition: path spelling is a relative path")
}

fn digest(hex: &str) -> ContentDigest {
    ContentDigest::new(hex).expect("precondition: lowercase sha256 spelling")
}

fn identity(mount: Option<&str>) -> BackendIdentity {
    BackendIdentity {
        library_id: LIBRARY.to_string(),
        mount_id: mount.map(str::to_string),
        backend: BackendKind::Local,
    }
}

fn grant(library: &str, mount: Option<&str>, revision: u64, outside: &[&str]) -> ReadDecision {
    ReadDecision::Admit(admit(library, mount, revision, outside))
}

fn admit(library: &str, mount: Option<&str>, revision: u64, outside: &[&str]) -> CurrentAdmission {
    CurrentAdmission {
        library_id: library.to_string(),
        mount_id: mount.map(str::to_string),
        revision,
        locality: LocalityPremise::OrdinaryLocal,
        outside_scope: outside.iter().copied().map(rel).collect(),
    }
}

fn bind<P: LocalReadPolicy>(path: &Path, policy: P) -> LocalMount<P> {
    LocalMount::open(path, LIBRARY, Some(MOUNT.to_string()), policy)
        .expect("precondition: fixture directory binds")
}

fn warm(path: &Path) {
    let _ = fs::read(path).unwrap();
}

fn current_snapshot<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    path: &str,
    facts: &FileFacts,
    mount_id: Option<&str>,
) -> StatSnapshot {
    let obs = mount.stat(&rel(path), room()).expect("current stat");
    assert_eq!(obs.availability, Availability::Available);
    let snapshot = obs.snapshot.expect("snapshot");
    assert_snapshot(&snapshot, facts, path, mount_id);
    snapshot
}

fn assert_snapshot(snapshot: &StatSnapshot, facts: &FileFacts, path: &str, mount_id: Option<&str>) {
    assert_eq!(snapshot.backend.library_id, LIBRARY);
    assert_eq!(snapshot.backend.mount_id.as_deref(), mount_id);
    assert_eq!(snapshot.backend.backend, BackendKind::Local);
    assert_eq!(snapshot.path.as_str(), path);
    assert_eq!(snapshot.size_bytes, facts.size);
    assert_eq!(snapshot.modified_ns, Some(facts.modified_ns));
    assert_eq!(snapshot.changed_ns, Some(facts.changed_ns));
    assert_eq!(snapshot.residency, Residency::Unknown);
    assert_eq!(snapshot.freshness, Freshness::Current);
    match &snapshot.file_id {
        Some(id) => {
            assert!(id.generation.is_none(), "Linux generation is not observed");
            assert_eq!(id.birth_ns, facts.birth_ns);
            if facts.birth_ns.is_none() {
                panic!("file id evidence was copied without birth or generation");
            }
        }
        None => assert!(
            facts.birth_ns.is_none(),
            "birth was reported and file id evidence was omitted"
        ),
    }
}

fn assert_complete_names(enumeration: Enumeration, names: &[&str]) {
    match enumeration {
        Enumeration::Complete { entries, .. } => {
            let mut got: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
            let mut expect = names.to_vec();
            got.sort_unstable();
            expect.sort_unstable();
            assert_eq!(got, expect);
            assert!(entries.iter().all(|entry| matches!(
                entry.kind,
                knowledge_core::backend::EntryKind::File
                    | knowledge_core::backend::EntryKind::Directory
            )));
        }
        Enumeration::Partial { stopped, .. } => panic!("complete listing stopped as {stopped:?}"),
    }
}

fn assert_partial(enumeration: Enumeration, stop: EnumerationStop, names: &[&str]) {
    match enumeration {
        Enumeration::Partial {
            entries,
            stopped,
            next_cursor,
            ..
        } => {
            assert_eq!(stopped, stop);
            assert!(next_cursor.is_none(), "this adapter issues no cursor");
            let got: Vec<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
            assert_eq!(got, names);
        }
        Enumeration::Complete { .. } => panic!("partial result was complete"),
    }
}

fn ok_product<T: std::fmt::Debug>(value: Result<T, BackendError>) -> T {
    match value {
        Ok(value) => value,
        Err(error) => panic!("expected a product observation, got {error:?}"),
    }
}

fn version_for(path: &Path, relative: &str, mount: Option<&str>) -> ExpectedVersion {
    let facts = facts_of(path);
    ExpectedVersion {
        digest: digest(&facts.digest),
        snapshot: StatSnapshot {
            backend: identity(mount),
            path: rel(relative),
            size_bytes: facts.size,
            modified_ns: Some(facts.modified_ns),
            changed_ns: Some(facts.changed_ns),
            file_id: None,
            backend_version: None,
            residency: Residency::Unknown,
            freshness: Freshness::Current,
        },
    }
}

struct Caller {
    decision: Mutex<ReadDecision>,
    calls: AtomicUsize,
}

impl Caller {
    fn new(decision: ReadDecision) -> Self {
        Self {
            decision: Mutex::new(decision),
            calls: AtomicUsize::new(0),
        }
    }

    fn set(&self, decision: ReadDecision) {
        *self.decision.lock().expect("policy") = decision;
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl LocalReadPolicy for Caller {
    fn decide(&self) -> ReadDecision {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.decision.lock().expect("policy").clone()
    }
}

impl LocalReadPolicy for &Caller {
    fn decide(&self) -> ReadDecision {
        (*self).decide()
    }
}

/// Admissions allocated before a measured call. `decide` moves one and does not copy it.
struct StoredAdmit {
    decisions: Mutex<Vec<ReadDecision>>,
    calls: AtomicUsize,
}

impl StoredAdmit {
    fn new(library: &str, mount: Option<&str>, revision: u64, copies: usize) -> Self {
        let decisions = (0..copies)
            .map(|_| grant(library, mount, revision, &[]))
            .collect();
        let stored = Self {
            decisions: Mutex::new(decisions),
            calls: AtomicUsize::new(0),
        };
        let held = stored.decisions.lock().expect("policy");
        assert_eq!(held.len(), copies);
        drop(held);
        stored
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn take(&self) -> ReadDecision {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.decisions
            .lock()
            .expect("policy")
            .pop()
            .expect("prebuilt admission")
    }
}

impl LocalReadPolicy for StoredAdmit {
    fn decide(&self) -> ReadDecision {
        self.take()
    }
}

impl LocalReadPolicy for &StoredAdmit {
    fn decide(&self) -> ReadDecision {
        self.take()
    }
}

struct Flip {
    base: CurrentAdmission,
    calls: AtomicUsize,
}

impl Flip {
    fn new(base: CurrentAdmission) -> Self {
        Self {
            base,
            calls: AtomicUsize::new(0),
        }
    }
}

impl LocalReadPolicy for Flip {
    fn decide(&self) -> ReadDecision {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let mut admission = self.base.clone();
        admission.revision = if call == 0 { 7 } else { 8 };
        ReadDecision::Admit(admission)
    }
}

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let id = FIXTURES.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("omni-local-{}-{id}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        relax_tree(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct ModeGuard {
    path: PathBuf,
    mode: u32,
}

impl ModeGuard {
    fn new(path: &Path, mode: u32) -> Self {
        let original = fs::symlink_metadata(path).unwrap().mode() & 0o777;
        fs::set_permissions(path, Permissions::from_mode(mode)).unwrap();
        Self {
            path: path.to_path_buf(),
            mode: original,
        }
    }
}

impl Drop for ModeGuard {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, Permissions::from_mode(self.mode));
    }
}

fn relax_tree(path: &Path) {
    let _ = fs::set_permissions(path, Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        let Ok(meta) = fs::symlink_metadata(&child) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            relax_tree(&child);
        } else {
            let _ = fs::set_permissions(&child, Permissions::from_mode(0o644));
        }
    }
}

struct ContentHit {
    nr: i32,
    kind: &'static str,
}

struct Report<T> {
    value: T,
    admitted: bool,
    content: Vec<ContentHit>,
    seen: Vec<i32>,
    unresolved: bool,
    before: Vec<ContentHit>,
    after: Vec<ContentHit>,
}

impl<T> Report<T> {
    fn admit_reason(&self) -> String {
        format!(
            "unresolved={} before={:?} after={:?} product={:?} seen={:?}",
            self.unresolved, self.before, self.after, self.content, self.seen
        )
    }
}

struct HitLog {
    hits: Mutex<Vec<ContentHit>>,
    seen: Mutex<Vec<i32>>,
    overflow: AtomicBool,
}

impl HitLog {
    fn new() -> Self {
        Self {
            hits: Mutex::new(Vec::with_capacity(256)),
            seen: Mutex::new(Vec::with_capacity(256)),
            overflow: AtomicBool::new(false),
        }
    }

    fn push_seen(&self, nr: i32) {
        let mut seen = self.seen.lock().expect("seen");
        if seen.len() < seen.capacity() {
            seen.push(nr);
        } else {
            self.overflow.store(true, Ordering::SeqCst);
        }
    }

    fn push_hit(&self, hit: ContentHit) {
        let mut hits = self.hits.lock().expect("hits");
        if hits.len() < hits.capacity() {
            hits.push(hit);
        } else {
            self.overflow.store(true, Ordering::SeqCst);
        }
    }
}

/// Run `action` on one worker thread whose content syscalls are observed.
///
/// Coverage is the x86-64 `read`, `pread64`, `readv`, `preadv`, `preadv2`,
/// file `mmap`, `sendfile`, `splice`, and `copy_file_range` calls of that
/// thread. `openat` is continued and is not a content hit. Stat metadata,
/// `io_uring`, other threads, and descendants are outside this coverage.
/// Every trapped call is continued. Before and after controls must both
/// `read` and `pread64` one byte. An unresolved identity is not a quiet pass.
/// This is not `/proc/self/io` physical-byte accounting.
fn observe<T: Send + 'static>(
    watched: &[&Path],
    action: impl FnOnce() -> T + Send + 'static,
) -> Report<T> {
    let ids: Vec<WatchId> = watched.iter().copied().map(watch_id).collect();
    let primary = watched[0].to_path_buf();
    warm(&primary);
    let admission = run_filtered(&ids, {
        let primary = primary.clone();
        move || admit_read(&primary)
    });
    let product = run_filtered(&ids, action);
    let again = run_filtered(&ids, move || admit_read(&primary));
    let unresolved = admission.unresolved || product.unresolved || again.unresolved;
    let admitted =
        control_ok(&admission) && control_ok(&again) && !product.overflow && !product.unresolved;
    Report {
        value: product.value,
        admitted,
        content: product.hits,
        seen: product.seen,
        unresolved,
        before: admission.hits,
        after: again.hits,
    }
}

fn control_ok<T>(window: &Window<T>) -> bool {
    !window.overflow
        && !window.unresolved
        && window.hits.iter().any(|hit| hit.nr == 0)
        && window.hits.iter().any(|hit| hit.nr == 17)
}

fn admit_read(path: &Path) {
    let mut file = File::open(path).expect("admission open");
    let mut buf = [0u8; 1];
    file.read_exact(&mut buf).expect("admission read");
    let rc = unsafe {
        syscall(
            17,
            i64::from(file.as_raw_fd()),
            buf.as_mut_ptr() as i64,
            1i64,
            0i64,
        )
    };
    assert_eq!(rc, 1, "admission pread");
}

struct Window<T> {
    hits: Vec<ContentHit>,
    value: T,
    overflow: bool,
    seen: Vec<i32>,
    unresolved: bool,
}

fn observer_bound() -> Duration {
    std::env::var("OMNI_OBSERVER_BOUND_MS")
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(HARNESS_BOUND)
}

fn run_filtered<T: Send + 'static>(
    watched: &[WatchId],
    action: impl FnOnce() -> T + Send + 'static,
) -> Window<T> {
    let log = Arc::new(HitLog::new());
    let unresolved = Arc::new(AtomicBool::new(false));
    let (fd_tx, fd_rx) = std::sync::mpsc::sync_channel::<(OwnedFd, u64)>(1);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (go_tx, go_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (value_tx, value_rx) = std::sync::mpsc::sync_channel(1);
    let (done_read, done_write) = pipe_cloexec();
    let watched = watched.to_vec();
    let observer_log = Arc::clone(&log);
    let observer_unresolved = Arc::clone(&unresolved);
    let observer = thread::spawn(move || {
        let Ok((listener, tid)) = fd_rx.recv() else {
            return;
        };
        let _ = ready_tx.send(());
        observe_loop(
            listener,
            done_read,
            tid,
            &watched,
            &observer_log,
            &observer_unresolved,
        );
    });
    let worker = thread::spawn(move || {
        let raw = match install_listener() {
            Ok(fd) => fd,
            Err(_) => return,
        };
        let listener = unsafe { OwnedFd::from_raw_fd(raw) };
        let tid = current_tid();
        if fd_tx.send((listener, tid)).is_err() {
            return;
        }
        if go_rx.recv().is_err() {
            return;
        }
        let outcome = panic::catch_unwind(AssertUnwindSafe(action));
        let _ = value_tx.send(outcome);
        drop(done_write);
    });
    if ready_rx.recv_timeout(Duration::from_secs(5)).is_err() {
        drop(go_tx);
        let _ = worker.join();
        let _ = observer.join();
        panic!("seccomp listener was not installed");
    }
    if go_tx.send(()).is_err() {
        let _ = worker.join();
        let _ = observer.join();
        panic!("worker exited before the observed action");
    }
    let outcome = match value_rx.recv_timeout(observer_bound()) {
        Ok(outcome) => outcome,
        Err(_) => exit_group(97),
    };
    let worker_result = worker.join();
    let observer_result = observer.join();
    let value = match outcome {
        Ok(value) => {
            worker_result.expect("worker");
            observer_result.expect("observer");
            value
        }
        Err(payload) => {
            let _ = worker_result;
            let _ = observer_result;
            note_settled();
            panic::resume_unwind(payload);
        }
    };
    Window {
        hits: std::mem::take(&mut *log.hits.lock().expect("hits")),
        value,
        overflow: log.overflow.load(Ordering::SeqCst),
        seen: std::mem::take(&mut *log.seen.lock().expect("seen")),
        unresolved: unresolved.load(Ordering::SeqCst),
    }
}

const POLLIN: i16 = 1;
const POLLERR: i16 = 8;
const POLLHUP: i16 = 0x10;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

fn observe_loop(
    listener: OwnedFd,
    done: OwnedFd,
    worker_tid: u64,
    watched: &[WatchId],
    log: &HitLog,
    unresolved: &AtomicBool,
) {
    loop {
        let mut polls = [
            PollFd {
                fd: listener.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
            PollFd {
                fd: done.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];
        let rc = unsafe { syscall(7, polls.as_mut_ptr(), 2i64, 250i64) };
        if rc < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(4) {
                continue;
            }
            unresolved.store(true, Ordering::SeqCst);
            break;
        }
        if polls[0].revents & POLLIN != 0
            && !recv_one(listener.as_raw_fd(), worker_tid, watched, log, unresolved)
        {
            unresolved.store(true, Ordering::SeqCst);
            break;
        }
        if polls[1].revents & (POLLIN | POLLERR | POLLHUP) != 0 {
            drain_listener(listener.as_raw_fd(), worker_tid, watched, log, unresolved);
            break;
        }
    }
}

fn drain_listener(
    listener: i32,
    worker_tid: u64,
    watched: &[WatchId],
    log: &HitLog,
    unresolved: &AtomicBool,
) {
    loop {
        let mut polls = [PollFd {
            fd: listener,
            events: POLLIN,
            revents: 0,
        }];
        let rc = unsafe { syscall(7, polls.as_mut_ptr(), 1i64, 0i64) };
        if rc < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(4) {
                continue;
            }
            unresolved.store(true, Ordering::SeqCst);
            break;
        }
        if rc == 0 || polls[0].revents & POLLIN == 0 {
            break;
        }
        if !recv_one(listener, worker_tid, watched, log, unresolved) {
            unresolved.store(true, Ordering::SeqCst);
            break;
        }
    }
}

fn recv_one(
    listener: i32,
    worker_tid: u64,
    watched: &[WatchId],
    log: &HitLog,
    unresolved: &AtomicBool,
) -> bool {
    let mut note = std::mem::MaybeUninit::<SeccompNotif>::zeroed();
    let rc = unsafe { ioctl(listener, 0xc050_2100, note.as_mut_ptr()) };
    if rc < 0 {
        return false;
    }
    let note = unsafe { note.assume_init() };
    log.push_seen(note.data.nr);
    if u64::from(note.pid) != worker_tid {
        unresolved.store(true, Ordering::SeqCst);
    } else if let Some(kind) = content_kind(note.data.nr) {
        let file_fd = file_descriptor(note.data.nr, &note.data.args);
        match classify_fd(note.pid, file_fd, watched) {
            FdClass::Hit => log.push_hit(ContentHit {
                nr: note.data.nr,
                kind,
            }),
            FdClass::Other | FdClass::Anonymous => {}
            FdClass::Unresolved => unresolved.store(true, Ordering::SeqCst),
        }
    }
    let response = SeccompResp {
        id: note.id,
        val: 0,
        error: 0,
        flags: 1,
    };
    unsafe { ioctl(listener, 0xc018_2101, &response) >= 0 }
}

fn content_kind(nr: i32) -> Option<&'static str> {
    match nr {
        0 | 17 | 19 | 295 | 327 => Some("read"),
        9 => Some("mmap"),
        40 | 275 | 326 => Some("transfer"),
        _ => None,
    }
}

fn file_descriptor(nr: i32, args: &[u64; 6]) -> i32 {
    let raw = match nr {
        40 => args[1],
        9 => args[4],
        _ => args[0],
    };
    raw as i32
}

#[derive(Clone, Copy)]
struct WatchId {
    dev: u64,
    ino: u64,
}

fn watch_id(path: &Path) -> WatchId {
    let meta = fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("watch identity {}: {error}", path.display()));
    WatchId {
        dev: meta.dev(),
        ino: meta.ino(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FdClass {
    Hit,
    Other,
    Unresolved,
    Anonymous,
}

fn classify_fd(tid: u32, fd: i32, watched: &[WatchId]) -> FdClass {
    if fd < 0 {
        return FdClass::Anonymous;
    }
    let mut buf = [0u8; 64];
    let Some(len) = write_proc_fd(&mut buf, tid, fd) else {
        return FdClass::Unresolved;
    };
    match fs::metadata(OsStr::from_bytes(&buf[..len])) {
        Ok(meta) => {
            let dev = meta.dev();
            let ino = meta.ino();
            if watched.iter().any(|id| id.dev == dev && id.ino == ino) {
                FdClass::Hit
            } else {
                FdClass::Other
            }
        }
        Err(_) => FdClass::Unresolved,
    }
}

fn write_proc_fd(buf: &mut [u8], pid: u32, fd: i32) -> Option<usize> {
    const PREFIX: &[u8] = b"/proc/";
    const MIDDLE: &[u8] = b"/fd/";
    let mut cursor = 0;
    for byte in PREFIX {
        *buf.get_mut(cursor)? = *byte;
        cursor += 1;
    }
    cursor = write_u32(buf, cursor, pid)?;
    for byte in MIDDLE {
        *buf.get_mut(cursor)? = *byte;
        cursor += 1;
    }
    cursor = write_u32(buf, cursor, u32::try_from(fd).ok()?)?;
    *buf.get_mut(cursor)? = 0;
    Some(cursor)
}

fn write_u32(buf: &mut [u8], mut cursor: usize, mut value: u32) -> Option<usize> {
    let mut digits = [0u8; 10];
    let mut count = 0;
    loop {
        digits[count] = b'0' + u8::try_from(value % 10).ok()?;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    while count > 0 {
        count -= 1;
        *buf.get_mut(cursor)? = digits[count];
        cursor += 1;
    }
    Some(cursor)
}

fn install_listener() -> std::io::Result<i32> {
    let no_new = unsafe { syscall(157, 38i64, 1i64, 0i64, 0i64, 0i64) };
    if no_new < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let filters = content_filter();
    let prog = SockFprog {
        len: u16::try_from(filters.len()).unwrap(),
        filter: filters.as_ptr(),
    };
    let fd = unsafe { syscall(317, 1i64, 8i64, &prog as *const SockFprog as i64) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let owned = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    let cloexec = unsafe { syscall(72, i64::from(owned.as_raw_fd()), 2i64, 1i64) };
    if cloexec < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(owned.into_raw_fd())
}

fn content_filter() -> Vec<SockFilter> {
    let syscalls = [0u32, 9, 17, 19, 40, 257, 275, 295, 326, 327, 437];
    let n = syscalls.len();
    let allow_at = 3 + n;
    let notif_at = 4 + n;
    let mut filters = Vec::with_capacity(notif_at + 1);
    filters.push(SockFilter {
        code: 0x20,
        jt: 0,
        jf: 0,
        k: 4,
    });
    filters.push(SockFilter {
        code: 0x15,
        jt: 0,
        jf: u8::try_from(allow_at - 2).unwrap(),
        k: 0xc000_003e,
    });
    filters.push(SockFilter {
        code: 0x20,
        jt: 0,
        jf: 0,
        k: 0,
    });
    for (index, nr) in syscalls.iter().enumerate() {
        let at = 3 + index;
        filters.push(SockFilter {
            code: 0x15,
            jt: u8::try_from(notif_at - (at + 1)).unwrap(),
            jf: 0,
            k: *nr,
        });
    }
    filters.push(SockFilter {
        code: 0x06,
        jt: 0,
        jf: 0,
        k: 0x7fff_0000,
    });
    filters.push(SockFilter {
        code: 0x06,
        jt: 0,
        jf: 0,
        k: 0x7fc0_0000,
    });
    filters
}

#[repr(C)]
struct SockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct SockFprog {
    len: u16,
    filter: *const SockFilter,
}

#[repr(C)]
struct SeccompData {
    nr: i32,
    arch: u32,
    instruction_pointer: u64,
    args: [u64; 6],
}

#[repr(C)]
struct SeccompNotif {
    id: u64,
    pid: u32,
    flags: u32,
    data: SeccompData,
}

#[repr(C)]
struct SeccompResp {
    id: u64,
    val: i64,
    error: i32,
    flags: u32,
}

unsafe extern "C" {
    fn syscall(num: i64, ...) -> i64;
    fn ioctl(fd: i32, req: u64, ...) -> i32;
    fn mkfifo(path: *const i8, mode: u32) -> i32;
}

impl std::fmt::Debug for ContentHit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}({})", self.kind, self.nr)
    }
}

fn pipe_cloexec() -> (OwnedFd, OwnedFd) {
    let mut fds = [0i32; 2];
    let rc = unsafe { syscall(293, fds.as_mut_ptr(), 0x8_0000i64) };
    assert_eq!(rc, 0, "pipe2 {}", std::io::Error::last_os_error());
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn exit_group(code: i32) -> ! {
    unsafe {
        syscall(231, i64::from(code));
    }
    loop {
        thread::yield_now();
    }
}

fn emit_line(text: &str) {
    let mut line = String::with_capacity(text.len() + 1);
    line.push_str(text);
    line.push('\n');
    let mut err = std::io::stderr();
    let _ = err.write_all(line.as_bytes());
    let _ = err.flush();
}

fn mark_start(mode: &str, fixture: &Path) {
    let line = format!(
        "OBSERVER_CHILD_START pid={} mode={mode} fixture={}",
        std::process::id(),
        fixture.display()
    );
    emit_line(&line);
    if let Some(path) = std::env::var_os("OMNI_OBSERVER_MARKER") {
        let mut file = File::create(path).unwrap();
        writeln!(file, "{line}").unwrap();
        file.sync_all().unwrap();
    }
}

fn note_settled() {
    emit_line("OBSERVER_SETTLED");
    if let Some(path) = std::env::var_os("OMNI_OBSERVER_MARKER")
        && let Ok(mut file) = OpenOptions::new().append(true).open(path)
    {
        let _ = writeln!(file, "OBSERVER_SETTLED");
        let _ = file.sync_all();
    }
}

fn observer_child_entered(mode: &str) -> bool {
    match std::env::var("OMNI_OBSERVER_CHILD") {
        Ok(actual) => {
            assert_eq!(actual, mode);
            run_observer_child(&actual);
            true
        }
        Err(_) => false,
    }
}

fn run_observer_child(mode: &str) {
    let fixture = Fixture::new();
    mark_start(mode, fixture.path());
    fs::write(fixture.path().join("ready"), b"ready").unwrap();
    match mode {
        "normal" => run_normal_child(&fixture),
        "content" => run_content_child(&fixture),
        "panic" => run_panic_child(&fixture),
        "timeout" => run_timeout_child(&fixture),
        other => panic!("unknown observer child {other}"),
    }
}

fn run_normal_child(fixture: &Fixture) {
    let path = fixture.path().join("note.txt");
    fs::write(&path, b"quiet-stat").unwrap();
    warm(&path);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let report = observe(&[&path], move || mount.stat(&rel("note.txt"), room()));
    assert!(report.admitted, "{}", report.admit_reason());
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert!(!report.unresolved, "{}", report.admit_reason());
    let obs = ok_product(report.value);
    assert_eq!(obs.availability, Availability::Available);
}

fn run_content_child(fixture: &Fixture) {
    let body = fixture.path().join("body");
    fs::write(&body, b"visible-read").unwrap();
    let alias = fixture.path().join("alias");
    fs::hard_link(&body, &alias).unwrap();
    warm(&body);
    let report = observe(&[&body], move || {
        fs::read(&alias).expect("deliberate read");
    });
    assert!(report.admitted, "{}", report.admit_reason());
    assert!(!report.unresolved, "{}", report.admit_reason());
    assert!(
        !report.content.is_empty(),
        "hard-link read was invisible: {}",
        report.admit_reason()
    );
}

fn run_panic_child(fixture: &Fixture) {
    let path = fixture.path().join("note.txt");
    fs::write(&path, b"panic-control").unwrap();
    warm(&path);
    let late = fixture.path().join("late");
    let _report = observe(&[&path], move || -> () {
        panic!("deliberate observer action panic");
    });
    fs::write(&late, b"late").unwrap();
}

fn run_timeout_child(fixture: &Fixture) {
    let path = fixture.path().join("note.txt");
    fs::write(&path, b"timeout-control").unwrap();
    warm(&path);
    let (read_end, write_end) = pipe_cloexec();
    let late = fixture.path().join("late");
    let late_after = late.clone();
    let _report = observe(&[&path], move || {
        let mut buf = [0u8; 1];
        let _ = unsafe {
            syscall(
                0,
                i64::from(read_end.as_raw_fd()),
                buf.as_mut_ptr() as i64,
                1i64,
            )
        };
        let _ = write_end;
        fs::write(&late, b"late").unwrap();
    });
    fs::write(&late_after, b"returned").unwrap();
}

struct ChildOutcome {
    status: ExitStatus,
    marker: String,
}

fn spawn_observer_child(test: &str, mode: &str, bound_ms: Option<u64>) -> ChildOutcome {
    let marker_dir = Fixture::new();
    let marker_path = marker_dir.path().join("marker");
    let mut command = Command::new(std::env::current_exe().expect("test executable"));
    command
        .arg("--exact")
        .arg(test)
        .env("OMNI_OBSERVER_CHILD", mode)
        .env("OMNI_OBSERVER_MARKER", &marker_path)
        .env_remove("OMNI_FIFO_WORKER")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(ms) = bound_ms {
        command.env("OMNI_OBSERVER_BOUND_MS", ms.to_string());
    }
    let mut child = command.spawn().expect("observer child");
    let pid = child.id();
    emit_line(&format!(
        "OBSERVER_PARENT_START pid={pid} mode={mode} test={test}"
    ));
    let limit = if bound_ms.is_some() {
        Duration::from_secs(20)
    } else {
        Duration::from_secs(15)
    };
    let status = wait_child(&mut child, limit, pid, mode);
    let marker = fs::read_to_string(&marker_path).unwrap_or_default();
    assert!(
        marker.contains(&format!("pid={pid}")),
        "marker {marker:?} does not name pid {pid}"
    );
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "residual pid {pid}"
    );
    ChildOutcome { status, marker }
}

fn wait_child(
    child: &mut std::process::Child,
    limit: Duration,
    pid: u32,
    mode: &str,
) -> ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let status = child.wait().unwrap();
            panic!("observer child {pid} mode {mode} exceeded the owned-process bound: {status}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn assert_child_exit(outcome: &ChildOutcome, code: Option<i32>) {
    assert_eq!(outcome.status.code(), code, "{}", outcome.status);
}

fn assert_no_late_work(outcome: &ChildOutcome) {
    let fixture = child_fixture(outcome);
    if fixture.exists() {
        assert!(
            !fixture.join("late").exists(),
            "late fixture work at {}",
            fixture.display()
        );
    }
}

fn child_fixture(outcome: &ChildOutcome) -> PathBuf {
    let line = outcome
        .marker
        .lines()
        .find(|line| line.contains("OBSERVER_CHILD_START"))
        .unwrap_or(&outcome.marker);
    let fixture = line
        .split_once("fixture=")
        .map(|(_, path)| path.trim())
        .unwrap_or("");
    assert!(!fixture.is_empty(), "no fixture in {}", outcome.marker);
    PathBuf::from(fixture)
}
