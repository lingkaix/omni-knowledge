//! Confined read adapter for one explicitly selected ordinary local root.
//!
//! The adapter implements [`MountBackend`](knowledge_core::backend::MountBackend) with
//! descriptor-relative no-follow metadata, listing, and full-digest range reads.
//! It does not discover libraries, detect cloud or NAS residency, follow symlinks,
//! write, watch, or authorize a caller. Read the crate guide before integrating it.
//! Linux and macOS share this source. macOS execution is not claimed from a Linux build.
#![cfg(any(target_os = "linux", target_os = "macos"))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod native;

use knowledge_core::backend::{
    Availability, BackendError, BackendIdentity, Budget, ByteRange, Capabilities, Cursor,
    EntryKind, Enumeration, EnumerationStop, ExpectedVersion, FileIdEvidence, Freshness,
    ListedEntry, MountBackend, Progress, RangeRead, Residency, StatObservation, StatSnapshot,
    Unavailable,
};
use knowledge_core::data::{BackendKind, ContentDigest, RelativePath};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Accounting reservation for one retained listing observation, excluding copied strings.
/// This is not a measured RSS value.
const ENTRY_RESERVE: u64 = 256;
/// Accounting reservation for one held directory frame, excluding its relative prefix.
/// This is not a file-descriptor or RSS ceiling.
const FRAME_RESERVE: u64 = 512;
/// Largest hash buffer. A tighter memory budget uses a shorter buffer instead.
const MAX_CHUNK: usize = 8192;

/// Caller premise for the selected directory. This adapter does not detect locality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalityPremise {
    /// The caller has already established that this selected directory is an ordinary local root.
    OrdinaryLocal,
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
        })
    }

    fn backend_identity(&self) -> BackendIdentity {
        BackendIdentity {
            library_id: self.library_id.clone(),
            mount_id: self.mount_id.clone(),
            backend: BackendKind::Local,
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
        reject_cursor(&self.backend_identity(), cursor)?;
        let root = self.root()?;
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id)? {
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
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id)? {
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
            Ok(observed) => observation_from(self, path, observed),
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
        if expected_version.snapshot.backend != self.backend_identity() {
            return Err(BackendError::InvalidRequest);
        }
        let root = self.root()?;
        let started = Instant::now();
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id)? {
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
        let snapshot = snapshot_from(self, path, &observed);
        if snapshot != expected_version.snapshot {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        let file_bytes = observed.size;
        let charged = (path.as_str().len() as u64)
            .checked_add(file_bytes)
            .ok_or(BackendError::BudgetExhausted)?;
        if charged > budget.max_bytes || budget.max_files == 0 {
            return Err(BackendError::BudgetExhausted);
        }
        if range.offset() > file_bytes {
            // The range is outside the file, but a matching stat can still hide a byte change.
            // Hash before distinguishing an invalid range from drift.
            let (_bytes, digest) =
                hash_file(&parent, &name, &observed, range, budget, started, false)?;
            if live(self, &admission)? == Live::Cancelled {
                return Err(BackendError::Unavailable(Unavailable::Unknown));
            }
            if digest.as_str() != expected_version.digest.as_str() {
                return Err(BackendError::Unavailable(Unavailable::Changed));
            }
            return Err(BackendError::InvalidRequest);
        }
        let (bytes, digest) = hash_file(&parent, &name, &observed, range, budget, started, true)?;
        if digest.as_str() != expected_version.digest.as_str() {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        let after = native::observe(&parent, &name).map_err(map_native)?;
        if snapshot_from(self, path, &after) != snapshot {
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

fn walk<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    root: &File,
    admission: &Admission,
    budget: Budget,
    started: Instant,
) -> Result<Enumeration, BackendError> {
    let mut stack = vec![Frame {
        iter: native::read_dir(root).map_err(map_native)?,
        prefix: String::new(),
        dir: duplicate_dir(root)?,
    }];
    let mut entries = Vec::new();
    let mut examined = 0u64;
    let mut bytes_read = 0u64;
    let mut memory = FRAME_RESERVE;
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
            FrameStep::Name { name, prefix } => {
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
                let relative = child_path(&prefix, text);
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
                let charge = observation_charge(
                    path.as_str(),
                    &observed,
                    &mount.library_id,
                    &mount.mount_id,
                );
                if memory.saturating_add(charge) > budget.max_memory_bytes {
                    return Ok(partial(
                        entries,
                        EnumerationStop::BudgetExhausted,
                        progress(examined, bytes_read, started),
                    ));
                }
                memory += charge;
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
                let stat = observation_from(mount, &path, observed.clone());
                if kind == EntryKind::Directory {
                    let frame_charge = FRAME_RESERVE.saturating_add(path.as_str().len() as u64);
                    if memory.saturating_add(frame_charge) > budget.max_memory_bytes {
                        entries.push(ListedEntry { path, kind, stat });
                        return Ok(partial(
                            entries,
                            EnumerationStop::BudgetExhausted,
                            progress(examined, bytes_read, started),
                        ));
                    }
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
                    memory += frame_charge;
                    entries.push(ListedEntry {
                        path: path.clone(),
                        kind,
                        stat,
                    });
                    stack.push(Frame {
                        iter: child_iter,
                        prefix: path.as_str().to_string(),
                        dir: child,
                    });
                } else {
                    entries.push(ListedEntry { path, kind, stat });
                }
            }
        }
    }
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
    match gate(&mount.policy, &mount.library_id, &mount.mount_id)? {
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
    Name { name: CString, prefix: String },
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
                let prefix = frame.prefix.clone();
                return FrameStep::Name { name, prefix };
            }
        }
    }
}

