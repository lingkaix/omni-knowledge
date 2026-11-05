# Ordinary Local Mount Test Corrections

This record corrects the independent local-mount tests after review.
It supersedes the product-bug classification of the four reds and the claim that two outside-file reads were an adapter symlink escape in `2026-11-05-local-mount-tests.md`.
The adapter sources, manifests, dependencies, gates, and the producer record were not edited by this correction.

## Oracle Corrections

A foreign library list and a foreign mount stat stay refusals and deliver nothing, and the frozen error is `InvalidRequest`.
Policy `Deny`, a presented cursor, and non-delivery remain separate cases.
An offset equal to the verified file size with a matching digest returns empty bytes and `end_of_file` true.
An offset strictly greater than that size is `InvalidRequest` while the digest matches, and drift at either position is `Unavailable::Changed`.
`max_bytes` equal to the source length does not include the relative path, and `max_memory_bytes` of 17 does not include the metadata reservation.
Those ceilings are refusals.
The requested-length guard is a longer request under a byte budget that already covers the source plus its path and a memory budget that already admits the passing range read.
The positive content-read control remains the finite `room` range read.

## Observation Window

Fixture digests and metadata are built before the product window.
The escaping symlink still requires `Unavailable::Unsupported` and no content syscall on the outside file.
The non-UTF-8 listing is its own test so a symlink failure cannot skip it.
Content filters were not narrowed and hits were not subtracted.

## Observer Settlement

The listener has one `OwnedFd` owner.
A blocked action ends the owned child with `exit_group` rather than closing a raw descriptor, and a panic joins both threads before it resumes.
Watched files match device and inode for the worker thread id.
A failed resolution is unresolved, not a quiet miss.
Before and after controls must observe both `read` and `pread64`.
The x86-64 syscall numbers are explicit.
This observer does not cover `io_uring` or other threads.
Owned-child modes `normal`, `content`, `panic`, and `timeout` are real tests.
Inotify `ENOSPC`, fanotify `EPERM`, and the absence of `strace` stay host facts from the original run.
No host limit or package was changed.

## Memory Regressions

A caller-created 8192-byte library id under a 512-byte memory budget must be `BudgetExhausted` before the adapter allocates that id.
The same stat with `room` memory must succeed and must show that allocation, or the probe is not evidence.
Policy values used in that window are moved from storage created beforehand.
An empty directory listed with one positive byte of memory must not be `Complete`.
A memory budget of the retained request plus one hash byte stays a refusal while metadata, the retained range, and the hash buffer are simultaneously live.
These regressions are frozen against the unchanged adapter.
The producer memory fix is a separate verification arm and is not part of this change.
