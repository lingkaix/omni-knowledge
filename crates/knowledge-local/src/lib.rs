//! Confined read adapter for one explicitly selected ordinary local root.
//!
//! The adapter implements [`MountBackend`] with
//! descriptor-relative no-follow metadata, listing, and full-digest range reads.
//! It does not discover libraries, detect cloud or NAS residency, follow symlinks,
//! write or authorize a caller. Optional caller-driven Linux watch hints never establish versions/absence.
//! Read the crate guide before integrating it.
//! Linux and macOS share this source. macOS execution is not claimed from a Linux build.
#![cfg(any(target_os = "linux", target_os = "macos"))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod monitor;
mod native;
mod stream;
mod watch;
pub use monitor::{Monitor, MonitorPolicy, TrackingMode, TrackingStatus};
pub use watch::{HintState, HintWatch, NativeCursor, WatchError};

use knowledge_core::backend::{
    Availability, BackendError, BackendIdentity, Budget, ByteRange, Capabilities, Cursor,
    EntryKind, Enumeration, EnumerationStop, ExpectedVersion, FileIdEvidence, Freshness,
    ListedEntry, MountBackend, Progress, RangeRead, Residency, StatObservation, StatSnapshot,
    Unavailable,
};
use knowledge_core::data::{BackendKind, ContentDigest, RelativePath};
use sha2::{Digest, Sha256};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Fixed reservation for one retained observation struct.
/// Variable path and qualifier copies are added before those copies are allocated.
/// This is not a measured RSS value.
const ENTRY_RESERVE: u64 = 256;
/// Fixed reservation for one directory stream and its held descriptor.
/// Checked before the stream is constructed. The relative prefix is added separately.
/// This is not a file-descriptor or RSS ceiling, and it is not the kernel getdents buffer.
const FRAME_RESERVE: u64 = 512;
/// Hex characters retained for one SHA-256 digest. The raw digest is 32 bytes.
const DIGEST_TEXT: u64 = 64;
/// Largest hash buffer. A tighter memory budget uses a shorter buffer instead.
const MAX_CHUNK: usize = 8192;

/// Caller premise for the selected directory. This adapter does not detect locality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalityPremise {
    /// The caller has already established that this selected directory is an ordinary local root.
    OrdinaryLocal,
    /// The native owner explicitly selected an existing OS-mounted NAS root; credentials/mounting stay external.
    OsMountedNas,
    /// The caller has not established that premise. No content probe is performed.
    NotEstablished,
}

/// Current read admission supplied by the trusted caller for this call only.
/// Constructing this value does not admit a root, a path, a digest, or a cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentAdmission {
    /// Library identity the caller currently binds to this selected root.
    pub library_id: String,
    /// Mount identity. `None` is the caller's home-relative selection, not a discovered home.
    pub mount_id: Option<String>,
    /// Admission revision. Any change before delivery refuses the call.
    pub revision: u64,
    /// Explicit locality premise. A local path spelling is not evidence.
    pub locality: LocalityPremise,
    /// Exposure exclusions and caller-identified child-owned scopes, including descendants.
    /// This adapter does not discover either category.
    pub outside_scope: Vec<RelativePath>,
}

/// Per-call decision from the trusted caller. The adapter does not cache it as a grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadDecision {
    /// Read admission for this call, still subject to root and scope checks.
    Admit(CurrentAdmission),
    /// Current policy denies the read. This is not absence.
    Deny,
    /// The caller cancelled this call.
    Cancel,
    /// The caller cannot currently establish admission.
    Unestablished,
}

/// Trusted owner of current read admission for one already selected root.
/// Implement this only in the admitted local owner. A manifest, digest, or cursor is not an implementation.
pub trait LocalReadPolicy: Send + Sync {
    /// Return the admission that applies at this moment.
    fn decide(&self) -> ReadDecision;
}

/// Bind failure. Per-call denials use [`BackendError`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalError {
    /// The path is not an absolute directory that can be bound without following a symlink.
    InvalidRoot,
    /// The library or mount identity is empty or contains NUL.
    InvalidIdentity,
    /// Unclassified I/O while binding the root.
    Io,
}

impl std::fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRoot => "local root cannot be bound",
            Self::InvalidIdentity => "invalid local mount identity",
            Self::Io => "local root bind failed",
        })
    }
}

impl std::error::Error for LocalError {}

/// One explicitly selected ordinary local root and its live read-policy seam.
/// The constructor records the directory identity. It does not mint read permission.
pub struct LocalMount<P: LocalReadPolicy> {
    path: PathBuf,
    device: u64,
    inode: u64,
    held: File,
    library_id: String,
    mount_id: Option<String>,
    policy: P,
    /// Actual selected native backend class, never inferred from path or portable declarations.
    kind: BackendKind,
}

impl<P: LocalReadPolicy> LocalMount<P> {
    /// Bind `path` as the selected root for `library_id` and `mount_id`.
    /// `path` must already be the caller's known ordinary directory. Symlink components are rejected.
    /// `policy` is stored and consulted on each later call; this function does not call it.
    pub fn open(
        path: &Path,
        library_id: impl Into<String>,
        mount_id: Option<String>,
        policy: P,
    ) -> Result<Self, LocalError> {
        let library_id = library_id.into();
        if !valid_id(&library_id) || mount_id.as_ref().is_some_and(|mount| !valid_id(mount)) {
            return Err(LocalError::InvalidIdentity);
        }
        let bound = match native::bind(path) {
            Ok(bound) => bound,
            Err(native::NativeError::Os(rustix::io::Errno::IO)) => return Err(LocalError::Io),
            Err(_) => return Err(LocalError::InvalidRoot),
        };
        Ok(Self {
            path: path.to_path_buf(),
            device: bound.device,
            inode: bound.inode,
            held: bound.file,
            library_id,
            mount_id,
            policy,
            kind: BackendKind::Local,
        })
    }

