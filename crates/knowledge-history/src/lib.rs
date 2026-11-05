//! Local Git object/ref transactions; home file installation and authorization belong to callers.
#![deny(unsafe_code)]

mod writer_lock;
use writer_lock::OwnedWriterLock as WriterLock;

use fs2::FileExt;
use knowledge_core::jobs::Claim;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Exact inert configuration for repositories created by this owner, never imported configuration.
const CONFIG: &str = "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = true\n\tlogallrefupdates = false\n[gc]\n\tauto = 0\n";
/// Accepted home reference; proposals never update it.
const ACCEPTED: &str = "refs/heads/omni-accepted";
/// Maximum receipt or control result; larger complete outputs refuse rather than truncate.
const CONTROL_LIMIT: usize = 524_288;

/// Validated immutable SHA-1 object identity, not authority to read a repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Oid(String);
impl Oid {
    /// Parse exactly 40 lowercase hexadecimal bytes; reject revision and option syntax.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() != 40
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid);
        }
        Ok(Self(value.to_owned()))
    }
    /// Return the validated object spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Oid {
    type Error = Error;
    /// Validate deserialized recovery identities too.
    fn try_from(value: String) -> Result<Self, Error> {
        Self::parse(&value)
    }
}
impl From<Oid> for String {
    /// Consume a validated identity without changing its spelling.
    fn from(value: Oid) -> Self {
        value.0
    }
}

