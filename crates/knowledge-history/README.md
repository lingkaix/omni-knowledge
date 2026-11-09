# Knowledge History

This crate owns Git objects, private indexes, operation receipts, expected-ref transactions, and native-selected remote effects.
Read [history](../../docs/specs/history.md) and [service](../../docs/specs/service.md) for product authority.
It does not install home files, admit service callers, create a scheduler/executor pool, or activate durable service authority.
Native remote consent and external observation use the existing Git executor and the host's actual Core queue.

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
Its durable intent retains the confirmed proposal ref and exact commit; an unpublished retry rechecks that expectation and refuses an obsolete tuple without deleting the candidate.
Publication verifies the proposal expectation inside the same native transaction as accepted-head and operation-ref CAS, not only in a preceding read.
A completed operation remains idempotent after the proposal advances; a legacy unpublished integration without a retained proposal expectation fails closed.
`Confirmation { proposal_id, base: Oid, candidate: Oid, diff_digest: String }` is supplied only after the service's real human-confirmation check; this Rust seam is trusted in-process, not a wire grant.

`Oid` is validated lowercase SHA-1 (`Oid::parse`, `Oid::as_str`); IDs and paths are validated before effects.
`Prepared { operation_id, candidate: Option<Oid>, tree: Oid, affected_paths, unchanged }` identifies the exact candidate for M1's safe-file installer.
`State` distinguishes `Prepared`, `Published`, `Unchanged`, `Conflict`, and `Indeterminate`, with retained `Prepared` recovery data.
`Error` distinguishes invalid input, operation mismatch, owner-context mismatch, signer required, capacity/deadline, busy, Git failure, I/O failure, and corrupt recovery facts.
Errors never reflect submitted strings or Git stderr.

## Additive Compiled Read Side

The existing M1 prepare/install/publish signatures remain unchanged.
`proposals(after: Option<&str>, limit: usize, &Claim) -> Result<ProposalPage, Error>` enumerates only the qualified direct app-proposal namespace, with bounded actual Git output and an exclusive continuation ID.
A bounded native namespace-glob union implements the exclusive ASCII cursor without all-ref output, a local branch index or a dependency on newer Git's `--start-after` flag.
`proposal_diff(proposal_id, &Claim) -> Result<ProposalDiff, Error>` returns the actual proposal, current expected accepted base, complete patch/digest/affected paths, and refuses observed tuple drift.
`commits(start: Option<&Oid>, limit, &Claim) -> Result<CommitPage, Error>` pages actual accepted first-parent history and requalifies a supplied continuation against accepted ancestry.
`version(&Oid, &Claim) -> Result<CommitInfo, Error>` and `version_text(&Oid, path, &Claim) -> Result<String, Error>` require actual accepted/proposal ancestry; an unrelated branch or orphan hash is not public read authority.
M1 keeps the older trusted `read_text` method for its unpublished installable candidate.
`read_candidate(operation_id, path, &Claim)` reads actual retained tree bytes while signing is blocked, without creating an unsigned commit.
`CommitInfo` preserves actual author, committer, ordered parents, clocks, message, and recorded operation/agent/tool/session/origin facts; imported trailers remain attribution claims, not authentication.
`SignatureFormat` reports presence only (`Unsigned`, `Ssh`, `Other`); it must not be mapped to a cryptographic valid-known/valid-unknown result.
Use `verify_signature` below for actual cryptographic state; remote/adoption/schedule completion remains an active owner obligation, not a scope waiver or a claim from read tests.

## Native Signing And Immutable Verification

