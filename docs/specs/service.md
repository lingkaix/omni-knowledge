# Service And Public Wire Contract

Status: Accepted full-version boundary, 2026-11-04; not an implementation report.
This document owns the version-1 JSON DTOs, operations, transport behavior, exposure, and human-confirmation boundary.
[Product](product.md) owns scope and platform qualification.
Domain owners govern storage, search, history, processing, and packages; no transport can relax their safety rules.
The declarative types below are the admitted public schema, not candidate implementation pseudocode.
Rust mirrors use the stated PascalCase type names and snake_case serialized names.

## Encoding And Compatibility

All commands and results use UTF-8 JSON with `wire_version: 1`.
Fields shown without `?` are required, including nullable fields; `?` means omission is permitted.
Unknown command fields, operations, enum tags, required features, and wire versions fail before effects.
Clients may ignore unknown optional response fields, but must refuse unknown result variants rather than assume success.
A breaking field or semantic change requires a new wire version; additive optional response fields do not.
IDs are opaque nonempty strings, not authority, paths, hashes, or caller-selected human identities.
`Digest` is lowercase hexadecimal SHA-256, exactly 64 characters.
`U64` is a canonical unsigned decimal string, including `"0"`, with no leading zeroes, bounded by 18446744073709551615.
This avoids JSON integer loss for large offsets, lengths, and sequence values.
Other integers are JSON safe integers; timestamps are UTC RFC 3339 strings and never ordering authority.
`RelativePath` uses normalized `/` separators with no absolute root, empty segment, dot segment, NUL, or traversal.
Paths identify data only inside an already admitted library/mount; a request cannot introduce a new root through a path field.
Non-UTF-8 filenames return `unsupported` with explicit coverage, not lossy aliases.
Strings, arrays, query results, byte ranges, and frames are bounded; exceeding a limit returns `budget_exhausted`, not truncation disguised as completion.
A control frame is at most 1 MiB; `read_range` returns at most 512 KiB of bytes per call, base64 encoded.
Larger bodies use repeated bounded ranges or admitted local handles inside the service, never an unbounded media JSON frame.

```typescript
type Id = string;
type Digest = string;
type U64 = string;
type Time = string;
type RelativePath = string;
type Availability = "available" | "offline" | "denied" | "placeholder" | "unknown" | "changed" | "unsupported" | "deleted";
type Request = { wire_version: 1; request_id: Id; command: Command };
type Response = { wire_version: 1; request_id: Id; result: Result };
type Budget = { max_files: number; max_bytes: U64; max_millis: number; max_memory_bytes: U64 };
type ExpectedVersion = {
  digest: Digest; size_bytes: U64; modified_ns: string | null; changed_ns: string | null;
  file_id: string | null; backend_version: string | null;
};
type SourceRef = { library_id: Id; source_id: Id };
type Target = { library_id: Id; mount_id: Id | null; path: RelativePath };
type Locator =
  | { type: "original_lines"; start_line: number; end_line: number; start_byte: U64 | null; end_byte: U64 | null }
  | { type: "representation_lines"; representation_digest: Digest; start_line: number; end_line: number }
  | { type: "page"; representation_digest: Digest; page: number; start_line: number | null; end_line: number | null }
  | { type: "slide"; representation_digest: Digest; slide: number }
  | { type: "sheet"; representation_digest: Digest; sheet: string; cell_range: string | null }
  | { type: "time"; start_ms: U64; end_ms: U64; representation_digest: Digest | null }
  | { type: "unlocated" };
type Coverage = {
  discovered: U64; verified: U64; pending_hash: U64; pending_extraction: U64; pending_embedding: U64;
  stale: U64; unavailable: U64; unsupported: U64; failed: U64; excluded: U64;
  enumeration: "complete" | "partial" | "not_started";
  result_extent: "enumeration" | "top_k"; stopped_reason: string | null;
};
type Exposure = {
  mode: "off" | "read_only" | "read_write"; sensitive: boolean; excluded_paths: RelativePath[];
  transports: ("owner_ipc" | "owner_stdio" | "http")[]; revision: Id; pending_expansion: boolean;
};
type Library = {
  library_id: Id; name: string; home_path: string; text_root: "pages" | "wiki";
  manifest_version: number | null; state: "unmarked" | "ready" | "migration_required" | "recovery_required" | "frozen";
  history_mode: "signed" | "unsigned_explicit" | "not_initialized";
  exposure: Exposure; mounts: Mount[];
};
type Mount = {
  mount_id: Id; name: string; backend: "local" | "nas" | "cloud_folder" | "git";
  location: string; availability: Availability; writable: boolean;
  capabilities: { range_read: boolean; stable_file_id: boolean; reliable_watch: boolean; conditional_write: boolean; residency: boolean };
  last_complete_scan: Time | null; reconciliation: "startup_periodic" | "nightly_manual" | "manual";
};
type Source = {
  library_id: Id; source_id: Id; mount_id: Id | null; path: RelativePath; logical_path: RelativePath;
  title: string; description: string; tags: string[]; annotation_path: RelativePath | null;
  digest: Digest | null; digest_state: "verified" | "unverified"; version: ExpectedVersion | null;
  size_bytes: U64 | null; media_type: string | null; created_at: Time | null; modified_at: Time | null;
  availability: Availability; residency: "resident" | "placeholder" | "unknown";
  activity: "stable" | "writing"; conflict_source_id: Id | null;
};
```

