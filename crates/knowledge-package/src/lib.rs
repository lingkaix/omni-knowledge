//! Portable `omni.knowledge-package` ZIP capture, preview, and staged restore.
//!
//! Public integration, frozen for later CLI and MCP wiring:
//! - [`inspect`] validates an archive and verifies every inventoried payload.
//! - [`capture`] writes a private archive, validates it, then hard-links it without replacement.
//! - [`stage`] copies verified payloads into an admitted private directory and keeps the previous complete generation.
//! - [`apply`] calls [`SafeApply`] and [`GitTransactions`]. It does not write the destination or run Git.
//! - [`Work`] borrows one [`knowledge_core::admission::Holding`]. `grow` runs before a buffer or generated write, and `written` runs after the bytes are on the volume.
//!
//! Package identity and digests are data. They do not grant mounts, roots, or signer authority.
//! EPUB publication stays with the processing owner. This crate does not activate a durable library.
//! Index search stays with the retrieval kernel. Bundle verification stays with the history owner.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use archive::{Member, Out, hex, method_of, stream, write_zip};
use knowledge_core::admission::{Account, Decision, Error as AdmissionError, Holding, Origin};
use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::Reservation;
use manifest::operation_id;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod archive;
mod manifest;

/// Format identifier stored in `manifest.json`.
pub const FORMAT: &str = "omni.knowledge-package";

/// Storage no-clobber apply. `apply_text_change` is the storage owner operation.
///
/// `install_new` is the exclusive create for original bytes. Neither method may delete unrelated files.
pub trait SafeApply {
    /// Install authored text when `expected` still matches, or create it when `expected` is absent.
    fn apply_text_change(
        &mut self,
        operation_id: &str,
        path: &RelativePath,
        expected: Option<&ContentDigest>,
        candidate: &Path,
    ) -> Result<ApplyOutcome, Error>;
    /// Create one original file only when the destination name is absent.
    fn install_new(
        &mut self,
        operation_id: &str,
        path: &RelativePath,
        candidate: &Path,
    ) -> Result<ApplyOutcome, Error>;
}

/// History handoff. The Git owner verifies and imports an inert bundle.
///
/// The fixed history seam is `History::prepare`, `publish`, `inspect`, and
/// `prepare_confirmation` at `2c86db6`. That seam has no `git bundle verify`.
/// This crate does not invoke Git, hooks, or network operations.
pub trait GitTransactions {
    /// Record one staged bundle for a later history-owner admission.
    fn admit_inert_bundle(&mut self, operation_id: &str, bundle: &Path) -> Result<GitAdmit, Error>;
}

/// What the storage owner observed. This crate does not reinterpret it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// The destination now has the staged bytes.
    Saved,
    /// An existing version was left in place.
    Conflict,
    /// The outcome cannot be classified. Do not retry blindly.
    Indeterminate,
}

/// History-owner response for a bundle that this crate has not executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitAdmit {
    /// The bundle remains inert for the history owner.
    Inert,
    /// The history owner refused the bundle.
    Refused,
}

/// Fixed failures. Display text contains no path, archive name, or payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The archive or manifest shape is not usable.
    Invalid,
    /// A caller quota or Core budget was exceeded.
    Quota,
    /// Actual expansion exceeded the caller ratio.
    Ratio,
    /// Framing, names, links, encryption, overlap, or ZIP64 disagreed.
    Hostile,
    /// CRC32 of the actual output did not match the header.
    Checksum,
    /// Local and central headers, or declared and actual sizes, disagreed.
    Disagreement,
    /// A required range was shorter than declared.
    Truncated,
    /// The method, version, or required feature is not installed.
    Unsupported,
    /// The cooperative deadline elapsed.
    Deadline,
    /// The caller cancelled further work.
    Cancelled,
    /// The process account has no current room.
    Waiting,
    /// One request exceeds the whole call budget.
    TooLarge,
    /// A destination or previous generation was left unchanged.
    Conflict,
    /// A partial result must be inspected rather than replayed.
    Indeterminate,
    /// An earlier private generation did not reach `COMPLETE`.
    Interrupted,
    /// `require_complete` saw selected content without a digest.
    RequireComplete,
    /// Foreign v1 context cannot be claimed from this library.
    CompatibilityRefused,
    /// The name is a Git, cache, or credential path.
    Forbidden,
    /// Current source bytes do not match the expected digest.
    Drift,
    /// A filesystem operation failed.
    Io,
}

impl std::fmt::Display for Error {
    /// Emit a fixed classification.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "invalid package",
            Self::Quota => "package quota exceeded",
            Self::Ratio => "package compression ratio exceeded",
            Self::Hostile => "hostile package archive",
            Self::Checksum => "package checksum mismatch",
            Self::Disagreement => "package header disagreement",
            Self::Truncated => "truncated package archive",
            Self::Unsupported => "unsupported package feature",
            Self::Deadline => "package deadline elapsed",
            Self::Cancelled => "package cancelled",
            Self::Waiting => "package admission waiting",
            Self::TooLarge => "package admission exceeds the whole budget",
            Self::Conflict => "package destination conflict",
            Self::Indeterminate => "package result indeterminate",
            Self::Interrupted => "package staging interrupted",
            Self::RequireComplete => "package coverage incomplete",
            Self::CompatibilityRefused => "package compatibility refused",
            Self::Forbidden => "package path forbidden",
            Self::Drift => "package source drift",
            Self::Io => "package io failure",
        })
    }
}

impl std::error::Error for Error {}