    /// Retain the native owner's actual already selected ordinary-local directory.
    /// The pathname only qualifies its current namespace; it never substitutes a reopened root.
    /// The caller prepays retained descriptors/context and supplies the same live read owner.
    pub fn from_selected_directory(
        path: &Path,
        root: File,
        library_id: impl Into<String>,
        mount_id: Option<String>,
        policy: P,
    ) -> Result<Self, LocalError> {
        let library_id = library_id.into();
        if !valid_id(&library_id) || mount_id.as_ref().is_some_and(|mount| !valid_id(mount)) {
            return Err(LocalError::InvalidIdentity);
        }
        let (device, inode, kind) =
            native::identity_of(&root).map_err(|_| LocalError::InvalidRoot)?;
        if kind != native::NodeKind::Directory || !path.is_absolute() {
            return Err(LocalError::InvalidRoot);
        }
        let selected = Self {
            path: path.to_path_buf(),
            device,
            inode,
            held: root,
            library_id,
            mount_id,
            policy,
            kind: BackendKind::Local,
        };
        selected.root().map_err(|_| LocalError::InvalidRoot)?;
        Ok(selected)
    }
    /// Bind one explicitly selected existing OS-mounted NAS without mounting/authenticating or assuming reliable events.
    /// Every operation still requires OsMountedNas from the actual current native read owner.
    pub fn open_os_mount(
        path: &Path,
        library_id: impl Into<String>,
        mount_id: Option<String>,
        policy: P,
    ) -> Result<Self, LocalError> {
        let mut mount = Self::open(path, library_id, mount_id, policy)?;
        mount.kind = BackendKind::Nas;
        Ok(mount)
    }
    /// Hash a complete actually admitted regular source before a verified source/job key exists.
    /// The caller retains a SAME-account pre-verification holding through all input/result lifetimes.
    /// No placeholder/locality guess or fabricated expected digest is needed; drift/refusal stays typed.
    pub fn verify(
        &self,
        path: &RelativePath,
        budget: Budget,
        holding: &knowledge_core::admission::Holding,
    ) -> Result<ExpectedVersion, BackendError> {
        admitted_budget(holding, budget)?;
        let root = self.root()?;
        let started = Instant::now();
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Proceed(a) => a,
            Gate::Cancel => return Err(BackendError::Unavailable(Unavailable::Unknown)),
            Gate::Refuse(e) => return Err(e),
        };
        if outside(path.as_str(), &admission.outside) {
            return Err(BackendError::Unavailable(Unavailable::Denied));
        }
        if !metadata_fits(path.as_str(), budget) || expired(started, budget.max_millis) {
            return Err(BackendError::BudgetExhausted);
        }
        let (parent, name) = open_parent(&root, path)?;
        let observed = native::observe(&parent, &name).map_err(map_native)?;
        if observed.kind != native::NodeKind::File {
            return Err(kind_error(observed.kind));
        }
        let charged = (path.as_str().len() as u64)
            .checked_add(observed.size)
            .ok_or(BackendError::BudgetExhausted)?;
        if charged > budget.max_bytes || budget.max_files == 0 {
            return Err(BackendError::BudgetExhausted);
        }
        let meta = stat_charge(path.as_str(), &observed, &self.library_id, &self.mount_id)
            .and_then(|n| n.checked_add(DIGEST_TEXT))
            .ok_or(BackendError::BudgetExhausted)?;
        if meta >= budget.max_memory_bytes {
            return Err(BackendError::BudgetExhausted);
        }
        let snapshot = snapshot_from(self, path, &observed);
        let mut hash_budget = budget;
        hash_budget.max_memory_bytes -= meta;
        let (_, digest) = hash_file(
            &parent,
            &name,
            &observed,
            ByteRange::new(0, 1).map_err(|_| BackendError::InvalidRequest)?,
            hash_budget,
            started,
            false,
        )?;
        let after = native::observe(&parent, &name).map_err(map_native)?;
        if !same_content(&snapshot, &after) {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        if live(self, &admission)? == Live::Cancelled {
            return Err(BackendError::Unavailable(Unavailable::Unknown));
        }
        Ok(ExpectedVersion { digest, snapshot })
    }
    /// Compare an already held descriptor with current root-qualified metadata without reading its body.
    /// This supplies neither source identity nor byte-verification authority. The native caller retains
    /// its actual descriptor/current read grant and prepays the metadata observation before this call.
    pub fn check_held_snapshot(
        &self,
        file: &File,
        expected: &StatSnapshot,
        budget: Budget,
    ) -> Result<(), BackendError> {
        if !same_bound_backend(self, &expected.backend) {
            return Err(BackendError::InvalidRequest);
        }
        let root = self.root()?;
        let started = Instant::now();
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Proceed(admission) => admission,
            Gate::Cancel => return Err(BackendError::Unavailable(Unavailable::Unknown)),
            Gate::Refuse(error) => return Err(error),
        };
        if outside(expected.path.as_str(), &admission.outside) {
            return Err(BackendError::Unavailable(Unavailable::Denied));
        }
        if budget.max_files == 0
            || !metadata_fits(expected.path.as_str(), budget)
            || expired(started, budget.max_millis)
        {
            return Err(BackendError::BudgetExhausted);
        }
        let (parent, name) = open_parent(&root, &expected.path)?;
        let observed = native::observe(&parent, &name).map_err(map_native)?;
        let charge = stat_charge(
            expected.path.as_str(),
            &observed,
            &self.library_id,
            &self.mount_id,
        )
        .ok_or(BackendError::BudgetExhausted)?;
        if charge > budget.max_memory_bytes {
            return Err(BackendError::BudgetExhausted);
        }
        if observed.kind != native::NodeKind::File
            || snapshot_from(self, &expected.path, &observed) != *expected
            || !native::same_sample(file, &observed).map_err(map_native)?
        {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        if live(self, &admission)? == Live::Cancelled {
            return Err(BackendError::Unavailable(Unavailable::Unknown));
        }
        if expired(started, budget.max_millis) {
            return Err(BackendError::BudgetExhausted);
        }
        Ok(())
    }
    /// Inspect real link multiplicity under current admission before using file identity as rename evidence.
    /// Multiple/changed/unknown links cannot select one annotation identity among ambiguous paths.
    pub fn link_count(
        &self,
        path: &RelativePath,
        budget: Budget,
        holding: &knowledge_core::admission::Holding,
    ) -> Result<u64, BackendError> {
        use std::os::unix::fs::MetadataExt;
        admitted_budget(holding, budget)?;
        let before = self.stat(path, budget)?;
        let root = self.root()?;
        let (parent, name) = open_parent(&root, path)?;
        let file = native::open_regular(&parent, &name).map_err(map_native)?;
        let links = file.metadata().map_err(|_| BackendError::Io)?.nlink();
        if before != self.stat(path, budget)? {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        Ok(links)
    }
    /// Enumerate only after the actual process account has prepaid the declared working-memory ceiling.
    pub fn list_admitted(
        &self,
        budget: Budget,
        holding: &knowledge_core::admission::Holding,
    ) -> Result<Enumeration, BackendError> {
        admitted_budget(holding, budget)?;
        self.list(None, budget)
    }
    /// Check actual descriptor ancestry before placing generated state outside this selected source tree.
    /// This is a placement fact only; it grants neither cache ownership nor source read permission.
    pub fn contains_directory(
        &self,
        directory: &File,
        budget: Budget,
        holding: &knowledge_core::admission::Holding,
    ) -> Result<bool, BackendError> {
        admitted_budget(holding, budget)?;
        let _ = self.read_scope(budget, holding)?;
        let target = self.root_identity()?;
        let mut dir = directory.try_clone().map_err(|_| BackendError::Io)?;
        let started = Instant::now();
        loop {
            if expired(started, budget.max_millis) {
                return Err(BackendError::BudgetExhausted);
            }
            let (d, i, k) = native::identity_of(&dir).map_err(map_native)?;
            if k != native::NodeKind::Directory {
                return Err(BackendError::InvalidRequest);
            }
            if (d, i) == target {
                return Ok(true);
            }
            let parent = native::open_directory(&dir, "..").map_err(map_native)?;
            let (pd, pi, _) = native::identity_of(&parent).map_err(map_native)?;
            if (pd, pi) == (d, i) {
                return Ok(false);
            }
            dir = parent;
        }
    }
    /// Fingerprint actual current native read revision/exclusions for a scoped durable scan boundary.
    /// This correlation string is NOT a source digest, approval token or permission to skip source checks.
    pub fn read_scope(
        &self,
        budget: Budget,
        holding: &knowledge_core::admission::Holding,
    ) -> Result<String, BackendError> {
        admitted_budget(holding, budget)?;
        self.root()?;
        let a = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Proceed(a) => a,
            Gate::Cancel => return Err(BackendError::Unavailable(Unavailable::Unknown)),
            Gate::Refuse(e) => return Err(e),
        };
        let mut h = Sha256::new();
        h.update(a.revision.to_le_bytes());
        h.update([matches!(a.locality, LocalityPremise::OsMountedNas) as u8]);
        for p in &a.outside {
            h.update((p.as_str().len() as u64).to_le_bytes());
            h.update(p.as_str().as_bytes());
        }
        Ok(format!("{:x}", h.finalize()))
    }
    /// Measure owned qualifier-copy costs before a native consumer allocates its context projection.
    pub fn context_bytes(&self) -> Option<u64> {
        (self.library_id.len() as u64)
            .checked_add(self.mount_id.as_ref().map_or(0, |m| m.len() as u64))?
            .checked_add(128)
    }
    /// Return this selected adapter's data qualifier; actual root/read admission is still checked per call.
    pub fn context(&self) -> BackendIdentity {
        self.backend_identity()
    }
    fn backend_identity(&self) -> BackendIdentity {
        BackendIdentity {
            library_id: self.library_id.clone(),
            mount_id: self.mount_id.clone(),
            backend: self.kind,
        }
    }

