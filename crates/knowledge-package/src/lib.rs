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
//! [`FrozenArchive`] provides verified held STORED ranges and compatible component directories.
//! [`V1Package`] verifies genuine directory/TAR inventories without extracting a TAR filesystem.
//! [`V1Projection`] pins explicit native/foreign core mappings and publishes host-manifest-bound artifacts.
//! On Linux and macOS, held descriptors call `Store::import_copy`, `Store::install`, `History::verify_bundle`, `History::admit_bundle`, and ordered `export_epub_held_funded`.
//! Signer trust, kernel search, snapshot activation, macOS execution, and external host publication remain open.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use archive::{Member, Out, hex, method_of, stream};
use knowledge_core::admission::{Account, Decision, Error as AdmissionError, Holding, Origin};
use knowledge_core::backend::Budget;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_core::jobs::Reservation;
use manifest::operation_id;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod archive;
mod capturing;
mod compatibility;
mod delegation;
mod foreign;
mod frozen;
mod gated;
mod manifest;
mod metadata;
mod native;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod owners;
mod projection;
mod remap;
mod restoring;
#[cfg(test)]
mod scalar_control;
mod source;
mod tar;

pub use capturing::capture;
pub use compatibility::{
    IndexDecision, IndexDirectory, IndexOmission, IndexRequirements, VectorSpace,
};
pub use delegation::OwnerHandoff;
pub use foreign::{V1Directory, V1Package};
pub use frozen::{ArchiveVersion, EntryRange, FrozenArchive, StoredRange};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use owners::{
    EpubOwnerError, HeldMember, HistoryOwner, StorageOwner, TextInstall, admit_inert_objects,
    export_ordered_epub, import_verified_source, install_verified_text, native_operation,
    verify_inert_bundle,
};
pub use projection::{V1Artifact, V1Carrier, V1ExportOutcome, V1MappedFile, V1Projection};
pub use remap::{V1PathMap, V1ReferenceMap};
pub use restoring::{apply, stage};
pub use source::VerifiedSource;

/// Format identifier stored in `manifest.json`.
pub const FORMAT: &str = "omni.knowledge-package";

