# Independent Wire Contract Tests

Date: 2026-11-04.
Status: independent codec checks against the local `omni-service` candidate. This record is not a transport proof, an exposure proof, or an M0 closure.

The service contract owns the version-1 JSON shapes.
These tests own only `crates/omni-service/tests/wire_contract/`.
They call the public decoder, encoder, and checked scalar constructors.
They do not read the codec or scalar implementation bodies, and they do not change production code.

## Fixed Expectations

[The service contract](../specs/service.md) at `921ab9c9a80438d7a9de61dd600e9e1383389b2b` freezes the 47 commands, the result union, locator forms, nullability, and the declared numeric bounds.
[The Web client record](2026-11-04-web-client-contract.md) keeps bounded `job_status` delivery as a later runtime proof.
The public crate guide names `decode_request`, `encode_response`, `CodecError`, and the scalar constructors.
Rustdoc supplies argument names and the fixed error variants.
A decoded owner-only command remains data.
An encoded response does not prove that a digest, coverage value, path, or effect is true.

## Checks

Each of the 47 command shapes is an independent JSON fixture.
The decoder admits it, Serde reserializes the DTO, and the decoder admits that encoding again.
The comparison is field equality with the fixture, including required null keys.
Keywords stay untrimmed and undeduplicated.
Representative results cover the result tags, seven locator forms, explicit nulls, and finite scores.
Null, missing, extra, and duplicate keys are separate refusals.
`U64` accepts `"0"` and `"18446744073709551615"` and rejects a JSON number, a sign, a leading zero, and overflow.
Safe integers stop at `9007199254740991` on both sides of zero.
`FiniteNumber` rejects NaN and both infinities, so a non-finite score cannot encode as null.
The empty browse path is only the selected-root sentinel.
Relative paths reject an empty path, a dot segment, traversal, a backslash, and NUL.
Browse and history limits are 1 through 1,000.
Range and diff lengths are 1 through 524,288.
Representation output is 1 through 20,000 lines.
Query limits are 1 through 10,000, and a nonempty keyword list needs a method.
An enabled schedule needs a positive interval.
An EPUB plan needs a nonempty page list.
A control payload of exactly 1 MiB can carry one JSON value.
One extra byte is `FrameTooLarge` before UTF-8 parsing.
A 600,000-byte message fits, and the same length of quote characters encodes beyond 1 MiB as `FrameTooLarge` with no partial bytes.
A 524,288-byte diff fits, and 524,289 bytes fail inside the frame.
A 500-scalar snippet fits, including U+4E2D, and 501 scalars fail.
Codec and scalar diagnostics omit a caller canary and have no source error.
Opening a missing path string does not create that path.

## Limits

The suite does not start a daemon, HTTP listener, MCP session, or user interface.
It does not prove admission, revocation, idempotency, package bytes, search truth, or history trust.
Web status snapshots remain runtime work.
The focused command is `cargo test -p omni-service --test wire_contract --locked --offline`.
That run reported 79 passed, 0 failed, and 0 ignored.
Package formatting and `cargo clippy -p omni-service --all-targets --locked --offline -- -D warnings` also passed.
No test is ignored and no expectation was replaced after a failure.