    fn root(&self) -> Result<File, BackendError> {
        let bound = match native::bind(&self.path) {
            Ok(bound) => bound,
            Err(error) => return Err(map_native(error)),
        };
        let (held_device, held_inode, held_kind) =
            native::identity_of(&self.held).map_err(map_native)?;
        if held_kind != native::NodeKind::Directory
            || bound.device != self.device
            || bound.inode != self.inode
            || held_device != self.device
            || held_inode != self.inode
        {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        Ok(bound.file)
    }
}

impl<P: LocalReadPolicy> MountBackend for LocalMount<P> {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            range_read: true,
            stable_file_id: false,
            reliable_watch: false,
            conditional_write: false,
            residency: false,
        }
    }

    fn list(&self, cursor: Option<&Cursor>, budget: Budget) -> Result<Enumeration, BackendError> {
        reject_cursor(cursor)?;
        let root = self.root()?;
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Cancel => {
                return Ok(partial(
                    Vec::new(),
                    EnumerationStop::Cancelled,
                    progress(0, 0, Instant::now()),
                ));
            }
            Gate::Proceed(admission) => admission,
            Gate::Refuse(error) => return Err(error),
        };
        let started = Instant::now();
        if !budget_allows_start(budget) || expired(started, budget.max_millis) {
            return deliver_list(
                self,
                partial(
                    Vec::new(),
                    EnumerationStop::BudgetExhausted,
                    progress(0, 0, started),
                ),
                &admission,
            );
        }
        let outcome = walk(self, &root, &admission, budget, started)?;
        deliver_list(self, outcome, &admission)
    }

    fn stat(&self, path: &RelativePath, budget: Budget) -> Result<StatObservation, BackendError> {
        let root = self.root()?;
        let started = Instant::now();
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Proceed(admission) => admission,
            Gate::Cancel => return Err(BackendError::Unavailable(Unavailable::Unknown)),
            Gate::Refuse(error) => return Err(error),
        };
        if outside(path.as_str(), &admission.outside) {
            return Err(BackendError::Unavailable(Unavailable::Denied));
        }
        if !metadata_fits(path.as_str(), budget) || expired(started, budget.max_millis) {
            return Err(BackendError::BudgetExhausted);
        }
        let observation = match observe_path(&root, path) {
            Ok(observed) if observed.kind == native::NodeKind::Unsupported => StatObservation {
                availability: Availability::Unavailable(Unavailable::Unsupported),
                snapshot: None,
            },
            Ok(observed) => {
                let charge =
                    stat_charge(path.as_str(), &observed, &self.library_id, &self.mount_id)
                        .ok_or(BackendError::BudgetExhausted)?;
                if charge > budget.max_memory_bytes {
                    return Err(BackendError::BudgetExhausted);
                }
                observation_from(self, path, &observed)
            }
            Err(BackendError::Unavailable(
                reason @ (Unavailable::Deleted | Unavailable::Denied | Unavailable::Unsupported),
            )) => StatObservation {
                availability: Availability::Unavailable(reason),
                snapshot: None,
            },
            Err(error) => return Err(error),
        };
        if live(self, &admission)? == Live::Cancelled {
            return Err(BackendError::Unavailable(Unavailable::Unknown));
        }
        Ok(observation)
    }

    fn open_range(
        &self,
        expected_version: &ExpectedVersion,
        range: ByteRange,
        budget: Budget,
    ) -> Result<RangeRead, BackendError> {
        if !same_bound_backend(self, &expected_version.snapshot.backend) {
            return Err(BackendError::InvalidRequest);
        }
        let root = self.root()?;
        let started = Instant::now();
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Proceed(admission) => admission,
            Gate::Cancel => return Err(BackendError::Unavailable(Unavailable::Unknown)),
            Gate::Refuse(error) => return Err(error),
        };
        let path = &expected_version.snapshot.path;
        if outside(path.as_str(), &admission.outside) {
            return Err(BackendError::Unavailable(Unavailable::Denied));
        }
        if expired(started, budget.max_millis) || !metadata_fits(path.as_str(), budget) {
            return Err(BackendError::BudgetExhausted);
        }
        let (parent, name) = open_parent(&root, path)?;
        let observed = native::observe(&parent, &name).map_err(map_native)?;
        if observed.kind != native::NodeKind::File {
            return Err(kind_error(observed.kind));
        }
        let file_bytes = observed.size;
        let charged = (path.as_str().len() as u64)
            .checked_add(file_bytes)
            .ok_or(BackendError::BudgetExhausted)?;
        if charged > budget.max_bytes || budget.max_files == 0 {
            return Err(BackendError::BudgetExhausted);
        }
        let meta = stat_charge(path.as_str(), &observed, &self.library_id, &self.mount_id)
            .and_then(|charge| charge.checked_add(DIGEST_TEXT))
            .ok_or(BackendError::BudgetExhausted)?;
        if meta > budget.max_memory_bytes {
            return Err(BackendError::BudgetExhausted);
        }
        let mut hash_budget = budget;
        hash_budget.max_memory_bytes = budget.max_memory_bytes - meta;
        let snapshot = snapshot_from(self, path, &observed);
        if snapshot != expected_version.snapshot {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        if range.offset() > file_bytes {
            // The range is outside the file, but a matching stat can still hide a byte change.
            // Hash before distinguishing an invalid range from drift.
            let (_bytes, digest) = hash_file(
                &parent,
                &name,
                &observed,
                range,
                hash_budget,
                started,
                false,
            )?;
            if live(self, &admission)? == Live::Cancelled {
                return Err(BackendError::Unavailable(Unavailable::Unknown));
            }
            if digest.as_str() != expected_version.digest.as_str() {
                return Err(BackendError::Unavailable(Unavailable::Changed));
            }
            return Err(BackendError::InvalidRequest);
        }
        let (bytes, digest) =
            hash_file(&parent, &name, &observed, range, hash_budget, started, true)?;
        if digest.as_str() != expected_version.digest.as_str() {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        let after = native::observe(&parent, &name).map_err(map_native)?;
        if !same_content(&snapshot, &after) {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        if live(self, &admission)? == Live::Cancelled {
            return Err(BackendError::Unavailable(Unavailable::Unknown));
        }
        Ok(RangeRead {
            version: ExpectedVersion { digest, snapshot },
            range,
            end_of_file: range.offset() + bytes.len() as u64 == file_bytes,
            bytes,
            progress: progress(1, charged, started),
        })
    }
}

fn hash_file(
    parent: &File,
    name: &CStr,
    observed: &native::Observed,
    range: ByteRange,
    budget: Budget,
    started: Instant,
    retain: bool,
) -> Result<(Vec<u8>, ContentDigest), BackendError> {
    let state = size_of::<Sha256>() as u64;
    if budget.max_memory_bytes <= state {
        return Err(BackendError::BudgetExhausted);
    }
    let budget = Budget {
        max_memory_bytes: budget.max_memory_bytes - state,
        ..budget
    };
    let file_bytes = observed.size;
    let keep = if !retain || range.offset() > file_bytes {
        0
    } else {
        (file_bytes - range.offset()).min(range.length())
    };
    let keep_usize = usize::try_from(keep).map_err(|_| BackendError::BudgetExhausted)?;
    let request_usize =
        usize::try_from(range.length()).map_err(|_| BackendError::BudgetExhausted)?;
    if range.length() > budget.max_memory_bytes || range.length() > budget.max_bytes {
        return Err(BackendError::BudgetExhausted);
    }
    let memory_usize = usize::try_from(budget.max_memory_bytes).unwrap_or(usize::MAX);
    let room = memory_usize.saturating_sub(request_usize);
    if room == 0 {
        return Err(BackendError::BudgetExhausted);
    }
    let chunk_len = room.min(MAX_CHUNK);
    let mut retained = vec![0u8; keep_usize];
    let mut buffer = vec![0u8; chunk_len];
    let file = native::open_regular(parent, name).map_err(map_native)?;
    let mut hasher = Sha256::new();
    let mut position = 0u64;
    while position < file_bytes {
        if expired(started, budget.max_millis) {
            return Err(BackendError::BudgetExhausted);
        }
        let want = usize::try_from((file_bytes - position).min(buffer.len() as u64))
            .map_err(|_| BackendError::Io)?;
        let read = native::read_at(&file, &mut buffer[..want], position).map_err(map_native)?;
        if read == 0 {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        hasher.update(&buffer[..read]);
        if keep_usize > 0 {
            copy_overlap(
                &mut retained,
                &buffer[..read],
                position,
                range.offset(),
                keep,
            );
        }
        position = position.checked_add(read as u64).ok_or(BackendError::Io)?;
    }
    // Scratch ends before the digest text. The digest uses the metadata reservation.
    drop(buffer);
    if position != file_bytes || !native::same_sample(&file, observed).map_err(map_native)? {
        return Err(BackendError::Unavailable(Unavailable::Changed));
    }
    let digest = hex_digest(hasher.finalize())?;
    Ok((retained, digest))
}

fn copy_overlap(retained: &mut [u8], just_read: &[u8], position: u64, range_off: u64, keep: u64) {
    let read_end = position + just_read.len() as u64;
    let range_end = range_off + keep;
    if position >= range_end || read_end <= range_off {
        return;
    }
    let start = position.max(range_off);
    let end = read_end.min(range_end);
    let src = (start - position) as usize;
    let dst = (start - range_off) as usize;
    let len = (end - start) as usize;
    retained[dst..dst + len].copy_from_slice(&just_read[src..src + len]);
}

/// Enumerate current admitted entries with borrowed-name path admission before any owned path copy.
/// Partial results retain accepted entries; frame/entry accounting covers every coexisting path spelling.
fn walk<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    root: &File,
    admission: &Admission,
    budget: Budget,
    started: Instant,
) -> Result<Enumeration, BackendError> {
    let Some(mut memory) = try_reserve(0, FRAME_RESERVE, budget.max_memory_bytes) else {
        return Ok(partial(
            Vec::new(),
            EnumerationStop::BudgetExhausted,
            progress(0, 0, started),
        ));
    };
    let mut stack = vec![Frame {
        iter: native::read_dir(root).map_err(map_native)?,
        prefix: String::new(),
        dir: duplicate_dir(root)?,
    }];
    let mut entries = Vec::new();
    let mut examined = 0u64;
    let mut bytes_read = 0u64;
    loop {
        if expired(started, budget.max_millis) {
            return Ok(partial(
                entries,
                EnumerationStop::BudgetExhausted,
                progress(examined, bytes_read, started),
            ));
        }
        match next_name(&mut stack) {
            FrameStep::Done => {
                return Ok(Enumeration::Complete {
                    entries,
                    progress: progress(examined, bytes_read, started),
                });
            }
            FrameStep::Error(error) => {
                return stop_or_err(entries, examined, bytes_read, started, error);
            }
            FrameStep::Name { name } => {
                let prefix = stack
                    .last()
                    .map(|frame| frame.prefix.as_str())
                    .ok_or(BackendError::Io)?;
                if examined >= budget.max_files {
                    return Ok(partial(
                        entries,
                        EnumerationStop::BudgetExhausted,
                        progress(examined, bytes_read, started),
                    ));
                }
                let raw = name.to_bytes();
                let next_bytes = bytes_read.saturating_add(raw.len() as u64);
                if next_bytes > budget.max_bytes {
                    return Ok(partial(
                        entries,
                        EnumerationStop::BudgetExhausted,
                        progress(examined, bytes_read, started),
                    ));
                }
                bytes_read = next_bytes;
                examined += 1;
                let Ok(text) = name.to_str() else {
                    return Ok(partial(
                        entries,
                        EnumerationStop::Unsupported,
                        progress(examined, bytes_read, started),
                    ));
                };
                // Even excluded/unsupported entries must not allocate their temporary path first.
                if child_path_len(prefix, text)
                    .and_then(|bytes| try_reserve(memory, bytes, budget.max_memory_bytes))
                    .is_none()
                {
                    return Ok(partial(
                        entries,
                        EnumerationStop::BudgetExhausted,
                        progress(examined, bytes_read, started),
                    ));
                }
                let relative = child_path(prefix, text);
                let Ok(path) = RelativePath::new(relative) else {
                    return Ok(partial(
                        entries,
                        EnumerationStop::Unsupported,
                        progress(examined, bytes_read, started),
                    ));
                };
                if outside(path.as_str(), &admission.outside) {
                    continue;
                }
                let observed = {
                    let parent = &stack.last().ok_or(BackendError::Io)?.dir;
                    match native::observe(parent, &name) {
                        Ok(observed) => observed,
                        Err(error) => {
                            return stop_or_err(entries, examined, bytes_read, started, error);
                        }
                    }
                };
                if observed.kind == native::NodeKind::Unsupported {
                    return Ok(partial(
                        entries,
                        EnumerationStop::Unsupported,
                        progress(examined, bytes_read, started),
                    ));
                }
                let Some(charge) =
                    list_entry_charge(path.as_str(), &observed, &mount.library_id, &mount.mount_id)
                else {
                    return Ok(partial(
                        entries,
                        EnumerationStop::BudgetExhausted,
                        progress(examined, bytes_read, started),
                    ));
                };
                let Some(next_memory) = try_reserve(memory, charge, budget.max_memory_bytes) else {
                    return Ok(partial(
                        entries,
                        EnumerationStop::BudgetExhausted,
                        progress(examined, bytes_read, started),
                    ));
                };
                memory = next_memory;
                let kind = match observed.kind {
                    native::NodeKind::Directory => EntryKind::Directory,
                    native::NodeKind::File => EntryKind::File,
                    native::NodeKind::Unsupported => {
                        return Ok(partial(
                            entries,
                            EnumerationStop::Unsupported,
                            progress(examined, bytes_read, started),
                        ));
                    }
                };
                let stat = observation_from(mount, &path, &observed);
                if kind == EntryKind::Directory {
                    let Some(frame_charge) =
                        (path.as_str().len() as u64).checked_add(FRAME_RESERVE)
                    else {
                        entries.push(ListedEntry { path, kind, stat });
                        return Ok(partial(
                            entries,
                            EnumerationStop::BudgetExhausted,
                            progress(examined, bytes_read, started),
                        ));
                    };
                    let Some(next_memory) =
                        try_reserve(memory, frame_charge, budget.max_memory_bytes)
                    else {
                        entries.push(ListedEntry { path, kind, stat });
                        return Ok(partial(
                            entries,
                            EnumerationStop::BudgetExhausted,
                            progress(examined, bytes_read, started),
                        ));
                    };
                    memory = next_memory;
                    let child = {
                        let parent = &stack.last().ok_or(BackendError::Io)?.dir;
                        match native::open_directory(parent, text) {
                            Ok(child) => child,
                            Err(error) => {
                                return stop_or_err(entries, examined, bytes_read, started, error);
                            }
                        }
                    };
                    let (device, inode, opened) =
                        native::identity_of(&child).map_err(map_native)?;
                    if opened != native::NodeKind::Directory
                        || device != observed.device
                        || inode != observed.inode
                    {
                        return Ok(partial(
                            entries,
                            EnumerationStop::Changed,
                            progress(examined, bytes_read, started),
                        ));
                    }
                    let child_iter = match native::read_dir(&child) {
                        Ok(iter) => iter,
                        Err(error) => {
                            entries.push(ListedEntry { path, kind, stat });
                            return stop_or_err(entries, examined, bytes_read, started, error);
                        }
                    };
                    // Two retained entry/snapshot paths plus this pre-reserved frame prefix.
                    // Move the original into the entry: never create a fourth simultaneous spelling.
                    let frame_prefix = path.as_str().to_owned();
                    entries.push(ListedEntry { path, kind, stat });
                    stack.push(Frame {
                        iter: child_iter,
                        prefix: frame_prefix,
                        dir: child,
                    });
                } else {
                    entries.push(ListedEntry { path, kind, stat });
                }
            }
        }
    }
}

/// Validate an actual unsettled process holding before allocating under a public backend budget.
fn admitted_budget(
    holding: &knowledge_core::admission::Holding,
    budget: Budget,
) -> Result<(), BackendError> {
    knowledge_core::admission::Account::current().map_err(|_| BackendError::InvalidRequest)?;
    let remaining = holding
        .remaining()
        .map_err(|_| BackendError::InvalidRequest)?;
    if budget.max_memory_bytes == 0 || budget.max_memory_bytes > remaining.memory_bytes {
        return Err(BackendError::BudgetExhausted);
    }
    Ok(())
}

fn deliver_list<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    enumeration: Enumeration,
    before: &Admission,
) -> Result<Enumeration, BackendError> {
    match live(mount, before)? {
        Live::Current => Ok(enumeration),
        Live::Cancelled => Ok(as_cancelled(enumeration)),
    }
}

