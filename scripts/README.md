# Build, CI, And Runtime Scripts

Read [the root guide](../README.md) and [toolchain](../docs/toolchain.md) first.
[The CI change record](../docs/changes/2026-11-04-ci.md) records scope, admission evidence, and known gaps.

## Deterministic CI Gate

`python3 scripts/gate.py self-test` admits command success, deliberate failure, timeout, and test-accounting refusals with stable stand-ins.
It does not prove product behavior.
`python3 scripts/gate.py headless` runs workspace formatting, all headless tests including doctests, Clippy with warnings denied, a workspace release build, and release CLI smoke.
`python3 scripts/gate.py tests` runs only the test inventory and accounted headless suite for diagnosis.
All Cargo commands use the lockfile; tests and release builds use `--workspace --no-default-features` so later workspace members join without a crate registry.
Formatting uses `--all`; Clippy also uses `--all-targets`.
The root compatibility member remains included.

Install the Rust version from `mise.toml`, rustfmt, Clippy, system ffmpeg/ffprobe, and Python 3.9 or newer before running the gate.
Use `CARGO_BUILD_JOBS=2` and a private `CARGO_TARGET_DIR` for shared-host local checks.
No container, model, cloud account, or private library is needed.
When the kernel supplier is present, it additionally requires Python 3.11 or newer.
The workflow uses standard runner package managers for FFmpeg and retains actual tool versions in evidence; it does not claim fixed FFmpeg output bytes across OS package versions.

## Pinned Kernel Dictionary Supply

Before any Cargo build, test, or lint in the gate, it runs the actual `tools/kernel-probe/prepare-dictionary.py` supplier.
The supplier verifies the repository's pinned IPADIC archive and prepares the `5.3.0-fmt2` cache; it does not download a replacement.
The supplier owns archive identity and verification; this gate adds no parallel checksum list or provider registry.
`LINDERA_BUILD_DICTIONARY_CACHE_DIR` selects the caller's private build cache and is passed to all child Cargo commands as an absolute path.
Without an explicit setting, the gate uses `dictionary-cache/` under `CARGO_TARGET_DIR` or the checkout's `target/` directory.
CI checkouts and shared-host lane targets are isolated; do not select a shared writable dictionary cache for unrelated runs.
An earlier head with neither supplier nor vendored kernel reports dictionary preparation as inapplicable.
If `vendor/retrieval-kernel` exists or the root manifest names that member, a missing supplier fails before Cargo starts.
An invalid source or cached archive also fails; there is no implicit dictionary-download fallback, including for offline Cargo use.
Cargo's registry dependency provisioning remains separate from dictionary supply.

`python3 scripts/gate.py prepare` performs only this prerequisite.
The local bundler uses `python3 scripts/gate.py bundle-build` so the prepared environment reaches its Cargo build without a second shell-side setup owner.
That internal helper keeps its supplier in the enclosing bundle process group and replaces itself with Cargo; it does not create a second Cargo supervisor/session.
During `gui`, the outer gate owns that entire group and its effective timeout/interruption cleanup, including the nested preparation/build.
Standalone local bundling retains the original foreground build behavior; use `gui` for the bounded CI composition.
See the [nested lifecycle correction](../docs/changes/2026-11-04-bundle-process-ownership.md); admission stand-ins are not native Mac proof.
For a direct Cargo invocation outside the gate, prepare explicitly first:

```sh
export LINDERA_BUILD_DICTIONARY_CACHE_DIR="$PWD/temp/dictionary-cache"
python3 tools/kernel-probe/prepare-dictionary.py --cache "$LINDERA_BUILD_DICTIONARY_CACHE_DIR"
cargo test --workspace --offline
```

The example requires the vendored supplier/archive and already provisioned Cargo registry dependencies.
An archive-supply pass alone does not prove a complete offline workspace build.

## Test Accounting And Evidence

The gate collects actual libtest inventories with `--list` and an ignored-only inventory before execution.
It rejects zero collection, unlisted outcomes, missing outcomes, failures, and undeclared ignored cases.
It uses `--no-fail-fast` to retain other target outcomes after a failure and never supplies a Cargo name filter.
The only declared ignored case is `actual_local_asr_recognizes_known_speech` on macOS, which requires the separately installed model and generated speech fixture.
That case remains an explicit real-model opt-in, not deterministic CI evidence.
Compile-time platform exclusions stay in the test owner's Rust `cfg` declarations, not in a second gate skip list.
New ignores need owner authorization and a gate declaration; a missing prerequisite after selection fails the gate.

