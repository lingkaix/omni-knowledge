//! Builder proofs over real disposable repositories and the frozen local public seam.
use knowledge_core::backend::Budget;
use knowledge_core::data::ContentDigest;
use knowledge_core::jobs::{Claim, Key, Limits, Outcome, Priority, Queue, Reservation, Submission};
use knowledge_history::{
    Change, Confirmation, Destination, Error, History, Mode, Oid, OwnerContext, Request, State,
};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

/// Actual existing queue claim; no parallel admission implementation or fabricated token.
struct Admission {
    /// Existing queue owns reservation until settled.
    queue: Queue,
    /// Live nonforgeable executor claim.
    claim: Claim,
    /// Stable synthetic work digest.
    digest: ContentDigest,
}
impl Admission {
    /// Reserve finite synthetic memory/disk/time through the public Core seam.
    fn new() -> Self {
        Self::with_budget(Budget {
            max_files: 100,
            max_bytes: 524_288,
            max_millis: 60_000,
            max_memory_bytes: 64 << 20,
        })
    }
    /// Use explicit existing Core work ceilings for capacity/deadline refusal checks.
    fn with_budget(budget: Budget) -> Self {
        let queue = Queue::new(Limits {
            max_pending: 2,
            max_records: 2,
            max_descriptor_bytes: 4096,
            per_mount: 2,
            memory_bytes: 128 << 20,
            temporary_bytes: 128 << 20,
        })
        .unwrap();
        let digest = ContentDigest::new("a".repeat(64)).unwrap();
        let key = Key {
            context: "selected-native-owner",
            library: "fixture",
            mount: None,
            source: "text-operation",
            version: &digest,
            recipe: &digest,
            processor: &digest,
        };
        queue
            .submit(Submission {
                operation_id: "reserved",
                job_id: "git",
                key,
                budget,
                reservation: Reservation {
                    memory_bytes: 64 << 20,
                    temporary_bytes: 64 << 20,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = queue.claim(Instant::now()).unwrap().unwrap();
        Self {
            queue,
            claim,
            digest,
        }
    }
}
impl Drop for Admission {
    /// Settle only after synchronous history calls have collected their exact children.
    fn drop(&mut self) {
        let key = Key {
            context: "selected-native-owner",
            library: "fixture",
            mount: None,
            source: "text-operation",
            version: &self.digest,
            recipe: &self.digest,
            processor: &self.digest,
        };
        self.queue
            .settle(&self.claim, key, Outcome::Completed)
            .unwrap();
    }
}
/// Disposable synthetic owner, not host Git configuration or a personal identity.
fn owner(agent: bool) -> OwnerContext {
    OwnerContext {
        name: "Synthetic Owner".into(),
        email: "owner@example.invalid".into(),
        agent: agent.then(|| "validated-agent".into()),
        tool: agent.then(|| "edit_text".into()),
        session: agent.then(|| "session-fixture".into()),
        mode: Mode::UnsignedExplicit,
    }
}
/// Owned fixture root below the canonical native temporary directory.
fn canonical_tempdir() -> tempfile::TempDir {
    let parent = std::env::temp_dir().canonicalize().unwrap();
    tempfile::Builder::new().tempdir_in(parent).unwrap()
}

/// Real local repository plus separate visible home and public reservation.
struct Fixture {
    /// All owned state is removed only with this disposable root.
    root: tempfile::TempDir,
    /// Live shared-seam admission.
    admission: Admission,
    /// Actual history owner.
    history: History,
}
impl Fixture {
    /// Initialize through public History::init, never handwrite owner state.
    fn new() -> Self {
        let root = canonical_tempdir();
        fs::create_dir(root.path().join("home")).unwrap();
        let admission = Admission::new();
        let history = History::init(
            &root.path().join("history"),
            &root.path().join("home"),
            "pages",
            owner(false),
            &admission.claim,
        )
        .unwrap();
        Self {
            root,
            admission,
            history,
        }
    }
    /// Return the selected owned Git directory.
    fn repo(&self) -> std::path::PathBuf {
        self.root.path().join("history")
    }
    /// Build an accepted multi-path request against a selected expected version.
    fn request(&self, id: &str, base: Option<Oid>, values: &[(&str, Option<&str>)]) -> Request {
        Request {
            operation_id: id.into(),
            base,
            destination: Destination::Accepted,
            changes: values
                .iter()
                .map(|(path, text)| Change {
                    path: (*path).into(),
                    text: text.map(str::to_owned),
                })
                .collect(),
        }
    }
    /// Commit one real accepted text version and assert publication.
    fn commit(&self, id: &str, base: Option<Oid>, text: &str) -> Oid {
        let prepared = self
            .history
            .prepare(
                &self.request(id, base, &[("pages/a.md", Some(text))]),
                &self.admission.claim,
            )
            .unwrap();
        assert!(matches!(
            self.history.publish(id, &self.admission.claim).unwrap(),
            State::Published(_)
        ));
        prepared.candidate.unwrap()
    }
    /// Reopen under another service-validated context using the same native selected roots.
    fn reopen(&self, agent: bool) -> History {
        History::open(
            &self.repo(),
            &self.root.path().join("home"),
            "pages",
            owner(agent),
            &self.admission.claim,
        )
        .unwrap()
    }
}
/// Run real observation/fault injection Git only in a fixture, never a personal repository/config.
fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
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

/// history.md: multi-file text operation has one commit; unchanged, reads, and duplicate delivery have none.
#[test]
fn actual_multi_file_idempotency_and_immutable_generations() {
    let f = Fixture::new();
    let request = f.request(
        "multi",
        None,
        &[
            ("pages/a.md", Some("first\n")),
            ("sources/source.md", Some("pointer\n")),
        ],
    );
    let p = f.history.prepare(&request, &f.admission.claim).unwrap();
    let first = p.candidate.clone().unwrap();
    assert_eq!(f.history.head(&f.admission.claim).unwrap(), None);
    assert_eq!(
        f.history
            .read_text(&first, "sources/source.md", &f.admission.claim)
            .unwrap(),
        "pointer\n"
    );
    assert!(matches!(
        f.history.publish("multi", &f.admission.claim).unwrap(),
        State::Published(_)
    ));
    assert_eq!(
        f.reopen(false)
            .prepare(&request, &f.admission.claim)
            .unwrap(),
        p
    );
    assert!(matches!(
        f.history.publish("multi", &f.admission.claim).unwrap(),
        State::Published(_)
    ));
    let mut reordered = request.clone();
    reordered.changes.reverse();
    assert_eq!(
        f.history.prepare(&reordered, &f.admission.claim).unwrap(),
        p
    );
    let mut changed = request;
    changed.changes[0].text = Some("different".into());
    assert_eq!(
        f.history.prepare(&changed, &f.admission.claim).unwrap_err(),
        Error::OperationMismatch
    );
    let unchanged = f.request(
        "same",
        Some(first.clone()),
        &[("pages/a.md", Some("first\n"))],
    );
    assert!(
        f.history
            .prepare(&unchanged, &f.admission.claim)
            .unwrap()
            .unchanged
    );
    assert!(matches!(
        f.history.publish("same", &f.admission.claim).unwrap(),
        State::Unchanged(_)
    ));
    let second = f.commit("second", Some(first.clone()), "second\n");
    assert_ne!(first, second);
    assert_eq!(
        f.history
            .read_text(&first, "pages/a.md", &f.admission.claim)
            .unwrap(),
        "first\n"
    );
    assert_eq!(
        git(&f.repo(), &["rev-list", "--count", second.as_str()]),
        "2"
    );
    assert_eq!(
        git(&f.repo(), &["rev-parse", "refs/omni/operations/multi"]),
        first.as_str()
    );
    assert!(matches!(
        f.history.inspect("multi", &f.admission.claim).unwrap(),
        State::Published(_)
    ));
    let message = git(
        &f.repo(),
        &["show", "-s", "--format=%an%n%ae%n%B", first.as_str()],
    );
    assert!(message.starts_with("Synthetic Owner\nowner@example.invalid\n"));
    assert!(message.contains("Omni-Operation-Id: multi"));
    assert!(
        fs::read_dir(f.root.path().join("home"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// history/service: agent proposals cannot advance accepted bytes; exact owner confirmation is a new commit.
#[test]
fn isolated_proposal_and_exact_owner_confirmation() {
    let f = Fixture::new();
    let first = f.commit("initial", None, "accepted\n");
    let agent = f.reopen(true);
    let mut request = f.request(
        "proposal-op",
        Some(first.clone()),
        &[("pages/a.md", Some("proposed\n"))],
    );
    assert_eq!(
        agent.prepare(&request, &f.admission.claim).unwrap_err(),
        Error::ContextMismatch
    );
    request.destination = Destination::Proposal {
        proposal_id: "proposal-one".into(),
    };
    let proposal = agent
        .prepare(&request, &f.admission.claim)
        .unwrap()
        .candidate
        .unwrap();
    assert!(matches!(
        agent.publish("proposal-op", &f.admission.claim).unwrap(),
        State::Published(_)
    ));
    assert_eq!(
        f.history.head(&f.admission.claim).unwrap(),
        Some(first.clone())
    );
    let diff = f
        .history
        .compare(Some(&first), &proposal, &f.admission.claim)
        .unwrap();
    assert_eq!(diff.affected_paths, ["pages/a.md"]);
    let confirmation = Confirmation {
        proposal_id: "proposal-one".into(),
        base: first.clone(),
        candidate: proposal.clone(),
        diff_digest: diff.diff_digest,
    };
    assert_eq!(
        agent
            .prepare_confirmation("not-human", &confirmation, &f.admission.claim)
            .unwrap_err(),
        Error::ContextMismatch
    );
    let mut wrong = confirmation.clone();
    wrong.diff_digest = "0".repeat(64);
    assert_eq!(
        f.history
            .prepare_confirmation("bad-diff", &wrong, &f.admission.claim)
            .unwrap_err(),
        Error::Conflict
    );
    let integration = f
        .history
        .prepare_confirmation("integration", &confirmation, &f.admission.claim)
        .unwrap()
        .candidate
        .unwrap();
    assert_ne!(integration, proposal);
    assert_eq!(
        f.history.head(&f.admission.claim).unwrap(),
        Some(first.clone())
    );
    assert!(matches!(
        f.history
            .publish("integration", &f.admission.claim)
            .unwrap(),
        State::Published(_)
    ));
    assert_eq!(
        git(
            &f.repo(),
            &["show", "-s", "--format=%P", integration.as_str()]
        ),
        format!("{} {}", first.as_str(), proposal.as_str())
    );
    assert_eq!(
        git(
            &f.repo(),
            &["rev-parse", "refs/heads/omni-proposals/proposal-one"]
        ),
        proposal.as_str()
    );
    let message = git(&f.repo(), &["show", "-s", "--format=%B", proposal.as_str()]);
    assert!(message.contains("Omni-Agent: validated-agent"));
    assert!(message.contains("Omni-Tool: edit_text"));
    assert!(message.contains("Omni-Session-Id: session-fixture"));
    assert_eq!(
        f.history
            .prepare_confirmation("obsolete", &confirmation, &f.admission.claim)
            .unwrap_err(),
        Error::Conflict
    );
}

/// Build a real published proposal and its complete displayed confirmation tuple.
fn proposal(f: &Fixture, id: &str, base: &Oid, text: &str) -> Confirmation {
    let agent = f.reopen(true);
    let mut request = f.request(
        &format!("{id}-operation"),
        Some(base.clone()),
        &[("pages/a.md", Some(text))],
    );
    request.destination = Destination::Proposal {
        proposal_id: id.to_owned(),
    };
    let candidate = agent
        .prepare(&request, &f.admission.claim)
        .unwrap()
        .candidate
        .unwrap();
    assert!(matches!(
        agent
            .publish(&request.operation_id, &f.admission.claim)
            .unwrap(),
        State::Published(_)
    ));
    let diff = f
        .history
        .compare(Some(base), &candidate, &f.admission.claim)
        .unwrap();
    Confirmation {
        proposal_id: id.to_owned(),
        base: base.clone(),
        candidate,
        diff_digest: diff.diff_digest,
    }
}

/// M2-S2-01: reopen/retry cannot publish an obsolete tuple; valid and completed confirmations remain usable.
#[test]
fn obsolete_confirmation_retry_preserves_candidate_and_completed_idempotency() {
    let f = Fixture::new();
    let base = f.commit("initial", None, "accepted");
    let original = proposal(&f, "reviewed", &base, "original proposal");
    let prepared = f
        .history
        .prepare_confirmation("stale-integration", &original, &f.admission.claim)
        .unwrap();
    let replacement = proposal(&f, "replacement", &base, "replacement proposal");
    git(
        &f.repo(),
        &[
            "update-ref",
            "refs/heads/omni-proposals/reviewed",
            replacement.candidate.as_str(),
            original.candidate.as_str(),
        ],
    );
    let reopened = f.reopen(false);
    assert_eq!(
        reopened.prepare_confirmation("fresh-old-tuple", &original, &f.admission.claim),
        Err(Error::Conflict)
    );
    assert_eq!(
        reopened.prepare_confirmation("stale-integration", &original, &f.admission.claim),
        Err(Error::Conflict)
    );
    assert!(
        matches!(reopened.inspect("stale-integration", &f.admission.claim).unwrap(), State::Conflict(p) if p == prepared)
    );
    assert!(
        matches!(reopened.publish("stale-integration", &f.admission.claim).unwrap(), State::Conflict(p) if p == prepared)
    );
    assert_eq!(
        reopened.head(&f.admission.claim).unwrap(),
        Some(base.clone())
    );
    assert_eq!(
        git(
            &f.repo(),
            &["for-each-ref", "refs/omni/operations/stale-integration"]
        ),
        ""
    );
    assert_eq!(
        git(
            &f.repo(),
            &[
                "cat-file",
                "-t",
                prepared.candidate.as_ref().unwrap().as_str()
            ]
        ),
        "commit"
    );
    assert_eq!(
        git(
            &f.repo(),
            &["rev-parse", "refs/omni/prepared/stale-integration"]
        ),
        prepared.candidate.unwrap().as_str()
    );
    let valid = Confirmation {
        proposal_id: "reviewed".into(),
        ..replacement
    };
    let accepted = reopened
        .prepare_confirmation("valid-integration", &valid, &f.admission.claim)
        .unwrap();
    assert!(
        matches!(reopened.publish("valid-integration", &f.admission.claim).unwrap(), State::Published(p) if p == accepted)
    );
    let advanced = proposal(
        &f,
        "later",
        accepted.candidate.as_ref().unwrap(),
        "later proposal",
    );
    git(
        &f.repo(),
        &[
            "update-ref",
            "refs/heads/omni-proposals/reviewed",
            advanced.candidate.as_str(),
            valid.candidate.as_str(),
        ],
    );
    let completed = f.reopen(false);
    assert_eq!(
        completed
            .prepare_confirmation("valid-integration", &valid, &f.admission.claim)
            .unwrap(),
        accepted
    );
    assert!(
        matches!(completed.publish("valid-integration", &f.admission.claim).unwrap(), State::Published(p) if p == accepted)
    );
    assert_eq!(
        completed.head(&f.admission.claim).unwrap(),
        accepted.candidate
    );
    assert_eq!(
        git(
            &f.repo(),
            &["rev-parse", "refs/heads/omni-proposals/reviewed"]
        ),
        advanced.candidate.as_str()
    );
}

/// Native publication must lock/verify the proposal too, not just recheck it before accepted/op CAS.
#[test]
fn confirmation_publication_transaction_verifies_proposal_ref() {
    let f = Fixture::new();
    let base = f.commit("initial", None, "accepted");
    let confirmation = proposal(&f, "reviewed", &base, "proposal");
    let prepared = f
        .history
        .prepare_confirmation("integration", &confirmation, &f.admission.claim)
        .unwrap();
    let lock = f.repo().join("refs/heads/omni-proposals/reviewed.lock");
    fs::write(&lock, "owned competing proposal ref lock").unwrap();
    assert!(matches!(
        f.history
            .publish("integration", &f.admission.claim)
            .unwrap(),
        State::Indeterminate(_)
    ));
    assert_eq!(f.history.head(&f.admission.claim).unwrap(), Some(base));
    assert_eq!(
        git(
            &f.repo(),
            &["for-each-ref", "refs/omni/operations/integration"]
        ),
        ""
    );
    fs::remove_file(lock).unwrap();
    let reopened = f.reopen(false);
    assert!(
        matches!(reopened.publish("integration", &f.admission.claim).unwrap(), State::Published(p) if p == prepared)
    );
}

/// history.md: conflicting expected refs retain both candidate and previous operation objects.
#[test]
fn competing_real_writers_and_stale_unchanged() {
    let f = Fixture::new();
    let first = f.commit("initial", None, "initial");
    let a = f.request(
        "writer-a",
        Some(first.clone()),
        &[("pages/a.md", Some("writer a"))],
    );
    let b = f.request(
        "writer-b",
        Some(first.clone()),
        &[("pages/a.md", Some("writer b"))],
    );
    let pa = f.history.prepare(&a, &f.admission.claim).unwrap();
    let pb = f.history.prepare(&b, &f.admission.claim).unwrap();
    let gate = Arc::new(Barrier::new(2));
    let repo = f.repo();
    let home = f.root.path().join("home");
    let results = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for id in ["writer-a", "writer-b"] {
            let gate = gate.clone();
            let repo = &repo;
            let home = &home;
            handles.push(s.spawn(move || {
                let admission = Admission::new();
                let h = History::open(repo, home, "pages", owner(false), &admission.claim).unwrap();
                gate.wait();
                (id, h.publish(id, &admission.claim))
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    let mut published = 0;
    for (id, result) in results {
        let state = match result {
            Err(Error::Busy) => f.history.publish(id, &f.admission.claim).unwrap(),
            other => other.unwrap(),
        };
        match state {
            State::Published(_) => published += 1,
            State::Conflict(_) => (),
            other => panic!("unexpected state {other:?}"),
        }
    }
    assert_eq!(published, 1);
    assert_ne!(pa.candidate, pb.candidate);
    for p in [pa, pb] {
        assert_eq!(
            git(
                &f.repo(),
                &["cat-file", "-t", p.candidate.unwrap().as_str()]
            ),
            "commit"
        );
    }
    assert_eq!(
        git(&f.repo(), &["rev-parse", "refs/omni/operations/initial"]),
        first.as_str()
    );
    let stale = f.request(
        "stale-unchanged",
        Some(first),
        &[("pages/a.md", Some("initial"))],
    );
    assert!(
        f.history
            .prepare(&stale, &f.admission.claim)
            .unwrap()
            .unchanged
    );
    assert!(matches!(
        f.history
            .publish("stale-unchanged", &f.admission.claim)
            .unwrap(),
        State::Conflict(_)
    ));
}

/// Native ref lock failure cannot masquerade as publication or discard a recoverable candidate.
#[test]
fn failed_ref_transaction_is_visible_and_explicitly_recoverable() {
    let f = Fixture::new();
    let p = f
        .history
        .prepare(
            &f.request("locked", None, &[("pages/a.md", Some("candidate"))]),
            &f.admission.claim,
        )
        .unwrap();
    fs::create_dir_all(f.repo().join("refs/heads")).unwrap();
    let lock = f.repo().join("refs/heads/omni-accepted.lock");
    fs::write(&lock, "owned injected competing ref lock").unwrap();
    assert!(matches!(
        f.history.publish("locked", &f.admission.claim).unwrap(),
        State::Indeterminate(_)
    ));
    assert_eq!(f.history.head(&f.admission.claim).unwrap(), None);
    assert_eq!(
        git(&f.repo(), &["for-each-ref", "refs/omni/operations/"]),
        ""
    );
    fs::remove_file(lock).unwrap();
    assert!(matches!(
        f.reopen(false)
            .inspect("locked", &f.admission.claim)
            .unwrap(),
        State::Prepared(_)
    ));
    assert!(matches!(
        f.history.publish("locked", &f.admission.claim).unwrap(),
        State::Published(_)
    ));
    assert_eq!(f.history.head(&f.admission.claim).unwrap(), p.candidate);
    assert_eq!(git(&f.repo(), &["rev-list", "--all", "--count"]), "1");
}

/// Simulate a filesystem-ref backend interruption after ONE ref is durable, never claim cross-ref crash atomicity.
#[test]
fn interrupted_native_ref_commit_remains_indeterminate() {
    let f = Fixture::new();
    let p = f
        .history
        .prepare(
            &f.request("partial", None, &[("pages/a.md", Some("candidate"))]),
            &f.admission.claim,
        )
        .unwrap();
    let candidate = p.candidate.unwrap();
    git(
        &f.repo(),
        &["update-ref", "refs/heads/omni-accepted", candidate.as_str()],
    );
    assert!(matches!(
        f.history.inspect("partial", &f.admission.claim).unwrap(),
        State::Indeterminate(_)
    ));
    assert!(matches!(
        f.history.publish("partial", &f.admission.claim).unwrap(),
        State::Indeterminate(_)
    ));
    assert_eq!(
        git(&f.repo(), &["for-each-ref", "refs/omni/operations/"]),
        ""
    );
    git(
        &f.repo(),
        &[
            "update-ref",
            "-d",
            "refs/heads/omni-accepted",
            candidate.as_str(),
        ],
    );
    git(
        &f.repo(),
        &[
            "update-ref",
            "refs/omni/operations/partial",
            candidate.as_str(),
        ],
    );
    assert!(matches!(
        f.history.inspect("partial", &f.admission.claim).unwrap(),
        State::Indeterminate(_)
    ));
    assert_eq!(
        git(&f.repo(), &["cat-file", "-t", candidate.as_str()]),
        "commit"
    );
}

/// Hostile arguments never become revisions/options, and imported config/hooks/filters remain inert/refused.
#[test]
fn hostile_inputs_config_and_native_binding_refuse() {
    for path in [
        "../escape",
        "/pages/x",
        "pages/../x",
        "pages/.git/config",
        "raw/a.md",
        "journal/w.jsonl",
        "pages/a\n.md",
        "pages/a:b",
        "pages/--output=x",
        "pages/a\\b",
    ] {
        // Each independent rejected operation receives a live native claim;
        // fixture setup for earlier operations must not consume its deadline.
        let f = Fixture::new();
        assert_eq!(
            f.history
                .prepare(
                    &f.request("hostile", None, &[(path, Some("x"))]),
                    &f.admission.claim
                )
                .unwrap_err(),
            Error::Invalid
        );
    }
    for id in ["--help", "a/b", "a..b", "a\nb", "a.lock", ""] {
        let f = Fixture::new();
        assert_eq!(
            f.history
                .prepare(
                    &f.request(id, None, &[("pages/a.md", Some("x"))]),
                    &f.admission.claim
                )
                .unwrap_err(),
            Error::Invalid
        );
    }
    let f = Fixture::new();
    assert_eq!(Oid::parse("HEAD^{tree}"), Err(Error::Invalid));
    assert_eq!(Oid::parse(&"A".repeat(40)), Err(Error::Invalid));
    let mut bad_owner = owner(false);
    bad_owner.name = "Someone\nOmni-Agent: false".into();
    assert!(matches!(
        History::open(
            &f.repo(),
            &f.root.path().join("home"),
            "pages",
            bad_owner,
            &f.admission.claim
        ),
        Err(Error::Invalid)
    ));
    let marker = f.root.path().join("executed");
    fs::create_dir_all(f.repo().join("hooks")).unwrap();
    fs::write(
        f.repo().join("hooks/reference-transaction"),
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        f.repo().join("hooks/reference-transaction"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    f.commit("no-hooks", None, "safe");
    assert!(!marker.exists());
    let config = fs::read(f.repo().join("config")).unwrap();
    fs::write(
        f.repo().join("config"),
        b"[include]\npath = /not-admitted\n",
    )
    .unwrap();
    assert_eq!(f.history.head(&f.admission.claim), Err(Error::Invalid));
    fs::write(f.repo().join("config"), config).unwrap();
    fs::rename(
        f.root.path().join("home"),
        f.root.path().join("original-home"),
    )
    .unwrap();
    fs::create_dir(f.root.path().join("home")).unwrap();
    assert_eq!(
        f.history.head(&f.admission.claim),
        Err(Error::ContextMismatch)
    );
}

/// A selected signed library retains its text tree/intent and never silently produces an unsigned commit.
#[test]
fn signing_required_is_not_unsigned_fallback() {
    let root = canonical_tempdir();
    fs::create_dir(root.path().join("home")).unwrap();
    let admission = Admission::new();
    let mut identity = owner(false);
    identity.mode = Mode::SigningRequired;
    let h = History::init(
        &root.path().join("repo"),
        &root.path().join("home"),
        "pages",
        identity,
        &admission.claim,
    )
    .unwrap();
    let r = Request {
        operation_id: "needs-signing".into(),
        base: None,
        destination: Destination::Accepted,
        changes: vec![Change {
            path: "pages/a.md".into(),
            text: Some("preserve me".into()),
        }],
    };
    assert_eq!(h.prepare(&r, &admission.claim), Err(Error::SigningRequired));
    assert_eq!(h.head(&admission.claim).unwrap(), None);
    assert_eq!(
        git(
            &root.path().join("repo"),
            &["show", "refs/omni/trees/needs-signing:pages/a.md"]
        ),
        "preserve me"
    );
    assert!(
        root.path()
            .join("repo/omni-state/operations/needs-signing.json")
            .exists()
    );
    assert_eq!(
        git(
            &root.path().join("repo"),
            &["for-each-ref", "refs/omni/operations"]
        ),
        ""
    );
}

/// A real process can die after prepare; reopening and duplicate delivery publish the same object once.
#[test]
fn killed_process_recovery_preserves_exact_operation() {
    if let Some(root) = std::env::var_os("OMNI_HISTORY_OWNED_RECOVERY_FIXTURE") {
        let root = Path::new(&root);
        fs::create_dir(root.join("home")).unwrap();
        let admission = Admission::new();
        let h = History::init(
            &root.join("repo"),
            &root.join("home"),
            "pages",
            owner(false),
            &admission.claim,
        )
        .unwrap();
        let r = Request {
            operation_id: "interrupted".into(),
            base: None,
            destination: Destination::Accepted,
            changes: vec![Change {
                path: "pages/a.md".into(),
                text: Some("retained".into()),
            }],
        };
        h.prepare(&r, &admission.claim).unwrap();
        fs::write(root.join("prepared-marker"), "ready").unwrap();
        loop {
            std::thread::park();
        }
    }
    let root = canonical_tempdir();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "killed_process_recovery_preserves_exact_operation",
            "--nocapture",
        ])
        .env("OMNI_HISTORY_OWNED_RECOVERY_FIXTURE", root.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let end = Instant::now() + Duration::from_secs(15);
    while !root.path().join("prepared-marker").exists() && Instant::now() < end {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("worker exited {status}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let ready = root.path().join("prepared-marker").exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        ready,
        "worker must reach real prepared state before interruption"
    );
    let admission = Admission::new();
    let h = History::open(
        &root.path().join("repo"),
        &root.path().join("home"),
        "pages",
        owner(false),
        &admission.claim,
    )
    .unwrap();
    let candidate = match h.inspect("interrupted", &admission.claim).unwrap() {
        State::Prepared(p) => p.candidate.unwrap(),
        other => panic!("unexpected {other:?}"),
    };
    assert!(matches!(
        h.publish("interrupted", &admission.claim).unwrap(),
        State::Published(_)
    ));
    assert!(matches!(
        h.publish("interrupted", &admission.claim).unwrap(),
        State::Published(_)
    ));
    assert_eq!(h.head(&admission.claim).unwrap(), Some(candidate));
    assert_eq!(
        git(&root.path().join("repo"), &["rev-list", "--all", "--count"]),
        "1"
    );
}

/// Unchanged receipts survive later accepted versions without creating empty commits; deletes are text changes.
#[test]
fn initial_unchanged_delete_and_private_index() {
    let f = Fixture::new();
    let empty = f.request("empty", None, &[]);
    let p = f.history.prepare(&empty, &f.admission.claim).unwrap();
    assert!(p.unchanged);
    assert!(p.candidate.is_none());
    assert!(matches!(
        f.history.publish("empty", &f.admission.claim).unwrap(),
        State::Unchanged(_)
    ));
    assert_eq!(f.history.head(&f.admission.claim).unwrap(), None);
    // An unrelated staged index in the selected owned repository is not the operation index.
    let unrelated_index = f.repo().join("index");
    let unrelated_file = f.root.path().join("unrelated.txt");
    fs::write(&unrelated_file, b"unrelated user staged text").unwrap();
    let blob = git(
        &f.repo(),
        &[
            "hash-object",
            "-w",
            "--no-filters",
            unrelated_file.to_str().unwrap(),
        ],
    );
    git(
        &f.repo(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{blob},unrelated.md"),
        ],
    );
    let staged_before = fs::read(&unrelated_index).unwrap();
    let first = f.commit("first", None, "content");
    assert!(matches!(
        f.history.publish("empty", &f.admission.claim).unwrap(),
        State::Unchanged(_)
    ));
    let delete = f.request("delete", Some(first.clone()), &[("pages/a.md", None)]);
    let deleted = f.history.prepare(&delete, &f.admission.claim).unwrap();
    assert_eq!(deleted.affected_paths, ["pages/a.md"]);
    assert!(matches!(
        f.history.publish("delete", &f.admission.claim).unwrap(),
        State::Published(_)
    ));
    assert_eq!(
        f.history
            .read_text(&first, "pages/a.md", &f.admission.claim)
            .unwrap(),
        "content"
    );
    assert_eq!(
        git(
            &f.repo(),
            &["rev-list", "--count", deleted.candidate.unwrap().as_str()]
        ),
        "2"
    );
    assert_eq!(fs::read(unrelated_index).unwrap(), staged_before);
}

/// A changed prepared ref cannot supply bytes for an unrelated recipe, even before publication.
#[test]
fn corrupt_candidate_binding_is_not_installable() {
    let f = Fixture::new();
    let a = f
        .history
        .prepare(
            &f.request("candidate-a", None, &[("pages/a.md", Some("a"))]),
            &f.admission.claim,
        )
        .unwrap();
    let b = f
        .history
        .prepare(
            &f.request("candidate-b", None, &[("pages/a.md", Some("b"))]),
            &f.admission.claim,
        )
        .unwrap();
    git(
        &f.repo(),
        &[
            "update-ref",
            "refs/omni/prepared/candidate-a",
            b.candidate.unwrap().as_str(),
            a.candidate.unwrap().as_str(),
        ],
    );
    assert_eq!(
        f.history.inspect("candidate-a", &f.admission.claim),
        Err(Error::Recovery)
    );
    assert_eq!(
        f.history.publish("candidate-a", &f.admission.claim),
        Err(Error::Recovery)
    );
    assert_eq!(f.history.head(&f.admission.claim).unwrap(), None);
}

/// Current admitted bounds are checked before cloning input or executing further Git work.
#[test]
fn finite_claim_capacity_and_deadline() {
    let f = Fixture::new();
    let limited = Admission::with_budget(Budget {
        max_files: 1,
        max_bytes: 64,
        max_millis: 60_000,
        max_memory_bytes: 256,
    });
    let request = f.request("too-large", None, &[("pages/a.md", Some(&"x".repeat(65)))]);
    assert_eq!(
        f.history.prepare(&request, &limited.claim),
        Err(Error::Capacity)
    );
    assert!(
        !f.repo()
            .join("omni-state/operations/too-large.json")
            .exists()
    );
    let expired = Admission::with_budget(Budget {
        max_files: 100,
        max_bytes: 524_288,
        max_millis: 1,
        max_memory_bytes: 64 << 20,
    });
    std::thread::sleep(Duration::from_millis(3));
    assert_eq!(f.history.head(&expired.claim), Err(Error::Deadline));
    assert_eq!(f.history.head(&f.admission.claim).unwrap(), None);
}