/// Explicit service-selected signing policy, never an automatic fallback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Owner explicitly selected visible unsigned commits.
    UnsignedExplicit,
    /// A signer must be integrated; this backend preserves intent and refuses unsigned commits.
    SigningRequired,
}
/// Trusted service-derived human identity and validated agent context; not a wire DTO.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerContext {
    /// Selected human author and committer name.
    pub name: String,
    /// Selected human email, supplied by the service, never read from user configuration.
    pub email: String,
    /// Validated agent-on-behalf-of-user fact, if any.
    pub agent: Option<String>,
    /// Validated tool fact, if any.
    pub tool: Option<String>,
    /// Validated session fact, if any.
    pub session: Option<String>,
    /// Explicit selected signing policy.
    pub mode: Mode,
}
/// A narrowly selected destination, not a general branch-management surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    /// Owner-only accepted home history.
    Accepted,
    /// Isolated immutable proposal branch.
    Proposal {
        /// Validated proposal identifier.
        proposal_id: String,
    },
}
/// One selected text delta; deletion uses None and never touches home bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// Normalized home-relative selected text path.
    pub path: String,
    /// Exact UTF-8 candidate bytes or deletion.
    pub text: Option<String>,
}
/// One idempotent Git intent over an exact immutable base tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Library-scoped operation identifier, bound to all normalized arguments and owner context.
    pub operation_id: String,
    /// Exact expected accepted/base commit, or an initial empty history.
    pub base: Option<Oid>,
    /// Accepted or isolated proposal destination.
    pub destination: Destination,
    /// Multi-file delta, sorted internally; duplicate paths refuse.
    pub changes: Vec<Change>,
}
/// Exact service-confirmed proposal tuple; creation of this struct is not human authorization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Confirmation {
    /// Proposal branch selected by the human.
    pub proposal_id: String,
    /// Accepted version displayed to the human.
    pub base: Oid,
    /// Proposal version displayed to the human.
    pub candidate: Oid,
    /// Complete displayed patch SHA-256, lowercase hexadecimal.
    pub diff_digest: String,
}
/// Immutable candidate retained for safe-file installation or recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prepared {
    /// Original idempotency key.
    pub operation_id: String,
    /// Operation commit; absent only for an initially empty unchanged operation.
    pub candidate: Option<Oid>,
    /// Exact selected resulting tree.
    pub tree: Oid,
    /// Changed paths, excluding unchanged supplied values.
    pub affected_paths: Vec<String>,
    /// No text difference; no empty operation commit was made.
    pub unchanged: bool,
}
/// Git-only durable inspection; no state asserts home-file or cloud atomicity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Candidate retained but target/operation refs not published.
    Prepared(Prepared),
    /// Operation ref proves publication, including after the target later advances.
    Published(Prepared),
    /// Durable unchanged receipt; no text-operation commit.
    Unchanged(Prepared),
    /// Another ref version won; the candidate and previous commits remain retained.
    Conflict(Prepared),
    /// Facts do not prove publication or safe replay; preserve all versions for inspection.
    Indeterminate(Prepared),
}
/// Complete bounded comparison for human confirmation, not a partial preview digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    /// SHA-256 over all patch bytes.
    pub diff_digest: String,
    /// All affected selected paths.
    pub affected_paths: Vec<String>,
    /// Exact Git patch bytes.
    pub patch: Vec<u8>,
}
/// Input-free error classifications; submitted paths, identities and Git stderr never escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Invalid path, ID, identity or unsupported repository shape.
    Invalid,
    /// Operation ID already binds a different normalized intent.
    OperationMismatch,
    /// Selected home/repository/human context does not match retained authority facts.
    ContextMismatch,
    /// Required signing is unavailable; never downgrade to unsigned.
    SigningRequired,
    /// Admitted per-call capacity is insufficient.
    Capacity,
    /// Admitted deadline expired; exact child has been collected before return.
    Deadline,
    /// Another cooperating writer owns this repository.
    Busy,
    /// A native Git command failed; inspect recovery before an explicit retry.
    Git,
    /// Local I/O failed; do not infer rollback.
    Io,
    /// Durable recovery facts are malformed or contradictory.
    Recovery,
    /// Expected proposal/base/diff no longer matches.
    Conflict,
}
impl std::fmt::Display for Error {
    /// Emit a fixed classification without data reflection.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "history {self:?}")
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    /// Classify I/O without leaking filesystem paths.
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

/// Native selected directory identity, not a stored-path authorization grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Identity {
    /// Native device.
    device: u64,
    /// Native inode.
    inode: u64,
}
/// Fixed binding persisted at creation and revalidated against caller-selected paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Binding {
    /// Schema version.
    version: u32,
    /// Native external repository directory.
    repo: Identity,
    /// Native visible home directory.
    home: Identity,
    /// Selected authored text root.
    text_root: String,
    /// Selected human name.
    name: String,
    /// Selected human email.
    email: String,
    /// Explicit signing choice.
    mode: Mode,
}
/// One durable operation recipe; commit construction is deterministic after this file is synced.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Intent {
    /// Local recipe version.
    version: u32,
    /// Original normalized request fingerprint, never raw caller arguments.
    fingerprint: String,
    /// Complete authoritative owner context fingerprint.
    context: String,
    /// Expected branch before the native transaction.
    expected: Option<Oid>,
    /// Exact validated target ref.
    target: String,
    /// Exact confirmed proposal binding; absent on ordinary and legacy operations.
    #[serde(default)]
    proposal: Option<ConfirmedProposal>,
    /// Commit parents; confirmation retains both histories.
    parents: Vec<Oid>,
    /// Prepared result; candidate is deterministically reconstructed if necessary.
    prepared: Prepared,
    /// Persisted commit timestamp prevents duplicate objects across interruption.
    seconds: u64,
}
/// Proposal version displayed for one integration; publication must verify this exact native ref.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConfirmedProposal {
    /// Fixed proposal namespace plus validated proposal identifier.
    reference: String,
    /// Exact proposal commit accepted by the human, also the second integration parent.
    oid: Oid,
}
/// Selected app-owned external bare repository; no admission pool or home-file writer.
pub struct History {
    /// Canonical repository selected by the owner.
    repo: PathBuf,
    /// Canonical visible home selected by the owner.
    home: PathBuf,
    /// Native and human binding.
    binding: Binding,
    /// Trusted current attribution context.
    owner: OwnerContext,
}
/// Per-call exact deadline and finite retained output bound, from the existing admitted claim.
struct Call {
    /// End of the entire public call, not a timeout restarted per subprocess.
    deadline: Instant,
    /// Maximum one complete control/output result.
    limit: usize,
    /// Maximum changed/base entries admitted by the caller.
    files: u64,
    /// Remaining total input/output bytes, not a new resource reservation.
    bytes_left: Cell<u64>,
    /// Remaining examined entries, conservatively counting repeated tree reads.
    files_left: Cell<u64>,
}
impl Call {
    /// Bind work to a live caller claim without creating or settling a reservation.
    fn new(claim: &Claim) -> Result<Self, Error> {
        let b = claim.budget();
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(b.max_millis))
            .ok_or(Error::Invalid)?
            .min(claim.deadline());
        if b.max_bytes == 0 || b.max_memory_bytes == 0 || b.max_files == 0 {
            return Err(Error::Capacity);
        }
        let limit = b
            .max_bytes
            .min(b.max_memory_bytes / 4)
            .min(CONTROL_LIMIT as u64) as usize;
        let call = Self {
            deadline,
            limit,
            files: b.max_files,
            bytes_left: Cell::new(b.max_bytes),
            files_left: Cell::new(b.max_files),
        };
        call.check()?;
        Ok(call)
    }
    /// Debit actual bounded I/O against the caller's whole-call work maximum.
    fn account_bytes(&self, bytes: usize) -> Result<(), Error> {
        self.bytes_left.set(
            self.bytes_left
                .get()
                .checked_sub(bytes as u64)
                .ok_or(Error::Capacity)?,
        );
        Ok(())
    }
    /// Debit examined objects before work; no parallel admission account is created.
    fn account_files(&self, files: u64) -> Result<(), Error> {
        self.files_left.set(
            self.files_left
                .get()
                .checked_sub(files)
                .ok_or(Error::Capacity)?,
        );
        Ok(())
    }
    /// Stop further work on expiration; existing children are collected by run.
    fn check(&self) -> Result<(), Error> {
        if Instant::now() >= self.deadline {
            Err(Error::Deadline)
        } else {
            Ok(())
        }
    }
}
impl History {
    /// Exclusively create a new external owned bare Git directory and persist native binding.
    /// The host must select a local non-synced path and reserve index/object/child costs in claim.
    pub fn init(
        repo: &Path,
        home: &Path,
        text_root: &str,
        owner: OwnerContext,
        claim: &Claim,
    ) -> Result<Self, Error> {
        validate_owner(&owner)?;
        validate_root(text_root)?;
        let call = Call::new(claim)?;
        let home = selected_directory(home)?;
        let parent = selected_directory(repo.parent().ok_or(Error::Invalid)?)?;
        let leaf = repo.file_name().ok_or(Error::Invalid)?;
        let repo = parent.join(leaf);
        if repo.starts_with(&home) || home.starts_with(&repo) {
            return Err(Error::Invalid);
        }
        fs::create_dir(&repo)?;
        let repo = selected_directory(&repo)?;
        let binding = Binding {
            version: 1,
            repo: identity(&repo)?,
            home: identity(&home)?,
            text_root: text_root.to_owned(),
            name: owner.name.clone(),
            email: owner.email.clone(),
            mode: owner.mode.clone(),
        };
        let history = Self {
            repo,
            home,
            binding,
            owner,
        };
        fs::create_dir(history.repo.join("omni-state"))?;
        fs::create_dir(history.repo.join("omni-state/operations"))?;
        let scratch = tempfile::tempdir_in(history.repo.join("omni-state"))?;
        // --template overrides all template configuration; this repo is newly owned and empty.
        run(
            &history.repo,
            scratch.path(),
            &["init", "--bare", "--object-format=sha1", "--template=", "."],
            b"",
            None,
            &call,
            None,
        )?;
        fs::write(history.repo.join("config"), CONFIG)?;
        File::open(history.repo.join("config"))?.sync_all()?;
        persist(
            &history.repo.join("omni-state/binding.json"),
            &history.binding,
        )?;
        File::open(&history.repo)?.sync_all()?;
        history.validate()?;
        Ok(history)
    }