The macOS headless inventory automatically includes FSEvents tests when their adapter lands.
Adapter owners must give those tests names containing `fsevent` so the evidence report can identify them.
The report states **PENDING** until such cases actually pass on macOS; a zero-test filter never supplies proof.
This name convention labels evidence only and does not change which tests execute.
An unrelated suite failure conservatively leaves adapter evidence pending.
Native UI, real OneDrive/NAS, WeMM, user signing prompts, and Gatekeeper remain separate observations.
Linux ARM64 is not configured or qualified by the x86_64 lane.

Evidence defaults to `temp/ci-evidence/`: exact commands, code identity, tool versions, raw output, test inventory, FSEvents status, and terminal PASS/FAIL/TIMEOUT results.
Use `--evidence-dir PATH` to isolate runs and `--timeout-seconds N` to set the positive per-command deadline (default 1800 seconds).
A timeout kills the command's process group, reaps its leader, and fails the gate.
Normal exit and interruption also settle remaining command descendants.
`python3 scripts/gate.py smoke --binary PATH` checks the exact executable's help and model-free JSON doctor output under a maximum 60-second deadline.
It checks FFmpeg discovery without opening a library or contacting a provider.
It is artifact startup/health evidence, not the full service or native GUI acceptance suite.

`.github/workflows/ci.yml` runs two fixed jobs on relevant main pushes or manual dispatch: Ubuntu 24.04 x86_64 and macOS 15 ARM64.
Each job checks its actual architecture and installs the repository-selected Rust with a minimal profile.
Actions are pinned by commit; checkout does not retain Git credentials.
New pushes cancel obsolete runs on the same ref.
Artifacts and diagnostic evidence have 14-day retention.
The Linux archive and macOS ZIP are unpacked and their bundled CLI is smoke-tested before upload.
The ZIP preserves app permissions and bundle structure; no release or deployment is performed.

## macOS Bundle

`sh scripts/bundle-macos.sh` builds the workspace release CLI and GPUI executable into a locally ad-hoc-signed app under `dist/`.
It honors Cargo's configured target directory, assembles a fresh bundle, signs the nested CLI before the outer app, and verifies the complete signature before replacement.
It preserves each previous bundle at the printed `dist/Omni Knowledge.previous-*` path, including added files.
Ad-hoc signing needs no developer account and does not provide Developer ID or notarization for distribution.
The script does not install or publish the app.

`python3 scripts/gate.py gui` requires native macOS ARM64, lints GUI targets, runs an additional accounted workspace suite with `--features gui`, builds that feature, bundles, and smoke-tests the bundled CLI.
GUI-feature test evidence is isolated in `temp/ci-evidence/gui-tests/`; the headless suite remains a separate configuration.
It calls `sh scripts/bundle-macos.sh --ci-unsigned`, which requires both `CI=true` and `GITHUB_ACTIONS=true`.
This explicit option omits app signing and app-signature verification only in the requested CI path.
It preserves required ARM64 linker signatures in executable files; the enclosing app has no distribution signature or notarization.
The no-argument local path retains all existing signing checks, including post-install verification and recovery behavior.
The CI artifact is labelled unsigned; this option does not change local signing defaults or claim native launch/Gatekeeper approval.

## Other Helpers

`scripts/collect-licenses.py` uses development-time Python 3.9 or newer to collect target-specific locked dependency notices and exact MPL source archives.
Its default remains the Darwin GUI graph for the existing macOS bundler.
Use `python3 scripts/collect-licenses.py --target x86_64-unknown-linux-gnu --headless OUTPUT` for the Linux CLI archive; `OUTPUT` must be a new directory.
The inventory binds the target, selected features, root package, and exact workspace lock digest and conservatively includes reachable normal/build dependencies, not dev-only edges.
It can reuse retained upstream notices only when crate/version/VCS identity and notice hash all match; it introduces no second notice registry.
For a registry package with no discovered notice text, it includes the exact checksum-verified original source archive as evidence and retains the gap rather than inventing copyright terms.
CI includes these notices/source archives in the Linux archive and checks the unpacked inventory and required Tantivy notice evidence before upload.
Coverage gaps remain explicit; discovered text or a source archive is not a complete legal compliance certification.
The [Linux artifact correction](../docs/changes/2026-11-04-linux-artifact-notices.md) records the actual composition and retained upstream notice gaps.
Python is not required by the installed CLI or app.
`scripts/setup-asr.sh` prepares a pinned optional whisper.cpp and multilingual model under ignored `temp/asr-runtime/`.
Read [the multimodal guide](../docs/multimodal.md) before setting executable and model paths in a library configuration.
