# Knowledge History

This crate owns only local Git objects, private indexes, operation receipts, and expected-ref transactions.
Read [history](../../docs/specs/history.md) and [service](../../docs/specs/service.md) for product authority.
It does not install home files, admit callers, run a scheduler, contact remotes, or activate durable service authority.

## Frozen Local Seam

`History::init(repo, home, text_root, owner, claim)` creates a new external, app-owned bare repository.
`History::open(repo, home, text_root, owner, claim)` reopens that selected repository without consulting user Git configuration.
Both return `Result<History, Error>`; every subsequent public operation borrows the current `knowledge_core::jobs::Claim`.
The service supplies `OwnerContext { name, email, agent, tool, session, mode }`, not MCP arguments.
`Mode::UnsignedExplicit` is an explicit owner choice; `Mode::SigningRequired` fails closed without an unsigned operation commit.
A claim is the existing reservation lifetime, not authorization; the host must use its single admitted process-wide queue, reserve child/index/object costs, and keep the claim unsettled until this call and its child collection finish.
No queue or independent admission pool is created here.

`prepare(&Request, &Claim) -> Result<Prepared, Error>` takes `Request { operation_id, base: Option<Oid>, destination: Destination, changes: Vec<Change> }`.
`Change { path, text: Option<String> }` replaces UTF-8 text or deletes a selected path; the request is a delta over the exact base tree, not a working-tree scan.
`Destination::Accepted` requires owner context without an agent; `Destination::Proposal { proposal_id }` isolates agent candidates from accepted refs and home bytes.
`publish(operation_id, &Claim) -> Result<State, Error>` locks and validates the expected target ref and creates an immutable operation ref in one native `git update-ref --stdin` transaction.
`inspect(operation_id, &Claim) -> Result<State, Error>` inspects receipts and actual refs without replay or filesystem writes.
`head(&Claim) -> Result<Option<Oid>, Error>` returns the accepted version.
`compare(base: Option<&Oid>, candidate: &Oid, &Claim) -> Result<Comparison, Error>` returns a complete bounded Git patch, its SHA-256 digest, and affected paths; overflow is an error, not a truncated confirmation.
`read_text(commit: &Oid, path, &Claim) -> Result<String, Error>` reads bounded selected text from an immutable version.
`prepare_confirmation(operation_id, &Confirmation, &Claim) -> Result<Prepared, Error>` requires owner context, the exact proposal/base/full-diff digest, and prepares a distinct two-parent integration operation.
`Confirmation { proposal_id, base: Oid, candidate: Oid, diff_digest: String }` is supplied only after the service's real human-confirmation check; this Rust seam is trusted in-process, not a wire grant.

`Oid` is validated lowercase SHA-1 (`Oid::parse`, `Oid::as_str`); IDs and paths are validated before effects.
`Prepared { operation_id, candidate: Option<Oid>, tree: Oid, affected_paths, unchanged }` identifies the exact candidate for M1's safe-file installer.
`State` distinguishes `Prepared`, `Published`, `Unchanged`, `Conflict`, and `Indeterminate`, with retained `Prepared` recovery data.
`Error` distinguishes invalid input, operation mismatch, owner-context mismatch, signer required, capacity/deadline, busy, Git failure, I/O failure, and corrupt recovery facts.
Errors never reflect submitted strings or Git stderr.

## Ordering And Recovery

Prepare retains immutable intent before commit construction and pins the candidate under `refs/omni/prepared/<operation_id>` before returning.
The same normalized operation and authoritative context return the same candidate after reopen; changed arguments refuse.
A successful text operation has one commit object, even if a process stops before recording publication.
Native Git transactions lock and validate accepted/proposal and operation refs as one set; prepared refs, retained tree refs, and previous operation refs are never deleted.
The native files backend does not provide crash-atomic multi-ref persistence or a simultaneous snapshot for lock-free readers; one-sided interrupted facts remain indeterminate.
Candidate inspection verifies actual commit tree, parents, authoritative author/context, timestamp, and operation trailers against retained intent before offering installable bytes.
No failed transaction is retried automatically.
Inspect after a timeout or interruption; a mismatched target is a conflict and ambiguous facts are indeterminate.
Only a subsequent explicit publish of a proven prepared operation can finish its Git effect.

M1 owns visible intent/result journaling, final digest checks, no-clobber file installation, and cross-domain recovery.
Prepare, install, and publish are NOT atomic across filesystems or files/refs.
`Published` means Git-only publication, never `saved_local`; the caller may report saved only after matching actual home bytes and its durable journal to this candidate.
A Git error after file installation must retain both filesystem recovery and this candidate and must not trigger blind reinstallation or a new operation ID.
A proposal never writes home bytes; confirmation still needs M1's safe installer before accepted publication.

## Current Bounds And Exclusions

System Git is executed directly with cleared environment, disabled global/system config, inert exact owned repository config, no hooks/helpers/filters/editors/network, explicit bare directory, and per-operation private index.
Commands use finite file-backed I/O and exact spawned-child kill/wait on deadline; the owned child handle remains uncollected until wait, so no PID rediscovery/reuse matching or broad cleanup is used.
Core whole-call byte/entry ceilings and the earliest claim/call deadline remain enforced; repeated tree reads conservatively consume the same finite work ceiling.
Output polling is not an OS disk/RSS sandbox; the admitted host owns the product's physical child/resource bound.
Home/repository native identities are rechecked; existing arbitrary repositories, imported config, symlinks, alternates, and unsafe selected text paths refuse.
The host selects an external local non-synced history root and validates all mount overlaps; this crate checks home overlap, not provider residency.
Signed commit production and remote synchronization remain separate integration capabilities; required signing currently returns `SigningRequired`, never silent downgrade.
This narrow backend does not claim complete history.md product acceptance.

## Necessity And Checks

A private index keeps unrelated staging untouched; dropping it would mix external staged bytes into operation commits.
A durable per-operation intent supplies a fixed timestamp and normalized fingerprint across a crash; dropping it permits duplicate or mismatched retries.
Prepared refs retain candidates through failed publication; operation refs retain immutable successful generations and make publication idempotent.
The writer lock serializes cooperating receipt/index ownership; native Git CAS still protects against writers outside that lock.
These are local Git transaction facts, not a generic transaction registry or second annotation authority.

Run `cargo test --locked --offline -j2 -p knowledge-history`, strict package Clippy, and workspace formatting with private target/TMPDIR/dictionary directories.
Builder tests use real disposable Git repositories and public operations, including interrupted prepare/publish, ref locks, concurrent writers, immutable old generations, proposals, and hostile inputs.
Independent milestone durability review and public-interface tests are integration obligations, not self-approval by this builder.