`SshSigner::select_agent(&File, native_socket: &Path, &Claim)` accepts an already selected **public** descriptor and explicit system-agent socket, never ambient `SSH_AUTH_SOCK` or a private key file.
`History::with_signer(SshSigner)` attaches that native provider without changing the library's explicit signing mode.
`with_committer(GitIdentity)` selects a distinct native committer for new operations; retries retain their original recorded committer.
`setup_system_signer(destination, socket, KeyProtection, &Claim)` offers real first-use Ed25519 generation and loading through system `ssh-keygen`/`ssh-add`, only outside the home/history/cache and without overwriting an existing directory.
`NativePrompt` requires a real native terminal; GUI/Keychain prompts remain native qualification, not a fabricated successful dialog. `UnencryptedExplicit` is an explicit human/native choice, never a model/default fallback.
Private keys stay in the selected system directory, not operation receipts, cache, Git, packages or logs. Deterministic tests use only disposable 0700 directories/0600 keys and their own finite agent/socket.
The exact public selected key and actual signed object are durably retained before object/ref publication, so retries do not generate a second randomized signature. Locked/missing providers preserve candidate text and return `SigningRequired`; immutable verification and completed retries do not need that provider.
`verify_signature(&Oid, &[KnownSigner], &Claim) -> Result<Verification, Error>` requalifies the actual version, then uses native SSH cryptographic verification for distinct `ValidKnown`, `ValidUnknown`, `Invalid`, `Unsigned` states. Other signature formats remain preserved as `SignatureFormat::Other` and explicitly refuse this SSH verifier, never become falsely invalid/known.
`KnownSigner::selected(principal, public_key)` is a **native user trust choice**, not a package claim or wire authorization. Author/committer/trailer names remain attribution data, even on a valid signature.
`prepare_observation(&Request, &[String], &Claim)` records `external_unknown` and actual observation IDs supplied by M1 after its stable-file/quiet-period check, without inventing an editor or claiming agent authorship.

## Held-File Inert Bundle API (M5)

`verify_bundle(&File, &ContentDigest, &Claim) -> Result<BundleReport, BundleError>` copies one bounded held regular-file exact stream into private owned output without moving its cursor, checks native before/after identity/version and actual SHA-256, then performs actual Git header/prerequisite verification **and native pack unpack/checksum validation** in private quarantine.
M5 must first use its actual `VerifiedSource.copy_to` into private owned output and verify that exact copied stream before handing this backend the resulting held file and expected stream digest. Neither a pathname nor a digest is source/current-context authority.
`admit_bundle` redoes the same checks and admits actual objects only. Neither method activates refs, changes accepted/home bytes, installs imported config, trusts public package signer claims, checks out files, runs hooks/filters/submodules/helpers, or contacts a remote.
Only bounded qualified app refs are reported; foreign refs, unsupported capabilities, bad pack checksums and missing prerequisites refuse. A hash of admitted objects still does not grant public immutable-version access.
`BundleError::Changed` distinguishes exact-stream change; `Integrity` means malformed/native pack refusal; `History(Error)` preserves actual execution/capacity/deadline/unsettled ownership failures.
Later ref activation requires separate native M6 context/permissions and actual expected-ref/human-confirmation rules, not `BundleReport.objects_admitted` or advertised refs as a grant.

```no_run
use std::{fs::File, path::Path};
use knowledge_core::{data::ContentDigest, jobs::Claim};
use knowledge_history::{History, SshSigner, Error, BundleError, BundleReport, ExistingRepository, OwnerContext};
// These inputs are supplied only by the already-admitted native host, not decoded as MCP grants.
fn selected_provider(public_file: &File, native_socket: &Path, claim: &Claim) -> Result<SshSigner, Error> {
    SshSigner::select_agent(public_file, native_socket, claim)
}
fn native_existing_choice(selection: ExistingRepository<'_>, home: &Path,
                          owner: OwnerContext, claim: &Claim) -> Result<History, Error> {
    History::use_existing(selection, home, "pages", owner, claim)
}
fn inert_package_history(history: &History, private_verified_output: &File,
                         exact_stream: &ContentDigest, claim: &Claim) -> Result<BundleReport, BundleError> {
    let inspection = history.verify_bundle(private_verified_output, exact_stream, claim)?;
    assert!(!inspection.objects_admitted);
    history.admit_bundle(private_verified_output, exact_stream, claim)
}
```

## Native Use-Existing Or Separate Selection

`History::use_existing(ExistingRepository { git_dir, baseline, placement }, home, text_root, owner, &Claim)` uses the actual selected user's object database and reserved app refs, with a private index. `baseline` is an explicit qualified user branch; its unchanged expectation is verified transactionally when creating the initial app accepted ref.
M6 supplies **current native** `GitPlacement::LocalUnsynced`, ownership and permission, never a decoded model/package placement/confirmed flag. `Synchronized`/`Unknown` refuse before app effects; native placement must be revalidated on later opens too. A stored binding/locator is not a placement or authority grant.
The explicit separate choice remains `History::init` in external local app data; neither choice relocates Git, writes an absolute home .git redirect, resets the user branch, rewrites history or deletes tracked media.
Use-existing leaves user HEAD/branches/index/staged bytes untouched and writes new app operations under the app accepted/proposal namespace. It retains original history authors/signatures and all unchanged tracked media/attributes/gitlinks; only selected regular UTF-8 text deltas are accepted. Executable regular text keeps its original mode; symlinks/submodules/binary originals cannot be silently reinterpreted as changed text.
Existing configuration is parsed by native Git with **no-includes/stdin/name-only**, not executed/copied into app metadata or queried from personal global/system config. Unsafe includes, transport/rewrite widening, alternates and linked-worktree config refuse. Native hooks/fsmonitor/filter/diff execution remains disabled on real plumbing commands; user configuration is not overwritten.
Configuration version drift refuses. Explicit native re-selection can validate new inert configuration, but never resets an already active app accepted history to today's user HEAD. Existing synced Git requires separate history or a separately authorized relocation; this seam does not perform relocation.

