# Knowledge Local

Confined read adapter for one explicitly selected ordinary local directory.
It implements the frozen [`MountBackend`](../knowledge-core/src/backend.rs) seam from `knowledge-core`.
The [storage owner](../../docs/specs/storage.md) governs mount identity, verify-on-read, budgets, and exclusions.
[Product](../../docs/specs/product.md) places ordinary local reads ahead of the optional GUI.
[Service](../../docs/specs/service.md) still owns exposure and read admission; this crate does not cut over to it.

This adapter includes M1 shared-admission verification and native hints behind the same public watch interface.
The [FSEvents consumer contract](fsevents.md) describes the macOS helper, lifetime, cursor handoff, and remaining native qualification.
Builder evidence is not independent M0/M1 acceptance or an application authority change.

## What The Caller Must Already Know

`LocalMount::open` binds one absolute directory. The path must already be the caller's known ordinary root.
`None` for the mount id means the caller stated a home-relative selection. It is not discovery of a home.
The constructor stores the directory's device and inode and does not call the read policy.
A symlink component, a relative path, or a non-directory is `LocalError::InvalidRoot`.
Only `EIO` while binding is `LocalError::Io`.

Each later call asks `LocalReadPolicy::decide` again. Constructing `CurrentAdmission` is not a grant.
`ReadDecision::Admit` must name the same library id and mount id that were bound.
`LocalityPremise::OrdinaryLocal` is the caller's premise that this directory is an ordinary local root.
`NotEstablished` and `ReadDecision::Unestablished` return `Unavailable::Unknown` and do not walk or read bytes.
`ReadDecision::Deny` returns `Unavailable::Denied`.

`outside_scope` is the caller's union of exposure exclusions and child-owned scopes, including descendants.
This adapter does not detect `omni-knowledge.json`, inner mounts, or which of those two categories a path belongs to.
A path equal to an outside scope, or a descendant of one, is refused before lookup.
A prefix that does not end on a path boundary is not an exclusion (`hidden` does not exclude `hidden-file`).

## Operations

`capabilities` reports range reads only. Stable file id, watch, conditional write, and residency are false.
Those flags are facts about this adapter, not a promise about a future filesystem.

`list` walks the selected tree with descriptor-relative `openat` and `statat` using `O_NOFOLLOW`.
The root itself is not an entry. `.` and `..` are skipped. Directory order is not a contract.
Entries outside the caller scope are omitted and are not descended. A complete listing may still omit them.
A symlink, fifo, socket, device, unknown node, or non-UTF-8 name stops the listing as `Partial` with `Unsupported` and a null cursor.
Names that `RelativePath` cannot represent stop the same way.
Denial, disappearance, or an unsupported errno while a walk is in progress keeps entries already accepted and stops as `Partial`.
Denial of the root directory stream, before any entry, is `Unavailable::Denied`.
Any presented cursor is `InvalidRequest`. This adapter issues none.

`stat` returns metadata only. It does not hash, sniff, or read file bodies.
A missing final node is an available observation of `Deleted`.
A symlink or special node is an available observation of `Unsupported`.
An excluded path is `Unavailable::Denied` before lookup, so denial is not an existence answer.
Inode, device, birth, and generation are copied only into `FileIdEvidence` when birth or generation was actually reported.
Linux `statx` birth time is used when the kernel mask includes it. Linux generation is not observed.
macOS uses `st_birthtime` and nonzero `st_gen`. Zero birth and zero generation stay `None`.
`stable_file_id` stays false because an inode is not a reuse proof.
`residency` on a snapshot is `Unknown`. That is not a cloud hydration detector, and it is not a reason to refuse an admitted ordinary-local read.

`open_range` takes the path only from `ExpectedVersion.snapshot`.
It rechecks the root, the current admission, the no-follow stat, and the full SHA-256 of the current file before acknowledging bytes.
The digest is compared with a bounded buffer. Only the requested overlap is retained.
A stat-preserved change outside the requested range fails as `Unavailable::Changed` and drops the bytes.
The whole source file is charged to `max_bytes`, including bytes outside the returned range, because those bytes were hashed.
The requested length must also fit `max_bytes` and `max_memory_bytes` before any allocation.
The hash buffer needs at least one byte of memory budget beyond the retained request.
An offset past the end is `InvalidRequest` only after the full digest still matches. Drift wins over that classification.
After the read, `fstat` of the descriptor and a fresh no-follow stat must still match, and the root and policy are checked again.

## Shared Admission And Native Hints

`verify(path, budget, holding)` obtains the first verified full SHA-256/stat version from actual admitted bytes.
The caller supplies an unsettled SAME-process Account holding before verification; it needs no fabricated source digest or second queue.
`list_admitted` checks that the declared working-memory ceiling is prepaid by that same account before enumeration.
The caller must retain the holding through all owned arguments/results, and current LocalReadPolicy still decides actual read admission.

