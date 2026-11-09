//! Builder regressions derived from history.md remote, quiet-batch and schedule clauses.
//! All effects use owned disposable local bare repositories, real local reads and synthetic identities.
use knowledge_core::admission::{Account, Configuration, Scheduling};
use knowledge_core::backend::{Budget, ExpectedVersion, MountBackend};
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::{Claim, Key, Limits, Outcome, Priority, Queue, Reservation, Submission};
use knowledge_history::{
    Change, Destination, Error, ExternalBatchState, ExternalText, History, HistoryWork, Mode,
    NativeHistoryOwner, Oid, OwnerContext, RemoteEffects, RemotePush, RemoteRelation, Request,
    State, external_batch_version,
};
use knowledge_local::{
    CurrentAdmission, LocalMount, LocalReadPolicy, LocalityPremise, ReadDecision,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Unchanged work ceilings; pack size, repeated reads and output must fit the original claim.
fn budget() -> Budget {
    Budget {
        max_files: 100,
        max_bytes: 524_288,
        max_millis: 60_000,
        max_memory_bytes: 64 << 20,
    }
}
/// Start one actual process-wide resource owner; all native work uses this same queue.
fn queue() -> Queue {
    let account = match Account::current() {
        Ok(account) => account,
        Err(knowledge_core::admission::Error::NotStarted) => {
            let cache = std::env::var_os("OMNI_HISTORY_ACCOUNT_CACHE")
                .expect("isolated actual account cache");
            Account::start(
                fs::File::open(cache).unwrap(),
                Configuration {
                    memory_bytes: None,
                    storage_bytes: None,
                },
                Scheduling {
                    max_pending: 8,
                    max_records: 16,
                    max_descriptor_bytes: 16384,
                    // Combined old fixtures retain bootstrap, observation and scheduled owners together.
                    per_mount: 4,
                },
            )
            .unwrap()
        }
        Err(error) => panic!("actual fixture account: {error}"),
    };
    account.queue().clone()
}

/// Keep each actual Account isolated; the restart control uses two sequential native processes.
fn isolated() -> bool {
    if std::env::var_os("OMNI_HISTORY_ACCOUNT_CHILD").is_some() {
        return false;
    }
    let test = std::thread::current().name().unwrap().to_owned();
    let cache = tempfile::tempdir().unwrap();
    fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let restart = tempfile::tempdir().unwrap();
    let phases = if test == "actual_remote_schedule_queue_restart_idempotency_and_next_due" {
        vec!["seed", "resume"]
    } else {
        vec!["normal"]
    };
    for phase in phases {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture"])
            .env("OMNI_HISTORY_ACCOUNT_CHILD", "1")
            .env("OMNI_HISTORY_ACCOUNT_CACHE", cache.path())
            .env("OMNI_HISTORY_RESTART_PHASE", phase)
            .env("OMNI_HISTORY_RESTART_ROOT", restart.path())
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
        assert!(
            output.status.success(),
            "actual History {test}/{phase}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    true
}
/// Synthetic identity is attribution only, distinct from live native selection.
fn owner(mode: Mode) -> OwnerContext {
    OwnerContext {
        name: "Remote Fixture".into(),
        email: "remote@example.invalid".into(),
        agent: None,
        tool: None,
        session: None,
        mode,
    }
}
/// A complete queue binding, kept live until actual native calls and returned values have been reconciled.
struct Work {
    /// Actual cooperating queue/account owner.
    queue: Queue,
    /// Original live reservation token.
    claim: Arc<Claim>,
    /// Fully qualified source-byte descriptor version.
    version: ContentDigest,
    /// Stable source identity, separate from its bytes.
    source: String,
}
impl Work {
    /// Create a real claimed job with the backend's complete required resource peak.
    fn new(version: ContentDigest, source: &str) -> Self {
        let queue = queue();
        let recipe = ContentDigest::new("b".repeat(64)).unwrap();
        let key = Key {
            context: "native-owner",
            library: "fixture",
            mount: None,
            source,
            version: &version,
            recipe: &recipe,
            processor: &recipe,
        };
        static NEXT_SETUP: AtomicU64 = AtomicU64::new(1);
        let id = format!("setup-{}", NEXT_SETUP.fetch_add(1, Ordering::SeqCst));
        Account::current()
            .unwrap()
            .submit(submission(&id, key))
            .unwrap();
        let claim = Arc::new(queue.claim(Instant::now()).unwrap().unwrap());
        Self {
            queue,
            claim,
            version,
            source: source.into(),
        }
    }
    /// Borrow every verified qualifier, not merely the input digest or operation ID.
    fn with<T>(&self, f: impl FnOnce(HistoryWork<'_>) -> T) -> T {
        let recipe = ContentDigest::new("b".repeat(64)).unwrap();
        f(HistoryWork {
            claim: &self.claim,
            key: Key {
                context: "native-owner",
                library: "fixture",
                mount: None,
                source: &self.source,
                version: &self.version,
                recipe: &recipe,
                processor: &recipe,
            },
        })
    }
}
impl Drop for Work {
    /// Calls in these tests must have collected real native owners before fixture reclamation.
    fn drop(&mut self) {
        // A failing assertion can carry an unknown native owner: do not manufacture completion during unwind.
        if std::thread::panicking() {
            return;
        }
        self.with(|w| {
            self.queue
                .settle(&self.claim, w.key, Outcome::Completed)
                .unwrap();
        });
    }
}
/// Explicit queue submission with no hidden budget escalation or extra executor.
fn submission<'a>(id: &'a str, key: Key<'a>) -> Submission<'a> {
    Submission {
        operation_id: id,
        job_id: id,
        key,
        budget: budget(),
        reservation: History::required_resources(&budget()).unwrap(),
        priority: Priority::Normal,
        max_attempts: 3,
    }
}
/// One owned filesystem tree, two actual bare histories and their actual native selection.
struct Fixture {
    /// Owned disposable state, retained if actual native collection fails.
    root: FixtureRoot,
    /// Actual external local-history owner.
    local: History,
    /// Actual disposable bare server owner.
    remote: History,
    /// Setup and observation reservation lifetime.
    work: Work,
    /// Live selected native capability, separate from the identity fixture.
    native: NativeHistoryOwner,
}
impl Fixture {
    /// Initialize through public owner operations; no private durable records are forged.
    fn new(mode: Mode) -> Self {
        let root = if std::env::var("OMNI_HISTORY_RESTART_PHASE").unwrap() == "seed" {
            FixtureRoot {
                owned: None,
                path: PathBuf::from(std::env::var_os("OMNI_HISTORY_RESTART_ROOT").unwrap()),
            }
        } else {
            let owned = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
            FixtureRoot {
                path: owned.path().to_owned(),
                owned: Some(owned),
            }
        };
        fs::create_dir(root.path().join("home")).unwrap();
        fs::create_dir(root.path().join("remote-home")).unwrap();
        let work = Work::new(
            ContentDigest::new("a".repeat(64)).unwrap(),
            "remote-selection",
        );
        let local = History::init(
            &root.path().join("local.git"),
            &root.path().join("home"),
            "pages",
            owner(mode.clone()),
            &work.claim,
        )
        .unwrap();
        let remote = History::init(
            &root.path().join("remote.git"),
            &root.path().join("remote-home"),
            "pages",
            owner(mode),
            &work.claim,
        )
        .unwrap();
        let native = NativeHistoryOwner::select(&local, "native-owner", "fixture").unwrap();
        Self {
            root,
            local,
            remote,
            work,
            native,
        }
    }
    /// Approve only the exact owned local endpoint and fixed actual branch.
    fn consent(&self, effects: RemoteEffects) -> knowledge_history::RemoteConsent {
        self.work
            .with(|w| {
                self.native.approve_local_remote(
                    &self.local,
                    &self.root.path().join("remote.git"),
                    "refs/heads/omni-accepted",
                    effects,
                    w,
                )
            })
            .unwrap()
    }
    /// Commit actual selected text through the public prepare/publish owner.
    fn commit(&self, history: &History, id: &str, text: &str) -> Oid {
        let prepared = history
            .prepare(
                &Request {
                    operation_id: id.into(),
                    base: history.head(&self.work.claim).unwrap(),
                    destination: Destination::Accepted,
                    changes: vec![Change {
                        path: "pages/a.md".into(),
                        text: Some(text.into()),
                    }],
                },
                &self.work.claim,
            )
            .unwrap();
        assert!(matches!(
            history.publish(id, &self.work.claim).unwrap(),
            State::Published(_)
        ));
        prepared.candidate.unwrap()
    }
    /// Observe actual refs/objects only in a fixture, with no user/system Git configuration.
    fn git(&self, repo: &str, args: &[&str]) -> Vec<u8> {
        git(&self.root.path().join(repo), args)
    }
}
/// Path wrapper that can preserve actual unknown-owner scratch before fixture fields drop.
struct FixtureRoot {
    /// Only ordinary fixture children own deletion; restart fixtures belong to their parent process.
    owned: Option<tempfile::TempDir>,
    /// Explicit owned synthetic root, never a service grant.
    path: PathBuf,
}
impl FixtureRoot {
    /// Borrow the live disposable path.
    fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for Fixture {
    /// Collect actual retained native owners before deleting their filesystem evidence; never signal unrelated processes.
    fn drop(&mut self) {
        let mut known = true;
        for history in [&self.local, &self.remote] {
            if history.collect_pending(&self.work.claim).is_err() {
                known = false;
            }
        }
        if !known {
            if let Some(owned) = self.root.owned.take() {
                let _ = owned.keep();
            }
            // Work::drop does not settle while unwinding this failure.
            assert!(
                std::thread::panicking(),
                "owned native collection unknown; fixture retained"
            );
        }
    }
}
/// Real bounded fixture observation, never a mocked successful Git process.
fn git(repo: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_DIR", repo)
        .env("GIT_AUTHOR_NAME", "Remote Fixture")
        .env("GIT_AUTHOR_EMAIL", "remote@example.invalid")
        .env("GIT_COMMITTER_NAME", "Remote Fixture")
        .env("GIT_COMMITTER_EMAIL", "remote@example.invalid")
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "fixture Git observation failed");
    out.stdout
}

/// history.md: real initial pull/continuation/push, candidate preservation and completed idempotency.
#[test]
fn actual_local_roundtrip_proposals_expected_lease_and_completed_retry() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    let first = f.commit(&f.remote, "remote-first", "remote one\n");
    fs::write(
        f.root.path().join("home/untouched"),
        "home stays authoritative",
    )
    .unwrap();
    let consent = f.consent(RemoteEffects::FetchAndPush);
    let fetched = f
        .work
        .with(|w| f.local.pull_remote("pull-one", &consent, w))
        .unwrap();
    assert_eq!(fetched.fetched, first);
    assert_eq!(fetched.relation, RemoteRelation::Initial);
    assert_eq!(f.local.head(&f.work.claim).unwrap(), None);
    assert_eq!(
        f.local
            .version_text(&first, "pages/a.md", &f.work.claim)
            .unwrap(),
        "remote one\n"
    );
    assert!(!f.root.path().join("home/pages/a.md").exists());
    let decision = f.native.confirm_remote(&consent, fetched.clone()).unwrap();
    let candidate = f
        .work
        .with(|w| {
            f.local
                .prepare_remote_confirmation("integrate-initial", &decision, &consent, w)
        })
        .unwrap();
    assert!(matches!(
        f.local.publish("integrate-initial", &f.work.claim).unwrap(),
        State::Published(_)
    ));
    assert_eq!(
        f.local
            .version(candidate.candidate.as_ref().unwrap(), &f.work.claim)
            .unwrap()
            .parents,
        vec![first.clone()]
    );
    let next = f.commit(&f.local, "local-two", "local two\n");
    let approval = f
        .native
        .approve_push(&consent, "push-one", &next, Some(&first))
        .unwrap();
    assert_eq!(
        f.work
            .with(|w| f.local.push_remote(&approval, &consent, w))
            .unwrap(),
        RemotePush::Published(next.clone())
    );
    let external = f.commit(&f.remote, "remote-external", "a later remote writer\n");
    assert_eq!(
        f.work
            .with(|w| f.local.push_remote(&approval, &consent, w))
            .unwrap(),
        RemotePush::Published(next)
    );
    assert_eq!(f.remote.head(&f.work.claim).unwrap(), Some(external));
    assert_eq!(
        f.work
            .with(|w| f.local.pull_remote("pull-one", &consent, w))
            .unwrap(),
        fetched
    );
    assert_eq!(
        fs::read_to_string(f.root.path().join("home/untouched")).unwrap(),
        "home stays authoritative"
    );
}

/// history.md: initial empty-tree remote adoption preserves the actual imported commit without inventing an empty operation commit.
#[test]
fn actual_empty_remote_adoption_reuses_its_original_commit() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    let tree = String::from_utf8(f.git("remote.git", &["mktree"])).unwrap();
    let commit = String::from_utf8(f.git(
        "remote.git",
        &[
            "commit-tree",
            tree.trim(),
            "-m",
            "original empty remote commit",
        ],
    ))
    .unwrap();
    let original = Oid::parse(commit.trim()).unwrap();
    f.git(
        "remote.git",
        &["update-ref", "refs/heads/omni-accepted", original.as_str()],
    );
    let consent = f.consent(RemoteEffects::FetchOnly);
    let fetched = f
        .work
        .with(|w| f.local.pull_remote("empty-pull", &consent, w))
        .unwrap();
    assert!(fetched.affected_paths.is_empty());
    let decision = f.native.confirm_remote(&consent, fetched).unwrap();
    let prepared = f
        .work
        .with(|w| {
            f.local
                .prepare_remote_confirmation("empty-adoption", &decision, &consent, w)
        })
        .unwrap();
    assert!(prepared.unchanged);
    assert_eq!(prepared.candidate, Some(original.clone()));
    assert!(matches!(
        f.local.publish("empty-adoption", &f.work.claim).unwrap(),
        State::Published(_)
    ));
    assert_eq!(f.local.head(&f.work.claim).unwrap(), Some(original));
    assert_eq!(
        f.git(
            "local.git",
            &["rev-list", "--count", "refs/heads/omni-accepted"]
        ),
        b"1\n"
    );
}

/// history.md: divergence never overwrites either writer, and stale inspected confirmations preserve their candidate.
#[test]
fn actual_divergence_stale_confirmation_and_push_refusal_preserve_both_sides() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    let base = f.commit(&f.remote, "remote-base", "base\n");
    let consent = f.consent(RemoteEffects::FetchAndPush);
    let pull = f
        .work
        .with(|w| f.local.pull_remote("seed", &consent, w))
        .unwrap();
    let approval = f.native.confirm_remote(&consent, pull).unwrap();
    f.work
        .with(|w| {
            f.local
                .prepare_remote_confirmation("seed-integration", &approval, &consent, w)
        })
        .unwrap();
    f.local.publish("seed-integration", &f.work.claim).unwrap();
    let local = f.commit(&f.local, "local-side", "local competitor\n");
    let remote = f.commit(&f.remote, "remote-side", "remote competitor\n");
    let pull = f
        .work
        .with(|w| f.local.pull_remote("diverged", &consent, w))
        .unwrap();
    assert_eq!(pull.relation, RemoteRelation::Diverged);
    assert_eq!(f.local.head(&f.work.claim).unwrap(), Some(local.clone()));
    assert_eq!(
        f.local
            .version_text(&remote, "pages/a.md", &f.work.claim)
            .unwrap(),
        "remote competitor\n"
    );
    let decision = f.native.confirm_remote(&consent, pull).unwrap();
    let candidate = f
        .work
        .with(|w| {
            f.local
                .prepare_remote_confirmation("merge-inspected", &decision, &consent, w)
        })
        .unwrap();
    let later = f.commit(&f.local, "local-later", "local after confirmation\n");
    assert!(matches!(
        f.local.publish("merge-inspected", &f.work.claim).unwrap(),
        State::Conflict(_)
    ));
    assert_eq!(
        f.local
            .read_text(
                candidate.candidate.as_ref().unwrap(),
                "pages/a.md",
                &f.work.claim
            )
            .unwrap(),
        "remote competitor\n"
    );
    let stale = f
        .native
        .approve_push(&consent, "stale-push", &later, Some(&base))
        .unwrap();
    assert!(matches!(
        f.work
            .with(|w| f.local.push_remote(&stale, &consent, w))
            .unwrap(),
        RemotePush::Refused { .. }
    ));
    let non_ff = f
        .native
        .approve_push(&consent, "non-ff-push", &later, Some(&remote))
        .unwrap();
    assert!(matches!(
        f.work
            .with(|w| f.local.push_remote(&non_ff, &consent, w))
            .unwrap(),
        RemotePush::Refused { .. }
    ));
    assert_eq!(f.remote.head(&f.work.claim).unwrap(), Some(remote));
    assert_eq!(f.local.head(&f.work.claim).unwrap(), Some(later));
}

/// history.md: no active config/helpers/hooks, URL data, read-only scope or revoked selection can authorize effects.
#[test]
fn active_config_never_executes_and_native_intention_is_not_attribution_or_key_data() {
    if isolated() {
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(Mode::UnsignedExplicit);
    let local = f.commit(&f.local, "local", "local\n");
    let marker = f.root.path().join("unexpected-hook");
    let hook = f.root.path().join("remote.git/hooks/pre-receive");
    fs::create_dir(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nprintf unexpected > '{}'\nexit 1\n",
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    let consent = f.consent(RemoteEffects::FetchAndPush);
    let approval = f
        .native
        .approve_push(&consent, "new-ref", &local, None)
        .unwrap();
    assert_eq!(
        f.work
            .with(|w| f.local.push_remote(&approval, &consent, w))
            .unwrap(),
        RemotePush::Published(local)
    );
    assert!(!marker.exists(), "remote fixture hook must stay inert");
    let read_only = f.consent(RemoteEffects::FetchOnly);
    assert!(matches!(
        f.native.approve_push(
            &read_only,
            "denied",
            &f.local.head(&f.work.claim).unwrap().unwrap(),
            None
        ),
        Err(Error::Denied)
    ));
    let original_config = fs::read_to_string(f.root.path().join("remote.git/config")).unwrap();
    let mut config = original_config.clone();
    config.push_str("[uploadpack]\n\tpackObjectsHook = imported-executable\n");
    fs::write(f.root.path().join("remote.git/config"), config).unwrap();
    assert!(matches!(
        f.work
            .with(|w| f.local.pull_remote("active-config", &consent, w)),
        Err(Error::Invalid)
    ));
    f.native.revoke();
    assert!(matches!(
        f.work.with(|w| f.local.pull_remote("revoked", &consent, w)),
        Err(Error::Denied)
    ));
    assert!(!marker.exists());
    fs::write(f.root.path().join("remote.git/config"), original_config).unwrap();
}

/// history/service: identical attribution/IDs cannot mint a native intention, and a symbolic remote ref cannot widen its scope.
#[test]
fn native_intention_and_exact_remote_ref_are_separate_from_matching_data() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    let base = f.commit(&f.remote, "seed", "remote\n");
    let candidate = f.commit(&f.local, "local", "local\n");
    let consent = f.consent(RemoteEffects::FetchAndPush);
    let independent = NativeHistoryOwner::select(&f.local, "native-owner", "fixture").unwrap();
    assert!(matches!(
        independent.approve_push(&consent, "not-the-live-owner", &candidate, Some(&base)),
        Err(Error::Denied)
    ));
    f.git(
        "remote.git",
        &["update-ref", "refs/heads/not-selected", base.as_str()],
    );
    f.git(
        "remote.git",
        &[
            "symbolic-ref",
            "refs/heads/omni-accepted",
            "refs/heads/not-selected",
        ],
    );
    let approval = f
        .native
        .approve_push(&consent, "redirected", &candidate, Some(&base))
        .unwrap();
    assert!(matches!(
        f.work.with(|w| f.local.push_remote(&approval, &consent, w)),
        Err(Error::Invalid)
    ));
    assert_eq!(
        f.git("remote.git", &["rev-parse", "refs/heads/not-selected"]),
        format!("{}\n", base.as_str()).as_bytes()
    );
    assert_eq!(f.local.head(&f.work.claim).unwrap(), Some(candidate));
}

/// Core jobs and history.md: full-key drift, insufficient prefunding and cancellation stop actual remote work.
#[test]
fn full_key_reservation_and_actual_queue_cancellation_are_checked_before_effects() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    f.commit(&f.remote, "seed", "seed\n");
    let consent = f.consent(RemoteEffects::FetchOnly);
    f.work.with(|w| {
        let bad = Key {
            source: "other-source",
            ..w.key
        };
        assert!(matches!(
            f.local.pull_remote(
                "wrong-key",
                &consent,
                HistoryWork {
                    claim: w.claim,
                    key: bad
                }
            ),
            Err(Error::ContextMismatch)
        ));
        f.work.queue.cancel(f.work.claim.job(), w.key).unwrap();
        assert!(matches!(
            f.local.pull_remote("cancelled", &consent, w),
            Err(Error::Cancelled)
        ));
    });
    assert_eq!(
        f.git(
            "local.git",
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/omni-proposals/"
            ]
        ),
        b""
    );
    let q = queue();
    f.work.with(|w| {
        let key = Key {
            source: "underfunded",
            ..w.key
        };
        let mut s = submission("underfunded", key);
        s.reservation = Reservation {
            memory_bytes: 1,
            temporary_bytes: 1,
        };
        q.submit(s).unwrap();
        let claim = Arc::new(q.claim(Instant::now()).unwrap().unwrap());
        assert!(matches!(
            f.local
                .pull_remote("underfunded", &consent, HistoryWork { claim: &claim, key }),
            Err(Error::Capacity)
        ));
        q.settle(&claim, key, Outcome::Failed).unwrap();
    });
}