    /// Reopen only an exactly bound app-owned repository; no imported/user config is consulted.
    pub fn open(
        repo: &Path,
        home: &Path,
        text_root: &str,
        owner: OwnerContext,
        claim: &Claim,
    ) -> Result<Self, Error> {
        validate_owner(&owner)?;
        validate_root(text_root)?;
        Call::new(claim)?;
        let repo = selected_directory(repo)?;
        let home = selected_directory(home)?;
        let binding: Binding = load(&repo.join("omni-state/binding.json"))?;
        if binding.version != 1
            || binding.text_root != text_root
            || binding.name != owner.name
            || binding.email != owner.email
            || binding.mode != owner.mode
        {
            return Err(Error::ContextMismatch);
        }
        let history = Self {
            repo,
            home,
            binding,
            owner,
        };
        history.validate()?;
        Ok(history)
    }

    /// Return the accepted immutable version without discovering another repository.
    pub fn head(&self, claim: &Claim) -> Result<Option<Oid>, Error> {
        let call = Call::new(claim)?;
        self.validate()?;
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        self.reference(ACCEPTED, scratch.path(), &call)
    }

    /// Prepare and retain a deterministic operation commit without changing home files or target refs.
    pub fn prepare(&self, request: &Request, claim: &Claim) -> Result<Prepared, Error> {
        let call = Call::new(claim)?;
        let _lock = self.lock()?;
        validate_id(&request.operation_id)?;
        let mut total = 0usize;
        for change in &request.changes {
            self.validate_path(&change.path)?;
            total = total
                .checked_add(change.path.len() + change.text.as_ref().map_or(0, String::len))
                .ok_or(Error::Capacity)?;
        }
        if total > call.limit || request.changes.len() as u64 > call.files {
            return Err(Error::Capacity);
        }
        call.account_bytes(total)?;
        call.account_files(request.changes.len() as u64)?;
        let mut request = request.clone();
        request.changes.sort_by(|a, b| a.path.cmp(&b.path));
        if request
            .changes
            .windows(2)
            .any(|pair| pair[0].path == pair[1].path)
        {
            return Err(Error::Invalid);
        }
        let target = self.target(&request.destination)?;
        let fingerprint = digest(&serde_json::to_vec(&request).map_err(|_| Error::Invalid)?);
        if let Some(intent) = self.existing(&request.operation_id, &fingerprint)? {
            return self.finish_prepare(intent, &call);
        }
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        let index = scratch.path().join("index");
        self.base_tree(request.base.as_ref(), scratch.path(), &call)?;
        if let Some(base) = &request.base {
            self.git(
                &["read-tree", base.as_str()],
                b"",
                scratch.path(),
                Some(&index),
                &call,
            )?;
        } else {
            self.git(
                &["read-tree", "--empty"],
                b"",
                scratch.path(),
                Some(&index),
                &call,
            )?;
        }
        let mut update = Vec::new();
        for change in &request.changes {
            let entry = if let Some(text) = &change.text {
                let oid = oid_output(self.git(
                    &["hash-object", "-w", "--stdin", "--no-filters"],
                    text.as_bytes(),
                    scratch.path(),
                    None,
                    &call,
                )?)?;
                format!("100644 {}\t{}\0", oid.as_str(), change.path)
            } else {
                format!("0 {}\t{}\0", "0".repeat(40), change.path)
            };
            update.extend_from_slice(entry.as_bytes());
        }
        self.git(
            &["update-index", "-z", "--index-info"],
            &update,
            scratch.path(),
            Some(&index),
            &call,
        )?;
        let tree =
            oid_output(self.git(&["write-tree"], b"", scratch.path(), Some(&index), &call)?)?;
        let comparison = self.compare_tree(request.base.as_ref(), &tree, scratch.path(), &call)?;
        let intent = self.intent(
            &request.operation_id,
            fingerprint,
            request.base.clone(),
            target,
            request.base.iter().cloned().collect(),
            tree,
            comparison.affected_paths,
        )?;
        self.retain_tree(&intent, scratch.path(), &call)?;
        persist(&self.intent_path(&request.operation_id), &intent)?;
        self.finish_prepare(intent, &call)
    }

