//! Public wire DTOs mirroring `docs/specs/service.md`.
//!
//! Every field is required, including fields represented by `Option`. `None`
//! encodes JSON null, not omission. Unknown input fields are refused; unknown
//! result variants are never interpreted as success. These are facts/requests,
//! not backend state machines, local approvals, or authorization capabilities.

use crate::scalar::*;
use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize an explicitly present nullable field, never a missing-field default.
fn required_nullable<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// One complete versioned command envelope; the codec checks `wire_version`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Protocol version; only 1 is accepted by the codec.
    pub wire_version: u64,
    /// Correlation only, never idempotency or authority.
    pub request_id: Id,
    /// Requested data operation; parsing it grants no permission.
    pub command: Command,
}

/// One complete versioned response envelope; the codec checks `wire_version`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    /// Protocol version; only 1 is emitted by the codec.
    pub wire_version: u64,
    /// Original request correlation identifier.
    pub request_id: Id,
    /// Typed observation or refusal, not an implied remote/domain success.
    pub result: Result,
}

/// Explicit finite work bounds; zero is not silently replaced by a default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Maximum admitted files.
    pub max_files: UnsignedInteger,
    /// Maximum bytes.
    pub max_bytes: U64,
    /// Maximum elapsed milliseconds.
    pub max_millis: UnsignedInteger,
    /// Maximum memory bytes.
    pub max_memory_bytes: U64,
}

/// Verified source version evidence, distinct from a cheap stat observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedVersion {
    /// Verified source SHA-256.
    pub digest: Digest,
    /// Verified byte length.
    pub size_bytes: U64,
    /// Canonical signed modification timestamp, if supplied by the backend.
    #[serde(deserialize_with = "required_nullable")]
    pub modified_ns: Option<SignedDecimal>,
    /// Canonical signed change timestamp, if supplied by the backend.
    #[serde(deserialize_with = "required_nullable")]
    pub changed_ns: Option<SignedDecimal>,
    /// Backend-qualified file evidence, not source identity.
    #[serde(deserialize_with = "required_nullable")]
    pub file_id: Option<String>,
    /// Backend version evidence, if available.
    #[serde(deserialize_with = "required_nullable")]
    pub backend_version: Option<String>,
}

/// Source identity in a selected library, never a pathname or permission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// Selected library data identifier.
    pub library_id: Id,
    /// Stable source data identifier, independent of digest.
    pub source_id: Id,
}

/// A non-root write target inside an already admitted home or mount.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Selected library data identifier.
    pub library_id: Id,
    /// Null selects the home; an ID selects an admitted mount.
    #[serde(deserialize_with = "required_nullable")]
    pub mount_id: Option<Id>,
    /// Nonempty normalized path; no new root is admitted by it.
    pub path: RelativePath,
}

/// Original or processor-provenance coordinates; the codec cannot certify truth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Locator {
    /// One-based inclusive original lines and optional half-open byte offsets.
    OriginalLines {
        /// First original line.
        start_line: PositiveInteger,
        /// Last original line.
        end_line: PositiveInteger,
        /// First byte, when known.
        #[serde(deserialize_with = "required_nullable")]
        start_byte: Option<U64>,
        /// Exclusive ending byte, when known.
        #[serde(deserialize_with = "required_nullable")]
        end_byte: Option<U64>,
    },
    /// Lines in a derived representation, not necessarily original lines.
    RepresentationLines {
        /// Exact representation digest.
        representation_digest: Digest,
        /// First representation line.
        start_line: PositiveInteger,
        /// Last representation line.
        end_line: PositiveInteger,
    },
    /// Processor-proven page coordinate.
    Page {
        /// Exact representation digest.
        representation_digest: Digest,
        /// One-based page number.
        page: PositiveInteger,
        /// First line, if mapped.
        #[serde(deserialize_with = "required_nullable")]
        start_line: Option<PositiveInteger>,
        /// Last line, if mapped.
        #[serde(deserialize_with = "required_nullable")]
        end_line: Option<PositiveInteger>,
    },
    /// Processor-proven slide coordinate.
    Slide {
        /// Exact representation digest.
        representation_digest: Digest,
        /// One-based slide number.
        slide: PositiveInteger,
    },
    /// Processor-proven sheet and optional cell range.
    Sheet {
        /// Exact representation digest.
        representation_digest: Digest,
        /// Sheet name.
        sheet: String,
        /// Cell range, if mapped.
        #[serde(deserialize_with = "required_nullable")]
        cell_range: Option<String>,
    },
    /// Milliseconds in the original recording; zero/zero can mean unknown timing.
    Time {
        /// Start time in the original clock.
        start_ms: U64,
        /// End time in the original clock.
        end_ms: U64,
        /// Exact representation digest, if present.
        #[serde(deserialize_with = "required_nullable")]
        representation_digest: Option<Digest>,
    },
    /// No defensible coordinate is available.
    Unlocated,
}

/// Availability observation; unavailable never means a successful empty source.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    /// Currently available.
    Available,
    /// Backend offline.
    Offline,
    /// Access denied.
    Denied,
    /// Cloud placeholder, not admitted resident bytes.
    Placeholder,
    /// Not yet established.
    Unknown,
    /// Source version changed.
    Changed,
    /// Unsupported representation or name.
    Unsupported,
    /// Verified deletion observation.
    Deleted,
}

/// Enumeration completeness, separate from result limiting.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enumeration {
    /// Enumeration completed.
    Complete,
    /// Enumeration is partial.
    Partial,
    /// Enumeration has not started.
    NotStarted,
}

/// Whether results enumerate observations or retain a ranked top subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultExtent {
    /// Enumeration observations.
    Enumeration,
    /// Ranked top-k observations.
    TopK,
}

/// Visible admitted-source counters; categories can overlap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    /// Visible discovered sources.
    pub discovered: U64,
    /// Digest-verified sources.
    pub verified: U64,
    /// Sources awaiting hashing.
    pub pending_hash: U64,
    /// Sources awaiting extraction.
    pub pending_extraction: U64,
    /// Sources awaiting embedding.
    pub pending_embedding: U64,
    /// Stale sources.
    pub stale: U64,
    /// Unavailable sources.
    pub unavailable: U64,
    /// Unsupported sources.
    pub unsupported: U64,
    /// Failed sources.
    pub failed: U64,
    /// Visible excluded count; unprivileged delivery must use zero.
    pub excluded: U64,
    /// Enumeration completion, not top-k completeness.
    pub enumeration: Enumeration,
    /// Result extent independently of enumeration state.
    pub result_extent: ResultExtent,
    /// Bounded reason for stopping, if any.
    #[serde(deserialize_with = "required_nullable")]
    pub stopped_reason: Option<String>,
}

