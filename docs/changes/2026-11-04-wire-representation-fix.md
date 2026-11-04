# Wire Representation And Borrowed Scalar Corrections

Status: scoped correction from `921ab9c9a80438d7a9de61dd600e9e1383389b2b`; independent OLD/NEW acceptance remains external.
The engineer assigned WIRE-S2-01 and WIRE-S2-02 from the independent wire review.
The [service contract](../specs/service.md#encoding-and-compatibility) owns JSON object records, string unions, scalar checks and the 1 MiB control-frame ceiling.
The [crate guide](../../crates/omni-service/README.md) describes the existing bounded codec, not runtime authorization.
No owner, public API, operation/tag/field schema, numeric limit, dependency, manifest/lock, test or gate changes.

## Checked Request Representation

Serde can decode positional sequences into records and externally tagged maps into unit enums.
The public request decoder now checks JSON kinds against the existing DTO serialization after typed decoding.
The recursive comparison includes every nested object, array element and primitive kind, without another operation/field catalog.
Typed parsing still runs first to preserve duplicate-key rejection and all existing required-null, extra-field, scalar and tag checks.
The comparison ignores primitive values and number spellings; it does not add a canonical JSON-number-token rule.
A representation mismatch returns the existing fixed InvalidWireData error before version and explicit command-bound checks.
Direct general Serde decoding remains insufficient for checked wire admission, as already documented.

The two temporary JSON trees contain only data from a request already admitted by the existing byte ceiling and typed parser.
This adds input-bounded parsing/serialization work, not a new quota, response tree, operation registry, schema generator or authoritative schema document.
The DTO declarations remain the single implementation representation; the accepted owner and independent literal conformance cases remain the authorities for correctness.
This comparison cannot detect a DTO declaration that itself disagrees with its owner.
An input-only deserializer adapter can lose its restrictions when internally tagged enums buffer and replay content.
Per-record custom deserializers or another manually maintained command/field list would be broader and risk duplicated interpretation.
Removing this shared check restores the demonstrated alternate-container language, including nesting; patching only the envelope or history field leaves the same family defect elsewhere.

## Borrowed String Scalars

Six checked String-backed newtypes retain derived serialization but no longer request owned String conversion for serialization.
Normal newtype serialization borrows the inner String; checked try_from String deserialization and all public owned conversions remain unchanged.
This includes Id, Digest, SignedDecimal, Time, RelativePath and RootRelativePath.
It removes the old derive-generated clone before the bounded writer sees a variable-length scalar.
No ID/path/time quota or allocator subsystem is introduced.
The response validator, serializer call, capped writer and no-partial-success behavior are unchanged.
Restoring the owned conversion recreates the avoidable full-scalar copy; changing only the writer cannot prevent a copy made before its first call.
Handwritten serializers for all six wrappers are unnecessary because existing newtype serialization supplies the borrowed representation.

## Evidence And Limits

The assigned external wire-fix log records source/artifact identities, exact focused commands, public-codec observations and process settlement.
The original 79 independent cases and frozen supplemental OLD failures remain separate evidence; the producer changes none of their code or oracles.
Builder smoke uses real public decoder/encoder calls and modest synthetic values, not an independent allocation detector or acceptance suite.
The independently frozen oversized-ID observation excludes caller fixture construction and is not replaced by observing FrameTooLarge alone.
A 4 MiB allocation-observer ceiling reported by the independent author is not a new product quota; the wire frame remains 1 MiB.
The service-owner question about integral values written as `1e2` remains pending; this correction leaves existing numeric decoding unchanged.
No whole-process RSS, allocator/platform qualification, actual service/transport, domain truth, application integration, native, M0 or M1 acceptance follows.
