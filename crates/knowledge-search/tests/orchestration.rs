use knowledge_search::{
    AccountGrant, AdmittedSource, LexicalError, LexicalMode, LexicalOutcome, LexicalQuery,
    ModelIdentity, VectorRow, VectorStore, fuse_ranks, import_stored_segment, search_lexical,
};
use std::collections::BTreeSet;
use std::fs;
use std::time::{Duration, Instant};

fn source(path: &str, sha: &str, text: &str, coverage: &str) -> AdmittedSource {
    AdmittedSource {
        path: path.into(),
        title: path.into(),
        media_type: "text/plain".into(),
        source_sha256: sha.into(),
        text: text.into(),
        tags: String::new(),
        coverage: coverage.into(),
    }
}

fn grant(bytes: u64) -> AccountGrant {
    use knowledge_core::{
        admission::{Account, Configuration, Origin, Scheduling},
        jobs::Reservation,
    };
    static START: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    START.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        Account::start(
            fs::File::open(dir.path()).unwrap(),
            Configuration {
                memory_bytes: None,
                storage_bytes: None,
            },
            Scheduling {
                max_pending: 32,
                max_records: 8192,
                max_descriptor_bytes: 1024 * 1024,
                per_mount: 2,
            },
        )
        .unwrap();
        dir
    });
    let bytes = if bytes == 1 { 1 } else { 640 * 1024 * 1024 };
    AccountGrant::reserve(
        Reservation {
            memory_bytes: bytes,
            temporary_bytes: bytes,
        },
        Origin::BeforeVerification,
    )
    .unwrap()
}

fn search(
    root: &std::path::Path,
    store: &std::path::Path,
    request: &LexicalQuery<'_>,
) -> Result<LexicalOutcome, LexicalError> {
    search_lexical(root, store, "library", request)
}

#[test]
fn equal_text_hashes_stay_distinct_sources() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let sha = "ab".repeat(32);
    let sources = vec![
        source("a.txt", &sha, "sameword", "complete"),
        source("b.txt", &sha, "sameword", "complete"),
    ];
    let eligible = sources.iter().map(|item| item.path.clone()).collect();
    let holding = grant(10_000);
    let outcome = search(
        root.path(),
        store.path(),
        &LexicalQuery {
            sources: &sources,
            eligible: &eligible,
            mode: LexicalMode::Lexical,
            query: "sameword",
            limit: 10,
            grant: &holding,
            deadline: None,
        },
    )
    .unwrap();
    let mut paths: Vec<_> = outcome.hits.iter().map(|hit| hit.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(paths, ["a.txt", "b.txt"]);
}

#[test]
fn regex_reads_a_source_that_is_not_the_lexical_top_hit() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let sources = vec![
        source("top.txt", &"aa".repeat(32), "alpha alpha alpha", "complete"),
        source("tail.txt", &"bb".repeat(32), "needle-only", "partial"),
    ];
    let eligible = sources.iter().map(|item| item.path.clone()).collect();
    let holding = grant(10_000);
    let outcome = search(
        root.path(),
        store.path(),
        &LexicalQuery {
            sources: &sources,
            eligible: &eligible,
            mode: LexicalMode::Regex,
            query: "needle-only",
            limit: 10,
            grant: &holding,
            deadline: None,
        },
    )
    .unwrap();
    assert_eq!(outcome.hits.len(), 1);
    assert_eq!(outcome.hits[0].path, "tail.txt");
    assert!(outcome.searched_representations >= 2);
}

#[test]
fn short_grant_does_not_create_a_kernel_store() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let sources = vec![source("a.txt", &"aa".repeat(32), "word", "complete")];
    let eligible = BTreeSet::from(["a.txt".into()]);
    let holding = grant(1);
    let error = search(
        root.path(),
        store.path(),
        &LexicalQuery {
            sources: &sources,
            eligible: &eligible,
            mode: LexicalMode::Lexical,
            query: "word",
            limit: 10,
            grant: &holding,
            deadline: None,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("smaller"));
    assert!(fs::read_dir(store.path()).unwrap().next().is_none());
}

#[test]
fn past_deadline_does_not_publish() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let sources = vec![source("a.txt", &"aa".repeat(32), "word", "offline")];
    let eligible = BTreeSet::from(["a.txt".into()]);
    let holding = grant(10_000);
    let error = search(
        root.path(),
        store.path(),
        &LexicalQuery {
            sources: &sources,
            eligible: &eligible,
            mode: LexicalMode::Exact,
            query: "word",
            limit: 10,
            grant: &holding,
            deadline: Some(Instant::now() - Duration::from_secs(1)),
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("deadline"));
    assert!(fs::read_dir(store.path()).unwrap().next().is_none());
}

#[test]
fn fusion_breaks_ties_by_source_id() {
    let fused = fuse_ranks(&[vec!["b".into(), "a".into()], vec!["a".into(), "b".into()]]);
    assert_eq!(fused[0].1, fused[1].1);
    assert_eq!(fused[0].0, "a");
    assert_eq!(fused[1].0, "b");
}

