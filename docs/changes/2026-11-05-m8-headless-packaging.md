# M8 Headless Packaging

Date: 2026-11-05.
Status: initial packaging wave. Not READY and not hosted CI acceptance.
Base: `faab733753f06591ada5ee54e4c41ec486d51d81`.

## What Changed

`scripts/bundle-headless.py` stages a headless directory after a release build.
It selects workspace bin targets with no required features and excludes vendor executables.
This trunk selects `omni` and omits `retrieval-kernel`, which the root product does not call.
The seven kernel faces stay in the headless test gate. No ceiling, ignore, or platform exclusion was added.
The script calls the existing `scripts/collect-licenses.py` collector unchanged.
`manifest.json` records binary and license identities. `notice-gaps.json` classifies the four crates that still have no dedicated notice file.
Their exact cached sources are adobe-cmap-parser 0.4.1, htmlescape 0.3.1, pdf-extract 0.12.1, and type1-encoding-parser 0.1.1.
No license text was invented.

`.github/workflows/ci.yml` uses that script for Linux x86_64 and macOS arm64 headless archives.
The macOS job no longer runs `python3 scripts/gate.py gui` or uploads an unsigned `.app`.
GUI milestone M7 stays deferred. Local `scripts/bundle-macos.sh` is unchanged.
`workflow_dispatch` remains in the file and was not invoked.

`first-usable-version.md` is IN PROGRESS.
The root guide, documentation map, toolchain, script guide, and dependency guide name the headless archive and the deferral.
`CONTRIBUTING.md` and `docs/change-execution.md` record the 2026-11-05 01:05 risk-based review rule: independent review for durability, the hostile ZIP parser, MCP or HTTP authority, and the admission seam; ordinary work is builder tests, the gate, and a forward landing.
No product specification, Cargo manifest, lock, Rust source, test, vendor tree, or `scripts/gate.py` file changed.

## Local Observation

Private target, `TMPDIR`, and dictionary cache. Rust 1.99.0. Cargo jobs 2. `--locked --offline`.
Dictionary prepare passed: `IPADIC 5.3.0-fmt2` verified.
`cargo build --workspace --release --no-default-features --locked --offline -j 2` passed.
The staged Linux archive contains `omni`, `LICENSE`, `ThirdParty`, `manifest.json`, and `notice-gaps.json`.
Collector summary: 298 packages, 543 dedicated notice files, two MPL archives, and four gap archives.
Unpacked `omni --help` and `omni doctor --json` passed through `scripts/gate.py smoke`.
`--verify` passed.
Archive SHA-256: `adb739c31724c9f13872637deaf635064c284b17c78fecc589bc0999ea89a11f`.
`omni` SHA-256: `e9b162accb3fb39b320277f73165c42790fbbbd3f0284a70ca2f1cd6eeae783d`.
`cargo fmt --all -- --check` passed.
Current-subject notice collection passed 8 tests at collector SHA-256 `e3bc710d94b36798f2ac3016dec75c31ad521e0386099ec20b92e60310710376`.
The full headless suite, macOS execution, and hosted CI were not run.

## Still Pending

Hosted Linux and macOS CI, milestone authority, durability review, ZIP admission, and actual service entry points.
Linux arm64 is unqualified.
The unsigned app, Developer ID, notarization, and Gatekeeper remain with M7.