`watch(budget)` reserves its complete declared metadata/buffer/kernel-watch plan through the same process Account before native setup.
Linux uses nonblocking inotify descriptors on held-directory `/proc/self/fd` names, without a background worker or detached thread.
Poll returns hints only; startup, overflow, lost/root/new-directory coverage and work exhaustion require actual bounded reconciliation.
Only the native owner may acknowledge a completed durable relevant scan at an unchanged hint epoch.
New/incomplete directory coverage cannot be repaired by a quiet interval or acknowledgement; rebuild the watch after reconciliation.
Native watch/instance/descriptor exhaustion is `WatchError::Exhausted`, never empty success or evidence of absence.
Drop closes the exact native descriptor and destroys owned policy metadata before settling its independent holding; execution mount slots are not retained.
On macOS, both watch and monitor use one actual device-relative FSEvents stream in the prepaid finite-lease omni-fsevents helper.
The helper receives a held root FD, resolves its no-firmlink device-relative path, and sends bounded metadata-only callback frames.
It retains callback context and native objects until process exit; only confirmed exit permits parent settlement.
Unknown stop retains exact process/transport handles and the Account charge; EOF and a void stop request are not quiescence.
Opaque CF runtime costs use the existing processing worker RSS observer, not a fabricated fixed CF allocation bound; sampling and syscall overshoot remain explicit limits.
Native drop/coalescing/root/volume loss and lease expiry require bounded catch-up and stream rebuild, not acknowledgement-based coverage repair.
The native-only delivery/resource/cursor/restart tests require actual macOS execution; Linux compilation does not qualify this binding.
The frozen backend reliable_watch capability remains false; these hints do not claim durable replay or end-to-end tracking qualification.

## Streaming, OS-Mounted NAS And Explicit Fallback

visit delivers bounded eager metadata directly to an admitted callback instead of retaining whole trees in RAM.
It checks current policy, dynamic child markers and native mount-device boundaries, and preserves explicit partial/error stops.
read_scope fingerprints actual native admission revision/exclusions; contains_directory checks actual source/cache descriptor ancestry without granting permission.
open_os_mount requires the native owner's explicit OsMountedNas premise; it does not discover credentials, log in, hydrate, or infer remote-version capabilities.
NAS reports reduced capabilities and refuses native watches rather than treating local stat success as reliable remote event evidence.

monitor exposes Inotify/FSEvents/Polling mode, native failure, hint epoch, coverage and durable_event_replay false.
Polling is permitted only by explicit MonitorPolicy approval, with a prepaid metadata budget and finite caller-driven interval; it is never an automatic response to ENOSPC.
A partial scan or unacknowledged hint remains dirty until the trusted owner acknowledges an actual complete durable reconciliation at the same epoch.
Quiet polls cannot establish absence, repair lost native coverage, claim full text/vector completeness or replay an event cursor.
No independent controller/thread is added for polling; actual native failure remains visible when fallback is approved.

## Budgets And Cancellation

A zero file, byte, time, or memory ceiling stops a listing as `Partial` / `BudgetExhausted` without claiming an empty directory.
Deny is decided before that empty budget result.
Time checks run between entries and hash chunks. They are not a preemptive syscall deadline.
Memory figures are reservations (`ENTRY_RESERVE`, `FRAME_RESERVE`, and the hash buffer), not a measured RSS.
`EMFILE` and other unclassified I/O stay `BackendError::Io`. There is no hidden file-descriptor cap.

`ReadDecision::Cancel` on `list` is `EnumerationStop::Cancelled`.
A cancel discovered before delivery keeps entries already observed.
The frozen `BackendError` has no cancel variant, so cancel on `stat` and `open_range` is `Unavailable::Unknown` and delivers nothing.
A policy revision, outside-scope set, or root identity that changed before delivery is `Unavailable::Changed` and delivers nothing.

## Example

`examples/local_read.rs` is builder smoke against disposable files. It is not an independent acceptance test.
It checks confinement, exclusion, stat without a large-file body read on Linux, a full-digest refusal when bytes outside the range change, budgets, non-UTF-8 coverage, a fifo on Linux, and a policy revision change between admission and delivery.

## Limits Still Open

Streaming visit detects child marker boundaries and mount-device crossings; the frozen list path retains its documented caller-supplied exclusions.
macOS source is compiled behind `cfg(target_os = "macos")` for birth time and generation. This Linux build does not execute it.
Other targets compile an empty crate because the module is limited to Linux and macOS.
Cooperative budgets do not bound a single kernel call.
Descriptor checks do not make a same-user directory swap atomic against a privileged remount.
No write backend, GC, package, service, or MCP cutover is included; durable reconciliation belongs to knowledge-storage.
The [M1 change record](../../docs/changes/2026-11-05-m1-storage.md) preserves the historical ENOSPC receipts and later actual positive Linux event/close/settlement passes.
No limits were changed and no positive case was skipped; native macOS qualification and independent review remain separate obligations.
