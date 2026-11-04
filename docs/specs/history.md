# Text History And Git-Server Libraries

Status: Accepted full-version contract, 2026-11-04; not a report of implemented Git behavior.
This document owns automatic text Git, attribution/signing, minimal proposals, remote synchronization, and recovery across file/ref effects.
[Storage](storage.md) owns authoritative files and competing-writer preservation.
[Service](service.md) owns public history/proposal/sync DTOs and owner confirmation.
This is ordinary product Git history, not OpenKit Notebook publication or a replacement PKI.

## Tracked Content And Commit Unit

Track only the chosen page root, source pointers/annotations, manifest, inert AGENTS/conventions/schema, and authored text companions such as corrected transcripts.
Keep originals, media, generated ASR, cache, previews, indexes, and runtime state out of automatic text commits.
The visible observation journal records media digests/outcomes but its append traffic does not trigger text commits.
Do not add Git LFS or promise retained historical media bytes.
Journal snapshots are independently exportable; a selected Git bundle can preserve text history in backup packages.

One durable text-changing omni operation produces exactly one operation commit, including multi-file edits.
Reads, searches, derived jobs, text-free observations, and unchanged edits do not produce empty commits.
A media import can create one pointer/annotation operation commit plus its separate media observation.
An accepted proposal and its human-confirmed integration are separate operations; integration has its own commit.
A file-level saved-local outcome is not pushed, cloud-synchronized, or atomically saved across several files.
Journal intent/results bind `operation_id`, affected/final digests, and the actual Git commit ID.
Duplicate operation delivery must not manufacture a second operation commit.

## Identity And Signing

Use the OS user's selected Git identity as human author.
The service supplies user identity and validated operation/tool/session context; an MCP argument cannot claim another human author.
For agent operations, record agent-on-behalf-of-user attribution with validated `Omni-Operation-Id`, `Omni-Agent`, `Omni-Tool`, and `Omni-Session-Id` trailers when applicable.
The human author and distinct agent facts must remain distinguishable in the history response.
Keep raw arguments, query text, credentials, and unrelated paths out of trailers and commit messages.
An agent-supplied display name is an attribution claim, not cryptographic identity proof.

Signed mode uses per-user SSH commit signing through system ssh-agent/signing providers.
Reuse a selected existing signing identity or offer explicit first-use system-key setup without a product account or hand-edited library configuration.
No signing key belongs in a home, cache, package, journal, remote URL, evidence, or diagnostic log.
Trust/known signers are user/system choices, not an omni certificate authority.
Rotation preserves old verification information and never rewrites old history merely to replace signatures.
Report valid-known, valid-unknown, invalid, and unsigned states separately.

If the signer is unavailable or locked, reads continue and mutation preserves its candidate with `signing_required`.
Do not silently commit unsigned, attribute to an unknown human, or discard a text candidate.
On Linux headless Git-server libraries, use the system ssh-agent/signing key when present.
Without one, the user may choose explicit visibly marked unsigned mode at library creation by CLI flag.
This is not an automatic fallback or a model choice; signed-mode acceptance must still exercise real disposable SSH signatures.

## History Location And Existing Repositories

The automatic Git directory lives in local app data outside all cloud-sync folders and mounts that synchronize Git internals.
Bind the working tree explicitly; do not put an absolute `.git` redirect file in a synced home.
Private indexes and proposal worktrees also live outside sync folders.
History is durable state, separate from disposable derived cache.
Moving or remapping a home requires explicit path/identity validation without turning a stored path into authorization.
Visible home bytes remain authoritative.

If the home is already a user Git repository, offer use-existing or separate omni history.
Do not relocate, reset, rewrite, or erase user history without permission.
Use-existing employs a private index and selected text paths, preserving unrelated staged changes, branches, and tracked files.
Never use `git add .`, run imported hooks/filters, or delete media from user Git to enforce omni's text-only commits.
Existing media history remains; new omni commits stay text-only and the interface states that distinction.
If the existing Git directory is inside cloud sync, require separate history or explicitly authorized relocation.
Use-existing cannot bypass the no-sync-Git invariant.

Use trusted system Git with bounded argument arrays and ordinary SSH/HTTPS verification.
Do not use shell interpolation, imported executable hooks/filters, submodule execution, trust-bypass flags, or credential-bearing URLs.
Imported bundles and repository configuration are inert until controlled validation/admission; opening a package cannot execute Git configuration.
Use the system credential helper/SSH agent; omni does not store transport tokens in library metadata.

