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

/// Actual host proposal pages, complete inspected tuples, and immutable history expose no unrelated branches or hash authority.
#[test]
fn qualified_proposal_pages_diff_and_history_are_actual_git_reads() {
    let f = Fixture::new();
    let first = f.commit("read-initial", None, "first");
    let second = f.commit("read-second", Some(first.clone()), "second");
    let agent = f.reopen(true);
    for id in ["a", "b", "c"] {
        let mut request = f.request(
            &format!("read-{id}"),
            Some(second.clone()),
            &[("pages/a.md", Some(id))],
        );
        request.destination = Destination::Proposal {
            proposal_id: id.into(),
        };
        agent.prepare(&request, &f.admission.claim).unwrap();
        assert!(matches!(
            agent
                .publish(&request.operation_id, &f.admission.claim)
                .unwrap(),
            State::Published(_)
        ));
    }
    git(
        &f.repo(),
        &[
            "update-ref",
            "refs/heads/unrelated-private-branch",
            first.as_str(),
        ],
    );
    let tree = git(
        &f.repo(),
        &["rev-parse", &format!("{}^{{tree}}", second.as_str())],
    );
    let orphan = Oid::parse(&git(
        &f.repo(),
        &[
            "-c",
            "user.name=Synthetic Detached Owner",
            "-c",
            "user.email=detached@example.invalid",
            "commit-tree",
            &tree,
            "-m",
            "unrelated detached root",
        ],
    ))
    .unwrap();
    git(
        &f.repo(),
        &[
            "update-ref",
            "refs/heads/unrelated-private-branch",
            orphan.as_str(),
            first.as_str(),
        ],
    );
    assert_eq!(
        f.history.version(&orphan, &f.admission.claim).unwrap_err(),
        Error::ContextMismatch
    );
    assert_eq!(
        f.history
            .version_text(&orphan, "pages/a.md", &f.admission.claim)
            .unwrap_err(),
        Error::ContextMismatch
    );
    let before = git(&f.repo(), &["rev-list", "--all", "--count"]);
    let page = f.history.proposals(None, 1, &f.admission.claim).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].proposal_id, "a");
    assert_eq!(page.entries[0].original_base, Some(second.clone()));
    assert_eq!(
        page.entries[0].commit.attribution.agent.as_deref(),
        Some("validated-agent")
    );
    assert_eq!(page.next.as_deref(), Some("a"));
    let next = f
        .history
        .proposals(page.next.as_deref(), 2, &f.admission.claim)
        .unwrap();
    assert_eq!(
        next.entries
            .iter()
            .map(|p| p.proposal_id.as_str())
            .collect::<Vec<_>>(),
        ["b", "c"]
    );
    assert!(next.next.is_none());
    let diff = f.history.proposal_diff("a", &f.admission.claim).unwrap();
    assert_eq!(diff.expected_base, Some(second.clone()));
    assert_eq!(diff.proposal.candidate, page.entries[0].candidate);
    assert_eq!(diff.comparison.affected_paths, ["pages/a.md"]);
    assert_eq!(
        diff.comparison,
        f.history
            .compare(Some(&second), &diff.proposal.candidate, &f.admission.claim)
            .unwrap()
    );
    assert_eq!(
        f.history
            .version_text(&diff.proposal.candidate, "pages/a.md", &f.admission.claim)
            .unwrap(),
        "a"
    );
    let history = f.history.commits(None, 1, &f.admission.claim).unwrap();
    assert_eq!(history.entries[0].version, second);
    assert_eq!(
        history.entries[0].attribution.operation_id.as_deref(),
        Some("read-second")
    );
    assert_eq!(history.next, Some(first.clone()));
    let older = f
        .history
        .commits(history.next.as_ref(), 2, &f.admission.claim)
        .unwrap();
    assert_eq!(older.entries[0].version, first);
    assert!(older.next.is_none());
    assert_eq!(
        git(&f.repo(), &["rev-list", "--all", "--count"]),
        before,
        "reads create no commits"
    );
    assert_eq!(
        f.history
            .proposals(Some("../unrelated"), 1, &f.admission.claim)
            .unwrap_err(),
        Error::Invalid
    );
    assert_eq!(
        f.history.commits(None, 0, &f.admission.claim).unwrap_err(),
        Error::Capacity
    );
}

