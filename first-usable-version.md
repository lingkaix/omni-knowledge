# First Usable Version

Status: qualified Linux x86_64 headless first usable version, updated for N06 on 2026-11-09.
Mac and real-model acceptance follow this version; N03 and N04 remain engineer items.
This result does not claim full product-contract completion or a public release.

## Qualified Artifact

- Code/build commit: `c4d746e4e6f10f15a4b016c5280687a5db551cfe`.
- Canonical import/init change: `5e65e3a8f3e7c9c04108786ba91618378f6ba6c0`.
- Separate fixture-only migration: `c4d746e4e6f10f15a4b016c5280687a5db551cfe`.
- Archive: `omni-knowledge-linux-x86_64-n06-c4d746e.tar.gz`.
- SHA-256: `db6efe41c72e60254eab7b776d563d1042e4af3b16bce902d152845f59393e86`.
- Installed N02 acceptance: 8/8 checks passed, no failures or blocked checks.
Checks cover archive hashes/notices, initialization, cold native lexical search, Japanese BM25, canonical MCP, legacy lexical MCP, real DEFLATE DOCX, and missing/restored parser controls.
The parser refused a new source while absent and succeeded on the same source after restoration; all 11 recorded process receipts were collected.
- N06 public CLI regression: actual OLD failure and NEW success for imported tags, whole-tag/case/AND filtering, actual inventory UUID pointers, and unchanged originals.
A second test proves version-1 Query refuses before refresh without changing any library bytes or directories.
- Affected root package: 177 cases accounted with zero ignored.
The full run had one legacy MCP preview failure; its narrow repair passed the unchanged bounded-text assertion and all seven focused checks.
Workspace all-target strict checks and formatting passed.
The earlier N02a decoded-bound regression remains green; extraction limits and refresh ceilings are unchanged.

Use the [README getting-started instructions](README.md) for installation, optional model prerequisites, and Mac build/acceptance steps.
The documentation update adds no executable changes to the qualified artifact.

## Evidence

Relative to `evidence/omni/`, the exact receipts are:

- `lead-artifacts/m4-vectors/lead2-n06-committed-source.json`: committed production/fixture file identities and exact fixture-only path list.
- `lead-artifacts/m4-vectors/lead2-n06-test-accounting.json`: 177-case inventory and corrected focused outcomes.
- `lead-artifacts/m4-vectors/lead2-n06-old-regressions-1/results.json` and `lead2-n06-final-focused-1/results.json` in the same directory: public OLD/NEW regression receipts.
- `lead-artifacts/m4-vectors/lead2-n06-committed-artifact.json`: actual built/staged/independently verified artifact.
- `lead-artifacts/m4-vectors/lead2-n06-committed-installed-evidence-1/result.json`: installed acceptance.

The driver is `tests/product_acceptance/linux_installed.py`, SHA-256 `6706ef6e0f3dd420d68d7d41503e815100650a60f16b9753a69e29724b698cfa`.
Its sole fixture migration changes `wiki/installed.md` to `pages/installed.md`; content, DEFLATE fixture, oracles, budgets, and parser controls are unchanged.
It used the actual installed artifact with fresh HOME/cache/cwd and restricted PATH.
Original N02/N05 failing receipts remain preserved; the import-tag failure under `lead2-n05-doc-probe-1` is now fixed, not erased.

## Open Items

- Libraries created before `5e65e3a` need the legacy migration, not yet shipped; version-1 inventory/preview reads remain available.
- The retained metadata editor and generic source-details adapter are not ported to canonical version-2 annotations; visible source pointers own those annotations.
- WeMM is optional/off by default, with no qualified real weights or native CLI enablement binding; N03 owns that selection and quality acceptance.
Provider settings and weights stay outside the portable manifest.
- Apple Silicon macOS, native FSEvents, Claude/Codex client acceptance, and optional ASR remain unqualified; N04 is the engineer's acceptance.
- GUI M7, Linux ARM64, physical resource guarantees, all-command implementation, notice-gap closure, notarization, and public-release approval are outside this result.