/// Caller ceilings. Zero rejects the call. These are not a second memory pool.
#[derive(Clone, Copy, Debug)]
pub struct Quotas {
    /// Maximum logical archive length.
    pub max_archive_bytes: u64,
    /// Maximum central-directory entries.
    pub max_entries: u64,
    /// Maximum actual output of one entry.
    pub max_entry_expanded: u64,
    /// Maximum actual output of the archive.
    pub max_total_expanded: u64,
    /// Maximum `expanded / compressed` ratio, using a divisor of at least 1.
    pub max_ratio: u64,
    /// Maximum UTF-8 path length.
    pub max_path_bytes: usize,
    /// Maximum path segments.
    pub max_depth: usize,
}

/// Core work ceiling plus format quotas for one call.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    /// Existing Core per-call ceiling.
    pub budget: Budget,
    /// Format quotas selected by the caller.
    pub quotas: Quotas,
}

/// Borrowed holding, deadline, and running actual counters.
///
/// `holding` is a capability from [`knowledge_core::admission::Account::current`].
/// Dropping it releases nothing. This crate never calls `finish`; the host finishes
/// after returned buffers and staged files are dropped or settled.
pub struct Work<'a> {
    /// Call ceilings.
    pub bounds: &'a Bounds,
    /// Same-account capability. Growth is preallocation, not a second pool.
    pub holding: &'a mut Holding,
    /// Cooperative cancellation flag.
    pub cancel: &'a AtomicBool,
    /// Deadline derived from `budget.max_millis`.
    pub deadline: Instant,
    /// Bytes actually read.
    pub read_bytes: u64,
    /// Actual expanded payload bytes accepted so far.
    pub expanded: u64,
    /// Archive members actually opened.
    pub files: u64,
}

impl<'a> Work<'a> {
    /// Reject a zero ceiling and record the deadline.
    pub fn new(
        bounds: &'a Bounds,
        holding: &'a mut Holding,
        cancel: &'a AtomicBool,
    ) -> Result<Self, Error> {
        let quotas = bounds.quotas;
        if bounds.budget.max_files == 0
            || bounds.budget.max_bytes == 0
            || bounds.budget.max_millis == 0
            || bounds.budget.max_memory_bytes == 0
            || quotas.max_archive_bytes == 0
            || quotas.max_entries == 0
            || quotas.max_entry_expanded == 0
            || quotas.max_total_expanded == 0
            || quotas.max_ratio == 0
            || quotas.max_path_bytes == 0
            || quotas.max_depth == 0
        {
            return Err(Error::Quota);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(bounds.budget.max_millis))
            .ok_or(Error::Invalid)?;
        Ok(Self {
            bounds,
            holding,
            cancel,
            deadline,
            read_bytes: 0,
            expanded: 0,
            files: 0,
        })
    }

    /// Grow `holding` before the caller allocates or writes.
    ///
    /// Freed scratch stays charged until the host finishes the holding, so a dropped
    /// buffer cannot release another caller's bytes. Waiting leaves the holding unchanged.
    /// A need above the whole account is [`Error::TooLarge`].
    pub fn charge(&mut self, memory_bytes: u64, temporary_bytes: u64) -> Result<(), Error> {
        charge_fresh(self, memory_bytes, temporary_bytes)
    }
}

/// Native content preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Selected files opened read-only.
    Frozen,
    /// Selected files plus explicitly selected original bytes.
    Backup,
    /// Selected material for an explicit destination mapping.
    Migration,
}

/// ZIP method stored in the inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Uncompressed bytes.
    Stored,
    /// Deflate, used for authored text.
    Deflate,
}

/// Inventory role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Authored text.
    Core,
    /// Selected original bytes.
    Original,
    /// Prebuilt index bytes. They are not installed into the authored home.
    Index,
    /// Inert history bundle.
    History,
    /// Inert journal stream.
    Journal,
}

/// Digest coverage of an external reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigestState {
    /// The caller supplied a digest.
    Verified,
    /// No digest is claimed.
    Unverified,
    /// Hashing has not finished.
    Pending,
}

/// Whether the caller can currently read an external reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The caller reports the bytes as readable.
    Available,
    /// The caller reports the bytes as absent.
    Unavailable,
}

/// Index coverage recorded in the manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexCoverage {
    /// No index was selected.
    None,
    /// Index bytes may be present, and direct search is not wired.
    Omitted,
    /// Reserved for a later kernel-backed complete index. This crate never emits it.
    Complete,
}

/// How much of the archive was actually hashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integrity {
    /// Every inventoried payload matched its SHA-256.
    AllEntriesVerified,
}

/// Detached `manifest.sig` state. This crate does not report a signature as valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signature {
    /// No signature entry is present.
    Unsigned,
    /// Bytes are present and were not given to a signer policy.
    Unverified,
    /// The signature entry is empty or not an SSH signature block.
    Invalid,
}

/// Observation window supplied by the caller, `YYYY-MM-DDTHH:MM:SSZ`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    /// Inclusive observation start.
    pub start: String,
    /// Inclusive observation end.
    pub end: String,
}

/// Selected coverage. Counts are package facts, not permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coverage {
    /// Inventoried payloads.
    pub selected: u64,
    /// Caller-reported omitted sources.
    pub omitted: u64,
    /// External references without a digest.
    pub unverified: u64,
    /// External references still hashing.
    pub pending: u64,
    /// External references the caller marked unavailable.
    pub unavailable: u64,
    /// Index coverage.
    pub indexes: IndexCoverage,
}