/// Storage no-clobber apply. `apply_text_change` is the storage owner operation.
///
/// `install_new` is the exclusive create for original bytes. Neither method may delete unrelated files.
/// A copy failure forbids publication. An existing destination stays in place.
pub trait SafeApply {
    /// Install authored text when `expected` still matches, or create it when `expected` is absent.
    fn apply_text_change(
        &mut self,
        operation_id: &str,
        path: &RelativePath,
        expected: Option<&ContentDigest>,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error>;
    /// Create one original file only when the destination name is absent.
    fn install_new(
        &mut self,
        operation_id: &str,
        path: &RelativePath,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error>;
}

/// History handoff for an inert bundle.
///
/// `GitAdmit::Inert` does not activate refs, checkout, config, hooks, remotes, or trust.
/// It is not snapshot activation and it is not package trust.
pub trait GitTransactions {
    /// Hand the held bundle to the history owner. A path is not source proof.
    /// `GitAdmit::Inert` is not snapshot activation, package trust, or a waiver of a later host check.
    fn admit_inert_bundle(
        &mut self,
        operation_id: &str,
        bundle: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<GitAdmit, Error>;
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
    /// Optional packaged generation cannot be opened by the actual native consumer.
    IndexOmitted(IndexOmission),
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
            Self::IndexOmitted(_) => "optional package index omitted",
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
    /// Object examinations admitted before native access, including unavailable objects and repeated verification/copy passes.
    pub files: u64,
    /// Live memory charged to this call, including scratch that has not dropped.
    live: Rc<Cell<u64>>,
    /// Generated bytes reserved during this call. `written` does not refund the ceiling.
    temporary_total: u64,
    /// Optional actual caller-currentness check, retained through every refill/effect.
    /// This callback is not authority itself; native hosts supply their existing opaque owner.
    current: Option<&'a dyn Fn() -> Result<(), Error>>,
    /// Read-only slice of the actual already reserved holding, never another pool.
    prepaid_memory: Option<u64>,
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
        let retained = holding.remaining().map_err(admission_error)?.memory_bytes;
        if retained > bounds.budget.max_memory_bytes {
            return Err(Error::TooLarge);
        }
        Ok(Self {
            bounds,
            holding,
            cancel,
            deadline,
            read_bytes: 0,
            expanded: 0,
            files: 0,
            live: Rc::new(Cell::new(retained)),
            temporary_total: 0,
            current: None,
            prepaid_memory: None,
        })
    }

    /// Bind the caller's existing current source/policy/Claim check for the whole operation.
    /// Checks before construction and at bounded refills/effects, including initial ZIP verification.
    /// Keep the actual native owner and this borrow alive; a path/digest/boolean callback is not a grant.
    /// Plain `new` remains a budget-only data seam, not qualified revocable native activation.
    pub fn guarded(
        bounds: &'a Bounds,
        holding: &'a mut Holding,
        cancel: &'a AtomicBool,
        current: &'a dyn Fn() -> Result<(), Error>,
    ) -> Result<Self, Error> {
        current()?;
        let mut work = Self::new(bounds, holding, cancel)?;
        work.current = Some(current);
        gate(&work)?;
        Ok(work)
    }

    /// Read under the caller's complete already paid peak without late/double admission.
    /// The borrowed holding remains live through all scratch, returned metadata and native readers.
    /// No generated writes are permitted by this read-only mode. Scratch releases only its local
    /// liveness count; the actual same-account holding is never finished or refunded here.
    pub fn guarded_reader(
        bounds: &'a Bounds,
        holding: &'a mut Holding,
        cancel: &'a AtomicBool,
        current: &'a dyn Fn() -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let mut work = Self::guarded(bounds, holding, cancel, current)?;
        work.prepaid_memory = Some(
            work.holding
                .remaining()
                .map_err(admission_error)?
                .memory_bytes,
        );
        work.live.set(0);
        Ok(work)
    }

    /// Grow `holding` before the caller allocates or writes.
    ///
    /// Retained allocations stay charged until the host finishes this holding.
    /// Local scratch has a separate same-account capability and finishes only after its buffers drop.
    /// Waiting leaves the holding unchanged.
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
    /// Whether the package claims this generation is searchable as stored.
    pub compatible: bool,
    /// Why the generation was not queried. Core bytes stay usable either way.
    pub fallback: Option<String>,
    /// Optional identity/extension facts retained verbatim; never native compatibility authority.
    pub metadata: Map<String, Value>,
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
    /// Held native generation and private snapshot/operation binding.
    authority: std::sync::Arc<restoring::StageAuthority>,
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
    let conflicts = conflicts_for(&opened.manifest.entries, existing, work)?;
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
    let root_fd = native::bind(root, work)?;
    let mut manifest_file = native::file(&root_fd, Path::new("manifest.json"), false, work)?;
    let bytes = read_source(&mut manifest_file, work)?;
    if !std::str::from_utf8(&bytes)
        .unwrap_or("")
        .contains("openkit.knowledge-package")
    {
        return Err(Error::Invalid);
    }
    let _ = host;
    let mut files = Vec::new();
    walk_v1(&root_fd, Path::new(""), work, &mut files, 0)?;
    files.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));
    Ok(V1Plan {
        state: V1State::CompatibilityRefused,
        files,
        host_context: host.is_some(),
    })
}

/// Retained source inventory only; payload bytes stay on the native streaming path.
struct Digested {
    path: RelativePath,
    role: Role,
    method: Method,
    mount_id: Option<String>,
    source_id: Option<String>,
    digest: ContentDigest,
    size: u64,
}

/// Verified framing, canonical bytes, and semantic inventory under the caller holding.
struct Opened {
    manifest: Manifest,
    snapshot_id: ContentDigest,
    signature: Signature,
    installable: bool,
    members: Vec<Member>,
    canonical: Vec<u8>,
}

