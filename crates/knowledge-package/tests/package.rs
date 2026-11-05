mod residual;

use crc32fast::Hasher;
use flate2::Compression;
use flate2::write::DeflateEncoder;
use knowledge_core::admission::{Account, Configuration, Decision, Holding, Origin, Scheduling};
use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::Reservation;
use knowledge_package::{
    ApplyOutcome, ApplyState, Availability, Bounds, Capture, DigestState, Error, ExternalRef,
    GitAdmit, GitTransactions, IndexCoverage, Input, Method, Preset, Quotas, Role, SafeApply,
    V1HostContext, V1State, VerifiedSource, Window, Work, apply, capture, inspect, stage,
    v1_import_plan,
};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard, Once};
use std::time::{SystemTime, UNIX_EPOCH};

struct Session {
    _guard: MutexGuard<'static, ()>,
    holding: Holding,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.holding.finish();
    }
}

fn open_account() -> Session {
    static START: Once = Once::new();
    static GATE: Mutex<()> = Mutex::new(());
    START.call_once(|| {
        let path = std::env::temp_dir().join("pkg-m5-account");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("account dir");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("mode");
        let directory = File::open(&path).expect("account fd");
        Account::start(
            directory,
            Configuration {
                memory_bytes: Some(16 * 1024 * 1024),
                storage_bytes: Some(256 * 1024 * 1024),
            },
            Scheduling {
                max_pending: 8,
                max_records: 256,
                max_descriptor_bytes: 1 << 20,
                per_mount: 4,
            },
        )
        .expect("start account");
    });
    let guard = GATE.lock().unwrap_or_else(|error| error.into_inner());
    let account = Account::current().expect("current account");
    let holding = match account
        .reserve(
            Reservation {
                memory_bytes: 1,
                temporary_bytes: 1,
            },
            Origin::BeforeVerification,
        )
        .expect("reserve")
    {
        Decision::Ready(holding) => holding,
        Decision::Waiting(_) => panic!("account waiting before the call"),
    };
    Session {
        _guard: guard,
        holding,
    }
}

struct Home {
    root: PathBuf,
    calls: Vec<String>,
}
impl SafeApply for Home {
    fn apply_text_change(
        &mut self,
        _: &str,
        path: &RelativePath,
        _: Option<&ContentDigest>,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error> {
        self.calls.push(path.as_str().to_owned());
        place(&self.root, path, candidate, work)
    }
    fn install_new(
        &mut self,
        _: &str,
        path: &RelativePath,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error> {
        self.calls.push(path.as_str().to_owned());
        place(&self.root, path, candidate, work)
    }
}
struct HistorySpy {
    calls: u32,
}
impl GitTransactions for HistorySpy {
    fn admit_inert_bundle(
        &mut self,
        _: &str,
        bundle: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<GitAdmit, Error> {
        self.calls += 1;
        let mut bytes = Vec::new();
        bundle.copy_to(&mut bytes, work)?;
        let mut header = [0u8; 4];
        header.copy_from_slice(bytes.get(..4).ok_or(Error::Io)?);
        assert_eq!(&header, b"PACK");
        Ok(GitAdmit::Inert)
    }
}

fn place(
    root: &Path,
    path: &RelativePath,
    candidate: &mut VerifiedSource,
    work: &mut Work<'_>,
) -> Result<ApplyOutcome, Error> {
    let dest = root.join(path.as_str());
    if dest.exists() {
        return Ok(ApplyOutcome::Conflict);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| Error::Io)?;
    }
    let pending = dest.with_extension("package-pending");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|_| Error::Io)?;
    candidate.copy_to(&mut output, work)?;
    output.sync_all().map_err(|_| Error::Io)?;
    fs::hard_link(&pending, dest).map_err(|_| Error::Io)?;
    drop(output);
    fs::remove_file(pending).map_err(|_| Error::Io)?;
    Ok(ApplyOutcome::Saved)
}

fn bounds(archive: u64) -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 1_000,
            max_bytes: 80_000_000,
            max_millis: 30_000,
            max_memory_bytes: 32_000_000,
        },
        quotas: Quotas {
            max_archive_bytes: archive,
            max_entries: 100,
            max_entry_expanded: 2_000_000,
            max_total_expanded: 8_000_000,
            max_ratio: 1_000,
            max_path_bytes: 256,
            max_depth: 8,
        },
    }
}

fn work<'a>(bounds: &'a Bounds, holding: &'a mut Holding, cancel: &'a AtomicBool) -> Work<'a> {
    Work::new(bounds, holding, cancel).expect("work")
}

struct Held<T> {
    value: Option<T>,
    session: Option<Session>,
}

impl<T> Drop for Held<T> {
    fn drop(&mut self) {
        self.value.take();
        self.session.take();
    }
}

impl<T> std::ops::Deref for Held<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value.as_ref().expect("held value")
    }
}

impl<T> Held<T> {
    /// Use the same retained holding for recovery and native owner copies without another account lock.
    fn with_work<R>(
        &mut self,
        limits: &Bounds,
        cancel: &AtomicBool,
        body: impl FnOnce(&T, &mut Work<'_>) -> R,
    ) -> R {
        let value = self.value.as_ref().expect("value");
        let session = self.session.as_mut().expect("session");
        let mut job = work(limits, &mut session.holding, cancel);
        body(value, &mut job)
    }

    fn account_open(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.holding.remaining().is_ok())
    }
}

