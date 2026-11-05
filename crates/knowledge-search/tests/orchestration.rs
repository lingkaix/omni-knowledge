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
        provider: "numeric-control".into(),
        model: "fixture".into(),
        revision: "immutable-revision-1".into(),
        dimension: 2,
        metric: knowledge_search::VectorMetric::Cosine,
        normalization: knowledge_search::VectorNormalization::Raw,
        package_fingerprint: "c".repeat(64),
        embedding_space: "fixture-space".into(),
        preprocessing: "none".into(),
    }
}

/// Complete identity is data, not native authority. Unknown fields must not silently
/// collapse two provider/revision/normalization spaces into one reusable generation.
fn full_identity_json() -> serde_json::Value {
    let mut json = serde_json::to_value(identity()).unwrap();
    json["provider"] = "numeric-control".into();
    json["revision"] = "immutable-revision-1".into();
    json["metric"] = "cosine".into();
    json["normalization"] = "raw".into();
    json
}

#[test]
fn complete_model_space_refuses_equal_dimension_provider_revision_or_normalization() {
    let root = vector_root();
    let original = full_identity_json();
    let selected: ModelIdentity = serde_json::from_value(original.clone()).unwrap();
    let rows = vec![VectorRow {
        source_id: "source".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    drop(VectorStore::build(root.path(), &selected, &rows, &grant(1024), None).unwrap());
    for (field, value) in [
        ("provider", "another-provider"),
        ("revision", "another-revision"),
        ("normalization", "l2"),
    ] {
        let mut changed = original.clone();
        changed[field] = value.into();
        let other = serde_json::from_value(changed).unwrap();
        assert!(
            VectorStore::open(root.path(), &other).is_err(),
            "ignored {field} in full model space"
        );
    }
}

#[test]
fn full_model_space_requires_every_component_and_rejects_unknown_metric() {
    for field in ["provider", "revision", "metric", "normalization"] {
        let mut incomplete = full_identity_json();
        incomplete.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<ModelIdentity>(incomplete).is_err(),
            "missing {field} accepted"
        );
    }
    let mut wrong_metric = full_identity_json();
    wrong_metric["metric"] = "unsupported-dot".into();
    assert!(serde_json::from_value::<ModelIdentity>(wrong_metric).is_err());
}

#[test]
fn declared_l2_contract_rejects_bad_rows_and_queries_without_changing_current() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "source".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    drop(VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap());
    let before = fs::read(root.path().join("CURRENT")).unwrap();
    let mut normalized = identity();
    normalized.normalization = knowledge_search::VectorNormalization::L2;
    let mut bad = rows.clone();
    bad[0].vector = vec![2.0, 0.0];
    assert!(VectorStore::build(root.path(), &normalized, &bad, &grant(1024), None).is_err());
    assert_eq!(fs::read(root.path().join("CURRENT")).unwrap(), before);
    let correct = VectorStore::build(root.path(), &normalized, &rows, &grant(1024), None).unwrap();
    let eligible = BTreeSet::from(["source".into()]);
    assert!(correct.query(&eligible, &[2.0, 0.0]).is_err());
    let correct = VectorStore::open(root.path(), &normalized).unwrap();
    assert_eq!(
        correct.query(&eligible, &[1.0, 0.0]).unwrap()[0].0,
        "source"
    );
}

#[test]
fn paged_vector_metadata_reopens_a_large_corpus_under_a_small_reader_envelope() {
    let root = vector_root();
    let rows: Vec<_> = (0..10_000)
        .map(|n| VectorRow {
            source_id: format!("source-{n:05}-{}", "x".repeat(32)),
            source_digest: "dd".repeat(32),
            locator: "line:1".into(),
            vector: vec![1.0, 0.0],
        })
        .collect();
    let selected = rows[123].source_id.clone();
    let built = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    drop(rows); // Input buffers die while their complete original funding is still retained.
    drop(built);
    let reader = AccountGrant::reserve(
        knowledge_core::jobs::Reservation {
            memory_bytes: 2 * 1024 * 1024,
            temporary_bytes: 0,
        },
        knowledge_core::admission::Origin::BeforeVerification,
    )
    .unwrap();
    let reopened = VectorStore::open_funded(root.path(), &identity(), &reader)
        .expect("corpus row metadata must be paged, not a resident JSON array");
    let eligible = BTreeSet::from([selected.clone()]);
    let result = reopened.query(&eligible, &[1.0, 0.0]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, selected);
}

#[test]
fn streaming_admission_refuses_before_advancing_the_real_producer_or_creating_a_root() {
    let root = vector_root();
    let destination = root.path().join("not-created");
    let mut called = 0;
    let producer = std::iter::from_fn(|| {
        called += 1;
        Some(Ok(VectorRow {
            source_id: "source".into(),
            source_digest: "dd".repeat(32),
            locator: "line:1".into(),
            vector: vec![1.0, 0.0],
        }))
    });
    let plan = knowledge_search::VectorBuildPlan {
        rows: 1,
        metadata_bytes: 1024,
        max_row_bytes: 1024,
    };
    assert!(
        VectorStore::build_stream(&destination, &identity(), &plan, producer, &grant(1), None)
            .is_err()
    );
    assert_eq!(called, 0);
    assert!(!destination.exists());
}