/// Verify every real payload and bind its digest/method/size to the canonical inventory.
fn open_verified(archive: &mut File, work: &mut Work<'_>) -> Result<Opened, Error> {
    let mut members = archive::list(archive, work)?;
    members.sort_unstable_by(|a, b| a.name.cmp(&b.name));
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
            charge_fresh(work, member.name.len() as u64 + 256, 0)?;
            hashes.try_reserve_exact(1).map_err(|_| Error::TooLarge)?;
            hashes.push((
                member.name.clone(),
                digest,
                method_of(member)?,
                member.uncompressed,
            ));
        }
    }
    let canonical = canonical.ok_or(Error::Invalid)?;
    let parsed = manifest::parse(&canonical, work)?;
    let snapshot = digest_bytes(&canonical)?;
    if snapshot.as_str() != manifest_sha.ok_or(Error::Invalid)? {
        return Err(Error::Checksum);
    }
    hashes.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    for entry in &parsed.entries {
        gate(work)?;
        let found = &hashes[hashes
            .binary_search_by(|item| item.0.as_str().cmp(entry.path.as_str()))
            .map_err(|_| Error::Disagreement)?];
        if found.2 != entry.method || found.3 != entry.size_bytes {
            return Err(Error::Disagreement);
        }
        if found.1 != entry.sha256.as_str() {
            return Err(Error::Checksum);
        }
    }
    if hashes.len() != parsed.entries.len() {
        return Err(Error::Hostile);
    }
    let installable = parsed.required_features.is_empty();
    Ok(Opened {
        manifest: parsed,
        snapshot_id: snapshot,
        signature,
        installable,
        members,
        canonical,
    })
}

/// Classify inert signature framing without making a signer-trust claim.
fn signature_bytes(
    archive: &mut File,
    member: &Member,
    work: &mut Work<'_>,
) -> Result<Signature, Error> {
    if member.uncompressed == 0 || member.uncompressed > 65_536 {
        return Ok(Signature::Invalid);
    }
    let _text_scratch = preallocate(work, member.uncompressed * 3)?;
    let mut bytes = Vec::new();
    stream(archive, member, work, &mut Out::Keep(&mut bytes))?;
    let text = String::from_utf8_lossy(&bytes);
    if text.contains("-----BEGIN SSH SIGNATURE-----") {
        Ok(Signature::Unverified)
    } else {
        Ok(Signature::Invalid)
    }
}