/// Real LocalMount policy is an outer admission boundary, not a replacement for the tested filesystem.
#[derive(Clone)]
struct Policy(Arc<AtomicBool>);
impl LocalReadPolicy for Policy {
    /// Recheck actual current policy on every native source read.
    fn decide(&self) -> ReadDecision {
        if !self.0.load(Ordering::Acquire) {
            return ReadDecision::Deny;
        }
        ReadDecision::Admit(CurrentAdmission {
            library_id: "fixture".into(),
            mount_id: None,
            revision: 1,
            locality: LocalityPremise::OrdinaryLocal,
            outside_scope: vec![],
        })
    }
}
/// Actual local source selection, with a known fixture digest subsequently reverified by real LocalMount.
fn source(
    backend: &impl MountBackend,
    path: &str,
    bytes: &[u8],
    observation: &str,
) -> ExternalText {
    let snapshot = backend
        .stat(&RelativePath::new(path).unwrap(), budget())
        .unwrap()
        .snapshot
        .unwrap();
    ExternalText {
        version: ExpectedVersion {
            digest: ContentDigest::new(format!("{:x}", Sha256::digest(bytes))).unwrap(),
            snapshot,
        },
        observation_id: observation.into(),
        deleted: false,
    }
}
/// Use the public LocalMount producer, never a hand-written fake filesystem or private Storage body.
fn backend(f: &Fixture) -> (LocalMount<Policy>, Arc<AtomicBool>) {
    let enabled = Arc::new(AtomicBool::new(true));
    (
        LocalMount::open(
            &f.root.path().join("home"),
            "fixture",
            None,
            Policy(enabled.clone()),
        )
        .unwrap(),
        enabled,
    )
}
/// history.md external edits: quiet/stable multi-file operation, external_unknown, restart and no duplicate periodic commit.
#[test]
fn actual_quiet_multifile_batch_reopen_and_unchanged_observation() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    fs::create_dir(f.root.path().join("home/pages")).unwrap();
    fs::write(f.root.path().join("home/pages/a.md"), "external a\n").unwrap();
    fs::write(f.root.path().join("home/pages/b.md"), "external b\n").unwrap();
    let (backend, _) = backend(&f);
    let sources = vec![
        source(&backend, "pages/a.md", b"external a\n", "observe-a"),
        source(&backend, "pages/b.md", b"external b\n", "observe-b"),
    ];
    let work = Work::new(external_batch_version(&sources).unwrap(), "external-batch");
    let quiet = Duration::from_millis(20);
    assert!(matches!(
        work.with(|w| f
            .local
            .observe_external_batch("batch", &sources, &backend, quiet, &f.native, w))
            .unwrap(),
        ExternalBatchState::Waiting { .. }
    ));
    assert_eq!(f.local.head(&f.work.claim).unwrap(), None);
    std::thread::sleep(quiet);
    assert!(matches!(
        work.with(|w| f
            .local
            .observe_external_batch("batch", &sources, &backend, quiet, &f.native, w))
            .unwrap(),
        ExternalBatchState::History(State::Published(_))
    ));
    let head = f.local.head(&f.work.claim).unwrap().unwrap();
    let info = f.local.version(&head, &f.work.claim).unwrap();
    assert_eq!(info.attribution.origin.as_deref(), Some("external_unknown"));
    assert_eq!(
        f.local
            .version_text(&head, "pages/b.md", &f.work.claim)
            .unwrap(),
        "external b\n"
    );
    let reopened = History::open(
        &f.root.path().join("local.git"),
        &f.root.path().join("home"),
        "pages",
        owner(Mode::UnsignedExplicit),
        &work.claim,
    )
    .unwrap();
    let native = NativeHistoryOwner::select(&reopened, "native-owner", "fixture").unwrap();
    assert!(matches!(
        work.with(
            |w| reopened.observe_external_batch("batch", &sources, &backend, quiet, &native, w)
        )
        .unwrap(),
        ExternalBatchState::History(State::Published(_))
    ));
    assert!(matches!(
        work.with(|w| reopened.observe_external_batch(
            "unchanged",
            &sources,
            &backend,
            quiet,
            &native,
            w
        ))
        .unwrap(),
        ExternalBatchState::Waiting { .. }
    ));
    std::thread::sleep(quiet);
    assert!(matches!(
        work.with(|w| reopened.observe_external_batch(
            "unchanged",
            &sources,
            &backend,
            quiet,
            &native,
            w
        ))
        .unwrap(),
        ExternalBatchState::History(State::Unchanged(_))
    ));
    assert_eq!(reopened.head(&work.claim).unwrap(), Some(head));
    assert_eq!(
        f.git(
            "local.git",
            &["rev-list", "--count", "refs/heads/omni-accepted"]
        ),
        b"1\n"
    );
}