Line, page, and slide numbers are 1-based and inclusive; byte ranges are zero-based and half-open.
Time ranges are milliseconds in the original recording, never a reset clip clock.
Zero-to-zero imported transcript timing means no known interval, not proof that speech occurs at zero.
`modified_ns` and `changed_ns` are canonical signed decimal timestamps when available; file IDs are backend-qualified evidence.
An unverified source has `digest: null` and `version: null`; it cannot satisfy a digest-required mutation or match claim.
An observed stat snapshot is not a substitute for a verified digest.
`Coverage` counters describe admitted, currently visible sources; excluded content contributes no names, counts, or snippets to an unprivileged caller.
For such a caller, `excluded` is zero rather than a disclosure of excluded inventory size.
Coverage categories may overlap; they are not a partition whose sum proves corpus size.
A page/slide/sheet locator must carry real processor provenance; absence uses representation lines or unlocated evidence, not invented coordinates.

## Search DTOs

```typescript
type SearchMethod = "bm25" | "literal" | "regex" | "files" | "tags" | "semantic";
type SearchRequest = {
  library_ids: Id[]; keywords: string[]; methods: SearchMethod[]; required_tags: string[];
  case_sensitive: boolean; limit: number; budget: Budget;
  image_query: SourceRef | null;
};
type Evidence = {
  method: SearchMethod; keyword: string | null; locator: Locator; snippet: string;
  kind: "verified_match" | "semantic_candidate" | "metadata_observation";
  source_digest: Digest | null; representation_digest: Digest | null; processor_id: string | null;
  model_space: string | null; score: number | null;
};
type FileHit = {
  source: Source; rank: number; score: number | null; evidence: Evidence[];
  retained_evidence_count: U64; useful_location_count: U64;
};
type SearchResult = {
  applied: SearchRequest; groups: {
    library_id: Id; generation_id: Id | null; partitioning_id: Id | null;
    files: FileHit[]; coverage: Coverage; returned_files: number; limit_reached: boolean;
  }[];
};
```