/// Required-signing refusal keeps actual candidate bytes readable without creating an unsigned operation commit.
#[test]
fn signing_blocked_candidate_has_an_actual_bounded_read_side() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("home")).unwrap();
    let admission = Admission::new();
    let mut context = owner(false);
    context.mode = Mode::SigningRequired;
    let history = History::init(
        &root.path().join("repo"),
        &root.path().join("home"),
        "pages",
        context,
        &admission.claim,
    )
    .unwrap();
    let request = Request {
        operation_id: "locked-read".into(),
        base: None,
        destination: Destination::Accepted,
        changes: vec![Change {
            path: "pages/a.md".into(),
            text: Some("preserved signed-mode text".into()),
        }],
    };
    assert_eq!(
        history.prepare(&request, &admission.claim).unwrap_err(),
        Error::SigningRequired
    );
    assert_eq!(
        history
            .read_candidate("locked-read", "pages/a.md", &admission.claim)
            .unwrap(),
        "preserved signed-mode text"
    );
    assert!(
        history
            .proposals(None, 10, &admission.claim)
            .unwrap()
            .entries
            .is_empty()
    );
    assert!(
        history
            .commits(None, 10, &admission.claim)
            .unwrap()
            .entries
            .is_empty()
    );
    assert_eq!(history.head(&admission.claim).unwrap(), None);
}

