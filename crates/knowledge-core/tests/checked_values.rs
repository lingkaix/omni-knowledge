//! Checked path, digest, range, and budget values on the public data seam.
//!
//! Storage and the rendered API define these lexical checks. Constructing a value
//! does not admit a root, hash bytes, or reserve memory. The trait test is API-only:
//! its stand-in is not confinement, residency, read, tracking, offline, or tombstone evidence.

use knowledge_core::backend::{
    Availability, BackendError, BackendIdentity, Budget, ByteRange, Capabilities, Cursor,
    Enumeration, ExpectedVersion, FileIdEvidence, Freshness, MountBackend, Progress, RangeRead,
    Residency, StatObservation, StatSnapshot, Unavailable,
};
use knowledge_core::data::{BackendKind, ContentDigest, DataError, RelativePath};
use std::error::Error;

const CANARY: &str = "SENTINEL_9f3c1a7b";
const CLAIMED_DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn assert_input_free(error: &impl Error, canary: &str) {
    assert!(
        error.source().is_none(),
        "data errors expose no source chain"
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

/// A relative spelling is kept verbatim. The constructor does not touch a filesystem.
#[test]
fn relative_path_keeps_accepted_spellings() {
    for spelling in [
        "pages",
        "pages/index.md",
        "secrets/a.md",
        "a b.md",
        "pages/索引.md",
    ] {
        let path =
            RelativePath::new(spelling).unwrap_or_else(|error| panic!("{spelling}: {error}"));
        assert_eq!(path.as_str(), spelling);
    }
}

/// Absolute, drive, empty, dot, traversal, NUL, and backslash spellings are refused.
#[test]
fn relative_path_rejects_the_documented_spellings() {
    let rejected = [
        "",
        ".",
        "..",
        "./pages",
        "pages/.",
        "pages/..",
        "../pages",
        "pages/../index.md",
        "/tmp/x",
        "C:/windows",
        "c:foo",
        "D:bar",
        "foo\\bar",
        "a\0b",
        &format!("{CANARY}/../hidden"),
    ];
    for spelling in rejected {
        let error = RelativePath::new(spelling).expect_err(spelling);
        assert_eq!(error, DataError::InvalidPath);
        assert_input_free(&error, CANARY);
        if spelling.len() >= 8 {
            assert!(
                !error.to_string().contains(spelling),
                "diagnostic echoed {spelling}"
            );
        }
    }
}

/// A claimed digest is exactly 64 lowercase hex characters and is not a computed hash.
#[test]
fn content_digest_checks_lowercase_sha256_spelling() {
    let digest = ContentDigest::new(CLAIMED_DIGEST).expect("lowercase digest");
    assert_eq!(digest.as_str(), CLAIMED_DIGEST);
    let unrelated = b"canary-bytes-that-this-constructor-must-not-hash";
    assert_ne!(digest.as_str().as_bytes(), unrelated);

    let rejected = [
        CLAIMED_DIGEST.to_ascii_uppercase(),
        format!("{CLAIMED_DIGEST}a"),
        CLAIMED_DIGEST[..63].to_string(),
        format!("gggg{CANARY}"),
        String::new(),
        CANARY.to_string(),
    ];
    for spelling in &rejected {
        let error = ContentDigest::new(spelling).expect_err(spelling);
        assert_eq!(error, DataError::InvalidDigest);
        assert_input_free(&error, CANARY);
        if !spelling.is_empty() {
            assert!(
                !error.to_string().contains(spelling),
                "diagnostic echoed {spelling}"
            );
        }
    }
}

/// A half-open range rejects zero length and an end that does not fit in `u64`.
#[test]
fn byte_range_rejects_empty_and_overflowing_ends() {
    let small = ByteRange::new(0, 1).expect("one byte");
    assert_eq!((small.offset(), small.length()), (0, 1));
    let shifted = ByteRange::new(10, 20).expect("shifted");
    assert_eq!((shifted.offset(), shifted.length()), (10, 20));
    let tail = ByteRange::new(u64::MAX - 4, 4).expect("tail");
    assert_eq!((tail.offset(), tail.length()), (u64::MAX - 4, 4));
    let wide = ByteRange::new(0, u64::MAX).expect("half-open end at u64::MAX");
    assert_eq!((wide.offset(), wide.length()), (0, u64::MAX));

    for (offset, length) in [
        (0, 0),
        (4, 0),
        (u64::MAX, 1),
        (u64::MAX - 4, 5),
        (5, u64::MAX),
    ] {
        let error = ByteRange::new(offset, length).expect_err("range");
        assert_eq!(error, DataError::InvalidRange);
        assert_input_free(&error, CANARY);
    }
}

/// Zero budget fields stay zero. They mean no corresponding work, not unlimited work.
/// This stores the public value. It does not prove that an adapter enforces the ceiling.
#[test]
fn budget_zero_remains_a_finite_stored_ceiling() {
    let zero = Budget {
        max_files: 0,
        max_bytes: 0,
        max_millis: 0,
        max_memory_bytes: 0,
    };
    assert_eq!(
        (
            zero.max_files,
            zero.max_bytes,
            zero.max_millis,
            zero.max_memory_bytes
        ),
        (0, 0, 0, 0)
    );
    let finite = Budget {
        max_files: 2,
        max_bytes: 9,
        max_millis: 30,
        max_memory_bytes: 100,
    };
    assert_eq!(
        (
            finite.max_files,
            finite.max_bytes,
            finite.max_millis,
            finite.max_memory_bytes
        ),
        (2, 9, 30, 100)
    );
}

/// Backend diagnostics stay fixed and input-free. Variant identity keeps the classification.
#[test]
fn backend_diagnostics_are_fixed_and_do_not_carry_a_canary() {
    let offline = BackendError::Unavailable(Unavailable::Offline);
    let deleted = BackendError::Unavailable(Unavailable::Deleted);
    assert_ne!(offline, deleted);
    assert_eq!(offline.to_string(), offline.to_string());
    assert_input_free(&offline, CANARY);
    assert_input_free(&BackendError::BudgetExhausted, CANARY);
    assert_input_free(&BackendError::InvalidRequest, CANARY);
    assert_input_free(&BackendError::Io, CANARY);
    assert!(offline.source().is_none());
}

/// API-only: the public trait is object-safe and accepts checked values.
/// The local stand-in is not invoked as evidence of confinement or any runtime effect.
#[test]
fn mount_backend_trait_is_callable_api_only() {
    let path = RelativePath::new("pages/index.md").expect("path");
    assert_eq!(path.as_str(), "pages/index.md");
    let digest = ContentDigest::new(CLAIMED_DIGEST).expect("digest");
    assert_eq!(digest.as_str(), CLAIMED_DIGEST);
    let range = ByteRange::new(0, 4).expect("range");
    assert_eq!((range.offset(), range.length()), (0, 4));
    let budget = Budget {
        max_files: 1,
        max_bytes: 4,
        max_millis: 5,
        max_memory_bytes: 64,
    };
    let identity = BackendIdentity {
        library_id: "lib-1".to_string(),
        mount_id: Some("mount-1".to_string()),
        backend: BackendKind::Local,
    };
    let expected = ExpectedVersion {
        digest,
        snapshot: StatSnapshot {
            backend: BackendIdentity {
                library_id: identity.library_id.clone(),
                mount_id: identity.mount_id.clone(),
                backend: identity.backend,
            },
            path: RelativePath::new("pages/index.md").expect("snapshot path"),
            size_bytes: 4,
            modified_ns: Some(-1),
            changed_ns: None,
            file_id: Some(FileIdEvidence {
                backend: identity,
                file_id: "file-1".to_string(),
                volume_id: None,
                generation: None,
                birth_ns: None,
            }),
            backend_version: None,
            residency: Residency::Unknown,
            freshness: Freshness::Unknown,
        },
    };
    let backend: &dyn MountBackend = &ApiOnlyStandIn;
    let _capabilities: Capabilities = backend.capabilities();
    let _listed: Result<Enumeration, BackendError> = backend.list(None, budget);
    let _stat: Result<StatObservation, BackendError> = backend.stat(&path, budget);
    let _read: Result<RangeRead, BackendError> = backend.open_range(&expected, range, budget);
}

struct ApiOnlyStandIn;

impl MountBackend for ApiOnlyStandIn {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            range_read: false,
            stable_file_id: false,
            reliable_watch: false,
            conditional_write: false,
            residency: false,
        }
    }

    fn list(&self, _cursor: Option<&Cursor>, _budget: Budget) -> Result<Enumeration, BackendError> {
        Ok(Enumeration::Complete {
            entries: Vec::new(),
            progress: Progress {
                examined: 0,
                bytes_read: 0,
                elapsed_millis: 0,
            },
        })
    }

    fn stat(&self, _path: &RelativePath, _budget: Budget) -> Result<StatObservation, BackendError> {
        Ok(StatObservation {
            availability: Availability::Unavailable(Unavailable::Unsupported),
            snapshot: None,
        })
    }

    fn open_range(
        &self,
        _expected_version: &ExpectedVersion,
        _range: ByteRange,
        _budget: Budget,
    ) -> Result<RangeRead, BackendError> {
        Err(BackendError::Unavailable(Unavailable::Unsupported))
    }
}