fn as_cancelled(enumeration: Enumeration) -> Enumeration {
    let (entries, progress) = match enumeration {
        Enumeration::Complete { entries, progress }
        | Enumeration::Partial {
            entries, progress, ..
        } => (entries, progress),
    };
    partial(entries, EnumerationStop::Cancelled, progress)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Live {
    Current,
    Cancelled,
}

fn live<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    before: &Admission,
) -> Result<Live, BackendError> {
    let _ = mount.root()?;
    match gate(
        &mount.policy,
        &mount.library_id,
        &mount.mount_id,
        mount.kind,
    )? {
        Gate::Proceed(after) if same_admission(before, &after) => Ok(Live::Current),
        Gate::Proceed(_) => Err(BackendError::Unavailable(Unavailable::Changed)),
        Gate::Cancel => Ok(Live::Cancelled),
        Gate::Refuse(error) => Err(error),
    }
}

fn stop_or_err(
    entries: Vec<ListedEntry>,
    examined: u64,
    bytes_read: u64,
    started: Instant,
    error: native::NativeError,
) -> Result<Enumeration, BackendError> {
    let stopped = match error {
        native::NativeError::Unsupported => Some(EnumerationStop::Unsupported),
        native::NativeError::Changed => Some(EnumerationStop::Changed),
        native::NativeError::Os(errno)
            if errno == rustix::io::Errno::ACCESS || errno == rustix::io::Errno::PERM =>
        {
            Some(EnumerationStop::Denied)
        }
        native::NativeError::Os(errno) if errno == rustix::io::Errno::NOENT => {
            Some(EnumerationStop::Changed)
        }
        native::NativeError::Os(errno) if unsupported_errno(errno) => {
            Some(EnumerationStop::Unsupported)
        }
        native::NativeError::Os(errno) if offline(errno) => Some(EnumerationStop::Offline),
        native::NativeError::Os(_) => None,
    };
    match stopped {
        Some(stopped) => Ok(partial(
            entries,
            stopped,
            progress(examined, bytes_read, started),
        )),
        None => Err(map_native(error)),
    }
}