## Native Cost And Unknown Ownership

`History::required_resources(&Budget) -> Result<Reservation, Error>` is a conservative concurrent memory/storage envelope for the native host to compose into the **same Core P1 account/claim/prefunded credit**, not another admission or a new pool. Existing M1 prepare/install/publish signatures are unchanged; the backend borrows the original claim and never reserves the same job again.
Native stdout/stderr/stdin are nonblocking and bounded in one loop with the original deadline and aggregate owned-group RSS observations; regular-file extent ceilings are installed before exec. No RSS-as-address-space rlimit or physical sandbox claim is substituted.
The host must retain its actual causing claim/credit for all live child, input, result-vector/String capacity and actual file/artifact lifetimes. Returned data retained after execution requires the host's real Core retained holding, not a completed metadata-only job.
`Error` is now `Clone`, **not `Copy`**, because `Error::Unsettled(UnsettledChild)` owns the actual unreaped process/group/I/O/lock/artifact capability, including when the History handle is dropped. Do not stringify/drop that capability and settle the causing claim.
`UnsettledChild::inspect/collect` inspect or finitely collect only that exact machine owner; PID data cannot reconstruct it, and only the creating process may collect/unlock it. Repeated collection of settled A cannot settle/discard newer B: completion belongs to each actual owner, and scratch/writer guards bind to their own public call's owner rather than reusable runtime state. A new native execution refuses while that exact pending owner remains uncollected. Collection does not replay Git effects, discard recovery evidence or settle Core credit; retain the causing credit through caller unwind, whole native collection and artifact/result reconciliation.
Original-process writer Drop explicitly unlocks only after known completion; an unresolved native worker retains the actual lock descriptor and private scratch. `pending_process/collect_pending` expose the same actual retained owner on a live History handle.

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
Commands use finite nonblocking I/O and actual PID/start-pinned owned-group collection; unknown process/I/O/file/lock ownership is retained rather than hidden by an unbounded wait or best-effort cleanup. No broad cleanup or serialized-PID rediscovery authority is used.
Core whole-call byte/entry ceilings and the earliest claim/call deadline remain enforced; repeated tree reads conservatively consume the same finite work ceiling.
Native resident observations and file extent ceilings are not an OS/cgroup physical sandbox; the admitted host owns the complete resource plan and actual retained Core-credit lifetime.
Home/repository native identities are rechecked; use-existing requires explicit native selection and bounded inert config validation. Unsupported/widening config, control-file symlinks, alternates and unsafe changed text paths refuse.
The host selects an external local non-synced history root and validates all mount overlaps; this crate checks home overlap, not provider residency.
Native SSH signed commit production/verification and inert bundle admission are real implemented mechanisms; real OS Keychain/GUI prompts and macOS native lifetime behavior remain platform qualification. The local native-selected remote and queued-observation framework below is implemented; actual service binding and broader transport/platform qualification remain active obligations, not accepted scope reductions.
Use-existing/separate selection remains implemented as above and still needs integrated/native review.
This narrow backend does not claim complete history.md product acceptance.

## Native Remote And Queued Observation API

`NativeHistoryOwner::select(&History, context, library)` is a trusted native host selection, separate from `OwnerContext` attribution.
The host must first establish actual current owner/root/effect admission and call `revoke()` when that selection or policy changes.
This non-serializable live capability cannot be recovered from URLs, timestamps, public keys, manifest data, queue keys or a saved schedule.
`HistoryWork { claim: &Arc<Claim>, key }` checks the actual original Claim generation with `stop_requested(key)`, deadline and cancellation before effects and on every native I/O tick. It splits an actual same-Claim Core Holding for `required_resources` before copying descriptors or starting native work. The call returns known-dead credit to that exact generation; stale settlement finishes only known-dead credit. An unresolved native owner retains the actual Holding and Arc<Claim> through whole group, pipe, reap and writer-lock collection.
The original claim and retained credit must remain held through results, unresolved owners and recovery artifacts.
New native methods can return `Error::Denied` or `Error::Cancelled`; service adapters must map them explicitly without stringifying or dropping `Error::Unsettled`.