/// history.md external edits: a proven quiet deletion changes history once; source reappearance refuses without byte loss.
#[test]
fn actual_quiet_deletion_and_reappearance_preserve_filesystem_authority() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    f.commit(&f.local, "initial", "was present\n");
    fs::create_dir(f.root.path().join("home/pages")).unwrap();
    let path = f.root.path().join("home/pages/a.md");
    fs::write(&path, "was present\n").unwrap();
    let (backend, _) = backend(&f);
    let mut deleted = source(&backend, "pages/a.md", b"was present\n", "observed-delete");
    deleted.deleted = true;
    fs::remove_file(&path).unwrap();
    let sources = vec![deleted];
    let work = Work::new(external_batch_version(&sources).unwrap(), "deletion-batch");
    let quiet = Duration::from_millis(10);
    work.with(|w| {
        f.local
            .observe_external_batch("deleted", &sources, &backend, quiet, &f.native, w)
    })
    .unwrap();
    std::thread::sleep(quiet);
    assert!(matches!(
        work.with(|w| f
            .local
            .observe_external_batch("deleted", &sources, &backend, quiet, &f.native, w))
            .unwrap(),
        ExternalBatchState::History(State::Published(_))
    ));
    let head = f.local.head(&work.claim).unwrap().unwrap();
    assert!(f.local.read_text(&head, "pages/a.md", &work.claim).is_err());
    assert!(!path.exists());
    work.with(|w| {
        f.local
            .observe_external_batch("reappeared", &sources, &backend, quiet, &f.native, w)
    })
    .unwrap();
    fs::write(&path, "new unrelated writer\n").unwrap();
    std::thread::sleep(quiet);
    assert!(matches!(
        work.with(|w| f.local.observe_external_batch(
            "reappeared",
            &sources,
            &backend,
            quiet,
            &f.native,
            w
        )),
        Err(Error::Conflict)
    ));
    assert_eq!(f.local.head(&work.claim).unwrap(), Some(head));
    assert_eq!(fs::read_to_string(&path).unwrap(), "new unrelated writer\n");
}