The desktop normalizes ASCII/Chinese comma-separated input into trimmed, nonempty, case-insensitively deduplicated `keywords`.
Keywords are alternatives through independently selected methods; `required_tags` is instead an all-required pre-limit filter.
An empty keyword list with no image query means browsing, not a failed search.
A nonempty query with no methods is `invalid_request`; image queries require semantic capability and admitted source bytes.
A selected method failure returns `error` with `method`; it does not silently drop the method or return partial success.
An interrupted or budget-limited enumeration may return `search` with truthful partial coverage.
The GUI keeps its last successful `applied` request when a later request fails.
`limit` is 1 through 10,000 distinct files; snippets contain at most 500 Unicode characters.
File ranks start at 1; scores are finite numbers, not probabilities or calibrated cross-index BM25 scores.
Multi-library results remain labelled groups; independent partitions use declared rank fusion, not raw-score comparison.
Repeated fragments cannot multiply a file's vote; RRF is `sum(1/(60+rank))` over distinct file ranks per method/keyword list.
Case-insensitive literal search uses escaped line regex; byte-exact literal and LF-line regex preserve kernel semantics.
Exact/regex scans cover all admitted partitions, not only BM25 candidates.
Before returning a hit or byte range, recheck availability, source version, current exposure, and exclusions.
Changed sources are withheld until reconciled; cached semantic evidence cannot certify changed originals.
Cheap unverified filename/tag discovery is labelled `metadata_observation`, never `verified_match`.

## Jobs, Mutations, History, And Package DTOs

```typescript
type Job = {
  job_id: Id; operation_id: Id; library_id: Id; kind: "reconcile" | "process" | "snapshot" | "restore" | "migrate";
  state: "queued" | "running" | "waiting_offline" | "completed" | "partial" | "failed" | "cancelled" | "superseded";
  priority: "normal" | "pinned"; coverage: Coverage; error: ServiceError | null;
};
type WriteResult = {
  operation_id: Id; state: "saved_local" | "proposed" | "conflict" | "indeterminate" | "unchanged";
  source: Source | null; commit_id: string | null; proposal: Proposal | null;
  recovery_paths: RelativePath[]; sync_state: "local_only" | "pending_push" | "pushed" | "not_applicable";
};
type Proposal = {
  proposal_id: Id; library_id: Id; operation_id: Id; kind: "text" | "merge";
  base_commit_id: string; candidate_commit_id: string; diff_digest: Digest;
  affected_paths: RelativePath[]; state: "pending" | "conflicted" | "accepted" | "obsolete";
};
type HistoryEntry = {
  commit_id: string; parent_ids: string[]; operation_id: Id | null; observed_at: Time;
  origin: "human" | "agent" | "external_unknown" | "remote"; author: string; committer: string;
  signature: "valid_known" | "valid_unknown" | "invalid" | "unsigned";
  affected_paths: RelativePath[];
};
type SyncResult = {
  operation_id: Id; state: "fetched" | "integrated" | "pushed" | "up_to_date" | "proposal" | "offline" | "auth_required" | "refused" | "indeterminate";
  local_head: string | null; remote_head: string | null; proposal: Proposal | null; message: string;
};
type PackageSelection = {
  preset: "frozen" | "backup" | "migration"; source_ids: Id[]; page_paths: RelativePath[];
  originals: "references" | "selected_bytes"; include_indexes: boolean; include_history: boolean;
  include_journal: boolean; require_complete: boolean;
};
type PackagePlan = {
  plan_id: Id; library_id: Id; format: "native_zip" | "openkit_v1"; action: "export" | "restore";
  selection: PackageSelection; estimated_bytes: U64;
  external_reference_count: U64; uncovered_count: U64; coverage: Coverage;
  destination: string | null; conflicts: RelativePath[]; warnings: string[];
};
type PackageResult = {
  operation_id: Id | null; state: "inspected" | "completed" | "conflict" | "indeterminate" | "compatibility_refused";
  snapshot_id: Digest | null; path: string | null; library_id: Id | null;
  signature: "valid_known" | "valid_unknown" | "invalid" | "unsigned";
  integrity: "manifest_only" | "selected_entries_verified" | "all_entries_verified";
  coverage: Coverage; omitted_indexes: string[]; warnings: string[];
};
type MigrationPlan = {
  plan_id: Id; library_id: Id; from_version: number; to_version: number;
  path_map: { from: RelativePath; to: RelativePath }[];
  orphan_annotation_paths: RelativePath[]; conflicts: RelativePath[]; warnings: string[];
};
type Status = {
  library: Library; coverage: Coverage; jobs: Job[]; generation_id: Id | null;
  cache_path: string; history_path: string | null; cache_used_bytes: U64; cache_budget_bytes: U64;
  provider: "ready" | "unavailable" | "incompatible" | "not_configured";
  signer: "ready" | "locked" | "missing" | "unsigned_explicit";
  recovery_operation_ids: Id[]; warnings: string[];
};
```

