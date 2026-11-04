//! Independent manifest-codec checks for the public knowledge-core surface.
//!
//! Expected observations come from the accepted storage owner, the service data
//! names for mount and exposure fields, the product layout, the crate guide, and
//! the rendered public API. The candidate implementation is not an oracle.
//!
//! Container depth counts the outermost object as level 1. Each nested object or
//! array adds one level. `MAX_JSON_DEPTH` is that ceiling, not a format defect rule.

use knowledge_core::data::BackendKind;
use knowledge_core::manifest::{
    CONVENTION_VERSION, ExposureMode, MANIFEST_FILE_NAME, MANIFEST_VERSION, MAX_JSON_DEPTH,
    ManifestDocument, ManifestError, ManifestLimits, ManifestState, Reconciliation, TextRoot,
    Transport,
};
use std::error::Error;

/// Caller-selected ceiling for ordinary fixtures. This is not a format quota.
const CALLER_CEILING: usize = 1024 * 1024;

const CANARY: &str = "SENTINEL_9f3c1a7b";

/// Readable version-2 fixture. Whitespace, key escapes, and numeric spellings are deliberate.
const PAGES_FIXTURE: &str = r#"  {
  "version": 2,
  "library_id": "lib-1",
  "name": "Shelf",
  "text_root": "pages",
  "mounts": [
    {
      "mount_id": "mount-1",
      "name": "photos",
      "backend": "local",
      "location": "../not-a-root",
      "writable": false,
      "reconciliation": "manual",
      "vendor_extension": {"keep":true,"n":9007199254740993,"k":1,"k":2}
    }
  ],
  "exposure": {
    "mode": "read_only",
    "sensitive": true,
    "excluded_paths": ["secrets/a.md"],
    "transports": ["owner_stdio"],
    "revision": "rev-not-granted"
  },
  "convention_version": "0.2",
  "opaque_count": 9007199254740993,
  "numbers": {"big":9007199254740993,"frac":1.2300,"exp":1e+2,"negzero":-0,"wide":18446744073709551616},
  "ex\u0074ra_note": "kept"
}
"#;

/// Admit bytes under the ordinary caller ceiling.
fn admit(bytes: &[u8]) -> Result<ManifestDocument, ManifestError> {
    ManifestDocument::decode(
        bytes,
        ManifestLimits {
            max_bytes: CALLER_CEILING,
        },
    )
}

/// Admit bytes and keep the codec error visible in assertion output.
fn admit_ok(bytes: &[u8]) -> ManifestDocument {
    admit(bytes).unwrap_or_else(|error| panic!("decode failed: {error}"))
}

/// Build one supported compact document whose name token is raw JSON, including quotes.
fn compact_v2(name_token: &str) -> String {
    format!(
        r#"{{"version":2,"library_id":"lib-1","name":{name_token},"text_root":"pages","mounts":[{{"mount_id":"mount-1","name":"photos","backend":"local","location":"opaque","writable":false,"reconciliation":"manual"}}],"exposure":{{"mode":"off","sensitive":false,"excluded_paths":[],"transports":[]}},"convention_version":"0.2"}}"#
    )
}

/// Replace one exact substring. A missing or repeated needle is a fixture defect.
fn replace_once(input: &str, from: &str, to: &str) -> String {
    let Some(start) = input.find(from) else {
        panic!("missing fixture text {from}");
    };
    let rest = &input[start + from.len()..];
    assert!(!rest.contains(from), "fixture text {from} is not unique");
    let mut output = String::with_capacity(input.len() + to.len());
    output.push_str(&input[..start]);
    output.push_str(to);
    output.push_str(rest);
    output
}