/// Exposure data, never a portable local approval.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposureMode {
    /// No exposure.
    Off,
    /// Read admission only.
    ReadOnly,
    /// Ordinary read/write admission, not owner-only administration.
    ReadWrite,
}

/// Transport names in exposure data; not authenticated caller classes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// OS-owner IPC.
    OwnerIpc,
    /// Explicit owner-launched stdio.
    OwnerStdio,
    /// Separately admitted local HTTP.
    Http,
}

/// Portable exposure facts, bounded by separate service-local approvals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exposure {
    /// Requested/observed exposure mode.
    pub mode: ExposureMode,
    /// Sensitive-library classification.
    pub sensitive: bool,
    /// Excluded normalized paths.
    pub excluded_paths: Vec<RelativePath>,
    /// Declared transport set, not authenticated access.
    pub transports: Vec<Transport>,
    /// Expected revision identity.
    pub revision: Id,
    /// Expansion awaiting local owner action.
    pub pending_expansion: bool,
}

/// Observable selected-remote scheduling facts, not remote-effect consent itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncSchedule {
    /// Whether scheduling is enabled.
    pub enabled: bool,
    /// Configured interval, positive when enabled.
    pub interval_minutes: UnsignedInteger,
    /// Whether the selected schedule permits push.
    pub allow_push: bool,
    /// Next UTC run; null for disabled scheduling.
    #[serde(deserialize_with = "required_nullable")]
    pub next_run_at: Option<Time>,
}

/// Text root spelling retained by the library layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRoot {
    /// New visible text root.
    Pages,
    /// Mapped retained text root.
    Wiki,
}

/// Library lifecycle observation, not an admission capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryState {
    /// In-memory identity without persisted marker.
    Unmarked,
    /// Ready for admitted operations.
    Ready,
    /// Migration required.
    MigrationRequired,
    /// Recovery inspection required.
    RecoveryRequired,
    /// Frozen read-only archive.
    Frozen,
}

/// History mode observed in a library, including not yet initialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryMode {
    /// Signed history.
    Signed,
    /// Explicitly selected unsigned history.
    UnsignedExplicit,
    /// History not initialized.
    NotInitialized,
}

/// The two creation-time choices; missing signing never silently chooses unsigned.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationHistoryMode {
    /// Require signing.
    Signed,
    /// Explicit unsigned selection.
    UnsignedExplicit,
}

/// Observed selected remote access, not a caller-supplied grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteAccess {
    /// No selected remote.
    None,
    /// Fetch/read capability.
    ReadOnly,
    /// Read/write capability.
    ReadWrite,
}

/// Public library facts; paths here do not authorize opening them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    /// Opaque library data identity.
    pub library_id: Id,
    /// Display name.
    pub name: String,
    /// Selected home path as data.
    pub home_path: String,
    /// Visible text root.
    pub text_root: TextRoot,
    /// Observed marker version; null for unmarked folders.
    #[serde(deserialize_with = "required_nullable")]
    pub manifest_version: Option<SafeInteger>,
    /// Lifecycle observation.
    pub state: LibraryState,
    /// History/signing observation.
    pub history_mode: HistoryMode,
    /// Exposure facts, not local approval.
    pub exposure: Exposure,
    /// Attached mount facts.
    pub mounts: Vec<Mount>,
    /// Observable selected-remote schedule.
    pub sync_schedule: SyncSchedule,
    /// Credential-free selected Git URL, if any.
    #[serde(deserialize_with = "required_nullable")]
    pub remote_url: Option<String>,
    /// Actual selected remote capability.
    pub remote_access: RemoteAccess,
}

/// Known mount backend choices; no plugin or remote-contact registry is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountBackend {
    /// Local folder.
    Local,
    /// OS-mounted NAS.
    Nas,
    /// OS-managed cloud-synced folder.
    CloudFolder,
    /// Explicit Git working copy.
    Git,
}

/// Backend capability facts, not caller authorization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountCapabilities {
    /// Bounded range reads.
    pub range_read: bool,
    /// Stable file identity.
    pub stable_file_id: bool,
    /// Reliable watch support.
    pub reliable_watch: bool,
    /// Conditional write support.
    pub conditional_write: bool,
    /// Residency inspection support.
    pub residency: bool,
}

/// Selected reconciliation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reconciliation {
    /// Startup and periodic reconciliation.
    StartupPeriodic,
    /// Nightly and manual reconciliation.
    NightlyManual,
    /// Explicit manual reconciliation.
    Manual,
}

/// Mount data after admission; a remote URL is not a local mount location.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mount {
    /// Mount data identity.
    pub mount_id: Id,
    /// Logical mount name.
    pub name: String,
    /// Known backend kind.
    pub backend: MountBackend,
    /// Selected local working location.
    pub location: String,
    /// Current availability observation.
    pub availability: Availability,
    /// Observed write capability, not permission.
    pub writable: bool,
    /// Backend mechanics.
    pub capabilities: MountCapabilities,
    /// Last complete scan observation.
    #[serde(deserialize_with = "required_nullable")]
    pub last_complete_scan: Option<Time>,
    /// Reconciliation policy.
    pub reconciliation: Reconciliation,
}

/// Whether the source's digest has been verified.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigestState {
    /// Verified source bytes.
    Verified,
    /// No verified digest/version claim.
    Unverified,
}

/// Observed residency; unknown is not permission to read or hydrate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Residency {
    /// Resident original bytes.
    Resident,
    /// Nonresident placeholder.
    Placeholder,
    /// Residency unestablished.
    Unknown,
}

/// Observed source write activity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    /// Stable observation.
    Stable,
    /// Being written.
    Writing,
}

