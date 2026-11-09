# First Usable Version

Status: qualified Linux x86_64 headless first usable version, updated for N07 on 2026-11-09.
Mac and real-model acceptance follow this version; N03 and N04 remain engineer items.
This result does not claim full product-contract completion or a public release.

## Qualified Artifact

- Code/build commit: `c330983245849cf6a52a1ba1e9546976204b4c13`.
- Archive: `omni-knowledge-linux-x86_64-n07-c330983.tar.gz`.
- SHA-256: `c179f63dc1ebfc86822291a3439bc69fb780ec6b11e08240b3ffd13c4357e217`.
- Installed N02 acceptance: 8/8 checks passed, no failures or blocked checks.
Checks cover archive hashes/notices, initialization, cold native lexical search, Japanese BM25, canonical MCP, legacy lexical MCP, real DEFLATE DOCX, and missing/restored parser controls.
The parser refused a new source while absent and succeeded on the same source after restoration; all 11 recorded process receipts were collected.
- N07 public stdio regression: read metadata equals Query's canonical Source, including tags, title, description, digest, UUID and annotation path.
The same process observes pointer edits, ignores conflicting hidden sidecars, preserves bytes, and selects the exact requested path despite 35 earlier metadata matches.
Extraction uses the confined native descriptor and existing paid collection owners through delivery; optional unobserved Source facts remain null.
- Affected public tests: 10 passed, zero ignored, followed by five focused passes after the final allocation-lifetime correction.
These include bounded UTF-8 preview, import tags, version-1 refusal without writes, canonical/legacy MCP mapping and delivery/error cleanup.
Workspace all-target strict checks and formatting passed.
- README examples run against this archive, including imported tagged DOCX search and canonical annotations in a real DOCX MCP read.
Decoded limits and refresh ceilings are unchanged.

Use the [README getting-started instructions](README.md) for installation, optional model prerequisites, and Mac build/acceptance steps.
The qualification documentation update adds no executable changes to this artifact.

## Evidence

Relative to `evidence/omni/`, the exact receipts are:

- `lead-artifacts/m4-vectors/lead2-n07-committed-source.json`: exact reviewed and gated file identities.
- `lead-artifacts/m4-vectors/lead2-n07-affected-final-1/results.json` and `lead2-n07-final-lifetime-1/results.json` in the same directory: affected public tests.
- `lead-artifacts/m4-vectors/lead2-n07-review-closed.md`: ownership, predicate, delivery ordering and allocation-lifetime review.
- `lead-artifacts/m4-vectors/lead2-n07-committed-artifact.json`: actual built/staged/independently verified artifact.
- `lead-artifacts/m4-vectors/lead2-n07-committed-installed-evidence-1/result.json`: installed acceptance.
- `lead-artifacts/m4-vectors/lead2-n07-doc-probe-evidence-1/result.json`: runnable Linux examples and opaque DOCX MCP read.

The installed driver is unchanged from N06: `tests/product_acceptance/linux_installed.py`, SHA-256 `6706ef6e0f3dd420d68d7d41503e815100650a60f16b9753a69e29724b698cfa`.
N06's fixture-only `wiki/installed.md` to `pages/installed.md` change preserved content, oracles, budgets and parser controls.
Acceptance used the actual installed artifact with fresh HOME/cache/cwd and restricted PATH.
Original N02/N05/N06/N07 failure and compiler-diagnostic receipts remain preserved.
One expanded N07 run had an intermittent changed-scope refusal; its exact schedule was not established.
Read framing now checked-flushes source data before the newline acknowledgement, then flushes only that delimiter while retaining all owners; independent review approved this ordering and final checks passed.

## Open Items

- Libraries created before 7852835 are pre-release format: initialize a new empty library and re-import originals.
Legacy migration is dropped; the old library stays untouched and readable for inventory/preview.
- The retained metadata editor and generic source-details adapter are not ported to canonical version-2 annotations; visible source pointers own those annotations.
- WeMM is optional/off by default, with no qualified real weights or native CLI enablement binding; N03 owns selection and quality acceptance.
Provider settings and weights stay outside the portable manifest.
- Apple Silicon macOS, native FSEvents, Claude/Codex client acceptance, and optional ASR remain unqualified; N04 is the engineer's acceptance.
- GUI M7, Linux ARM64, physical resource guarantees, all-command implementation, notice-gap closure, notarization, and public-release approval are outside this result.
