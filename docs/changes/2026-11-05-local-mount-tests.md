# Ordinary Local Mount Observations

This record owns independent tests of the committed ordinary local mount adapter.
It does not change the adapter, and it does not authorize a repair of the four red cases below.
The lease is `crates/knowledge-local/tests/local_mount.rs` and this record.
No manifest, lock, dependency, crate source, example, owner, gate, core, cache, or wire file was edited.

## Source And Base

The requested subject is the ordinary local `MountBackend` at commit `1dad86ccce4dacca9b12a79dcb98bc32732b7d2a`.
Expectations come from the storage, product, and service owners, the knowledge-local crate guide, and the public Rustdoc of `knowledge-local` and `knowledge-core`.
They do not come from adapter output, from the builder example, or from a private helper.
The adapter bodies `src/lib.rs` and `src/native.rs`, the builder example, and the producer change record were not read.

## What The Suite Checks

The tests call the public `LocalMount` constructor and the public `MountBackend` methods on a real directory owned by this process.
A policy value is only the trusted-caller input the constructor already requires.
Identifiers, digests, cursors, and naked public data are not treated as permission.
The covered effects are non-hashing stat, a bounded ordinary listing, and a range read that hashes the current file with the locked `sha2` 0.10.9 primitive.
Empty-complete, zero budget, tight budget, denial, cancellation, an unestablished premise, a missing path, a partial stop, exclusion boundaries, root replacement, and a revision change are separate cases.
Symlinks, a fifo, a unix socket, and a non-UTF-8 name are separate cases.
A mode `000` file and a mode `000` directory use this unprivileged uid, and the original mode is restored on cleanup.
There is no device node, because creating one returns `EPERM` here.
There is no offline stimulus, and an empty complete listing is not reported as offline.

## Observer

Inotify could not watch a directory: `inotify_add_watch` returns `ENOSPC` against this user's existing watch count, and `fanotify_init` returns `EPERM`.
The read check is a per-worker-thread seccomp user notification that continues every trapped syscall.
Coverage is `read`, `pread64`, `readv`, `preadv`, `preadv2`, `mmap` of a file descriptor, `sendfile`, `splice`, and `copy_file_range`.
`openat` and `openat2` are continued and are not content hits.
Stat metadata calls, `io_uring`, and `/proc` physical byte counters are outside this coverage.
A quiet result counts only when a warm-cache `read` and `pread64` of the control file were observed both before and after the product call and the hit log did not overflow.
The successful interior range read is the positive product control: it must itself produce a content hit.
That control passed, so the quiet passes are not an untested observer.
The escaping-symlink case is the negative ablation: the call returned `Unavailable(Unsupported)` and the observer still recorded two `read` hits on the outside file.
A mode `000` file cannot be the warm-cache control, because this uid cannot read it.
That case watches the locked path and uses a readable control file outside the bound root.

## Pinned Results That Stay Red

These expectations were fixed before the candidate ran, and they were not edited after the mismatch.
An `Admit` whose library id or mount id differs from the bound mount was pinned as `Unavailable(Denied)`.
The candidate returned `InvalidRequest` for both the other library's list and the other mount's stat.
An offset equal to the file length with length 1 and a matching digest was pinned as `InvalidRequest`.
The candidate returned `Ok` with empty bytes and `end_of_file: true` (`bytes_read` 18 on a 10-byte file).
The same past-end request after a byte change, using the old digest and current metadata, returned `Unavailable(Changed)`, which matches the drift pin.
A 16-byte request of a 64-byte file with `max_bytes` 64 and `max_memory_bytes` 17 was pinned as success, because retained bytes plus one hash byte fit.
The candidate returned `BudgetExhausted`.
The tighter quiet ceilings in that test matched `BudgetExhausted` and showed no content hit: `max_bytes` 63, `max_memory_bytes` 16, `max_memory_bytes` 0, and `max_files` 0.
An escaping symlink's range read was pinned as `Unavailable(Unsupported)` with no content syscall on the outside file.
The candidate returned `Unavailable(Unsupported)` after two `read` syscalls on that file.

## Non-Claims

This run is Linux on this host only.
A macOS binary is not a pass, because the suite is `cfg(target_os = "linux")`.
The suite does not claim NAS, cloud, account authorization, power loss, or a completed M1 milestone.
It does not claim that the adapter detects child libraries, inner mounts, or provider hydration.
`BackendKind::Local` is the kind the caller selected.
`stable_file_id`, watch, conditional write, and residency capabilities stay false in the passing capability test.
The service frame size is not an adapter budget.
The harness bound around the fifo child and the seccomp observer is not a product kernel-call timeout.
No producer repair was applied.

## Run

Command: `cargo test -p knowledge-local --locked --offline --jobs 2` with `CARGO_BUILD_JOBS=2`, a private target directory, and `--test-threads=2`.
Outcome: library unit tests `0 passed`; `tests/local_mount.rs` `16 passed; 4 failed` in `0.02s`; exit status 101.
Command: `cargo test -p knowledge-local --doc --locked --offline --jobs 2`.
Outcome: `Doc-tests knowledge_local` ran 0 tests, exit status 0.
Command: `cargo fmt --all -- --check`.
Outcome: exit status 0.
Command: `cargo clippy -p knowledge-local --all-targets --locked --offline --jobs 2 -- -D warnings`.
Outcome: exit status 0.
Toolchain: rustc 1.99.0. Host: Linux 7.2.5-3-omarchy, uid 1000.
The raw log is outside this repository.