/// Public source facts; unverified metadata is not an expected-version claim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Library data identity.
    pub library_id: Id,
    /// Source identity, independent of content digest.
    pub source_id: Id,
    /// Null for home-relative sources.
    #[serde(deserialize_with = "required_nullable")]
    pub mount_id: Option<Id>,
    /// Backend-relative nonempty path.
    pub path: RelativePath,
    /// Home-relative or logical raw/mount/path name.
    pub logical_path: RelativePath,
    /// Display title.
    pub title: String,
    /// Authored description.
    pub description: String,
    /// Exact tag values.
    pub tags: Vec<String>,
    /// Authored annotation path, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub annotation_path: Option<RelativePath>,
    /// Verified digest or null, never a fabricated stat digest.
    #[serde(deserialize_with = "required_nullable")]
    pub digest: Option<Digest>,
    /// Digest observation class.
    pub digest_state: DigestState,
    /// Complete verified version, or null for unverified observations.
    #[serde(deserialize_with = "required_nullable")]
    pub version: Option<ExpectedVersion>,
    /// Observed source size, if available.
    #[serde(deserialize_with = "required_nullable")]
    pub size_bytes: Option<U64>,
    /// Observed media type.
    #[serde(deserialize_with = "required_nullable")]
    pub media_type: Option<String>,
    /// UTC creation timestamp, if known.
    #[serde(deserialize_with = "required_nullable")]
    pub created_at: Option<Time>,
    /// UTC modification timestamp, if known.
    #[serde(deserialize_with = "required_nullable")]
    pub modified_at: Option<Time>,
    /// Current availability.
    pub availability: Availability,
    /// Current residency.
    pub residency: Residency,
    /// Current write activity.
    pub activity: Activity,
    /// Separate conflict-copy source identity, if observed.
    #[serde(deserialize_with = "required_nullable")]
    pub conflict_source_id: Option<Id>,
}

/// Independently selected search method.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMethod {
    /// Lexical BM25.
    Bm25,
    /// Literal matching.
    Literal,
    /// LF-line regex matching.
    Regex,
    /// Filename/description matching.
    Files,
    /// Exact tag matching.
    Tags,
    /// Vector candidate retrieval.
    Semantic,
}

/// Already-separated query values; the codec never trims or deduplicates them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    /// Selected libraries.
    pub library_ids: Vec<Id>,
    /// Alternative query strings.
    pub keywords: Vec<String>,
    /// Independently enabled methods.
    pub methods: Vec<SearchMethod>,
    /// All-required exact tag values.
    pub required_tags: Vec<String>,
    /// Explicit case behavior.
    pub case_sensitive: bool,
    /// Distinct-file limit, 1 through 10,000 at the codec boundary.
    pub limit: UnsignedInteger,
    /// Explicit work bounds.
    pub budget: Budget,
    /// Selected admitted image source, if any.
    #[serde(deserialize_with = "required_nullable")]
    pub image_query: Option<SourceRef>,
}

/// Strength of an observation, independent of the selected method name.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// Verified current match.
    VerifiedMatch,
    /// Semantic candidate, not literal proof.
    SemanticCandidate,
    /// Cheap metadata observation.
    MetadataObservation,
}

/// One retained match or candidate with actual source/processor provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// Method that produced this evidence.
    pub method: SearchMethod,
    /// Alternative keyword, or null for image queries.
    #[serde(deserialize_with = "required_nullable")]
    pub keyword: Option<String>,
    /// Defensible coordinate, or unlocated.
    pub locator: Locator,
    /// Bounded Unicode excerpt, not a source-body log.
    pub snippet: String,
    /// Verification/candidate class.
    pub kind: EvidenceKind,
    /// Verified source digest, when known.
    #[serde(deserialize_with = "required_nullable")]
    pub source_digest: Option<Digest>,
    /// Representation digest, when present.
    #[serde(deserialize_with = "required_nullable")]
    pub representation_digest: Option<Digest>,
    /// Processor provenance, when present.
    #[serde(deserialize_with = "required_nullable")]
    pub processor_id: Option<String>,
    /// Exact model-space identity, when present.
    #[serde(deserialize_with = "required_nullable")]
    pub model_space: Option<String>,
    /// Finite uncalibrated score, when present.
    #[serde(deserialize_with = "required_nullable")]
    pub score: Option<FiniteNumber>,
}

/// Ranked file with aggregated, bounded evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileHit {
    /// Source observation.
    pub source: Source,
    /// One-based file rank.
    pub rank: PositiveInteger,
    /// Finite fused score, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub score: Option<FiniteNumber>,
    /// Retained evidence, not fragment votes.
    pub evidence: Vec<Evidence>,
    /// Retained evidence count.
    pub retained_evidence_count: U64,
    /// Useful mapped location count.
    pub useful_location_count: U64,
}

/// One labelled library's search observations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchGroup {
    /// Selected library identity.
    pub library_id: Id,
    /// Generation used, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub generation_id: Option<Id>,
    /// Partitioning identity, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub partitioning_id: Option<Id>,
    /// Distinct ranked files.
    pub files: Vec<FileHit>,
    /// Truthful visible coverage.
    pub coverage: Coverage,
    /// Number of files returned.
    pub returned_files: UnsignedInteger,
    /// Whether the distinct-file limit was reached.
    pub limit_reached: bool,
}

/// Applied request and independently labelled library groups.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    /// Actual request whose results are shown.
    pub applied: SearchRequest,
    /// Library-labelled result groups.
    pub groups: Vec<SearchGroup>,
}

/// Owned job category; no worker starts merely by decoding it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// Reconciliation.
    Reconcile,
    /// Processing.
    Process,
    /// Explicit hydration.
    Hydrate,
    /// Selected EPUB publication.
    EpubExport,
    /// Native/foreign snapshot export.
    Snapshot,
    /// Restore materialization.
    Restore,
    /// Layout migration.
    Migrate,
}

/// Job observation; terminal does not imply full coverage or rollback.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Queued.
    Queued,
    /// Running.
    Running,
    /// Waiting on an offline dependency.
    WaitingOffline,
    /// Completed within reported coverage.
    Completed,
    /// Partial outcome.
    Partial,
    /// Failed.
    Failed,
    /// Cancellation observed, not a rollback assertion.
    Cancelled,
    /// Replaced by a newer source version.
    Superseded,
}

/// Cost-tiered job priority, not implicit hydration consent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobPriority {
    /// Ordinary priority.
    Normal,
    /// Explicitly pinned priority.
    Pinned,
}

/// Inspectable operation job facts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    /// Job identity.
    pub job_id: Id,
    /// Idempotency identity.
    pub operation_id: Id,
    /// Selected library identity.
    pub library_id: Id,
    /// Job category.
    pub kind: JobKind,
    /// Current observed state.
    pub state: JobState,
    /// Selected priority.
    pub priority: JobPriority,
    /// Current truthful visible coverage.
    pub coverage: Coverage,
    /// Classified failure, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub error: Option<ServiceError>,
}

/// Local write outcome, distinct from remote synchronization.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteState {
    /// Saved locally with the required durable commit.
    SavedLocal,
    /// Prepared as a proposal.
    Proposed,
    /// Competing versions preserved.
    Conflict,
    /// Effect outcome requires inspection before replay.
    Indeterminate,
    /// No new bytes or empty commit.
    Unchanged,
}

/// Git synchronization observation; never a cloud-sync assertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// Local only.
    LocalOnly,
    /// Awaiting push.
    PendingPush,
    /// Git push observed.
    Pushed,
    /// No Git synchronization applies.
    NotApplicable,
}