/// One mount fact. Location is not a credential or a grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountFact {
    /// Stable mount id.
    pub mount_id: String,
    /// Single-component mount name.
    pub name: String,
    /// `local`, `nas`, `cloud_folder`, or `git`.
    pub backend: String,
    /// Opaque location text.
    pub location: String,
}

/// One inventoried payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryRecord {
    /// Confined relative path.
    pub path: RelativePath,
    /// Actual payload length.
    pub size_bytes: u64,
    /// SHA-256 of the actual payload.
    pub sha256: ContentDigest,
    /// Stored or deflate.
    pub method: Method,
    /// Inventory role.
    pub role: Role,
    /// Present for original bytes.
    pub mount_id: Option<String>,
    /// Present for original bytes.
    pub source_id: Option<String>,
}

/// External reference that was not copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalRef {
    /// Source identity.
    pub source_id: String,
    /// Mount identity.
    pub mount_id: String,
    /// Source-relative path.
    pub path: RelativePath,
    /// Digest coverage.
    pub digest_state: DigestState,
    /// Digest, when one exists.
    pub digest: Option<ContentDigest>,
    /// Caller-reported availability.
    pub availability: Availability,
    /// Known size, when one exists.
    pub size_bytes: Option<u64>,
}

/// Index paths kept in the archive and not extracted into the home.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexSet {
    /// Inventory paths of STORED index components.
    pub paths: Vec<RelativePath>,
}

/// History or journal pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobRef {
    /// Inventory path.
    pub path: RelativePath,
    /// Caller-selected scope token.
    pub scope: String,
}

/// Parsed manifest. `extra` preserves unknown top-level JSON values.
#[derive(Clone, Debug)]
pub struct Manifest {
    /// Format version. Only 1 is installed.
    pub version: u32,
    /// Library identity.
    pub library_id: String,
    /// Display name.
    pub name: String,
    /// `pages` or `wiki`.
    pub text_root: String,
    /// Content preset.
    pub preset: Preset,
    /// Features the destination must understand.
    pub required_features: Vec<String>,
    /// Mount facts.
    pub mounts: Vec<MountFact>,
    /// Sorted inventory.
    pub entries: Vec<EntryRecord>,
    /// Sorted external references.
    pub external: Vec<ExternalRef>,
    /// Optional index set.
    pub indexes: Option<IndexSet>,
    /// Optional history bundle.
    pub history: Option<BlobRef>,
    /// Optional journal.
    pub journal: Option<BlobRef>,
    /// Caller observation window.
    pub window: Window,
    /// Coverage counts.
    pub coverage: Coverage,
    /// Unknown top-level fields.
    pub extra: Map<String, Value>,
}

/// Verified archive.
#[derive(Clone, Debug)]
pub struct Preview {
    /// SHA-256 of the canonical manifest.
    pub snapshot_id: ContentDigest,
    /// Parsed manifest.
    pub manifest: Manifest,
    /// Paths that already exist at the destination, compared case-insensitively.
    pub conflicts: Vec<RelativePath>,
    /// Signature observation.
    pub signature: Signature,
    /// Payload verification.
    pub integrity: Integrity,
    /// False when required features are present. Installation must refuse.
    pub installable: bool,
    /// Sum of inventoried sizes.
    pub estimated_bytes: u64,
}

/// One selected input. The host already admitted the read.
pub struct Input<'a> {
    /// Destination inventory path.
    pub path: RelativePath,
    /// Role. Method is derived from the role.
    pub role: Role,
    /// Mount id for original bytes.
    pub mount_id: Option<String>,
    /// Source id for original bytes.
    pub source_id: Option<String>,
    /// Expected digest. A mismatch is drift.
    pub expected: Option<ContentDigest>,
    /// Exact source bytes.
    pub reader: &'a mut dyn Read,
}

/// Capture description.
pub struct Capture<'a> {
    /// Stable operation id, used as a private file name.
    pub operation_id: &'a str,
    /// Content preset.
    pub preset: Preset,
    /// Library id.
    pub library_id: &'a str,
    /// Display name.
    pub name: &'a str,
    /// `pages` or `wiki`.
    pub text_root: &'a str,
    /// Observation window.
    pub window: Window,
    /// Mount facts.
    pub mounts: Vec<MountFact>,
    /// External references.
    pub external: Vec<ExternalRef>,
    /// Sources the caller intentionally omitted.
    pub omitted: u64,
    /// Refuse completion while selected references are uncovered.
    pub require_complete: bool,
    /// Unknown names here refuse installation.
    pub required_features: Vec<String>,
    /// Admitted private directory.
    pub private_dir: &'a Path,
    /// Final archive path. An existing file is left unchanged.
    pub destination: &'a Path,
}

/// Completed or conflicting capture.
#[derive(Clone, Debug)]
pub struct Captured {
    /// Snapshot id of the validated private archive.
    pub snapshot_id: ContentDigest,
    /// Private complete archive.
    pub private_path: PathBuf,
    /// Destination path when this call created the link.
    pub installed: Option<PathBuf>,
    /// False when the destination already had a file and was left unchanged.
    pub linked: bool,
}

/// One staged payload.
#[derive(Clone, Debug)]
pub struct StagedFile {
    /// Inventory path.
    pub path: RelativePath,
    /// Role.
    pub role: Role,
    /// Actual digest.
    pub digest: ContentDigest,
    /// File inside the private generation.
    pub staged: PathBuf,
}