    /// Prepare an exact owner-confirmed two-parent integration; storage must install its tree safely.
    pub fn prepare_confirmation(
        &self,
        operation_id: &str,
        confirmation: &Confirmation,
        claim: &Claim,
    ) -> Result<Prepared, Error> {
        validate_id(operation_id)?;
        validate_id(&confirmation.proposal_id)?;
        if self.owner.agent.is_some() {
            return Err(Error::ContextMismatch);
        }
        let call = Call::new(claim)?;
        let _lock = self.lock()?;
        let fingerprint =
            digest(&serde_json::to_vec(&(operation_id, confirmation)).map_err(|_| Error::Invalid)?);
        if let Some(intent) = self.existing(operation_id, &fingerprint)? {
            if matches!(
                self.inspect_intent(&intent, &call)?,
                State::Conflict(_) | State::Indeterminate(_)
            ) {
                return Err(Error::Conflict);
            }
            return self.finish_prepare(intent, &call);
        }
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        let proposal_ref = format!("refs/heads/omni-proposals/{}", confirmation.proposal_id);
        if self.reference(ACCEPTED, scratch.path(), &call)? != Some(confirmation.base.clone())
            || self.reference(&proposal_ref, scratch.path(), &call)?
                != Some(confirmation.candidate.clone())
        {
            return Err(Error::Conflict);
        }
        let tree = self.base_tree(Some(&confirmation.candidate), scratch.path(), &call)?;
        let comparison =
            self.compare_tree(Some(&confirmation.base), &tree, scratch.path(), &call)?;
        if comparison.diff_digest != confirmation.diff_digest {
            return Err(Error::Conflict);
        }
        let mut intent = self.intent(
            operation_id,
            fingerprint,
            Some(confirmation.base.clone()),
            ACCEPTED.to_owned(),
            vec![confirmation.base.clone(), confirmation.candidate.clone()],
            tree,
            comparison.affected_paths,
        )?;
        intent.proposal = Some(ConfirmedProposal {
            reference: proposal_ref,
            oid: confirmation.candidate.clone(),
        });
        self.retain_tree(&intent, scratch.path(), &call)?;
        persist(&self.intent_path(operation_id), &intent)?;
        self.finish_prepare(intent, &call)
    }

    /// Publish target plus immutable operation ref in ONE native expected-ref transaction.
    /// This is Git-only: callers retain independent file/journal recovery and never infer saved_local.
    pub fn publish(&self, operation_id: &str, claim: &Claim) -> Result<State, Error> {
        validate_id(operation_id)?;
        let call = Call::new(claim)?;
        let _lock = self.lock()?;
        let intent = self.read_intent(operation_id)?;
        let state = self.inspect_intent(&intent, &call)?;
        if !matches!(state, State::Prepared(_)) {
            return Ok(state);
        }
        let prepared = self.finish_prepare(intent.clone(), &call)?;
        let receipt = format!("refs/omni/operations/{operation_id}");
        let expected = intent
            .expected
            .as_ref()
            .map_or_else(|| "0".repeat(40), |o| o.0.clone());
        let mut input = String::from("start\n");
        if let Some(proposal) = &intent.proposal {
            input.push_str(&format!(
                "verify {} {}\n",
                proposal.reference,
                proposal.oid.as_str()
            ));
        }
        input.push_str(&if prepared.unchanged {
            format!(
                "verify {} {}\ncreate {} {}\nprepare\ncommit\n",
                intent.target,
                expected,
                receipt,
                prepared.tree.as_str()
            )
        } else {
            let candidate = prepared.candidate.as_ref().ok_or(Error::Recovery)?;
            format!(
                "update {} {} {}\ncreate {} {}\nprepare\ncommit\n",
                intent.target,
                candidate.as_str(),
                expected,
                receipt,
                candidate.as_str()
            )
        });
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        match self.git(
            &["update-ref", "--stdin", "--no-deref"],
            input.as_bytes(),
            scratch.path(),
            None,
            &call,
        ) {
            Ok(_) => match self.inspect_intent(&intent, &call) {
                Ok(state) => Ok(state),
                Err(_) => Ok(State::Indeterminate(prepared)),
            },
            Err(Error::Git) => match self.inspect_intent(&intent, &call) {
                Ok(State::Prepared(p)) => Ok(State::Indeterminate(p)),
                result => result,
            },
            // A deadline may follow actual ref effects; never blindly retry or assert rollback.
            Err(Error::Deadline) => Ok(State::Indeterminate(prepared)),
            Err(_) => Ok(State::Indeterminate(prepared)),
        }
    }

    /// Inspect actual immutable operation/target refs without replaying a commit or file write.
    pub fn inspect(&self, operation_id: &str, claim: &Claim) -> Result<State, Error> {
        validate_id(operation_id)?;
        let call = Call::new(claim)?;
        let _lock = self.lock()?;
        self.inspect_intent(&self.read_intent(operation_id)?, &call)
    }

    /// Compare complete bounded immutable versions; overflow refuses confirmation, never truncates.
    pub fn compare(
        &self,
        base: Option<&Oid>,
        candidate: &Oid,
        claim: &Claim,
    ) -> Result<Comparison, Error> {
        let call = Call::new(claim)?;
        self.validate()?;
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        let tree = self.base_tree(Some(candidate), scratch.path(), &call)?;
        self.compare_tree(base, &tree, scratch.path(), &call)
    }

    /// Read one selected immutable UTF-8 text; revision/path option injection is rejected first.
    pub fn read_text(&self, commit: &Oid, path: &str, claim: &Claim) -> Result<String, Error> {
        self.validate_path(path)?;
        let call = Call::new(claim)?;
        self.validate()?;
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        self.base_tree(Some(commit), scratch.path(), &call)?;
        let spec = format!("{}:{path}", commit.as_str());
        String::from_utf8(self.git(
            &["cat-file", "blob", &spec],
            b"",
            scratch.path(),
            None,
            &call,
        )?)
        .map_err(|_| Error::Recovery)
    }

    /// Recheck native root bindings and all configuration capable of redirecting Git object access.
    fn validate(&self) -> Result<(), Error> {
        selected_directory(&self.repo)?;
        selected_directory(&self.home)?;
        if identity(&self.repo)? != self.binding.repo
            || identity(&self.home)? != self.binding.home
            || self.repo.starts_with(&self.home)
            || self.home.starts_with(&self.repo)
        {
            return Err(Error::ContextMismatch);
        }
        if bounded_read(&self.repo.join("config"), CONTROL_LIMIT)? != CONFIG.as_bytes() {
            return Err(Error::Invalid);
        }
        for path in [
            "objects/info/alternates",
            "objects/info/http-alternates",
            "shallow",
            "commondir",
            "config.worktree",
        ] {
            if fs::symlink_metadata(self.repo.join(path)).is_ok() {
                return Err(Error::Invalid);
            }
        }
        for path in [
            "objects",
            "objects/info",
            "objects/pack",
            "refs",
            "omni-state",
        ] {
            selected_directory(&self.repo.join(path))?;
        }
        for path in [
            "refs/heads",
            "refs/heads/omni-proposals",
            "refs/omni",
            "refs/omni/prepared",
            "refs/omni/operations",
            "refs/omni/trees",
            "omni-state/operations",
        ] {
            let path = self.repo.join(path);
            if fs::symlink_metadata(&path).is_ok() {
                selected_directory(&path)?;
            }
        }
        Ok(())
    }