/// Inspectable write result, not a generic boolean success.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteResult {
    /// Idempotency identity.
    pub operation_id: Id,
    /// Local outcome.
    pub state: WriteState,
    /// Final source facts, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub source: Option<Source>,
    /// Durable operation commit, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub commit_id: Option<String>,
    /// Proposed text/integration outcome, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub proposal: Option<Proposal>,
    /// Inspectable preserved competing paths.
    pub recovery_paths: Vec<RelativePath>,
    /// Separate Git synchronization state.
    pub sync_state: SyncState,
}

/// Minimal proposal category, not a general branch framework.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalKind {
    /// Text mutation.
    Text,
    /// Remote integration.
    Merge,
}

/// Proposal lifecycle observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState {
    /// Awaiting exact owner confirmation.
    Pending,
    /// Conflicted and preserved.
    Conflicted,
    /// Accepted through owner confirmation.
    Accepted,
    /// Superseded or stale.
    Obsolete,
}

/// Exact proposal/base/diff tuple; receiving it is not confirmation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// Proposal identity.
    pub proposal_id: Id,
    /// Selected library identity.
    pub library_id: Id,
    /// Original operation identity.
    pub operation_id: Id,
    /// Proposal category.
    pub kind: ProposalKind,
    /// Accepted base commit.
    pub base_commit_id: String,
    /// Candidate commit.
    pub candidate_commit_id: String,
    /// Complete canonical diff digest, not a preview digest.
    pub diff_digest: Digest,
    /// Affected authored paths.
    pub affected_paths: Vec<RelativePath>,
    /// Current observation.
    pub state: ProposalState,
}

/// Recorded history origin, not caller-supplied authorship authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryOrigin {
    /// Local human operation.
    Human,
    /// Validated agent context.
    Agent,
    /// External writer of unknown origin.
    ExternalUnknown,
    /// Selected remote integration.
    Remote,
}

/// Signature validity and trust are separate from display authorship.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signature {
    /// Valid and known signer.
    ValidKnown,
    /// Valid but unknown signer.
    ValidUnknown,
    /// Invalid signature.
    Invalid,
    /// No signature, explicitly classified.
    Unsigned,
}

/// Public text-history observation after exposure filtering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    /// Commit identifier.
    pub commit_id: String,
    /// Parent identifiers.
    pub parent_ids: Vec<String>,
    /// Operation identity when known.
    #[serde(deserialize_with = "required_nullable")]
    pub operation_id: Option<Id>,
    /// UTC observation time, not ordering authority.
    pub observed_at: Time,
    /// Recorded origin.
    pub origin: HistoryOrigin,
    /// Author display claim.
    pub author: String,
    /// Committer display claim.
    pub committer: String,
    /// Validated agent context, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub agent: Option<String>,
    /// Validated tool context, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub tool: Option<String>,
    /// Validated session identity, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub session_id: Option<Id>,
    /// Independently verified signature state.
    pub signature: Signature,
    /// Visible affected paths.
    pub affected_paths: Vec<RelativePath>,
}

/// Selected-remote operation outcome, not assumed atomic with local writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOutcome {
    /// Fetch observed.
    Fetched,
    /// Integration observed.
    Integrated,
    /// Push observed.
    Pushed,
    /// Already current.
    UpToDate,
    /// Integration proposal awaits confirmation.
    Proposal,
    /// Remote offline.
    Offline,
    /// Credential helper action required.
    AuthRequired,
    /// Operation refused.
    Refused,
    /// Remote effect requires inspection.
    Indeterminate,
}

/// Inspectable selected-remote outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncResult {
    /// Idempotency identity.
    pub operation_id: Id,
    /// Observed outcome.
    pub state: SyncOutcome,
    /// Observed local head.
    #[serde(deserialize_with = "required_nullable")]
    pub local_head: Option<String>,
    /// Observed remote head.
    #[serde(deserialize_with = "required_nullable")]
    pub remote_head: Option<String>,
    /// Integration proposal, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub proposal: Option<Proposal>,
    /// Bounded redacted diagnostic.
    pub message: String,
}

/// Native ZIP content preset; EPUB is not a fourth preset.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackagePreset {
    /// Read-only frozen library.
    Frozen,
    /// Backup selection.
    Backup,
    /// Migration selection.
    Migration,
}

/// Whether selected originals remain references or contribute selected bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginalSelection {
    /// References only.
    References,
    /// Explicitly selected original bytes.
    SelectedBytes,
}

/// Native/foreign package selection; empty selections never imply recursive copy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSelection {
    /// Native content preset.
    pub preset: PackagePreset,
    /// Explicit source selection.
    pub source_ids: Vec<Id>,
    /// Explicit authored page selection.
    pub page_paths: Vec<RelativePath>,
    /// Original-byte policy.
    pub originals: OriginalSelection,
    /// Include compatible indexes.
    pub include_indexes: bool,
    /// Include text history.
    pub include_history: bool,
    /// Include journal.
    pub include_journal: bool,
    /// Require complete coverage rather than silently omit bytes.
    pub require_complete: bool,
}

/// Ordered EPUB pages and explicitly selected contained media.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpubSelection {
    /// Ordered Markdown/HTML pages.
    pub page_paths: Vec<RelativePath>,
    /// Explicit contained-media selection; empty means no media bytes.
    pub media_source_ids: Vec<Id>,
}

/// Exactly the owner-declared selection union, with no extra wire discriminator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PlanSelection {
    /// Native/foreign package selection.
    Package(PackageSelection),
    /// EPUB publication selection.
    Epub(EpubSelection),
}

/// Public plan format; foreign formats do not share native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageFormat {
    /// Native searchable ZIP/ZIP64.
    NativeZip,
    /// Genuine foreign v1 adapter.
    OpenkitV1,
    /// EPUB publication.
    Epub,
}

/// Plan effect category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageAction {
    /// Export to an approved destination.
    Export,
    /// Restore with no-clobber materialization.
    Restore,
}

/// Frozen selection/destination plan data; a plan ID is not effect consent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackagePlan {
    /// Plan identity.
    pub plan_id: Id,
    /// Selected library identity.
    pub library_id: Id,
    /// Native, foreign, or publication format.
    pub format: PackageFormat,
    /// Export/restore action.
    pub action: PackageAction,
    /// Format-appropriate owner-declared selection.
    pub selection: PlanSelection,
    /// Estimated selected bytes.
    pub estimated_bytes: U64,
    /// References whose bytes are external.
    pub external_reference_count: U64,
    /// Uncovered selection count.
    pub uncovered_count: U64,
    /// Truthful coverage.
    pub coverage: Coverage,
    /// Approved output location, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub destination: Option<String>,
    /// No-clobber conflicts.
    pub conflicts: Vec<RelativePath>,
    /// Bounded warnings.
    pub warnings: Vec<String>,
}