/// Nest `depth` objects. The innermost object carries the sentinel number.
fn object_chain(depth: usize) -> String {
    assert!(depth >= 1, "depth counts objects");
    let mut encoded = String::from(r#"{"SENTINEL_DEPTH_9f3c":9007199254740993}"#);
    for _ in 1..depth {
        encoded = format!(r#"{{"n":{encoded}}}"#);
    }
    encoded
}

/// Supported document whose deepest container is exactly `depth` levels, including the root.
fn manifest_at_depth(depth: usize) -> String {
    assert!(
        depth >= 4,
        "the supported mount object already uses three levels"
    );
    let extension = object_chain(depth - 1);
    replace_once(
        &compact_v2(r#""Shelf""#),
        r#""convention_version":"0.2"}"#,
        &format!(r#""convention_version":"0.2","ext":{extension}}}"#),
    )
}

/// Walk one JSON value and return the index after it.
fn scan_value(bytes: &[u8], index: usize) -> Result<usize, &'static str> {
    let index = skip_ws(bytes, index);
    match bytes.get(index).copied() {
        Some(b'"') => scan_string(bytes, index),
        Some(b'{' | b'[') => scan_container(bytes, index),
        Some(b't') => expect_literal(bytes, index, b"true"),
        Some(b'f') => expect_literal(bytes, index, b"false"),
        Some(b'n') => expect_literal(bytes, index, b"null"),
        Some(b'-' | b'0'..=b'9') => scan_number(bytes, index),
        _ => Err("json value"),
    }
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\n' | b'\r' | b'\t')) {
        index += 1;
    }
    index
}

fn expect_literal(bytes: &[u8], index: usize, literal: &[u8]) -> Result<usize, &'static str> {
    let end = index + literal.len();
    if bytes.get(index..end) == Some(literal) {
        Ok(end)
    } else {
        Err("json literal")
    }
}

fn scan_number(bytes: &[u8], mut index: usize) -> Result<usize, &'static str> {
    if bytes.get(index) == Some(&b'-') {
        index += 1;
    }
    if bytes.get(index) == Some(&b'0') {
        index += 1;
    } else if matches!(bytes.get(index), Some(b'1'..=b'9')) {
        while matches!(bytes.get(index), Some(b'0'..=b'9')) {
            index += 1;
        }
    } else {
        return Err("json number");
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while matches!(bytes.get(index), Some(b'0'..=b'9')) {
            index += 1;
        }
        if index == start {
            return Err("json fraction");
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let start = index;
        while matches!(bytes.get(index), Some(b'0'..=b'9')) {
            index += 1;
        }
        if index == start {
            return Err("json exponent");
        }
    }
    Ok(index)
}

fn scan_string(bytes: &[u8], mut index: usize) -> Result<usize, &'static str> {
    if bytes.get(index) != Some(&b'"') {
        return Err("json string");
    }
    index += 1;
    while let Some(byte) = bytes.get(index).copied() {
        index += 1;
        match byte {
            b'"' => return Ok(index),
            b'\\' => {
                let escape = bytes.get(index).copied().ok_or("json escape")?;
                index += 1;
                if escape == b'u' {
                    if index + 4 > bytes.len() {
                        return Err("json unicode escape");
                    }
                    index += 4;
                }
            }
            _ => {}
        }
    }
    Err("unterminated json string")
}

fn scan_container(bytes: &[u8], index: usize) -> Result<usize, &'static str> {
    let open = bytes[index];
    let close = if open == b'{' { b'}' } else { b']' };
    let mut index = index + 1;
    let mut depth = 1usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = scan_string(bytes, index)?,
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                index += 1;
                if depth == 0 {
                    if bytes[index - 1] != close {
                        return Err("mismatched json container");
                    }
                    return Ok(index);
                }
            }
            _ => index += 1,
        }
    }
    Err("unclosed json container")
}

fn unescape_json_string(quoted: &[u8]) -> Result<String, &'static str> {
    if quoted.first() != Some(&b'"') || quoted.last() != Some(&b'"') || quoted.len() < 2 {
        return Err("quoted json string");
    }
    let inner = &quoted[1..quoted.len() - 1];
    let mut output = String::new();
    let mut index = 0usize;
    while index < inner.len() {
        if inner[index] != b'\\' {
            let character = std::str::from_utf8(&inner[index..])
                .map_err(|_| "json utf-8")?
                .chars()
                .next()
                .ok_or("json char")?;
            output.push(character);
            index += character.len_utf8();
            continue;
        }
        let escape = inner.get(index + 1).copied().ok_or("json escape")?;
        match escape {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{0008}'),
            b'f' => output.push('\u{000c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => {
                let hex = inner.get(index + 2..index + 6).ok_or("json unicode")?;
                let digits = std::str::from_utf8(hex).map_err(|_| "json unicode utf-8")?;
                let code = u32::from_str_radix(digits, 16).map_err(|_| "json unicode hex")?;
                output.push(char::from_u32(code).ok_or("json unicode scalar")?);
                index += 6;
                continue;
            }
            _ => return Err("json escape"),
        }
        index += 2;
    }
    Ok(output)
}