struct Frame {
    iter: rustix::fs::Dir,
    prefix: String,
    dir: File,
}

enum FrameStep {
    Done,
    Error(native::NativeError),
    Name { name: CString },
}

fn next_name(stack: &mut Vec<Frame>) -> FrameStep {
    loop {
        let Some(frame) = stack.last_mut() else {
            return FrameStep::Done;
        };
        match frame.iter.next() {
            None => {
                stack.pop();
            }
            Some(Err(error)) => return FrameStep::Error(native::NativeError::Os(error)),
            Some(Ok(entry)) => {
                let bytes = entry.file_name().to_bytes();
                if bytes == b"." || bytes == b".." {
                    continue;
                }
                let name = entry.file_name().to_owned();
                return FrameStep::Name { name };
            }
        }
    }
}

fn duplicate_dir(dir: &File) -> Result<File, BackendError> {
    native::open_directory(dir, ".").map_err(map_native)
}

struct Admission {
    revision: u64,
    locality: LocalityPremise,
    outside: Vec<RelativePath>,
}

enum Gate {
    Proceed(Admission),
    Cancel,
    Refuse(BackendError),
}

fn gate<P: LocalReadPolicy>(
    policy: &P,
    library_id: &str,
    mount_id: &Option<String>,
    kind: BackendKind,
) -> Result<Gate, BackendError> {
    match policy.decide() {
        ReadDecision::Cancel => Ok(Gate::Cancel),
        ReadDecision::Deny => Ok(Gate::Refuse(BackendError::Unavailable(Unavailable::Denied))),
        ReadDecision::Unestablished => Ok(Gate::Refuse(BackendError::Unavailable(
            Unavailable::Unknown,
        ))),
        ReadDecision::Admit(admission) => {
            if admission.library_id != library_id || admission.mount_id != *mount_id {
                return Ok(Gate::Refuse(BackendError::InvalidRequest));
            }
            if !valid_id(&admission.library_id)
                || admission
                    .mount_id
                    .as_ref()
                    .is_some_and(|mount| !valid_id(mount))
            {
                return Ok(Gate::Refuse(BackendError::InvalidRequest));
            }
            if !matches!(
                (kind, admission.locality),
                (BackendKind::Local, LocalityPremise::OrdinaryLocal)
                    | (BackendKind::Nas, LocalityPremise::OsMountedNas)
            ) {
                return Ok(Gate::Refuse(BackendError::Unavailable(
                    Unavailable::Unknown,
                )));
            }
            Ok(Gate::Proceed(Admission {
                revision: admission.revision,
                locality: admission.locality,
                outside: admission.outside_scope,
            }))
        }
    }
}

