# First Usable Version

Status: qualified Linux x86_64 headless first usable version, 2026-11-09.
The engineer's 2026-11-04 sequencing, confirmed by the coordinator on 2026-11-09, puts Mac and real-model acceptance after this version.
N03 and N04 remain engineer items and are not prerequisites for N05.
This scoped result does not claim full product-contract completion or a public release.

## Qualified Artifact

- Code commit: `dfa0902f6e6de34a160da6a99454200ecf401fde`.
- Archive: `omni-knowledge-linux-x86_64-n02-dfa0902.tar.gz`.
- SHA-256: `1e9c56156cc316ee35497f6c62369cf3c900ea195bdf8cef246abb1d462175f8`.
- Installed N02 acceptance: 8/8 checks passed, no failures or blocked checks.
- Checks: archive hashes/notices, initialization, cold native lexical search, Japanese BM25, canonical MCP, legacy lexical MCP, native DEFLATE DOCX, and missing/restored parser controls.
- Parser control: a new source was refused with the sibling absent; the same source succeeded after restoration.
Actual installed parser PIDs were observed, and all 11 recorded process receipts were collected.
- Related N02a proof: 227 affected runtime/documentation tests passed with zero ignored cases; strict checks and formatting passed.
One regression proves benign DEFLATE expansion succeeds and expansion above the existing 8 MiB decoded limit is refused.
Refresh ceilings and the N02 fixture/oracle stayed unchanged.

Use the [README getting-started instructions](README.md) for Linux installation, optional provider setup, Mac source builds, and the Mac acceptance checklist.
The documentation commit adds no executable changes to the qualified artifact.

## Evidence

The qualification copy accompanies the delivery evidence under `evidence/omni/first-usable-version.md`.
Relative to that evidence directory, the exact receipts are:

- `lead-artifacts/m4-vectors/lead2-n02-completed-checkpoint.json`.
- `lead-artifacts/m4-vectors/lead2-n02-committed-artifact.json`.
- `lead-artifacts/m4-vectors/lead2-n02-committed-installed-evidence-1/result.json`.
- Open import-tag gap: `lead-artifacts/m4-vectors/lead2-n05-doc-probe-1/results.json` records failure; `lead-artifacts/m4-vectors/lead2-n05-doc-probe-evidence-1/docx-tag-oracle.stdout.log` records the empty native result.

The unchanged acceptance driver is `tests/product_acceptance/linux_installed.py`, SHA-256 `b2d9962949d9eb752b29aa0419b47fa694dab99c4355e95e0f757348e593fddc`.
It used the actual installed artifact, a fresh HOME/cache/cwd, and restricted PATH.
Earlier failing receipts remain preserved.

## Open Items

- WeMM is optional and off by default; real weights and quality are unqualified.
Provider configuration and compatibility vector indexing do not create the native model selection needed for semantic CLI/MCP queries; N03 owns that remaining binding.
- Apple Silicon macOS, native FSEvents, Claude/Codex client acceptance, and the ASR opt-in remain unqualified on this code; N04 is the engineer's acceptance.
- The getting-started probe found that legacy `add --tag` metadata did not reach native Query filters.
The example uses qualified Markdown front-matter tags; the import-tag bridge remains open, with the failing receipt preserved.
- GUI M7, Linux ARM64, physical resource guarantees, all-command implementation, notice-gap closure, notarization, and public-release approval are outside this result.
