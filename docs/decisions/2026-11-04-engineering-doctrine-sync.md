# Engineering Doctrine Synchronization

Date: 2026-11-04. Decision source: the engineer's explicit request to synchronize Omni Knowledge with OpenKit and the supplied synchronization brief. This record preserves provenance and reasons; operative rules remain in the linked owners.

Historical adoption record: the [accepted review corrections](2026-11-04-doctrine-review-rulings.md) subsequently narrow the ablation and OLD/NEW scopes, restore source qualifiers, and separate accounting provenance. Its original review status below describes the initial execution, not the later supplied review.

## Decision

Adopt applicable OpenKit engineering doctrine and testing changes since the 2026-10-14 baseline. The source snapshot is OpenKit `b67cd0f2c07fba11f189a8850b9f5e5cf0a703f1`, inspected through `origin/main` without modifying the checkout. The starting Omni Knowledge snapshot is `fa7a3c9d974c8a8e622e6e9ad88105c6aa6f781b`.

Port the October 22 test-authority decision: public-interface units, real owned stores, owner-based fixtures, failure triage, and independent retirement. Port the October 31 rebuild-corpus and falsifiable-delegation rulings where they apply to repository engineering. Keep existing owner filenames where the mapping is clean. Split role contracts into `docs/roles/` and separate test strategy from evidence quality because those responsibilities now need independent discovery and maintenance. Retain execution rationale separately from operative rules.

Also adopt genuine worker rules supplied by the brief that are absent from, or more explicit than, the inspected OpenKit owners. These include old/new replacement proof, shrink-only structural ratchets, two-sided concept inventory, targeted landings with batch accounting, fix-forward quality assurance, measurement controls, and final-head push gates. The external synchronization report distinguishes those additions from source-document parity; this record does not claim they were already OpenKit rules.

## Reason

Contributors in both repositories need the same authority boundaries, public-interface testing method, and evidence discipline. A coupled suite can make correct refactors expensive and tempt builders to change product behavior merely to satisfy tests. Independent disposition and replacement proof preserve required behavior without preserving internal mechanisms.

Shared doctrine does not require copying another product's concepts or implementation toolchain. Cargo and native macOS observations provide the local proof boundaries; JavaScript runners and server or browser architecture do not. Explicit provenance prevents supervisor-supplied additions from being mistaken for already documented upstream authority.

## Rejected Alternatives

- Preserve one short role table: rejected because test authors, builders, reviewers, and writers now have distinct operating obligations.
- Mix test lifecycle and oracle admission in one owner: rejected because test selection and evidence validity answer different questions.
- Rename every existing guide: rejected because existing names map cleanly and renaming alone adds migration cost.
- Copy OpenKit's product models, compatibility policy, coordinator mechanics, harness bindings, CI budgets, or host machinery: rejected as product-specific.
- Add generators, validators, frontmatter schemas, or permanent runners for this documentation sync: rejected without a local need.
- Claim independent approval from self-review: rejected; the current task prohibits starting agents, so independent comparison remains explicitly outstanding.

## Revisit When

A shared migration needs a different ownership split, an independent comparison finds a lost criterion, or a supplied rule proves unsuitable for local work. Repeated independent calibration that cannot meet required correctness at acceptable engineering cost also challenges delegation itself.

## Affected Owners

- [Root execution contract](../../AGENTS.md) and [contribution procedure](../../CONTRIBUTING.md).
- [Engineering principles](../engineering-principles.md) and [change execution](../change-execution.md).
- [Role contracts](../roles/README.md), [test strategy](../specs/test-strategy.md), and [verification](../verification.md).
- [Documentation governance](../documentation.md), [writing](../writing.md), and [toolchain](../toolchain.md).
