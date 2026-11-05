# Toolchain

This guide owns setup and command discovery. The product uses Rust 1.99.0 and Cargo, selected through `mise.toml`.

## Setup And Dependencies

Use `mise` to provision and pin runtimes, not as a second command surface. Run bare Cargo commands after provisioning the declared toolchain for the executing shell. Keep `mise.toml` limited to required tool versions; do not add pass-through tasks over existing Cargo commands. Add only tools justified by accepted work. Record the versions and setup procedure with the first implementation. Read setup scripts before running them and keep procedures version-controlled. Prefer official package-manager or generator paths; add a handwritten setup layer only for a demonstrated gap.

Before adopting a dependency, identify its owner, purpose, license, and maintenance cost. Use Cargo's normal dependency and lockfile procedure and commit the corresponding lockfile change with the dependency when authorized. Prefer maintained current versions unless the accepted work requires another version. When a dependency changes architecture or workflow, record the reason in the appropriate owner or guide. When a pin or command changes, update affected setup and contribution guidance in the same change.

## Checks And Packaging

Before a root Cargo build, choose a private build dictionary cache and seed the exact retained IPADIC archive:

```sh
export LINDERA_BUILD_DICTIONARY_CACHE_DIR="$PWD/temp/build-dictionary"
python3 tools/kernel-probe/prepare-dictionary.py --cache "$LINDERA_BUILD_DICTIONARY_CACHE_DIR"
```

The supplier requires Python 3.11 or newer and refuses damaged archive/cache bytes.
Keep this environment for all Cargo commands in that build; Cargo offline mode alone does not stop build-script downloads.
Cargo's locked dependency provisioning can use public registries; deterministic checks can then use `--locked --offline` with the verified dictionary supply.
The [kernel probe guide](../tools/kernel-probe/README.md) owns exact historical downstream reproduction and dictionary identity; no model download is required.
This build cache is development state, not a product library or runtime cache location.

Use `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace --release`, and `cargo fmt --all -- --check` for the complete headless integration gate on Linux and macOS. The headless gate does not wait on a GUI launch. On macOS, `--features gui` test, Clippy, and `sh scripts/bundle-macos.sh` remain the local desktop path and are deferred with milestone M7; they are not the headless CI blocker. Native platform observations remain separate from compilation. For a focused landing, run changed tests and directly related tests in affected Cargo packages. The workspace contains the retained `omni-knowledge` package and vendored `retrieval-kernel`; shared and service packages join when their first owned behavior lands. Use `cargo test --package omni-knowledge --features gui <test-filter>` for an existing native check with an actual test filter, or select the affected shared package without GUI features. Do not treat a filtered run as a complete suite. Account for all selected tests when batching a complete gate, and retain exact code identities and outcomes.

For a documentation-only change, record the reviewed base after integration and run `git diff --check <reviewed-base>..HEAD` over the committed candidate. Use the actual reviewed baseline, not a permanently copied commit ID. Separately check staged changes with `git diff --cached --check` and unstaged changes with `git diff --check`; an empty working diff does not check committed edits. Resolve local Markdown links and anchors, and inspect retained criteria and owner consistency. Also run `cargo fmt --all -- --check` as the repository-wide format check. Content-only changes do not require Cargo compilation, Clippy, product tests, or packaging unless they alter executable examples or another accepted boundary requires them. Record omitted checks as not run, not passing. Run these applicable checks on the final integrated or rebased HEAD before pushing. The repository has no separate documentation linter or generated documentation-index command. Do not add a permanent validator merely for this sync; a bounded temporary link probe is sufficient.

Run `sh scripts/bundle-macos.sh` for the local macOS app and CLI. After a headless release build, run `python3 -B scripts/bundle-headless.py --target <host-triple> --stage <new-directory>` for the Linux or macOS headless archive. That archive installs `parser-worker` beside `omni` only when Cargo and the parent lookup both name it, and installs `omni-fsevents` beside `omni` only for an apple-darwin target. `python3 -B scripts/helper-lookup-smoke.py --stage <unpacked-archive> --evidence-dir <directory>` checks the public HTML and EPUB entry points; the macOS FSEvents watch runs only on Darwin. Packaging uses Python 3.9 or newer to collect third-party notices; the app has no Python runtime dependency. Socket mock tests require loopback permission; media tests require ffmpeg and ffprobe. Native UI and macOS-only behavior require the actual supported platform. Declare platform exclusions and real-model opt-ins before execution. A missing prerequisite after a test is selected fails or blocks the check; do not manufacture a runtime skip. No successful command proves native layout or real-model accuracy without the corresponding direct observation.

## Temporary State And Evidence

Honor the caller-selected temporary directory and use an invocation-private directory for shared runs where needed. On shared machines, choose private disk-backed storage instead of assuming shared temporary-storage capacity. Use existing Rust temporary-directory ownership and cleanup facilities; close resources and remove owned fixtures on success and failure. A forcibly killed run may need separately authorized reclamation; never delete another run's state. The [test strategy](specs/test-strategy.md#reach-state-through-shared-builders) owns fixture setup, and [verification](verification.md) owns evidence and environment limits.

Keep temporary probes under `temp/`. Add a permanent script only when repeated work establishes its need.
