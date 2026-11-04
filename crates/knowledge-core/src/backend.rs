//! Neutral, serializable observations and one root-bound ordinary-backend contract.
//!
//! This module has no adapter, filesystem access, credential store, registry, or authorization.
//! Constructing any value asserts data only; real adapters and the service must check current admission.
use crate::data::{BackendKind, ContentDigest, DataError, RelativePath};
use serde::Serialize;

/// A data qualifier for an already selected library/home or mount, not root admission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BackendIdentity {
    /// Stable nonempty library data ID; never inferred from a name or digest.
    pub library_id: String,
    /// Null denotes home-relative content; non-null denotes the selected mount's stable ID.
    pub mount_id: Option<String>,
    /// The concrete ordinary backend kind, not a plugin identifier.
    pub backend: BackendKind,
}

/// Backend-qualified continuity evidence, not content identity or permission to reassociate assets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileIdEvidence {
    /// Library/mount/backend in which this evidence was observed.
    pub backend: BackendIdentity,
    /// Opaque backend file identifier; an inode alone is insufficient reuse protection.
    pub file_id: String,
    /// Observed volume/server identity, when reliably available.
    pub volume_id: Option<String>,
    /// Generation/reuse discriminator, when reliably available.
    pub generation: Option<String>,
    /// Birth time in signed nanoseconds, when reliably available; never invented from mtime.
    pub birth_ns: Option<i128>,
}

/// Residency evidence; unknown cloud residency never implicitly permits a body read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Residency {
    /// Tested backend facts identify currently resident bytes.
    Resident,
    /// The object is a nonresident placeholder and must not be sniffed or hashed implicitly.
    Placeholder,
    /// Residency is not known; metadata may still be available.
    Unknown,
}

/// Freshness of an observation at its observation boundary, not a permanent validity promise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// The backend checked the relevant current facts; this does not imply a verified digest.
    Current,
    /// The backend detected drift from the expected observation.
    Changed,
    /// The backend cannot establish currentness.
    Unknown,
}

/// A typed nonavailable state; none is equivalent to a successful empty enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    /// The selected backend cannot currently be reached.
    Offline,
    /// Access was denied; do not infer absence.
    Denied,
    /// Bytes are a nonresident placeholder.
    Placeholder,
    /// Availability or required residency/currentness cannot be established.
    Unknown,
    /// Identity or content changed; do not deliver the expected version's bytes.
    Changed,
    /// The backend does not support the requested operation or object.
    Unsupported,
    /// Absence was actually established, not inferred from a failed/incomplete listing.
    Deleted,
}

/// Metadata availability, deliberately separate from freshness and byte-read admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    /// Metadata is available; residency and read admission still need their own checks.
    Available,
    /// A classified nonavailable state, not an empty result.
    Unavailable(Unavailable),
}

/// A non-hashing stat observation; no field supplies a digest or portable authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StatSnapshot {
    /// Backend-qualified context in which this snapshot was observed.
    pub backend: BackendIdentity,
    /// Path inside that already selected root, not a new absolute root.
    pub path: RelativePath,
    /// Observed source size, not a content digest or a read allocation request.
    pub size_bytes: u64,
    /// High-resolution signed modification time, when supported.
    pub modified_ns: Option<i128>,
    /// High-resolution signed change time, when supported.
    pub changed_ns: Option<i128>,
    /// Qualified continuity facts; null means unavailable, not path-based identity.
    pub file_id: Option<FileIdEvidence>,
    /// Provider/server content-version observation, when supported.
    pub backend_version: Option<String>,
    /// Tested provider residency, not a manifest claim.
    pub residency: Residency,
    /// Currentness of these facts; stat-only facts do not detect every stat-preserved edit.
    pub freshness: Freshness,
}

/// Metadata can retain prior/placeholder facts without admitting any source bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StatObservation {
    /// Actual availability, including explicit offline/denied/placeholder/unknown states.
    pub availability: Availability,
    /// Available or last-known metadata; never evidence of current bytes by itself.
    pub snapshot: Option<StatSnapshot>,
}

