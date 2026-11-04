//! Independent observations of the ordinary local mount adapter.
//!
//! Expectations come from the storage, product, and service owners, the
//! knowledge-local crate guide, and the public Rustdoc of this crate and
//! `knowledge_core::backend`. They do not come from adapter output.
//!
//! The caller supplies locality and exclusion. This suite does not claim NAS,
//! cloud, account, or service authorization, and it does not claim that the
//! adapter discovers child libraries or inner mounts.

#![cfg(target_os = "linux")]

use std::ffi::{CString, OsStr};
use std::fs::{self, File, Permissions};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
        "content observer was not admitted; seen syscalls {:?}",
        report.seen
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
    assert!(report.admitted, "observer missed the warm-cache control");
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

/// A symlink is not followed, and a non-UTF-8 name stops the listing.
///
/// Storage forbids an undeclared escaping symlink and requires an explicit
/// non-UTF-8 failure. The outside file is watched for content syscalls.
#[test]
fn symlink_escape_and_non_utf8_names_are_unsupported() {
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
    let watched = outside.clone();
    let report = observe(&[&watched], move || {
        mount.open_range(
            &version_for(&outside, "escape", Some(MOUNT)),
            ByteRange::new(0, 1).unwrap(),
            room(),
        )
    });
    assert!(report.admitted, "observer missed the outside-file control");
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

    let odd = fixture.path().join("odd");
    fs::create_dir(&odd).unwrap();
    let raw = odd.join(OsStr::from_bytes(b"bad\xffname"));
    fs::write(&raw, b"x").unwrap();
    let mount = bind(&odd, Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])));
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
    assert!(report.admitted, "observer missed the control read");
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Unknown))
    );

    let mount = bind(fixture.path(), Caller::new(ReadDecision::Unestablished));
    let report = observe(&[&path], move || mount.stat(&rel("body.txt"), room()));
    assert!(report.admitted, "observer missed the control read");
    assert!(report.content.is_empty(), "{:?}", report.content);
    assert_eq!(
        report.value.err(),
        Some(BackendError::Unavailable(Unavailable::Unknown))
    );

    let mount = bind(fixture.path(), Caller::new(ReadDecision::Deny));
    let report = observe(&[&path], move || mount.list(None, room()));
    assert!(report.admitted, "observer missed the control read");
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
/// A correct digest under `Deny`, or an `Admit` for another library or mount,
/// does not return bytes.
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
    assert!(report.admitted, "observer missed the control read");
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
        Some(&BackendError::Unavailable(Unavailable::Denied)),
        "other library {other_library:?}; other mount {other_mount:?}"
    );
    assert_eq!(
        other_mount.as_ref().err(),
        Some(&BackendError::Unavailable(Unavailable::Denied)),
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
    assert!(report.admitted, "observer missed the control read");
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
    assert!(report.admitted, "observer missed the control read");
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
        "observer missed the control read; seen {:?}",
        report.seen
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

/// An offset past EOF is `InvalidRequest` only while the full digest matches.
///
/// Drift wins over that classification. The past-end call uses the current
/// independent metadata and the digest from before the edit.
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
    let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
    let matching = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot,
        },
        ByteRange::new(facts.size, 1).unwrap(),
        room(),
    );

    let mut edited = facts.bytes.clone();
    edited[0] = b'Z';
    fs::write(&path, &edited).unwrap();
    let after = facts_of(&path);
    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "body.txt", &after, Some(MOUNT));
    let drifted = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot,
        },
        ByteRange::new(after.size, 1).unwrap(),
        room(),
    );
    assert_eq!(
        matching.as_ref().map(|_| ()).err(),
        Some(&BackendError::InvalidRequest),
        "matching past-end {matching:?}; drifted {drifted:?}"
    );
    assert_eq!(
        drifted.as_ref().map(|_| ()).err(),
        Some(&BackendError::Unavailable(Unavailable::Changed)),
        "drifted past-end {drifted:?}"
    );
}

