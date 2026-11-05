//! Native home bytes, visible journals and no-clobber recovery; neither transport authority nor Git.
#![cfg(any(target_os = "linux", target_os = "macos"))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod auxiliary;
mod bootstrap;
mod migration;
mod tracking;
pub use auxiliary::{JsonSize, start_resources};
pub use migration::{MigrationInventory, MigrationItem, MigrationKind};
pub use tracking::{
    ChangeKind, Checkpoint, Observation, ReconcileOptions, Reconciled, Replay, Schedule,
    Verification,
};
mod journal;
mod native;
pub use bootstrap::{ImportResult, InitOutcome, Preflight, Seed};
use fs2::FileExt;
use knowledge_core::{
    admission::{Account, Holding},
    jobs::{Claim, Key, Reservation},
};
use knowledge_history::{Destination, History, Prepared, Request, State as GitState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

/// Fixed errors never disclose rejected content, native paths or Git stderr.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Invalid normalized input or unsupported durable encoding.
    Invalid,
    /// Current native root/library/policy does not match; data declarations are not grants.
    Context,
    /// Current selected file/ref differs or an exclusive destination already exists.
    Conflict,
    /// Native syscall or sync failed before a proven complete outcome.
    Io,
    /// Native expected node was absent.
    Missing,
    /// Native file kind/platform/selected operation is unsupported.
    Unsupported,
    /// Finite admitted work/scratch/deadline does not fit.
    Budget,
    /// The exact running claim has a stop request; no settlement or rollback is implied.
    Stopped(knowledge_core::jobs::Cancellation),
    /// The execution claim/key/generation is no longer live.
    Claim(knowledge_core::jobs::JobError),
    /// A writer already owns this home operation lane.
    Busy,
    /// Malformed/truncated/contradictory evidence is retained for inspection.
    Recovery,
    /// Some effects may have happened; never infer rollback or replay automatically.
    Indeterminate,
    /// The one existing process resource account refused admission.
    Admission(knowledge_core::admission::Error),
    /// Actual source backend refusal, never an empty successful observation.
    Backend(knowledge_core::backend::BackendError),
    /// Actual shared-account contention without a retry loop or partial promise.
    Waiting(knowledge_core::admission::Waiting),
    /// The independently owned local Git seam refused the operation.
    History(knowledge_history::Error),
}
impl std::fmt::Display for Error {
    /// Format only a fixed classification.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "storage {self:?}")
    }
}
impl std::error::Error for Error {}
impl From<knowledge_core::admission::Error> for Error {
    /// Preserve typed memory/wait/account errors without input reflection.
    fn from(e: knowledge_core::admission::Error) -> Self {
        Self::Admission(e)
    }
}
impl From<knowledge_history::Error> for Error {
    /// Preserve Git failure without treating it as filesystem success.
    fn from(e: knowledge_history::Error) -> Self {
        Self::History(e)
    }
}