fn duplicate_dir(dir: &File) -> Result<File, BackendError> {
    native::open_directory(dir, ".").map_err(map_native)
}

struct Admission {
    revision: u64,
    ordinary: bool,
    outside: BTreeSet<String>,
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
            if admission.locality != LocalityPremise::OrdinaryLocal {
                return Ok(Gate::Refuse(BackendError::Unavailable(
                    Unavailable::Unknown,
                )));
            }
            Ok(Gate::Proceed(Admission {
                revision: admission.revision,
                ordinary: true,
                outside: admission
                    .outside_scope
                    .iter()
                    .map(|path| path.as_str().to_string())
                    .collect(),
            }))
        }
    }
}

fn same_admission(left: &Admission, right: &Admission) -> bool {
    left.revision == right.revision
        && left.ordinary == right.ordinary
        && left.outside == right.outside
}

fn outside(path: &str, scopes: &BTreeSet<String>) -> bool {
    scopes.iter().any(|scope| {
        path == scope || path.starts_with(scope) && path.as_bytes().get(scope.len()) == Some(&b'/')
    })
}

fn reject_cursor(identity: &BackendIdentity, cursor: Option<&Cursor>) -> Result<(), BackendError> {
    match cursor {
        None => Ok(()),
        Some(cursor) if &cursor.backend == identity => Err(BackendError::InvalidRequest),
        Some(_) => Err(BackendError::InvalidRequest),
    }
}

fn observation_from<P: LocalReadPolicy>(
    mount: &LocalMount<P>,
    path: &RelativePath,
    observed: native::Observed,
) -> StatObservation {
    if observed.kind == native::NodeKind::Unsupported {
        return StatObservation {
            availability: Availability::Unavailable(Unavailable::Unsupported),
            snapshot: None,
        };
    }
    StatObservation {
        availability: Availability::Available,
        snapshot: Some(snapshot_from(mount, path, &observed)),
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

fn observation_charge(
    path: &str,
    observed: &native::Observed,
    library_id: &str,
    mount_id: &Option<String>,
) -> u64 {
    let mut charge = ENTRY_RESERVE.saturating_add(path.len() as u64);
    charge = charge.saturating_add(library_id.len() as u64);
    charge = charge.saturating_add(mount_id.as_ref().map(|value| value.len()).unwrap_or(0) as u64);
    charge = charge.saturating_add(observed.inode.to_string().len() as u64);
    charge = charge.saturating_add(observed.device.to_string().len() as u64);
    if let Some(generation) = &observed.generation {
        charge = charge.saturating_add(generation.len() as u64);
    }
    charge
}

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