/// Inspectable package outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageState {
    /// Inspection only.
    Inspected,
    /// Materialization completed.
    Completed,
    /// Conflicts preserved.
    Conflict,
    /// Outcome requires inspection.
    Indeterminate,
    /// Genuine foreign context/compatibility unavailable.
    CompatibilityRefused,
}

/// Entry verification level, separate from signer trust.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Integrity {
    /// Manifest only.
    ManifestOnly,
    /// Selected entries verified.
    SelectedEntriesVerified,
    /// All entries verified.
    AllEntriesVerified,
}

/// Public archive/publication outcome; EPUB has no native snapshot ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageResult {
    /// Original effect identity, if any.
    #[serde(deserialize_with = "required_nullable")]
    pub operation_id: Option<Id>,
    /// Observed outcome.
    pub state: PackageState,
    /// Native snapshot digest, if applicable.
    #[serde(deserialize_with = "required_nullable")]
    pub snapshot_id: Option<Digest>,
    /// Selected output/archive path as data.
    #[serde(deserialize_with = "required_nullable")]
    pub path: Option<String>,
    /// Materialized library identity, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub library_id: Option<Id>,
    /// Signature validity/trust.
    pub signature: Signature,
    /// Actual entry verification level.
    pub integrity: Integrity,
    /// Truthful coverage.
    pub coverage: Coverage,
    /// Indexes intentionally not materialized.
    pub omitted_indexes: Vec<String>,
    /// Bounded warnings.
    pub warnings: Vec<String>,
}

/// One collision-checked migration path mapping.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathMapping {
    /// Old authored path.
    pub from: RelativePath,
    /// Proposed authored path.
    pub to: RelativePath,
}

/// Preview of a non-destructive layout migration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlan {
    /// Plan identity.
    pub plan_id: Id,
    /// Selected library identity.
    pub library_id: Id,
    /// Old manifest version.
    pub from_version: SafeInteger,
    /// New manifest version.
    pub to_version: SafeInteger,
    /// Proposed path changes.
    pub path_map: Vec<PathMapping>,
    /// Authored orphan annotations retained by migration.
    pub orphan_annotation_paths: Vec<RelativePath>,
    /// No-clobber conflicts.
    pub conflicts: Vec<RelativePath>,
    /// Bounded warnings.
    pub warnings: Vec<String>,
}

/// Provider observation, not a credential or authorization state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    /// Ready for admitted work.
    Ready,
    /// Unavailable.
    Unavailable,
    /// Model identity incompatible.
    Incompatible,
    /// Not configured.
    NotConfigured,
}

/// Signing observation; unsigned is never an implicit fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerState {
    /// Signing ready.
    Ready,
    /// Key helper locked.
    Locked,
    /// Signing material unavailable.
    Missing,
    /// Explicit unsigned history.
    UnsignedExplicit,
}

/// Effective process policy, not a new resource pool or a source-access grant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudgets {
    /// Effective process-wide working-memory ceiling in bytes.
    pub memory_budget_bytes: U64,
    /// Effective generated-storage ceiling, further limited by current native free-space/floor checks.
    pub storage_budget_bytes: U64,
    /// Explicit startup memory override, or null for the native default.
    #[serde(deserialize_with = "required_nullable")]
    pub memory_override_bytes: Option<U64>,
    /// Explicit startup storage override, or null for the native default.
    #[serde(deserialize_with = "required_nullable")]
    pub storage_override_bytes: Option<U64>,
    /// Selected caller-visible waiting jobs, or null when that host adapter has not supplied a count.
    #[serde(deserialize_with = "required_nullable")]
    pub waiting_for_memory: Option<U64>,
}

/// Selected-library status data, subject to transport/exposure projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Additive optional effective policy; omission preserves existing version-1 response bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_budgets: Option<ResourceBudgets>,
    /// Public selected-library facts.
    pub library: Library,
    /// Current coverage.
    pub coverage: Coverage,
    /// Admitted job observations.
    pub jobs: Vec<Job>,
    /// Published generation, if present.
    #[serde(deserialize_with = "required_nullable")]
    pub generation_id: Option<Id>,
    /// External cache location as data.
    pub cache_path: String,
    /// External history location, if initialized.
    #[serde(deserialize_with = "required_nullable")]
    pub history_path: Option<String>,
    /// Observed cache usage.
    pub cache_used_bytes: U64,
    /// Explicit cache budget.
    pub cache_budget_bytes: U64,
    /// Provider observation.
    pub provider: ProviderState,
    /// Signing observation.
    pub signer: SignerState,
    /// Operations requiring inspection.
    pub recovery_operation_ids: Vec<Id>,
    /// Bounded redacted warnings.
    pub warnings: Vec<String>,
}

/// Explicit transfer effect; never inferred from a digest match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferMode {
    /// Copy without deleting the original.
    Copy,
    /// Explicit selected move.
    Move,
}

/// Reconciliation verification policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    /// Cheap stat reconciliation before necessary hashes.
    StatFirst,
    /// Explicit full selected hashing.
    FullHash,
}

/// Selected bounded processor recipe, not a shell command.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessRecipe {
    /// Text extraction.
    Extract,
    /// Image/video representation processing.
    Media,
    /// Speech recognition.
    Asr,
    /// Embedding.
    Embed,
}

/// Minimal selected-remote action, not general Git administration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    /// Fetch selected remote.
    Fetch,
    /// Prepare/perform inspected integration.
    Integrate,
    /// Push within explicit consent.
    Push,
}

/// Archive inspection extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageVerification {
    /// Verify manifest.
    Manifest,
    /// Verify all entries.
    AllEntries,
}

/// Explicit restore location mapping; imported data cannot grant root access.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountMapping {
    /// Referenced mount identity.
    pub mount_id: Id,
    /// Separately approved local location.
    pub location: String,
}

