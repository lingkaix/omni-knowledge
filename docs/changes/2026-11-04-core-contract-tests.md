# Knowledge-Core Contract Tests

This record owns the independent integration tests for the preparatory knowledge-core codec and checked data values.
It does not accept the codec, amend storage, or prove a mount adapter.
The tests are `crates/knowledge-core/tests/manifest_codec.rs` and `crates/knowledge-core/tests/checked_values.rs`.
Production lines in this change: 0. Test lines: 1153. The lease forbids production edits, so the test-to-production ratio is not a reason to add implementation.

## Authority And Independence

Expected observations come from the accepted [storage](../specs/storage.md) and [product](../specs/product.md) owners, the [service](../specs/service.md) data names for mount and exposure fields, the [crate guide](../../crates/knowledge-core/README.md), and the rendered public API.
The candidate bodies in `src/manifest.rs` and `src/data.rs`, builder smoke scripts, smoke results, and oracle tables were not inputs.
A failing check stays a failing check. This record does not retune an expectation to obtain a pass.
Only the public codec's real behavior is acceptance evidence. The `MountBackend` test is labelled API-only.

## What The Suite Decides

The editable fixture uses version 2, one `pages` or mapped `wiki` root, nonempty non-UUID ids, unique single-component mount names, and convention string `"0.2"`.
Mount and exposure tags use the service spellings `local`, `nas`, `cloud_folder`, `git`, `startup_periodic`, `nightly_manual`, `manual`, `off`, `read_only`, `read_write`, `owner_ipc`, `owner_stdio`, and `http`.
An unchanged name must copy the original bytes, including whitespace, escaped keys, and escaped name spellings.
A changed name may normalize only the outer object. Every other raw field value must survive, including nested duplicate keys and numeric spellings such as `9007199254740993`, `1.2300`, `1e+2`, `-0`, and `18446744073709551616`.
The comparison parses those bytes with a test-local scanner. It does not treat `field_json` as the only oracle.
Legacy version 1, unsigned versions other than 1 or 2, and unrecognized required tags stay inspectable and refuse even a no-op rename.
Missing, duplicated, inconsistent, or wrongly typed recognized fields are read-only. Syntax, outer duplicate keys, non-objects, invalid UTF-8, and input past the caller ceiling return the documented errors.
The output-ceiling case keeps each raw name within the selected ceiling. One name's JSON escaping alone exceeds that ceiling. A second encoded name fits while the complete mutated manifest exceeds it. A changed name whose mathematical compact candidate fits succeeds, and the candidate is non-empty. Refusal returns no candidate, leaves the original bytes unchanged, and stays input-free. Those lengths are the public JSON spelling of the fixture, not codec output.
Container depth counts the outer object as level 1. Depth 128 stays editable. Depth 129 and depth 200 return `DepthLimit`.
`RelativePath`, `ContentDigest`, and `ByteRange` enforce the documented lexical rejects. A zero `Budget` remains a stored zero. Nothing in that check calls an adapter.

## Coverage Matrix