fn same_admission(left: &Admission, right: &Admission) -> bool {
    left.revision == right.revision
        && left.locality == right.locality
        && same_scopes(&left.outside, &right.outside)
}

fn same_scopes(left: &[RelativePath], right: &[RelativePath]) -> bool {
    left.iter()
        .all(|path| right.iter().any(|other| other.as_str() == path.as_str()))
        && right
            .iter()
            .all(|path| left.iter().any(|other| other.as_str() == path.as_str()))
}

fn outside(path: &str, scopes: &[RelativePath]) -> bool {
    scopes.iter().any(|scope| {
        let scope = scope.as_str();
        path == scope || path.starts_with(scope) && path.as_bytes().get(scope.len()) == Some(&b'/')
    })
}

fn reject_cursor(cursor: Option<&Cursor>) -> Result<(), BackendError> {
    if cursor.is_some() {
        Err(BackendError::InvalidRequest)
    } else {
        Ok(())
    }
}

fn same_bound_backend<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    backend: &BackendIdentity,
) -> bool {
    backend.backend == mount.kind
        && backend.library_id == mount.library_id
        && backend.mount_id == mount.mount_id
}

fn observation_from<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    path: &RelativePath,
    observed: &native::Observed,
) -> StatObservation {
    if observed.kind == native::NodeKind::Unsupported {
        return StatObservation {
            availability: Availability::Unavailable(Unavailable::Unsupported),
            snapshot: None,
        };
    }
    StatObservation {
        availability: Availability::Available,
        snapshot: Some(snapshot_from(mount, path, observed)),
    }
}

