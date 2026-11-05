# Canonical cache fixture parent

This record owns the cache fixture directory parent and the owned-child observations in the cache support module and the public consumer.
It does not change production cache checks, history fixtures, or `crates/knowledge-core/tests/cache_owner.rs`.
Hosted macOS job 127081567085 failed 17 cache cases at cache identity with `UnsafeRoot` because the fixture parent kept the `/var` temporary alias.
`Lab::create` now creates the owned directory under the canonical temporary parent and requires that stored path to stay canonical.
Production symlink, permission, and cache-identity checks stay in place.
The start token stays `{pid}:{start}` with one start field.
Linux still reads `/proc/<pid>/stat` field 22.
macOS reads `proc_pidinfo` `PROC_PIDTBSDINFO` for that pid only and records the start time in microseconds.
A short write, a pid mismatch, or a failure other than an absent pid is an observer failure, not a missing token and not a pass.
The forked child counts as a zombie only when that pid's `pbi_status` is `SZOMB` and `waitid` with `WEXITED`, `WNOHANG`, and `WNOWAIT` reports that same pid's exit.
`WNOWAIT` does not collect the child.
The existing `ChildGuard` still collects it with bounded `waitpid`.
Linux `prctl` and the macOS after-fork C thread stay.
The expected parent is still captured before `fork`.
`cache_owner.rs` still has its own `/proc` state reader.
This loan does not change that file, so that reader is not a macOS exit observation.
The separate history `Busy` failure is outside this change.

## Checks

Rust 1.99.0 and Cargo 1.99.0.
No macOS target or SDK was installed.
macOS was not compiled or run.
Commands used `--locked --offline`, Cargo jobs 2, `CARGO_TARGET_DIR=/tmp/omni-native-cache-port-a742/target`, and `TMPDIR=/tmp/omni-native-cache-port-a742/tmp`.
`cargo test -p knowledge-core --test cache_contract --test cache_owner --locked --offline -j 2 -- --test-threads=2` reported 16 passed and 3 passed, with 0 failed and 0 ignored.
`cargo test -p knowledge-core --test cache_contract --test cache_owner --locked --offline -j 2 -- --list` listed 16 tests and 3 tests, and 0 benchmarks.
`cargo check -p knowledge-core --all-targets --locked --offline -j 2` passed.
`cargo clippy -p knowledge-core --all-targets --locked --offline -j 2 -- -D warnings` passed.
`cargo fmt --all -- --check` passed.
The public consumer was clippy-clean with `--cfg cache_public_consumer`, `--locked --offline`, jobs 2, and its private target.
A clippy launch from the repository root without that cfg stops in the harness half of the shared support file and is not the helper gate.
The same 19 tests passed again with `TMPDIR` set to a symlink of a real directory.
That alias run is a Linux probe, not a macOS result.
Raw command output is in `native-cache-port-a742` under the external evidence directory.