fn root_fields(bytes: &[u8]) -> Result<Vec<(String, &[u8])>, &'static str> {
    let mut index = skip_ws(bytes, 0);
    if bytes.get(index) != Some(&b'{') {
        return Err("json object");
    }
    index += 1;
    let mut fields = Vec::new();
    loop {
        index = skip_ws(bytes, index);
        if bytes.get(index) == Some(&b'}') {
            let rest = skip_ws(bytes, index + 1);
            if rest != bytes.len() {
                return Err("trailing json");
            }
            return Ok(fields);
        }
        let key_end = scan_string(bytes, index)?;
        let key = unescape_json_string(&bytes[index..key_end])?;
        index = skip_ws(bytes, key_end);
        if bytes.get(index) != Some(&b':') {
            return Err("json colon");
        }
        index = skip_ws(bytes, index + 1);
        let value_end = scan_value(bytes, index)?;
        fields.push((key, &bytes[index..value_end]));
        index = skip_ws(bytes, value_end);
        match bytes.get(index) {
            Some(b',') => index += 1,
            Some(b'}') => {}
            _ => return Err("json separator"),
        }
    }
}

fn field<'a>(fields: &[(String, &'a [u8])], key: &str) -> &'a [u8] {
    fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| *value)
        .unwrap_or_else(|| panic!("missing field {key}"))
}

/// Compare every top-level value except the decoded display name.
fn assert_raw_fields_preserved(original: &[u8], candidate: &[u8], new_name: &str) {
    let left = root_fields(original).expect("original object");
    let right = root_fields(candidate).expect("candidate object");
    let mut left_keys: Vec<_> = left.iter().map(|(key, _)| key.clone()).collect();
    let mut right_keys: Vec<_> = right.iter().map(|(key, _)| key.clone()).collect();
    left_keys.sort();
    right_keys.sort();
    assert_eq!(left_keys, right_keys, "top-level names changed");
    for (key, value) in &left {
        let observed = field(&right, key);
        if key == "name" {
            let decoded = unescape_json_string(observed).expect("name string");
            assert_eq!(decoded, new_name);
        } else {
            assert_eq!(observed, *value, "raw value changed for {key}");
        }
    }
}

trait ManifestResult {
    fn manifest_err(self, label: &str) -> ManifestError;
}

impl ManifestResult for Result<ManifestDocument, ManifestError> {
    fn manifest_err(self, label: &str) -> ManifestError {
        match self {
            Err(error) => error,
            Ok(_) => panic!("{label} produced a document"),
        }
    }
}

fn assert_input_free(error: &impl Error, canary: &str) {
    assert!(
        error.source().is_none(),
        "codec errors expose no source chain"
    );
    let rendered = error.to_string();
    assert!(!rendered.is_empty(), "diagnostic is empty");
    assert!(
        !rendered.contains(canary),
        "diagnostic echoed input: {rendered}"
    );
    let debug = format!("{error:?}");
    assert!(!debug.contains(canary), "debug echoed input: {debug}");
}

fn assert_read_only(bytes: &str, state: ManifestState, version: Option<u64>) {
    let document = admit_ok(bytes.as_bytes());
    assert_eq!(document.state(), state);
    assert_eq!(document.version(), version);
    assert!(document.metadata().is_none());
    assert_eq!(document.original_bytes(), bytes.as_bytes());
    let error = document.with_name(CANARY).manifest_err("read-only rename");
    assert_eq!(error, ManifestError::ReadOnly);
    assert_input_free(&error, CANARY);
    assert_eq!(document.original_bytes(), bytes.as_bytes());
}

/// Storage names `omni-knowledge.json`, version 2, and OKF 0.2. The public constants publish those spellings.
#[test]
fn public_manifest_constants_match_storage_identity() {
    assert_eq!(MANIFEST_FILE_NAME, "omni-knowledge.json");
    assert_eq!(MANIFEST_VERSION, 2);
    assert_eq!(CONVENTION_VERSION, "0.2");
    assert_eq!(MAX_JSON_DEPTH, 128);
}