/// Complete private generation.
#[derive(Clone, Debug)]
pub struct Staged {
    /// Operation that produced the generation.
    pub operation_id: String,
    /// Snapshot id.
    pub snapshot_id: ContentDigest,
    /// Private generation directory.
    pub generation: PathBuf,
    /// Staged payloads.
    pub files: Vec<StagedFile>,
    /// Pre-existing destination paths.
    pub conflicts: Vec<RelativePath>,
    /// Inert history bundle, when selected.
    pub history: Option<PathBuf>,
    /// Inert journal, when selected.
    pub journal: Option<PathBuf>,
    /// False when required features refuse installation.
    pub installable: bool,
}

/// Result of calling the storage and history owners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// Overall outcome. Earlier saved paths are not rolled back.
    pub state: ApplyState,
    /// Paths the storage owner saved.
    pub applied: Vec<RelativePath>,
    /// Paths left unchanged.
    pub conflicts: Vec<RelativePath>,
}

/// Apply outcome after owner calls return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyState {
    /// Every selected home path was saved and history was inert.
    Completed,
    /// At least one path or the bundle was a conflict or refusal.
    Conflict,
    /// An owner returned an indeterminate result.
    Indeterminate,
    /// The private generation is not complete.
    Interrupted,
    /// Required features or a stale marker refused the call.
    Refused,
}

/// Foreign v1 preview. This crate does not invent owner, revision, or review facts.
#[derive(Clone, Debug)]
pub struct V1Plan {
    /// Always a refusal in this crate.
    pub state: V1State,
    /// Hashed regular files under the foreign root.
    pub files: Vec<V1File>,
    /// Whether the caller supplied host context.
    pub host_context: bool,
}

/// Foreign import state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V1State {
    /// Required foreign semantics were not claimed.
    CompatibilityRefused,
}

/// One hashed foreign file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V1File {
    /// Relative path.
    pub path: RelativePath,
    /// Actual digest.
    pub sha256: ContentDigest,
    /// Actual length.
    pub size: u64,
}

/// Host context required by current v1. Supplying it does not fabricate publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V1HostContext {
    /// Foreign owner token.
    pub owner: String,
    /// Foreign revision token.
    pub revision: String,
    /// Foreign profile token.
    pub profile: String,
}

/// Validate structure, manifest, and every inventoried payload.
pub fn inspect(
    archive: &mut File,
    work: &mut Work<'_>,
    existing: &[RelativePath],
) -> Result<Preview, Error> {
    let opened = open_verified(archive, work)?;
    let conflicts = conflicts_for(&opened.manifest.entries, existing);
    let estimated_bytes = opened
        .manifest
        .entries
        .iter()
        .try_fold(0u64, |sum, entry| {
            sum.checked_add(entry.size_bytes).ok_or(Error::Quota)
        })?;
    Ok(Preview {
        snapshot_id: opened.snapshot_id,
        manifest: opened.manifest,
        conflicts,
        signature: opened.signature,
        integrity: Integrity::AllEntriesVerified,
        installable: opened.installable,
        estimated_bytes,
    })
}