/// history.md: changing/denied sources never become accepted history; missing required signer preserves the exact candidate.
#[test]
fn actual_source_change_policy_revocation_and_signing_required_preserve_candidates() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::SigningRequired);
    fs::create_dir(f.root.path().join("home/pages")).unwrap();
    fs::write(f.root.path().join("home/pages/a.md"), "stable before\n").unwrap();
    let (backend, enabled) = backend(&f);
    let sources = vec![source(
        &backend,
        "pages/a.md",
        b"stable before\n",
        "observed",
    )];
    let work = Work::new(external_batch_version(&sources).unwrap(), "external-batch");
    let quiet = Duration::from_millis(10);
    work.with(|w| {
        f.local
            .observe_external_batch("blocked", &sources, &backend, quiet, &f.native, w)
    })
    .unwrap();
    std::thread::sleep(quiet);
    assert!(matches!(
        work.with(|w| f
            .local
            .observe_external_batch("blocked", &sources, &backend, quiet, &f.native, w)),
        Err(Error::SigningRequired)
    ));
    assert_eq!(
        f.local
            .read_candidate("blocked", "pages/a.md", &work.claim)
            .unwrap(),
        "stable before\n"
    );
    fs::write(f.root.path().join("home/pages/a.md"), "changed after\n").unwrap();
    assert!(matches!(
        work.with(|w| f
            .local
            .observe_external_batch("blocked", &sources, &backend, quiet, &f.native, w)),
        Err(Error::Conflict)
    ));
    assert_eq!(
        f.local
            .read_candidate("blocked", "pages/a.md", &work.claim)
            .unwrap(),
        "stable before\n"
    );
    enabled.store(false, Ordering::Release);
    assert!(matches!(
        work.with(|w| f
            .local
            .observe_external_batch("denied", &sources, &backend, quiet, &f.native, w)),
        Err(Error::Denied)
    ));
    assert_eq!(f.local.head(&work.claim).unwrap(), None);
}