#[test]
fn incomplete_or_unfunded_row_stream_never_replaces_the_complete_pointer() {
    let root = vector_root();
    let row = VectorRow {
        source_id: "source".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    };
    drop(
        VectorStore::build(
            root.path(),
            &identity(),
            std::slice::from_ref(&row),
            &grant(1024),
            None,
        )
        .unwrap(),
    );
    let previous = fs::read(root.path().join("CURRENT")).unwrap();
    let short = knowledge_search::VectorBuildPlan {
        rows: 2,
        metadata_bytes: 2048,
        max_row_bytes: 1024,
    };
    assert!(
        VectorStore::build_stream(
            root.path(),
            &identity(),
            &short,
            [Ok(row.clone())],
            &grant(1024),
            None
        )
        .is_err()
    );
    assert_eq!(fs::read(root.path().join("CURRENT")).unwrap(), previous);
    let under = knowledge_search::VectorBuildPlan {
        rows: 1,
        metadata_bytes: 16,
        max_row_bytes: 16,
    };
    assert!(
        VectorStore::build_stream(
            root.path(),
            &identity(),
            &under,
            [Ok(row)],
            &grant(1024),
            None
        )
        .is_err()
    );
    assert_eq!(fs::read(root.path().join("CURRENT")).unwrap(), previous);
}

#[test]
fn fragment_count_never_displaces_another_eligible_file_before_limit() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let rows = vec![
        VectorRow {
            source_id: "a".into(),
            source_digest: "aa".repeat(32),
            locator: "line:1".into(),
            vector: vec![1.0, 0.0],
        },
        VectorRow {
            source_id: "a".into(),
            source_digest: "aa".repeat(32),
            locator: "line:2".into(),
            vector: vec![0.99, 0.1],
        },
        VectorRow {
            source_id: "a".into(),
            source_digest: "aa".repeat(32),
            locator: "line:3".into(),
            vector: vec![0.98, 0.2],
        },
        VectorRow {
            source_id: "b".into(),
            source_digest: "bb".repeat(32),
            locator: "line:1".into(),
            vector: vec![0.8, 0.6],
        },
    ];
    let built = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let result = built
        .query(&BTreeSet::from(["a".into(), "b".into()]), &[1.0, 0.0])
        .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, "a");
    assert_eq!(result[1].0, "b");
}

#[test]
fn distinct_fragment_evidence_streams_without_multiplying_the_file_rank() {
    let root = vector_root();
    let rows: Vec<_> = (1..=3)
        .map(|n| VectorRow {
            source_id: "a".into(),
            source_digest: "aa".repeat(32),
            locator: format!("line:{n}"),
            vector: vec![1.0, 0.0],
        })
        .collect();
    let built = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let result = built.query_filtered(&[1.0, 0.0], 1, |_| Ok(true)).unwrap();
    assert_eq!(result.len(), 1);
    let mut locations = Vec::new();
    result
        .visit_evidence(|hit| {
            locations.push(hit.locator.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(locations, ["line:1", "line:2", "line:3"]);
}

#[test]
fn actual_open_reader_keeps_its_generation_pinned_without_a_manual_marker() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "source".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    let first = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let generation = first.generation().to_owned();
    let mut next = rows.clone();
    next[0].vector = vec![0.0, 1.0];
    let second = VectorStore::build(root.path(), &identity(), &next, &grant(1024), None).unwrap();
    assert!(
        VectorStore::retire(root.path(), &generation).is_err(),
        "actual open reader was not pinned"
    );
    drop(first);
    assert!(VectorStore::retire(root.path(), &generation).is_ok());
    assert!(root.path().join(second.generation()).is_dir());
}

#[test]
fn identical_bytes_in_a_replaced_segment_are_not_the_admitted_file() {
    let root = vector_root();
    let rows = vec![VectorRow {
        source_id: "source".into(),
        source_digest: "dd".repeat(32),
        locator: "line:1".into(),
        vector: vec![1.0, 0.0],
    }];
    let built = VectorStore::build(root.path(), &identity(), &rows, &grant(1024), None).unwrap();
    let generation = built.generation().to_owned();
    drop(built);
    let admitted = VectorStore::open(root.path(), &identity()).unwrap();
    let directory = root.path().join(generation);
    let segment = directory.join("segment.bin");
    let old = directory.join("owned-original-segment.bin");
    fs::rename(&segment, &old).unwrap();
    fs::copy(&old, &segment).unwrap();
    assert!(
        admitted
            .query(&BTreeSet::from(["source".into()]), &[1.0, 0.0])
            .is_err(),
        "hashing an FD must not authorize reopening identical bytes from another inode"
    );
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
    let generation = first.generation().to_owned();
    // An opened store is now an actual FD reader, not just cached metadata.
    drop(first);
    assert!(VectorStore::retire(root.path(), &generation).is_ok());
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
    let generation = first.generation().to_owned();
    drop(first);
    let mut next = rows.clone();
    next[0].vector = vec![0.0, 1.0];
    let _second = VectorStore::build(root.path(), &identity(), &next, &grant(1024), None).unwrap();
    drop(one);
    assert!(VectorStore::retire(root.path(), &generation).is_err());
    drop(two);
    assert!(VectorStore::retire(root.path(), &generation).is_ok());
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
    let generation = first.generation().to_owned();
    drop(first); // Exercise unknown-file refusal, not the independent reader-pin refusal.
    assert!(VectorStore::retire(root.path(), &generation).is_err());
    assert_eq!(fs::read(&unknown).unwrap(), b"retain me");
}