/// The supported string convention projects service field names without treating metadata as a manifest.
#[test]
fn supported_version_two_projects_recognized_fields() {
    let fields = root_fields(PAGES_FIXTURE.as_bytes()).expect("fixture scanner");
    assert_eq!(field(&fields, "extra_note"), br#""kept""#);
    assert_eq!(field(&fields, "opaque_count"), b"9007199254740993");
    assert_eq!(
        field(&fields, "numbers"),
        br#"{"big":9007199254740993,"frac":1.2300,"exp":1e+2,"negzero":-0,"wide":18446744073709551616}"#
    );

    let document = admit_ok(PAGES_FIXTURE.as_bytes());
    assert_eq!(document.state(), ManifestState::EditableV2);
    assert_eq!(document.version(), Some(2));
    assert_eq!(document.original_bytes(), PAGES_FIXTURE.as_bytes());
    let metadata = document.metadata().expect("supported metadata");
    assert_eq!(metadata.library_id, "lib-1");
    assert_eq!(metadata.name, "Shelf");
    assert_eq!(metadata.text_root, TextRoot::Pages);
    assert_eq!(metadata.mounts.len(), 1);
    let mount = &metadata.mounts[0];
    assert_eq!(mount.mount_id, "mount-1");
    assert_eq!(mount.name, "photos");
    assert_eq!(mount.backend, BackendKind::Local);
    assert_eq!(mount.location, "../not-a-root");
    assert!(!mount.writable);
    assert_eq!(mount.reconciliation, Reconciliation::Manual);
    assert_eq!(metadata.exposure.mode, ExposureMode::ReadOnly);
    assert!(metadata.exposure.sensitive);
    assert_eq!(metadata.exposure.excluded_paths.len(), 1);
    assert_eq!(metadata.exposure.excluded_paths[0].as_str(), "secrets/a.md");
    assert_eq!(metadata.exposure.transports, vec![Transport::OwnerStdio]);
    assert_eq!(document.field_json("convention_version"), Some(r#""0.2""#));
    assert_eq!(document.field_json("version"), Some("2"));
    assert_eq!(document.field_json("missing"), None);
    assert!(
        document
            .field_json("exposure")
            .unwrap()
            .contains("revision")
    );
    assert!(
        document
            .field_json("mounts")
            .unwrap()
            .contains("vendor_extension")
    );
}

/// Wiki remains a mapped text root, and every ordinary backend tag is data rather than an adapter.
#[test]
fn wiki_root_and_ordinary_backend_tags_are_editable() {
    let bytes = r#"{
  "version": 2,
  "library_id": "not-a-uuid",
  "name": "Mapped",
  "text_root": "wiki",
  "mounts": [
    {"mount_id":"m-local","name":"local-dir","backend":"local","location":"opaque-local","writable":true,"reconciliation":"startup_periodic"},
    {"mount_id":"m-nas","name":"nas-dir","backend":"nas","location":"opaque-nas","writable":false,"reconciliation":"nightly_manual"},
    {"mount_id":"m-cloud","name":"cloud-dir","backend":"cloud_folder","location":"opaque-cloud","writable":false,"reconciliation":"manual"},
    {"mount_id":"m-git","name":"git-dir","backend":"git","location":"opaque-git","writable":false,"reconciliation":"manual"}
  ],
  "exposure": {"mode":"read_write","sensitive":false,"excluded_paths":["wiki/private.md"],"transports":["owner_ipc","http"]},
  "convention_version": "0.2"
}
"#;
    let document = admit_ok(bytes.as_bytes());
    assert_eq!(document.state(), ManifestState::EditableV2);
    let metadata = document.metadata().expect("wiki metadata");
    assert_eq!(metadata.library_id, "not-a-uuid");
    assert_eq!(metadata.name, "Mapped");
    assert_eq!(metadata.text_root, TextRoot::Wiki);
    let kinds: Vec<_> = metadata.mounts.iter().map(|mount| mount.backend).collect();
    assert_eq!(
        kinds,
        vec![
            BackendKind::Local,
            BackendKind::Nas,
            BackendKind::CloudFolder,
            BackendKind::Git
        ]
    );
    assert!(metadata.mounts[0].writable);
    assert!(!metadata.mounts[1].writable);
    assert_eq!(
        metadata.mounts[0].reconciliation,
        Reconciliation::StartupPeriodic
    );
    assert_eq!(
        metadata.mounts[1].reconciliation,
        Reconciliation::NightlyManual
    );
    assert_eq!(metadata.exposure.mode, ExposureMode::ReadWrite);
    assert!(!metadata.exposure.sensitive);
    assert_eq!(
        metadata.exposure.transports,
        vec![Transport::OwnerIpc, Transport::Http]
    );
    assert_eq!(
        metadata.exposure.excluded_paths[0].as_str(),
        "wiki/private.md"
    );
}

/// An unchanged display name is a byte copy, including escaped keys and surrounding whitespace.
#[test]
fn unchanged_name_preserves_original_bytes() {
    let document = admit_ok(PAGES_FIXTURE.as_bytes());
    let candidate = document.with_name("Shelf").expect("unchanged name");
    assert_eq!(candidate.original_bytes(), PAGES_FIXTURE.as_bytes());
    assert_eq!(document.original_bytes(), PAGES_FIXTURE.as_bytes());

    let escaped = compact_v2(r#""\u0053helf""#);
    let escaped_document = admit_ok(escaped.as_bytes());
    assert_eq!(escaped_document.metadata().expect("name").name, "Shelf");
    let copied = escaped_document.with_name("Shelf").expect("escaped no-op");
    assert_eq!(copied.original_bytes(), escaped.as_bytes());
}

/// A changed name may normalize only the outer object. Every other raw value stays verbatim.
#[test]
fn changed_name_preserves_unrelated_raw_values_and_numeric_spellings() {
    let document = admit_ok(PAGES_FIXTURE.as_bytes());
    let candidate = document.with_name("Renamed shelf").expect("rename");
    assert_ne!(candidate.original_bytes(), PAGES_FIXTURE.as_bytes());
    assert_eq!(document.original_bytes(), PAGES_FIXTURE.as_bytes());
    assert_raw_fields_preserved(
        PAGES_FIXTURE.as_bytes(),
        candidate.original_bytes(),
        "Renamed shelf",
    );
    assert_eq!(candidate.metadata().expect("renamed").name, "Renamed shelf");
    let again = admit_ok(candidate.original_bytes());
    assert_eq!(again.state(), ManifestState::EditableV2);
    assert_eq!(again.field_json("opaque_count"), Some("9007199254740993"));
    assert_eq!(
        again.field_json("numbers"),
        Some(
            r#"{"big":9007199254740993,"frac":1.2300,"exp":1e+2,"negzero":-0,"wide":18446744073709551616}"#
        )
    );
    assert!(
        again
            .field_json("mounts")
            .unwrap()
            .contains(r#""k":1,"k":2"#)
    );
}

/// Display names are preserved without trimming, and escaping belongs only to the changed name.
#[test]
fn changed_name_keeps_spacing_and_json_escapes_inside_the_name() {
    let spaced = compact_v2(r#""  A  ""#);
    let document = admit_ok(spaced.as_bytes());
    assert_eq!(document.metadata().expect("spaced").name, "  A  ");
    let same = document.with_name("  A  ").expect("spaced no-op");
    assert_eq!(same.original_bytes(), spaced.as_bytes());

    let renamed = document
        .with_name("A \"quoted\" \\\n\u{0001}")
        .expect("escaped rename");
    assert_eq!(
        renamed.metadata().expect("escaped").name,
        "A \"quoted\" \\\n\u{0001}"
    );
    assert_raw_fields_preserved(
        spaced.as_bytes(),
        renamed.original_bytes(),
        "A \"quoted\" \\\n\u{0001}",
    );
    assert_eq!(document.original_bytes(), spaced.as_bytes());
}

/// Legacy version 1 stays inspectable. A no-op rename still refuses migration.
#[test]
fn legacy_version_one_is_inspectable_and_refuses_mutation() {
    let bytes = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""version":2"#,
        r#""version":1"#,
    );
    let document = admit_ok(bytes.as_bytes());
    assert_eq!(document.state(), ManifestState::LegacyV1);
    assert_eq!(document.version(), Some(1));
    assert!(document.metadata().is_none());
    assert_eq!(document.field_json("version"), Some("1"));
    assert_eq!(document.field_json("text_root"), Some(r#""pages""#));
    assert_eq!(document.original_bytes(), bytes.as_bytes());
    let error = document.with_name("Shelf").manifest_err("legacy no-op");
    assert_eq!(error, ManifestError::ReadOnly);
    assert_input_free(&error, "Shelf");
    assert_eq!(document.original_bytes(), bytes.as_bytes());
}

/// A future required version remains verbatim and read-only.
#[test]
fn future_and_other_unsigned_versions_are_read_only() {
    for version in [0u64, 3, 99] {
        let bytes = replace_once(
            &compact_v2(r#""Shelf""#),
            r#""version":2"#,
            &format!(r#""version":{version}"#),
        );
        assert_read_only(&bytes, ManifestState::UnsupportedVersion, Some(version));
    }
}

/// A version that is not an unsigned integer is a malformed recognized field, not a migration.
#[test]
fn non_unsigned_version_shapes_are_invalid_known_fields() {
    let samples = [
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""version":2"#,
            r#""version":"2""#,
        ),
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""version":2"#,
            r#""version":-1"#,
        ),
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""version":2"#,
            r#""version":18446744073709551616"#,
        ),
        replace_once(&compact_v2(r#""Shelf""#), r#""version":2,"#, ""),
    ];
    for bytes in samples {
        assert_read_only(&bytes, ManifestState::InvalidKnownFields, None);
    }
}

/// Storage requires OKF 0.2 but does not choose a JSON encoding. Only the string "0.2" is editable.
#[test]
fn other_convention_representations_remain_verbatim_read_only() {
    let numeric = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""convention_version":"0.2""#,
        r#""convention_version":0.2"#,
    );
    let prefixed = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""convention_version":"0.2""#,
        r#""convention_version":"OKF 0.2""#,
    );
    assert_read_only(&numeric, ManifestState::UnsupportedSemantics, Some(2));
    assert_eq!(
        admit_ok(numeric.as_bytes()).field_json("convention_version"),
        Some("0.2")
    );
    assert_read_only(&prefixed, ManifestState::UnsupportedSemantics, Some(2));
    assert_eq!(
        admit_ok(prefixed.as_bytes()).field_json("convention_version"),
        Some(r#""OKF 0.2""#)
    );
}

/// An unrecognized required tag is read-only. It is not rewritten into a supported tag.
#[test]
fn unrecognized_required_semantics_are_read_only() {
    let cases = [
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""text_root":"pages""#,
            r#""text_root":"notes""#,
        ),
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""backend":"local""#,
            r#""backend":"s3""#,
        ),
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""reconciliation":"manual""#,
            r#""reconciliation":"hourly""#,
        ),
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""mode":"off""#,
            r#""mode":"admin""#,
        ),
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""transports":[]"#,
            r#""transports":["websocket"]"#,
        ),
    ];
    for bytes in cases {
        assert_read_only(&bytes, ManifestState::UnsupportedSemantics, Some(2));
    }
}

/// Absent, duplicated, or inconsistent recognized fields stay inspectable and refuse mutation.
#[test]
fn malformed_recognized_fields_are_read_only() {
    let missing = replace_once(&compact_v2(r#""Shelf""#), r#""library_id":"lib-1","#, "");
    let duplicate_names = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""mounts":[{"mount_id":"mount-1","name":"photos","backend":"local","location":"opaque","writable":false,"reconciliation":"manual"}]"#,
        r#""mounts":[{"mount_id":"mount-1","name":"photos","backend":"local","location":"opaque","writable":false,"reconciliation":"manual"},{"mount_id":"mount-2","name":"photos","backend":"nas","location":"opaque","writable":false,"reconciliation":"manual"}]"#,
    );
    let nested_name = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""name":"photos""#,
        r#""name":"a/b""#,
    );
    let empty_mount = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""mount_id":"mount-1""#,
        r#""mount_id":"""#,
    );
    let duplicate_exposure = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""sensitive":false"#,
        r#""sensitive":false,"sensitive":true"#,
    );
    let absolute_exclusion = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""excluded_paths":[]"#,
        r#""excluded_paths":["/tmp/x"]"#,
    );
    let wrong_writable = replace_once(
        &compact_v2(r#""Shelf""#),
        r#""writable":false"#,
        r#""writable":"false""#,
    );
    for bytes in [
        missing,
        duplicate_names,
        nested_name,
        empty_mount,
        duplicate_exposure,
        absolute_exclusion,
        wrong_writable,
    ] {
        assert_read_only(&bytes, ManifestState::InvalidKnownFields, Some(2));
    }
}