/// history.md schedule: actual existing queue submission, disabled default, stable restart descriptor and due/status completion.
#[test]
fn actual_remote_schedule_queue_restart_idempotency_and_next_due() {
    if isolated() {
        return;
    }
    if std::env::var("OMNI_HISTORY_RESTART_PHASE").unwrap() == "seed" {
        let f = Fixture::new(Mode::UnsignedExplicit);
        f.commit(&f.remote, "seed", "seed\n");
        let consent = f.consent(RemoteEffects::FetchOnly);
        assert!(
            !f.local
                .schedule_status("remote", &f.native)
                .unwrap()
                .enabled
        );
        let (status, schedule) = f
            .work
            .with(|w| {
                f.local
                    .enable_remote_schedule("remote", &consent, 1, false, w)
            })
            .unwrap();
        let q = queue();
        let bootstrap_jobs = q.usage().unwrap().jobs;
        f.work.with(|w| {
            let recipe = status.work_recipe().unwrap();
            let key = Key {
                recipe: &recipe,
                ..w.key
            };
            // Deliberate corruption after valid public setup: durable flags cannot widen live native schedule intention.
            let settings = f
                .root
                .path()
                .join("local.git/omni-state/schedule-remote.json");
            let mut widened = status.clone();
            widened.allow_push = true;
            fs::write(&settings, serde_json::to_vec(&widened).unwrap()).unwrap();
            assert!(matches!(
                f.local.submit_remote_schedule(
                    "remote",
                    &schedule,
                    &consent,
                    &q,
                    submission(&status.operation_id, key)
                ),
                Err(Error::Denied)
            ));
            assert_eq!(q.usage().unwrap().jobs - bootstrap_jobs, 0);
            fs::write(&settings, serde_json::to_vec(&status).unwrap()).unwrap();

            let first = f
                .local
                .submit_remote_schedule(
                    "remote",
                    &schedule,
                    &consent,
                    &q,
                    submission(&status.operation_id, key),
                )
                .unwrap()
                .unwrap();
            let duplicate = f
                .local
                .submit_remote_schedule(
                    "remote",
                    &schedule,
                    &consent,
                    &q,
                    submission(&status.operation_id, key),
                )
                .unwrap()
                .unwrap();
            assert_eq!(first.job, duplicate.job);
            assert!(!duplicate.created);
        });
        // No scheduled worker was claimed. Drop/collect every actual native setup owner before seed exit.
        assert!(f.local.pending_process(&f.work.claim).unwrap().is_none());
        fs::write(
            f.root.path().join("fixture-schedule-status.json"),
            serde_json::to_vec(&status).unwrap(),
        )
        .unwrap();
        return;
    }
    let root = PathBuf::from(std::env::var_os("OMNI_HISTORY_RESTART_ROOT").unwrap());
    let work = Work::new(
        ContentDigest::new("a".repeat(64)).unwrap(),
        "remote-selection",
    );
    let local = History::open(
        &root.join("local.git"),
        &root.join("home"),
        "pages",
        owner(Mode::UnsignedExplicit),
        &work.claim,
    )
    .unwrap();
    let remote = History::open(
        &root.join("remote.git"),
        &root.join("remote-home"),
        "pages",
        owner(Mode::UnsignedExplicit),
        &work.claim,
    )
    .unwrap();
    let native = NativeHistoryOwner::select(&local, "native-owner", "fixture").unwrap();
    let mut f = Fixture {
        root: FixtureRoot {
            owned: None,
            path: root,
        },
        local,
        remote,
        work,
        native,
    };
    let status: knowledge_history::HistorySchedule = serde_json::from_slice(
        &fs::read(f.root.path().join("fixture-schedule-status.json")).unwrap(),
    )
    .unwrap();
    f.native.revoke();
    let reopened = History::open(
        &f.root.path().join("local.git"),
        &f.root.path().join("home"),
        "pages",
        owner(Mode::UnsignedExplicit),
        &f.work.claim,
    )
    .unwrap();
    drop(std::mem::replace(&mut f.local, reopened));
    f.native = NativeHistoryOwner::select(&f.local, "native-owner", "fixture").unwrap();
    let consent = f.consent(RemoteEffects::FetchOnly);
    // A fresh actual native decision, not a deserialized enabled flag, restores the exact previous scope.
    let (restored, schedule) = f
        .work
        .with(|w| {
            f.local
                .enable_remote_schedule("remote", &consent, 1, false, w)
        })
        .unwrap();
    assert_eq!(restored.operation_id, status.operation_id);
    let restarted = queue();
    let bootstrap_unsettled = restarted.usage().unwrap().unsettled;
    f.work.with(|w| {
        let recipe = status.work_recipe().unwrap();
        let key = Key {
            recipe: &recipe,
            ..w.key
        };

        let submitted = f
            .local
            .submit_remote_schedule(
                "remote",
                &schedule,
                &consent,
                &restarted,
                submission(&status.operation_id, key),
            )
            .unwrap()
            .unwrap();
        let claim = Arc::new(restarted.claim(Instant::now()).unwrap().unwrap());
        assert_eq!(claim.job(), submitted.job);
        let actual = HistoryWork { claim: &claim, key };
        let result = f
            .local
            .run_remote_schedule("remote", &schedule, &consent, actual)
            .unwrap();
        assert_eq!(result.pull.relation, RemoteRelation::Initial);
        assert!(result.push.is_none());
        let completed = f
            .local
            .finish_schedule("remote", &schedule, actual)
            .unwrap();
        assert_ne!(completed.operation_id, status.operation_id);
        assert!(completed.next_run_unix_millis > status.next_run_unix_millis);
        assert!(
            f.local
                .submit_remote_schedule(
                    "remote",
                    &schedule,
                    &consent,
                    &restarted,
                    submission(&completed.operation_id, key)
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(
            restarted.usage().unwrap().unsettled - bootstrap_unsettled,
            0
        );
    });
    assert_eq!(f.local.head(&f.work.claim).unwrap(), None);
}

/// storage/history schedule: actual queued source versions, quiet waiting and budget waiting never become metadata-only success.
#[test]
fn actual_external_schedule_commits_and_queue_budget_waiting_is_visible() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    fs::create_dir(f.root.path().join("home/pages")).unwrap();
    fs::write(f.root.path().join("home/pages/a.md"), "queued external\n").unwrap();
    let (backend, _) = backend(&f);
    let sources = vec![source(
        &backend,
        "pages/a.md",
        b"queued external\n",
        "queue-observation",
    )];
    let work = Work::new(external_batch_version(&sources).unwrap(), "batch-schedule");
    let (status, schedule) = work
        .with(|w| f.local.enable_external_schedule("text", &f.native, 1, w))
        .unwrap();
    let q = queue();
    let bootstrap_unsettled = q.usage().unwrap().unsettled;
    work.with(|w| {
        let recipe = status.work_recipe().unwrap();
        let key = Key {
            recipe: &recipe,
            ..w.key
        };

        let job = f
            .local
            .submit_external_schedule("text", &schedule, &q, submission(&status.operation_id, key))
            .unwrap()
            .unwrap();
        let claim = Arc::new(q.claim(Instant::now()).unwrap().unwrap());
        assert_eq!(job.job, claim.job());
        let actual = HistoryWork { claim: &claim, key };
        let quiet = Duration::from_millis(20);
        assert!(matches!(
            f.local
                .run_external_schedule("text", &sources, &backend, quiet, &schedule, actual)
                .unwrap(),
            ExternalBatchState::Waiting { .. }
        ));
        assert!(f.local.finish_schedule("text", &schedule, actual).is_err());
        assert_eq!(q.usage().unwrap().unsettled - bootstrap_unsettled, 1);
        std::thread::sleep(quiet);
        assert!(matches!(
            f.local
                .run_external_schedule("text", &sources, &backend, quiet, &schedule, actual)
                .unwrap(),
            ExternalBatchState::History(State::Published(_))
        ));
        f.local.finish_schedule("text", &schedule, actual).unwrap();
        assert_eq!(q.usage().unwrap().unsettled - bootstrap_unsettled, 0);
    });
    // A real native disable/re-enable makes the next occurrence due now, without a fake clock or a longer test deadline.
    work.with(|w| {
        f.local.disable_schedule("text", &f.native, w).unwrap();
        let (next, next_schedule) = f
            .local
            .enable_external_schedule("text", &f.native, 1, w)
            .unwrap();
        let recipe = next.work_recipe().unwrap();
        assert_ne!(recipe, status.work_recipe().unwrap());
        let key = Key {
            recipe: &recipe,
            ..w.key
        };
        let job = f
            .local
            .submit_external_schedule(
                "text",
                &next_schedule,
                &q,
                submission(&next.operation_id, key),
            )
            .unwrap()
            .unwrap();
        assert!(
            job.created,
            "terminal-key dedup must not swallow a later scheduled observation"
        );
        let claim = Arc::new(q.claim(Instant::now()).unwrap().unwrap());
        let actual = HistoryWork { claim: &claim, key };
        let quiet = Duration::from_millis(20);
        assert!(matches!(
            f.local
                .run_external_schedule("text", &sources, &backend, quiet, &next_schedule, actual)
                .unwrap(),
            ExternalBatchState::Waiting { .. }
        ));
        std::thread::sleep(quiet);
        assert!(matches!(
            f.local
                .run_external_schedule("text", &sources, &backend, quiet, &next_schedule, actual)
                .unwrap(),
            ExternalBatchState::History(State::Unchanged(_))
        ));
        f.local
            .finish_schedule("text", &next_schedule, actual)
            .unwrap();
    });
    assert_eq!(
        f.git(
            "local.git",
            &["rev-list", "--count", "refs/heads/omni-accepted"]
        ),
        b"1\n"
    );
    // Reserve all memory in one real job. Another admitted descriptor remains visibly queued, not falsely complete.
    let limited = Queue::new(Limits {
        max_pending: 4,
        max_records: 4,
        max_descriptor_bytes: 8192,
        per_mount: 2,
        memory_bytes: 64 << 20,
        temporary_bytes: 64 << 20,
    })
    .unwrap();
    work.with(|w| {
        limited.submit(submission("holding", w.key)).unwrap();
        let held = limited.claim(Instant::now()).unwrap().unwrap();
        let other_key = Key {
            source: "another-source",
            ..w.key
        };
        let pending = limited.submit(submission("waiting", other_key)).unwrap();
        assert!(limited.claim(Instant::now()).unwrap().is_none());
        assert!(matches!(
            limited.status(pending.job, other_key).unwrap().state,
            knowledge_core::jobs::State::Queued
        ));
        limited.settle(&held, w.key, Outcome::Completed).unwrap();
        let next = limited.claim(Instant::now()).unwrap().unwrap();
        limited
            .settle(&next, other_key, Outcome::Cancelled)
            .unwrap();
        let mut too_large = submission(
            "too-large",
            Key {
                source: "too-large",
                ..w.key
            },
        );
        too_large.reservation.memory_bytes = (64 << 20) + 1;
        assert!(matches!(
            limited.submit(too_large),
            Err(knowledge_core::jobs::JobError::Capacity)
        ));
    });
}

/// history.md: a real receive-side lock refusal and uncertain restarted push never replay over a newer writer.
#[test]
fn actual_receive_refusal_and_uncertain_restart_never_overwrite_remote_writer() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    let base = f.commit(&f.remote, "remote-first", "base\n");
    let consent = f.consent(RemoteEffects::FetchAndPush);
    let pull = f
        .work
        .with(|w| f.local.pull_remote("initial", &consent, w))
        .unwrap();
    let approval = f.native.confirm_remote(&consent, pull).unwrap();
    f.work
        .with(|w| {
            f.local
                .prepare_remote_confirmation("initial-integration", &approval, &consent, w)
        })
        .unwrap();
    f.local
        .publish("initial-integration", &f.work.claim)
        .unwrap();
    let candidate = f.commit(&f.local, "local-change", "candidate\n");
    let locked = f
        .native
        .approve_push(&consent, "locked-push", &candidate, Some(&base))
        .unwrap();
    let lock_path = f
        .root
        .path()
        .join("remote.git/refs/heads/omni-accepted.lock");
    let lock = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    assert!(matches!(
        f.work
            .with(|w| f.local.push_remote(&locked, &consent, w))
            .unwrap(),
        RemotePush::Refused { .. }
    ));
    drop(lock);
    fs::remove_file(lock_path).unwrap();
    assert_eq!(f.remote.head(&f.work.claim).unwrap(), Some(base.clone()));
    let successful = f
        .native
        .approve_push(&consent, "interrupted-receipt", &candidate, Some(&base))
        .unwrap();
    assert_eq!(
        f.work
            .with(|w| f.local.push_remote(&successful, &consent, w))
            .unwrap(),
        RemotePush::Published(candidate.clone())
    );
    // Deliberate interruption after valid public setup: erase only this disposable operation's final receipt.
    fs::remove_file(
        f.root
            .path()
            .join("local.git/omni-state/operations/interrupted-receipt.pushed.json"),
    )
    .unwrap();
    let other = f.commit(&f.remote, "later-remote", "other writer\n");
    let reopened = History::open(
        &f.root.path().join("local.git"),
        &f.root.path().join("home"),
        "pages",
        owner(Mode::UnsignedExplicit),
        &f.work.claim,
    )
    .unwrap();
    let native = NativeHistoryOwner::select(&reopened, "native-owner", "fixture").unwrap();
    let selected = f
        .work
        .with(|w| {
            native.approve_local_remote(
                &reopened,
                &f.root.path().join("remote.git"),
                "refs/heads/omni-accepted",
                RemoteEffects::FetchAndPush,
                w,
            )
        })
        .unwrap();
    let retry = native
        .approve_push(&selected, "interrupted-receipt", &candidate, Some(&base))
        .unwrap();
    assert_eq!(
        f.work
            .with(|w| reopened.push_remote(&retry, &selected, w))
            .unwrap(),
        RemotePush::Indeterminate {
            candidate: candidate.clone()
        }
    );
    assert_eq!(f.remote.head(&f.work.claim).unwrap(), Some(other));
    assert_eq!(reopened.head(&f.work.claim).unwrap(), Some(candidate));
}

