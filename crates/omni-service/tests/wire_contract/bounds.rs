use std::error::Error;
use std::path::Path;

use omni_service::{
    CodecError, Digest, FiniteNumber, Id, MAX_CONTROL_FRAME_BYTES, MAX_SAFE_INTEGER,
    PositiveInteger, RelativePath, RootRelativePath, SafeInteger, ScalarError, SignedDecimal, Time,
    U64, UnsignedInteger, WIRE_VERSION, decode_request,
};

use crate::support::{CANARY, DIGEST, assert_input_free, envelope, refuse};

const EXPECTED: &str = concat!(
    r#"{"digest":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","#,
    r#""size_bytes":"1","modified_ns":"0","changed_ns":null,"file_id":null,"backend_version":null}"#
);

fn body(command: &str) -> CodecError {
    let raw = envelope(command);
    refuse(raw.as_bytes())
}

#[test]
fn published_bounds_match_the_owner_contract() {
    assert_eq!(WIRE_VERSION, 1);
    assert_eq!(MAX_CONTROL_FRAME_BYTES, 1024 * 1024);
    assert_eq!(MAX_SAFE_INTEGER, 9_007_199_254_740_991);
    assert_eq!(DIGEST.len(), 64);
    assert_eq!(U64::new(0).get(), 0);
    assert_eq!(U64::new(u64::MAX).get(), 18_446_744_073_709_551_615);
}

