//! Real store, inert history, and ordered held EPUB calls. No stand-in owner.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use knowledge_core::admission::{Account, Configuration, Decision, Holding, Origin, Scheduling};
use knowledge_core::backend::Budget;
use knowledge_core::data::RelativePath;
use knowledge_core::jobs::{Key, Outcome, Priority, Reservation, Submission};
use knowledge_history::{Change, Destination, History, Mode, OwnerContext, Request};
use knowledge_package::{
    Bounds, EpubOwnerError, GitTransactions, HeldMember, HistoryOwner, Quotas, SafeApply,
    StorageOwner, TextInstall, VerifiedSource, Work, export_ordered_epub, import_verified_source,
    install_verified_text, verify_inert_bundle,
};
use knowledge_processing::Admitted;
use knowledge_processing::admission::AdmitError;
use knowledge_processing::funding::Funding;
use knowledge_processing::held::HeldEpub;
use knowledge_storage::{Phase, Preflight, Scope, Store, WriteContext, WritePolicy};
use sha2::{Digest, Sha256};

const M: u64 = 1024 * 1024;
const TEXT_LEN: usize = 256 * 1024;
const PATH_LEN: usize = 64 * 1024;

struct ObservedAllocator;

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let previous = CURRENT.fetch_add(size, Ordering::SeqCst);
        raise_peak(previous.saturating_add(size));
        // SAFETY: `System` is the process allocator and `layout` is the caller's layout.
        let pointer = unsafe { System.alloc(layout) };
        if pointer.is_null() {
            CURRENT.fetch_sub(size, Ordering::SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::SeqCst);
        // SAFETY: `pointer` and `layout` came from this allocator's `alloc`.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static OBSERVED: ObservedAllocator = ObservedAllocator;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn raise_peak(now: usize) {
    let mut peak = PEAK.load(Ordering::SeqCst);
    while now > peak {
        match PEAK.compare_exchange_weak(peak, now, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(seen) => peak = seen,
        }
    }
}

fn allocation_delta<T>(body: impl FnOnce() -> T) -> (T, usize) {
    let start = CURRENT.load(Ordering::SeqCst);
    PEAK.store(start, Ordering::SeqCst);
    let value = body();
    let peak = PEAK.load(Ordering::SeqCst);
    (value, peak.saturating_sub(start))
}
static GATE: Mutex<()> = Mutex::new(());
static ACCOUNT: OnceLock<()> = OnceLock::new();
const MARKER: &[u8] = b"{\"version\":2,\"library_id\":\"lib\",\"name\":\"fixture\",\"text_root\":\"pages\",\"mounts\":[],\"convention_version\":\"0.2\",\"exposure\":{\"mode\":\"off\",\"sensitive\":false,\"excluded_paths\":[],\"transports\":[]}}";

#[derive(Clone)]
struct Policy;
impl WritePolicy for Policy {
    fn current(&self) -> Result<WriteContext, knowledge_storage::Error> {
        Ok(WriteContext {
            library_id: "lib".into(),
            revision: 1,
            text_root: "pages".into(),
            writable: true,
            raw_original_writes: false,
            outside_scope: vec![],
        })
    }
}

struct Closed;
impl WritePolicy for Closed {
    fn current(&self) -> Result<WriteContext, knowledge_storage::Error> {
        Err(knowledge_storage::Error::Context)
    }
}

struct Home {
    parent: PathBuf,
    version: knowledge_core::data::ContentDigest,
    recipe: knowledge_core::data::ContentDigest,
    claim: knowledge_core::jobs::Claim,
}