/// history.md: actual offline settlement/backoff, cancellation and disablement never assert unknown work completed.
#[test]
fn actual_offline_schedule_backoff_and_cancelled_worker_do_not_complete() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    f.commit(&f.remote, "remote-seed", "seed\n");
    let consent = f.consent(RemoteEffects::FetchOnly);
    let (status, schedule) = f
        .work
        .with(|w| {
            f.local
                .enable_remote_schedule("offline", &consent, 1, false, w)
        })
        .unwrap();
    let q = queue();
    f.work.with(|w| {
        let recipe = status.work_recipe().unwrap();
        let key = Key {
            recipe: &recipe,
            ..w.key
        };

        let job = f
            .local
            .submit_remote_schedule(
                "offline",
                &schedule,
                &consent,
                &q,
                submission(&status.operation_id, key),
            )
            .unwrap()
            .unwrap();
        let claim = Arc::new(q.claim(Instant::now()).unwrap().unwrap());
        fs::rename(
            f.root.path().join("remote.git"),
            f.root.path().join("offline.git"),
        )
        .unwrap();
        assert!(
            f.local
                .run_remote_schedule(
                    "offline",
                    &schedule,
                    &consent,
                    HistoryWork { claim: &claim, key }
                )
                .is_err()
        );
        fs::rename(
            f.root.path().join("offline.git"),
            f.root.path().join("remote.git"),
        )
        .unwrap();
        // The synchronous call has no pending owner: native I/O/offline failure is actually settled before reporting Offline.
        assert!(f.local.pending_process(&claim).unwrap().is_none());
        q.settle(&claim, key, Outcome::Offline).unwrap();
        let waiting = f
            .local
            .backoff_schedule("offline", &schedule, &q, job.job, key)
            .unwrap();
        assert_eq!(waiting.attempts, 1);
        assert_eq!(waiting.operation_id, status.operation_id);
        assert!(waiting.next_run_unix_millis > status.next_run_unix_millis);
        assert!(
            f.local
                .submit_remote_schedule(
                    "offline",
                    &schedule,
                    &consent,
                    &q,
                    submission(&waiting.operation_id, key)
                )
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            q.status(job.job, key).unwrap().state,
            knowledge_core::jobs::State::WaitingOffline
        ));
        let (cancelled, cancel_schedule) = f
            .work
            .with(|setup| {
                f.local
                    .enable_remote_schedule("cancel", &consent, 1, false, setup)
            })
            .unwrap();
        let recipe = cancelled.work_recipe().unwrap();
        let key = Key {
            recipe: &recipe,
            ..w.key
        };
        let q2 = queue();
        f.local
            .submit_remote_schedule(
                "cancel",
                &cancel_schedule,
                &consent,
                &q2,
                submission(&cancelled.operation_id, key),
            )
            .unwrap()
            .unwrap();
        let cancelled_claim = Arc::new(q2.claim(Instant::now()).unwrap().unwrap());
        q2.cancel(cancelled_claim.job(), key).unwrap();
        let actual = HistoryWork {
            claim: &cancelled_claim,
            key,
        };
        assert!(matches!(
            f.local
                .run_remote_schedule("cancel", &cancel_schedule, &consent, actual),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            f.local.finish_schedule("cancel", &cancel_schedule, actual),
            Err(Error::Cancelled)
        ));
        q2.settle(&cancelled_claim, key, Outcome::Cancelled)
            .unwrap();
        f.local.disable_schedule("cancel", &f.native, w).unwrap();
        assert!(
            !f.local
                .schedule_status("cancel", &f.native)
                .unwrap()
                .enabled
        );
    });
    assert_eq!(f.local.head(&f.work.claim).unwrap(), None);
}