`operation_id` is required for effectful commands and is scoped to the library, or service for creation.
Reusing it with the same normalized command returns the existing outcome/job; different arguments return `operation_mismatch`.
A disconnected or timed-out client must inspect `operation_status` before retrying an uncertain effect.
`request_id` only correlates transport frames; it is not idempotency or authorization.
New source versions supersede obsolete jobs; cancellation never asserts rollback of a completed write or remote effect.
`job_status` reports progress; completed snapshot/restore/migration outcomes are retrieved through `operation_status`.
A terminal job does not imply full coverage: `partial` and classified failures remain visible.
No runtime event stream or workflow engine is required; polling these DTOs is sufficient.

A `saved_local` write binds final source digests and exactly one text-operation commit; `unchanged` creates no empty commit.
A durable file/ref ambiguity returns `indeterminate` with recovery state, not a false saved result.
Raw-original writes default off; unsafe in-place replacement returns `conflict` with preserved versions/proposal.
An agent text write returns `proposed` on a proposal branch instead of advancing the accepted home.
History authorship comes from the service's owner identity and validated agent/tool/session context, never command arguments.
Unsigned mode is explicit and visible; missing signing capability never silently downgrades a signed library.
`sync_state` concerns Git only; a local save does not assert completed cloud synchronization.
Branches are minimal proposals: list, diff, and confirmation only, not general branch administration.
The service may prepare a proposal from diverged remote integration without exposing reset/rebase/branch-delete tools.

## Commands And Result Mapping

The following union freezes operation names and argument field names.
`Owner` in the table below is a service-derived caller capability, never a supplied JSON field.
A path-bearing owner operation uses an explicit user-selected local target; imported declarations cannot authorize contact.