#[test]
fn missing_extra_and_duplicate_keys_are_refused() {
    let missing_mount =
        body(r#"{"op":"browse","library_id":"syn-library","path":"","cursor":null,"limit":1}"#);
    assert_ne!(missing_mount, CodecError::FrameTooLarge);
    let missing_cursor =
        body(r#"{"op":"browse","library_id":"syn-library","mount_id":null,"path":"","limit":1}"#);
    assert_ne!(missing_cursor, CodecError::UnsupportedVersion);
    let missing_expected = body(
        r#"{"op":"edit_text","target":{"library_id":"syn-library","mount_id":null,"path":"pages/index.md"},"text":"synthetic","operation_id":"syn-op"}"#,
    );
    assert_ne!(missing_expected, CodecError::FrameTooLarge);
    let omitted_modified = body(&format!(
        r#"{{"op":"read_range","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{{"digest":"{DIGEST}","size_bytes":"1","changed_ns":null,"file_id":null,"backend_version":null}},"offset":"0","length":1}}"#
    ));
    assert_ne!(omitted_modified, CodecError::FrameTooLarge);

    let extra = envelope(&format!(
        r#"{{"op":"library_list","confirmed":true,"owner":"{CANARY}"}}"#
    ));
    let extra_err = refuse(extra.as_bytes());
    assert_input_free(&extra_err, CANARY);

    let nested = envelope(&format!(
        r#"{{"op":"library_open","path":"synthetic-os-folder","owner":"{CANARY}"}}"#
    ));
    assert_input_free(&refuse(nested.as_bytes()), CANARY);

    let duplicate = format!(
        r#"{{"wire_version":1,"request_id":"syn-bound","request_id":"{CANARY}","command":{{"op":"library_list"}}}}"#
    );
    assert_input_free(&refuse(duplicate.as_bytes()), CANARY);

    let duplicate_limit = body(
        r#"{"op":"browse","library_id":"syn-library","mount_id":null,"path":"","cursor":null,"limit":1,"limit":2}"#,
    );
    assert_ne!(duplicate_limit, CodecError::FrameTooLarge);
}

#[test]
fn explicit_null_expected_version_fields_decode() {
    let raw = envelope(&format!(
        r#"{{"op":"read_range","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"offset":"0","length":1}}"#
    ));
    let decoded = decode_request(raw.as_bytes()).expect("null version fields");
    let wire = serde_json::to_value(&decoded).expect("json");
    let expected = &wire["command"]["expected"];
    assert!(expected.get("changed_ns").is_some());
    assert!(expected["changed_ns"].is_null());
    assert!(expected.get("file_id").is_some());
    assert!(expected["file_id"].is_null());
    assert!(expected.get("backend_version").is_some());
    assert!(expected["backend_version"].is_null());
    assert_eq!(expected["modified_ns"], "0");
}

#[test]
fn malformed_utf8_and_version_are_distinct() {
    let truncated =
        refuse(br#"{"wire_version":1,"request_id":"syn-bound","command":{"op":"library_list""#);
    assert_ne!(truncated, CodecError::UnsupportedVersion);
    assert_ne!(truncated, CodecError::FrameTooLarge);

    let pair = refuse(
        br#"{"wire_version":1,"request_id":"syn-bound","command":{"op":"library_list"}}{"op":"library_list"}"#,
    );
    assert_ne!(pair, CodecError::UnsupportedVersion);

    let bad_utf8 = vec![0xff, 0xfe];
    assert!(bad_utf8.len() <= MAX_CONTROL_FRAME_BYTES);
    let utf8_err = refuse(&bad_utf8);
    assert_eq!(utf8_err, CodecError::InvalidUtf8);

    let version =
        envelope(r#"{"op":"library_list"}"#).replace("\"wire_version\":1", "\"wire_version\":2");
    let version_err = refuse(version.as_bytes());
    assert_eq!(version_err, CodecError::UnsupportedVersion);
    assert!(!version_err.to_string().contains("library_list"));

    let zero =
        envelope(r#"{"op":"library_list"}"#).replace("\"wire_version\":1", "\"wire_version\":0");
    assert_eq!(refuse(zero.as_bytes()), CodecError::UnsupportedVersion);

    let unknown = envelope(&format!(r#"{{"op":"{CANARY}-nope"}}"#));
    let unknown_a = refuse(unknown.as_bytes());
    let unknown_b = refuse(envelope(r#"{"op":"other-nope"}"#).as_bytes());
    assert_eq!(unknown_a, unknown_b);
    assert_input_free(&unknown_a, CANARY);
    assert_ne!(unknown_a, CodecError::UnsupportedVersion);
}

#[test]
fn frame_bound_is_checked_before_utf8_or_partial_success() {
    let minimal = r#"{"wire_version":1,"request_id":"syn-bound","command":{"op":"library_list"}}"#;
    assert!(minimal.len() < MAX_CONTROL_FRAME_BYTES);
    let mut exact = minimal.as_bytes().to_vec();
    exact.resize(MAX_CONTROL_FRAME_BYTES, b' ');
    assert_eq!(exact.len(), MAX_CONTROL_FRAME_BYTES);
    let decoded = decode_request(&exact).expect("exact frame of one JSON value plus spaces");
    assert!(matches!(
        decoded.command,
        omni_service::Command::LibraryList {}
    ));

    let mut over = exact.clone();
    over.push(b' ');
    assert_eq!(over.len(), MAX_CONTROL_FRAME_BYTES + 1);
    assert_eq!(
        decode_request(&over).expect_err("one byte over"),
        CodecError::FrameTooLarge
    );

    let mut invalid_inside = vec![b'x'; MAX_CONTROL_FRAME_BYTES];
    invalid_inside[0] = 0xff;
    assert_eq!(
        decode_request(&invalid_inside).expect_err("utf-8 inside the frame"),
        CodecError::InvalidUtf8
    );

    let invalid_over = vec![0xff; MAX_CONTROL_FRAME_BYTES + 1];
    assert_eq!(
        decode_request(&invalid_over).expect_err("bound before utf-8"),
        CodecError::FrameTooLarge
    );
}

#[test]
fn trailing_newline_stays_inside_one_frame() {
    let raw =
        "{\"wire_version\":1,\"request_id\":\"syn-bound\",\"command\":{\"op\":\"library_list\"}}\n";
    assert!(raw.len() <= MAX_CONTROL_FRAME_BYTES);
    let decoded = decode_request(raw.as_bytes()).expect("one value plus delimiter");
    assert!(matches!(
        decoded.command,
        omni_service::Command::LibraryList {}
    ));
    assert_eq!(decoded.request_id.as_str(), "syn-bound");
}

#[test]
fn u64_decimal_strings_cover_the_full_width() {
    let accept = |offset: &str| {
        let raw = envelope(&format!(
            r#"{{"op":"read_range","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"offset":"{offset}","length":1}}"#
        ));
        decode_request(raw.as_bytes()).unwrap_or_else(|err| panic!("offset {offset}: {err}"))
    };
    match accept("0").command {
        omni_service::Command::ReadRange { offset, .. } => assert_eq!(offset.get(), 0),
        other => panic!("unexpected command: {other:?}"),
    }
    match accept("18446744073709551615").command {
        omni_service::Command::ReadRange { offset, .. } => assert_eq!(offset.get(), u64::MAX),
        other => panic!("unexpected command: {other:?}"),
    }

    for rejected in [
        "18446744073709551616",
        "01",
        "00",
        "-1",
        "+1",
        " 1",
        "",
        "1 ",
    ] {
        let raw = envelope(&format!(
            r#"{{"op":"read_range","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"offset":"{rejected}","length":1}}"#
        ));
        let err = refuse(raw.as_bytes());
        assert_ne!(err, CodecError::FrameTooLarge);
        assert_ne!(err, CodecError::UnsupportedVersion);
    }

    let number = envelope(&format!(
        r#"{{"op":"read_range","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"offset":1,"length":1}}"#
    ));
    assert_ne!(refuse(number.as_bytes()), CodecError::UnsupportedVersion);
}

#[test]
fn json_safe_integers_stop_at_the_published_limit() {
    let budget = |files: &str| {
        envelope(&format!(
            r#"{{"op":"verify_source","source":{{"library_id":"syn-library","source_id":"syn-source"}},"budget":{{"max_files":{files},"max_bytes":"0","max_millis":0,"max_memory_bytes":"0"}}}}"#
        ))
    };
    let max = budget("9007199254740991");
    let decoded = decode_request(max.as_bytes()).expect("max safe integer");
    match decoded.command {
        omni_service::Command::VerifySource { budget, .. } => {
            assert_eq!(budget.max_files.get(), MAX_SAFE_INTEGER);
            assert_eq!(budget.max_bytes.get(), 0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    for rejected in ["9007199254740992", "-1", "1.5", "1e2"] {
        let err = refuse(budget(rejected).as_bytes());
        assert_ne!(err, CodecError::FrameTooLarge);
    }

    assert_eq!(
        UnsignedInteger::new(MAX_SAFE_INTEGER).expect("max").get(),
        MAX_SAFE_INTEGER
    );
    assert_eq!(
        UnsignedInteger::new(MAX_SAFE_INTEGER + 1).expect_err("above max"),
        ScalarError::InvalidInteger
    );
    assert_eq!(
        SafeInteger::new(MAX_SAFE_INTEGER as i64)
            .expect("positive")
            .get(),
        MAX_SAFE_INTEGER as i64
    );
    assert_eq!(
        SafeInteger::new(-(MAX_SAFE_INTEGER as i64))
            .expect("negative")
            .get(),
        -(MAX_SAFE_INTEGER as i64)
    );
    assert!(SafeInteger::new((MAX_SAFE_INTEGER as i64) + 1).is_err());
    assert!(SafeInteger::new(-(MAX_SAFE_INTEGER as i64) - 1).is_err());
}

#[test]
fn positive_integers_reject_zero_and_unsafe_values() {
    assert_eq!(PositiveInteger::new(1).expect("one").get(), 1);
    assert_eq!(
        PositiveInteger::new(MAX_SAFE_INTEGER).expect("max").get(),
        MAX_SAFE_INTEGER
    );
    assert!(PositiveInteger::new(0).is_err());
    assert!(PositiveInteger::new(MAX_SAFE_INTEGER + 1).is_err());

    let zero = envelope(&format!(
        r#"{{"op":"read_representation","source":{{"library_id":"syn-library","source_id":"syn-source"}},"representation_digest":"{DIGEST}","start_line":0,"max_lines":1}}"#
    ));
    assert_ne!(refuse(zero.as_bytes()), CodecError::FrameTooLarge);
    let top = envelope(&format!(
        r#"{{"op":"read_representation","source":{{"library_id":"syn-library","source_id":"syn-source"}},"representation_digest":"{DIGEST}","start_line":9007199254740991,"max_lines":1}}"#
    ));
    decode_request(top.as_bytes()).expect("max positive line");
    let over = envelope(&format!(
        r#"{{"op":"read_representation","source":{{"library_id":"syn-library","source_id":"syn-source"}},"representation_digest":"{DIGEST}","start_line":9007199254740992,"max_lines":1}}"#
    ));
    assert_ne!(refuse(over.as_bytes()), CodecError::FrameTooLarge);
}

#[test]
fn non_finite_scores_cannot_become_null() {
    assert!(FiniteNumber::new(0.0).is_ok());
    assert!(FiniteNumber::new(-0.0).is_ok());
    assert_eq!(FiniteNumber::new(1.5).expect("finite").get(), 1.5);
    assert_eq!(FiniteNumber::new(-1.5).expect("negative").get(), -1.5);
    assert!(f64::MAX.is_finite());
    assert!(FiniteNumber::new(f64::MAX).is_ok());
    assert_eq!(
        FiniteNumber::new(f64::NAN).expect_err("nan"),
        ScalarError::InvalidNumber
    );
    assert_eq!(
        FiniteNumber::new(f64::INFINITY).expect_err("infinity"),
        ScalarError::InvalidNumber
    );
    assert_eq!(
        FiniteNumber::new(f64::NEG_INFINITY).expect_err("negative infinity"),
        ScalarError::InvalidNumber
    );
    let err = FiniteNumber::new(f64::NAN).expect_err("nan");
    assert_input_free(&err, CANARY);
    assert!(err.source().is_none());
}

#[test]
fn paths_distinguish_browse_root_from_normalized_relatives() {
    assert_eq!(RootRelativePath::new("").expect("root").as_str(), "");
    assert_eq!(
        RootRelativePath::new("pages").expect("child").as_str(),
        "pages"
    );
    assert_eq!(
        RelativePath::new("pages/文档.md")
            .expect("unicode")
            .as_str(),
        "pages/文档.md"
    );
    assert_eq!(
        RelativePath::new("pages/my file.md")
            .expect("space")
            .as_str(),
        "pages/my file.md"
    );
    for rejected in [
        "", ".", "..", "/abs", "a/../b", "a/./b", "a//b", "a\\b", "a/b/", "./a", "a/.", "a/..",
        "a\u{0}b",
    ] {
        assert!(
            RelativePath::new(rejected).is_err(),
            "relative path accepted {rejected:?}"
        );
        if !rejected.is_empty() {
            assert!(
                RootRelativePath::new(rejected).is_err(),
                "browse path accepted {rejected:?}"
            );
        }
    }

    let root = envelope(
        r#"{"op":"browse","library_id":"syn-library","mount_id":null,"path":"","cursor":null,"limit":1}"#,
    );
    decode_request(root.as_bytes()).expect("empty browse root");
    let child = envelope(
        r#"{"op":"browse","library_id":"syn-library","mount_id":null,"path":"pages/index.md","cursor":null,"limit":1}"#,
    );
    decode_request(child.as_bytes()).expect("child browse path");
    let missing = envelope(
        r#"{"op":"edit_text","target":{"library_id":"syn-library","mount_id":null,"path":"pages/not-on-disk.md"},"expected":null,"text":"synthetic","operation_id":"syn-op"}"#,
    );
    let marker = Path::new("pages/not-on-disk.md");
    assert!(!marker.exists());
    decode_request(missing.as_bytes()).expect("missing path is data");
    assert!(!marker.exists());

    for rejected in ["", ".", "..", "/abs", "a/../b", "a\\b", "a\u{0}b"] {
        let raw = envelope(&format!(
            r#"{{"op":"edit_text","target":{{"library_id":"syn-library","mount_id":null,"path":{}}},"expected":null,"text":"synthetic","operation_id":"syn-op"}}"#,
            serde_json::to_string(rejected).expect("json string")
        ));
        let err = refuse(raw.as_bytes());
        assert_ne!(err, CodecError::FrameTooLarge);
        if rejected.contains('\u{0}') {
            assert_input_free(&err, rejected);
        }
    }
}

#[test]
fn declared_request_limits_refuse_the_adjacent_integers() {
    let browse = |limit: i64| {
        envelope(&format!(
            r#"{{"op":"browse","library_id":"syn-library","mount_id":null,"path":"","cursor":null,"limit":{limit}}}"#
        ))
    };
    decode_request(browse(1).as_bytes()).expect("browse lower bound");
    decode_request(browse(1_000).as_bytes()).expect("browse upper bound");
    assert_ne!(refuse(browse(0).as_bytes()), CodecError::FrameTooLarge);
    assert_ne!(refuse(browse(1_001).as_bytes()), CodecError::FrameTooLarge);

    let history = |limit: i64| {
        envelope(&format!(
            r#"{{"op":"history_list","library_id":"syn-library","cursor":null,"limit":{limit}}}"#
        ))
    };
    decode_request(history(1).as_bytes()).expect("history lower bound");
    decode_request(history(1_000).as_bytes()).expect("history upper bound");
    assert!(decode_request(history(0).as_bytes()).is_err());
    assert!(decode_request(history(1_001).as_bytes()).is_err());

    let length = |length: i64| {
        envelope(&format!(
            r#"{{"op":"history_diff","library_id":"syn-library","base_commit_id":"syn-base","candidate_commit_id":"syn-candidate","offset":"0","length":{length}}}"#
        ))
    };
    decode_request(length(1).as_bytes()).expect("diff lower bound");
    decode_request(length(524_288).as_bytes()).expect("diff upper bound");
    let over_length = length(524_289);
    assert!(over_length.len() < MAX_CONTROL_FRAME_BYTES);
    assert!(decode_request(over_length.as_bytes()).is_err());

    let lines = |max_lines: i64| {
        envelope(&format!(
            r#"{{"op":"read_representation","source":{{"library_id":"syn-library","source_id":"syn-source"}},"representation_digest":"{DIGEST}","start_line":1,"max_lines":{max_lines}}}"#
        ))
    };
    decode_request(lines(1).as_bytes()).expect("line lower bound");
    decode_request(lines(20_000).as_bytes()).expect("line upper bound");
    assert!(decode_request(lines(0).as_bytes()).is_err());
    assert!(decode_request(lines(20_001).as_bytes()).is_err());

    let query_limit = |limit: i64, keywords: &str, methods: &str| {
        envelope(&format!(
            r#"{{"op":"query","query":{{"library_ids":["syn-library"],"keywords":{keywords},"methods":{methods},"required_tags":[],"case_sensitive":false,"limit":{limit},"budget":{{"max_files":1,"max_bytes":"1","max_millis":1,"max_memory_bytes":"1"}},"image_query":null}}}}"#
        ))
    };
    decode_request(query_limit(1, "[\"syn\"]", "[\"bm25\"]").as_bytes()).expect("query lower");
    decode_request(query_limit(10_000, "[\"syn\"]", "[\"bm25\"]").as_bytes()).expect("query upper");
    assert!(decode_request(query_limit(0, "[\"syn\"]", "[\"bm25\"]").as_bytes()).is_err());
    assert!(decode_request(query_limit(10_001, "[\"syn\"]", "[\"bm25\"]").as_bytes()).is_err());
    assert!(decode_request(query_limit(10, "[\"syn\"]", "[]").as_bytes()).is_err());
    decode_request(query_limit(10, "[]", "[]").as_bytes()).expect("browsing query");
}

#[test]
fn schedule_and_epub_selection_bounds() {
    let schedule = |enabled: bool, minutes: i64| {
        envelope(&format!(
            r#"{{"op":"sync_schedule","library_id":"syn-library","enabled":{enabled},"interval_minutes":{minutes},"allow_push":false,"operation_id":"syn-op"}}"#
        ))
    };
    decode_request(schedule(true, 1).as_bytes()).expect("enabled one minute");
    assert!(decode_request(schedule(true, 0).as_bytes()).is_err());
    decode_request(schedule(false, 0).as_bytes()).expect("disabled retains a zero interval");
    decode_request(schedule(false, 15).as_bytes()).expect("disabled retains interval");

    let empty_pages = envelope(
        r#"{"op":"epub_plan","library_id":"syn-library","page_paths":[],"media_source_ids":[],"destination":"/synthetic/book.epub","budget":{"max_files":1,"max_bytes":"1","max_millis":1,"max_memory_bytes":"1"}}"#,
    );
    assert!(decode_request(empty_pages.as_bytes()).is_err());
    let empty_media = envelope(
        r#"{"op":"epub_plan","library_id":"syn-library","page_paths":["pages/index.md"],"media_source_ids":[],"destination":"/synthetic/book.epub","budget":{"max_files":1,"max_bytes":"1","max_millis":1,"max_memory_bytes":"1"}}"#,
    );
    decode_request(empty_media.as_bytes()).expect("empty media list");
}

#[test]
fn empty_selection_and_opaque_ids_stay_data() {
    let empty = envelope(
        r#"{"op":"hydrate","library_id":"syn-library","source_ids":[],"make_available_offline":false,"operation_id":"syn-op"}"#,
    );
    let decoded = decode_request(empty.as_bytes()).expect("explicit empty selection");
    match decoded.command {
        omni_service::Command::Hydrate { source_ids, .. } => assert!(source_ids.is_empty()),
        other => panic!("unexpected command: {other:?}"),
    }

    let odd = "../not-a-grant";
    assert_eq!(Id::new(odd).expect("opaque").as_str(), odd);
    assert!(Id::new("").is_err());
    let raw = envelope(&format!(
        r#"{{"op":"exposure_set","library_id":"{odd}","expected_revision":"syn-revision","mode":"off","sensitive":true,"excluded_paths":[],"transports":[],"operation_id":"syn-op"}}"#
    ));
    let decoded = decode_request(raw.as_bytes()).expect("owner command is data");
    match decoded.command {
        omni_service::Command::ExposureSet {
            library_id,
            transports,
            ..
        } => {
            assert_eq!(library_id.as_str(), odd);
            assert!(transports.is_empty());
        }
        other => panic!("unexpected command: {other:?}"),
    }
    let folder = Path::new("synthetic-os-folder/omni-wire-9f3c-not-created");
    assert!(!folder.exists());
    let open = envelope(
        r#"{"op":"library_open","path":"synthetic-os-folder/omni-wire-9f3c-not-created"}"#,
    );
    decode_request(open.as_bytes()).expect("path string is data");
    assert!(!folder.exists());
}

#[test]
fn scalar_diagnostics_do_not_retain_the_rejected_text() {
    let cases = [
        ScalarError::InvalidId,
        Id::new("").expect_err("empty"),
        Digest::new(format!("{CANARY}not-a-digest")).expect_err("digest"),
        RelativePath::new(format!("../{CANARY}")).expect_err("path"),
        SignedDecimal::new("+1").expect_err("plus"),
        SignedDecimal::new("-0").expect_err("negative zero"),
        Time::new(format!("2026-11-04T00:00:00+01:00{CANARY}")).expect_err("offset"),
    ];
    assert_eq!(cases[0], cases[1]);
    for err in &cases {
        assert_input_free(err, CANARY);
    }
    assert_eq!(
        Time::new("2026-11-04T00:00:00Z").expect("utc z").as_str(),
        "2026-11-04T00:00:00Z"
    );
    assert_eq!(
        Time::new("2026-11-04T00:00:00+00:00")
            .expect("utc numeric")
            .as_str(),
        "2026-11-04T00:00:00+00:00"
    );
    assert!(Digest::new("A".repeat(64)).is_err());
    assert!(Digest::new("ab".repeat(31)).is_err());
    assert!(SignedDecimal::new("01").is_err());
    assert_eq!(SignedDecimal::new("-12").expect("negative").as_str(), "-12");
}