/// Actual regular-file byte version, not portable asset identity or permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    /// Native device qualified by the selected home.
    pub device: u64,
    /// Native inode, never by itself rename/reuse proof.
    pub inode: u64,
    /// Complete hashed length.
    pub length: u64,
    /// Native seconds/nanoseconds modification observation.
    pub modified: (i64, i64),
    /// Native seconds/nanoseconds change observation.
    pub changed: (i64, i64),
    /// SHA-256 of complete actually read bytes.
    pub digest: String,
}
/// Fresh trusted native owner facts; constructing them is not an authorization grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteContext {
    /// Selected library identity, not root discovery.
    pub library_id: String,
    /// Native owner policy revision; checked before/after effects.
    pub revision: u64,
    /// Exactly pages or wiki, never a second authored tree.
    pub text_root: String,
    /// Current native writable admission, supplied only by the actual owner.
    pub writable: bool,
    /// Default false; raw-original replacement is never implied by text write access.
    pub raw_original_writes: bool,
    /// Current exclusions and child-owned scopes, including descendants.
    pub outside_scope: Vec<String>,
}
/// Narrow current native write owner, not a generic permission registry or wire implementation.
pub trait WritePolicy: Send + Sync {
    /// Supply current validated context, or refuse when authority cannot be established.
    fn current(&self) -> Result<WriteContext, Error>;
}
/// Existing process claim and an explicit FS scratch portion, separate from unchanged work maxima.
///
/// M6/native consumers pass their actual owner, Git instance and live SAME-account claim:
/// ```no_run
/// use knowledge_core::jobs::{Claim, Key};
/// use knowledge_history::History;
/// use knowledge_storage::{Error, Inspection, Scope, Store, WritePolicy};
/// fn publish_current<'a, P: WritePolicy>(
///     store: &Store<P>, history: &History, operation: &str,
///     claim: &'a Claim, key: Key<'a>, scratch_bytes: u64,
/// ) -> Result<Inspection, Error> {
///     if let Some(stop) = claim.stop_requested(key).map_err(Error::Claim)? {
///         return Err(Error::Stopped(stop));
///     }
///     store.publish(history, operation, &Scope { claim, key, scratch_bytes })
/// }
/// ```
/// A stop result retains recovery evidence and does not settle the worker or release its mount slot.
pub struct Scope<'a> {
    /// Actual SAME-account unsettled execution; Git child collection borrows it too.
    pub claim: &'a Claim,
    /// Exact retained claim facts, not a fabricated pre-verification digest.
    pub key: Key<'a>,
    /// Explicit filesystem scratch memory already included in the claim's reservation.
    /// Caller-owned arguments/results and Git costs remain covered by the parent's remainder.
    pub scratch_bytes: u64,
}
/// Expected current bytes per selected path; None requires exclusive creation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Normalized home-relative selected path.
    pub path: String,
    /// Exact full digest/stat witness, or absence.
    pub version: Option<Version>,
}
/// Local durable phase; SavedLocal requires verified bytes, Git and a synchronized visible record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Visible intent/candidates/Git preparation are retained; installation is explicit.
    Prepared,
    /// All selected home bytes were installed; Git/final journal are not yet proven complete.
    FilesInstalled,
    /// Bytes/Git/final journal matched at completion; inspect current_matches separately later.
    SavedLocal,
    /// A proposal affected Git only, never home bytes.
    Proposal,
    /// Another native writer won; all recovery versions remain.
    Conflict,
    /// Facts require inspection and fresh explicit owner action, never blind retry.
    Indeterminate,
}
/// Honest inspection of retained evidence and CURRENT bytes, without replay.
pub struct Inspection {
    /// True only for this call's freshly prepared operation; duplicates are inspected, never replayed.
    pub newly_prepared: bool,
    /// Recorded/proven local phase, not a claim of cross-filesystem atomicity.
    pub phase: Phase,
    /// Whether actual current home bytes still match this historical operation's intended result.
    pub current_matches: bool,
    /// Immutable Git preparation, when available.
    pub prepared: Option<Prepared>,
    /// Actual independently inspected Git state.
    pub git: Option<GitState>,
}
/// One bound home; local cooperating writers serialize but external edits still require witnesses.
pub struct Store<P: WritePolicy> {
    /// Selected absolute path, revalidated rather than falling back to an ancestor.
    path: PathBuf,
    /// Captured native root.
    root: File,
    /// Selected stable library data identity.
    library: String,
    /// Visible stream writer identity.
    writer: String,
    /// Current native authority owner.
    policy: P,
}
/// Fixed metadata of one staged selected file; no source body is journal data.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    /// Exact expected selected path/version.
    expected: Expected,
    /// Intended resulting digest, null for a selected deletion.
    digest: Option<String>,
}
/// Immutable visible recovery recipe, never an authority or second annotation store.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    /// Explicit durable encoding version.
    format_version: u32,
    /// Stable operation id.
    operation_id: String,
    /// Selected writer identity.
    writer_id: String,
    /// Captured root device/inode.
    root: (u64, u64),
    /// Selected library id.
    library_id: String,
    /// Policy revision observed at preparation, retained only as evidence.
    revision: u64,
    /// Canonical complete normalized intent fingerprint.
    fingerprint: String,
    /// Proposal operations never install home files.
    proposal: bool,
    /// Exact immutable Git base used by the independently validated operation intent.
    base: Option<knowledge_history::Oid>,
    /// Exact accepted/proposal destination, not an authority declaration.
    destination: Destination,
    /// Normalized path records, not text bodies.
    entries: Vec<Entry>,
}
/// Synchronous filesystem scratch lifetime; all buffers/owned I/O end before this guard.
struct Work<'a> {
    /// Parent execution remains charged across synchronous phase loans.
    claim: &'a Claim,
    /// Exact parent key facts for checked return, not a new grant.
    key: Key<'a>,
    /// SAME-account portion split from the existing claim, not a separate pool.
    holding: Holding,
    /// Earliest actual host deadline.
    deadline: Instant,
    /// Explicit finite I/O limit for native observations/control data.
    bytes: u64,
    /// Scratch bytes admitted before allocations.
    memory: u64,
}
impl<'a> Work<'a> {
    /// Split before any operation-controlled metadata/buffer allocation.
    fn new(scope: &Scope<'a>) -> Result<Self, Error> {
        if let Some(stop) = scope
            .claim
            .stop_requested(scope.key)
            .map_err(Error::Claim)?
        {
            return Err(Error::Stopped(stop));
        }
        let budget = scope.claim.budget();
        if scope.scratch_bytes < 32768
            || scope.scratch_bytes > budget.max_memory_bytes
            || budget.max_files == 0
            || budget.max_bytes == 0
            || budget.max_millis == 0
        {
            return Err(Error::Budget);
        }
        let holding = Account::current()?.split_claim(
            scope.claim,
            scope.key,
            Reservation {
                memory_bytes: scope.scratch_bytes,
                temporary_bytes: 0,
            },
        )?;
        Ok(Self {
            holding,
            claim: scope.claim,
            key: scope.key,
            deadline: scope.claim.deadline(),
            bytes: scope.claim.budget().max_bytes,
            memory: scope.scratch_bytes,
        })
    }
    /// Observe exact claim cancellation/supersession and deadline between controllable steps.
    /// No check settles the worker, its mount slot or any outstanding resource owner.
    fn check(&self) -> Result<(), Error> {
        if let Some(stop) = self.claim.stop_requested(self.key).map_err(Error::Claim)? {
            return Err(Error::Stopped(stop));
        }
        if Instant::now() >= self.deadline {
            Err(Error::Budget)
        } else {
            Ok(())
        }
    }
    /// Bound parser-owned control input by its declared schema expansion allowance before allocation.
    fn control_limit(&self) -> u64 {
        self.bytes.min(self.memory.saturating_sub(32768) / 64)
    }
    /// Decode bounded control metadata without continuing parser reads after a stop request.
    fn control<T: serde::de::DeserializeOwned>(&self, dir: &File, name: &str) -> Result<T, Error> {
        native::control_checked(dir, name, self.control_limit(), || self.check())
    }
    /// Hash actual bytes with live-claim checks before each bounded native read.
    fn version(&self, file: File) -> Result<Version, Error> {
        native::version_checked(file, self.bytes, self.deadline, || self.check())
    }
    /// Observe a selected native file under the same live execution and fixed scratch plan.
    fn observe(&self, root: &File, path: &str) -> Result<Option<Version>, Error> {
        self.check()?;
        let (parent, name) = match native::parent(root, path, false) {
            Ok(p) => p,
            Err(Error::Missing) => return Ok(None),
            Err(e) => return Err(e),
        };
        native::regular(&parent, &name)?
            .map(|f| self.version(f))
            .transpose()
    }
}
impl Drop for Work<'_> {
    /// Return the loan without a charge gap; a stale parent conservatively leaves the holding charged.
    /// Git reaps its own children and the executor remains responsible for final whole-claim settlement.
    fn drop(&mut self) {
        if let Ok(account) = Account::current() {
            let _ = account.rejoin_claim(&mut self.holding, self.claim, self.key);
        }
    }
}
/// Native writer lock and visible bookkeeping directories.
struct Lane {
    /// Lifetime lock, released only after synchronous effects finish.
    _lock: File,
    /// Visible journal directory.
    journal: File,
    /// Visible same-filesystem recovery directory.
    recovery: File,
}

