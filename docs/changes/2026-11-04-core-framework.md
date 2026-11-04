# Preparatory Knowledge-Core Framework

## Source, Authority, And Scope

The engineer assigned this disjoint preparatory M1 slice from published `923fa107aae5b91d970365d7b8be6e3ed29d7847`.
M0 native observations remain in progress; this record neither accepts M0 nor completes M1.
The accepted [storage](../specs/storage.md), [product](../specs/product.md), and relevant [service](../specs/service.md) data owners govern behavior.
The preserved unaccepted wire branch is not an input dependency.

The exclusive loan covers `crates/knowledge-core/**`, root Cargo.toml/Cargo.lock, and this record.
There is no application/authority cutover, provider/backend activation, filesystem implementation, initialization, migration, agent delegation, or push.
Root manifests/lock are returned after the local committed handoff.
Independent tests and key review follow separately.

## Intended Result And Boundaries

The crate implements a real bounded version-aware manifest codec and name-only pure candidate editor.
It retains exact original input for read-only inspection and raw field values for extension-preserving name mutation.
Unknown required versions/recognized semantic tags refuse mutation; legacy version 1 is never migrated or activated.
The storage marker remains omni-knowledge.json version 2 with one pages/wiki authored tree; no alternative authority/layout is invented.

One neutral MountBackend trait exposes observed capabilities, bounded enumeration, stat, and expected-version range reads.
Typed observations distinguish unavailable, partial, changed, and deleted states; backend-qualified file/version evidence cannot be replaced by a naked hash or cursor.
No adapter, registry, success stub, permission constructor, watch/write engine, callback, network access, or library discovery is supplied.
The [crate guide](../../crates/knowledge-core/README.md) documents API locations, bounds, preservation semantics, adapter obligations, and necessity/reasoned ablation.

## Explicit Interpretation Limits

The storage owner names convention_version and OKF 0.2 without specifying the field's JSON encoding.
The initial codec recognizes string `"0.2"`; other representations remain verbatim read-only.
Manifest writable/reconciliation fields project the current service data names without importing runtime availability/capabilities or local approvals.
These projections require independent owner/public-API scrutiny before persistent integration.
This record is not authority for a new format rule, critical-field protocol, or narrowing of the accepted owner.

Caller-selected byte ceilings avoid inventing an on-disk size quota.
A documented 128-level parser safety ceiling protects bounded parsing; it does not classify deeper files as corrupt.
Bounded encoding returns no partial candidate; unchanged names preserve original bytes, while changed names normalize only the outer object and name encoding.
Every other raw value, including nested optional authored fields and large numeric spellings, survives unchanged.
Metadata snapshots are not complete writable manifests.

## Dependencies And Structural Accounting

The only direct dependencies are already-locked shared serde and serde_json, with serde derive and serde_json raw_value support.
The raw-value feature preserves arbitrary authored numeric/object data without a floating-point AST or new parser.
No external version upgrade, service dependency, platform library, model, or registry is intended.
The existing filesystem-coupled version-1 Config remains unchanged; the new pure codec has no current application callers and does not dual-write it.

New concepts are one manifest document/editor with explicit states/limits, one backend trait with its input/output facts, and checked relative-path/digest/range values used by that seam.
Existing provider/wire catalogs are not copied.
The crate guide gives each mechanism's present necessity and the behavior lost without it.
No experimental ablation is claimed or required for these understood additive responsibilities.

## Planned Evidence And Handoff Limits

Builder checks select the new package only, with jobs 2 and dedicated owned target/TMPDIR/dictionary state.
Expected gates are focused check, strict all-target Clippy, repository formatting, public example build/smoke, documentation build, and whitespace/diff/lock inspection at the final clean committed candidate after fetch/rebase.
Independent acceptance cases: zero at builder handoff.
Builder smoke can demonstrate public codec execution and refusal, but cannot establish independent conformance or native/backend effects.
Exact final commands, exits, commit/package/lock identities, remaining interpretation questions, and residual state belong to the assigned external core-framework log.

Ordinary local, existing-OS-mount NAS, and tested cloud-folder adapters are separate proposed leases.
Initialization/migration, durable writes/journal/tracking/cache/history/packages, processing/retrieval/service integration, transport approvals, and application cutover remain unimplemented.
No full current-source workspace/native/GUI gate or M0/M1 completion is inferred from focused gates.
