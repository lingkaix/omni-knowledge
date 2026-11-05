# Admitted package framework and public controls

Integrate the native package parser/capture/staging/recovery framework at `e031847c36c79453d125e9b0a899cb9c2dee8045` and its ABI-ported independent public tests at `5c1d6145552a76c8d5808de65d55356fc825aec5`. Independent parser/resource reviews and the affected examined-file-budget closure are retained separately; this is not full package or service acceptance.

The package uses the existing Core process account, pre-allocation and generated-extent reservations, actual examined-object quotas, streaming ZIP output, checked ZIP framing, mature JCS canonicalization, descriptor-relative traversal, bounded recovery, and same-handle `VerifiedSource` owner delegation. Removing that admission or returning a staged pathname would lose the resource/lifetime guarantees. Native storage/Git authority remains with its owner; no transport activation is added here.

Keep the original 21 package controls and all 22 public test oracles, now using real `Work` holdings, owned files and verified copying. Also import the exact 14 independent Core admission controls from `24e4437`; those establish logical accounting/claim semantics, not universal physical-RSS enforcement.

Workspace membership is the union of both branches. The root lock adds only `knowledge-package`, `serde_json_canonicalizer 0.4.1` and `ryu-js 1.0.3`, with no removed/upgraded prior package identities. The nested cache-consumer lock must include Core's already-required `sysinfo` dependency and its target/macro closure: without that edge a clean `--locked` consumer cannot build. This changes setup resolution, not the existing cache oracles or memory ceilings.

Signer/trust decisions, searchable STORED/kernel composition, genuine v1 mapping/export context, inert bundle validation, native M1 installation and EPUB binding remain required. Native macOS and final CLI/MCP/HTTP activation are not inferred from Linux crate tests. Existing native-fixture portability and fixed M2 proposal publication are preserved.