fn run<T>(limits: &Bounds, cancel: &AtomicBool, body: impl FnOnce(&mut Work<'_>) -> T) -> Held<T> {
    let mut session = open_account();
    let value = {
        let mut job = work(limits, &mut session.holding, cancel);
        body(&mut job)
    };
    Held {
        value: Some(value),
        session: Some(session),
    }
}

fn rel(value: &str) -> RelativePath {
    RelativePath::new(value).expect("path")
}

fn dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("pkg-{name}-{nanos}"));
    fs::create_dir(&path).expect("dir");
    path
}

fn window() -> Window {
    Window {
        start: "2026-11-05T00:00:00Z".into(),
        end: "2026-11-05T00:00:01Z".into(),
    }
}

#[test]
fn roundtrip_preserves_bytes_and_skips_history_index_and_conflicts() {
    let root = dir("round");
    let private = root.join("private");
    let home = root.join("home");
    fs::create_dir(&private).unwrap();
    fs::create_dir(&home).unwrap();
    fs::write(home.join("pages").join("keep.md"), b"keep").ok();
    fs::create_dir(home.join("pages")).unwrap();
    fs::write(home.join("pages").join("keep.md"), b"keep").unwrap();
    let dest = root.join("out.zip");
    let text = b"a\r\nb\n";
    let original = [0u8, 255, 10, 13];
    let mut text_reader = &text[..];
    let mut original_reader = &original[..];
    let history = b"PACKh";
    let mut history_reader = &history[..];
    let index = b"INDEX";
    let mut index_reader = &index[..];
    let mut inputs = [
        Input {
            path: rel("pages/note.md"),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut text_reader,
        },
        Input {
            path: rel("pages/keep.md"),
            role: Role::Core,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut &b"new"[..],
        },
        Input {
            path: rel("media/blob.bin"),
            role: Role::Original,
            mount_id: Some("mount".into()),
            source_id: Some("source".into()),
            expected: None,
            reader: &mut original_reader,
        },
        Input {
            path: rel("history/bundle.pack"),
            role: Role::History,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut history_reader,
        },
        Input {
            path: rel("indexes/lex.bin"),
            role: Role::Index,
            mount_id: None,
            source_id: None,
            expected: None,
            reader: &mut index_reader,
        },
    ];
    let mut request = Capture {
        operation_id: "cap1",
        preset: Preset::Backup,
        library_id: "lib",
        name: "Demo",
        text_root: "pages",
        window: window(),
        mounts: vec![],
        external: vec![],
        omitted: 2,
        require_complete: false,
        required_features: vec![],
        private_dir: &private,
        destination: &dest,
    };
    let limits = bounds(8_000_000);
    let cancel = AtomicBool::new(false);
    {
        let captured = run(&limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job).expect("capture")
        });
        assert!(captured.linked);
        assert!(captured.account_open());
    }
    let mut archive = File::open(&dest).unwrap();
    {
        let preview = run(&limits, &cancel, |job| {
            inspect(&mut archive, job, &[rel("pages/keep.md")]).expect("inspect")
        });
        assert!(preview.account_open());
        assert_eq!(preview.conflicts, vec![rel("pages/keep.md")]);
        assert!(preview.installable);
        assert_eq!(preview.manifest.coverage.omitted, 2);
        assert!(preview.manifest.indexes.is_some());
        assert!(preview.manifest.indexes.as_ref().is_some_and(|set| {
            !set.compatible && set.fallback.as_deref() == Some("kernel_not_wired")
        }));
    }
    let mut archive = File::open(&dest).unwrap();
    let mut staged = run(&limits, &cancel, |job| {
        stage(
            &mut archive,
            job,
            &private,
            "restore1",
            &[rel("pages/keep.md")],
        )
        .expect("stage")
    });
    let mut home_apply = Home {
        root: home.clone(),
        calls: vec![],
    };
    let mut git = HistorySpy { calls: 0 };
    let applied = staged
        .with_work(&limits, &cancel, |s, job| {
            apply(s, &mut home_apply, &mut git, job)
        })
        .expect("apply");
    assert_eq!(applied.state, ApplyState::Conflict);
    assert_eq!(fs::read(home.join("pages/note.md")).unwrap(), text);
    assert_eq!(fs::read(home.join("pages/keep.md")).unwrap(), b"keep");
    assert_eq!(fs::read(home.join("media/blob.bin")).unwrap(), original);
    assert!(!home.join(".git").exists());
    assert!(!home.join("indexes").exists());
    assert_eq!(git.calls, 1);
    assert!(!home_apply.calls.iter().any(|call| call == "pages/keep.md"));
    let _ = Method::Stored;
}