/// Stream selected inputs into a private archive and link it only if the destination is absent.
pub fn capture(
    capture: &mut Capture<'_>,
    inputs: &mut [Input<'_>],
    work: &mut Work<'_>,
) -> Result<Captured, Error> {
    operation_id(capture.operation_id)?;
    prepare_dir(capture.private_dir)?;
    let private_final = capture
        .private_dir
        .join(format!("{}.zip", capture.operation_id));
    let partial = capture
        .private_dir
        .join(format!("{}.partial", capture.operation_id));
    if private_final.exists() || partial.exists() {
        return Err(Error::Conflict);
    }
    let mut digested = Vec::new();
    for input in inputs.iter_mut() {
        if manifest::forbidden(input.path.as_str()) {
            return Err(Error::Forbidden);
        }
        let method = method_for(input.role)?;
        let bytes = read_source(input.reader, work)?;
        let digest = digest_bytes(&bytes)?;
        if input
            .expected
            .as_ref()
            .is_some_and(|expected| expected.as_str() != digest.as_str())
        {
            return Err(Error::Drift);
        }
        if input.role == Role::Original && (input.mount_id.is_none() || input.source_id.is_none()) {
            return Err(Error::Invalid);
        }
        digested.push(Digested {
            path: input.path.clone(),
            role: input.role,
            method,
            mount_id: input.mount_id.clone(),
            source_id: input.source_id.clone(),
            digest,
            bytes,
        });
    }
    digested.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));
    if digested.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(Error::Invalid);
    }
    let coverage = coverage_for(
        digested.len() as u64,
        capture.omitted,
        &capture.external,
        digested.iter().any(|item| item.role == Role::Index),
    );
    if capture.require_complete
        && (coverage.unverified > 0 || coverage.pending > 0 || coverage.unavailable > 0)
    {
        return Err(Error::RequireComplete);
    }
    let manifest = Manifest {
        version: 1,
        library_id: capture.library_id.to_owned(),
        name: capture.name.to_owned(),
        text_root: capture.text_root.to_owned(),
        preset: capture.preset,
        required_features: capture.required_features.clone(),
        mounts: capture.mounts.clone(),
        entries: digested
            .iter()
            .map(|item| EntryRecord {
                path: item.path.clone(),
                size_bytes: item.bytes.len() as u64,
                sha256: item.digest.clone(),
                method: item.method,
                role: item.role,
                mount_id: item.mount_id.clone(),
                source_id: item.source_id.clone(),
            })
            .collect(),
        external: {
            let mut external = capture.external.clone();
            external.sort_by(|left, right| {
                (left.source_id.as_str(), left.path.as_str())
                    .cmp(&(right.source_id.as_str(), right.path.as_str()))
            });
            external
        },
        indexes: index_set(&digested),
        history: blob_for(&digested, Role::History),
        journal: blob_for(&digested, Role::Journal),
        window: capture.window.clone(),
        coverage,
        extra: Map::new(),
    };
    let canonical = manifest::emit(&manifest)?;
    let mut parts: Vec<(String, Method, Vec<u8>)> = digested
        .into_iter()
        .map(|item| (item.path.as_str().to_owned(), item.method, item.bytes))
        .collect();
    parts.push(("manifest.json".into(), Method::Stored, canonical));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|_| Error::Io)?;
    let mut refs: Vec<(&str, Method, &[u8])> = Vec::new();
    for part in &parts {
        account_temporary(work, part.2.len() as u64)?;
        refs.push((part.0.as_str(), part.1, part.2.as_slice()));
    }
    let finished = write_zip(file, &refs)?;
    finished.sync_all().map_err(|_| Error::Io)?;
    for part in &parts {
        note_written(work, part.2.len() as u64)?;
    }
    drop(parts);
    let mut check = File::open(&partial).map_err(|_| Error::Io)?;
    let verified = open_verified(&mut check, work)?;
    fs::rename(&partial, &private_final).map_err(|_| Error::Io)?;
    if fs::symlink_metadata(capture.destination).is_ok() {
        return Ok(Captured {
            snapshot_id: verified.snapshot_id,
            private_path: private_final,
            installed: None,
            linked: false,
        });
    }
    match fs::hard_link(&private_final, capture.destination) {
        Ok(()) => Ok(Captured {
            snapshot_id: verified.snapshot_id,
            private_path: private_final,
            installed: Some(capture.destination.to_path_buf()),
            linked: true,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(Captured {
            snapshot_id: verified.snapshot_id,
            private_path: private_final,
            installed: None,
            linked: false,
        }),
        Err(_) => Err(Error::Indeterminate),
    }
}

/// Verify an archive into a new private generation. An incomplete generation for the same operation is left unchanged.
pub fn stage(
    archive: &mut File,
    work: &mut Work<'_>,
    private_dir: &Path,
    operation_id_value: &str,
    existing: &[RelativePath],
) -> Result<Staged, Error> {
    operation_id(operation_id_value)?;
    prepare_dir(private_dir)?;
    let generation = private_dir.join(operation_id_value);
    if generation.join("INCOMPLETE").exists() && !generation.join("COMPLETE").exists() {
        return Err(Error::Interrupted);
    }
    if generation.join("COMPLETE").exists() {
        return read_staged(&generation);
    }
    let opened = open_verified(archive, work)?;
    if !opened.installable {
        return Err(Error::Unsupported);
    }
    fs::create_dir(&generation).map_err(|_| Error::Io)?;
    lock_down(&generation)?;
    File::create(generation.join("INCOMPLETE")).map_err(|_| Error::Io)?;
    let mut files = Vec::new();
    for entry in &opened.manifest.entries {
        let member = opened
            .members
            .iter()
            .find(|member| member.name == entry.path.as_str())
            .ok_or(Error::Disagreement)?;
        let relative = staged_name(entry);
        let destination = generation.join(&relative);
        if let Some(parent) = destination.parent() {
            create_parents(&generation, parent)?;
        }
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(Error::Conflict);
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|_| Error::Io)?;
        stream(archive, member, work, &mut Out::Disk(&mut output))?;
        output.sync_all().map_err(|_| Error::Io)?;
        files.push(StagedFile {
            path: entry.path.clone(),
            role: entry.role,
            digest: entry.sha256.clone(),
            staged: destination,
        });
        if work.cancel.load(Ordering::Relaxed) {
            return Err(Error::Interrupted);
        }
    }
    let staged = Staged {
        operation_id: operation_id_value.to_owned(),
        snapshot_id: opened.snapshot_id,
        generation: generation.clone(),
        files,
        conflicts: conflicts_for(&opened.manifest.entries, existing),
        history: None,
        journal: None,
        installable: true,
    };
    let staged = attach_roles(staged);
    write_plan(&staged)?;
    File::create(generation.join("COMPLETE")).map_err(|_| Error::Io)?;
    fs::remove_file(generation.join("INCOMPLETE")).map_err(|_| Error::Io)?;
    Ok(staged)
}