/// The only credential fixture is a disposable explicit native agent/socket and a 0700 system-key directory outside cache.
struct OwnedAgent {
    root: Option<tempfile::TempDir>,
    child: std::process::Child,
    start: String,
}
impl OwnedAgent {
    /// Native Linux start observation for the exact child fixture; no host agent/configuration lookup.
    fn start_of(pid: u32) -> String {
        #[cfg(target_os = "linux")]
        {
            fs::read_to_string(format!("/proc/{pid}/stat"))
                .unwrap()
                .rsplit_once(')')
                .unwrap()
                .1
                .split_whitespace()
                .nth(19)
                .unwrap()
                .to_owned()
        }
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("/bin/ps")
                .args(["-p", &pid.to_string(), "-o", "lstart="])
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        }
    }
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("omni-owned-native-ssh-")
            .tempdir_in(std::path::Path::new("/tmp").canonicalize().unwrap())
            .unwrap();
        let socket = root.path().join("agent.sock");
        let child = Command::new("/usr/bin/ssh-agent")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .args(["-D", "-a"])
            .arg(&socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Self::start_of(child.id());
        let end = Instant::now() + Duration::from_secs(2);
        while !socket.exists() {
            assert!(Instant::now() < end, "owned native agent startup deadline");
            std::thread::sleep(Duration::from_millis(2));
        }
        Self {
            root: Some(root),
            child,
            start,
        }
    }
    fn socket(&self) -> std::path::PathBuf {
        self.root.as_ref().unwrap().path().join("agent.sock")
    }
    fn key(&self, name: &str) -> std::path::PathBuf {
        self.root.as_ref().unwrap().path().join(name)
    }
    /// Lock the actual owned agent using an ephemeral random askpass fixture, not a signing/verification replacement.
    fn lock(&mut self) {
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;
        let mut bytes = [0u8; 24];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        let pass: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let askpass = self.key("owned-askpass");
        fs::write(&askpass, format!("#!/bin/sh\nprintf '%s\\n' '{pass}'\n")).unwrap();
        fs::set_permissions(&askpass, fs::Permissions::from_mode(0o700)).unwrap();
        let mut helper = Command::new("/usr/bin/ssh-add")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("SSH_AUTH_SOCK", self.socket())
            .env("SSH_ASKPASS", &askpass)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", "owned-fixture:0")
            .arg("-x")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = helper.try_wait().unwrap() {
                assert!(status.success(), "actual owned agent lock refused");
                break;
            }
            if Instant::now() >= end {
                let _ = helper.kill();
                let cleanup = Instant::now() + Duration::from_secs(2);
                while helper.try_wait().unwrap().is_none() {
                    if Instant::now() >= cleanup {
                        if let Some(root) = self.root.take() {
                            let _ = root.keep();
                        }
                        std::mem::forget(helper);
                        panic!("owned lock helper collection unknown");
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                panic!("owned lock helper exceeded deadline after actual collection");
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        fs::remove_file(askpass).unwrap();
    }
    /// Finite exact-child stop/reap; if collection were unknown its credential directory would be retained rather than deleted.
    fn stop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            assert_eq!(Self::start_of(self.child.id()), self.start);
            self.child.kill().unwrap();
            let end = Instant::now() + Duration::from_secs(2);
            while self.child.try_wait().unwrap().is_none() {
                if Instant::now() >= end {
                    if let Some(root) = self.root.take() {
                        let _ = root.keep();
                    }
                    panic!("owned agent collection unknown");
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
impl Drop for OwnedAgent {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Real SSH signatures, native trust, actual provider refusal and distinct author/committer survive reopen without unsigned fallback.
#[test]
fn actual_native_ssh_signing_verification_locked_missing_and_reopen() {
    use knowledge_history::{GitIdentity, KeyProtection, KnownSigner, SignatureState};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("home")).unwrap();
    let admission = Admission::new();
    let mut context = owner(false);
    context.mode = Mode::SigningRequired;
    let history = History::init(
        &root.path().join("history"),
        &root.path().join("home"),
        "pages",
        context.clone(),
        &admission.claim,
    )
    .unwrap();
    let mut native = OwnedAgent::new();
    let first_key = native.key("first-system-key");
    let signer = history
        .setup_system_signer(
            &first_key,
            &native.socket(),
            KeyProtection::UnencryptedExplicit,
            &admission.claim,
        )
        .unwrap();
    for path in [first_key.join("identity"), first_key.join("identity.pub")] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(
        fs::metadata(&first_key).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let known = KnownSigner::selected("native-owner@example.invalid", signer.public_key()).unwrap();
    let other = history
        .setup_system_signer(
            &native.key("other-system-key"),
            &native.socket(),
            KeyProtection::UnencryptedExplicit,
            &admission.claim,
        )
        .unwrap();
    let unrelated = KnownSigner::selected("unrelated@example.invalid", other.public_key()).unwrap();
    let history = history
        .with_signer(signer.clone())
        .with_committer(
            GitIdentity::new("Native Device Committer", "device@example.invalid").unwrap(),
        )
        .unwrap();
    let request = Request {
        operation_id: "actual-native-signed".into(),
        base: None,
        destination: Destination::Accepted,
        changes: vec![Change {
            path: "pages/a.md".into(),
            text: Some("Actual signed native bytes\n".into()),
        }],
    };
    let prepared = history.prepare(&request, &admission.claim).unwrap();
    assert!(matches!(
        history
            .publish(&request.operation_id, &admission.claim)
            .unwrap(),
        State::Published(_)
    ));
    let oid = prepared.candidate.clone().unwrap();
    let info = history.version(&oid, &admission.claim).unwrap();
    assert_eq!(info.author.name, "Synthetic Owner");
    assert_eq!(info.committer.name, "Native Device Committer");
    assert_ne!(info.author.email, info.committer.email);
    assert_eq!(
        history
            .verify_signature(&oid, &[], &admission.claim)
            .unwrap()
            .state,
        SignatureState::ValidUnknown
    );
    assert_eq!(
        history
            .verify_signature(&oid, &[unrelated], &admission.claim)
            .unwrap()
            .state,
        SignatureState::ValidUnknown
    );
    assert_eq!(
        history
            .verify_signature(&oid, std::slice::from_ref(&known), &admission.claim)
            .unwrap()
            .state,
        SignatureState::ValidKnown
    );
    // Independent real Git verifier proves the standard commit gpgsig encoding, not only our parser's interpretation.
    let allowed = native.key("allowed-signers");
    fs::write(
        &allowed,
        format!(
            "native-owner@example.invalid namespaces=\"git\" {}",
            signer.public_key()
        ),
    )
    .unwrap();
    git(
        &root.path().join("history"),
        &[
            "-c",
            "gpg.format=ssh",
            "-c",
            &format!("gpg.ssh.allowedSignersFile={}", allowed.display()),
            "verify-commit",
            oid.as_str(),
        ],
    );
    let actual = git(
        &root.path().join("history"),
        &["cat-file", "commit", oid.as_str()],
    );
    let corrupt = actual.replace("Omni text operation", "Tampered text operation") + "\n";
    let mut child = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .arg("--git-dir")
        .arg(root.path().join("history"))
        .args(["hash-object", "-t", "commit", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(corrupt.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let invalid = Oid::parse(std::str::from_utf8(&output.stdout).unwrap().trim()).unwrap();
    git(
        &root.path().join("history"),
        &[
            "update-ref",
            "refs/heads/omni-proposals/invalid",
            invalid.as_str(),
            &"0".repeat(40),
        ],
    );
    assert_eq!(
        history
            .verify_signature(&invalid, std::slice::from_ref(&known), &admission.claim)
            .unwrap()
            .state,
        SignatureState::Invalid
    );
    // Actual native locked provider refuses a new operation while preserving its actual candidate tree and old accepted head.
    native.lock();
    let mut blocked = request.clone();
    blocked.operation_id = "locked-native-provider".into();
    blocked.base = Some(oid.clone());
    blocked.changes[0].text = Some("Retained locked-provider bytes".into());
    assert_eq!(
        history.prepare(&blocked, &admission.claim).unwrap_err(),
        Error::SigningRequired
    );
    assert_eq!(
        history
            .read_candidate(&blocked.operation_id, "pages/a.md", &admission.claim)
            .unwrap(),
        "Retained locked-provider bytes"
    );
    assert_eq!(history.head(&admission.claim).unwrap(), Some(oid.clone()));
    assert_eq!(
        history
            .version_text(&oid, "pages/a.md", &admission.claim)
            .unwrap(),
        "Actual signed native bytes\n"
    );
    // Agent absence does not prevent verification, inspection or completed-operation idempotency.
    native.stop();
    let reopened = History::open(
        &root.path().join("history"),
        &root.path().join("home"),
        "pages",
        context,
        &admission.claim,
    )
    .unwrap();
    assert_eq!(
        reopened.prepare(&request, &admission.claim).unwrap(),
        prepared
    );
    assert!(
        matches!(reopened.publish(&request.operation_id, &admission.claim).unwrap(), State::Published(p) if p == prepared)
    );
    assert_eq!(
        reopened
            .verify_signature(&oid, &[known], &admission.claim)
            .unwrap()
            .state,
        SignatureState::ValidKnown
    );
    blocked.operation_id = "missing-native-provider".into();
    assert_eq!(
        reopened
            .with_signer(signer)
            .prepare(&blocked, &admission.claim)
            .unwrap_err(),
        Error::SigningRequired
    );
    let unsigned = Fixture::new();
    let unsigned_oid =
        unsigned.commit("explicit-unsigned", None, "Explicit native unsigned choice");
    assert_eq!(
        unsigned
            .history
            .verify_signature(&unsigned_oid, &[], &unsigned.admission.claim)
            .unwrap()
            .state,
        SignatureState::Unsigned
    );
}

/// Actual native pack validation/object admission uses the held exact stream, not a replacement path or imported ref grant.
#[test]
fn held_exact_bundle_verifies_and_admits_inert_native_objects() {
    use knowledge_core::data::ContentDigest;
    use sha2::{Digest, Sha256};
    use std::io::Seek;
    let source = Fixture::new();
    let version = source.commit("bundle-source", None, "Actual bundled text\n");
    let path = source.root.path().join("selected.bundle");
    git(
        &source.repo(),
        &[
            "bundle",
            "create",
            path.to_str().unwrap(),
            "refs/heads/omni-accepted",
        ],
    );
    let bytes = fs::read(&path).unwrap();
    let digest = ContentDigest::new(format!("{:x}", Sha256::digest(&bytes))).unwrap();
    let mut file = fs::File::open(&path).unwrap();
    file.seek(std::io::SeekFrom::End(0)).unwrap();
    // A replaced visible path cannot redirect the actual held source supplied by M5's private verified-stream handoff.
    fs::remove_file(&path).unwrap();
    fs::write(&path, "Not the held bundle").unwrap();
    let destination = Fixture::new();
    let before = git(&destination.repo(), &["count-objects", "-v"]);
    let verified = destination
        .history
        .verify_bundle(&file, &digest, &destination.admission.claim)
        .unwrap();
    assert!(!verified.objects_admitted);
    assert_eq!(verified.version, digest);
    assert_eq!(verified.bytes, bytes.len() as u64);
    assert_eq!(verified.refs.len(), 1);
    assert_eq!(verified.refs[0].name, "refs/heads/omni-accepted");
    assert_eq!(verified.refs[0].version, version);
    assert_eq!(git(&destination.repo(), &["count-objects", "-v"]), before);
    assert_eq!(file.stream_position().unwrap(), bytes.len() as u64);
    let admitted = destination
        .history
        .admit_bundle(&file, &digest, &destination.admission.claim)
        .unwrap();
    assert!(admitted.objects_admitted);
    assert_eq!(admitted.refs, verified.refs);
    assert_eq!(
        git(&destination.repo(), &["cat-file", "-t", version.as_str()]),
        "commit"
    );
    assert_eq!(
        destination
            .history
            .head(&destination.admission.claim)
            .unwrap(),
        None
    );
    assert!(
        destination
            .history
            .proposals(None, 10, &destination.admission.claim)
            .unwrap()
            .entries
            .is_empty()
    );
    assert_eq!(
        destination
            .history
            .version(&version, &destination.admission.claim)
            .unwrap_err(),
        Error::ContextMismatch
    );
    assert_eq!(file.stream_position().unwrap(), bytes.len() as u64);
}

/// Byte-version mismatch, actual bad pack checksum, missing prerequisites and foreign refs refuse before main object/ref admission.
#[test]
fn bundle_version_pack_prerequisite_and_namespace_refusals_are_real() {
    use knowledge_core::data::ContentDigest;
    use knowledge_history::BundleError;
    use sha2::{Digest, Sha256};
    let source = Fixture::new();
    let base = source.commit("bundle-base", None, "Base bundle bytes");
    let path = source.root.path().join("complete.bundle");
    git(
        &source.repo(),
        &[
            "bundle",
            "create",
            path.to_str().unwrap(),
            "refs/heads/omni-accepted",
        ],
    );
    let digest = |bytes: &[u8]| ContentDigest::new(format!("{:x}", Sha256::digest(bytes))).unwrap();
    let bytes = fs::read(&path).unwrap();
    let destination = Fixture::new();
    let before = git(&destination.repo(), &["count-objects", "-v"]);
    assert_eq!(
        destination
            .history
            .admit_bundle(
                &fs::File::open(&path).unwrap(),
                &digest(b"not the exact stream"),
                &destination.admission.claim
            )
            .unwrap_err(),
        BundleError::Changed
    );
    let mut damaged = bytes.clone();
    let end = damaged.len() - 1;
    damaged[end] ^= 1;
    let damaged_path = source.root.path().join("bad-pack.bundle");
    fs::write(&damaged_path, &damaged).unwrap();
    assert_eq!(
        destination
            .history
            .admit_bundle(
                &fs::File::open(&damaged_path).unwrap(),
                &digest(&damaged),
                &destination.admission.claim
            )
            .unwrap_err(),
        BundleError::Integrity
    );
    source.commit("bundle-next", Some(base.clone()), "Next bundle bytes");
    let incremental = source.root.path().join("incremental.bundle");
    git(
        &source.repo(),
        &[
            "bundle",
            "create",
            incremental.to_str().unwrap(),
            "refs/heads/omni-accepted",
            &format!("^{}", base.as_str()),
        ],
    );
    assert_eq!(
        destination
            .history
            .admit_bundle(
                &fs::File::open(&incremental).unwrap(),
                &digest(&fs::read(&incremental).unwrap()),
                &destination.admission.claim
            )
            .unwrap_err(),
        BundleError::Integrity
    );
    git(
        &source.repo(),
        &[
            "update-ref",
            "refs/heads/unrelated-private",
            base.as_str(),
            &"0".repeat(40),
        ],
    );
    let foreign = source.root.path().join("foreign.bundle");
    git(
        &source.repo(),
        &[
            "bundle",
            "create",
            foreign.to_str().unwrap(),
            "refs/heads/unrelated-private",
        ],
    );
    assert_eq!(
        destination
            .history
            .admit_bundle(
                &fs::File::open(&foreign).unwrap(),
                &digest(&fs::read(&foreign).unwrap()),
                &destination.admission.claim
            )
            .unwrap_err(),
        BundleError::Integrity
    );
    assert_eq!(git(&destination.repo(), &["count-objects", "-v"]), before);
    assert_eq!(
        destination
            .history
            .head(&destination.admission.claim)
            .unwrap(),
        None
    );
}

/// Real stable owned external bytes are observation evidence, never fabricated editor/agent authorship.
#[test]
fn external_observation_keeps_native_evidence_and_not_agent_authorship() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new();
    let base = f.commit("before-observation", None, "Before external observation");
    fs::create_dir(f.root.path().join("home/pages")).unwrap();
    let path = f.root.path().join("home/pages/a.md");
    fs::write(&path, "Actual external filesystem text\n").unwrap();
    let before = fs::metadata(&path).unwrap();
    let bytes = fs::read_to_string(&path).unwrap();
    let after = fs::metadata(&path).unwrap();
    assert_eq!(
        (before.ino(), before.len(), before.mtime_nsec()),
        (after.ino(), after.len(), after.mtime_nsec())
    );
    let id = format!(
        "native-observation-{}-{}",
        before.ino(),
        before.ctime_nsec()
    );
    let request = f.request(
        "observed-external",
        Some(base),
        &[("pages/a.md", Some(&bytes))],
    );
    let prepared = f
        .history
        .prepare_observation(&request, std::slice::from_ref(&id), &f.admission.claim)
        .unwrap();
    assert!(matches!(
        f.history
            .publish(&request.operation_id, &f.admission.claim)
            .unwrap(),
        State::Published(_)
    ));
    let info = f
        .history
        .version(prepared.candidate.as_ref().unwrap(), &f.admission.claim)
        .unwrap();
    assert_eq!(info.attribution.origin.as_deref(), Some("external_unknown"));
    assert_eq!(
        info.attribution.observation_ids.as_slice(),
        std::slice::from_ref(&id)
    );
    assert!(info.attribution.agent.is_none());
    assert_eq!(info.committer.name, "Synthetic Owner");
    assert_eq!(
        f.history
            .prepare_observation(&request, &[id], &f.admission.claim)
            .unwrap(),
        prepared
    );
    assert_eq!(
        f.history
            .prepare_observation(
                &request,
                &["different-native-observation".into()],
                &f.admission.claim
            )
            .unwrap_err(),
        Error::OperationMismatch
    );
    assert_eq!(
        f.reopen(true)
            .prepare_observation(
                &request,
                &["native-agent-cannot-claim-editor".into()],
                &f.admission.claim
            )
            .unwrap_err(),
        Error::ContextMismatch
    );
}

/// Use only this selected disposable working tree, never the harness cwd or personal configuration.
fn user_worktree_git(repo: &Path, args: &[&str]) -> String {
    let worktree = format!("--work-tree={}", repo.parent().unwrap().display());
    let mut selected = vec![worktree.as_str()];
    selected.extend_from_slice(args);
    git(repo, &selected)
}

/// Real owned ordinary user repository, not imported host Git configuration.
fn legacy_user_repo() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let output = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(["init", "--initial-branch=main", "--template="])
        .arg(&home)
        .output()
        .unwrap();
    assert!(output.status.success());
    fs::create_dir(home.join("pages")).unwrap();
    fs::create_dir(home.join("raw")).unwrap();
    fs::write(home.join("pages/a.md"), "Legacy authored text\n").unwrap();
    fs::write(home.join("raw/photo.bin"), b"actual legacy media\0bytes").unwrap();
    fs::write(
        home.join(".gitattributes"),
        "* filter=fixture-evil diff=fixture-evil\n",
    )
    .unwrap();
    let repo = home.join(".git");
    user_worktree_git(
        &repo,
        &["add", "--", "pages/a.md", "raw/photo.bin", ".gitattributes"],
    );
    user_worktree_git(
        &repo,
        &[
            "-c",
            "user.name=Legacy Human Author",
            "-c",
            "user.email=legacy@example.invalid",
            "commit",
            "-m",
            "Original actual user history",
        ],
    );
    (root, home, repo)
}

/// Use-existing retains actual user history/media/branches/staging while making one selected text-only app commit through a private index.
#[test]
fn existing_user_git_preserves_staging_media_history_and_inert_configuration() {
    use knowledge_history::{ExistingRepository, GitPlacement};
    use std::os::unix::fs::PermissionsExt;
    let (root, home, repo) = legacy_user_repo();
    let base = Oid::parse(&git(&repo, &["rev-parse", "refs/heads/main"])).unwrap();
    git(
        &repo,
        &[
            "update-ref",
            "refs/heads/unrelated-user-branch",
            base.as_str(),
            &"0".repeat(40),
        ],
    );
    fs::write(
        home.join("raw/photo.bin"),
        b"Unrelated actually staged media\0bytes",
    )
    .unwrap();
    user_worktree_git(&repo, &["add", "--", "raw/photo.bin"]);
    let index = fs::read(repo.join("index")).unwrap();
    let staged = git(&repo, &["rev-parse", ":raw/photo.bin"]);
    let media = git(
        &repo,
        &["rev-parse", &format!("{}:raw/photo.bin", base.as_str())],
    );
    let hooks = root.path().join("hostile-hooks");
    fs::create_dir(&hooks).unwrap();
    let marker = root.path().join("must-not-execute");
    let hostile = hooks.join("post-commit");
    fs::write(
        &hostile,
        format!(
            "#!/bin/sh\nprintf forbidden > '{}'\nexit 17\n",
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hostile, fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = fs::read_to_string(repo.join("config")).unwrap();
    config.push_str(&format!("\n[core]\n hooksPath = {}\n fsmonitor = {}\n[filter \"fixture-evil\"]\n process = {}\n required = true\n[diff \"fixture-evil\"]\n textconv = {}\n", hooks.display(), hostile.display(), hostile.display(), hostile.display()));
    fs::write(repo.join("config"), &config).unwrap();
    let admission = Admission::new();
    let selected = ExistingRepository {
        git_dir: &repo,
        baseline: "refs/heads/main",
        placement: GitPlacement::LocalUnsynced,
    };
    let history =
        History::use_existing(selected, &home, "pages", owner(false), &admission.claim).unwrap();
    assert_eq!(history.head(&admission.claim).unwrap(), Some(base.clone()));
    let request = Request {
        operation_id: "actual-existing-user-git".into(),
        base: Some(base.clone()),
        destination: Destination::Accepted,
        changes: vec![Change {
            path: "pages/a.md".into(),
            text: Some("New app authored text\n".into()),
        }],
    };
    let prepared = history.prepare(&request, &admission.claim).unwrap();
    assert_eq!(prepared.affected_paths, ["pages/a.md"]);
    assert!(matches!(
        history
            .publish(&request.operation_id, &admission.claim)
            .unwrap(),
        State::Published(_)
    ));
    let candidate = prepared.candidate.as_ref().unwrap();
    assert_eq!(
        git(
            &repo,
            &[
                "rev-parse",
                &format!("{}:raw/photo.bin", candidate.as_str())
            ]
        ),
        media
    );
    assert_eq!(git(&repo, &["rev-parse", "refs/heads/main"]), base.as_str());
    assert_eq!(
        git(&repo, &["rev-parse", "refs/heads/unrelated-user-branch"]),
        base.as_str()
    );
    assert_eq!(
        git(
            &repo,
            &["-c", "core.fsmonitor=false", "rev-parse", ":raw/photo.bin"]
        ),
        staged
    );
    assert_eq!(fs::read(repo.join("index")).unwrap(), index);
    assert_eq!(fs::read_to_string(repo.join("config")).unwrap(), config);
    assert!(!marker.exists());
    assert_eq!(
        fs::read_to_string(home.join("pages/a.md")).unwrap(),
        "Legacy authored text\n",
        "Git Published is not filesystem saved_local"
    );
    let entries = history.commits(None, 10, &admission.claim).unwrap().entries;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].author.name, "Legacy Human Author");
    let mut unselected = request.clone();
    unselected.operation_id = "never-purge-user-media".into();
    unselected.base = Some(candidate.clone());
    unselected.changes = vec![Change {
        path: "raw/photo.bin".into(),
        text: None,
    }];
    assert_eq!(
        history.prepare(&unselected, &admission.claim).unwrap_err(),
        Error::Invalid
    );
    // Native re-selection may revalidate changed inert config, but may never reset app history back to user HEAD.
    config.push_str("\n# Actual native user changed a comment\n");
    fs::write(repo.join("config"), &config).unwrap();
    assert!(matches!(
        History::open(&repo, &home, "pages", owner(false), &admission.claim),
        Err(Error::ContextMismatch)
    ));
    let reopened =
        History::use_existing(selected, &home, "pages", owner(false), &admission.claim).unwrap();
    assert_eq!(
        reopened.head(&admission.claim).unwrap().as_ref(),
        Some(candidate)
    );
    assert_eq!(fs::read(repo.join("index")).unwrap(), index);
    assert_eq!(fs::read_to_string(repo.join("config")).unwrap(), config);
    assert!(!marker.exists());
}

/// Native synchronized/unknown placement cannot authorize use-existing; explicit separate history leaves user Git/HEAD untouched.
#[test]
fn existing_selection_requires_native_non_sync_and_separate_never_redirects_home_git() {
    use knowledge_history::{ExistingRepository, GitPlacement};
    let (root, home, repo) = legacy_user_repo();
    let admission = Admission::new();
    let head = git(&repo, &["rev-parse", "refs/heads/main"]);
    let index = fs::read(repo.join("index")).unwrap();
    let config = fs::read(repo.join("config")).unwrap();
    for placement in [GitPlacement::Synchronized, GitPlacement::Unknown] {
        assert!(matches!(
            History::use_existing(
                ExistingRepository {
                    git_dir: &repo,
                    baseline: "refs/heads/main",
                    placement
                },
                &home,
                "pages",
                owner(false),
                &admission.claim
            ),
            Err(Error::ContextMismatch)
        ));
        assert!(!repo.join("omni-state").exists());
    }
    let separate = History::init(
        &root.path().join("separate-local-history"),
        &home,
        "pages",
        owner(false),
        &admission.claim,
    )
    .unwrap();
    assert_eq!(separate.head(&admission.claim).unwrap(), None);
    assert!(home.join(".git").is_dir());
    assert_eq!(fs::read(repo.join("index")).unwrap(), index);
    assert_eq!(fs::read(repo.join("config")).unwrap(), config);
    assert_eq!(git(&repo, &["rev-parse", "refs/heads/main"]), head);
}

/// The native config parser refuses include/rewrite widening before creating app state, without reading/executing the include target.
#[test]
fn existing_config_include_is_inert_and_never_opened() {
    use knowledge_history::{ExistingRepository, GitPlacement};
    let (root, home, repo) = legacy_user_repo();
    let fifo = root.path().join("must-not-read-private-include");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .env_clear()
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let mut config = fs::read_to_string(repo.join("config")).unwrap();
    config.push_str(&format!("\n[include]\n path = {}\n", fifo.display()));
    fs::write(repo.join("config"), &config).unwrap();
    let admission = Admission::with_budget(Budget {
        max_files: 100,
        max_bytes: 524_288,
        max_millis: 2000,
        max_memory_bytes: 64 << 20,
    });
    let start = Instant::now();
    assert!(matches!(
        History::use_existing(
            ExistingRepository {
                git_dir: &repo,
                baseline: "refs/heads/main",
                placement: GitPlacement::LocalUnsynced
            },
            &home,
            "pages",
            owner(false),
            &admission.claim
        ),
        Err(Error::Invalid)
    ));
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(!repo.join("omni-state").exists());
}
