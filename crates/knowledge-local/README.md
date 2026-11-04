# Knowledge Local

Confined read adapter for one explicitly selected ordinary local directory.
It implements the frozen [`MountBackend`](../knowledge-core/src/backend.rs) seam from `knowledge-core`.
The [storage owner](../../docs/specs/storage.md) governs mount identity, verify-on-read, budgets, and exclusions.
[Product](../../docs/specs/product.md) places ordinary local reads ahead of the optional GUI.
[Service](../../docs/specs/service.md) still owns exposure and read admission; this crate does not cut over to it.

This is a builder implementation. It is not M0 acceptance, M1, or an application authority change.

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

Inner mount points are not detected or refused.
macOS source is compiled behind `cfg(target_os = "macos")` for birth time and generation. This Linux build does not execute it.
Other targets compile an empty crate because the module is limited to Linux and macOS.
Cooperative budgets do not bound a single kernel call.
Descriptor checks do not make a same-user directory swap atomic against a privileged remount.
No write, watch, tracker, GC, job, package, service, or MCP integration is included.
