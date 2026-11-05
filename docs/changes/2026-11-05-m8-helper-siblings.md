# M8 helper sibling packaging

Date: 2026-11-05.
Status: first-usable DRAFT packaging delta. Not READY. Not full M8 acceptance.
Base: `5571116ecbd0af65c17f50a69e09bc6012edd4e7`.
Selector source: committed packaging delta `5c6d453ee9e27785823b2f724956279173808e79` only.
Producer `9a807f9` and held-fd API `2e64` were not imported. No production Rust, Core, authority, `scripts/gate.py`, or `Cargo.lock` edit.

## What Changed

`scripts/bundle-headless.py` still creates a missing stage parent and still refuses an existing file, directory, or dangling link.
Cargo metadata JSON remains the bin authority. `parser-worker` is installed only when that metadata names exactly one normal bin from `parser-worker.rs` and the knowledge-processing parent lookup joins `parser-worker`.
This trunk has neither, so the helper is not invented. `omni-fsevents` must be the single knowledge-local normal bin, and `fsevents.rs` must join `omni-fsevents`.
Apple-darwin archives install that sibling beside `omni`. Every other triple omits it. A Linux binary is not FSEvents proof.
Unmapped bins, install-name collisions, and helper bytes that match `omni` fail the stage. Missing selected refs stay hard failures.
Shipped helper normal and build registry crates close with the existing collector. `knowledge-local` registry crates are required only when the FSEvents helper is installed.
`scripts/gate.py` diagnostic separation is unchanged.

`scripts/helper-lookup-smoke.py` installs the unpacked archive and calls the public file APIs through one directory-fd account: `extract_admitted` and `read_epub_admitted`.
On this trunk the lookup sample is in-process because no `parser-worker` file is present. The macOS harness calls `LocalMount::watch` on a canonical root and is compiled for native Darwin CI only. This host records that path as unobserved and does not build an Apple target.

## Local Observation

Private target `temp/m8-native-target`, `TMPDIR` `temp/m8-native-tmp`, Rust 1.99.0, Cargo jobs 2, `--locked --offline`.
Linux release binary `omni`: 25725760 bytes, mode 0755, SHA-256 `c61f5a1844853fba3a817a99c80b9cbb94fbbfdcc8bd0f60671e1730bc7c134b`.
Staged `dist/linux-x86_64` contains `omni`, `LICENSE`, `ThirdParty`, `manifest.json`, and `notice-gaps.json`. No `parser-worker`. No `omni-fsevents`.
Manifest: `parser_helper.status` `not-selected`; `fsevents_helper.status` `omitted`; vendor omission `retrieval-kernel`.
Notice inventory: 307 packages, 547 dedicated notice entries. `Sources` retains cssparser 0.38.0 and dtoa-short 0.3.5 (MPL-2.0) plus the four named upstream gap crates.
Archive `dist/omni-linux-x86_64.tar.gz` SHA-256 `7dccf507ad56a4df72ddd41e2849f8332c0c53f7139c471cc1ace46137625626`. Unpacked `--verify` passed.
`scripts/gate.py smoke` on the unpacked binary: `omni --help` PASS exit 0, `omni doctor --json` PASS exit 0.
Helper lookup: HTML status `extracted`, EPUB text present, `holdings_after` 0, harness pid 47655 reaped, `native_mac` `unobserved`.
`tests.tools.packaging_selection` and `tests.tools.headless_stage`: 23 tests OK. `rustfmt --edition 2024 --check` on both harnesses passed.
The full headless suite was not run. The native watch ENOSPC case was not rerun and stays red. No Apple SDK was installed.

## Integration

The lead removed the selector's redundant exact-README-wording condition: prose is not required to select a Cargo target whose actual parent lookup is known. The old tree-absence assertion was replaced with a check of the actual current Cargo graph, so adding the required processing helper does not intentionally break packaging. All 19 selection cases remain, plus six stage/accounting controls (25 total); CI runs them for both platforms. Producer artifact hashes above remain historical observations, not hashes of a newly rebuilt integration artifact.

## Still Pending

Native macOS CI must execute the FSEvents harness through `LocalMount::watch` on real owned files. Until that run, macOS helper behavior is unobserved.
Hosted CI, milestone authority, and product acceptance remain open.
