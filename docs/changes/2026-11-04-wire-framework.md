# Public Wire Framework Slice

Date: 2026-11-04.
Status: Local implementation slice for integration and independent acceptance; not a published service or M0 closure report.
Source: engineer-authorized first `omni-service` behavior against the committed [service contract](../specs/service.md), based on lead commit `5ad6caeba915783deff9f3b74d3975ac8c332c1e`.
The earlier M0 document-writing lease is complete and is not extended by this change.

## Outcome And Boundary

Add one real workspace member, `omni-service`, with public version-1 DTOs and finite JSON request decoding/response encoding.
Mirror the actual owner union, including `verify_source`, EPUB publication, foreign-v1 plans, and the existing `job_status` data used by bounded Web status delivery.
Do not implement a second command catalog, registry, IDL generator, plugin layer, fake backend, authority object, or operation handler.
No command executes during decoding or encoding.

The public functions are `decode_request(&[u8]) -> std::result::Result<Request, CodecError>` and `encode_response(&Response) -> std::result::Result<Vec<u8>, CodecError>`.
The [crate guide](../../crates/omni-service/README.md) describes signatures, scalar construction, Serde limits, and focused commands.
RustDoc documents the public types and fields so independent authors need not derive expectations from implementation internals.
`Result` is the owner's tagged wire union; normal Rust function results retain `std::result::Result`.
Large result boxes and the recursive operation-outcome box do not change wire fields or nullability.

## Wire Rules Implemented

- Check the supplied 1 MiB control-payload bound before UTF-8 and JSON parsing; reject invalid UTF-8, incomplete/multiple JSON texts, unsupported decoded versions, unknown fields/tags, and missing required fields.
- Retain required nullable keys instead of Serde's implicit absent-Option default; null remains explicit on output.
- Retain opaque nonempty IDs without UUID requirements, path inference, or normalization.
- Check lowercase SHA-256 digests, canonical decimal-string `U64`, canonical signed stat timestamps, safe numeric integers, UTC RFC 3339 timestamps, and finite scores.
- Distinguish nonempty normalized relative paths from the browse-only empty-root sentinel; perform no filesystem resolution or admission.
- Check explicit browse/history, range/diff, representation-line, query, schedule, and EPUB-selection bounds without substituting defaults.
- Bound output growth during serialization, return no partial frame, and refuse non-finite numeric serialization rather than silently replace it with null.
- Check bounded response snippets/diffs, format/selection alternatives, and original non-nested operation outcomes.
- Return fixed typed codec/scalar errors without rejected input, raw parser diagnostics, or logging.

The codec deals with one supplied JSON payload, not a stream or HTTP/MCP framing.
It emits no delimiter; a delimiter supplied on input counts toward the byte limit.
Public Serde DTO traits alone do not enforce envelope-size, supported-version, or command-bound checks.
The codec is the admitting wire boundary, not a domain/exposure authority.

## Excluded Behavior And Existing Consumers

No filesystem library access, Git, provider, cache, daemon, transport loop, HTTP/MCP server, shared Web component, or GUI dependency is added.
GUI technology remains undecided and lower priority, not cancelled.
Existing CLI, GPUI, tests, product owners, and workflows remain unchanged; they do not call the new crate yet.
No empty sibling crate is added, and the retained root and vendored kernel remain workspace members.
The bounded Web status route remains an accepted transport contract, not runtime work in this slice.

A parsed `Target`, `Exposure`, `MountCapabilities`, ID, imported declaration, or owner-only command is not a capability.
Execution must still derive authenticated caller context and recheck selected roots, availability, residency, versions, exclusions, and current exposure.
Source digests, coverage truth, Base64 bytes/provenance, continuations, archive integrity, signing, idempotency, remote effects, and confirmation tuples still need their owners' real implementations and independent proof.
Do not infer those behaviors from codec success or a compiler pass.
No acceptance threshold, test oracle, required feature, or retained test is changed.

## Dependency And Mechanism Necessity

| Addition | Present necessity | Reasoned ablation |
| --- | --- | --- |
| One declared service crate with actual codec behavior | Provides the already accepted shared product boundary without linking retained GUI/backend behavior. | Removing it leaves no concrete version-1 public producer/consumer surface for separately authored conformance checks. |
| Explicit DTO structs/enums | Preserve operation/field/tag/nullability distinctions from the owner's declarative union. | Replacing them with generic JSON loses exhaustive public types and permits unknown/omitted authority-sensitive fields to escape shape admission. |
| Small checked wire scalars | Prevent JavaScript integer loss, malformed path/digest values, and non-finite scores at the wire boundary. | Plain unvalidated numbers/strings permit lost offsets and NaN-to-null conversion; they also conflate root browsing with write paths. |
| Shared bounded codec and private output writer | Checks size before parsing and prevents unbounded output/partial success. | An after-the-fact size check has already allocated oversized JSON; direct generic Serde use alone misses version and command-bound checks. |
| Existing `serde`/`serde_json` | Mature typed JSON encoding and decoding with strict field handling. | A handwritten JSON parser duplicates maintained syntax/escaping machinery without a demonstrated need. |
| Existing locked `time` with parsing | Validates real UTC RFC 3339 calendar/offset syntax without another handwritten parser. | A shape-only timestamp check admits invalid dates; recreating a calendar parser adds unnecessary interpretation rules. |

No experimental ablation is claimed or needed for these direct, currently required responsibilities.
There was no previous version-1 codec in the retained application source; the existing unversioned interfaces remain unmodified until an independently gated integration.
The lock adds only the actual `omni-service` package using existing locked `serde`, `serde_json`, and `time` versions; it adds no external package or GUI dependency.
No parallel schema catalog, generator, command dispatch map, or reflective validator is maintained.
The plain scalar, DTO, and codec modules separate representation from byte admission without an extra service layer.

## Evidence And Handoff

An initial offline `cargo check -p omni-service` passed after supplying the verified retained IPADIC archive; this invocation also let Cargo add the actual member to its lock normally.
Final local-commit gates cover repository formatting, focused locked/offline compilation, strict all-target Clippy, test and doctest accounting, and a small real external producer/public-codec/consumer smoke using only synthetic frames.
The implementation handoff retains exact final code/lock/source identities, commands, actual smoke bytes, results, and omissions for the integrator.
There are no builder-authored acceptance tests or edits to existing tests; the RustDoc/example demonstration is not independent acceptance.
Independent authors must derive lasting DTO/malformed-frame/numeric/path/nullability/error checks from the owner and public Rust surface.
The integrator owns final workspace/GUI/transport gates, independent review, M0 closure, and publication.
No push or backend/authority cutover is authorized by this slice.