#[test]
fn destination_and_require_complete_do_not_clobber() {
    let root = dir("clobber");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let dest = root.join("out.zip");
    fs::write(&dest, b"keep").unwrap();
    let text = b"hello";
    let mut reader = &text[..];
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut reader,
    }];
    let mut request = Capture {
        operation_id: "cap2",
        preset: Preset::Frozen,
        library_id: "lib",
        name: "Demo",
        text_root: "pages",
        window: window(),
        mounts: vec![],
        external: vec![ExternalRef {
            source_id: "s".into(),
            mount_id: "m".into(),
            path: rel("nas/a.bin"),
            digest_state: DigestState::Unverified,
            digest: None,
            availability: Availability::Unavailable,
            size_bytes: None,
        }],
        omitted: 0,
        require_complete: true,
        required_features: vec![],
        private_dir: &private,
        destination: &dest,
    };
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    let error = run(&limits, &cancel, |job| {
        capture(&mut request, &mut inputs, job).expect_err("incomplete")
    });
    assert_eq!(&*error, &Error::RequireComplete);
    assert_eq!(fs::read(&dest).unwrap(), b"keep");
}

#[test]
fn drift_and_forbidden_source_refuse() {
    let root = dir("drift");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let text = b"hello";
    let mut reader = &text[..];
    let wrong =
        ContentDigest::new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: Some(wrong),
        reader: &mut reader,
    }];
    let dest = root.join("out.zip");
    let mut request = base_capture(&private, &dest);
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    let error = run(&limits, &cancel, |job| {
        capture(&mut request, &mut inputs, job).expect_err("drift")
    });
    assert_eq!(&*error, &Error::Drift);
    assert!(!dest.exists());
}

#[test]
fn hostile_names_flags_overlap_and_checksums() {
    let limits = bounds(1_000_000);
    assert_eq!(
        inspect_raw(&classic(&[item(b"../x", b"a")]), &limits),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(&classic(&[item(b"/tmp/x", b"a")]), &limits),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(
            &classic(&[item(b"A.txt", b"a"), item(b"a.txt", b"b")]),
            &limits
        ),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(
            &classic(&[item(b"a.txt", b"a"), item(b"a.txt", b"b")]),
            &limits
        ),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(&classic(&[item(b".git/config", b"a")]), &limits),
        Err(Error::Forbidden)
    );
    let mut encrypted = item(b"a.txt", b"a");
    encrypted.flags |= 1;
    assert_eq!(
        inspect_raw(&classic(&[encrypted]), &limits),
        Err(Error::Hostile)
    );
    let mut link = item(b"a.txt", b"a");
    link.made_by = 3 << 8;
    link.external = 0o120777 << 16;
    assert_eq!(inspect_raw(&classic(&[link]), &limits), Err(Error::Hostile));
    let mut descriptor = item(b"a.txt", b"a");
    descriptor.flags |= 0x8;
    assert_eq!(
        inspect_raw(&classic(&[descriptor]), &limits),
        Err(Error::Unsupported)
    );
    let mut bad_crc = item(b"a.txt", b"hello");
    bad_crc.crc ^= 0xff;
    assert_eq!(
        inspect_raw(&classic(&[bad_crc]), &limits),
        Err(Error::Checksum)
    );
    assert_eq!(
        inspect_raw(&[0, 1, 2, 3, 4], &limits),
        Err(Error::Truncated)
    );
    assert_eq!(
        inspect_raw(
            &classic(&[item(b"docs", b"a"), item(b"docs/a", b"b")]),
            &limits
        ),
        Err(Error::Hostile)
    );
    let overlapped = overlap_zip();
    assert_eq!(inspect_raw(&overlapped, &limits), Err(Error::Hostile));
}

#[test]
fn declared_size_ratio_and_admission_do_not_allocate_the_lie() {
    let mut limits = bounds(1_000_000);
    limits.quotas.max_entry_expanded = 64;
    let huge = declared_zip(u32::MAX, b"hi");
    let (error, charged) = inspect_charge(&huge, &limits, false);
    assert_eq!(error, Err(Error::Quota));
    assert!(charged < 1_000_000);
    let bomb = deflate_bomb(40_000);
    let mut limits = bounds(1_000_000);
    limits.quotas.max_ratio = 4;
    limits.quotas.max_entry_expanded = 80_000;
    assert_eq!(inspect_raw(&bomb, &limits), Err(Error::Ratio));
    let mut limits = bounds(1_000_000);
    limits.budget.max_memory_bytes = 8;
    assert_eq!(
        inspect_raw(&classic(&[item(b"a.txt", b"hi")]), &limits),
        Err(Error::TooLarge)
    );
    {
        let mut session = open_account();
        let policy = Account::current().expect("account").policy().memory_bytes;
        let mut wide = bounds(1_000_000);
        wide.budget.max_memory_bytes = policy.saturating_add(8);
        let cancel = AtomicBool::new(false);
        {
            let mut job = work(&wide, &mut session.holding, &cancel);
            assert_eq!(
                job.charge(policy.saturating_add(1), 0),
                Err(Error::TooLarge)
            );
        }
        assert_eq!(
            session.holding.remaining().expect("remaining").memory_bytes,
            1
        );
    }
    assert_eq!(
        inspect_while_memory_is_short(&classic(&[item(b"a.txt", b"hi")]), &bounds(1_000_000)),
        Err(Error::Waiting)
    );
    assert_eq!(
        inspect_charge(&classic(&[item(b"a.txt", b"hi")]), &bounds(1_000_000), true).0,
        Err(Error::Cancelled)
    );
}