`approve_local_remote(history, path, ref, RemoteEffects, work)` records native human consent for an exact local bare transport and branch.
It rejects active imported config, alternates and widening hooks/helpers; it never uses personal Git/SSH configuration, credentials or network discovery.
`pull_remote(operation_id, consent, work)` fetches real objects into private quarantine, verifies/adopts them through the existing inert bundle owner, and CAS-publishes a retained proposal only.
It returns actual accepted/fetched versions, divergence class and complete comparison digest/paths, without changing accepted refs or home bytes.
`confirm_remote(consent, RemotePull)` is a separate trusted native human decision on the displayed tuple, not a model flag or scheduled action.
`prepare_remote_confirmation(operation_id, approval, consent, work)` continues through the existing signing/expected-proposal transaction owner.
Initial continuation retains imported history as a parent; an empty-tree adoption reuses the exact imported commit without an empty operation commit.
Imported authors/signatures are not rewritten.
M1 must safely install and journal the exact prepared tree before ordinary `publish`; `Published` remains Git-only.
`approve_push(consent, operation_id, candidate, expected_remote)` binds a distinct native intention to one exact push.
`push_remote(approval, consent, work)` requires accepted ancestry, an actual fast-forward proof and an exact expected-ref lease; it preserves competing remote writers and retained local candidates.
An uncertain restarted push inspects without blind replay; a completed retry remains idempotent after later remote advancement.

`external_batch_version(&[ExternalText])` identifies the complete qualified versions and observation IDs for `Key.version`.
`observe_external_batch` accepts the concrete `LocalMount<P: LocalReadPolicy>`, binds its actual current held root to the selected home before every read/deletion and prepublication, and reads actual versions through the backend. It checks before/after metadata and full byte digests, waits an actual monotonic quiet period, and rechecks sources after preparing/signing.
It commits one multi-file `external_unknown` operation under the chosen observing identity, not the external editor's invented identity.
`ExternalText.deleted` requires actual before/after backend deletion observations; offline, denial and reappearance refuse rather than deleting history.
Reopen conservatively starts a fresh monotonic quiet period; wall-clock next-due facts cannot bypass it.
A completed retry and an unchanged later observation produce no duplicate text commit.
This is the Git side of observation, not proof of M1 journal/install publication.

Schedules default disabled/no-push.
`enable_remote_schedule` and `enable_external_schedule` return `(HistorySchedule, ScheduleConsent)` after explicit native enablement.
The live, non-serializable `ScheduleConsent` separately binds current selection, exact schedule/remote, interval, allowed effects and base recipe; stored flags cannot widen it.
Keep that capability for submission/execution/completion; reopening requires a fresh native decision with the same narrowed scope.
`schedule_status` exposes interval, next due, current operation ID and attempts, but cannot mint a schedule capability.
`HistorySchedule::work_recipe()` qualifies the host's immutable base recipe by occurrence; use it as `Key.recipe` for that submitted/claimed occurrence.
This prevents Core terminal-key dedup from swallowing later periodic polls while preserving same-occurrence idempotency.
`submit_remote_schedule`/`submit_external_schedule` actually submit that stable operation/job ID and full descriptor to the host's existing Core queue.
`run_remote_schedule` performs real fetch and, only for local-ahead state under explicit scheduled push consent, a leased push; divergence stays a proposal.
`run_external_schedule` performs actual quiet-batch work; `Waiting` is not completion and the original claim stays unsettled.
`finish_schedule` verifies actual immutable domain publication while the original claim still owns the lifetime, advances due facts and settles that exact claim.
Before calling it, the host must reconcile returned data/artifacts and transfer any remaining lifetimes into its actual retained Core holding; queue counters alone are not retained credit.
`backoff_schedule` accepts only actual settled-offline queue status; retry/reconnect is explicit and Core-attempt-bounded.
Reopening and resubmitting an active occurrence uses the same operation ID; a saved enabled flag still needs a fresh current native capability.
`disable_schedule` stops later dispatch but never asserts that an existing worker stopped.

### Host Binding Need And Qualification