/// Call storage and history owners. Paths inside `conflicts` are not sent to an owner.
pub fn apply(
    staged: &Staged,
    install: &mut dyn SafeApply,
    git: &mut dyn GitTransactions,
) -> Result<Applied, Error> {
    if !staged.generation.join("COMPLETE").is_file()
        || staged.generation.join("INCOMPLETE").exists()
    {
        return Ok(Applied {
            state: ApplyState::Interrupted,
            applied: Vec::new(),
            conflicts: staged.conflicts.clone(),
        });
    }
    if !staged.installable {
        return Ok(Applied {
            state: ApplyState::Refused,
            applied: Vec::new(),
            conflicts: staged.conflicts.clone(),
        });
    }
    let mut applied = Vec::new();
    let mut conflicts = staged.conflicts.clone();
    for file in &staged.files {
        if file.role == Role::Index || file.role == Role::Journal || file.role == Role::History {
            continue;
        }
        if conflicts
            .iter()
            .any(|path| path.as_str().to_lowercase() == file.path.as_str().to_lowercase())
        {
            continue;
        }
        let outcome = if file.role == Role::Core {
            install.apply_text_change(&staged.operation_id, &file.path, None, &file.staged)?
        } else {
            install.install_new(&staged.operation_id, &file.path, &file.staged)?
        };
        match outcome {
            ApplyOutcome::Saved => applied.push(file.path.clone()),
            ApplyOutcome::Conflict => {
                conflicts.push(file.path.clone());
                return Ok(Applied {
                    state: ApplyState::Conflict,
                    applied,
                    conflicts,
                });
            }
            ApplyOutcome::Indeterminate => {
                return Ok(Applied {
                    state: ApplyState::Indeterminate,
                    applied,
                    conflicts,
                });
            }
        }
    }
    if let Some(bundle) = &staged.history {
        match git.admit_inert_bundle(&staged.operation_id, bundle)? {
            GitAdmit::Inert => {}
            GitAdmit::Refused => {
                return Ok(Applied {
                    state: ApplyState::Conflict,
                    applied,
                    conflicts,
                });
            }
        }
    }
    let state = if conflicts.is_empty() {
        ApplyState::Completed
    } else {
        ApplyState::Conflict
    };
    Ok(Applied {
        state,
        applied,
        conflicts,
    })
}

/// Hash a foreign directory and refuse compatibility. Symlinks are not followed.
pub fn v1_import_plan(
    root: &Path,
    host: Option<&V1HostContext>,
    work: &mut Work<'_>,
) -> Result<V1Plan, Error> {
    if host.is_some_and(|context| {
        context.owner.is_empty() || context.revision.is_empty() || context.profile.is_empty()
    }) {
        return Err(Error::Invalid);
    }
    let mut manifest_file = File::open(root.join("manifest.json")).map_err(|_| Error::Invalid)?;
    let bytes = read_source(&mut manifest_file, work)?;
    if !std::str::from_utf8(&bytes)
        .unwrap_or("")
        .contains("openkit.knowledge-package")
    {
        return Err(Error::Invalid);
    }
    let _ = host;
    let mut files = Vec::new();
    walk_v1(root, root, work, &mut files, 0)?;
    files.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));
    Ok(V1Plan {
        state: V1State::CompatibilityRefused,
        files,
        host_context: host.is_some(),
    })
}

struct Digested {
    path: RelativePath,
    role: Role,
    method: Method,
    mount_id: Option<String>,
    source_id: Option<String>,
    digest: ContentDigest,
    bytes: Vec<u8>,
}

struct Opened {
    manifest: Manifest,
    snapshot_id: ContentDigest,
    signature: Signature,
    installable: bool,
    members: Vec<Member>,
}

fn open_verified(archive: &mut File, work: &mut Work<'_>) -> Result<Opened, Error> {
    let members = archive::list(archive, work)?;
    let mut canonical = None;
    let mut manifest_sha = None;
    let mut signature = Signature::Unsigned;
    let mut hashes = Vec::new();
    for member in &members {
        if member.directory {
            continue;
        }
        if member.name == "manifest.json" {
            let mut bytes = Vec::new();
            manifest_sha = Some(stream(archive, member, work, &mut Out::Keep(&mut bytes))?);
            canonical = Some(bytes);
        } else if member.name == "manifest.sig" {
            signature = signature_bytes(archive, member, work)?;
        } else {
            let digest = stream(archive, member, work, &mut Out::Drop)?;
            hashes.push((
                member.name.clone(),
                digest,
                method_of(member)?,
                member.uncompressed,
            ));
        }
    }
    let canonical = canonical.ok_or(Error::Invalid)?;
    let parsed = manifest::parse(&canonical)?;
    let snapshot = digest_bytes(&canonical)?;
    if snapshot.as_str() != manifest_sha.ok_or(Error::Invalid)? {
        return Err(Error::Checksum);
    }
    for entry in &parsed.entries {
        let found = hashes
            .iter()
            .find(|item| item.0 == entry.path.as_str())
            .ok_or(Error::Disagreement)?;
        if found.2 != entry.method || found.3 != entry.size_bytes {
            return Err(Error::Disagreement);
        }
        if found.1 != entry.sha256.as_str() {
            return Err(Error::Checksum);
        }
    }
    for (name, _, _, _) in &hashes {
        if !parsed
            .entries
            .iter()
            .any(|entry| entry.path.as_str() == name)
        {
            return Err(Error::Hostile);
        }
    }
    let installable =
        parsed.required_features.is_empty() && parsed.coverage.indexes != IndexCoverage::Complete;
    Ok(Opened {
        manifest: parsed,
        snapshot_id: snapshot,
        signature,
        installable,
        members,
    })
}

fn signature_bytes(
    archive: &mut File,
    member: &Member,
    work: &mut Work<'_>,
) -> Result<Signature, Error> {
    if member.uncompressed == 0 || member.uncompressed > 65_536 {
        return Ok(Signature::Invalid);
    }
    let mut bytes = Vec::new();
    stream(archive, member, work, &mut Out::Keep(&mut bytes))?;
    let text = String::from_utf8_lossy(&bytes);
    if text.contains("-----BEGIN SSH SIGNATURE-----") {
        Ok(Signature::Unverified)
    } else {
        Ok(Signature::Invalid)
    }
}

fn conflicts_for(entries: &[EntryRecord], existing: &[RelativePath]) -> Vec<RelativePath> {
    entries
        .iter()
        .filter(|entry| matches!(entry.role, Role::Core | Role::Original))
        .filter(|entry| {
            existing
                .iter()
                .any(|path| path.as_str().to_lowercase() == entry.path.as_str().to_lowercase())
        })
        .map(|entry| entry.path.clone())
        .collect()
}

