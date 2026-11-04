# Native Kernel Observers

This record classifies two fixture observations.
It does not change production path handling, resource ceilings, or controller policy.
The executable's own failing `getrlimit` or `setrlimit` call is the resource diagnostic subject.
`faces.rs` stays outside this change.
No product test or assertion is removed.

## Directory Race Target

`directory_stat_and_open_races_are_coverage_only` removes one fixture file before `stat` or `open`.
`Directory::new` canonicalizes the root before that walk.
`TempDir` keeps the created `TMPDIR` path.
On macOS that path can retain the `/var` alias while the syscall path is `/private/var`.
The fixture now resolves only its fault target through the canonical root.
The pre-stat and pre-open removal is unchanged.
The coverage, hit, unavailable-count, and privacy-marker assertions are unchanged.
Production code does not gain a path-normalization workaround.

Linux admission uses one owned `TMPDIR` symlink.
The unchanged test reports `documents.len` 2 against 1 and exits 101.
The corrected test passes on that alias and on a normal Linux temporary directory.
The normal directory also passed before the correction.
The hosted macOS log remains the original red result at `tests.rs:1977`.
This Linux alias does not replace that macOS run.

## Resident-Set Unit

`large_file_actual_allocations_and_peak_memory` still requires the largest allocation to stay below the 48 MiB source and peak resident memory to stay below 512 MiB.
XNU assigns `ru_maxrss` from `resident_size_max` in `bsd/kern/kern_resource.c`.
The Mach header `osfmk/mach/task_info.h` defines `resident_size_max` as maximum resident memory size in bytes.
Linux `getrusage(2)` reports `ru_maxrss` in kibibytes.
The observer converts those units to bytes and compares them with `512 * 1024 * 1024`.
An archived Apple manual that says kilobytes is not the authority for this observer.
The allocator check, both exact-marker checks, and the child process identity stay in place.
The parent failure text now includes the child stdout and stderr as diagnostics.
Those streams are not a new deciding oracle.
The corrected Linux child reported `ru_maxrss` 30288 and `peak_rss_bytes` 31014912.
That byte value is 30288 times 1024, and it remains under the fixed 512 MiB ceiling.
The largest single allocation was 4194304 bytes, below the unchanged 48 MiB source bound.
A Linux pass does not prove the macOS run. The hosted macOS child failure remains unobserved until the lead runs the corrected test on that runner.

## Removed Separate-Process Probe

Commit `a97f92f85ef5b981bbd98d21681d89bb505ae700` added a 617-line probe that called `getrlimit` and `setrlimit` in other processes.
That probe cannot identify the executable's failing call or the limits that call inherited.
The follow-up removes the probe, its CI steps, and the claims that described it.
It does not add another observer framework, and it does not change the executable's limits.
The existing face tests still execute the native guard and fail closed.
The prior probe commit and its admission logs remain as rejected-overdesign history.

## Not Claimed

This change does not accept the macOS kernel suite, the seven face tests, or overall M0.
The hosted macOS result remains 46 of 48 library tests and 0 of 7 face tests at head `6a52d333f37c8e4b9be3d2985f08e3ac21a66dae`.
The face failure at that head is `process resource enforcement failed`.
`PROVENANCE.json` still records the original vendored hash of `src/tests.rs`.
Updating that inventory is outside this lease.
