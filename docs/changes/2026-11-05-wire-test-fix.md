# Wire Container And Allocation Regressions

Date: 2026-11-05.
Status: independent checks against local `fd5d8c8e4c7d4591e2497017dd7e81b503f212b7`. This record is not a transport proof, an exposure proof, or an M0 closure.

## Responsibility

These checks extend `crates/omni-service/tests/wire_contract/`.
The service contract owns the version-1 JSON records and string unions.
The earlier 79 checks and their expectations remain in the suite.
This change does not edit production code, add an identifier length quota, or alter the existing safe-integer checks.

## Schema

`Request`, `Command`, `SourceRef`, `Budget`, `ExpectedVersion`, `Target`, `SearchRequest`, `PackageSelection`, and each `MountMapping` are JSON objects.
Declared command string unions are JSON strings.
The request array is `[1,"wire-shape",{"op":"library_status","library_id":"synthetic"}]`.
`source_details.source` and a nested `image_query` use `["lib","src"]`.
`library_create.history_mode` uses `{"signed":null}`.
Each other request string union has one externally tagged object in place of its string.
The 47 command goldens remain the valid object and string controls.

On this source, `decode_request` accepted 17 of those malformed values.
The accepted families are `Request`, `Command`, `SourceRef`, `MountMapping`, and every checked string union.
`Budget`, `ExpectedVersion`, `Target`, `SearchRequest`, and `PackageSelection` arrays were refused.

## Allocation

An ordinary empty `Libraries` response carries one already constructed public `Id`.
The counter starts after that fixture exists and counts only the encoding thread.
A short id and a 256 KiB id encode inside the 1 MiB frame, and the counter sees that allocation.
The 16 MiB value is a public `Id`.
The encoder sample for that value recorded peak 16777388 and total 16777388 against an id of 16777216 bytes, then returned `FrameTooLarge`.
The allocation limit is 4194304 bytes, four times the frame and below half of the caller id.
The returned error variant is not the deciding check.

## Result

`cargo test -p omni-service --test wire_contract --locked --offline` reported 86 passed, 18 failed, and 0 ignored.
The 18 failures are the 17 accepted containers and the oversized-id allocation.
Package formatting and `cargo clippy -p omni-service --all-targets --locked --offline -- -D warnings` passed.
No earlier test was removed or ignored.
