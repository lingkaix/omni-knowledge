//! Canonical integer tokens for JSON safe-integer request fields.
//!
//! The service contract admits only canonical integer spelling on those fields
//! and requires a typed refusal for fractional spelling and negative zero.
//! Exponent spelling stays in `json_safe_integers_stop_at_the_published_limit`.
//! `U64` and signed-decimal strings keep their own spelling. Finite scores are
//! non-integer numbers, and this module does not refuse them.

use serde_json::Value;

use omni_service::{CodecError, MAX_SAFE_INTEGER, decode_request};

use crate::support::{DIGEST, assert_input_free, envelope, refuse};

fn verify_budget(max_files: &str, max_millis: &str) -> String {
    envelope(&format!(
        r#"{{"op":"verify_source","source":{{"library_id":"syn-library","source_id":"syn-source"}},"budget":{{"max_files":{max_files},"max_bytes":"0","max_millis":{max_millis},"max_memory_bytes":"0"}}}}"#
    ))
}

fn browse_limit(limit: &str) -> String {
    envelope(&format!(
        r#"{{"op":"browse","library_id":"syn-library","mount_id":null,"path":"","cursor":null,"limit":{limit}}}"#
    ))
}

fn representation(start_line: &str, max_lines: &str) -> String {
    envelope(&format!(
        r#"{{"op":"read_representation","source":{{"library_id":"syn-library","source_id":"syn-source"}},"representation_digest":"{DIGEST}","start_line":{start_line},"max_lines":{max_lines}}}"#
    ))
}

fn assert_integer_token(value: &Value, expected: u64, spelling: &str) {
    let number = value
        .as_number()
        .unwrap_or_else(|| panic!("JSON number for {spelling}"));
    assert!(
        !number.is_f64(),
        "canonical token {spelling} was stored as a fraction"
    );
    assert_eq!(number.as_u64(), Some(expected), "token {spelling}");
    assert_eq!(
        serde_json::to_string(value).expect("integer spelling"),
        spelling
    );
}

fn refused(raw: &str, token: &str) {
    let err = refuse(raw.as_bytes());
    assert_eq!(err, CodecError::InvalidWireData);
    assert_input_free(&err, token);
}

/// Zero and 100 stay exact integers on the shared budget, and decimal strings stay strings.
#[test]
fn budget_preserves_zero_and_one_hundred() {
    let zero = decode_request(verify_budget("0", "0").as_bytes()).expect("budget zero");
    match &zero.command {
        omni_service::Command::VerifySource { budget, .. } => {
            assert_eq!(budget.max_files.get(), 0);
            assert_eq!(budget.max_millis.get(), 0);
            assert_eq!(budget.max_bytes.get(), 0);
            assert_eq!(budget.max_memory_bytes.get(), 0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    let zero_wire = serde_json::to_value(&zero).expect("json");
    assert_integer_token(&zero_wire["command"]["budget"]["max_files"], 0, "0");
    assert_integer_token(&zero_wire["command"]["budget"]["max_millis"], 0, "0");
    assert_eq!(zero_wire["command"]["budget"]["max_bytes"], "0");
    assert_eq!(zero_wire["command"]["budget"]["max_memory_bytes"], "0");

    let hundred = decode_request(verify_budget("100", "100").as_bytes()).expect("budget 100");
    match &hundred.command {
        omni_service::Command::VerifySource { budget, .. } => {
            assert_eq!(budget.max_files.get(), 100);
            assert_eq!(budget.max_millis.get(), 100);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    let hundred_wire = serde_json::to_value(&hundred).expect("json");
    assert_integer_token(&hundred_wire["command"]["budget"]["max_files"], 100, "100");
    assert_integer_token(&hundred_wire["command"]["budget"]["max_millis"], 100, "100");
}

/// A browse limit of 100 stays the integer token, inside the published 1 through 1,000 bound.
#[test]
fn browse_limit_preserves_one_hundred() {
    let decoded = decode_request(browse_limit("100").as_bytes()).expect("browse 100");
    match &decoded.command {
        omni_service::Command::Browse { limit, .. } => assert_eq!(limit.get(), 100),
        other => panic!("unexpected command: {other:?}"),
    }
    let wire = serde_json::to_value(&decoded).expect("json");
    assert_integer_token(&wire["command"]["limit"], 100, "100");
}

/// One-based start lines preserve 100 and the published safe endpoint.
#[test]
fn start_line_preserves_one_hundred_and_the_safe_endpoint() {
    let hundred = decode_request(representation("100", "1").as_bytes()).expect("line 100");
    match &hundred.command {
        omni_service::Command::ReadRepresentation {
            start_line,
            max_lines,
            ..
        } => {
            assert_eq!(start_line.get(), 100);
            assert_eq!(max_lines.get(), 1);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    let hundred_wire = serde_json::to_value(&hundred).expect("json");
    assert_integer_token(&hundred_wire["command"]["start_line"], 100, "100");
    assert_integer_token(&hundred_wire["command"]["max_lines"], 1, "1");

    let endpoint = decode_request(representation("9007199254740991", "1").as_bytes())
        .expect("safe start line");
    match &endpoint.command {
        omni_service::Command::ReadRepresentation { start_line, .. } => {
            assert_eq!(start_line.get(), MAX_SAFE_INTEGER);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    let endpoint_wire = serde_json::to_value(&endpoint).expect("json");
    assert_integer_token(
        &endpoint_wire["command"]["start_line"],
        MAX_SAFE_INTEGER,
        "9007199254740991",
    );
}

/// `100.0` and `-0` are refused where the corresponding canonical integer is legal.
#[test]
fn fractional_and_negative_zero_spellings_are_typed_refusals() {
    refused(&verify_budget("100.0", "0"), "100.0");
    refused(&verify_budget("0", "100.0"), "100.0");
    refused(&verify_budget("-0", "0"), "-0");
    refused(&verify_budget("0", "-0"), "-0");
    refused(&browse_limit("100.0"), "100.0");
    refused(&representation("100.0", "1"), "100.0");
}