/// Syntax, outer duplicate keys, and a non-object value fail before a document exists.
#[test]
fn invalid_json_and_duplicate_outer_keys_fail_closed() {
    let base = compact_v2(r#""Shelf""#);
    let duplicate = format!(r#"{{"version":1,{}"#, &base[1..]);
    let samples = [
        format!(r#"{{"broken":"{CANARY}""#),
        "[]".to_string(),
        "null".to_string(),
        format!(r#""{CANARY}""#),
        "1".to_string(),
        "{}{}".to_string(),
        duplicate,
    ];
    for bytes in samples {
        let error = admit(bytes.as_bytes()).manifest_err("invalid json");
        assert_eq!(error, ManifestError::InvalidJson);
        assert_input_free(&error, CANARY);
    }
}

/// Invalid UTF-8 inside the caller ceiling is a fixed UTF-8 failure.
#[test]
fn invalid_utf8_does_not_echo_the_byte_stream() {
    let mut bytes = format!(r#"{{"note":"{CANARY}","version":2}}"#).into_bytes();
    bytes.push(0xff);
    let error = admit(&bytes).manifest_err("utf-8");
    assert_eq!(error, ManifestError::InvalidUtf8);
    assert_input_free(&error, CANARY);
}

/// A ceiling rejects input that exceeds it. Equal length still distinguishes valid JSON from syntax.
/// Empty bytes do not exceed a zero ceiling, so they remain invalid JSON rather than a budget failure.
#[test]
fn input_byte_ceiling_refuses_without_echoing_the_body() {
    let bytes = compact_v2(&format!(r#""{CANARY}""#));
    let zero = ManifestDocument::decode(bytes.as_bytes(), ManifestLimits { max_bytes: 0 })
        .manifest_err("zero ceiling");
    assert_eq!(zero, ManifestError::ByteLimit);
    assert_input_free(&zero, CANARY);

    let short = ManifestDocument::decode(
        bytes.as_bytes(),
        ManifestLimits {
            max_bytes: bytes.len() - 1,
        },
    )
    .manifest_err("short ceiling");
    assert_eq!(short, ManifestError::ByteLimit);
    assert_input_free(&short, CANARY);

    let empty_within =
        ManifestDocument::decode(b"", ManifestLimits { max_bytes: 1 }).manifest_err("empty within");
    assert_eq!(empty_within, ManifestError::InvalidJson);
    assert_input_free(&empty_within, CANARY);

    // Length zero does not exceed a zero ceiling. Both "invalid JSON" and "zero admits nothing"
    // describe this input, and neither public text orders those reject reasons.
    let empty_zero =
        ManifestDocument::decode(b"", ManifestLimits { max_bytes: 0 }).manifest_err("empty zero");
    assert!(matches!(
        empty_zero,
        ManifestError::ByteLimit | ManifestError::InvalidJson
    ));
    assert_input_free(&empty_zero, CANARY);

    let exact = ManifestDocument::decode(
        bytes.as_bytes(),
        ManifestLimits {
            max_bytes: bytes.len(),
        },
    )
    .expect("exact ceiling");
    assert_eq!(exact.state(), ManifestState::EditableV2);
    assert_eq!(exact.original_bytes(), bytes.as_bytes());
}

/// Public JSON string length. Quotes, backslashes, and controls expand; other
/// characters stay one scalar. This is not codec output.
fn json_string(name: &str) -> String {
    let mut out = String::from("\"");
    for ch in name.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn compact_shell() -> (String, String) {
    let marker = "\"__NAME_MARK__\"";
    let skeleton = compact_v2(marker);
    let at = skeleton.find(marker).expect("name marker");
    (
        skeleton[..at].to_string(),
        skeleton[at + marker.len()..].to_string(),
    )
}

fn compact_len(prefix: &str, suffix: &str, name: &str) -> usize {
    prefix.len() + json_string(name).len() + suffix.len()
}

fn refuse_output(document: &ManifestDocument, name: &str, label: &str) {
    match document.with_name(name) {
        Ok(candidate) => panic!(
            "{label} returned {} candidate bytes",
            candidate.original_bytes().len()
        ),
        Err(error) => {
            assert_eq!(error, ManifestError::ByteLimit, "{label}");
            assert_input_free(&error, CANARY);
        }
    }
}

/// Output accounting includes escape expansion and the complete candidate.
/// Lengths are the public JSON spelling of this fixture, not codec output.
#[test]
fn escaped_rename_over_the_output_ceiling_returns_no_candidate() {
    let bytes = compact_v2(r#""Shelf""#);
    let (prefix, suffix) = compact_shell();
    assert_eq!(
        compact_len(&prefix, &suffix, "Shelf"),
        bytes.len(),
        "fixture shell does not match the compact name token"
    );
    let ceiling = bytes.len();
    let document =
        ManifestDocument::decode(bytes.as_bytes(), ManifestLimits { max_bytes: ceiling })
            .expect("input fits");
    let copied = document.with_name("Shelf").expect("no-op within ceiling");
    assert_eq!(copied.original_bytes(), bytes.as_bytes());

    let mut escaped_name = String::from(CANARY);
    escaped_name.push_str(&"\"".repeat(ceiling / 2));
    let escaped_len = json_string(&escaped_name).len();
    assert!(
        escaped_name.len() <= ceiling,
        "raw name {} exceeded ceiling {ceiling}",
        escaped_name.len()
    );
    assert!(
        escaped_len > ceiling,
        "escaped name {escaped_len} did not exceed ceiling {ceiling}"
    );
    refuse_output(&document, &escaped_name, "escaped name");
    assert_eq!(document.original_bytes(), bytes.as_bytes());

    let encoded_name = CANARY.to_string();
    let encoded_len = json_string(&encoded_name).len();
    let complete = compact_len(&prefix, &suffix, &encoded_name);
    assert!(encoded_name.len() <= ceiling, "raw encoded-name stimulus");
    assert!(
        encoded_len <= ceiling,
        "encoded name {encoded_len} exceeded ceiling {ceiling}"
    );
    assert!(
        complete > ceiling,
        "complete manifest {complete} did not exceed ceiling {ceiling}"
    );
    refuse_output(&document, &encoded_name, "complete manifest");
    assert_eq!(document.original_bytes(), bytes.as_bytes());

    let changed = "Renamed";
    let changed_complete = compact_len(&prefix, &suffix, changed);
    assert_ne!(changed, "Shelf");
    assert!(changed.len() <= changed_complete);
    assert!(json_string(changed).len() <= changed_complete);
    assert!(changed_complete >= bytes.len());
    let wide = ManifestDocument::decode(
        bytes.as_bytes(),
        ManifestLimits {
            max_bytes: changed_complete,
        },
    )
    .expect("changed-name ceiling admits the input");
    let candidate = wide.with_name(changed).expect("changed name within budget");
    assert!(!candidate.original_bytes().is_empty(), "empty candidate");
    assert!(
        candidate.original_bytes().len() <= changed_complete,
        "candidate {} exceeded budget {changed_complete}",
        candidate.original_bytes().len()
    );
    assert_eq!(candidate.metadata().expect("renamed").name, changed);
    assert_eq!(wide.original_bytes(), bytes.as_bytes());
}

/// Depth equal to the published ceiling remains a normal supported document.
#[test]
fn object_depth_at_public_ceiling_stays_editable() {
    let bytes = manifest_at_depth(MAX_JSON_DEPTH);
    let document = admit_ok(bytes.as_bytes());
    assert_eq!(document.state(), ManifestState::EditableV2);
    let extension = document.field_json("ext").expect("deep extension");
    assert!(extension.contains("9007199254740993"));
    assert!(extension.contains("SENTINEL_DEPTH_9f3c"));
    let renamed = document.with_name("Deep").expect("rename at ceiling");
    assert_raw_fields_preserved(bytes.as_bytes(), renamed.original_bytes(), "Deep");
}

/// One level past the ceiling, and a far deeper object, are parser limits rather than corrupt files.
#[test]
fn object_depth_past_public_ceiling_is_depth_limit() {
    for depth in [MAX_JSON_DEPTH + 1, MAX_JSON_DEPTH + 72] {
        let bytes = manifest_at_depth(depth);
        let error = admit(bytes.as_bytes()).manifest_err("deep object");
        assert_eq!(error, ManifestError::DepthLimit);
        assert_input_free(&error, "SENTINEL_DEPTH_9f3c");
        assert_input_free(&error, "9007199254740993");
    }
}

/// The same codec failure keeps one fixed diagnostic and does not borrow a Serde payload.
#[test]
fn repeated_codec_failures_keep_a_fixed_input_free_diagnostic() {
    let first = admit(b"[").manifest_err("first");
    let second = admit(b"]").manifest_err("second");
    assert_eq!(first, second);
    assert_eq!(first.to_string(), second.to_string());
    assert_input_free(&first, CANARY);

    let legacy = admit_ok(
        replace_once(
            &compact_v2(r#""Shelf""#),
            r#""version":2"#,
            r#""version":1"#,
        )
        .as_bytes(),
    );
    let left = legacy.with_name("one").manifest_err("left");
    let right = legacy.with_name("two").manifest_err("right");
    assert_eq!(left, right);
    assert_eq!(left.to_string(), right.to_string());
}