/// An explicit expected byte version to revalidate, not a constructor-issued verification grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExpectedVersion {
    /// Claimed verified SHA-256; the caller must obtain it from actual admitted bytes.
    pub digest: ContentDigest,
    /// Backend-qualified stat, identity, path, residency, and provider-version evidence.
    pub snapshot: StatSnapshot,
}

/// Concrete backend observations, never portable manifest assertions or permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Capabilities {
    /// Confined bounded range reads are supported.
    pub range_read: bool,
    /// Reliable qualified file-ID evidence is available; it is not content identity.
    pub stable_file_id: bool,
    /// Reliable event continuity is supported; events remain hints requiring reconciliation.
    pub reliable_watch: bool,
    /// Real conditional-write primitives are supported, not a simulated unconditional rename.
    pub conditional_write: bool,
    /// Tested provider residency metadata is supported; false does not mean resident.
    pub residency: bool,
}

/// Explicit per-call ceilings; zero permits no corresponding work, never unlimited work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Budget {
    /// Maximum objects examined in this call, including unavailable entries.
    pub max_files: u64,
    /// Maximum bytes read/retained for the call; metadata must also be budgeted.
    pub max_bytes: u64,
    /// Maximum elapsed milliseconds before stopping further work.
    pub max_millis: u64,
    /// Maximum working-memory reservation for the call, not a claimed measured RSS value.
    pub max_memory_bytes: u64,
}

/// Per-call observed progress, not a global corpus size or fabricated completion percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// Objects actually examined, including unavailable entries.
    pub examined: u64,
    /// Bytes actually read, not an estimate of original corpus size.
    pub bytes_read: u64,
    /// Elapsed time observed by the backend for this call.
    pub elapsed_millis: u64,
}

/// An opaque backend-qualified continuation, never a durable source ID or permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Cursor {
    /// Backend/root context that must still match before continuation.
    pub backend: BackendIdentity,
    /// Bounded opaque observation data; reject stale/wrong-context cursors, never redirect roots.
    pub token: String,
}

/// Only confined regular files and ordinary directories appear in ordinary listings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// An ordinary directory, not a followed symlink or a discovered new library grant.
    Directory,
    /// A regular file, including formats supported only by metadata search.
    File,
}

/// One bounded listing observation, with no authored annotation or global source identity inference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ListedEntry {
    /// Root-relative UTF-8 path; non-UTF-8 names require an explicit unsupported observation.
    pub path: RelativePath,
    /// Supported ordinary object kind.
    pub kind: EntryKind,
    /// Metadata and its actual availability; no implicit hash/read is permitted.
    pub stat: StatObservation,
}

/// Why an enumeration stopped without establishing absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnumerationStop {
    /// A declared file/byte/time/memory budget was exhausted.
    BudgetExhausted,
    /// The caller cancelled work.
    Cancelled,
    /// Connectivity was lost after partial observations.
    Offline,
    /// Access was denied in part of the selected scope.
    Denied,
    /// Event/cursor/root continuity was lost and reconciliation is required.
    Changed,
    /// The backend cannot establish complete relevant coverage.
    Unknown,
    /// Some selected objects/names cannot be represented or enumerated.
    Unsupported,
}

/// Completion is explicit; only a complete successful relevant listing can establish absence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "extent", rename_all = "snake_case")]
pub enum Enumeration {
    /// Entire relevant selected scope was observed successfully, including the legitimate empty case.
    Complete {
        /// Entries observed within the declared call budget.
        entries: Vec<ListedEntry>,
        /// Actual bounded per-call progress.
        progress: Progress,
    },
    /// Partial observations are useful metadata, never a successful empty/complete directory.
    Partial {
        /// Entries observed before stopping, possibly empty.
        entries: Vec<ListedEntry>,
        /// Explicit reason that absence cannot be inferred.
        stopped: EnumerationStop,
        /// Continuation when trustworthy; null can require a new reconciliation.
        next_cursor: Option<Cursor>,
        /// Actual bounded per-call progress.
        progress: Progress,
    },
}

/// A checked nonempty half-open byte range, without permission to read the referenced source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ByteRange {
    /// Checked zero-based beginning, interpreted only inside the expected selected source.
    offset: u64,
    /// Positive length whose addition to the offset cannot overflow.
    length: u64,
}