fn snapshot_from<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    path: &RelativePath,
    observed: &native::Observed,
) -> StatSnapshot {
    let backend = mount.backend_identity();
    StatSnapshot {
        file_id: file_evidence(&backend, observed),
        backend,
        path: path.clone(),
        size_bytes: observed.size,
        modified_ns: observed.modified_ns,
        changed_ns: observed.changed_ns,
        backend_version: None,
        residency: Residency::Unknown,
        freshness: Freshness::Current,
    }
}

fn file_evidence(backend: &BackendIdentity, observed: &native::Observed) -> Option<FileIdEvidence> {
    if observed.birth_ns.is_none() && observed.generation.is_none() {
        return None;
    }
    Some(FileIdEvidence {
        backend: backend.clone(),
        file_id: observed.inode.to_string(),
        volume_id: Some(observed.device.to_string()),
        generation: observed.generation.clone(),
        birth_ns: observed.birth_ns,
    })
}

fn observe_path(root: &File, path: &RelativePath) -> Result<native::Observed, BackendError> {
    let (parent, name) = open_parent(root, path)?;
    native::observe(&parent, &name).map_err(map_native)
}

fn open_parent(root: &File, path: &RelativePath) -> Result<(File, CString), BackendError> {
    let mut parts = path.as_str().split('/');
    let last = parts.next_back().ok_or(BackendError::InvalidRequest)?;
    let mut dir = duplicate_dir(root)?;
    for part in parts {
        let observed = native::observe(
            &dir,
            &CString::new(part).map_err(|_| BackendError::InvalidRequest)?,
        )
        .map_err(map_native)?;
        if observed.kind != native::NodeKind::Directory {
            return Err(kind_error(observed.kind));
        }
        let next = native::open_directory(&dir, part).map_err(map_native)?;
        let (device, inode, kind) = native::identity_of(&next).map_err(map_native)?;
        if kind != native::NodeKind::Directory
            || device != observed.device
            || inode != observed.inode
        {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        dir = next;
    }
    let name = CString::new(last).map_err(|_| BackendError::InvalidRequest)?;
    Ok((dir, name))
}

fn kind_error(kind: native::NodeKind) -> BackendError {
    match kind {
        native::NodeKind::Unsupported => BackendError::Unavailable(Unavailable::Unsupported),
        native::NodeKind::Directory | native::NodeKind::File => {
            BackendError::Unavailable(Unavailable::Unsupported)
        }
    }
}

fn list_entry_charge(
    path: &str,
    observed: &native::Observed,
    library_id: &str,
    mount_id: &Option<String>,
) -> Option<u64> {
    // The entry owns one path and the snapshot owns a second copy.
    stat_charge(path, observed, library_id, mount_id)?.checked_add(path.len() as u64)
}

fn stat_charge(
    path: &str,
    observed: &native::Observed,
    library_id: &str,
    mount_id: &Option<String>,
) -> Option<u64> {
    let qual = qualifier_len(library_id, mount_id)?;
    let mut charge = ENTRY_RESERVE
        .checked_add(path.len() as u64)?
        .checked_add(qual)?;
    if retains_file_id(observed) {
        charge = charge.checked_add(qual)?;
        charge = charge.checked_add(decimal_len(observed.inode))?;
        charge = charge.checked_add(decimal_len(observed.device))?;
        if let Some(generation) = &observed.generation {
            charge = charge.checked_add(generation.len() as u64)?;
        }
    }
    Some(charge)
}

fn qualifier_len(library_id: &str, mount_id: &Option<String>) -> Option<u64> {
    (library_id.len() as u64).checked_add(match mount_id {
        Some(mount) => mount.len() as u64,
        None => 0,
    })
}

fn retains_file_id(observed: &native::Observed) -> bool {
    observed.birth_ns.is_some() || observed.generation.is_some()
}

fn try_reserve(used: u64, charge: u64, ceiling: u64) -> Option<u64> {
    let next = used.checked_add(charge)?;
    (next <= ceiling).then_some(next)
}

fn same_content(snapshot: &StatSnapshot, observed: &native::Observed) -> bool {
    snapshot.size_bytes == observed.size
        && snapshot.modified_ns == observed.modified_ns
        && snapshot.changed_ns == observed.changed_ns
        && same_file_id(&snapshot.file_id, observed)
}

fn same_file_id(evidence: &Option<FileIdEvidence>, observed: &native::Observed) -> bool {
    if !retains_file_id(observed) {
        return evidence.is_none();
    }
    let Some(evidence) = evidence else {
        return false;
    };
    evidence.birth_ns == observed.birth_ns
        && evidence.generation == observed.generation
        && decimal_eq(&evidence.file_id, observed.inode)
        && evidence
            .volume_id
            .as_deref()
            .is_some_and(|text| decimal_eq(text, observed.device))
}

fn decimal_len(value: u64) -> u64 {
    match value.checked_ilog10() {
        Some(exp) => u64::from(exp) + 1,
        None => 1,
    }
}

fn decimal_eq(text: &str, value: u64) -> bool {
    let mut buf = [0u8; 20];
    let mut rest = value;
    let mut cursor = buf.len();
    loop {
        cursor -= 1;
        buf[cursor] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    text.as_bytes() == &buf[cursor..]
}

/// Measure the next UTF-8 path from borrowed components before allocation; overflow refuses admission.
fn child_path_len(prefix: &str, name: &str) -> Option<u64> {
    prefix
        .len()
        .checked_add(name.len())?
        .checked_add(usize::from(!prefix.is_empty()))
        .and_then(|bytes| u64::try_from(bytes).ok())
}

/// Construct an owned relative spelling only after child_path_len fits the current reservation.
fn child_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

fn partial(entries: Vec<ListedEntry>, stopped: EnumerationStop, progress: Progress) -> Enumeration {
    Enumeration::Partial {
        entries,
        stopped,
        next_cursor: None,
        progress,
    }
}

fn progress(examined: u64, bytes_read: u64, started: Instant) -> Progress {
    Progress {
        examined,
        bytes_read,
        elapsed_millis: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

fn budget_allows_start(budget: Budget) -> bool {
    budget.max_files > 0
        && budget.max_bytes > 0
        && budget.max_millis > 0
        && budget.max_memory_bytes > 0
}

fn metadata_fits(path: &str, budget: Budget) -> bool {
    budget.max_files > 0
        && budget.max_bytes >= path.len() as u64
        && budget.max_memory_bytes >= ENTRY_RESERVE.saturating_add(path.len() as u64)
        && budget.max_millis > 0
}

fn expired(started: Instant, max_millis: u64) -> bool {
    started.elapsed().as_millis() >= u128::from(max_millis)
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && !value.contains('\0')
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> Result<ContentDigest, BackendError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(64);
    for byte in bytes.as_ref() {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0xf) as usize] as char);
    }
    ContentDigest::new(text).map_err(|_| BackendError::Io)
}

fn map_native(error: native::NativeError) -> BackendError {
    match error {
        native::NativeError::Changed => BackendError::Unavailable(Unavailable::Changed),
        native::NativeError::Unsupported => BackendError::Unavailable(Unavailable::Unsupported),
        native::NativeError::Os(errno) => map_errno(errno),
    }
}

fn map_errno(errno: rustix::io::Errno) -> BackendError {
    if errno == rustix::io::Errno::ACCESS || errno == rustix::io::Errno::PERM {
        BackendError::Unavailable(Unavailable::Denied)
    } else if errno == rustix::io::Errno::NOENT {
        BackendError::Unavailable(Unavailable::Deleted)
    } else if unsupported_errno(errno) {
        BackendError::Unavailable(Unavailable::Unsupported)
    } else if offline(errno) {
        BackendError::Unavailable(Unavailable::Offline)
    } else {
        BackendError::Io
    }
}

fn unsupported_errno(errno: rustix::io::Errno) -> bool {
    errno == rustix::io::Errno::NOTDIR
        || errno == rustix::io::Errno::LOOP
        || errno == rustix::io::Errno::ISDIR
        || errno == rustix::io::Errno::NXIO
        || errno == rustix::io::Errno::NOTSUP
        || errno == rustix::io::Errno::NOSYS
}

fn offline(errno: rustix::io::Errno) -> bool {
    errno == rustix::io::Errno::STALE
        || errno == rustix::io::Errno::NETDOWN
        || errno == rustix::io::Errno::NETUNREACH
        || errno == rustix::io::Errno::HOSTDOWN
        || errno == rustix::io::Errno::NETRESET
        || errno == rustix::io::Errno::TIMEDOUT
        || linux_nonet(errno)
}

#[cfg(target_os = "linux")]
fn linux_nonet(errno: rustix::io::Errno) -> bool {
    errno == rustix::io::Errno::NONET
}

#[cfg(not(target_os = "linux"))]
fn linux_nonet(_errno: rustix::io::Errno) -> bool {
    false
}