impl Home {
    fn new(name: &str) -> Self {
        ensure_account();
        let parent = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("pkg-native-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(parent.join("home")).unwrap();
        fs::create_dir_all(parent.join("private")).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(parent.join("home"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(parent.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(parent.join("home/omni-knowledge.json"), MARKER).unwrap();
        let version =
            knowledge_core::data::ContentDigest::new(format!("{:x}", Sha256::digest(MARKER)))
                .unwrap();
        let recipe = knowledge_core::data::ContentDigest::new(format!(
            "{:x}",
            Sha256::digest(name.as_bytes())
        ))
        .unwrap();
        let context = parent.to_str().unwrap();
        let account = Account::current().unwrap();
        account
            .submit(Submission {
                operation_id: context,
                job_id: context,
                key: Key {
                    context,
                    library: "lib",
                    mount: None,
                    source: "omni-knowledge.json",
                    version: &version,
                    recipe: &recipe,
                    processor: &recipe,
                },
                budget: Budget {
                    max_files: 64,
                    max_bytes: 8 * M,
                    max_millis: 120_000,
                    max_memory_bytes: 320 * M,
                },
                reservation: Reservation {
                    memory_bytes: 320 * M,
                    temporary_bytes: 16 * M,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        Self {
            parent,
            version,
            recipe,
            claim,
        }
    }

    fn key(&self) -> Key<'_> {
        Key {
            context: self.parent.to_str().unwrap(),
            library: "lib",
            mount: None,
            source: "omni-knowledge.json",
            version: &self.version,
            recipe: &self.recipe,
            processor: &self.recipe,
        }
    }

    fn scope(&self) -> Scope<'_> {
        Scope {
            claim: &self.claim,
            key: self.key(),
            scratch_bytes: 16 * M,
        }
    }

    fn home(&self) -> PathBuf {
        self.parent.join("home")
    }

    fn store(&self) -> Store<Policy> {
        Store::open(&self.home(), "lib", "writer-a", Policy).unwrap()
    }

    fn history(&self) -> History {
        History::init(
            &self.parent.join("repo"),
            &self.home(),
            "pages",
            owner(),
            &self.claim,
        )
        .unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let account = Account::current().unwrap();
        let _ = account
            .queue()
            .settle(&self.claim, self.key(), Outcome::Completed);
        let _ = account.queue().retire(self.claim.job(), self.key());
        let _ = fs::remove_dir_all(&self.parent);
    }
}

fn owner() -> OwnerContext {
    OwnerContext {
        name: "Fixture Owner".into(),
        email: "fixture@example.invalid".into(),
        agent: None,
        tool: None,
        session: None,
        mode: Mode::UnsignedExplicit,
    }
}

fn ensure_account() {
    ACCOUNT.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("pkg-m5-native-account-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let file = File::open(&dir).unwrap();
        Account::start(
            file,
            Configuration {
                memory_bytes: Some(512 * M),
                storage_bytes: Some(1024 * M),
            },
            Scheduling {
                max_pending: 32,
                max_records: 256,
                max_descriptor_bytes: M as usize,
                per_mount: 8,
            },
        )
        .unwrap();
    });
}

fn reserve(memory: u64) -> Holding {
    match Account::current()
        .unwrap()
        .reserve(
            Reservation {
                memory_bytes: memory,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(holding) => holding,
        Decision::Waiting(_) => panic!("account waiting before the call"),
    }
}

fn bounds() -> Bounds {
    Bounds {
        budget: Budget {
            max_files: 4096,
            max_bytes: 64 * M,
            max_millis: 120_000,
            max_memory_bytes: 64 * M,
        },
        quotas: Quotas {
            max_archive_bytes: 64 * M,
            max_entries: 256,
            max_entry_expanded: 16 * M,
            max_total_expanded: 64 * M,
            max_ratio: 64,
            max_path_bytes: 256,
            max_depth: 8,
        },
    }
}

fn with_work(body: impl FnMut(&mut Work<'_>)) {
    with_bounds(&bounds(), body);
}

fn with_bounds(limits: &Bounds, mut body: impl FnMut(&mut Work<'_>)) {
    let mut holding = reserve(4096);
    let cancel = AtomicBool::new(false);
    let mut work = Work::new(limits, &mut holding, &cancel).unwrap();
    body(&mut work);
    drop(work);
    holding.finish().unwrap();
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args([
            "--no-pager",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
            "-c",
            "credential.helper=",
        ])
        .arg("--git-dir")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "owned Git observation failed");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn locate_worker() {
    if std::env::var_os("KNOWLEDGE_PROCESSING_PARSER_WORKER").is_some() {
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let worker = exe
        .parent()
        .and_then(|dir| dir.parent())
        .unwrap()
        .join("parser-worker");
    assert!(worker.is_file(), "parser-worker is not built");
    // SAFETY: these tests are serialized and set the variable before any parser child starts.
    unsafe { std::env::set_var("KNOWLEDGE_PROCESSING_PARSER_WORKER", worker) };
}

/// Install exclusive text, keep a stale witness and unknown files, and refuse a second copy.
#[test]
fn native_install_conflict_stale_and_unknown_file() {
    let _gate = GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    let home = Home::new("install");
    let store = home.store();
    let history = home.history();
    let scope = home.scope();
    let source_path = home.parent.join("one.md");
    fs::write(&source_path, b"one\n").unwrap();
    with_work(|work| {
        let mut source =
            VerifiedSource::retain_held(File::open(&source_path).unwrap(), work).unwrap();
        let handoff = install_verified_text(
            &mut source,
            &store,
            &history,
            &TextInstall {
                operation: "install-one",
                path: "pages/a.md",
                expected: None,
                base: None,
                private: &home.parent.join("private"),
            },
            &scope,
            work,
        )
        .unwrap();
        let inspection = handoff.owner.unwrap();
        assert!(handoff.source_revalidation.is_ok());
        assert_eq!(inspection.phase, Phase::FilesInstalled);
        assert_eq!(fs::read(home.home().join("pages/a.md")).unwrap(), b"one\n");
        assert_eq!(history.head(&home.claim).unwrap(), None);
        let current = store.observe("pages/a.md", &scope).unwrap().unwrap();
        fs::write(home.home().join("pages/keep.md"), b"keep\n").unwrap();
        fs::write(home.home().join("stray.txt"), b"stray\n").unwrap();
        fs::write(home.home().join("pages/a.md"), b"changed\n").unwrap();
        let replacement = home.parent.join("other.md");
        fs::write(&replacement, b"other\n").unwrap();
        let mut replacement_source =
            VerifiedSource::retain_held(File::open(&replacement).unwrap(), work).unwrap();
        let stale = install_verified_text(
            &mut replacement_source,
            &store,
            &history,
            &TextInstall {
                operation: "install-stale",
                path: "pages/a.md",
                expected: Some(current),
                base: None,
                private: &home.parent.join("private"),
            },
            &scope,
            work,
        )
        .unwrap();
        assert!(matches!(
            stale.owner,
            Err(knowledge_storage::Error::Conflict)
        ));
        assert_eq!(
            fs::read(home.home().join("pages/a.md")).unwrap(),
            b"changed\n"
        );
        assert_eq!(
            fs::read(home.home().join("pages/keep.md")).unwrap(),
            b"keep\n"
        );
        assert_eq!(fs::read(home.home().join("stray.txt")).unwrap(), b"stray\n");
        assert_eq!(fs::read(&replacement).unwrap(), b"other\n");
        let mut preflight_holding = reserve(16 * M);
        let preflight = Preflight {
            holding: &preflight_holding,
            budget: home.claim.budget(),
        };
        let note = home.parent.join("note.bin");
        fs::write(&note, b"note\n").unwrap();
        let mut note_source =
            VerifiedSource::retain_held(File::open(&note).unwrap(), work).unwrap();
        let private = home.parent.join("private");
        let mut owner = StorageOwner::new(&store, &history, home.scope(), preflight, &private);
        let path = RelativePath::new("raw/note.bin").unwrap();
        assert_eq!(
            owner
                .install_new("note", &path, &mut note_source, work)
                .unwrap(),
            knowledge_package::ApplyOutcome::Saved
        );
        assert_eq!(
            fs::read(home.home().join("raw/note.bin")).unwrap(),
            b"note\n"
        );
        assert_eq!(
            owner
                .install_new("note-again", &path, &mut note_source, work)
                .unwrap(),
            knowledge_package::ApplyOutcome::Conflict
        );
        assert_eq!(
            fs::read(home.home().join("raw/note.bin")).unwrap(),
            b"note\n"
        );
        assert_eq!(fs::read(&note).unwrap(), b"note\n");
        assert!(owner.take_unsettled().is_none());
        drop(owner);
        preflight_holding.finish().unwrap();
    });
}

/// Work credit above the text length and below the admitted peak refuses before the path clone or that buffer.
#[test]
fn native_text_credit_refuses_before_buffer_allocation() {
    let _gate = GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    let home = Home::new("credit");
    let store = home.store();
    let history = home.history();
    let scope = home.scope();
    let source_path = home.parent.join("wide.md");
    {
        let bytes = vec![b'a'; TEXT_LEN];
        fs::write(&source_path, &bytes).unwrap();
    }
    let wide_path = format!("pages/{}.md", "a".repeat(PATH_LEN));
    let limits = Bounds {
        budget: Budget {
            max_files: 4096,
            max_bytes: 8 * M,
            max_millis: 120_000,
            max_memory_bytes: TEXT_LEN as u64 + 2048,
        },
        quotas: Quotas {
            max_archive_bytes: 8 * M,
            max_entries: 64,
            max_entry_expanded: 8 * M,
            max_total_expanded: 8 * M,
            max_ratio: 64,
            max_path_bytes: 256,
            max_depth: 8,
        },
    };
    with_bounds(&limits, |work| {
        let mut source =
            VerifiedSource::retain_held(File::open(&source_path).unwrap(), work).unwrap();
        let (result, delta) = allocation_delta(|| {
            install_verified_text(
                &mut source,
                &store,
                &history,
                &TextInstall {
                    operation: "credit-refuse",
                    path: &wide_path,
                    expected: None,
                    base: None,
                    private: &home.parent.join("private"),
                },
                &scope,
                work,
            )
        });
        assert!(
            delta < PATH_LEN,
            "path or text buffer was allocated before credit refusal"
        );
        assert!(matches!(result, Err(knowledge_package::Error::TooLarge)));
        assert!(!home.parent.join("private").join("credit-refuse").exists());
        assert_eq!(history.head(&home.claim).unwrap(), None);
        assert_eq!(source_path.metadata().unwrap().len(), TEXT_LEN as u64);
    });
}

/// A policy refusal returns the typed context error and creates no destination.
#[test]
fn native_context_refusal_keeps_source() {
    let _gate = GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    let home = Home::new("context");
    let store = Store::open(&home.home(), "lib", "writer-a", Closed).unwrap();
    let source_path = home.parent.join("bytes.bin");
    fs::write(&source_path, b"bytes\n").unwrap();
    let mut holding = reserve(16 * M);
    let preflight = Preflight {
        holding: &holding,
        budget: home.claim.budget(),
    };
    with_work(|work| {
        let mut source =
            VerifiedSource::retain_held(File::open(&source_path).unwrap(), work).unwrap();
        let handoff = import_verified_source(
            &mut source,
            &store,
            "import-closed",
            "raw/bytes.bin",
            &home.parent.join("private"),
            &preflight,
            work,
        )
        .unwrap();
        assert!(matches!(
            handoff.owner,
            Err(knowledge_storage::Error::Context)
        ));
        assert!(!home.home().join("raw/bytes.bin").exists());
        assert_eq!(fs::read(&source_path).unwrap(), b"bytes\n");
    });
    holding.finish().unwrap();
}

/// A stop request stays a typed cancellation. The adapter does not settle the claim.
#[test]
fn native_stop_retains_cancellation() {
    let _gate = GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    let home = Home::new("stop");
    let store = home.store();
    let history = home.history();
    let scope = home.scope();
    home.claim
        .owner()
        .cancel(home.claim.job(), home.key())
        .unwrap();
    let source_path = home.parent.join("stopped.md");
    fs::write(&source_path, b"stopped\n").unwrap();
    with_work(|work| {
        let mut source =
            VerifiedSource::retain_held(File::open(&source_path).unwrap(), work).unwrap();
        let handoff = install_verified_text(
            &mut source,
            &store,
            &history,
            &TextInstall {
                operation: "install-stop",
                path: "pages/stopped.md",
                expected: None,
                base: None,
                private: &home.parent.join("private"),
            },
            &scope,
            work,
        )
        .unwrap();
        assert!(matches!(
            handoff.owner,
            Err(knowledge_storage::Error::Stopped(
                knowledge_core::jobs::Cancellation::Requested
            ))
        ));
        assert!(!home.home().join("pages/stopped.md").exists());
    });
}

/// Verify then admit objects from a bundle this history framework created. Refs stay absent.
#[test]
fn native_bundle_admits_objects_without_refs() {
    let _gate = GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    let home = Home::new("bundle");
    let history = home.history();
    let request = Request {
        operation_id: "bundle-source".into(),
        base: None,
        destination: Destination::Accepted,
        changes: vec![Change {
            path: "pages/a.md".into(),
            text: Some("Actual bundled text\n".into()),
        }],
    };
    history.prepare(&request, &home.claim).unwrap();
    history.publish("bundle-source", &home.claim).unwrap();
    let bundle_path = home.parent.join("selected.bundle");
    git(
        &home.parent.join("repo"),
        &[
            "bundle",
            "create",
            bundle_path.to_str().unwrap(),
            "refs/heads/omni-accepted",
        ],
    );
    let held = File::open(&bundle_path).unwrap();
    fs::remove_file(&bundle_path).unwrap();
    fs::write(&bundle_path, b"not the held bundle").unwrap();
    fs::create_dir_all(home.parent.join("dest-home")).unwrap();
    let destination = History::init(
        &home.parent.join("dest-repo"),
        &home.parent.join("dest-home"),
        "pages",
        owner(),
        &home.claim,
    )
    .unwrap();
    with_work(|work| {
        let mut source = VerifiedSource::retain_held(held.try_clone().unwrap(), work).unwrap();
        let verified = verify_inert_bundle(
            &mut source,
            &destination,
            "bundle-verify",
            &home.parent.join("private"),
            &home.claim,
            work,
        )
        .unwrap();
        let report = verified.owner.unwrap();
        assert!(verified.source_revalidation.is_ok());
        assert!(!report.objects_admitted);
        assert_eq!(report.refs.len(), 1);
        let version = report.refs[0].version.clone();
        let private = home.parent.join("private");
        let mut owner = HistoryOwner::new(&destination, &home.claim, &private);
        assert_eq!(
            owner
                .admit_inert_bundle("bundle-admit", &mut source, work)
                .unwrap(),
            knowledge_package::GitAdmit::Inert
        );
        assert!(owner.take_unsettled().is_none());
        assert_eq!(destination.head(&home.claim).unwrap(), None);
        assert!(
            destination
                .proposals(None, 10, &home.claim)
                .unwrap()
                .entries
                .is_empty()
        );
        assert_eq!(
            git(
                &home.parent.join("dest-repo"),
                &["cat-file", "-t", version.as_str()]
            ),
            "commit"
        );
    });
}

/// Approved member order is the spine order. Sorted logical names do not reorder it.
#[test]
fn native_epub_keeps_selected_order() {
    let _gate = GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    locate_worker();
    let mut home = Home::new("epub");
    let zebra = b"# Zebra\n\nZEBRA_PAGE_BODY\n\nSee [alpha](a.md).\n";
    let alpha = b"# Alpha\n\nALPHA_PAGE_BODY\n\nSee [zebra](z.md).\n";
    let zebra_path = home.parent.join("zebra.md");
    let alpha_path = home.parent.join("alpha.md");
    fs::write(&zebra_path, zebra).unwrap();
    fs::write(&alpha_path, alpha).unwrap();
    let output = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(home.parent.join("book.epub"))
        .unwrap();
    let context = home.parent.to_str().unwrap().to_owned();
    let version = home.version.clone();
    let recipe = home.recipe.clone();
    let key = knowledge_core::jobs::Key {
        context: &context,
        library: "lib",
        mount: None,
        source: "omni-knowledge.json",
        version: &version,
        recipe: &recipe,
        processor: &recipe,
    };
    let mut funding = Funding::Claim {
        claim: &mut home.claim,
        expected: key,
    };
    let private = home.parent.join("private");
    with_work(|work| {
        let mut zebra_source =
            VerifiedSource::retain_held(File::open(&zebra_path).unwrap(), work).unwrap();
        let mut alpha_source =
            VerifiedSource::retain_held(File::open(&alpha_path).unwrap(), work).unwrap();
        let z_name = RelativePath::new("z.md").unwrap();
        let a_name = RelativePath::new("a.md").unwrap();
        let outer = zebra_source
            .with_private_file(
                &private,
                "epub-z",
                work,
                |zebra_file, zebra_digest, work| {
                    alpha_source.with_private_file(
                        &private,
                        "epub-a",
                        work,
                        |alpha_file, alpha_digest, _work| {
                            let members = [
                                HeldMember {
                                    file: zebra_file,
                                    version: zebra_digest,
                                    logical_name: &z_name,
                                },
                                HeldMember {
                                    file: alpha_file,
                                    version: alpha_digest,
                                    logical_name: &a_name,
                                },
                            ];
                            export_ordered_epub(
                                &members,
                                &output,
                                "Approved order",
                                false,
                                Some(128 * M),
                                2 * M,
                                &mut funding,
                            )
                        },
                    )
                },
            )
            .unwrap();
        assert!(outer.source_revalidation.is_ok());
        let inner = outer.owner.unwrap();
        assert!(inner.source_revalidation.is_ok());
        let admitted = match inner.owner {
            Ok(admitted) => admitted,
            Err(error) => panic!("ordered epub failed: {error:?}"),
        };
        assert_eq!(admitted.value.chapters, 2);
        assert_eq!(
            admitted.value.file.metadata().unwrap().ino(),
            output.metadata().unwrap().ino()
        );
        let bodies = spine_bodies(&admitted.value.file);
        assert!(bodies[0].contains("ZEBRA_PAGE_BODY"));
        assert!(!bodies[0].contains("ALPHA_PAGE_BODY"));
        assert!(bodies[1].contains("ALPHA_PAGE_BODY"));
        assert!(!bodies[1].contains("ZEBRA_PAGE_BODY"));
        retire(admitted);
    });
}

/// A storage promise larger than the claim's credit fails before a book is written.
#[test]
fn native_epub_prefund_keeps_empty_output() {
    let _gate = GATE.lock().unwrap_or_else(|poison| poison.into_inner());
    locate_worker();
    let mut home = Home::new("prefund");
    let page = home.parent.join("page.md");
    fs::write(&page, b"# Page\n\nBody\n").unwrap();
    let output = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(home.parent.join("empty.epub"))
        .unwrap();
    let context = home.parent.to_str().unwrap().to_owned();
    let version = home.version.clone();
    let recipe = home.recipe.clone();
    let key = knowledge_core::jobs::Key {
        context: &context,
        library: "lib",
        mount: None,
        source: "omni-knowledge.json",
        version: &version,
        recipe: &recipe,
        processor: &recipe,
    };
    let mut funding = Funding::Claim {
        claim: &mut home.claim,
        expected: key,
    };
    let private = home.parent.join("private");
    with_work(|work| {
        let mut source = VerifiedSource::retain_held(File::open(&page).unwrap(), work).unwrap();
        let name = RelativePath::new("page.md").unwrap();
        let handoff = source
            .with_private_file(&private, "epub-prefund", work, |file, digest, _work| {
                let members = [HeldMember {
                    file,
                    version: digest,
                    logical_name: &name,
                }];
                export_ordered_epub(
                    &members,
                    &output,
                    "Prefund",
                    false,
                    Some(128 * M),
                    64 * M,
                    &mut funding,
                )
            })
            .unwrap();
        let refused = match handoff.owner {
            Err(error) => error,
            Ok(_) => panic!("prefund wrote a book"),
        };
        assert!(matches!(
            refused,
            EpubOwnerError::Admission(AdmitError::InsufficientCredit)
        ));
        assert_eq!(output.metadata().unwrap().len(), 0);
    });
}

fn retire(admitted: Admitted<HeldEpub>) {
    drop(admitted.value);
    for mut holding in admitted.retained {
        holding.finish().unwrap();
    }
}

fn spine_bodies(file: &File) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(file.try_clone().unwrap()).unwrap();
    let mut opf = String::new();
    zip.by_name("OEBPS/content.opf")
        .unwrap()
        .read_to_string(&mut opf)
        .unwrap();
    let spine = opf
        .split("<spine>")
        .nth(1)
        .unwrap()
        .split("</spine>")
        .next()
        .unwrap();
    let mut bodies = Vec::new();
    let mut rest = spine;
    while let Some(start) = rest.find("idref=\"") {
        rest = &rest[start + 7..];
        let end = rest.find('"').unwrap();
        let id = &rest[..end];
        rest = &rest[end..];
        let marker = format!("id=\"{id}\"");
        let item = opf.find(&marker).unwrap();
        let after = &opf[item..];
        let href_at = after.find("href=\"").unwrap() + 6;
        let href = after[href_at..].split('"').next().unwrap();
        let mut body = String::new();
        zip.by_name(&format!("OEBPS/{href}"))
            .unwrap()
            .read_to_string(&mut body)
            .unwrap();
        bodies.push(body);
    }
    bodies
}
