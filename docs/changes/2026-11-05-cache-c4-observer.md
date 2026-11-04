# Knowledge-Core Cache Observer Cleanup

This record owns the remaining C4 correction to the public cache process instrument.
It does not accept the cache, change production source, or add a process registry.
The files are `crates/knowledge-core/tests/cache/public_consumer.rs` and `crates/knowledge-core/tests/cache_owner.rs`.
Production lines in this change: 0.

## What Was Wrong

The parent armed its forked-child guard only after readiness output and the pipe read.
`fork-destroy-owner` could return on a pipe timeout, a pipe error, or a child that had not left userspace without a `waitpid`.
`inherited-owner` could panic while emitting readiness before any parent-side guard existed.
`PR_SET_PDEATHSIG` can kill a live orphan when the helper exits. It does not reap a zombie, and reparenting to the host is not the cleanup result.
The observer's terminal cleanup called unbounded `Child::wait`, discarded the signal and wait errors, and set `settled` even when the wait failed.
The fork guard did the same with blocking `waitpid`.
The earlier unwind admission covered a direct `admit-hang` child, not a helper that had already forked.

## What Changed

On the parent side of a successful `fork`, the child guard is armed before any readiness line or pipe read.
Each existing early return and unwind settles that child.
Settlement uses `waitpid` with `WNOHANG` or `try_wait`, bounded by two seconds.
A delivered cleanup prints `CHILD_REAPED` with the collected pid and status.
A kill or wait that does not collect the child prints `ERROR Unresolved` and does not mark the child settled.
The inherited child still blocks on its pipe until `UNBLOCK`, which the owner test sends only after the competitor observations.
`hold-ready` drives the existing `fork-destroy-owner` timeout path: the forked child does not signal readiness. The admission requires `ERROR ChildTimeout`, then `CHILD_REAPED` for that same pid, and requires the pid not to remain a zombie.
The direct `admit-hang` unwind admission stays.
No product assertion, key, or expected cache value changes.
The earlier red same-key result and the closed C1, C2, C3, and manifest proofs stay on their commits.
The full 45-case source inventory is not re-run here.

## Checks

Rust 1.99.0 and Cargo 1.99.0, via `rustup run 1.99.0`.
Focused command: `cargo test -p knowledge-core --test cache_owner --locked --offline -j 2 -- --test-threads=8`.
The same observer sources are run on this branch and on the frozen oracle `003457c0018547b6e41f224c4c6ab567f54f7cc0`.
`cargo fmt` covers the two leased Rust files.
`cargo clippy -p knowledge-core --all-targets --locked --offline -j 2 -- -D warnings` and the auxiliary consumer clippy with `--cfg cache_public_consumer` are required at the clean commit.
Exact pids, exits, and hashes are in the C4 evidence log.
