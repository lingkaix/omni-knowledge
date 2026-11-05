# Documentation Map

This map locates engineering owners and project guidance. It is a navigation aid, not a second source of rules.

## Engineering Owners

| Question | Read |
| --- | --- |
| What rules apply to every task? | [Agent execution](../AGENTS.md) |
| Which design principles guide a change? | [Engineering principles](engineering-principles.md) |
| How do we coordinate and recover work? | [Change execution](change-execution.md) |
| What may a delegated role do? | [Agent roles](roles/README.md) |
| Which document governs a decision? | [Documentation governance](documentation.md) |
| Which tests protect public behavior, and who may retire them? | [Test strategy](specs/test-strategy.md) |
| What evidence is sufficient? | [Verification](verification.md) |
| Why does execution use an adaptive method? | [Execution rationale](change-execution-rationale.md) |
| How do we write technical text? | [Technical writing](writing.md) |
| How do we contribute and commit? | [Contributing](../CONTRIBUTING.md) |
| How do we set up and run checks? | [Toolchain](toolchain.md) |
| Is the headless delivery ready to use? | [First usable version](../first-usable-version.md) |

Read only the owners needed for the task. Add a product contract or local guide when the project has that responsibility.

## Product And Implementation

| Question | Read |
| --- | --- |
| What did the engineer request? | [Product requirements](specs/product.md) |
| Which files own durable knowledge? | [Storage and recovery](specs/storage.md) |
| What do search modes and scores mean? | [Retrieval semantics](specs/search.md) |
| How do text commits, signing, proposals, and remotes work? | [Text history](specs/history.md) |
| How are representations and model inputs bounded? | [Processing](specs/processing.md) |
| What do frozen packages, backups, and restore preserve? | [Packages](specs/package.md) |
| Which wire DTOs and exposure rules do all faces share? | [Service contract](specs/service.md) |
| What full-version requirements changed, and how are they verified? | [Full-version record](changes/2026-11-04-full-version.md) |
| How is the pinned kernel built and audited? | [Downstream proof](../tools/kernel-probe/README.md), [vendor provenance](../vendor/retrieval-kernel/VENDOR.md) |
| Why these design choices? | [Product decisions](decisions/README.md) |
| How do I use the desktop app? | [GUI guide](gui.md) |
| How do agents use the tools? | [Agent operations](agents.md) |
| How does local multimodal processing run? | [Multimodal guide](multimodal.md) |
| Which dependencies were chosen? | [Dependency decisions](dependencies.md) |
| What was directly verified? | [First-version record](changes/2026-10-14-first-version.md), [desktop workspace](changes/2026-10-15-desktop-workspace.md), [status and menu correction](changes/2026-10-15-status-and-menus.md), [search and file details](changes/2026-10-15-workspace-usability.md), [search performance](changes/2026-10-15-search-performance.md), [ranked results and text locations](changes/2026-10-15-ranked-results.md), [unified search](changes/2026-10-15-unified-search.md) |