    /// Take only the owned cooperative lock; native Git CAS still handles outside ref writers.
    fn lock(&self) -> Result<WriterLock, Error> {
        self.validate()?;
        let path = self.repo.join("omni-state/writer.lock");
        if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
            return Err(Error::Invalid);
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        FileExt::try_lock_exclusive(&file).map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                Error::Busy
            } else {
                Error::Io
            }
        })?;
        Ok(WriterLock::new(file))
    }

    /// Resolve only the fixed accepted ref or validated immutable proposal namespace.
    fn target(&self, destination: &Destination) -> Result<String, Error> {
        match destination {
            Destination::Accepted if self.owner.agent.is_some() => Err(Error::ContextMismatch),
            Destination::Accepted => Ok(ACCEPTED.to_owned()),
            Destination::Proposal { proposal_id } => {
                validate_id(proposal_id)?;
                Ok(format!("refs/heads/omni-proposals/{proposal_id}"))
            }
        }
    }

    /// Validate text-only selected paths; no Git internals, originals, journal, or derived bytes.
    fn validate_path(&self, path: &str) -> Result<(), Error> {
        if path.len() > 4096
            || path
                .bytes()
                .any(|b| b < 32 || b == 127 || b == b'\\' || b == b':')
        {
            return Err(Error::Invalid);
        }
        let components: Vec<_> = path.split('/').collect();
        if components.iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || p.starts_with('-')
                || p.eq_ignore_ascii_case(".git")
                || p.eq_ignore_ascii_case(".omni")
        }) {
            return Err(Error::Invalid);
        }
        let selected = (components.len() > 1 && components[0] == self.binding.text_root)
            || (components.len() == 2
                && components[0] == "sources"
                && components[1].ends_with(".md"))
            || matches!(path, "omni-knowledge.json" | "AGENTS.md" | "schema.yaml");
        if !selected {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    /// Return a bounded validated immutable base tree, rejecting symlinks/submodules/nonselected paths.
    fn base_tree(&self, base: Option<&Oid>, scratch: &Path, call: &Call) -> Result<Oid, Error> {
        call.account_files(1)?;
        let Some(base) = base else {
            return oid_output(self.git(&["mktree"], b"", scratch, None, call)?);
        };
        let kind = self.git(&["cat-file", "-t", base.as_str()], b"", scratch, None, call)?;
        if kind != b"commit\n" {
            return Err(Error::Invalid);
        }
        let spec = format!("{}^{{tree}}", base.as_str());
        let tree =
            oid_output(self.git(&["rev-parse", "--verify", &spec], b"", scratch, None, call)?)?;
        let listing = self.git(
            &["ls-tree", "-r", "-z", tree.as_str()],
            b"",
            scratch,
            None,
            call,
        )?;
        let mut count = 0;
        for entry in listing.split(|b| *b == 0).filter(|e| !e.is_empty()) {
            count += 1;
            call.account_files(1)?;
            if count > call.files {
                return Err(Error::Capacity);
            }
            let entry = std::str::from_utf8(entry).map_err(|_| Error::Invalid)?;
            let (meta, path) = entry.split_once('\t').ok_or(Error::Recovery)?;
            if !meta.starts_with("100644 blob ") {
                return Err(Error::Invalid);
            }
            self.validate_path(path)?;
        }
        Ok(tree)
    }

    /// Build complete patch/path facts against a base tree without executing diff helpers.
    fn compare_tree(
        &self,
        base: Option<&Oid>,
        tree: &Oid,
        scratch: &Path,
        call: &Call,
    ) -> Result<Comparison, Error> {
        let base_tree = self.base_tree(base, scratch, call)?;
        let names = self.git(
            &[
                "diff-tree",
                "--no-commit-id",
                "-r",
                "--name-only",
                "-z",
                "--no-renames",
                base_tree.as_str(),
                tree.as_str(),
                "--",
            ],
            b"",
            scratch,
            None,
            call,
        )?;
        let mut affected_paths = Vec::new();
        for name in names.split(|b| *b == 0).filter(|s| !s.is_empty()) {
            let path = std::str::from_utf8(name)
                .map_err(|_| Error::Recovery)?
                .to_owned();
            self.validate_path(&path)?;
            affected_paths.push(path);
        }
        if affected_paths.len() as u64 > call.files {
            return Err(Error::Capacity);
        }
        let patch = self.git(
            &[
                "diff-tree",
                "--no-commit-id",
                "-r",
                "-p",
                "--binary",
                "--full-index",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--no-color",
                "--no-indent-heuristic",
                base_tree.as_str(),
                tree.as_str(),
                "--",
            ],
            b"",
            scratch,
            None,
            call,
        )?;
        Ok(Comparison {
            diff_digest: digest(&patch),
            affected_paths,
            patch,
        })
    }

    /// Construct one immutable recipe with a persisted timestamp and exact destination expectation.
    #[allow(clippy::too_many_arguments)]
    fn intent(
        &self,
        operation_id: &str,
        fingerprint: String,
        base: Option<Oid>,
        target: String,
        parents: Vec<Oid>,
        tree: Oid,
        affected_paths: Vec<String>,
    ) -> Result<Intent, Error> {
        let unchanged = affected_paths.is_empty();
        let expected = if target == ACCEPTED {
            base.clone()
        } else {
            None
        };
        Ok(Intent {
            version: 1,
            fingerprint,
            context: self.context_digest()?,
            expected,
            target,
            proposal: None,
            parents,
            prepared: Prepared {
                operation_id: operation_id.to_owned(),
                candidate: if unchanged { base } else { None },
                tree,
                affected_paths,
                unchanged,
            },
            seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Io)?
                .as_secs(),
        })
    }

    /// Look up normalized duplicate intent before any new object construction.
    fn existing(&self, operation_id: &str, fingerprint: &str) -> Result<Option<Intent>, Error> {
        if !self.intent_path(operation_id).try_exists()? {
            return Ok(None);
        }
        let intent = self.read_intent(operation_id)?;
        if intent.fingerprint != fingerprint {
            return Err(Error::OperationMismatch);
        }
        Ok(Some(intent))
    }

    /// Validate an owned receipt before using any durable ref/path as a Git argument.
    fn read_intent(&self, operation_id: &str) -> Result<Intent, Error> {
        let intent: Intent = load(&self.intent_path(operation_id))?;
        if intent.context != self.context_digest()? {
            return Err(Error::ContextMismatch);
        }
        if intent.version != 1
            || intent.prepared.operation_id != operation_id
            || intent.parents.len() > 2
            || !valid_digest(&intent.fingerprint)
            || intent.prepared.unchanged != intent.prepared.affected_paths.is_empty()
        {
            return Err(Error::Recovery);
        }
        if intent.target != ACCEPTED {
            let id = intent
                .target
                .strip_prefix("refs/heads/omni-proposals/")
                .ok_or(Error::Recovery)?;
            validate_id(id)?;
            if intent.expected.is_some() {
                return Err(Error::Recovery);
            }
        } else if self.owner.agent.is_some() {
            return Err(Error::ContextMismatch);
        }
        if let Some(proposal) = &intent.proposal {
            let id = proposal
                .reference
                .strip_prefix("refs/heads/omni-proposals/")
                .ok_or(Error::Recovery)?;
            validate_id(id)?;
            if intent.target != ACCEPTED
                || intent.parents.len() != 2
                || intent.parents[1] != proposal.oid
            {
                return Err(Error::Recovery);
            }
        }
        for path in &intent.prepared.affected_paths {
            self.validate_path(path)?;
        }
        Ok(intent)
    }

    /// Pin even unsigned/signing-blocked candidate bytes before durable intent publication.
    fn retain_tree(&self, intent: &Intent, scratch: &Path, call: &Call) -> Result<(), Error> {
        let name = format!("refs/omni/trees/{}", intent.prepared.operation_id);
        if let Some(previous) = self.reference(&name, scratch, call)? {
            if previous != intent.prepared.tree {
                return Err(Error::Recovery);
            }
        } else {
            self.git(
                &[
                    "update-ref",
                    "--no-deref",
                    &name,
                    intent.prepared.tree.as_str(),
                    &"0".repeat(40),
                ],
                b"",
                scratch,
                None,
                call,
            )?;
        }
        Ok(())
    }

    /// Deterministically materialize and pin a recipe; the same recipe never yields a new commit.
    fn finish_prepare(&self, mut intent: Intent, call: &Call) -> Result<Prepared, Error> {
        if intent.prepared.unchanged {
            return Ok(intent.prepared);
        }
        if self.owner.mode == Mode::SigningRequired {
            return Err(Error::SigningRequired);
        }
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        let mut args = vec!["commit-tree".to_owned(), intent.prepared.tree.0.clone()];
        for parent in &intent.parents {
            args.push("-p".to_owned());
            args.push(parent.0.clone());
        }
        let message = self.commit_message(&intent);
        let refs: Vec<_> = args.iter().map(String::as_str).collect();
        self.validate()?;
        let oid = oid_output(run(
            &self.repo,
            scratch.path(),
            &refs,
            message.as_bytes(),
            None,
            call,
            Some((&self.owner, intent.seconds)),
        )?)?;
        let prepared_ref = format!("refs/omni/prepared/{}", intent.prepared.operation_id);
        match self.reference(&prepared_ref, scratch.path(), call)? {
            Some(previous) if previous != oid => return Err(Error::Recovery),
            Some(_) => (),
            None => {
                self.git(
                    &[
                        "update-ref",
                        "--no-deref",
                        &prepared_ref,
                        oid.as_str(),
                        &"0".repeat(40),
                    ],
                    b"",
                    scratch.path(),
                    None,
                    call,
                )?;
            }
        }
        intent.prepared.candidate = Some(oid);
        Ok(intent.prepared)
    }

    /// Construct only fixed operation/context trailers, never submitted text or raw arguments.
    fn commit_message(&self, intent: &Intent) -> String {
        let mut message = format!(
            "Omni text operation\n\nOmni-Operation-Id: {}\nOmni-Intent-Digest: {}\n",
            intent.prepared.operation_id, intent.fingerprint
        );
        for (label, value) in [
            ("Omni-Agent", &self.owner.agent),
            ("Omni-Tool", &self.owner.tool),
            ("Omni-Session-Id", &self.owner.session),
        ] {
            if let Some(value) = value {
                message.push_str(&format!("{label}: {value}\n"));
            }
        }
        message
    }

    /// Verify actual immutable candidate bytes against all persisted recipe and owner facts.
    fn verify_candidate(
        &self,
        intent: &Intent,
        candidate: &Oid,
        scratch: &Path,
        call: &Call,
    ) -> Result<(), Error> {
        if self.owner.mode != Mode::UnsignedExplicit {
            return Err(Error::Recovery);
        }
        let actual = self.git(
            &["cat-file", "commit", candidate.as_str()],
            b"",
            scratch,
            None,
            call,
        )?;
        let mut expected = format!("tree {}\n", intent.prepared.tree.as_str());
        for parent in &intent.parents {
            expected.push_str(&format!("parent {}\n", parent.as_str()));
        }
        expected.push_str(&format!(
            "author {} <{}> {} +0000\ncommitter {} <{}> {} +0000\n\n",
            self.owner.name,
            self.owner.email,
            intent.seconds,
            self.owner.name,
            self.owner.email,
            intent.seconds
        ));
        expected.push_str(&self.commit_message(intent));
        if actual != expected.as_bytes() {
            return Err(Error::Recovery);
        }
        Ok(())
    }

    /// Check an unpublished integration's exact proposal; legacy missing expectations refuse replay.
    fn proposal_matches(
        &self,
        intent: &Intent,
        scratch: &Path,
        call: &Call,
    ) -> Result<bool, Error> {
        match &intent.proposal {
            Some(proposal) => {
                Ok(self.reference(&proposal.reference, scratch, call)?
                    == Some(proposal.oid.clone()))
            }
            None => Ok(intent.parents.len() < 2),
        }
    }

    /// Derive publication from actual immutable refs, never an optimistic receipt status bit.
    fn inspect_intent(&self, intent: &Intent, call: &Call) -> Result<State, Error> {
        let mut prepared = intent.prepared.clone();
        let scratch = tempfile::tempdir_in(self.repo.join("omni-state"))?;
        let candidate = self.reference(
            &format!("refs/omni/prepared/{}", prepared.operation_id),
            scratch.path(),
            call,
        )?;
        let operation = self.reference(
            &format!("refs/omni/operations/{}", prepared.operation_id),
            scratch.path(),
            call,
        )?;
        let target = self.reference(&intent.target, scratch.path(), call)?;
        if self.reference(
            &format!("refs/omni/trees/{}", prepared.operation_id),
            scratch.path(),
            call,
        )? != Some(prepared.tree.clone())
        {
            return Err(Error::Recovery);
        }
        if let Some(candidate) = &candidate {
            self.verify_candidate(intent, candidate, scratch.path(), call)?;
        }
        if prepared.unchanged {
            if let Some(operation) = operation {
                return Ok(if operation == prepared.tree {
                    State::Unchanged(prepared)
                } else {
                    State::Indeterminate(prepared)
                });
            }
            return Ok(
                if target == intent.expected
                    && self.proposal_matches(intent, scratch.path(), call)?
                {
                    State::Prepared(prepared)
                } else {
                    State::Conflict(prepared)
                },
            );
        }
        prepared.candidate = candidate.clone();
        if let Some(operation) = operation {
            let consistent = Some(&operation) == candidate.as_ref()
                && match &target {
                    Some(target) if target == &operation => true,
                    Some(target) => self
                        .git(
                            &[
                                "merge-base",
                                "--is-ancestor",
                                operation.as_str(),
                                target.as_str(),
                            ],
                            b"",
                            scratch.path(),
                            None,
                            call,
                        )
                        .is_ok(),
                    None => false,
                };
            return Ok(if consistent {
                State::Published(prepared)
            } else {
                State::Indeterminate(prepared)
            });
        }
        if let (Some(target), Some(candidate)) = (&target, &candidate)
            && (target == candidate
                || self
                    .git(
                        &[
                            "merge-base",
                            "--is-ancestor",
                            candidate.as_str(),
                            target.as_str(),
                        ],
                        b"",
                        scratch.path(),
                        None,
                        call,
                    )
                    .is_ok())
        {
            return Ok(State::Indeterminate(prepared));
        }
        if target != intent.expected || !self.proposal_matches(intent, scratch.path(), call)? {
            return Ok(State::Conflict(prepared));
        }
        Ok(State::Prepared(prepared))
    }

    /// Read one exact ref, refusing symbolic redirection in the owned transaction namespace.
    fn reference(&self, name: &str, scratch: &Path, call: &Call) -> Result<Option<Oid>, Error> {
        // for-each-ref returns empty for absent refs but exposes symbolic targets explicitly.
        let output = self.git(
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname) %(symref)",
                name,
            ],
            b"",
            scratch,
            None,
            call,
        )?;
        let text = std::str::from_utf8(&output).map_err(|_| Error::Recovery)?;
        if text.is_empty() {
            return Ok(None);
        }
        let mut result = None;
        for line in text.lines() {
            let parts: Vec<_> = line.split(' ').collect();
            if parts.len() != 3 || parts[0] != name || !parts[2].is_empty() || result.is_some() {
                return Err(Error::Recovery);
            }
            result = Some(Oid::parse(parts[1])?);
        }
        Ok(result)
    }

    /// Execute only fixed local plumbing with inert selected repository configuration.
    fn git(
        &self,
        args: &[&str],
        input: &[u8],
        scratch: &Path,
        index: Option<&Path>,
        call: &Call,
    ) -> Result<Vec<u8>, Error> {
        self.validate()?;
        run(&self.repo, scratch, args, input, index, call, None)
    }

    /// Return a validated operation-specific durable intent filename.
    fn intent_path(&self, operation_id: &str) -> PathBuf {
        self.repo
            .join("omni-state/operations")
            .join(format!("{operation_id}.json"))
    }

    /// Bind retry attribution to the exact authoritative service context without persisting raw fields.
    fn context_digest(&self) -> Result<String, Error> {
        Ok(digest(
            &serde_json::to_vec(&self.owner).map_err(|_| Error::Invalid)?,
        ))
    }
}