/// Range budgets charge the whole source and one byte beyond the retained request.
///
/// A short memory or byte ceiling fails before a content read. The successful
/// ceiling is the request length plus one hash byte, with `max_bytes` covering
/// the whole file. These are caller ceilings, not RSS or a syscall deadline.
#[test]
fn range_budgets_charge_the_whole_source_and_one_hash_byte() {
    let fixture = Fixture::new();
    let path = fixture.path().join("body.txt");
    let bytes: Vec<u8> = (0..64).collect();
    fs::write(&path, &bytes).unwrap();
    warm(&path);
    let facts = facts_of(&path);
    for budget in [
        Budget {
            max_bytes: 63,
            ..room()
        },
        Budget {
            max_memory_bytes: 16,
            ..room()
        },
        Budget {
            max_memory_bytes: 0,
            ..room()
        },
        Budget {
            max_files: 0,
            ..room()
        },
    ] {
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
            mount.open_range(&expected, ByteRange::new(0, 16).unwrap(), budget)
        });
        assert!(report.admitted, "observer missed the control read");
        assert!(
            report.content.is_empty(),
            "budget refusal still read the file under {budget:?}: {:?}",
            report.content
        );
        assert_eq!(report.value.err(), Some(BackendError::BudgetExhausted));
    }

    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
    let oversize = mount.open_range(
        &ExpectedVersion {
            digest: digest(&facts.digest),
            snapshot,
        },
        ByteRange::new(0, 80).unwrap(),
        Budget {
            max_bytes: 64,
            max_memory_bytes: 81,
            ..room()
        },
    );
    assert_eq!(oversize.err(), Some(BackendError::BudgetExhausted));

    let mount = bind(
        fixture.path(),
        Caller::new(grant(LIBRARY, Some(MOUNT), 1, &[])),
    );
    let snapshot = current_snapshot(&mount, "body.txt", &facts, Some(MOUNT));
    let read = mount
        .open_range(
            &ExpectedVersion {
                digest: digest(&facts.digest),
                snapshot,
            },
            ByteRange::new(0, 16).unwrap(),
            Budget {
                max_bytes: 64,
                max_memory_bytes: 17,
                ..room()
            },
        )
        .expect("request plus one hash byte");
    assert_eq!(read.bytes, &bytes[..16]);
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

/// Run `action` on a thread whose content syscalls are observed.
///
/// Coverage is `read`, `pread64`, `readv`, `preadv`, `preadv2`, `mmap` of a
/// file descriptor, `sendfile`, `splice`, and `copy_file_range` on that thread.
/// `openat` is continued and is not itself a content hit. Stat metadata calls
/// are outside this coverage. The observer always continues the syscall.
/// A warm-cache `read` and `pread64` of the watched file must be visible before
/// a quiet result is evidence. This is not `/proc/self/io` physical-byte accounting.
fn observe<T: Send + 'static>(
    watched: &[&Path],
    action: impl FnOnce() -> T + Send + 'static,
) -> Report<T> {
    let watched: Vec<Vec<u8>> = watched
        .iter()
        .map(|path| path.as_os_str().as_bytes().to_vec())
        .collect();
    let primary = PathBuf::from(OsStr::from_bytes(&watched[0]));
    warm(&primary);
    let admission = run_filtered(&watched, move || admit_read(&primary));
    let product = run_filtered(&watched, action);
    let primary = PathBuf::from(OsStr::from_bytes(&watched[0]));
    let again = run_filtered(&watched, move || admit_read(&primary));
    let admitted =
        !admission.0.is_empty() && !again.0.is_empty() && !admission.2 && !again.2 && !product.2;
    Report {
        value: product.1,
        admitted,
        content: product.0,
        seen: product.3,
    }
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
    assert!(rc >= 0, "admission pread failed");
}