```typescript
type Command =
  | { op: "library_list" }
  | { op: "library_open"; path: string }
  | { op: "library_create"; path: string; name: string; history_mode: "signed" | "unsigned_explicit"; operation_id: Id }
  | { op: "library_status"; library_id: Id }
  | { op: "mount_attach"; library_id: Id; name: string; backend: "local" | "nas" | "cloud_folder" | "git"; location: string; operation_id: Id }
  | { op: "mount_detach"; library_id: Id; mount_id: Id; operation_id: Id }
  | { op: "mount_list"; library_id: Id }
  | { op: "browse"; library_id: Id; mount_id: Id | null; path: RelativePath; cursor: string | null; limit: number }
  | { op: "source_details"; source: SourceRef }
  | { op: "read_range"; source: SourceRef; expected: ExpectedVersion; offset: U64; length: number }
  | { op: "read_representation"; source: SourceRef; representation_digest: Digest; start_line: number; max_lines: number }
  | { op: "annotate"; source: SourceRef; expected_annotation: ExpectedVersion | null; tags: string[]; description: string; operation_id: Id }
  | { op: "edit_text"; target: Target; expected: ExpectedVersion | null; text: string; operation_id: Id }
  | { op: "import_file"; library_id: Id; input: SourceRef; destination: Target; operation_id: Id }
  | { op: "transfer"; source: SourceRef; expected: ExpectedVersion; destination: Target; mode: "copy" | "move"; operation_id: Id }
  | { op: "trash"; source: SourceRef; expected: ExpectedVersion; operation_id: Id }
  | { op: "hydrate"; library_id: Id; source_ids: Id[]; make_available_offline: boolean; operation_id: Id }
  | { op: "refresh"; library_id: Id; mount_ids: Id[]; verification: "stat_first" | "full_hash"; embeddings: boolean; budget: Budget; operation_id: Id }
  | { op: "pin"; library_id: Id; source_ids: Id[]; pinned: boolean; operation_id: Id }
  | { op: "process"; library_id: Id; source_ids: Id[]; recipe: "extract" | "media" | "asr" | "embed"; budget: Budget; operation_id: Id }
  | { op: "query"; query: SearchRequest }
  | { op: "job_status"; job_id: Id }
  | { op: "cancel"; job_id: Id; operation_id: Id }
  | { op: "operation_status"; operation_id: Id; library_id: Id | null }
  | { op: "history_list"; library_id: Id; cursor: string | null; limit: number }
  | { op: "history_diff"; library_id: Id; base_commit_id: string; candidate_commit_id: string; offset: U64; length: number }
  | { op: "proposal_list"; library_id: Id }
  | { op: "proposal_diff"; library_id: Id; proposal_id: Id; offset: U64; length: number }
  | { op: "confirm_merge"; library_id: Id; proposal_id: Id; base_commit_id: string; diff_digest: Digest; operation_id: Id }
  | { op: "sync"; library_id: Id; action: "fetch" | "integrate" | "push"; operation_id: Id }
  | { op: "sync_schedule"; library_id: Id; enabled: boolean; interval_minutes: number; allow_push: boolean; operation_id: Id }
  | { op: "snapshot_plan"; library_id: Id; selection: PackageSelection; destination: string }
  | { op: "snapshot_create"; library_id: Id; plan_id: Id; operation_id: Id }
  | { op: "package_inspect"; path: string; verification: "manifest" | "all_entries" }
  | { op: "open_frozen"; path: string }
  | { op: "restore_plan"; path: string; destination: string; mount_map: { mount_id: Id; location: string }[] }
  | { op: "restore_apply"; plan_id: Id; operation_id: Id }
  | { op: "migration_plan"; library_id: Id }
  | { op: "migration_apply"; library_id: Id; plan_id: Id; operation_id: Id }
  | { op: "v1_import_plan"; path: string; destination: string }
  | { op: "v1_export_plan"; library_id: Id; destination: string; host_context_id: Id | null }
  | { op: "exposure_get"; library_id: Id }
  | { op: "exposure_set"; library_id: Id; expected_revision: Id; mode: "off" | "read_only" | "read_write"; sensitive: boolean; excluded_paths: RelativePath[]; transports: ("owner_ipc" | "owner_stdio" | "http")[]; operation_id: Id };
```

An `expected: null` text edit means exclusive new-file creation, not blind replacement.
`expected_annotation: null` means no existing annotation; existing page/pointer annotations require their own expected bytes, not the original media digest.
`import_file` only reads an admitted source; attaching an external input is owner-only and cannot be smuggled in as a filesystem string.
`transfer` preserves annotations and rejects destination collisions; a cross-mount move is an explicit operation, not digest-based identity guessing.
Empty source selections never mean recursive hydration or whole-corpus copying.
`refresh` with no mounts selects currently admitted library mounts, not newly discovered roots.
Plans bind their selection, destination, captured versions, and conflicts; applying a stale plan refuses without overwriting.
`host_context_id` refers to genuine context already supplied by an authorized host adapter; it is not user-invented governance JSON.
The v1 export plan returns `compatibility_refused` when genuine required context is absent.
Successful foreign import/export materialization uses `restore_apply`/`snapshot_create` with the returned plan ID; the plan retains its format.
There is no general branch-management command or model-supplied author, root override, signing key, shell command, `confirmed`, or exposure-administration argument in ordinary tools.