The integrated owner uses the existing Core Account and actual nonforgeable Arc<Claim>, never deadline/status equality as a generation grant. A standalone Queue without the native Account cannot pay native HistoryWork effects. The concrete LocalMount root binding rejects an unrelated empty root before accepting deletion. No second account, resource engine or liveness token was introduced.
The minimum product binding remains a current native NativeHistoryOwner, the actual full-key HistoryWork, and M1's prepared-tree install/journal/publication sequence. Git publication alone does not install home bytes.
Actual local transport and disposable signatures can qualify this Git framework; production CLI/MCP admission, M1 publication, native macOS events/prompts and SSH/HTTPS transport remain separate open qualification work, not an `Unsupported` completion waiver.
No production remote or real credential is contacted by this crate's deterministic tests.
The [change record](../../docs/changes/2026-11-05-native-history-remotes-and-observation.md#remaining-queue-and-evidence-limits) tracks the remaining owners; close these limits only after actual host binding, native observations and consequential review.

```no_run
use knowledge_history::{History, HistoryWork, NativeHistoryOwner, RemoteConsent, RemoteEffects, RemotePull, Prepared, ScheduleConsent, Error};
use knowledge_core::jobs::{Queue, Key, Submission, Submitted, Priority};
use std::path::Path;
// Called only by the admitted interactive native owner, never from portable DTO authorization fields.
fn inspect_selected_remote(history: &History, native: &NativeHistoryOwner, bare: &Path,
                           work: HistoryWork<'_>) -> Result<RemotePull, Error> {
    let consent = native.approve_local_remote(history, bare, "refs/heads/main", RemoteEffects::FetchOnly, work)?;
    history.pull_remote("owner-inspected-pull", &consent, work)
}
// NEED: the actual native M1 adapter consumes this Prepared through its safe installer and visible journal.
// This function performs real tuple/signing preparation, not a fake successful file-install callback.
fn prepare_for_m1_installer(history: &History, native: &NativeHistoryOwner,
                            consent: &RemoteConsent, displayed: RemotePull,
                            work: HistoryWork<'_>) -> Result<Prepared, Error> {
    let human_intention = native.confirm_remote(consent, displayed)?;
    history.prepare_remote_confirmation("human-integration", &human_intention, consent, work)
}
// Retain the returned native capability for actual dispatch; do not reconstruct one from schedule_status.
fn enqueue_native_schedule(history: &History, consent: &RemoteConsent, queue: &Queue,
                           bootstrap: HistoryWork<'_>) -> Result<(ScheduleConsent, Option<Submitted>), Error> {
    let (status, schedule_intention) = history.enable_remote_schedule("selected-sync", consent, 15, false, bootstrap)?;
    let recipe = status.work_recipe()?;
    let key = Key { recipe: &recipe, ..bootstrap.key };
    let budget = bootstrap.claim.budget();
    let submitted = history.submit_remote_schedule("selected-sync", &schedule_intention, consent, queue, Submission {
        operation_id: &status.operation_id, job_id: &status.operation_id, key, budget,
        reservation: History::required_resources(&budget)?, priority: Priority::Normal, max_attempts: 3,
    })?;
    Ok((schedule_intention, submitted))
}
```

## Necessity And Checks

A private index keeps unrelated staging untouched; dropping it would mix external staged bytes into operation commits.
A durable per-operation intent supplies a fixed timestamp and normalized fingerprint across a crash; dropping it permits duplicate or mismatched retries.
Prepared refs retain candidates through failed publication; operation refs retain immutable successful generations and make publication idempotent.
The writer lock serializes cooperating receipt/index ownership; native Git CAS still protects against writers outside that lock.
These are local Git transaction facts, not a generic transaction registry or second annotation authority.

Run `cargo test --locked --offline -j2 -p knowledge-history`, strict package Clippy, and workspace formatting with private target/TMPDIR/dictionary directories.
Builder tests use real disposable Git repositories and public operations, including interrupted prepare/publish, ref locks, concurrent writers, immutable old generations, proposals, and hostile inputs.
Independent milestone durability review and public-interface tests are integration obligations, not self-approval by this builder.

## Detached native snapshot signatures

`History::verify_snapshot_signature` checks the decoded 32-byte canonical snapshot digest with the existing native SSHSIG verifier under namespace `omni-knowledge.snapshot.v1`.
It requires the actual NativeHistoryOwner and HistoryWork with a live Arc<Claim> and exact full key; Git object verification still uses namespace `git`.
Unsigned, valid unknown, valid known and invalid signatures remain distinct facts.
The native caller decides known-key or explicit-unsigned activation policy; verification itself grants no imported root access or publication authority.
Unknown native collection keeps its existing owning error and actual funded work until group, pipes and I/O settle.
