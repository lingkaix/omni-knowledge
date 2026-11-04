# Kernel Workspace Integration

Date: 2026-11-04.
Status: integration and initial targeted verification complete; final rebased-head gate and full integration batch remain pending.
No production retrieval cutover is claimed.
Owners: [retrieval](../specs/search.md), [product crate boundaries](../specs/product.md#shared-crates-and-platform-boundaries), and [toolchain](../toolchain.md).
The [vendor record](2026-11-04-kernel-vendor.md) retains prior standalone and downstream evidence separately.

## Intent And Integration

Include the exact vendored `retrieval-kernel` as a root Cargo workspace member with its actual combined root Cargo.lock.
Keep the two historical downstream proof workspaces isolated; the Git proof is not a second production engine.
The retained root compatibility application is not switched to the kernel by this build integration.
Its later cutover still requires independent retrieval conformance and explicit consuming-generation compatibility checks.
No OpenKit source is changed.

A direct vendor-manifest command previously failed because the package was under a workspace root but was neither a member nor excluded.
Membership fixes that composition rather than adding a nested workspace, copying source to a different test subject, or excluding required tests.
The root workspace suite now selects the real vendor unit and executable-face tests alongside the compatibility application.
The retained vendor Cargo.lock remains historical standalone evidence; workspace builds use the root lock.

## Dictionary And Dependency Supply

Use the retained SHA-256-verified IPADIC archive and existing supplier before any Cargo build that can compile the kernel.
Keep `LINDERA_BUILD_DICTIONARY_CACHE_DIR` explicit for the complete build and use private owned build state.
Cargo offline mode alone does not prevent a dictionary build script from downloading content.
The probe and toolchain guides state the exact supplier command, required dictionary dependency version, and integrity refusal.
No model, cloud account, private library, or container is required.

The first offline resolution failed before compilation because required existing optional-platform package versions were absent from the local registry cache.
Normal Cargo provisioning resolved the combined graph and the real kernel check then passed.
This setup failure is not a passing offline run or a product defect.
The combined resolution adds kernel dependencies and necessarily advances two previously shared transitive packages.
Attempting to preserve `webpki-roots` 1.0.9 is refused because the selected dictionary dependency's `ureq` requires at least 1.0.10.
Attempting to preserve `zstd-sys` 2.1.1 is refused because the selected Jieba compression dependency requires at least 2.1.2.
No blanket dependency update, lint allowance, assertion change, or alternative production tokenizer is used.
The committed root lock identifies the actual selected versions.

## Necessity And Reasoned Ablation

Workspace membership is necessary to make root `--workspace` gates include the retained kernel and to permit direct manifest checks in this checkout.
Removing membership reproduces the observed Cargo setup refusal and drops the kernel from the application integration inventory.
The existing dictionary supplier is necessary to keep build-script archive provenance explicit; without it a cold build can contact a remote archive host even under Cargo offline mode.
No new registry, source adapter, framework, runtime process, or retrieval algorithm is added here.

## Verification And Remaining Boundary

Run root metadata, formatting, strict workspace Clippy, vendor Rust tests, and the actual path consumer against the combined root resolution.
At the integration batch, account for the complete root suite, headless release build, and exact artifact smoke.
Retain historical Git/path proof results; do not rerun measured arms merely to improve an observation.
A fresh final-head gate after integration is distinct from replacing an earlier failure.

Initial actual-root observations pass: the 48 kernel library tests and seven executable-face tests, strict workspace all-target Clippy, and the real path consumer.
The 60-second cold smoke compiled for 51 seconds and completed 47 of 48 library cases before its deadline; that interrupted selection is not called a passing suite.
The later complete selected vendor run accounts for all 55 tests and retains the bounded-smoke timeout separately.
The tests use the actual root lock, not the historical standalone lock; no existing assertions are changed.

Exact upstream notice bytes contain trailing spaces and final blank lines.
Preserve their verified provenance rather than modifying license/attribution bytes to make a generic whitespace check green.
Report that known imported-data result separately from authored committed-range, staged, and unstaged whitespace checks.
It does not authorize new whitespace defects or a higher debt allowance in authored source.

The upstream machinery stamp still binds upstream lock evidence, not the combined application graph automatically.
Before production generation reuse, the retrieval owner must bind the actual root lock, features/target, vendor patches/source, and dictionary identities at publication/open/query.
A mismatch must reject reuse and rebuild before kernel access.
This build integration does not claim that future seam, archive search, caller budgets/sharding, semantic quality, native platform effects, or the final application cutover is implemented.