fn role_token(role: Role) -> &'static str {
    match role {
        Role::Core => "core",
        Role::Original => "original",
        Role::Index => "index",
        Role::History => "history",
        Role::Journal => "journal",
    }
}

fn role_from(value: &str) -> Result<Role, Error> {
    match value {
        "core" => Ok(Role::Core),
        "original" => Ok(Role::Original),
        "index" => Ok(Role::Index),
        "history" => Ok(Role::History),
        "journal" => Ok(Role::Journal),
        _ => Err(Error::Indeterminate),
    }
}

fn method_for(role: Role) -> Result<Method, Error> {
    match role {
        Role::Core => Ok(Method::Deflate),
        Role::Original | Role::Index | Role::History | Role::Journal => Ok(Method::Stored),
    }
}

fn read_source(reader: &mut dyn Read, work: &mut Work<'_>) -> Result<Vec<u8>, Error> {
    let _scratch = preallocate(work, 8192)?;
    let mut bytes = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if work.cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        if Instant::now() >= work.deadline {
            return Err(Error::Deadline);
        }
        let n = reader.read(&mut buf).map_err(|_| Error::Io)?;
        if n == 0 {
            break;
        }
        if bytes.len() as u64 + n as u64 > work.bounds.quotas.max_entry_expanded {
            return Err(Error::Quota);
        }
        account_memory(work, n as u64)?;
        bytes.extend_from_slice(&buf[..n]);
        work.read_bytes = work.read_bytes.checked_add(n as u64).ok_or(Error::Quota)?;
        if work.read_bytes > work.bounds.budget.max_bytes {
            return Err(Error::Quota);
        }
    }
    Ok(bytes)
}

fn digest_bytes(bytes: &[u8]) -> Result<ContentDigest, Error> {
    ContentDigest::new(hex(&Sha256::digest(bytes))).map_err(|_| Error::Invalid)
}

fn coverage_for(
    selected: u64,
    omitted: u64,
    external: &[ExternalRef],
    has_index: bool,
) -> Coverage {
    Coverage {
        selected,
        omitted,
        unverified: external
            .iter()
            .filter(|item| item.digest.is_none() || item.digest_state == DigestState::Unverified)
            .count() as u64,
        pending: external
            .iter()
            .filter(|item| item.digest_state == DigestState::Pending)
            .count() as u64,
        unavailable: external
            .iter()
            .filter(|item| item.availability == Availability::Unavailable)
            .count() as u64,
        indexes: if has_index {
            IndexCoverage::Omitted
        } else {
            IndexCoverage::None
        },
    }
}

fn index_set(items: &[Digested]) -> Option<IndexSet> {
    let paths: Vec<_> = items
        .iter()
        .filter(|item| item.role == Role::Index)
        .map(|item| item.path.clone())
        .collect();
    if paths.is_empty() {
        None
    } else {
        Some(IndexSet { paths })
    }
}

fn blob_for(items: &[Digested], role: Role) -> Option<BlobRef> {
    items
        .iter()
        .find(|item| item.role == role)
        .map(|item| BlobRef {
            path: item.path.clone(),
            scope: "selected".into(),
        })
}

/// Scratch reservation on the same account. Finish runs when this value drops, after the buffer.
pub(crate) struct Scratch {
    holding: Option<Holding>,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(mut holding) = self.holding.take() {
            let _ = holding.finish();
        }
    }
}

/// Reserve `bytes` before a buffer that dies before the call returns.
///
/// The returned capability is not the caller's retained holding. Finishing it
/// does not release retained output or another job's charge.
pub(crate) fn preallocate(work: &Work<'_>, bytes: u64) -> Result<Scratch, Error> {
    if work.cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    if Instant::now() >= work.deadline {
        return Err(Error::Deadline);
    }
    if bytes > work.bounds.budget.max_memory_bytes {
        return Err(Error::TooLarge);
    }
    if bytes == 0 {
        return Ok(Scratch { holding: None });
    }
    let account = Account::current().map_err(admission_error)?;
    match account
        .reserve(
            Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .map_err(admission_error)?
    {
        Decision::Ready(holding) => Ok(Scratch {
            holding: Some(holding),
        }),
        Decision::Waiting(_) => Err(Error::Waiting),
    }
}

fn admission_error(error: AdmissionError) -> Error {
    match error {
        AdmissionError::TooLargeMemory | AdmissionError::TooLargeStorage => Error::TooLarge,
        _ => Error::Indeterminate,
    }
}

fn account_memory(work: &mut Work<'_>, bytes: u64) -> Result<(), Error> {
    charge_fresh(work, bytes, 0)
}

fn account_temporary(work: &mut Work<'_>, bytes: u64) -> Result<(), Error> {
    charge_fresh(work, 0, bytes)
}

pub(crate) fn charge_fresh(
    work: &mut Work<'_>,
    memory_bytes: u64,
    temporary_bytes: u64,
) -> Result<(), Error> {
    if memory_bytes > work.bounds.budget.max_memory_bytes
        || temporary_bytes > work.bounds.budget.max_bytes
    {
        return Err(Error::TooLarge);
    }
    if memory_bytes == 0 && temporary_bytes == 0 {
        return Ok(());
    }
    match work.holding.grow(Reservation {
        memory_bytes,
        temporary_bytes,
    }) {
        Ok(Decision::Ready(())) => Ok(()),
        Ok(Decision::Waiting(_)) => Err(Error::Waiting),
        Err(error) => Err(admission_error(error)),
    }
}

pub(crate) fn note_written(work: &mut Work<'_>, bytes: u64) -> Result<(), Error> {
    if bytes == 0 {
        return Ok(());
    }
    work.holding
        .written(bytes)
        .map(|_| ())
        .map_err(|_| Error::Indeterminate)
}

fn prepare_dir(path: &Path) -> Result<(), Error> {
    let meta = fs::symlink_metadata(path).map_err(|_| Error::Io)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(Error::Hostile);
    }
    Ok(())
}