#[test]
fn zip64_sparse_offset_reads_the_payload() {
    let root = dir("zip64");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let plain = root.join("plain.zip");
    let payload = b"hi";
    let mut reader = &payload[..];
    let mut inputs = [Input {
        path: rel("media/a.bin"),
        role: Role::Original,
        mount_id: Some("mount".into()),
        source_id: Some("source".into()),
        expected: None,
        reader: &mut reader,
    }];
    let mut request = base_capture(&private, &plain);
    request.operation_id = "zip64src";
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    run(&limits, &cancel, |job| {
        capture(&mut request, &mut inputs, job).unwrap()
    });
    let mut plain_zip = zip::ZipArchive::new(File::open(&plain).unwrap()).unwrap();
    let mut manifest = String::new();
    plain_zip
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    let path = root.join("sparse.zip");
    write_sparse_zip64(&path, manifest.as_bytes(), b"media/a.bin", payload);
    let len = fs::metadata(&path).unwrap().len();
    assert!(len > (1 << 32));
    let mut limits = bounds(len);
    limits.budget.max_bytes = len;
    let mut archive = File::open(&path).unwrap();
    let cancel = AtomicBool::new(false);
    let mut session = open_account();
    let preview = inspect(
        &mut archive,
        &mut work(&limits, &mut session.holding, &cancel),
        &[],
    )
    .expect("zip64");
    assert_eq!(preview.manifest.entries.len(), 1);
    assert_eq!(preview.manifest.entries[0].size_bytes, 2);
    assert!(session.holding.remaining().expect("remaining").memory_bytes < 2_000_000);
}

#[test]
fn interrupted_generation_is_not_replayed() {
    let root = dir("stop");
    let private = root.join("private");
    fs::create_dir_all(private.join("op")).unwrap();
    File::create(private.join("op").join("INCOMPLETE")).unwrap();
    fs::write(private.join("op").join("partial.bin"), b"half").unwrap();
    let bytes = classic(&[item(b"a.txt", b"hi")]);
    let mut file = File::open(write_bytes(&root.join("a.zip"), &bytes)).unwrap();
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    let error = run(&limits, &cancel, |job| {
        stage(&mut file, job, &private, "op", &[]).expect_err("interrupted")
    });
    assert_eq!(&*error, &Error::Interrupted);
    assert_eq!(
        fs::read(private.join("op").join("partial.bin")).unwrap(),
        b"half"
    );
}

#[test]
fn foreign_v1_is_hashed_and_refused() {
    let root = dir("v1");
    fs::write(
        root.join("manifest.json"),
        b"{\"format\":\"openkit.knowledge-package\"}",
    )
    .unwrap();
    fs::write(root.join("page.md"), b"body\r\n").unwrap();
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    {
        let plan = run(&limits, &cancel, |job| {
            v1_import_plan(&root, None, job).expect("v1")
        });
        assert!(plan.account_open());
        assert_eq!(plan.state, V1State::CompatibilityRefused);
        assert!(!plan.host_context);
        assert_eq!(plan.files.len(), 1);
    }
    assert_eq!(fs::read(root.join("page.md")).unwrap(), b"body\r\n");
    let host = V1HostContext {
        owner: "o".into(),
        revision: "r".into(),
        profile: "p".into(),
    };
    let again = run(&limits, &cancel, |job| {
        v1_import_plan(&root, Some(&host), job).unwrap()
    });
    assert!(again.account_open());
    assert_eq!(again.state, V1State::CompatibilityRefused);
    assert!(again.host_context);
}

#[test]
fn framing_flags_disk_and_eocd_offset_refuse() {
    let limits = bounds(1_000_000);
    let mut flags = classic(&[item(b"a.txt", b"hi")]);
    let cd = 30 + b"a.txt".len() + 2;
    flags[cd + 8] = 0;
    flags[cd + 9] = 0;
    assert_eq!(inspect_raw(&flags, &limits), Err(Error::Disagreement));
    assert_eq!(
        inspect_raw(&zip64_disk_seven(), &limits),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(&eocd_tail_outside_window(), &limits),
        Err(Error::Hostile)
    );
    assert_eq!(
        inspect_raw(&classic(&[item(b"a.txt", b"hi")]), &limits),
        Err(Error::Invalid)
    );
}

#[test]
fn aggregate_call_memory_and_archive_bytes_are_bounded() {
    let huge = classic(&[item(b"manifest.json", &vec![b'a'; 200_000])]);
    let mut tight = bounds(1_000_000);
    tight.budget.max_memory_bytes = 90_000;
    assert_eq!(inspect_raw(&huge, &tight), Err(Error::TooLarge));
    let mut small = bounds(1_000_000);
    small.budget.max_memory_bytes = 90_000;
    assert_eq!(
        inspect_raw(&classic(&[item(b"manifest.json", b"{}")]), &small),
        Err(Error::Invalid)
    );
    let root = dir("zipbytes");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let dest = root.join("out.zip");
    let text = b"hi";
    let mut reader = &text[..];
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut reader,
    }];
    let mut request = base_capture(&private, &dest);
    let mut narrow = bounds(1_000_000);
    narrow.budget.max_bytes = 40;
    let cancel = AtomicBool::new(false);
    {
        let error = run(&narrow, &cancel, |job| {
            capture(&mut request, &mut inputs, job).expect_err("zip")
        });
        assert_eq!(&*error, &Error::TooLarge);
        assert!(!dest.exists());
    }
    let mut reader = &text[..];
    inputs[0].reader = &mut reader;
    let wide = bounds(1_000_000);
    let captured = run(&wide, &cancel, |job| {
        capture(&mut request, &mut inputs, job).expect("control")
    });
    assert!(captured.linked);
    assert!(captured.account_open());
}