| Commands | Success result `type` | Admission/effect boundary |
| --- | --- | --- |
| `library_list`, `library_open`, `library_create` | `libraries`, `library`, `library` | List only admitted libraries; open/create is owner-only. Creation changes visible files and history, with no-clobber. |
| `library_status`, `mount_list` | `status`, `mounts` | Visible library facts only; no background hydration or model start. |
| `mount_attach`, `mount_detach` | `library` | Owner-only; attach persists data facts, Git clone requires remote consent, detach never deletes originals. |
| `browse`, `source_details` | `listing`, `source` | Read admission; cursor binds library/mount/current observation, never permission. |
| `read_range`, `read_representation` | `content` | Read admission; verify current source and requested representation mapping. |
| `annotate`, `edit_text`, `import_file`, `transfer` | `write` | Explicit read-write admission; agents propose text changes; raw originals default read-only. |
| `trash`, `hydrate` | `write`, `job` | Owner-only selected-file effect; interactive Trash confirmation; no permanent deletion/mass hydration. |
| `refresh`, `process`, `pin` | `job`, `job`, `sources` | Explicit processing/write admission, per-mount budgets; pin does not imply hydration. |
| `query` | `search` | Read admission plus current exclusions; no synchronous whole-NAS derivation. |
| `job_status`, `cancel`, `operation_status` | `job`, `cancelled`, `operation` | Caller can inspect/control only its admitted operations; cancellation needs write admission. |
| `history_list`, `history_diff`, `proposal_list`, `proposal_diff` | `history`, `diff`, `proposals`, `diff` | Read admission; excluded paths and bodies stay excluded, including history. |
| `confirm_merge` | `write` | Owner-only exact proposal/base/diff confirmation; changed base or bytes refuses. |
| `sync`, `sync_schedule` | `sync`, `library` | Owner-only initial remote/schedule consent; subsequent permitted sync stays within that exact remote/effect. |
| `snapshot_plan`, `snapshot_create` | `package_plan`, `job` | Owner-only selection/destination approval; sensitive export and large copy require explicit effect selection. |
| `package_inspect`, `open_frozen` | `package`, `library` | Owner-only explicit archive selection; no declared-root contact; frozen is read-only. |
| `restore_plan`, `restore_apply` | `package_plan`, `job` | Owner-only; verify first, default new home, explicit mount remap, preserve conflicts. |
| `migration_plan`, `migration_apply` | `migration_plan`, `job` | Owner-only preview and verified no-clobber migration; retain old authored data until verification. |
| `v1_import_plan`, `v1_export_plan` | `package_plan`, `package_plan` or `package` | Owner/host adapter only; core mapping or explicit compatibility refusal, no fabricated scope/revision. |
| `exposure_get`, `exposure_set` | `exposure`, `exposure` | Owner-only administration with expected revision; never in MCP catalog. |

Every command can instead return the common `error` result.
Domain conflict/indeterminate outcomes remain their typed domain result when an operation has an inspectable outcome.
Precondition failures without an admitted effect use `error`.