## Minimal Proposal Branches

Branches identify writers/proposals, not permissions or independent working-tree writers.
One home writer installs accepted bytes after expected-base/content checks.
Human accepted operations advance the accepted head only after those checks succeed.
Agent writes go to proposal branches and do not overwrite the accepted home.
If the accepted head advanced, prepare an inspected integration candidate; preserve all existing branch commits and competing bytes.
A lock on Git refs does not replace storage's safe-write checks against external editors.

The public branch surface is only proposal list, diff, and human confirmation.
There is no general branch create/delete/checkout/reset/rebase API.
The service can prepare merge/rebase candidates internally but cannot expose an alternate model self-approval route.
An agent-proposed integration always needs human confirmation bound to proposal ID, expected base, complete diff digest, and affected paths.
A changed base/diff or obsolete proposal refuses and requires a new inspected candidate.
GUI presents that exact tuple; headless CLI confirmation is interactive and tuple-bound.
Neither MCP nor `confirmed: true` can supply human confirmation.
A merge is its own operation/commit and preserves remote author/signature metadata rather than relabelling the imported commits.

## External Edits And Recovery

After a quiet period, compare external text edits with the last observed accepted tree and commit one observed batch.
Record origin as `external_unknown` with observation IDs/evidence; do not invent the actual external editor's identity.
The observing user signs as committer; observed bytes are not evidence of agent authorship.
Remote-origin commits with actual metadata retain their authors/signatures and are not relabelled unknown filesystem edits.
Growing/changing files wait for storage's stable observations; cloud conflict copies remain separate assets.

Files, journal, Git objects/refs, and cloud uploads are separate effect domains.
A crash between file installation and ref advancement must retain operation identity and recover without duplicate operation commits.
Inspect final file digests, operation intent/result, commit trailers, and actual refs on restart.
Complete the same operation only if the intended result is provable; otherwise return indeterminate/recovery state with preserved candidates.
Never replay uncertain writes blindly or overwrite another writer to restore an expected tree.
An unsigned/signing failure is not permission to publish a saved result that lacks its required history.

## Git-Server Library And Schedule

Clone a supported Git remote into a user-visible local working copy and keep its actual Git directory outside cloud sync.
The content then uses the local mount backend; no Git server account is required in omni itself.
Clone, fetch, and push require explicit consent for that remote/effect; discovery never contacts a remote.
Support on-demand sync and explicitly enabled scheduled fetch/integration/push.
The service `SyncSchedule` reports enabled state, interval, allowed push, and next run; new libraries default to disabled/no-push.
A schedule cannot widen its selected remote, roots, or allowed effects and cannot supply human confirmation for agent proposals.

Pull means fetch plus inspected integration.
Under an enabled schedule, clean fast-forwards may integrate; diverged histories/conflicts become preserved proposals, never force-push or destructive reset.
Agent-proposed integrations remain human-confirmed even when fetched under a schedule.
Show both sides of a conflict, local/remote heads, and retained signatures/paths.
Push non-fast-forward/refusal triggers fetch plus surfaced merge state rather than automatic overwrite.
Authentication/offline failures preserve local commits and report unsynced status.
A timeout with uncertain remote effect returns indeterminate and requires inspection before retry.

Without push rights, default to a read-only remote library.
An explicit local-only editable copy/proposal branch may be offered without implying it can publish upstream.
Do not test a real user's production remotes or credentials.
Use local bare repositories or an unprivileged local Git transport/server and disposable synthetic identities/signing keys.

## Acceptance

Through public operations, inspect actual home bytes, refs, signatures, trailers, selected paths, and staged user state.
Prove one operation/commit, no empty read/job commit, exact proposal/base/diff confirmation, and agent inability to confirm itself.
Inject locked/missing signer, external edits, two competing writers, crash between file/ref effects, duplicate retry, and branch/head advancement.
Preserve both versions and operation identity after each uncertain outcome.
With real local Git transport, prove clone/fetch/integration/push, divergence, refusal, offline/auth failure, read-only rights, and schedule observability.
A mock returning Git success or a compiled DTO does not prove these effects.
Real Keychain/signing-agent prompts remain native qualification, separate from deterministic disposable-key proof.