/// Admit folded-name workspace and output clones before case-insensitive conflict matching.
fn conflicts_for(
    entries: &[EntryRecord],
    existing: &[RelativePath],
    work: &mut Work<'_>,
) -> Result<Vec<RelativePath>, Error> {
    let names = entries
        .iter()
        .map(|e| e.path.as_str().len() as u64)
        .sum::<u64>()
        .checked_add(
            existing
                .iter()
                .map(|p| p.as_str().len() as u64)
                .sum::<u64>(),
        )
        .ok_or(Error::TooLarge)?;
    let _scratch = preallocate(
        work,
        names
            .checked_mul(3)
            .and_then(|n| n.checked_add(existing.len() as u64 * 32))
            .ok_or(Error::TooLarge)?,
    )?;
    charge_fresh(
        work,
        entries
            .iter()
            .map(|e| e.path.as_str().len() as u64 + 64)
            .sum(),
        0,
    )?;
    let mut folded: Vec<_> = existing.iter().map(|p| p.as_str().to_lowercase()).collect();
    folded.sort_unstable();
    let mut conflicts = Vec::new();
    for entry in entries {
        gate(work)?;
        if matches!(entry.role, Role::Core | Role::Original)
            && folded
                .binary_search(&entry.path.as_str().to_lowercase())
                .is_ok()
        {
            conflicts.push(entry.path.clone());
        }
    }
    Ok(conflicts)
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

fn method_for(role: Role) -> Result<Method, Error> {
    match role {
        Role::Core => Ok(Method::Deflate),
        Role::Original | Role::Index | Role::History | Role::Journal => Ok(Method::Stored),
    }
}

/// Read one bounded metadata input under the caller holding with exact-capacity growth.
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
        bytes.try_reserve_exact(n).map_err(|_| Error::TooLarge)?;
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
        Some(IndexSet {
            paths,
            compatible: false,
            fallback: Some("kernel_not_wired".into()),
            metadata: Map::new(),
        })
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
    bytes: u64,
    live: Option<Rc<Cell<u64>>>,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(live) = self.live.take() {
            live.set(live.get().saturating_sub(self.bytes));
        }
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
    gate(work)?;
    if bytes == 0 {
        return Ok(Scratch {
            holding: None,
            bytes: 0,
            live: None,
        });
    }
    note_live(work, bytes)?;
    if work.prepaid_memory.is_some() {
        return Ok(Scratch {
            holding: None,
            bytes,
            live: Some(Rc::clone(&work.live)),
        });
    }
    let account = Account::current().map_err(|error| {
        release_live(work, bytes);
        admission_error(error)
    })?;
    match account
        .reserve(
            Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .map_err(|error| {
            release_live(work, bytes);
            admission_error(error)
        })? {
        Decision::Ready(holding) => Ok(Scratch {
            holding: Some(holding),
            bytes,
            live: Some(Rc::clone(&work.live)),
        }),
        Decision::Waiting(_) => {
            release_live(work, bytes);
            Err(Error::Waiting)
        }
    }
}

pub(crate) fn admission_error(error: AdmissionError) -> Error {
    match error {
        AdmissionError::TooLargeMemory | AdmissionError::TooLargeStorage => Error::TooLarge,
        _ => Error::Indeterminate,
    }
}

fn account_memory(work: &mut Work<'_>, bytes: u64) -> Result<(), Error> {
    charge_fresh(work, bytes, 0)
}

pub(crate) fn charge_fresh(
    work: &mut Work<'_>,
    memory_bytes: u64,
    temporary_bytes: u64,
) -> Result<(), Error> {
    gate(work)?;
    if memory_bytes == 0 && temporary_bytes == 0 {
        return Ok(());
    }
    let next = work
        .live
        .get()
        .checked_add(memory_bytes)
        .ok_or(Error::TooLarge)?;
    let next_temporary = work
        .temporary_total
        .checked_add(temporary_bytes)
        .ok_or(Error::TooLarge)?;
    if next > work.bounds.budget.max_memory_bytes || next_temporary > work.bounds.budget.max_bytes {
        return Err(Error::TooLarge);
    }
    if let Some(capacity) = work.prepaid_memory {
        if temporary_bytes != 0 || next > capacity {
            return Err(Error::TooLarge);
        }
        work.live.set(next);
        return Ok(());
    }
    match work.holding.grow(Reservation {
        memory_bytes,
        temporary_bytes,
    }) {
        Ok(Decision::Ready(())) => {
            work.live.set(next);
            work.temporary_total = next_temporary;
            Ok(())
        }
        Ok(Decision::Waiting(_)) => Err(Error::Waiting),
        Err(error) => Err(admission_error(error)),
    }
}

pub(crate) fn note_live(work: &Work<'_>, bytes: u64) -> Result<(), Error> {
    let next = work.live.get().checked_add(bytes).ok_or(Error::TooLarge)?;
    if next > work.bounds.budget.max_memory_bytes
        || work.prepaid_memory.is_some_and(|capacity| next > capacity)
    {
        return Err(Error::TooLarge);
    }
    work.live.set(next);
    Ok(())
}

pub(crate) fn release_live(work: &Work<'_>, bytes: u64) {
    work.live.set(work.live.get().saturating_sub(bytes));
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

/// Hash foreign regular files through descriptor-relative traversal with finite work.
fn walk_v1(
    root: &File,
    prefix: &Path,
    work: &mut Work<'_>,
    files: &mut Vec<V1File>,
    depth: usize,
) -> Result<(), Error> {
    if depth > work.bounds.quotas.max_depth {
        return Err(Error::Quota);
    }
    let _scratch = preallocate(work, 32_768 + 8192)?;
    note_file(work)?;
    let mut entries = rustix::fs::Dir::read_from(root).map_err(|_| Error::Io)?;
    while let Some(entry) = entries.read() {
        gate(work)?;
        let entry = entry.map_err(|_| Error::Io)?;
        let name = entry.file_name().to_str().map_err(|_| Error::Hostile)?;
        if matches!(name, "." | ".." | ".git" | ".omni") {
            continue;
        }
        note_file(work)?;
        let stat = rustix::fs::statat(root, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Error::Io)?;
        let _path_scratch = preallocate(work, (prefix.as_os_str().len() + name.len() + 1) as u64)?;
        let path = prefix.join(name);
        let text = path.to_str().ok_or(Error::Hostile)?;
        if text.len() > work.bounds.quotas.max_path_bytes {
            return Err(Error::Quota);
        }
        match rustix::fs::FileType::from_raw_mode(stat.st_mode) {
            rustix::fs::FileType::Directory => {
                let dir = native::directory(root, Path::new(name), false, work)?;
                walk_v1(&dir, &path, work, files, depth + 1)?;
            }
            rustix::fs::FileType::RegularFile => {
                if text == "manifest.json" {
                    continue;
                }
                if files.len() as u64 >= work.bounds.quotas.max_entries {
                    return Err(Error::Quota);
                }
                charge_fresh(work, text.len() as u64 * 4 + 512, 0)?;
                let relative = manifest::path(text)?;
                let mut input = native::file(root, Path::new(name), false, work)?;
                let mut sha = Sha256::new();
                let mut size = 0u64;
                let mut buffer = [0u8; 8192];
                loop {
                    gate(work)?;
                    let n = input.read(&mut buffer).map_err(|_| Error::Io)?;
                    if n == 0 {
                        break;
                    }
                    size = size.checked_add(n as u64).ok_or(Error::Quota)?;
                    if size > work.bounds.quotas.max_entry_expanded {
                        return Err(Error::Quota);
                    }
                    note_read(work, n as u64)?;
                    sha.update(&buffer[..n]);
                }
                files.try_reserve_exact(1).map_err(|_| Error::TooLarge)?;
                files.push(V1File {
                    path: relative,
                    sha256: ContentDigest::new(hex(&sha.finalize())).map_err(|_| Error::Invalid)?,
                    size,
                });
            }
            _ => return Err(Error::Hostile),
        }
    }
    Ok(())
}

/// Check cooperative cancellation and deadline before CPU or native I/O work.
pub(crate) fn gate(work: &Work<'_>) -> Result<(), Error> {
    if work.cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    if Instant::now() >= work.deadline {
        return Err(Error::Deadline);
    }
    if let Some(current) = work.current {
        current()?;
    }
    Ok(())
}

/// Refuse another object examination or delegation before any native access or copy effect.
pub(crate) fn check_file(work: &Work<'_>) -> Result<(), Error> {
    gate(work)?;
    if work.files >= work.bounds.budget.max_files {
        return Err(Error::Quota);
    }
    Ok(())
}

/// Admit one object examination, including an unavailable object, without exceeding the ceiling.
pub(crate) fn note_file(work: &mut Work<'_>) -> Result<(), Error> {
    check_file(work)?;
    work.files = work.files.checked_add(1).ok_or(Error::Quota)?;
    Ok(())
}

/// Charge actual read bytes against the call, never a declared source length.
pub(crate) fn note_read(work: &mut Work<'_>, bytes: u64) -> Result<(), Error> {
    work.read_bytes = work.read_bytes.checked_add(bytes).ok_or(Error::Quota)?;
    if work.read_bytes > work.bounds.budget.max_bytes {
        return Err(Error::Quota);
    }
    Ok(())
}