/// history.md: too-small actual pack/output admission refuses without a proposal, accepted mutation or source loss.
#[test]
fn actual_remote_pack_budget_refusal_keeps_source_and_local_refs() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    let text: String = (0..8192)
        .map(|i| char::from(b'a' + ((i * 17 + i / 11) % 26) as u8))
        .collect();
    let source = f.commit(&f.remote, "large-source", &text);
    let consent = f.consent(RemoteEffects::FetchOnly);
    let q = queue();
    f.work.with(|w| {
        let key = Key {
            source: "small-work",
            ..w.key
        };
        let mut s = submission("small-work", key);
        s.budget.max_bytes = 4096;
        s.reservation = History::required_resources(&s.budget).unwrap();
        q.submit(s).unwrap();
        let claim = Arc::new(q.claim(Instant::now()).unwrap().unwrap());
        assert!(matches!(
            f.local
                .pull_remote("too-large", &consent, HistoryWork { claim: &claim, key }),
            Err(Error::Capacity) | Err(Error::Git)
        ));
        assert!(f.local.pending_process(&claim).unwrap().is_none());
        q.settle(&claim, key, Outcome::Failed).unwrap();
    });
    assert_eq!(
        f.remote
            .read_text(&source, "pages/a.md", &f.work.claim)
            .unwrap(),
        text
    );
    assert_eq!(f.local.head(&f.work.claim).unwrap(), None);
    assert_eq!(
        f.git(
            "local.git",
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/omni-proposals/"
            ]
        ),
        b""
    );
}