impl ByteRange {
    /// Reject zero length or end overflow; caller/backend budgets impose the actual read ceiling.
    pub fn new(offset: u64, length: u64) -> Result<Self, DataError> {
        if length == 0 || offset.checked_add(length).is_none() {
            return Err(DataError::InvalidRange);
        }
        Ok(Self { offset, length })
    }

    /// Return the zero-based requested start.
    pub fn offset(self) -> u64 {
        self.offset
    }

    /// Return the finite requested length, not an allocation instruction.
    pub fn length(self) -> u64 {
        self.length
    }
}

/// One exact bounded range of actual bytes after successful before/after version checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RangeRead {
    /// Root/backend/path-qualified version actually checked; a naked digest cannot locate content.
    pub version: ExpectedVersion,
    /// Requested range, capped independently by the operation budget.
    pub range: ByteRange,
    /// Actual bytes; length is the requested length or the remaining length at established EOF.
    pub bytes: Vec<u8>,
    /// True only when the read reached the verified source's end.
    pub end_of_file: bool,
    /// Actual per-call work, not a claim that any larger source was fully read.
    pub progress: Progress,
}

/// Classified operation failures; ordinary availability is not collapsed into success or absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendError {
    /// Required current availability/residency/version was not established.
    Unavailable(Unavailable),
    /// The declared budget cannot complete the operation; no partial range is acknowledged as complete.
    BudgetExhausted,
    /// Request/cursor/expected-version data does not match the selected root or operation.
    InvalidRequest,
    /// An unclassified I/O failure, with no raw paths, credentials, or provider diagnostic payload.
    Io,
}

impl std::fmt::Display for BackendError {
    /// Emit a fixed input-free diagnostic; the typed variant retains the actionable classification.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable(_) => "backend observation unavailable",
            Self::BudgetExhausted => "backend budget exhausted",
            Self::InvalidRequest => "invalid backend request",
            Self::Io => "backend I/O failed",
        })
    }
}

impl std::error::Error for BackendError {}

/// The single ordinary-backend seam; instances bind an explicitly selected root/context externally.
///
/// Implementors must keep root identity stable, exclude child-owned paths and undeclared escaping
/// symlinks/special files, and reject non-UTF-8 names explicitly rather than invent aliases.
/// Budgets bound all work and observations, including metadata/cursors/buffers; this trait provides
/// no default implementation that could fake enforcement, empty success, residency, or permission.
/// NAS uses existing OS mounts only; cloud unknown/placeholder residency remains metadata-only
/// without independently established selected-file read/hydration admission.
/// Local/service approval, exclusions, and identity must be rechecked before every delivered body.
/// Optional watch/conditional-write support is represented in capabilities only in this slice;
/// no tracking/write API, plugin extension mechanism, or backend implementation is supplied.
pub trait MountBackend {
    /// Return observed capabilities, never grants or manifest-derived claims.
    fn capabilities(&self) -> Capabilities;

    /// Enumerate the selected scope within budget, validating any cursor's backend/root continuity.
    ///
    /// Return partial coverage for incomplete work and a classified error for an unavailable scope.
    /// Never convert denial/offline/cancellation/uncertainty into a complete empty listing.
    fn list(&self, cursor: Option<&Cursor>, budget: Budget) -> Result<Enumeration, BackendError>;

    /// Inspect current metadata without hashing, sniffing a placeholder, hydration, or annotations.
    ///
    /// Preserve typed availability/residency/freshness and qualified file/version facts.
    /// Failure to inspect is not deletion; last-known metadata must not be called current.
    fn stat(&self, path: &RelativePath, budget: Budget) -> Result<StatObservation, BackendError>;

    /// Read an admitted regular source range only after checking the expected identity and version.
    ///
    /// Check availability/residency/exclusions before reading and recheck version/identity afterward.
    /// Drift returns `Unavailable::Changed`, never mixed or stale bytes.
    /// Unknown/placeholder residency alone, a digest, cursor, or public data constructor never admits
    /// a read or hydration; selected-file admission must come from the independent actual owner.
    /// Return a classified error if unsupported, unavailable, over budget, or unable to finish the range.
    fn open_range(
        &self,
        expected_version: &ExpectedVersion,
        range: ByteRange,
        budget: Budget,
    ) -> Result<RangeRead, BackendError>;
}