```typescript
type Result =
  | { type: "libraries"; libraries: Library[] }
  | { type: "library"; library: Library }
  | { type: "status"; status: Status }
  | { type: "mounts"; mounts: Mount[] }
  | { type: "listing"; entries: { name: string; path: RelativePath; kind: "directory" | "file"; source: Source | null }[]; next_cursor: string | null; complete: boolean }
  | { type: "source"; source: Source }
  | { type: "sources"; sources: Source[] }
  | { type: "content"; source: SourceRef; source_digest: Digest; representation_digest: Digest | null; locator: Locator; bytes_base64: string; truncated: boolean; next_offset: U64 | null; next_line: number | null }
  | { type: "search"; search: SearchResult }
  | { type: "job"; job: Job }
  | { type: "cancelled"; job_id: Id; state: "requested" | "already_terminal" }
  | { type: "operation"; operation_id: Id; state: "pending" | "completed" | "indeterminate"; outcome: Result | null; job: Job | null }
  | { type: "write"; write: WriteResult }
  | { type: "history"; entries: HistoryEntry[]; next_cursor: string | null }
  | { type: "diff"; base_commit_id: string; candidate_commit_id: string; diff_digest: Digest; unified_diff: string; offset: U64; next_offset: U64 | null; truncated: boolean }
  | { type: "proposals"; proposals: Proposal[] }
  | { type: "sync"; sync: SyncResult }
  | { type: "package_plan"; plan: PackagePlan }
  | { type: "migration_plan"; plan: MigrationPlan }
  | { type: "package"; package: PackageResult }
  | { type: "exposure"; exposure: Exposure }
  | { type: "error"; error: ServiceError };
type ErrorCode =
  | "invalid_request" | "unsupported_version" | "not_found" | "not_exposed" | "read_only"
  | "owner_required" | "confirmation_required" | "excluded" | "sensitive_transport"
  | "offline" | "denied" | "placeholder" | "residency_unknown" | "changed" | "unverified"
  | "conflict" | "indeterminate" | "busy" | "operation_mismatch" | "cancelled"
  | "budget_exhausted" | "unsupported" | "provider_unavailable" | "provider_identity"
  | "invalid_vector" | "signing_required" | "remote_auth" | "remote_refused"
  | "invalid_package" | "integrity_failed" | "incompatible_index" | "compatibility_refused"
  | "recovery_required" | "internal";
type ServiceError = {
  code: ErrorCode; message: string; retryable: boolean; operation_id: Id | null;
  library_id: Id | null; source_id: Id | null; job_id: Id | null; method: SearchMethod | null;
  recovery_paths: RelativePath[];
};
```

Messages are bounded, redacted diagnostics and are not stable test oracles.
`retryable` is a scheduling hint, not permission to replay an indeterminate effect or bypass owner consent.
Unavailable/denied roots never become an empty successful listing or deletion inventory.
Errors must not reveal excluded paths or whether an unexposed library exists; use `not_exposed` consistently at that boundary.
Signature state and trust remain distinct from an author's display claim.
`diff_digest` binds the complete canonical UTF-8 diff, not only a truncated preview; confirmation requires access to the complete proposal diff.
A truncated diff cannot itself authorize acceptance; clients must read all bounded diff ranges under the same digest or refuse confirmation.
Diff `offset` and `length` address canonical UTF-8 bytes; ranges end on UTF-8 boundaries and responses identify the next byte offset.
`read_representation` uses `next_line`; `read_range` uses `next_offset`, and the unused continuation field is null.
An `operation` outcome is its original terminal `Result` or null while pending; it must not contain another `operation` result.

## Transports And Exposure

Normal GUI/CLI clients use OS-owner-only IPC with peer identity and a single daemon/library writer.
The CLI can start/reuse the daemon under a single-instance lock; failed boot never starts a competing writer.
The headless binary and service require no display or macOS framework.
IPC carries one newline-delimited `Request`/`Response` per frame with matching IDs; stdout for protocol processes contains no logs.
`--json` data commands return the same `Response`; successful/accepted results exit 0, rejected requests exit 2, dependency failures exit 3, and conflict/indeterminate results exit 4.
CLI cancellation uses the same operation/job IDs; diagnostics go to bounded stderr.