/// An owned foreground agent on disk-backed /var/tmp; no real user's key, socket, agent or configuration is used.
struct Agent {
    /// Exact owned unreaped foreground agent.
    child: Child,
    /// Owned credential/socket directory; retained on unknown cleanup.
    root: Option<tempfile::TempDir>,
}
impl Agent {
    /// Start only this finite owned fixture and wait for its own socket.
    fn new() -> Self {
        let root = tempfile::tempdir_in(Path::new("/var/tmp").canonicalize().unwrap()).unwrap();
        let child = Command::new("/usr/bin/ssh-agent")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-D", "-a"])
            .arg(root.path().join("agent.sock"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let result = Self {
            child,
            root: Some(root),
        };
        let end = Instant::now() + Duration::from_secs(2);
        while !result.socket().exists() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        }
        result
    }
    /// Return only the explicitly owned socket, never ambient SSH_AUTH_SOCK.
    fn socket(&self) -> PathBuf {
        self.root.as_ref().unwrap().path().join("agent.sock")
    }
}
impl Drop for Agent {
    /// Reap only the actual held Child; unknown cleanup retains the directory rather than discarding a credential/native owner.
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
            let end = Instant::now() + Duration::from_secs(2);
            while self.child.try_wait().unwrap().is_none() {
                if Instant::now() >= end {
                    let _ = self.root.take().unwrap().keep();
                    panic!("owned agent collection unknown");
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
/// history.md: explicitly enabled scheduled push performs actual leased I/O without manufacturing integration approval.
#[test]
fn actual_enabled_schedule_pushes_only_fast_forward_local_history() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    f.commit(&f.remote, "seed", "remote\n");
    let consent = f.consent(RemoteEffects::FetchAndPush);
    let pull = f
        .work
        .with(|w| f.local.pull_remote("initial-fetch", &consent, w))
        .unwrap();
    let decision = f.native.confirm_remote(&consent, pull).unwrap();
    f.work
        .with(|w| {
            f.local
                .prepare_remote_confirmation("initial-merge", &decision, &consent, w)
        })
        .unwrap();
    f.local.publish("initial-merge", &f.work.claim).unwrap();
    let candidate = f.commit(&f.local, "local-ahead", "local\n");
    let (status, schedule) = f
        .work
        .with(|w| {
            f.local
                .enable_remote_schedule("push-schedule", &consent, 1, true, w)
        })
        .unwrap();
    let q = queue();
    f.work.with(|w| {
        let recipe = status.work_recipe().unwrap();
        let key = Key {
            recipe: &recipe,
            ..w.key
        };

        f.local
            .submit_remote_schedule(
                "push-schedule",
                &schedule,
                &consent,
                &q,
                submission(&status.operation_id, key),
            )
            .unwrap()
            .unwrap();
        let claim = Arc::new(q.claim(Instant::now()).unwrap().unwrap());
        let actual = HistoryWork { claim: &claim, key };
        let result = f
            .local
            .run_remote_schedule("push-schedule", &schedule, &consent, actual)
            .unwrap();
        assert_eq!(result.pull.relation, RemoteRelation::LocalAhead);
        assert_eq!(result.push, Some(RemotePush::Published(candidate.clone())));
        f.local
            .finish_schedule("push-schedule", &schedule, actual)
            .unwrap();
    });
    assert_eq!(
        f.remote.head(&f.work.claim).unwrap(),
        Some(candidate.clone())
    );
    assert_eq!(f.local.head(&f.work.claim).unwrap(), Some(candidate));
}

/// history.md: restart cannot use a persisted wall timestamp to bypass a real new monotonic quiet observation.
#[test]
fn actual_unfinished_batch_restart_observes_a_fresh_quiet_period() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    fs::create_dir(f.root.path().join("home/pages")).unwrap();
    fs::write(f.root.path().join("home/pages/a.md"), "stable\n").unwrap();
    let (backend, _) = backend(&f);
    let sources = vec![source(
        &backend,
        "pages/a.md",
        b"stable\n",
        "observed-before-restart",
    )];
    let work = Work::new(external_batch_version(&sources).unwrap(), "external-batch");
    let quiet = Duration::from_millis(20);
    assert!(matches!(
        work.with(|w| f.local.observe_external_batch(
            "unfinished",
            &sources,
            &backend,
            quiet,
            &f.native,
            w
        ))
        .unwrap(),
        ExternalBatchState::Waiting { .. }
    ));
    std::thread::sleep(quiet);
    let reopened = History::open(
        &f.root.path().join("local.git"),
        &f.root.path().join("home"),
        "pages",
        owner(Mode::UnsignedExplicit),
        &work.claim,
    )
    .unwrap();
    let native = NativeHistoryOwner::select(&reopened, "native-owner", "fixture").unwrap();
    assert!(matches!(
        work.with(|w| reopened.observe_external_batch(
            "unfinished",
            &sources,
            &backend,
            quiet,
            &native,
            w
        ))
        .unwrap(),
        ExternalBatchState::Waiting { .. }
    ));
    assert_eq!(reopened.head(&work.claim).unwrap(), None);
    std::thread::sleep(quiet);
    assert!(matches!(
        work.with(|w| reopened.observe_external_batch(
            "unfinished",
            &sources,
            &backend,
            quiet,
            &native,
            w
        ))
        .unwrap(),
        ExternalBatchState::History(State::Published(_))
    ));
}

/// history.md: actual SSH-signed remote objects retain exact bytes/signatures, without turning known trust into permission.
#[test]
fn actual_signed_remote_fetch_preserves_object_and_verification_without_signer() {
    if isolated() {
        return;
    }
    use knowledge_history::{KeyProtection, KnownSigner, SignatureState};
    let mut f = Fixture::new(Mode::SigningRequired);
    let agent = Agent::new();
    let key_dir = agent.root.as_ref().unwrap().path().join("system-key");
    let signer = f
        .remote
        .setup_system_signer(
            &key_dir,
            &agent.socket(),
            KeyProtection::UnencryptedExplicit,
            &f.work.claim,
        )
        .unwrap();
    let known = KnownSigner::selected("fixture@example.invalid", signer.public_key()).unwrap();
    let replacement = History::open(
        &f.root.path().join("remote.git"),
        &f.root.path().join("remote-home"),
        "pages",
        owner(Mode::SigningRequired),
        &f.work.claim,
    )
    .unwrap();
    let remote = std::mem::replace(&mut f.remote, replacement);
    f.remote = remote.with_signer(signer);
    let signed = f.commit(&f.remote, "signed-remote", "signed remote\n");
    let exact = f.git("remote.git", &["cat-file", "commit", signed.as_str()]);
    let consent = f.consent(RemoteEffects::FetchOnly);
    let fetched = f
        .work
        .with(|w| f.local.pull_remote("signed-pull", &consent, w))
        .unwrap();
    assert_eq!(fetched.fetched, signed);
    assert_eq!(
        f.git("local.git", &["cat-file", "commit", signed.as_str()]),
        exact
    );
    assert_eq!(
        f.local
            .verify_signature(&signed, &[known], &f.work.claim)
            .unwrap()
            .state,
        SignatureState::ValidKnown
    );
    let decision = f.native.confirm_remote(&consent, fetched).unwrap();
    assert!(matches!(
        f.work.with(|w| f.local.prepare_remote_confirmation(
            "signing-blocked",
            &decision,
            &consent,
            w
        )),
        Err(Error::SigningRequired)
    ));
    assert_eq!(
        f.local
            .read_candidate("signing-blocked", "pages/a.md", &f.work.claim)
            .unwrap(),
        "signed remote\n"
    );
    assert_eq!(f.local.head(&f.work.claim).unwrap(), None);
}

/// Core live-generation authority: numerically identical replacement work never revives the old Claim.
#[test]
fn stale_original_claim_cannot_borrow_reclaimed_job_authority() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    f.commit(&f.remote, "generation-remote-seed", "generation fixture\n");
    let consent = f.consent(RemoteEffects::FetchOnly);
    let queue = queue();
    f.work.with(|setup| {
        let key = Key { source: "claim-generation-control", ..setup.key };
        let submitted = Account::current().unwrap().submit(submission("claim-generation-control", key)).unwrap();
        let now = Instant::now();
        let stale = Arc::new(queue.claim(now).unwrap().unwrap());
        assert_eq!(stale.job(), submitted.job);
        queue.settle(&stale, key, Outcome::Offline).unwrap();
        queue.reconnect(submitted.job, key).unwrap();
        let live = Arc::new(queue.claim(now).unwrap().unwrap());
        assert_eq!(stale.job(), live.job());
        assert_eq!(stale.deadline(), live.deadline(), "the deciding discriminator must be actual generation, not deadline equality");
        assert_eq!(stale.budget(), live.budget());
        let before = Account::current().unwrap().status().unwrap().reserved;
        let refs = f.git("local.git", &["for-each-ref", "--format=%(refname):%(objectname)"]);
        assert!(matches!(f.local.pull_remote("stale-generation", &consent, HistoryWork { claim: &stale, key }),
            Err(Error::ContextMismatch)), "old original Claim must refuse while the same job/key is unsettled under its replacement");
        assert_eq!(f.git("local.git", &["for-each-ref", "--format=%(refname):%(objectname)"]), refs);
        assert_eq!(Account::current().unwrap().status().unwrap().reserved, before,
            "refusing stale A cannot borrow, release, or leak replacement B's actual credit");
        assert!(f.local.pending_process(&f.work.claim).unwrap().is_none());
        let result = f.local.pull_remote("live-generation", &consent, HistoryWork { claim: &live, key }).unwrap();
        assert_eq!(result.relation, RemoteRelation::Initial);
        drop(result);
        queue.settle(&live, key, Outcome::Completed).unwrap();
    });
}

/// Storage/history authority: absence in a matching-label foreign root cannot delete selected-home history.
#[test]
fn unrelated_empty_backend_cannot_certify_selected_home_deletion() {
    if isolated() {
        return;
    }
    let f = Fixture::new(Mode::UnsignedExplicit);
    let text = "selected home authoritative text\n";
    let head = f.commit(&f.local, "selected-home-seed", text);
    fs::create_dir(f.root.path().join("home/pages")).unwrap();
    fs::write(f.root.path().join("home/pages/a.md"), text).unwrap();
    let (selected, _) = backend(&f);
    let mut deleted = source(
        &selected,
        "pages/a.md",
        text.as_bytes(),
        "selected-home-deletion",
    );
    deleted.deleted = true;
    let sources = vec![deleted];
    let unrelated = f.root.path().join("unrelated-empty");
    fs::create_dir(&unrelated).unwrap();
    let wrong = LocalMount::open(
        &unrelated,
        "fixture",
        None,
        Policy(Arc::new(AtomicBool::new(true))),
    )
    .unwrap();
    assert_ne!(
        selected.root_identity().unwrap(),
        wrong.root_identity().unwrap()
    );
    let work = Work::new(
        external_batch_version(&sources).unwrap(),
        "selected-home-deletion",
    );
    let quiet = Duration::from_millis(20);
    assert!(
        matches!(
            work.with(|w| f.local.observe_external_batch(
                "wrong-home-deletion",
                &sources,
                &wrong,
                quiet,
                &f.native,
                w
            )),
            Err(Error::ContextMismatch)
        ),
        "matching library labels and real foreign absence supply no selected-home authority"
    );
    assert_eq!(f.local.head(&f.work.claim).unwrap(), Some(head.clone()));
    assert_eq!(
        f.local
            .read_text(&head, "pages/a.md", &f.work.claim)
            .unwrap(),
        text
    );
    assert_eq!(
        fs::read_to_string(f.root.path().join("home/pages/a.md")).unwrap(),
        text
    );
    assert!(
        !f.root
            .path()
            .join("local.git/omni-state/operations/wrong-home-deletion.quiet.json")
            .exists(),
        "wrong-root refusal precedes even a durable quiet-operation intent"
    );
    // The same actual work succeeds only after the fixture deletes the selected original itself.
    fs::remove_file(f.root.path().join("home/pages/a.md")).unwrap();
    assert!(matches!(
        work.with(|w| f.local.observe_external_batch(
            "actual-home-deletion",
            &sources,
            &selected,
            quiet,
            &f.native,
            w
        ))
        .unwrap(),
        ExternalBatchState::Waiting { .. }
    ));
    std::thread::sleep(quiet);
    assert!(matches!(
        work.with(|w| f.local.observe_external_batch(
            "actual-home-deletion",
            &sources,
            &selected,
            quiet,
            &f.native,
            w
        ))
        .unwrap(),
        ExternalBatchState::History(State::Published(_))
    ));
    let deleted_head = f.local.head(&f.work.claim).unwrap().unwrap();
    assert_ne!(deleted_head, head);
    assert!(
        f.local
            .read_text(&deleted_head, "pages/a.md", &f.work.claim)
            .is_err()
    );
    assert_eq!(fs::read_dir(unrelated).unwrap().count(), 0);
}