fn run_filtered<T: Send + 'static>(
    watched: &[Vec<u8>],
    action: impl FnOnce() -> T + Send + 'static,
) -> (Vec<ContentHit>, T, bool, Vec<i32>) {
    let log = Arc::new(HitLog::new());
    let fd_slot = Arc::new(AtomicI32::new(-1));
    let ready = Arc::new(AtomicBool::new(false));
    let go = Arc::new(AtomicBool::new(false));
    let (value_tx, value_rx) = std::sync::mpsc::sync_channel(1);
    let watched = watched.to_vec();
    let observer_log = Arc::clone(&log);
    let observer_fd = Arc::clone(&fd_slot);
    let observer_ready = Arc::clone(&ready);
    let observer = thread::spawn(move || {
        let fd = wait_fd(&observer_fd);
        observer_ready.store(true, Ordering::SeqCst);
        observe_loop(fd, &watched, &observer_log);
    });
    let worker_fd = Arc::clone(&fd_slot);
    let worker_go = Arc::clone(&go);
    let worker = thread::spawn(move || {
        let fd = match install_listener() {
            Ok(fd) => fd,
            Err(_) => {
                worker_fd.store(-2, Ordering::SeqCst);
                return;
            }
        };
        worker_fd.store(fd, Ordering::SeqCst);
        while !worker_go.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        let outcome = panic::catch_unwind(AssertUnwindSafe(action));
        let _ = value_tx.send(outcome);
        unsafe { close(fd) };
    });
    let start = Instant::now();
    loop {
        if fd_slot.load(Ordering::SeqCst) == -2 {
            panic!("seccomp listener was not installed");
        }
        if ready.load(Ordering::SeqCst) {
            break;
        }
        if start.elapsed() > HARNESS_BOUND {
            panic!("content observer did not become ready");
        }
        thread::yield_now();
    }
    go.store(true, Ordering::SeqCst);
    let outcome = value_rx.recv_timeout(HARNESS_BOUND).unwrap_or_else(|_| {
        let fd = fd_slot.load(Ordering::SeqCst);
        if fd >= 0 {
            unsafe { close(fd) };
        }
        panic!("observed call exceeded the harness bound");
    });
    let value = match outcome {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    };
    worker.join().expect("worker");
    observer.join().expect("observer");
    let hits = std::mem::take(&mut *log.hits.lock().expect("hits"));
    let seen = std::mem::take(&mut *log.seen.lock().expect("seen"));
    let overflow = log.overflow.load(Ordering::SeqCst);
    (hits, value, overflow, seen)
}

fn wait_fd(slot: &AtomicI32) -> i32 {
    let start = Instant::now();
    loop {
        match slot.load(Ordering::SeqCst) {
            -1 => {
                if start.elapsed() > HARNESS_BOUND {
                    panic!("filtered thread did not publish a listener");
                }
                thread::yield_now();
            }
            -2 => panic!("seccomp listener was not installed"),
            fd => return fd,
        }
    }
}

fn observe_loop(fd: i32, watched: &[Vec<u8>], log: &HitLog) {
    loop {
        let mut note = std::mem::MaybeUninit::<SeccompNotif>::zeroed();
        let rc = unsafe { ioctl(fd, 0xc050_2100, note.as_mut_ptr()) };
        if rc < 0 {
            break;
        }
        let note = unsafe { note.assume_init() };
        log.push_seen(note.data.nr);
        if let Some(kind) = content_kind(note.data.nr) {
            let file_fd = file_descriptor(note.data.nr, &note.data.args);
            if file_fd >= 0 && fd_matches(note.pid, file_fd, watched) {
                log.push_hit(ContentHit {
                    nr: note.data.nr,
                    kind,
                });
            }
        }
        let response = SeccompResp {
            id: note.id,
            val: 0,
            error: 0,
            flags: 1,
        };
        let sent = unsafe { ioctl(fd, 0xc018_2101, &response) };
        if sent < 0 {
            break;
        }
    }
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

fn fd_matches(pid: u32, fd: i32, watched: &[Vec<u8>]) -> bool {
    let mut path = [0u8; 64];
    let Some(path_len) = write_proc_fd(&mut path, pid, fd) else {
        return false;
    };
    let mut target = [0u8; 4096];
    let n = unsafe {
        readlink(
            path.as_ptr() as *const i8,
            target.as_mut_ptr() as *mut i8,
            target.len(),
        )
    };
    if n < 0 {
        return false;
    }
    let _ = path_len;
    let got = &target[..n as usize];
    watched.iter().any(|wanted| got == wanted.as_slice())
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
    Ok(fd as i32)
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
    fn close(fd: i32) -> i32;
    fn readlink(path: *const i8, buf: *mut i8, size: usize) -> isize;
    fn mkfifo(path: *const i8, mode: u32) -> i32;
}

impl std::fmt::Debug for ContentHit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}({})", self.kind, self.nr)
    }
}