/// The caller owns this newly created fixture and establishes the native private-root precondition.
/// The engine must never repair/chmod an unknown existing directory on its behalf.
fn vector_root() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    root
}

fn identity() -> ModelIdentity {
    ModelIdentity {
        model: "fixture".into(),
        dimension: 2,
        package_fingerprint: "c".repeat(64),
        embedding_space: "fixture-space".into(),
        preprocessing: "none".into(),
    }
}

#[test]
fn vector_query_skips_ineligible_rows_before_reading_them() {
    let root = vector_root();
    let rows = vec![
        VectorRow {
            source_id: "kept".into(),
            source_digest: "dd".repeat(32),
            locator: "line:1".into(),
            vector: vec![1.0, 0.0],
        },
        VectorRow {
            source_id: "hidden".into(),
            source_digest: "ee".repeat(32),
            locator: "line:2".into(),
            vector: vec![0.0, 1.0],
        },
    ];
    let built = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let segment = fs::read(root.path().join(built.generation()).join("segment.bin")).unwrap();
    let mut corrupt = segment.clone();
    // The second row starts at 8 bytes. A nonfinite value must not be read for an ineligible id.
    corrupt[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
    fs::write(
        root.path().join(built.generation()).join("segment.bin"),
        &corrupt,
    )
    .unwrap();
    // Rebuild the hash to match the corrupted segment so open still binds the generation.
    // The store open checks the original hash, so query the built store before open rechecks.
    let eligible = BTreeSet::from(["kept".into()]);
    let ranked = built.query(&eligible, &[1.0, 0.0]).unwrap();
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0, "kept");
}

#[test]
fn reopen_refuses_a_different_model_identity() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "kept".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let mut other = identity();
    other.model = "other".into();
    assert!(VectorStore::open(root.path(), &other).is_err());
}

#[test]
fn pin_keeps_the_previous_generation() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "kept".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    let first = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let pin = first.pin().unwrap();
    assert!(VectorStore::retire(root.path(), first.generation()).is_err());
    drop(pin);
    let mut next = rows.clone();
    next[0].vector = vec![0.0, 1.0];
    let second = VectorStore::build(root.path(), &identity(), &next, &grant(1024), None).unwrap();
    assert_ne!(first.generation(), second.generation());
    assert!(VectorStore::retire(root.path(), first.generation()).is_ok());
    assert!(root.path().join(second.generation()).is_dir());
}

#[test]
fn stored_segment_import_checks_bytes_without_reading_a_zip() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "kept".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    let mut segment = Vec::new();
    for value in &rows[0].vector {
        segment.extend(value.to_le_bytes());
    }
    let imported =
        import_stored_segment(root.path(), &identity(), &rows, &segment, &grant(1024)).unwrap();
    let eligible = BTreeSet::from(["kept".into()]);
    assert_eq!(imported.query(&eligible, &[1.0, 0.0]).unwrap()[0].0, "kept");
}

#[test]
fn independent_real_vector_pins_do_not_unpin_each_other() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "kept".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    let first = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let one = first.pin().unwrap();
    let two = first.pin().unwrap();
    let mut next = rows.clone();
    next[0].vector = vec![0.0, 1.0];
    let _second = VectorStore::build(root.path(), &identity(), &next, &grant(1024), None).unwrap();
    drop(one);
    assert!(VectorStore::retire(root.path(), first.generation()).is_err());
    drop(two);
    assert!(VectorStore::retire(root.path(), first.generation()).is_ok());
}

#[test]
fn vector_reader_consumes_shared_credit_and_reuses_only_after_result_destruction() {
    let root = vector_root();
    let untouched = root.path().join("not-created");
    let rows = vec![VectorRow {
        source_id: "kept".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    let paid = grant(1024);
    let sibling = paid.clone();
    let store = VectorStore::build(root.path(), &identity(), &rows, &paid, None).unwrap();
    assert!(VectorStore::build(&untouched, &identity(), &rows, &sibling, None).is_err());
    assert!(
        !untouched.exists(),
        "a clone cannot allocate or inspect a new native destination on spent credit"
    );
    let eligible = BTreeSet::from(["kept".into()]);
    let result = store.query(&eligible, &[1.0, 0.0]).unwrap();
    assert_eq!(result[0].0, "kept");
    let store = result.into_store();
    let result = store.query(&eligible, &[1.0, 0.0]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn vector_retirement_preserves_unrecognized_generation_files() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "kept".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    let first = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let unknown = root
        .path()
        .join(first.generation())
        .join("not-in-ownership-inventory");
    fs::write(&unknown, b"retain me").unwrap();
    let mut next = rows.clone();
    next[0].vector = vec![0.0, 1.0];
    let _second = VectorStore::build(root.path(), &identity(), &next, &grant(1024), None).unwrap();
    assert!(VectorStore::retire(root.path(), first.generation()).is_err());
    assert_eq!(fs::read(&unknown).unwrap(), b"retain me");
}