| Owner or public clause | Test |
| --- | --- |
| Manifest filename, version 2, OKF spelling `"0.2"`, depth ceiling 128 | `public_manifest_constants_match_storage_identity` |
| Supported version-2 projection, opaque location, extension retention | `supported_version_two_projects_recognized_fields` |
| Wiki root and the four ordinary backend tags | `wiki_root_and_ordinary_backend_tags_are_editable` |
| Unchanged name preserves original bytes | `unchanged_name_preserves_original_bytes` |
| Changed name preserves raw values and numeric spellings | `changed_name_preserves_unrelated_raw_values_and_numeric_spellings` |
| Display name is not trimmed; escapes belong to the new name | `changed_name_keeps_spacing_and_json_escapes_inside_the_name` |
| Legacy version 1 is inspectable and refuses mutation | `legacy_version_one_is_inspectable_and_refuses_mutation` |
| Other unsigned versions are read-only | `future_and_other_unsigned_versions_are_read_only` |
| Non-unsigned version shapes are invalid known fields | `non_unsigned_version_shapes_are_invalid_known_fields` |
| Other convention representations stay verbatim and read-only | `other_convention_representations_remain_verbatim_read_only` |
| Unrecognized required semantics are read-only | `unrecognized_required_semantics_are_read_only` |
| Malformed recognized fields are read-only | `malformed_recognized_fields_are_read_only` |
| Invalid JSON and duplicate outer keys fail closed | `invalid_json_and_duplicate_outer_keys_fail_closed` |
| Invalid UTF-8 does not echo input | `invalid_utf8_does_not_echo_the_byte_stream` |
| Caller byte ceiling, including escape expansion | `input_byte_ceiling_refuses_without_echoing_the_body`, `escaped_rename_over_the_output_ceiling_returns_no_candidate` |
| Depth ceiling versus a shallow supported document | `object_depth_at_public_ceiling_stays_editable`, `object_depth_past_public_ceiling_is_depth_limit` |
| Fixed input-free diagnostics | `repeated_codec_failures_keep_a_fixed_input_free_diagnostic`, `backend_diagnostics_are_fixed_and_do_not_carry_a_canary` |
| Relative path lexical contract | `relative_path_keeps_accepted_spellings`, `relative_path_rejects_the_documented_spellings` |
| Claimed lowercase SHA-256 spelling, not a hash | `content_digest_checks_lowercase_sha256_spelling` |
| Nonempty half-open range and end overflow | `byte_range_rejects_empty_and_overflowing_ends` |
| Zero budget is a stored finite ceiling | `budget_zero_remains_a_finite_stored_ceiling` |
| Trait object shape only | `mount_backend_trait_is_callable_api_only` |

## Gaps That Are Not Format Amendments

Storage names `convention_version` and OKF 0.2, but it does not specify the JSON encoding.
The public projection recognizes the string `"0.2"`. Numeric `0.2` and the string `"OKF 0.2"` are tested as verbatim read-only projections, not as a new rule that those encodings are corrupt.
Storage also does not enumerate the manifest's writable and reconciliation JSON members.
The tests use the service field names because the committed public projection says it reads those names. A passing editable fixture is not acceptance of a disk-format amendment, and it does not close the projection review required before persistence.
`ManifestMetadata` has no convention, extension, or service-revision field. The tests do not serialize that snapshot as a manifest.
A JSON number spelling such as `2.0` for `version` is not specified. The suite does not invent a verdict for it.
The service owner explicitly excludes empty path segments. That exclusion is decided. This suite does not add a blanket path-test expansion for it. Trailing slashes and UNC prefixes remain unenumerated here and are not given a new normative result by this record.
Empty bytes at a zero ceiling match both "invalid JSON" and "zero admits nothing", and neither text orders those reasons. Length zero does not exceed a zero ceiling. The test accepts `InvalidJson` or `ByteLimit` and requires an input-free error. One earlier execution observed `InvalidJson`. That observation is not promoted to a format rule.
Changed-name tests do not pin outer key order or diagnostic wording. They pin raw non-name values, decoded names, diagnostic stability, and canary absence.
No test opens a directory, activates a mount, migrates version 1, writes a journal, or grants exposure. The API-only stand-in is not evidence of confinement, residency, range reads, tracking, offline behavior, tombstones, or budget enforcement.

## Checks

The package build used Rust 1.99.0, Cargo 1.99.0, `--locked --offline`, two Cargo jobs, and a private target directory and `TMPDIR`.
The knowledge-core package does not compile the vendored kernel, so no dictionary cache was required.
`cargo fmt --all -- --check` passed.
`cargo clippy -p knowledge-core --all-targets --locked --offline -- -D warnings` passed.
`cargo test -p knowledge-core --all-targets --locked --offline` selected 26 integration tests, passed 26, and ignored 0.
The library target and the manifest example target each reported 0 tests. Those are empty harnesses, not a filtered acceptance run.
The example binary was not executed and is not an oracle.
Independent key review has not accepted this suite.
The 26-pass receipt above is the historical result from that run. It is retained. The later output-ceiling stimulus correction does not relax those bounds and does not replace that receipt.