/// Validate only service-selected human/context fields before putting them in Git environment/messages.
fn validate_owner(owner: &OwnerContext) -> Result<(), Error> {
    for field in [&owner.name, &owner.email].into_iter().chain(
        [&owner.agent, &owner.tool, &owner.session]
            .into_iter()
            .filter_map(Option::as_ref),
    ) {
        if field.is_empty()
            || field.len() > 256
            || field
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>'))
        {
            return Err(Error::Invalid);
        }
    }
    for field in [&owner.name, &owner.email] {
        // Git strips these identity-edge characters. Refuse rather than change selected authorship.
        let scrubbed = |c: char| {
            c.is_whitespace() || matches!(c, '.' | ',' | ':' | ';' | '<' | '>' | '\\' | '\"' | '\'')
        };
        if field.starts_with(scrubbed) || field.ends_with(scrubbed) {
            return Err(Error::Invalid);
        }
    }
    if owner.email.chars().any(char::is_whitespace) {
        return Err(Error::Invalid);
    }
    Ok(())
}
/// Accept a finite ASCII identifier, excluding all ref, path and option syntax.
fn validate_id(id: &str) -> Result<(), Error> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || !id.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
/// Preserve the owner-selected pages/wiki mapping without adding a new authored tree.
fn validate_root(root: &str) -> Result<(), Error> {
    if matches!(root, "pages" | "wiki") {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}
/// Require a selected native directory whose spelling has no symlink components.
fn selected_directory(path: &Path) -> Result<PathBuf, Error> {
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    let canonical = path.canonicalize()?;
    if canonical != path {
        return Err(Error::Invalid);
    }
    identity(path)?;
    Ok(canonical)
}
/// Capture directory identity without following a leaf symlink.
fn identity(path: &Path) -> Result<Identity, Error> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err(Error::Invalid);
    }
    Ok(Identity {
        device: m.dev(),
        inode: m.ino(),
    })
}
/// Hash complete bounded bytes using lowercase SHA-256.
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Check durable digest spelling before trusting a recovery recipe.
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// Parse one exact native Git object response.
fn oid_output(bytes: Vec<u8>) -> Result<Oid, Error> {
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::Recovery)?;
    Oid::parse(text.strip_suffix('\n').ok_or(Error::Recovery)?)
}
/// Read a finite owned regular file; never follow a symlink or return an unbounded recovery body.
fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(Error::Invalid);
    }
    if meta.len() > limit as u64 {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::Capacity);
    }
    Ok(bytes)
}
/// Decode owned bounded recovery state, rejecting unsupported or malformed facts.
fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, Error> {
    serde_json::from_slice(&bounded_read(path, CONTROL_LIMIT)?).map_err(|_| Error::Recovery)
}
/// Durably install one immutable receipt exclusively; never overwrite another operation's state.
fn persist<T: Serialize>(path: &Path, value: &T) -> Result<(), Error> {
    let parent = path.parent().ok_or(Error::Invalid)?;
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Recovery)?;
    if bytes.len() > CONTROL_LIMIT {
        return Err(Error::Capacity);
    }
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path).map_err(|_| Error::Io)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