/// Owner-declared command union; no variant is a caller capability or handler.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// List only already admitted libraries when executed by a service.
    LibraryList {},
    /// Owner-selected local open; decoding does not open a filesystem path.
    LibraryOpen {
        /// Explicit selected path.
        path: String,
    },
    /// Owner-selected no-clobber library creation.
    LibraryCreate {
        /// Explicit selected destination.
        path: String,
        /// Display name.
        name: String,
        /// Explicit signing choice.
        history_mode: CreationHistoryMode,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Owner-selected remote clone, without backend work in this crate.
    LibraryClone {
        /// Credential-free selected remote.
        remote_url: String,
        /// Explicit local working destination.
        destination: String,
        /// Display name.
        name: String,
        /// Explicit signing choice.
        history_mode: CreationHistoryMode,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Inspect admitted library facts.
    LibraryStatus {
        /// Selected library.
        library_id: Id,
    },
    /// Owner-only mount attachment; data arguments are not approval.
    MountAttach {
        /// Selected library.
        library_id: Id,
        /// Logical mount name.
        name: String,
        /// Known backend.
        backend: MountBackend,
        /// Selected OS path or credential-free Git URL.
        location: String,
        /// Local working-copy destination for Git; otherwise null.
        #[serde(deserialize_with = "required_nullable")]
        destination: Option<String>,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Owner-only detach that never implies original deletion.
    MountDetach {
        /// Selected library.
        library_id: Id,
        /// Selected mount.
        mount_id: Id,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// List admitted mounts.
    MountList {
        /// Selected library.
        library_id: Id,
    },
    /// Browse one admitted root/child with an opaque observation cursor.
    Browse {
        /// Selected library.
        library_id: Id,
        /// Null selects home; an ID selects an admitted mount.
        #[serde(deserialize_with = "required_nullable")]
        mount_id: Option<Id>,
        /// Empty denotes only the already selected root.
        path: RootRelativePath,
        /// Opaque continuation, never permission.
        #[serde(deserialize_with = "required_nullable")]
        cursor: Option<String>,
        /// Limit from 1 through 1,000.
        limit: UnsignedInteger,
    },
    /// Non-hashing selected-file inspector.
    SourceDetails {
        /// Selected source.
        source: SourceRef,
    },
    /// Explicit bounded resident-source verification, not initialization or write.
    VerifySource {
        /// Selected source.
        source: SourceRef,
        /// Explicit bounded hashing work.
        budget: Budget,
    },
    /// Bounded byte read requiring complete verified version evidence.
    ReadRange {
        /// Selected source.
        source: SourceRef,
        /// Expected verified version.
        expected: ExpectedVersion,
        /// Zero-based byte offset.
        offset: U64,
        /// Length from 1 through 524,288.
        length: UnsignedInteger,
    },
    /// Bounded derived-representation line read.
    ReadRepresentation {
        /// Selected original source.
        source: SourceRef,
        /// Exact selected representation.
        representation_digest: Digest,
        /// First one-based representation line.
        start_line: PositiveInteger,
        /// Limit from 1 through 20,000.
        max_lines: UnsignedInteger,
    },
    /// Expected-annotation mutation, never blind original replacement.
    Annotate {
        /// Selected source.
        source: SourceRef,
        /// Null means no previous annotation, not unconditional overwrite.
        #[serde(deserialize_with = "required_nullable")]
        expected_annotation: Option<ExpectedVersion>,
        /// Authored exact tags.
        tags: Vec<String>,
        /// Authored description.
        description: String,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Authored text mutation; owner confirmation/proposal policy is not here.
    EditText {
        /// Nonempty admitted write target.
        target: Target,
        /// Null means exclusive new-file creation.
        #[serde(deserialize_with = "required_nullable")]
        expected: Option<ExpectedVersion>,
        /// Inline UTF-8 text bounded by the frame.
        text: String,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Copy an already admitted input, never a model-supplied external path.
    ImportFile {
        /// Selected library.
        library_id: Id,
        /// Already admitted source.
        input: SourceRef,
        /// No-clobber admitted destination.
        destination: Target,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Explicit selected transfer preserving annotations.
    Transfer {
        /// Selected source.
        source: SourceRef,
        /// Expected verified original.
        expected: ExpectedVersion,
        /// No-clobber admitted destination.
        destination: Target,
        /// Selected transfer effect.
        mode: TransferMode,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Owner-confirmed selected Trash effect, not permanent deletion.
    Trash {
        /// Selected source.
        source: SourceRef,
        /// Expected verified original.
        expected: ExpectedVersion,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Explicit selected hydration; an empty selection is not recursive.
    Hydrate {
        /// Selected library.
        library_id: Id,
        /// Explicit selected originals.
        source_ids: Vec<Id>,
        /// Requested offline availability policy.
        make_available_offline: bool,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Bounded reconciliation/processing request.
    Refresh {
        /// Selected library.
        library_id: Id,
        /// Selected mounts; empty selects current admitted mounts only.
        mount_ids: Vec<Id>,
        /// Verification policy.
        verification: Verification,
        /// Whether embedding work was selected.
        embeddings: bool,
        /// Explicit work bounds.
        budget: Budget,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Set source pin priority, without implied hydration.
    Pin {
        /// Selected library.
        library_id: Id,
        /// Explicit selected sources.
        source_ids: Vec<Id>,
        /// Desired pin setting.
        pinned: bool,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Selected bounded derivation, not an arbitrary tool command.
    Process {
        /// Selected library.
        library_id: Id,
        /// Explicit selected sources.
        source_ids: Vec<Id>,
        /// Known processing recipe.
        recipe: ProcessRecipe,
        /// Explicit work bounds.
        budget: Budget,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Query selected libraries without rewriting caller strings.
    Query {
        /// Explicit shared search request.
        query: SearchRequest,
    },
    /// Inspect an admitted job.
    JobStatus {
        /// Job identity, not authority.
        job_id: Id,
    },
    /// Request cancellation, never assert rollback.
    Cancel {
        /// Selected job.
        job_id: Id,
        /// Cancellation operation identity.
        operation_id: Id,
    },
    /// Inspect an uncertain operation before any replay.
    OperationStatus {
        /// Original idempotency identity.
        operation_id: Id,
        /// Library scope, or null for service creation scope.
        #[serde(deserialize_with = "required_nullable")]
        library_id: Option<Id>,
    },
    /// Bounded visible history listing.
    HistoryList {
        /// Selected library.
        library_id: Id,
        /// Opaque continuation.
        #[serde(deserialize_with = "required_nullable")]
        cursor: Option<String>,
        /// Limit from 1 through 1,000.
        limit: UnsignedInteger,
    },
    /// Bounded canonical UTF-8 diff bytes.
    HistoryDiff {
        /// Selected library.
        library_id: Id,
        /// Exact base commit.
        base_commit_id: String,
        /// Exact candidate commit.
        candidate_commit_id: String,
        /// Byte offset, not Unicode character offset.
        offset: U64,
        /// Length from 1 through 524,288.
        length: UnsignedInteger,
    },
    /// Inspect minimal admitted proposals.
    ProposalList {
        /// Selected library.
        library_id: Id,
    },
    /// Bounded exact-proposal diff bytes.
    ProposalDiff {
        /// Selected library.
        library_id: Id,
        /// Selected proposal.
        proposal_id: Id,
        /// Canonical UTF-8 byte offset.
        offset: U64,
        /// Length from 1 through 524,288.
        length: UnsignedInteger,
    },
    /// Exact owner-confirmation tuple; parsing never confirms anything.
    ConfirmMerge {
        /// Selected library.
        library_id: Id,
        /// Exact proposal.
        proposal_id: Id,
        /// Expected base commit.
        base_commit_id: String,
        /// Complete canonical diff digest.
        diff_digest: Digest,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Minimal selected-remote operation.
    Sync {
        /// Selected library.
        library_id: Id,
        /// Selected effect.
        action: SyncAction,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Owner-selected schedule data, not portable remote consent.
    SyncSchedule {
        /// Selected library.
        library_id: Id,
        /// Enable or disable scheduling.
        enabled: bool,
        /// Positive when enabled; retained configured interval when disabled.
        interval_minutes: UnsignedInteger,
        /// Whether selected schedule permits push.
        allow_push: bool,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Plan selected EPUB publication without implicit media collection.
    EpubPlan {
        /// Selected library.
        library_id: Id,
        /// Nonempty ordered selected pages.
        page_paths: Vec<RelativePath>,
        /// Explicit contained-media selection.
        media_source_ids: Vec<Id>,
        /// Approved output location as data, not consent itself.
        destination: String,
        /// Explicit work bounds.
        budget: Budget,
    },
    /// Apply an exact selected EPUB plan with no-clobber semantics.
    EpubExport {
        /// Selected library.
        library_id: Id,
        /// Exact EPUB plan.
        plan_id: Id,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Plan selected native/foreign package export.
    SnapshotPlan {
        /// Selected library.
        library_id: Id,
        /// Explicit package selection.
        selection: PackageSelection,
        /// Approved output location as data.
        destination: String,
    },
    /// Apply selected native/foreign export plan, never an EPUB plan.
    SnapshotCreate {
        /// Selected library.
        library_id: Id,
        /// Exact export plan.
        plan_id: Id,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Inspect a separately owner-selected archive without declared-root contact.
    PackageInspect {
        /// Selected archive path as data.
        path: String,
        /// Verification extent.
        verification: PackageVerification,
    },
    /// Open a separately selected read-only frozen archive.
    OpenFrozen {
        /// Selected archive path as data.
        path: String,
    },
    /// Plan no-clobber restore with explicit mount remapping.
    RestorePlan {
        /// Selected archive path as data.
        path: String,
        /// Selected new home destination as data.
        destination: String,
        /// Explicit remapping; imported entries cannot authorize paths.
        mount_map: Vec<MountMapping>,
    },
    /// Apply an exact native/foreign restore plan.
    RestoreApply {
        /// Exact restore plan.
        plan_id: Id,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Preview non-destructive migration.
    MigrationPlan {
        /// Selected library.
        library_id: Id,
    },
    /// Apply an exact migration plan without destroying old authored data.
    MigrationApply {
        /// Selected library.
        library_id: Id,
        /// Exact migration plan.
        plan_id: Id,
        /// Idempotency identity.
        operation_id: Id,
    },
    /// Plan genuine foreign v1 core-content import.
    #[serde(rename = "v1_import_plan")]
    V1ImportPlan {
        /// Selected foreign source path as data.
        path: String,
        /// Selected destination as data.
        destination: String,
    },
    /// Plan genuine foreign v1 export or explicit compatibility refusal.
    #[serde(rename = "v1_export_plan")]
    V1ExportPlan {
        /// Selected library.
        library_id: Id,
        /// Selected destination as data.
        destination: String,
        /// Genuine separately supplied host context, never fabricated governance.
        #[serde(deserialize_with = "required_nullable")]
        host_context_id: Option<Id>,
    },
    /// Owner-only exposure inspection; never an ordinary model tool.
    ExposureGet {
        /// Selected library.
        library_id: Id,
    },
    /// Owner-only revision-bound exposure administration; not self-approval.
    ExposureSet {
        /// Selected library.
        library_id: Id,
        /// Expected exposure revision.
        expected_revision: Id,
        /// Requested exposure mode.
        mode: ExposureMode,
        /// Requested sensitivity classification.
        sensitive: bool,
        /// Explicit excluded normalized paths.
        excluded_paths: Vec<RelativePath>,
        /// Requested transports, bounded by local approvals.
        transports: Vec<Transport>,
        /// Idempotency identity.
        operation_id: Id,
    },
}

/// Listing entry kind, not a promise that bytes remain available.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Directory entry.
    Directory,
    /// Regular admitted file entry.
    File,
}

/// One current browse observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListingEntry {
    /// Entry display name.
    pub name: String,
    /// Nonempty normalized child path.
    pub path: RelativePath,
    /// File/directory observation.
    pub kind: EntryKind,
    /// Source facts if assigned, not automatic verification.
    #[serde(deserialize_with = "required_nullable")]
    pub source: Option<Source>,
}

/// Cancellation acknowledgment, not rollback of completed effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelState {
    /// Cancellation requested.
    Requested,
    /// Job already terminal.
    AlreadyTerminal,
}

/// Inspectable operation lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    /// Still pending; outcome is null.
    Pending,
    /// Original terminal outcome available.
    Completed,
    /// Requires inspection before replay.
    Indeterminate,
}

/// Exact tagged result union. Boxes change Rust storage, never JSON shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Result {
    /// Admitted library list.
    Libraries {
        /// Visible libraries.
        libraries: Vec<Library>,
    },
    /// Selected library facts.
    Library {
        /// Public library observation.
        library: Box<Library>,
    },
    /// Selected-library status.
    Status {
        /// Projected admitted status, not an owner capability.
        status: Box<Status>,
    },
    /// Admitted mount observations.
    Mounts {
        /// Visible mounts.
        mounts: Vec<Mount>,
    },
    /// Current bounded browse page.
    Listing {
        /// Current entries.
        entries: Vec<ListingEntry>,
        /// Opaque continuation, or null.
        #[serde(deserialize_with = "required_nullable")]
        next_cursor: Option<String>,
        /// Whether enumeration completed.
        complete: bool,
    },
    /// Selected source observation.
    Source {
        /// Verified or explicitly unverified source facts.
        source: Box<Source>,
    },
    /// Selected source observations.
    Sources {
        /// Visible sources.
        sources: Vec<Source>,
    },
    /// Bounded admitted original/representation bytes.
    Content {
        /// Original source identity.
        source: SourceRef,
        /// Current verified original digest.
        source_digest: Digest,
        /// Representation digest for derived content.
        #[serde(deserialize_with = "required_nullable")]
        representation_digest: Option<Digest>,
        /// Actual provenance coordinate.
        locator: Locator,
        /// Base64-encoded bytes; no raw media body field.
        bytes_base64: String,
        /// Whether continuation is required.
        truncated: bool,
        /// Next byte for range reads; otherwise null.
        #[serde(deserialize_with = "required_nullable")]
        next_offset: Option<U64>,
        /// Next line for representation reads; otherwise null.
        #[serde(deserialize_with = "required_nullable")]
        next_line: Option<PositiveInteger>,
    },
    /// Shared query observations.
    Search {
        /// Applied request and truthful library groups.
        search: Box<SearchResult>,
    },
    /// Inspectable job state.
    Job {
        /// Admitted job observation.
        job: Box<Job>,
    },
    /// Cancellation acknowledgment only.
    Cancelled {
        /// Selected job.
        job_id: Id,
        /// Request acknowledgment.
        state: CancelState,
    },
    /// Inspection of an uncertain or completed operation.
    Operation {
        /// Original idempotency identity.
        operation_id: Id,
        /// Current lifecycle observation.
        state: OperationState,
        /// Original terminal result, never another operation envelope.
        #[serde(deserialize_with = "required_nullable")]
        outcome: Option<Box<Result>>,
        /// Current job, if present.
        #[serde(deserialize_with = "required_nullable")]
        job: Option<Box<Job>>,
    },
    /// Inspectable authored write outcome.
    Write {
        /// Local write/proposal/recovery facts.
        write: Box<WriteResult>,
    },
    /// Bounded visible history page.
    History {
        /// Visible commits.
        entries: Vec<HistoryEntry>,
        /// Opaque continuation, or null.
        #[serde(deserialize_with = "required_nullable")]
        next_cursor: Option<String>,
    },
    /// Bounded canonical UTF-8 diff bytes.
    Diff {
        /// Base commit.
        base_commit_id: String,
        /// Candidate commit.
        candidate_commit_id: String,
        /// Digest of complete canonical diff, not this preview alone.
        diff_digest: Digest,
        /// Current bounded UTF-8 diff range.
        unified_diff: String,
        /// Current canonical byte offset.
        offset: U64,
        /// Next canonical byte offset, or null.
        #[serde(deserialize_with = "required_nullable")]
        next_offset: Option<U64>,
        /// Whether continuation is required.
        truncated: bool,
    },
    /// Minimal admitted proposal observations.
    Proposals {
        /// Visible proposals.
        proposals: Vec<Proposal>,
    },
    /// Selected-remote outcome.
    Sync {
        /// Inspectable remote/local outcome.
        sync: Box<SyncResult>,
    },
    /// Format-specific selected package/publication plan.
    PackagePlan {
        /// Frozen plan facts, not destination consent.
        plan: Box<PackagePlan>,
    },
    /// Non-destructive migration preview.
    MigrationPlan {
        /// Exact migration plan facts.
        plan: Box<MigrationPlan>,
    },
    /// Archive/publication inspection or effect outcome.
    Package {
        /// Actual integrity, trust, and coverage.
        package: Box<PackageResult>,
    },
    /// Owner-projected exposure facts.
    Exposure {
        /// Exposure data, not an imported local approval.
        exposure: Box<Exposure>,
    },
    /// Common precondition/refusal result.
    Error {
        /// Classified bounded redacted diagnostic.
        error: ServiceError,
    },
}

/// Stable classified error code; diagnostic message wording is not an oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Malformed/invalid request.
    InvalidRequest,
    /// Unsupported wire version.
    UnsupportedVersion,
    /// Admitted target not found.
    NotFound,
    /// Non-disclosing refusal for unexposed targets.
    NotExposed,
    /// Read-only admission.
    ReadOnly,
    /// Local owner capability required.
    OwnerRequired,
    /// Exact human confirmation required.
    ConfirmationRequired,
    /// Target excluded from admission.
    Excluded,
    /// Sensitive data forbidden on this transport.
    SensitiveTransport,
    /// Dependency offline.
    Offline,
    /// Access denied.
    Denied,
    /// Nonresident placeholder.
    Placeholder,
    /// Residency unknown and not authorized for read.
    ResidencyUnknown,
    /// Source/version changed.
    Changed,
    /// Verified version evidence absent.
    Unverified,
    /// Competing versions preserved.
    Conflict,
    /// Effect outcome uncertain; inspect before replay.
    Indeterminate,
    /// Busy resource.
    Busy,
    /// Idempotency identity reused with different arguments.
    OperationMismatch,
    /// Cancellation observed.
    Cancelled,
    /// Explicit work/delivery bound exhausted.
    BudgetExhausted,
    /// Unsupported operation/format/capability.
    Unsupported,
    /// Provider unavailable.
    ProviderUnavailable,
    /// Provider/model identity mismatch.
    ProviderIdentity,
    /// Invalid vector response.
    InvalidVector,
    /// Required signing capability absent.
    SigningRequired,
    /// Remote credential helper action needed.
    RemoteAuth,
    /// Remote effect refused.
    RemoteRefused,
    /// Invalid archive/package.
    InvalidPackage,
    /// Digest/integrity verification failed.
    IntegrityFailed,
    /// Index is not directly compatible.
    IncompatibleIndex,
    /// Genuine foreign-format context unavailable.
    CompatibilityRefused,
    /// Inspectable recovery required.
    RecoveryRequired,
    /// Internal failure, with no input or credential disclosure.
    Internal,
}

/// Typed service/domain error data, not a codec exception or replay permission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceError {
    /// Classified refusal/failure.
    pub code: ErrorCode,
    /// Bounded redacted message; not stable wording or a raw input echo.
    pub message: String,
    /// Scheduling hint, not permission to repeat an effect.
    pub retryable: bool,
    /// Original operation identity, if known.
    #[serde(deserialize_with = "required_nullable")]
    pub operation_id: Option<Id>,
    /// Visible library identity, if permitted.
    #[serde(deserialize_with = "required_nullable")]
    pub library_id: Option<Id>,
    /// Visible source identity, if permitted.
    #[serde(deserialize_with = "required_nullable")]
    pub source_id: Option<Id>,
    /// Visible job identity, if permitted.
    #[serde(deserialize_with = "required_nullable")]
    pub job_id: Option<Id>,
    /// Selected method that failed, if applicable.
    #[serde(deserialize_with = "required_nullable")]
    pub method: Option<SearchMethod>,
    /// Visible inspectable competing paths, never excluded paths.
    pub recovery_paths: Vec<RelativePath>,
}