`omni serve --stdio` directly serves MCP; `omni mcp --stdio` proxies the daemon for multiple clients.
MCP uses JSON-RPC lifecycle and `tools/list`/`tools/call`; tool names equal permitted `Command.op` values and arguments are the command fields excluding `op`.
Tool results carry the `Response` as structured content and equivalent JSON text, with `isError` true only for `result.type: "error"`.
MCP protocol errors concern framing/lifecycle; domain failures use `ServiceError` rather than fabricated successful empty content.
MCP negotiates protocol revision `2025-11-25`; unsupported revisions and transport semantics are refused explicitly.
Legacy `serve-mcp` can remain an alias for selected-root stdio, but cannot preserve root override, self-confirmation, or unrestricted write behavior.
Local HTTP is opt-in and loopback-only by default; `POST /v1/command` carries the same envelopes and `/mcp` offers Streamable-HTTP MCP.
HTTP returns 200 for an admitted domain response including typed failure; malformed framing is 400, invalid host/origin is 403, oversized frames are 413, and unsupported wire versions are 400.
Host and Origin checks use literal configured allowlists; reject cross-origin browser requests, redirects, and ambient proxy-driven provider routing.
No public listener, account server, OAuth stack, raw index-file endpoint, or exposed inference port is added.
Existing secure tunnels are the recommended remote access method; loopback is not proof of per-user identity or an OS ACL.

Library MCP exposure defaults to `read_only` within an owner-selected set; HTTP initially selects no libraries.
The MCP catalog excludes every owner-only operation, even in read-write mode.
Read-write mode adds only the ordinary mutation/processing tools in the table; a model cannot approve itself, attach roots, change sensitivity, or widen exposure.
Explicit owner-launched stdio can select a sensitive library; a stdio proxy does not acquire that permission merely by connecting to the daemon.
Sensitive libraries are owner IPC or explicitly owner-launched direct stdio only, never HTTP/network, and absent from exports/diagnostics without explicit user selection of that effect.
Exclusions apply to browsing names/counts, metadata, history, snippets, representations, byte reads, processing, and distance eligibility, not just final bodies.
Recheck exposure before every delivered chunk/result and write; controllable in-flight disclosures stop after a change, but already delivered bytes cannot be recalled.
Synced/imported manifest expansion is pending owner action, not a local grant or authorization to contact a root.
Service-side local approvals bound roots/transports/effects independently of portable data declarations.
Keep query history off by default; journals and logs contain no source bodies, query text, credentials, keys, or raw command arguments.

## Wire Examples

A list request has no library-path override:

```json
{"wire_version":1,"request_id":"req-1","command":{"op":"library_list"}}
```

An admitted empty selection is distinct from an offline mount:

```json
{"wire_version":1,"request_id":"req-1","result":{"type":"libraries","libraries":[]}}
```

A model confirmation attempt is rejected without an effect:

```json
{"wire_version":1,"request_id":"req-2","result":{"type":"error","error":{"code":"owner_required","message":"This operation requires the local owner.","retryable":false,"operation_id":null,"library_id":null,"source_id":null,"job_id":null,"method":null,"recovery_paths":[]}}}
```

## Owner Confirmation And Independent Acceptance

The service derives caller class from the transport/session, never from an `owner`, `author`, or `confirmed` field.
GUI confirmation presents the exact proposal, base commit, affected paths, and complete diff and binds that tuple to `confirm_merge`.
Headless CLI confirmation is interactive and tuple-bound; a model-supplied boolean or noninteractive MCP call cannot confirm.
A stale base, changed diff, or obsolete proposal refuses; human confirmation does not override source expected-version checks.
Remote-effect consent, library selection, exposure administration, Trash, hydration, large copy, sensitive export, and restore destination selection also remain explicit owner actions.
Schedules authorize only their selected remote and effects; discovery never causes remote contact.
Imported AGENTS/schema text, manifests, archives, and Git branches cannot grant these capabilities.

Independent test authors can implement DTO round trips and malformed/unknown-field refusals without candidate source.
They must also exercise real producer/consumer seams and actual daemon transports for lifecycle, isolation, cancellation, idempotency, and restart claims.
Test exposure revocation during delivery, root injection, hidden counts, sensitive HTTP absence, cross-origin refusal, proposal self-confirmation denial, and changed-base confirmation.
DTO shape checks alone do not prove any storage, signing, network, or GUI behavior.