#[test]
fn stale_complete_generation_is_not_reused() {
    let root = dir("stale");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let first = root.join("first.zip");
    let second = root.join("second.zip");
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    let text = b"alpha";
    let mut reader = &text[..];
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut reader,
    }];
    let mut request = base_capture(&private, &first);
    {
        let _captured = run(&limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job).expect("first")
        });
    }
    let mut archive = File::open(&first).unwrap();
    let snapshot = {
        let staged = run(&limits, &cancel, |job| {
            stage(&mut archive, job, &private, "op", &[rel("pages/a.md")]).expect("stage")
        });
        assert_eq!(staged.conflicts.len(), 1);
        assert!(staged.account_open());
        staged.snapshot_id.as_str().to_owned()
    };
    let mut broken = File::open(write_bytes(&root.join("broken.zip"), &[0, 1, 2, 3, 4])).unwrap();
    {
        let error = run(&limits, &cancel, |job| {
            stage(&mut broken, job, &private, "op", &[rel("pages/a.md")]).expect_err("truncated")
        });
        assert_eq!(&*error, &Error::Truncated);
    }
    let plan = fs::read_to_string(private.join("op").join("PLAN")).unwrap();
    assert!(plan.starts_with(&snapshot));
    let other = b"beta";
    let mut reader = &other[..];
    inputs[0].reader = &mut reader;
    request.destination = &second;
    request.operation_id = "other";
    {
        let _captured = run(&limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job).expect("second")
        });
    }
    let mut other_zip = File::open(&second).unwrap();
    {
        let error = run(&limits, &cancel, |job| {
            stage(&mut other_zip, job, &private, "op", &[]).expect_err("changed")
        });
        assert_eq!(&*error, &Error::Conflict);
    }
    assert!(
        fs::read_to_string(private.join("op").join("PLAN"))
            .unwrap()
            .starts_with(&snapshot)
    );
    let mut archive = File::open(&first).unwrap();
    let mut staged = run(&limits, &cancel, |job| {
        stage(&mut archive, job, &private, "op", &[rel("pages/a.md")]).expect("reuse")
    });
    assert_eq!(staged.conflicts.len(), 1);
    assert_eq!(staged.snapshot_id.as_str(), snapshot);
    let core = staged
        .files
        .iter()
        .find(|file| file.path.as_str() == "pages/a.md")
        .expect("core")
        .staged
        .clone();
    fs::write(&core, b"tampered").unwrap();
    let mut home = Home {
        root: root.join("home"),
        calls: Vec::new(),
    };
    fs::create_dir_all(&home.root).unwrap();
    let mut git = HistorySpy { calls: 0 };
    let applied = staged
        .with_work(&limits, &cancel, |s, job| {
            apply(s, &mut home, &mut git, job)
        })
        .expect("apply");
    assert_eq!(applied.state, ApplyState::Refused);
    assert!(home.calls.is_empty());
    assert_eq!(fs::read(&core).unwrap(), b"tampered");
}

#[test]
fn canonical_extension_and_index_fallback_keep_core() {
    let root = dir("canon");
    let private = root.join("private");
    fs::create_dir(&private).unwrap();
    let dest = root.join("out.zip");
    let text = b"hi";
    let mut reader = &text[..];
    let mut inputs = [Input {
        path: rel("pages/a.md"),
        role: Role::Core,
        mount_id: None,
        source_id: None,
        expected: None,
        reader: &mut reader,
    }];
    let mut request = base_capture(&private, &dest);
    let limits = bounds(1_000_000);
    let cancel = AtomicBool::new(false);
    {
        let captured = run(&limits, &cancel, |job| {
            capture(&mut request, &mut inputs, job).expect("capture")
        });
        assert!(captured.linked);
    }
    let mut archive = zip::ZipArchive::new(File::open(&dest).unwrap()).unwrap();
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    drop(archive);
    let mut value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    value["ratio_hint"] = serde_json::json!(0.5);
    value["coverage"]["indexes"] = serde_json::json!("complete");
    value["indexes"] = serde_json::json!({
        "compatible": true,
        "paths": ["indexes/gen"]
    });
    value["\u{10000}"] = serde_json::json!(1);
    value["\u{E000}"] = serde_json::json!(2);
    let canonical = serde_json_canonicalizer::to_vec(&value).unwrap();
    let bytes = zip_with_manifest(&canonical, b"hi");
    {
        let mut file = File::open(write_bytes(&root.join("canon.zip"), &bytes)).unwrap();
        let preview = run(&limits, &cancel, |job| {
            inspect(&mut file, job, &[]).expect("inspect")
        });
        assert!(preview.account_open());
        assert!(preview.installable);
        assert_eq!(preview.manifest.extra["ratio_hint"], serde_json::json!(0.5));
        let indexes = preview.manifest.indexes.as_ref().expect("indexes");
        assert!(indexes.compatible);
        assert_eq!(indexes.fallback.as_deref(), Some("kernel_not_wired"));
        assert_eq!(preview.manifest.coverage.indexes, IndexCoverage::Complete);
        assert_eq!(preview.manifest.required_features.len(), 0);
    }
    let canonical_text = std::str::from_utf8(&canonical).unwrap();
    let first = canonical_text.find("\"\u{10000}\"").expect("utf16 low key");
    let second = canonical_text.find("\"\u{E000}\"").expect("utf16 high key");
    assert!(first < second);
    let reversed = reverse_utf16_keys(&canonical);
    assert_eq!(
        inspect_raw(&zip_with_manifest(&reversed, b"hi"), &limits),
        Err(Error::Invalid)
    );
    let mut required = value;
    required["required_features"] = serde_json::json!(["search"]);
    let required_bytes = serde_json_canonicalizer::to_vec(&required).unwrap();
    let required_zip = zip_with_manifest(&required_bytes, b"hi");
    let need = write_bytes(&root.join("need.zip"), &required_zip);
    {
        let mut file = File::open(&need).unwrap();
        let preview = run(&limits, &cancel, |job| {
            inspect(&mut file, job, &[]).expect("required")
        });
        assert!(!preview.installable);
    }
    let mut file = File::open(&need).unwrap();
    let error = run(&limits, &cancel, |job| {
        stage(&mut file, job, &private, "need", &[]).expect_err("feature")
    });
    assert_eq!(&*error, &Error::Unsupported);
}