impl<P: WritePolicy> Store<P> {
    /// Bind one already selected home without creating files or discovering authority.
    /// The caller must reserve constructor/native-owner overhead in its startup/runtime costs.
    pub fn open(path: &Path, library: &str, writer: &str, policy: P) -> Result<Self, Error> {
        if library.is_empty() || !component(writer) {
            return Err(Error::Invalid);
        }
        let root = native::bind(path)?;
        Ok(Self {
            path: path.to_owned(),
            root,
            library: library.to_owned(),
            writer: writer.to_owned(),
            policy,
        })
    }
    /// Validate current root/policy facts without using a journal or manifest as a grant.
    fn gate(&self) -> Result<WriteContext, Error> {
        native::bound(&self.path, &self.root)?;
        let c = self.policy.current()?;
        if !c.writable
            || c.library_id != self.library
            || !matches!(c.text_root.as_str(), "pages" | "wiki")
        {
            return Err(Error::Context);
        }
        Ok(c)
    }
    /// Recheck the exact current native context; a changed revision/scope never inherits old approval.
    fn live(&self, context: &WriteContext) -> Result<(), Error> {
        if self.gate()? != *context {
            return Err(Error::Context);
        }
        Ok(())
    }
    /// Check lexical confinement, selected text/raw mode and all exclusion boundaries.
    fn selected(&self, path: &str, c: &WriteContext) -> Result<(), Error> {
        knowledge_core::data::RelativePath::new(path.to_owned()).map_err(|_| Error::Invalid)?;
        if c.outside_scope
            .iter()
            .any(|p| path == p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
        {
            return Err(Error::Context);
        }
        let authored = path.starts_with(&format!("{}/", c.text_root))
            || path.starts_with("sources/")
            || matches!(
                path,
                "omni-knowledge.json" | "AGENTS.md" | "schema.yaml" | "index.md" | "log.md"
            );
        if !authored && !(c.raw_original_writes && path.starts_with("raw/")) {
            return Err(Error::Context);
        }
        Ok(())
    }
    /// Acquire the real native home writer lock before journal/transaction mutation.
    fn lane(&self) -> Result<Lane, Error> {
        let journal = native::private_child(&self.root, "journal")?;
        let lock = native::append_file(&journal, ".write.lock")?;
        lock.try_lock_exclusive().map_err(|_| Error::Busy)?;
        let recovery = native::private_child(&journal, "recovery")?;
        Ok(Lane {
            _lock: lock,
            journal,
            recovery,
        })
    }
    /// Recheck native approval and the exact claim immediately before lane bookkeeping effects.
    fn checked_lane(&self, context: &WriteContext, work: &Work<'_>) -> Result<Lane, Error> {
        self.live(context)?;
        work.check()?;
        self.lane()
    }
    /// Open retained evidence without creating files during inspection.
    fn existing_lane(&self) -> Result<Lane, Error> {
        let journal = native::directory(&self.root, "journal")?;
        let lock = native::regular(&journal, ".write.lock")?.ok_or(Error::Recovery)?;
        lock.try_lock_exclusive().map_err(|_| Error::Busy)?;
        let recovery = native::directory(&journal, "recovery")?;
        Ok(Lane {
            _lock: lock,
            journal,
            recovery,
        })
    }
    /// Match actual cost attribution to the selected library before borrowing a claim portion.
    fn work<'a>(&self, scope: &Scope<'a>) -> Result<Work<'a>, Error> {
        if scope.key.library != self.library {
            return Err(Error::Context);
        }
        Work::new(scope)
    }
    /// Observe actual complete selected bytes under current native admission and a real shared claim.
    pub fn observe(&self, path: &str, scope: &Scope<'_>) -> Result<Option<Version>, Error> {
        let work = self.work(scope)?;
        work.check()?;
        let c = self.gate()?;
        if path.len() as u64 > work.control_limit() / 8 {
            return Err(Error::Budget);
        }
        self.selected(path, &c)?;
        let v = work.observe(&self.root, path)?;
        self.live(&c)?;
        Ok(v)
    }
    /// Execute a NEW admitted operation in prepare/install/publish order; duplicates only inspect.
    /// After possible home effects, failures require explicit inspection, never blind rollback/replay.
    pub fn apply(
        &self,
        history: &History,
        request: &Request,
        expected: &[Expected],
        scope: &Scope<'_>,
    ) -> Result<Inspection, Error> {
        let prepared = self.prepare(history, request, expected, scope)?;
        if !prepared.newly_prepared {
            return Ok(prepared);
        }
        if matches!(request.destination, Destination::Proposal { .. }) {
            return self.publish(history, &request.operation_id, scope);
        }
        match self.install(history, &request.operation_id, scope) {
            Ok(_) => {}
            Err(Error::Conflict) => {
                return Ok(Inspection {
                    newly_prepared: false,
                    phase: Phase::Conflict,
                    current_matches: false,
                    prepared: prepared.prepared,
                    git: None,
                });
            }
            Err(_) => return Err(Error::Indeterminate),
        }
        self.publish(history, &request.operation_id, scope)
            .map_err(|_| Error::Indeterminate)
    }
    /// Persist visible normalized intent/candidates and prepare real Git BEFORE any home replacement.
    /// An existing operation is inspected rather than replayed; differing normalized intent refuses.
    pub fn prepare(
        &self,
        history: &History,
        request: &Request,
        expected: &[Expected],
        scope: &Scope<'_>,
    ) -> Result<Inspection, Error> {
        let work = self.work(scope)?;
        work.check()?;
        let c = self.gate()?;
        if request.changes.len() as u64 > scope.claim.budget().max_files
            || request.changes.len() != expected.len()
        {
            return Err(Error::Budget);
        }
        // Size the complete simultaneous metadata plan before any collect, sort, clone or serialization.
        let mut metadata = [
            request.operation_id.len(),
            self.writer.len(),
            self.library.len(),
            match &request.destination {
                Destination::Proposal { proposal_id } => proposal_id.len(),
                _ => 0,
            },
        ]
        .into_iter()
        .try_fold(4096u64, |n, len| {
            n.checked_add((len as u64).checked_mul(6)?)
        })
        .ok_or(Error::Budget)?;
        if metadata > work.control_limit() {
            return Err(Error::Budget);
        }
        let mut input_bytes = 0u64;
        for change in &request.changes {
            if change.path.len() as u64 > work.control_limit() / 8 {
                return Err(Error::Budget);
            }
            work.check()?;
            let e = expected
                .iter()
                .find(|e| e.path == change.path)
                .ok_or(Error::Invalid)?;
            let bytes = change.text.as_ref().map_or(0, |s| s.len() as u64);
            input_bytes = input_bytes.checked_add(bytes).ok_or(Error::Budget)?;
            if input_bytes > work.bytes {
                return Err(Error::Budget);
            }
            if change.path == "omni-knowledge.json" && change.text.is_none() {
                return Err(Error::Context);
            }
            metadata = metadata
                .checked_add(change.path.len() as u64 * 8 + 1024)
                .ok_or(Error::Budget)?;
            if metadata > work.control_limit() {
                return Err(Error::Budget);
            }
            if e.version.as_ref().is_some_and(|v| !digest_valid(&v.digest)) {
                return Err(Error::Invalid);
            }
        }
        let vectors = (request.changes.len() as u64)
            .checked_mul(
                (std::mem::size_of::<&knowledge_history::Change>() + std::mem::size_of::<Entry>())
                    as u64,
            )
            .ok_or(Error::Budget)?;
        if metadata
            .checked_mul(32)
            .and_then(|n| n.checked_add(vectors))
            .is_none_or(|n| n > work.memory.saturating_sub(16384))
            || input_bytes.checked_mul(2).is_none_or(|n| n > work.bytes)
        {
            return Err(Error::Budget);
        }
        let mut changes = request.changes.iter().collect::<Vec<_>>();
        changes.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        if changes.windows(2).any(|w| w[0].path == w[1].path) {
            return Err(Error::Invalid);
        }
        let mut entries = Vec::with_capacity(changes.len());
        for change in &changes {
            work.check()?;
            self.selected(&change.path, &c)?;
            let e = expected
                .iter()
                .find(|e| e.path == change.path)
                .ok_or(Error::Invalid)?;
            entries.push(Entry {
                expected: e.clone(),
                digest: change.text.as_ref().map(|t| hash(t.as_bytes())),
            });
        }
        let fingerprint = fingerprint(request, &entries, &changes)?;
        let intent = Intent {
            format_version: 1,
            operation_id: request.operation_id.clone(),
            writer_id: self.writer.clone(),
            root: native::identity(&self.root)?,
            library_id: self.library.clone(),
            revision: c.revision,
            fingerprint,
            proposal: matches!(request.destination, Destination::Proposal { .. }),
            base: request.base.clone(),
            destination: request.destination.clone(),
            entries,
        };
        self.admit_intent(&intent, &c, &work)?;
        let lane = self.checked_lane(&c, &work)?;
        let op = hash(request.operation_id.as_bytes());
        match native::directory(&lane.recovery, &op) {
            Ok(dir) => {
                let old: Intent = work.control(&dir, "intent.json")?;
                if old.fingerprint != intent.fingerprint || old.operation_id != intent.operation_id
                {
                    return Err(Error::Conflict);
                }
                drop(old);
                drop(intent);
                drop(changes);
                drop(dir);
                drop(lane);
                drop(op);
                drop(c);
                drop(work);
                return self.inspect(history, &request.operation_id, scope);
            }
            Err(Error::Missing) => {}
            Err(e) => return Err(e),
        }
        for e in &intent.entries {
            if work.observe(&self.root, &e.expected.path)? != e.expected.version {
                return Err(Error::Conflict);
            }
        }
        self.admit_intent(&intent, &c, &work)?;
        let dir = native::new_child(&lane.recovery, &op)?.ok_or(Error::Conflict)?;
        let mut journal =
            journal::Journal::open(&lane.journal, &self.writer, work.control_limit())?;
        self.admit_intent(&intent, &c, &work)?;
        let begin = journal.append(&intent.operation_id, &intent.fingerprint, None, "intent")?;
        self.admit_intent(&intent, &c, &work)?;
        native::control_new(&dir, "intent.json", &intent)?;
        self.admit_intent(&intent, &c, &work)?;
        native::control_new(&dir, "begin.json", &begin)?;
        self.live(&c)?;
        work.check()?;
        let prepared = history.prepare(request, scope.claim)?;
        self.admit_intent(&intent, &c, &work)?;
        native::control_new(&dir, "prepared.json", &prepared)?;
        for (n, change) in changes.iter().enumerate() {
            self.admit_intent(&intent, &c, &work)?;
            if let Some(text) = &change.text {
                native::create_checked(&dir, &format!("new-{n}"), text.as_bytes(), || {
                    self.live(&c)?;
                    work.check()
                })?;
            }
        }
        self.admit_intent(&intent, &c, &work)?;
        journal.append(
            &intent.operation_id,
            &intent.fingerprint,
            prepared.candidate.as_ref().map(|o| o.as_str()),
            "prepared",
        )?;
        self.live(&c)?;
        Ok(Inspection {
            newly_prepared: true,
            phase: Phase::Prepared,
            current_matches: false,
            prepared: Some(prepared),
            git: None,
        })
    }
    /// Install explicitly after preparation; preserve displaced/competing/candidate bytes on every failure.
    /// No automatic rollback, unconditional rename, multi-file atomicity or worker-stop claim occurs.
    pub fn install(
        &self,
        history: &History,
        operation: &str,
        scope: &Scope<'_>,
    ) -> Result<Inspection, Error> {
        let work = self.work(scope)?;
        work.check()?;
        let c = self.gate()?;
        work.check()?;
        let lane = self.existing_lane()?;
        let dir = native::directory(&lane.recovery, &hash(operation.as_bytes()))?;
        let intent: Intent = work.control(&dir, "intent.json")?;
        self.intent(&intent, operation)?;
        self.begin(&lane, &dir, &intent, &work)?;
        if intent.proposal {
            return Err(Error::Context);
        }
        self.admit_intent(&intent, &c, &work)?;
        let prepared = self.verify_prepared(history, &dir, &intent, scope, &work, &c)?;
        let mut journal =
            journal::Journal::open(&lane.journal, &self.writer, work.control_limit())?;
        for (n, entry) in intent.entries.iter().enumerate() {
            work.check()?;
            self.live(&c)?;
            let path = &entry.expected.path;
            self.selected(path, &c)?;
            let current = work.observe(&self.root, path)?;
            let old = native::regular(&dir, &format!("old-{n}"))?
                .map(|f| work.version(f))
                .transpose()?;
            let staged = native::regular(&dir, &format!("new-{n}"))?
                .map(|f| work.version(f))
                .transpose()?;
            if staged.as_ref().map(|v| &v.digest) != entry.digest.as_ref() {
                return Err(Error::Recovery);
            }
            if old.as_ref().is_some_and(|v| {
                !entry
                    .expected
                    .version
                    .as_ref()
                    .is_some_and(|e| same_displaced(e, v))
            }) {
                return Err(Error::Recovery);
            }
            let already = match (&current, &staged, &entry.expected.version, &old) {
                (Some(now), Some(new), Some(_), Some(_)) | (Some(now), Some(new), None, None) => {
                    now.length == new.length && now.digest == new.digest
                }
                (None, None, Some(_), Some(_)) | (None, None, None, None) => true,
                _ => false,
            };
            if already {
                continue;
            }
            let displaced = entry.expected.version.is_some() && old.is_some() && current.is_none();
            if !displaced && current != entry.expected.version {
                return Err(Error::Conflict);
            }
            self.admit_intent(&intent, &c, &work)?;
            let (parent, name) = native::parent(&self.root, path, true)?;
            self.admit_intent(&intent, &c, &work)?;
            journal.append(
                operation,
                &intent.fingerprint,
                prepared.candidate.as_ref().map(|o| o.as_str()),
                "installing",
            )?;
            // Recheck bytes after journal I/O, then policy and native ancestry at the last effect boundary.
            if !displaced && work.observe(&self.root, path)? != entry.expected.version {
                return Err(Error::Conflict);
            }
            self.admit_intent(&intent, &c, &work)?;
            self.parent_live(path, &parent, &name, &work)?;
            if !displaced && let Some(expected) = &entry.expected.version {
                native::displace(&parent, &name, &dir, &format!("old-{n}"))?;
                let old = work
                    .version(native::regular(&dir, &format!("old-{n}"))?.ok_or(Error::Recovery)?)?;
                if !same_displaced(expected, &old) {
                    return Err(Error::Conflict);
                }
            }
            self.live(&c)?;
            if entry.digest.is_some() {
                native::install_evidence(
                    &dir,
                    &format!("new-{n}"),
                    &parent,
                    &name,
                    staged.as_ref().ok_or(Error::Recovery)?,
                    work.bytes,
                    || {
                        self.live(&c)?;
                        self.parent_live(path, &parent, &name, &work)
                    },
                )?;
            }
        }
        self.live(&c)?;
        if !self.matches(&intent, &work)? {
            return Err(Error::Indeterminate);
        }
        self.admit_intent(&intent, &c, &work)?;
        if native::regular(&dir, "installed")?.is_none() {
            self.admit_intent(&intent, &c, &work)?;
            native::create(&dir, "installed", b"1\n")?;
        }
        self.admit_intent(&intent, &c, &work)?;
        journal.append(
            operation,
            &intent.fingerprint,
            prepared.candidate.as_ref().map(|o| o.as_str()),
            "files_installed",
        )?;
        Ok(Inspection {
            newly_prepared: false,
            phase: Phase::FilesInstalled,
            current_matches: true,
            prepared: Some(prepared),
            git: None,
        })
    }
    /// Publish real Git only after matching installed bytes, then prove the final visible saved record.
    pub fn publish(
        &self,
        history: &History,
        operation: &str,
        scope: &Scope<'_>,
    ) -> Result<Inspection, Error> {
        let work = self.work(scope)?;
        work.check()?;
        let c = self.gate()?;
        work.check()?;
        let lane = self.existing_lane()?;
        let dir = native::directory(&lane.recovery, &hash(operation.as_bytes()))?;
        let intent: Intent = work.control(&dir, "intent.json")?;
        self.intent(&intent, operation)?;
        self.admit_intent(&intent, &c, &work)?;
        self.begin(&lane, &dir, &intent, &work)?;
        let prepared = self.verify_prepared(history, &dir, &intent, scope, &work, &c)?;
        if !intent.proposal
            && (native::regular(&dir, "installed")?.is_none() || !self.matches(&intent, &work)?)
        {
            return Err(Error::Conflict);
        }
        self.admit_intent(&intent, &c, &work)?;
        let git = history.publish(operation, scope.claim)?;
        self.admit_intent(&intent, &c, &work)?;
        let mut journal =
            journal::Journal::open(&lane.journal, &self.writer, work.control_limit())?;
        if !matches!(git, GitState::Published(_) | GitState::Unchanged(_)) {
            return Ok(Inspection {
                newly_prepared: false,
                phase: Phase::Indeterminate,
                current_matches: false,
                prepared: Some(prepared),
                git: Some(git),
            });
        }
        if intent.proposal {
            self.admit_intent(&intent, &c, &work)?;
            journal.append(
                operation,
                &intent.fingerprint,
                prepared.candidate.as_ref().map(|o| o.as_str()),
                "proposal",
            )?;
            return Ok(Inspection {
                newly_prepared: false,
                phase: Phase::Proposal,
                current_matches: false,
                prepared: Some(prepared),
                git: Some(git),
            });
        }
        if !self.matches(&intent, &work)? || history.head(scope.claim)? != prepared.candidate {
            return Err(Error::Indeterminate);
        }
        self.admit_intent(&intent, &c, &work)?;
        let proof = journal.append(
            operation,
            &intent.fingerprint,
            prepared.candidate.as_ref().map(|o| o.as_str()),
            "saved_local",
        )?;
        if native::regular(&dir, "saved.json")?.is_none() {
            self.admit_intent(&intent, &c, &work)?;
            native::control_new(&dir, "saved.json", &proof)?;
        }
        Ok(Inspection {
            newly_prepared: false,
            phase: Phase::SavedLocal,
            current_matches: true,
            prepared: Some(prepared),
            git: Some(git),
        })
    }
    /// Inspect actual retained home/Git/journal facts without reinstalling, deleting or publishing anything.
    pub fn inspect(
        &self,
        history: &History,
        operation: &str,
        scope: &Scope<'_>,
    ) -> Result<Inspection, Error> {
        let work = self.work(scope)?;
        work.check()?;
        let c = self.gate()?;
        let lane = self.existing_lane()?;
        let dir = native::directory(&lane.recovery, &hash(operation.as_bytes()))?;
        let intent: Intent = work.control(&dir, "intent.json")?;
        self.intent(&intent, operation)?;
        self.begin(&lane, &dir, &intent, &work)?;
        for e in &intent.entries {
            self.selected(&e.expected.path, &c)?;
        }
        let _request = self.restored_request(&dir, &intent, &work)?;
        let prepared: Prepared = work.control(&dir, "prepared.json")?;
        let git = history.inspect(operation, scope.claim)?;
        let actual = match &git {
            GitState::Prepared(p)
            | GitState::Published(p)
            | GitState::Unchanged(p)
            | GitState::Conflict(p)
            | GitState::Indeterminate(p) => p,
        };
        if actual != &prepared {
            return Err(Error::Recovery);
        }
        let current_matches = match self.matches(&intent, &work) {
            Ok(matches) => matches,
            Err(Error::Io | Error::Missing | Error::Unsupported | Error::Conflict) => false,
            Err(e) => return Err(e),
        };
        let phase = if native::regular(&dir, "saved.json")?.is_some() {
            let proof: journal::Proof = work.control(&dir, "saved.json")?;
            let mut journal =
                journal::Journal::read(&lane.journal, &proof.writer, work.control_limit())?;
            journal.verify(&proof, operation, &intent.fingerprint, "saved_local")?;
            if let Some(issue) = journal.issue() {
                return Err(issue);
            }
            if !matches!(git, GitState::Published(_) | GitState::Unchanged(_)) {
                Phase::Indeterminate
            } else {
                Phase::SavedLocal
            }
        } else if intent.proposal && matches!(git, GitState::Published(_) | GitState::Unchanged(_))
        {
            Phase::Proposal
        } else if native::regular(&dir, "installed")?.is_some() && current_matches {
            Phase::FilesInstalled
        } else if matches!(git, GitState::Conflict(_)) {
            Phase::Conflict
        } else if matches!(git, GitState::Indeterminate(_))
            || self.progressed(&dir, &intent, &work)?
        {
            Phase::Indeterminate
        } else {
            Phase::Prepared
        };
        self.live(&c)?;
        Ok(Inspection {
            newly_prepared: false,
            phase,
            current_matches,
            prepared: Some(prepared),
            git: Some(git),
        })
    }
    /// Reconstruct only bounded staged bytes and ask the actual Git owner to validate its immutable intent.
    /// A forged recovery entry cannot authorize a different deletion/body or manufacture matching Git facts.
    fn verify_prepared(
        &self,
        history: &History,
        dir: &File,
        intent: &Intent,
        scope: &Scope<'_>,
        work: &Work<'_>,
        context: &WriteContext,
    ) -> Result<Prepared, Error> {
        let stored: Prepared = work.control(dir, "prepared.json")?;
        let request = self.restored_request(dir, intent, work)?;
        self.admit_intent(intent, context, work)?;
        let actual = history.prepare(&request, scope.claim)?;
        self.admit_intent(intent, context, work)?;
        if actual != stored {
            return Err(Error::Recovery);
        }
        match history.inspect(&intent.operation_id, scope.claim)? {
            GitState::Conflict(_) => return Err(Error::Conflict),
            GitState::Indeterminate(_) => return Err(Error::Indeterminate),
            _ => {}
        }
        Ok(actual)
    }
    /// Reconstruct and verify bounded native metadata/body intent WITHOUT Git mutation during inspection.
    fn restored_request(
        &self,
        dir: &File,
        intent: &Intent,
        work: &Work<'_>,
    ) -> Result<Request, Error> {
        work.check()?;
        let mut total = 0u64;
        for (n, e) in intent.entries.iter().enumerate() {
            work.check()?;
            if e.digest.is_some() {
                let f = native::regular(dir, &format!("new-{n}"))?.ok_or(Error::Recovery)?;
                total = total
                    .checked_add(f.metadata().map_err(|_| Error::Io)?.len())
                    .ok_or(Error::Budget)?;
            }
        }
        // Control/parser expansion uses at most half scratch; bodies and their containers use the rest.
        let destination_bytes = match &intent.destination {
            Destination::Proposal { proposal_id } => proposal_id.len() as u64,
            Destination::Accepted => 0,
        };
        let containers = (intent.operation_id.len() as u64)
            .checked_add(destination_bytes)
            .and_then(|n| n.checked_add(std::mem::size_of::<Request>() as u64 + 128))
            .and_then(|n| {
                intent.entries.iter().try_fold(n, |n, e| {
                    n.checked_add(e.expected.path.len() as u64)?
                        .checked_add(std::mem::size_of::<knowledge_history::Change>() as u64)
                })
            })
            .ok_or(Error::Budget)?;
        if total.checked_mul(2).is_none_or(|n| n > work.bytes)
            || total
                .checked_add(containers)
                .is_none_or(|n| n > work.memory.saturating_sub(32768) / 2)
        {
            return Err(Error::Budget);
        }
        let mut changes = Vec::with_capacity(intent.entries.len());
        let mut remaining = total;
        for (n, e) in intent.entries.iter().enumerate() {
            work.check()?;
            let text = if let Some(digest) = &e.digest {
                let t = native::text_checked(dir, &format!("new-{n}"), remaining, || work.check())?;
                remaining = remaining.checked_sub(t.len() as u64).ok_or(Error::Budget)?;
                if hash(t.as_bytes()) != *digest {
                    return Err(Error::Recovery);
                }
                Some(t)
            } else {
                None
            };
            changes.push(knowledge_history::Change {
                path: e.expected.path.clone(),
                text,
            });
        }
        let request = Request {
            operation_id: intent.operation_id.clone(),
            base: intent.base.clone(),
            destination: intent.destination.clone(),
            changes,
        };
        if fingerprint(&request, &intent.entries, &request.changes)? != intent.fingerprint {
            return Err(Error::Recovery);
        }
        Ok(request)
    }
    /// Detect possible retained filesystem progress; never label partial effects as merely unstarted.
    fn progressed(&self, dir: &File, intent: &Intent, work: &Work<'_>) -> Result<bool, Error> {
        for (n, e) in intent.entries.iter().enumerate() {
            work.check()?;
            if native::regular(dir, &format!("old-{n}"))?.is_some() {
                return Ok(true);
            }
            if let Some(staged) = native::regular(dir, &format!("new-{n}"))?
                && let Some(current) = work.observe(&self.root, &e.expected.path)?
                && work.version(staged)?.digest == current.digest
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// Anchor immutable intent to actual synchronized visible evidence, never declarations alone.
    fn begin(
        &self,
        lane: &Lane,
        dir: &File,
        intent: &Intent,
        work: &Work<'_>,
    ) -> Result<(), Error> {
        let proof: journal::Proof = work.control(dir, "begin.json")?;
        if proof.writer != intent.writer_id {
            return Err(Error::Recovery);
        }
        let mut journal =
            journal::Journal::read(&lane.journal, &proof.writer, work.control_limit())?;
        journal.verify(&proof, &intent.operation_id, &intent.fingerprint, "intent")?;
        if let Some(issue) = journal.issue() {
            return Err(issue);
        }
        Ok(())
    }
    /// Validate recovery scope, not declarations as authority; current policy is checked separately.
    fn intent(&self, intent: &Intent, operation: &str) -> Result<(), Error> {
        if intent.format_version != 1
            || intent.operation_id != operation
            || !component(&intent.writer_id)
            || intent.library_id != self.library
            || intent.root != native::identity(&self.root)?
        {
            return Err(Error::Recovery);
        }
        Ok(())
    }
    /// Re-admit every retained path before effects; immutable intent never grants current access.
    fn admit_intent(
        &self,
        intent: &Intent,
        context: &WriteContext,
        work: &Work<'_>,
    ) -> Result<(), Error> {
        work.check()?;
        self.live(context)?;
        for entry in &intent.entries {
            work.check()?;
            self.selected(&entry.expected.path, context)?;
        }
        work.check()
    }
    /// Re-resolve actual child/root boundaries after policy callbacks and immediately before native effects.
    /// A captured parent FD or unchanged policy snapshot cannot grant a newly child-owned path.
    /// Returns native/path errors, Context on parent/name drift, or the exact live-claim stop error.
    fn parent_live(
        &self,
        path: &str,
        parent: &File,
        name: &str,
        work: &Work<'_>,
    ) -> Result<(), Error> {
        work.check()?;
        let (current_parent, current_name) = native::parent(&self.root, path, false)?;
        if native::identity(&current_parent)? != native::identity(parent)? || current_name != name {
            return Err(Error::Context);
        }
        work.check()
    }
    /// Compare CURRENT actual file digests, including explicit absence for selected deletions.
    fn matches(&self, intent: &Intent, work: &Work<'_>) -> Result<bool, Error> {
        for e in &intent.entries {
            work.check()?;
            let v = work.observe(&self.root, &e.expected.path)?;
            if v.as_ref().map(|v| &v.digest) != e.digest.as_ref() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
/// Preserve full byte/version evidence across rename's expected ctime change; body drift still refuses.
fn same_displaced(a: &Version, b: &Version) -> bool {
    a.device == b.device
        && a.inode == b.inode
        && a.length == b.length
        && a.modified == b.modified
        && a.digest == b.digest
}
/// Require only a native single component, never a UUID/identity-format quota.
fn component(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\', '\0'])
}
/// Check native witness spelling without allocating another digest string.
fn digest_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// Hash complete actual metadata/body bytes, never synthesize a verified original digest.
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Hash already normalized borrowed changes and witnesses without another vector or serialization tree.
/// Prepare supplies its prepaid sorted references; restoration supplies changes in exact intent order.
fn fingerprint(
    request: &Request,
    entries: &[Entry],
    changes: &impl Serialize,
) -> Result<String, Error> {
    let mut sink = HashWriter(Sha256::new());
    serde_json::to_writer(
        &mut sink,
        &(&request.base, &request.destination, &changes, entries),
    )
    .map_err(|_| Error::Invalid)?;
    Ok(format!("{:x}", sink.0.finalize()))
}
/// Feed canonical borrowed serialization directly into a digest without copying request bodies.
struct HashWriter(Sha256);
impl Write for HashWriter {
    /// Hash every supplied serialized byte; no output buffer/tree is retained.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    /// There is no buffered external output to flush.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
