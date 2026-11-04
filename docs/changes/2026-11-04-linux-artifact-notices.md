# Linux Artifact Notice Contents

Date: 2026-11-04.
Status: independent CI review correction; final committed-head gate/review remains separate.
Owner: [product licensing](../specs/product.md#project-license), [dependency guidance](../dependencies.md), and the build scripts.

## Finding And Smallest Correction

The first Linux archive recipe copied only the CLI and the project's Apache license.
It did not carry notices for bundled third-party Rust components; successful executable smoke cannot prove notice presence.
Reuse the existing collector with explicit target/headless selection instead of copying Darwin GUI output or adding another licensing framework.
Keep the default Darwin GUI invocation unchanged for the existing macOS bundle.
Bind each generated inventory to target, selected features, root package, and the actual workspace lock SHA-256.
Stage its notices and source archives in the Linux tar, then inspect the unpacked inventory and CLI before upload.

When the registry omits notice files, reuse existing retained upstream evidence only for matching crate/version/VCS identities and matching notice hashes.
There is no new registry or network lookup during collection.
For remaining registry notice gaps, preserve the exact lock-checksummed original crate source using the collector's existing bounded archive validation.
This retains available original evidence; it does not fabricate copyright text or certify a missing upstream notice.
MPL-only source obligations still include their verified original archives and remain separately identified.

The target parameter is necessary because the Darwin GUI dependency closure is not Linux headless evidence.
Removing the collection step reproduces the independently observed missing-notice artifact.
Removing lock/target checks permits the wrong graph to pass; removing exact retained evidence leaves avoidable gaps despite already owned notice material.
The same unpacked-artifact check passes real included notices and refuses a deliberately omitted Tantivy notice entry.
An existing output directory is refused without overwriting its inventory.
These are bounded artifact observations, not a universal legal oracle.

## Actual Local Composition And Limits

The real locked Linux metadata producer and collector emitted an inventory of 298 conservative normal/build dependencies.
The artifact contains discovered/retained notice text, two MPL-only source archives, and four original archives for remaining notice gaps.
The real release CLI, project license, inventory, full notice document, and source archives were tarred, unpacked, and inspected together.
Unpacked help and model-free JSON doctor smoke passed; target, empty GUI feature set, exact lock digest, Tantivy notice presence, and all six archive hashes passed.
The deliberately removed-notice control failed the same deciding assertion; the original artifact was unchanged.

Four upstream dedicated-notice gaps remain explicit: adobe-cmap-parser 0.4.1, htmlescape 0.3.1, pdf-extract 0.12.1, and type1-encoding-parser 0.1.1.
Their original source archives and declared license metadata are included; no copyright or permission text is invented.
Exact-revision root LICENSE probes for the three affected parser repositories returned 404 and are not claimed as supplied notices.
This does not certify full public-distribution compliance; unresolved license evidence remains a delivery review obligation rather than a green metadata count.
Native Mac bundling, final remote artifact publication, and later workspace-local shared-crate notice coverage remain separate observations.