struct Item<'a> {
    name: &'a [u8],
    flags: u16,
    method: u16,
    data: &'a [u8],
    compressed: Vec<u8>,
    crc: u32,
    external: u32,
    made_by: u16,
}

fn item<'a>(name: &'a [u8], data: &'a [u8]) -> Item<'a> {
    Item {
        name,
        flags: 1 << 11,
        method: 0,
        data,
        compressed: data.to_vec(),
        crc: crc(data),
        external: 0,
        made_by: 0,
    }
}

fn crc(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

fn classic(items: &[Item<'_>]) -> Vec<u8> {
    let mut locals = Vec::new();
    let mut central = Vec::new();
    let mut offset = 0u32;
    for item in items {
        let local = local_header(item, 0);
        offset += local.len() as u32 + item.compressed.len() as u32;
        locals.extend(local);
        locals.extend(&item.compressed);
    }
    let cd_offset = locals.len() as u32;
    for (item, local_offset) in items.iter().zip(local_offsets(items)) {
        central.extend(central_header(item, local_offset, &[]));
        let _ = local_offset;
    }
    let mut out = locals;
    let cd_size = central.len() as u32;
    out.extend(central);
    out.extend(eocd(items.len() as u16, cd_size, cd_offset));
    let _ = offset;
    out
}

fn local_offsets(items: &[Item<'_>]) -> Vec<u32> {
    let mut at = 0u32;
    let mut offsets = Vec::new();
    for item in items {
        offsets.push(at);
        at += 30 + item.name.len() as u32 + item.compressed.len() as u32;
    }
    offsets
}

fn local_header(item: &Item<'_>, extra: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0x04034b50u32.to_le_bytes());
    bytes.extend(20u16.to_le_bytes());
    bytes.extend(item.flags.to_le_bytes());
    bytes.extend(item.method.to_le_bytes());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend(item.crc.to_le_bytes());
    bytes.extend((item.compressed.len() as u32).to_le_bytes());
    bytes.extend((item.data.len() as u32).to_le_bytes());
    bytes.extend((item.name.len() as u16).to_le_bytes());
    bytes.extend(extra.to_le_bytes());
    bytes.extend(item.name);
    bytes
}

fn central_header(item: &Item<'_>, offset: u32, extra: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0x02014b50u32.to_le_bytes());
    bytes.extend(item.made_by.to_le_bytes());
    bytes.extend(20u16.to_le_bytes());
    bytes.extend(item.flags.to_le_bytes());
    bytes.extend(item.method.to_le_bytes());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend(item.crc.to_le_bytes());
    bytes.extend((item.compressed.len() as u32).to_le_bytes());
    bytes.extend((item.data.len() as u32).to_le_bytes());
    bytes.extend((item.name.len() as u16).to_le_bytes());
    bytes.extend((extra.len() as u16).to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(item.external.to_le_bytes());
    bytes.extend(offset.to_le_bytes());
    bytes.extend(item.name);
    bytes.extend(extra);
    bytes
}

fn eocd(entries: u16, cd_size: u32, cd_offset: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0x06054b50u32.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(entries.to_le_bytes());
    bytes.extend(entries.to_le_bytes());
    bytes.extend(cd_size.to_le_bytes());
    bytes.extend(cd_offset.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes
}

fn overlap_zip() -> Vec<u8> {
    let first = item(b"a.txt", b"0123456789abcdef");
    let second = item(b"b.txt", b"zzzz");
    let mut bytes = local_header(&first, 0);
    bytes.extend(&first.compressed);
    let second_at = 40u32;
    let second_local = local_header(&second, 0);
    let second_end = second_at as usize + second_local.len() + second.compressed.len();
    if bytes.len() < second_end {
        bytes.resize(second_end, 0);
    }
    bytes[second_at as usize..second_at as usize + second_local.len()]
        .copy_from_slice(&second_local);
    let payload_at = second_at as usize + second_local.len();
    bytes[payload_at..payload_at + second.compressed.len()].copy_from_slice(&second.compressed);
    let cd_offset = bytes.len() as u32;
    let mut central = central_header(&first, 0, &[]);
    central.extend(central_header(&second, second_at, &[]));
    let cd_size = central.len() as u32;
    bytes.extend(central);
    bytes.extend(eocd(2, cd_size, cd_offset));
    bytes
}

fn declared_zip(declared: u32, payload: &[u8]) -> Vec<u8> {
    let mut item = item(b"a.txt", payload);
    item.data = payload;
    let mut bytes = classic(&[item]);
    // Both local and central uncompressed fields sit at fixed offsets for one stored entry.
    let local_uncomp = 22;
    bytes[local_uncomp..local_uncomp + 4].copy_from_slice(&declared.to_le_bytes());
    let cd = bytes.len() - 22 - 46 - b"a.txt".len();
    let central_uncomp = cd + 24;
    bytes[central_uncomp..central_uncomp + 4].copy_from_slice(&declared.to_le_bytes());
    bytes
}

fn deflate_bomb(count: usize) -> Vec<u8> {
    let data = vec![b'a'; count];
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&data).unwrap();
    let compressed = encoder.finish().unwrap();
    let mut item = item(b"a.txt", &data);
    item.method = 8;
    item.compressed = compressed;
    classic(std::slice::from_ref(&item))
}

fn write_sparse_zip64(path: &Path, manifest: &[u8], name: &[u8], payload: &[u8]) {
    let offset = 1u64 << 32;
    let payload_crc = crc(payload);
    let manifest_crc = crc(manifest);
    let mut manifest_local = local_header(
        &Item {
            name: b"manifest.json",
            flags: 1 << 11,
            method: 0,
            data: manifest,
            compressed: manifest.to_vec(),
            crc: manifest_crc,
            external: 0,
            made_by: 45,
        },
        0,
    );
    manifest_local.extend(manifest);
    let mut local = Vec::new();
    local.extend(0x04034b50u32.to_le_bytes());
    local.extend(45u16.to_le_bytes());
    local.extend((1u16 << 11).to_le_bytes());
    local.extend(0u16.to_le_bytes());
    local.extend([0, 0, 0, 0]);
    local.extend(payload_crc.to_le_bytes());
    local.extend(2u32.to_le_bytes());
    local.extend(2u32.to_le_bytes());
    local.extend((name.len() as u16).to_le_bytes());
    local.extend(0u16.to_le_bytes());
    local.extend(name);
    local.extend(payload);
    let cd_offset = offset + local.len() as u64;
    let extra = zip64_offset(offset);
    let manifest_central = central_header(
        &Item {
            name: b"manifest.json",
            flags: 1 << 11,
            method: 0,
            data: manifest,
            compressed: manifest.to_vec(),
            crc: manifest_crc,
            external: 0,
            made_by: 45,
        },
        0,
        &[],
    );
    let central = central_header(
        &Item {
            name,
            flags: 1 << 11,
            method: 0,
            data: payload,
            compressed: payload.to_vec(),
            crc: payload_crc,
            external: 0,
            made_by: 45,
        },
        u32::MAX,
        &extra,
    );
    // central_header wrote the real compressed length but offset MAX. Extra carries the real offset.
    let mut central_all = manifest_central;
    central_all.extend(central);
    let cd_size = central_all.len() as u64;
    let zip64_at = cd_offset + cd_size;
    let mut zip64 = Vec::new();
    zip64.extend(0x06064b50u32.to_le_bytes());
    zip64.extend(44u64.to_le_bytes());
    zip64.extend(45u16.to_le_bytes());
    zip64.extend(45u16.to_le_bytes());
    zip64.extend(0u32.to_le_bytes());
    zip64.extend(0u32.to_le_bytes());
    zip64.extend(2u64.to_le_bytes());
    zip64.extend(2u64.to_le_bytes());
    zip64.extend(cd_size.to_le_bytes());
    zip64.extend(cd_offset.to_le_bytes());
    let mut locator = Vec::new();
    locator.extend(0x07064b50u32.to_le_bytes());
    locator.extend(0u32.to_le_bytes());
    locator.extend(zip64_at.to_le_bytes());
    locator.extend(1u32.to_le_bytes());
    let mut eocd = Vec::new();
    eocd.extend(0x06054b50u32.to_le_bytes());
    eocd.extend(0u16.to_le_bytes());
    eocd.extend(0u16.to_le_bytes());
    eocd.extend(u16::MAX.to_le_bytes());
    eocd.extend(u16::MAX.to_le_bytes());
    eocd.extend(u32::MAX.to_le_bytes());
    eocd.extend(u32::MAX.to_le_bytes());
    eocd.extend(0u16.to_le_bytes());
    let len = zip64_at + zip64.len() as u64 + locator.len() as u64 + eocd.len() as u64;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.set_len(len).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&manifest_local).unwrap();
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(&local).unwrap();
    file.write_all(&central_all).unwrap();
    file.write_all(&zip64).unwrap();
    file.write_all(&locator).unwrap();
    file.write_all(&eocd).unwrap();
    file.sync_all().unwrap();
}

fn zip64_offset(offset: u64) -> Vec<u8> {
    let mut extra = Vec::new();
    extra.extend(1u16.to_le_bytes());
    extra.extend(8u16.to_le_bytes());
    extra.extend(offset.to_le_bytes());
    extra
}

fn inspect_raw(bytes: &[u8], limits: &Bounds) -> Result<(), Error> {
    inspect_charge(bytes, limits, false).0
}

fn inspect_charge(bytes: &[u8], limits: &Bounds, cancel: bool) -> (Result<(), Error>, u64) {
    let root = dir("raw");
    let path = write_bytes(&root.join("a.zip"), bytes);
    let mut file = File::open(path).unwrap();
    let flag = AtomicBool::new(cancel);
    let mut session = open_account();
    let result = inspect(
        &mut file,
        &mut work(limits, &mut session.holding, &flag),
        &[],
    )
    .map(|_| ());
    let charged = session.holding.remaining().expect("remaining").memory_bytes;
    (result, charged)
}

fn inspect_while_memory_is_short(bytes: &[u8], limits: &Bounds) -> Result<(), Error> {
    let root = dir("wait");
    let path = write_bytes(&root.join("a.zip"), bytes);
    let mut file = File::open(path).unwrap();
    let mut session = open_account();
    let account = Account::current().expect("account");
    let available = account.status().expect("status").memory_available;
    let mut blocker = match account
        .reserve(
            Reservation {
                memory_bytes: available.saturating_sub(4),
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .expect("blocker")
    {
        Decision::Ready(holding) => holding,
        Decision::Waiting(_) => panic!("blocker was not ready"),
    };
    let cancel = AtomicBool::new(false);
    let result = inspect(
        &mut file,
        &mut work(limits, &mut session.holding, &cancel),
        &[],
    )
    .map(|_| ());
    blocker.finish().expect("finish blocker");
    result
}

fn write_bytes(path: &Path, bytes: &[u8]) -> PathBuf {
    fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

fn zip64_disk_seven() -> Vec<u8> {
    let payload = b"hi";
    let name = b"a.txt";
    let mut extra = Vec::new();
    extra.extend(1u16.to_le_bytes());
    extra.extend(4u16.to_le_bytes());
    extra.extend(7u32.to_le_bytes());
    let mut local = local_header(&item(name, payload), 0);
    local.extend(payload);
    let mut central = central_header(&item(name, payload), 0, &extra);
    let disk_at = 34;
    central[disk_at..disk_at + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    let cd_offset = local.len() as u32;
    let cd_size = central.len() as u32;
    let mut out = local;
    out.extend(central);
    out.extend(eocd(1, cd_size, cd_offset));
    out
}

fn eocd_tail_outside_window() -> Vec<u8> {
    let len = 70_000;
    let eocd_at = 4_480;
    let mut bytes = vec![0u8; len];
    let locator = eocd_at - 20;
    bytes[locator..locator + 4].copy_from_slice(&0x07064b50u32.to_le_bytes());
    bytes[locator + 8..locator + 16].copy_from_slice(&4_442u64.to_le_bytes());
    bytes[locator + 16..locator + 20].copy_from_slice(&1u32.to_le_bytes());
    bytes[eocd_at..eocd_at + 4].copy_from_slice(&0x06054b50u32.to_le_bytes());
    bytes[eocd_at + 8..eocd_at + 10].copy_from_slice(&u16::MAX.to_le_bytes());
    bytes[eocd_at + 10..eocd_at + 12].copy_from_slice(&u16::MAX.to_le_bytes());
    bytes[eocd_at + 12..eocd_at + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    bytes[eocd_at + 16..eocd_at + 20].copy_from_slice(&u32::MAX.to_le_bytes());
    let comment = u16::try_from(len - (eocd_at + 22)).unwrap();
    bytes[eocd_at + 20..eocd_at + 22].copy_from_slice(&comment.to_le_bytes());
    bytes
}

fn zip_with_manifest(manifest: &[u8], page: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(page).unwrap();
    let compressed = encoder.finish().unwrap();
    let mut core = item(b"pages/a.md", page);
    core.method = 8;
    core.compressed = compressed;
    let manifest_item = item(b"manifest.json", manifest);
    classic(&[manifest_item, core])
}

fn reverse_utf16_keys(canonical: &[u8]) -> Vec<u8> {
    std::str::from_utf8(canonical)
        .unwrap()
        .replacen("\"\u{10000}\"", "\"@@SWAP@@\"", 1)
        .replacen("\"\u{E000}\"", "\"\u{10000}\"", 1)
        .replacen("\"@@SWAP@@\"", "\"\u{E000}\"", 1)
        .into_bytes()
}

fn base_capture<'a>(private_dir: &'a Path, destination: &'a Path) -> Capture<'a> {
    Capture {
        operation_id: "cap",
        preset: Preset::Frozen,
        library_id: "lib",
        name: "Demo",
        text_root: "pages",
        window: window(),
        mounts: vec![],
        external: vec![],
        omitted: 0,
        require_complete: false,
        required_features: vec![],
        private_dir,
        destination,
    }
}