fn lock_down(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| Error::Io)?;
    }
    let _ = path;
    Ok(())
}

fn create_parents(root: &Path, path: &Path) -> Result<(), Error> {
    if path == root {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        create_parents(root, parent)?;
    }
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(Error::Hostile),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(Error::Conflict),
        Err(_) => {
            fs::create_dir(path).map_err(|_| Error::Io)?;
            lock_down(path)
        }
    }
}

fn staged_name(entry: &EntryRecord) -> PathBuf {
    match entry.role {
        Role::Index => PathBuf::from("indexes").join(entry.path.as_str()),
        Role::History => PathBuf::from("history").join(entry.path.as_str()),
        Role::Journal => PathBuf::from("journal").join(entry.path.as_str()),
        Role::Core | Role::Original => PathBuf::from("files").join(entry.path.as_str()),
    }
}

fn attach_roles(mut staged: Staged) -> Staged {
    staged.history = staged
        .files
        .iter()
        .find(|file| file.role == Role::History)
        .map(|file| file.staged.clone());
    staged.journal = staged
        .files
        .iter()
        .find(|file| file.role == Role::Journal)
        .map(|file| file.staged.clone());
    staged
}

fn write_plan(staged: &Staged) -> Result<(), Error> {
    let mut lines = String::new();
    lines.push_str(staged.snapshot_id.as_str());
    lines.push('\n');
    for file in &staged.files {
        lines.push_str(file.path.as_str());
        lines.push('\n');
        lines.push_str(role_token(file.role));
        lines.push('\n');
        lines.push_str(file.digest.as_str());
        lines.push('\n');
    }
    let mut plan = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(staged.generation.join("PLAN"))
        .map_err(|_| Error::Io)?;
    plan.write_all(lines.as_bytes()).map_err(|_| Error::Io)?;
    plan.sync_all().map_err(|_| Error::Io)
}

fn read_staged(generation: &Path) -> Result<Staged, Error> {
    let text = fs::read_to_string(generation.join("PLAN")).map_err(|_| Error::Indeterminate)?;
    let mut lines = text.lines();
    let snapshot_id =
        ContentDigest::new(lines.next().unwrap_or("")).map_err(|_| Error::Indeterminate)?;
    let mut files = Vec::new();
    while let Some(path) = lines.next() {
        let role = role_from(lines.next().ok_or(Error::Indeterminate)?)?;
        let digest = lines.next().ok_or(Error::Indeterminate)?;
        let path = RelativePath::new(path).map_err(|_| Error::Indeterminate)?;
        let digest = ContentDigest::new(digest).map_err(|_| Error::Indeterminate)?;
        let staged_path = generation.join(staged_name(&EntryRecord {
            path: path.clone(),
            size_bytes: 0,
            sha256: digest.clone(),
            method: Method::Stored,
            role,
            mount_id: None,
            source_id: None,
        }));
        if !staged_path.is_file() {
            return Err(Error::Indeterminate);
        }
        files.push(StagedFile {
            path,
            role,
            digest,
            staged: staged_path,
        });
    }
    let operation_id = generation
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(Error::Invalid)?
        .to_owned();
    Ok(attach_roles(Staged {
        operation_id,
        snapshot_id,
        generation: generation.to_path_buf(),
        files,
        conflicts: Vec::new(),
        history: None,
        journal: None,
        installable: true,
    }))
}

fn walk_v1(
    root: &Path,
    dir: &Path,
    work: &mut Work<'_>,
    files: &mut Vec<V1File>,
    depth: usize,
) -> Result<(), Error> {
    if depth > work.bounds.quotas.max_depth {
        return Err(Error::Quota);
    }
    let entries = fs::read_dir(dir).map_err(|_| Error::Io)?;
    for entry in entries {
        let entry = entry.map_err(|_| Error::Io)?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| Error::Io)?;
        if meta.file_type().is_symlink() {
            return Err(Error::Hostile);
        }
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::Hostile)?;
        if name == ".git" || name == ".omni" {
            continue;
        }
        if meta.is_dir() {
            walk_v1(root, &path, work, files, depth + 1)?;
            continue;
        }
        if !meta.is_file() {
            return Err(Error::Hostile);
        }
        let relative = path.strip_prefix(root).map_err(|_| Error::Hostile)?;
        let text = relative.to_str().ok_or(Error::Hostile)?.replace('\\', "/");
        if text == "manifest.json" {
            continue;
        }
        let relative_path = manifest::path(&text)?;
        let mut file = File::open(&path).map_err(|_| Error::Io)?;
        let bytes = read_source(&mut file, work)?;
        files.push(V1File {
            path: relative_path,
            sha256: digest_bytes(&bytes)?,
            size: bytes.len() as u64,
        });
        if files.len() as u64 > work.bounds.quotas.max_entries {
            return Err(Error::Quota);
        }
    }
    Ok(())
}