/// Execute a finite local plumbing child without pipes, shell, imported config, helpers or network.
/// File-backed I/O avoids pipe deadlock; polling enforces deadline/output bounds and collects this child.
fn run(
    repo: &Path,
    scratch: &Path,
    args: &[&str],
    input: &[u8],
    index: Option<&Path>,
    call: &Call,
    author: Option<(&OwnerContext, u64)>,
) -> Result<Vec<u8>, Error> {
    call.check()?;
    if input.len() > call.limit {
        return Err(Error::Capacity);
    }
    call.account_bytes(input.len())?;
    let output_limit = call.limit.min(call.bytes_left.get() as usize);
    let mut stdin = tempfile::tempfile_in(scratch)?;
    stdin.write_all(input)?;
    stdin.seek(SeekFrom::Start(0))?;
    let mut stdout = tempfile::tempfile_in(scratch)?;
    let stderr = tempfile::tempfile_in(scratch)?;
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("TMPDIR", scratch)
        .env("GIT_DIR", repo)
        .args([
            "--no-pager",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "credential.helper=",
            "-c",
            "core.askPass=",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "tag.gpgSign=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.fsync=all",
            "-c",
            "core.fsyncMethod=fsync",
        ])
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?));
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    if let Some((owner, seconds)) = author {
        let date = format!("@{seconds} +0000");
        command
            .env("GIT_AUTHOR_NAME", &owner.name)
            .env("GIT_AUTHOR_EMAIL", &owner.email)
            .env("GIT_COMMITTER_NAME", &owner.name)
            .env("GIT_COMMITTER_EMAIL", &owner.email)
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date);
    }
    let mut child = command.spawn()?;
    let status = loop {
        let failure = if call.check().is_err() {
            Some(Error::Deadline)
        } else {
            match (stdout.metadata(), stderr.metadata()) {
                (Ok(out), Ok(err))
                    if out.len() <= output_limit as u64 && err.len() <= output_limit as u64 =>
                {
                    None
                }
                (Ok(_), Ok(_)) => Some(Error::Capacity),
                _ => Some(Error::Io),
            }
        };
        if let Some(error) = failure {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Io);
            }
        }
    };
    call.account_bytes(stderr.metadata()?.len() as usize)?;
    if !status.success() {
        return Err(Error::Git);
    }
    stdout.seek(SeekFrom::Start(0))?;
    let mut output = Vec::new();
    stdout
        .take(output_limit as u64 + 1)
        .read_to_end(&mut output)?;
    call.account_bytes(output.len())?;
    if output.len() > output_limit {
        return Err(Error::Capacity);
    }
    Ok(output)
}
