# Kernel Downstream Proof

This directory contains two isolated Cargo workspaces that compile and run the same public-API consumer.
They do not join the application workspace and do not install a second production engine.
`git/` retains historical proof of the exact public revision; `path/` proves the selected vendored dependency.
Each workspace retains its actual Cargo-generated dependency lock.
The standalone vendor lock records its Rust unit/process test resolution.
The application's future root lock remains the workspace owner's responsibility.

## Run

Use Rust/Cargo 1.99.0 and Python 3.11 or newer on Linux or another supported Unix host.
The recorded platform proof is Linux x86_64 only.
No GUI, model, container runtime, or real user library is required.
A cold Cargo cache needs public Git/crates.io access; dictionary supply uses the retained verified archive instead of a build-time download.

Choose a dedicated local target and dictionary cache before running Cargo.
The following example bounds Cargo jobs to two and keeps all generated build state outside the repository:

```sh
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR="${TMPDIR:-/tmp}/omni-kernel-target"
export LINDERA_BUILD_DICTIONARY_CACHE_DIR="${TMPDIR:-/tmp}/omni-kernel-lindera-cache"
python3 tools/kernel-probe/prepare-dictionary.py --cache "$LINDERA_BUILD_DICTIONARY_CACHE_DIR"

cargo run --locked --manifest-path tools/kernel-probe/git/Cargo.toml
cargo run --locked --manifest-path tools/kernel-probe/path/Cargo.toml
cargo test --locked --manifest-path tools/kernel-probe/git/Cargo.toml
cargo test --locked --manifest-path tools/kernel-probe/path/Cargo.toml
cargo clippy --locked --manifest-path tools/kernel-probe/git/Cargo.toml --all-targets -- -D warnings
cargo clippy --locked --manifest-path tools/kernel-probe/path/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path tools/kernel-probe/git/Cargo.toml -- --check
cargo fmt --manifest-path tools/kernel-probe/path/Cargo.toml -- --check
```

Before concurrent invocations, select distinct target/cache paths or serialize builds against the same paths.
The cache supplier verifies both the vendored archive and any existing cached archive; it does not replace damaged cache bytes silently.
It is adapted from the pinned upstream supplier, with only its documentation and default archive location changed.

The consumer publishes synthetic English, Chinese and Japanese files through `Directory::inventory` and `Kernel::publish`.
It queries lexical, exact and LF-line regex modes, checks canonical offsets and overlapping exact occurrences, reopens the kernel, verifies no-op publication, and withholds changed source hits.
It uses real filesystem generations and the linked kernel, not a fake producer or a subprocess pretending to be the library.
Temporary fixtures are invocation-private and removed by `tempfile` ownership.
A failed assertion or kernel error returns a nonzero result.
This is feasibility/characterization evidence, not the independent M4 conformance suite, arbitrary-scale proof, or Mac qualification.

## Root Workspace And CI Dictionary Supply

After the workspace owner admits the vendor and generates the actual root Cargo.lock, use the same verified supplier before any root build.
Keep the two probe workspaces isolated; the application must use only the path dependency.
Use explicit root workspace exclusions for `tools/kernel-probe/git` and `tools/kernel-probe/path` if a broader member glob would otherwise select them.

```sh
# Public Cargo dependency provisioning; run once while network access is permitted.
cargo fetch --locked

# Deterministic build phase: seed verified retained dictionary bytes, then use Cargo offline.
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR="${TMPDIR:-/tmp}/omni-kernel-root-target"
export LINDERA_BUILD_DICTIONARY_CACHE_DIR="${TMPDIR:-/tmp}/omni-kernel-root-lindera-cache"
python3 tools/kernel-probe/prepare-dictionary.py --cache "$LINDERA_BUILD_DICTIONARY_CACHE_DIR"
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build --workspace --locked --offline --release
```

The supplier installs the verified archive at `$LINDERA_BUILD_DICTIONARY_CACHE_DIR/5.3.0-fmt2/mecab-ipadic-2.7.0-20250920.tar.gz`.
Lindera 5.3.0 reuses those bytes and builds its embedded target dictionary locally.
The root lock must retain the proved lindera-ipadic/lindera-dictionary 5.3.0 resolution for this cache recipe; a dictionary dependency change needs a new verified supply recipe before building.
Do not run with an unset/different cache environment or assume `cargo --offline` alone constrains build-script HTTP.
These root commands describe the handoff recipe, not an M0 claim that application integration has already run.

## Vendor Checks

The application workspace does not yet admit the vendor as a member or explicitly exclude it.
Path dependency builds work, but Cargo refuses direct vendor-manifest tests in that parent workspace.
Until the workspace owner resolves membership, test an exact committed-source snapshot without the unrelated parent workspace manifest:

```sh
(
  gate=$(mktemp -d "${TMPDIR:-/tmp}/omni-kernel-gate.XXXXXX")
  trap 'rm -rf "$gate"' EXIT
  git archive HEAD vendor/retrieval-kernel vendor/upstream | tar -x -C "$gate"
  cargo fmt --manifest-path "$gate/vendor/retrieval-kernel/Cargo.toml" -- --check
  cargo clippy --locked --manifest-path "$gate/vendor/retrieval-kernel/Cargo.toml" --all-targets -- -D warnings
  cargo test --locked --manifest-path "$gate/vendor/retrieval-kernel/Cargo.toml" --all-targets
)
cargo build --locked --manifest-path tools/kernel-probe/path/Cargo.toml
```

This gate uses the exact final HEAD source, manifest and retained lock with no test/source edits.
It does not establish application workspace integration.
When the workspace owner includes `vendor/retrieval-kernel` as a member or explicitly excludes it, direct manifest checks can replace the snapshot route.
Recompute the gate from that actual workspace; do not try to test a nonmember dependency through the probe's `-p retrieval-kernel` selection.
Do not silently replace the upstream lock evidence with a generated application lock.
The [vendor guide](../../vendor/retrieval-kernel/VENDOR.md) explains provenance, the minimal patches, unimplemented seams, and the required actual consuming-lock compatibility binding.

The exact upstream notice files contain original trailing spaces and final blank lines.
Do not normalize license or attribution bytes to satisfy a whitespace check; verify their provenance hashes instead.
Run `git diff --check` on authored files and report the retained upstream-notice whitespace separately.

The copied upstream Python notice/image tests depend on upstream repository collectors and image infrastructure.
They remain unchanged as source evidence but are not portable checks of this isolated crate; M0 runs the Rust suite and verifies retained notice/dictionary hashes.
No built-image notice audit or newly resolved transitive-license completeness is claimed.
Use the [change record](../../docs/changes/2026-11-04-kernel-vendor.md) for proof scope and limits.
