//! Contract tests for source preservation, general metadata, and lexical retrieval.
use omni_knowledge::{IndexOptions, KnowledgeBase, SearchMode, SearchOptions};
use std::fs;
use std::path::{Path, PathBuf};

/// Returns the current manifest path without altering the published pointer.
fn manifest_path(root: &Path) -> PathBuf {
    let generation: String =
        serde_json::from_slice(&fs::read(root.join(".omni/current.json")).unwrap()).unwrap();
    root.join(".omni/indexes")
        .join(generation)
        .join("manifest.json")
}

/// Removes new identity fields to reproduce an actual legacy marker and manifest.
fn make_legacy(root: &Path) {
    for path in [root.join("omni-knowledge.json"), manifest_path(root)] {
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("library_id");
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
}

/// Copies a disposable regular-file fixture without reinitializing its root identity.
fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            assert!(entry.file_type().unwrap().is_file());
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Serves exactly one semantic query on loopback with a fixed compatible identity and finite deadlines.
fn portable_query_service() -> (
    omni_knowledge::embedding::EmbeddingConfig,
    std::thread::JoinHandle<Vec<String>>,
) {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = omni_knowledge::embedding::EmbeddingConfig {
        endpoint: format!("http://{}", listener.local_addr().unwrap()),
        timeout_seconds: 2,
        ..Default::default()
    };
    let identity = config.clone();
    let thread = std::thread::spawn(move || {
        let mut observed = vec![];
        for expected in [
            "GET /health",
            "GET /v1/models",
            "GET /health",
            "GET /v1/models",
            "POST /v1/embeddings",
        ] {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(
                    Instant::now() < deadline,
                    "semantic request was not received"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut headers = vec![];
            let mut byte = [0_u8; 1];
            while !headers.ends_with(b"\r\n\r\n") {
                assert!(headers.len() < 8192);
                stream.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
            }
            let headers = String::from_utf8(headers).unwrap();
            let first = headers.lines().next().unwrap();
            assert_eq!(first, format!("{expected} HTTP/1.1"));
            observed.push(expected.to_owned());
            let length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(length < 65536);
            let mut input = vec![0; length];
            stream.read_exact(&mut input).unwrap();
            let reply = if expected == "GET /health" {
                serde_json::json!({"status":"ready", "backend":"mock", "package_fingerprint":identity.package_fingerprint})
            } else if expected == "GET /v1/models" {
                serde_json::json!({"data":[{"id":identity.model,"dimension":identity.dimension,"embedding_space":identity.embedding_space}]})
            } else {
                let payload: serde_json::Value = serde_json::from_slice(&input).unwrap();
                assert_eq!(payload["input"], "relocated vector question");
                let mut vector = vec![0.0; identity.dimension];
                vector[0] = 1.0;
                serde_json::json!({"model":identity.model,"data":[{"index":0,"embedding":vector}],"indexed":{"dimension":identity.dimension,"embedding_space":identity.embedding_space}})
            };
            let body = serde_json::to_vec(&reply).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
        observed
    });
    (config, thread)
}

/// Creates an isolated library for public-interface acceptance checks.
fn library() -> (tempfile::TempDir, KnowledgeBase) {
    let directory = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(directory.path(), "Test Library").unwrap();
    (directory, kb)
}

/// Proves initialization preserves existing files and refuses repeat initialization.
#[test]
fn initialization_preserves_existing_content() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("personal.txt"), "keep me").unwrap();
    KnowledgeBase::init(dir.path(), "Personal").unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("personal.txt")).unwrap(),
        "keep me"
    );
    assert!(KnowledgeBase::init(dir.path(), "Again").is_err());
    assert!(dir.path().join("wiki/sources/llm-wiki.md").exists());
    assert!(
        dir.path()
            .join("wiki/sources/open-knowledge-format.md")
            .exists()
    );
}

/// Detects exact-tag leakage and verifies arbitrary binary metadata retrieval.
#[test]
fn binary_metadata_and_exact_tags_are_searchable() {
    let (_dir, kb) = library();
    fs::write(kb.root.join("raw/firmware.unknown"), [0, 255, 1, 0]).unwrap();
    kb.set_metadata(
        "raw/firmware.unknown",
        &["hardware".into()],
        Some("device recovery image"),
    )
    .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let options = SearchOptions {
        query: "recovery".into(),
        tags: vec!["hardware".into()],
        ..Default::default()
    };
    assert_eq!(kb.search(&options).unwrap()[0].path, "raw/firmware.unknown");
    assert!(
        kb.search(&SearchOptions {
            tags: vec!["hard".into()],
            ..options
        })
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        kb.list()
            .unwrap()
            .iter()
            .find(|r| r.path.ends_with("firmware.unknown"))
            .unwrap()
            .status,
        "metadata-only"
    );
}

/// Checks Chinese BM25 tokenization and grep source lines independently.
#[test]
fn chinese_bm25_and_grep_return_source_evidence() {
    let (_dir, kb) = library();
    fs::write(
        kb.root.join("wiki/test.md"),
        "---\ntype: Note\ntags: [研究]\n---\n# 研究笔记\n苹果芯片上的知识检索实验\n",
    )
    .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let hits = kb
        .search(&SearchOptions {
            query: "知识检索".into(),
            tags: vec!["研究".into()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits[0].path, "wiki/test.md");
    let grep = kb
        .search(&SearchOptions {
            query: "知识检索".into(),
            mode: SearchMode::Grep,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(grep[0].line, Some(6));
}

/// Detects stale documents after the published index is reconciled.
#[test]
fn reconciliation_updates_and_removes_content() {
    let (_dir, kb) = library();
    let path = kb.root.join("raw/note.txt");
    fs::write(&path, "uniqueoldword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    fs::write(&path, "uniquenewword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    assert!(
        kb.search(&SearchOptions {
            query: "uniqueoldword".into(),
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        kb.search(&SearchOptions {
            query: "uniquenewword".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
    fs::remove_file(path).unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    assert!(
        kb.search(&SearchOptions {
            query: "uniquenewword".into(),
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
}

/// Proves nested root ownership and path escape rejection.
#[test]
fn nested_roots_and_path_containment() {
    let (_dir, parent) = library();
    let child = KnowledgeBase::init(&parent.root.join("child"), "Child").unwrap();
    fs::write(child.root.join("raw/child.txt"), "childonlytoken").unwrap();
    parent.index(&IndexOptions::default()).unwrap();
    child.index(&IndexOptions::default()).unwrap();
    assert!(
        parent
            .search(&SearchOptions {
                query: "childonlytoken".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        KnowledgeBase::open(&child.root.join("raw")).unwrap().root,
        child.root
    );
    assert!(parent.resolve("../private").is_err());
    assert!(parent.resolve("child/raw/child.txt").is_err());
}

/// Proves import keeps originals and does not overwrite name collisions.
#[test]
fn import_preserves_sources_and_rejects_collisions() {
    let (_dir, kb) = library();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("a.bin");
    fs::write(&source, [4, 0, 9]).unwrap();
    assert_eq!(kb.import(&source, &["asset".into()]).unwrap(), "raw/a.bin");
    assert!(kb.import(&source, &[]).is_err());
    assert_eq!(fs::read(source).unwrap(), vec![4, 0, 9]);
}

/// Verifies symlinks cannot bring files outside a library into its index.
#[test]
#[cfg(unix)]
fn symlink_sources_and_state_are_rejected() {
    let (_dir, kb) = library();
    let external = tempfile::tempdir().unwrap();
    fs::write(external.path().join("secret.txt"), "escapedsecret").unwrap();
    std::os::unix::fs::symlink(
        external.path().join("secret.txt"),
        kb.root.join("raw/link.txt"),
    )
    .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    assert!(!kb.list().unwrap().iter().any(|r| r.path == "raw/link.txt"));
    assert!(kb.resolve("raw/link.txt").is_err());
    assert!(kb.set_metadata("raw/link.txt", &[], None).is_err());
}

/// Proves short literal patterns do not lose matches to trigram candidate filtering.
#[test]
fn grep_short_patterns_and_regex_alternatives() {
    let (_dir, kb) = library();
    fs::write(kb.root.join("raw/grep.txt"), "ab\nalpha\nBETA\n").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    for query in ["ab", "alpha|beta", "^BETA$"] {
        assert!(
            !kb.search(&SearchOptions {
                query: query.into(),
                mode: SearchMode::Grep,
                ..Default::default()
            })
            .unwrap()
            .is_empty()
        );
    }
}

/// Detects stale transcriptions after a recording sidecar changes or disappears.
#[test]
fn transcript_reconciliation_tracks_external_sidecar() {
    let (_dir, kb) = library();
    fs::write(kb.root.join("raw/talk.wav"), [0, 1, 2]).unwrap();
    let transcript = kb.root.join("raw/talk.wav.transcript.json");
    fs::write(
        &transcript,
        r#"{"segments":[{"start":0,"end":1,"text":"oldspokenword"}]}"#,
    )
    .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    assert!(
        kb.search(&SearchOptions {
            query: "oldspokenword".into(),
            ..Default::default()
        })
        .unwrap()
        .iter()
        .any(|hit| hit.path == "raw/talk.wav")
    );
    fs::write(
        &transcript,
        r#"{"segments":[{"start":0,"end":1,"text":"newspokenword"}]}"#,
    )
    .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    assert!(
        !kb.search(&SearchOptions {
            query: "oldspokenword".into(),
            ..Default::default()
        })
        .unwrap()
        .iter()
        .any(|hit| hit.path == "raw/talk.wav")
    );
    assert!(
        kb.search(&SearchOptions {
            query: "newspokenword".into(),
            ..Default::default()
        })
        .unwrap()
        .iter()
        .any(|hit| hit.path == "raw/talk.wav")
    );
    fs::remove_file(transcript).unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    assert!(
        !kb.search(&SearchOptions {
            query: "newspokenword".into(),
            ..Default::default()
        })
        .unwrap()
        .iter()
        .any(|hit| hit.path == "raw/talk.wav")
    );
}

/// A provider failure must preserve the previously published lexical generation.
#[test]
fn provider_failure_preserves_published_index() {
    let (_dir, mut kb) = library();
    fs::write(kb.root.join("raw/alive.txt"), "previousgenerationword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let current = fs::read(kb.root.join(".omni/current.json")).unwrap();
    kb.config.embedding.as_mut().unwrap().endpoint = "http://127.0.0.1:1".into();
    assert!(
        kb.index(&IndexOptions {
            embeddings: true,
            force: true
        })
        .is_err()
    );
    assert_eq!(
        fs::read(kb.root.join(".omni/current.json")).unwrap(),
        current
    );
    assert_eq!(
        kb.search(&SearchOptions {
            query: "previousgenerationword".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
}

/// A concurrent mutation cannot acquire a second writer or replace an active generation.
#[test]
fn second_writer_is_rejected() {
    use fs2::FileExt;
    let (_dir, kb) = library();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(kb.root.join(".omni/write.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    assert!(kb.index(&IndexOptions::default()).is_err());
}

/// Import must reject a raw directory owned by a child root before creating any destination.
#[test]
fn import_cannot_write_into_child_owned_raw() {
    let (_dir, kb) = library();
    KnowledgeBase::init(&kb.root.join("raw"), "Child raw").unwrap();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("payload.bin");
    fs::write(&source, [0, 3, 9]).unwrap();
    assert!(kb.import(&source, &[]).is_err());
    assert!(!kb.root.join("raw/payload.bin").exists());
    assert!(source.exists());
}

/// Missing optional ASR tools cannot disable ordinary text reconciliation.
#[test]
fn missing_asr_does_not_block_lexical_index() {
    let (_dir, mut kb) = library();
    kb.config.whisper_executable = Some(kb.root.join("missing-recognizer"));
    kb.config.whisper_model = Some(kb.root.join("missing-model"));
    fs::write(kb.root.join("raw/note.txt"), "lexicalsurvivesasr").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    assert_eq!(
        kb.search(&SearchOptions {
            query: "lexicalsurvivesasr".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
}

/// A marker-named ordinary asset cannot hide existing raw sources from their parent library.
#[test]
fn import_aliases_reserved_root_marker_name() {
    let (_dir, kb) = library();
    fs::write(kb.root.join("raw/note.txt"), "muststaysearchable").unwrap();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("omni-knowledge.json");
    fs::write(&source, r#"{"ordinary":"asset"}"#).unwrap();
    assert_eq!(
        kb.import(&source, &[]).unwrap(),
        "raw/omni-knowledge.asset.json"
    );
    kb.index(&IndexOptions::default()).unwrap();
    assert_eq!(
        kb.search(&SearchOptions {
            query: "muststaysearchable".into(),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );
}

/// Speech grep and BM25 must return recording times instead of invented binary source lines.
#[test]
fn lexical_speech_hits_return_recording_times() {
    let (_dir, kb) = library();
    fs::write(kb.root.join("raw/talk.wav"), [0, 1, 2]).unwrap();
    fs::write(
        kb.root.join("raw/talk.wav.transcript.json"),
        r#"{"segments":[{"start":3,"end":8,"text":"spokenlocationword"}]}"#,
    )
    .unwrap();
    kb.set_metadata("raw/talk.wav", &["recording".into()], None)
        .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    for mode in [SearchMode::Grep, SearchMode::Bm25] {
        let hits = kb
            .search(&SearchOptions {
                query: "spokenlocationword".into(),
                mode,
                tags: vec!["recording".into()],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits[0].line, None);
        assert_eq!(hits[0].start_seconds, Some(3.0));
        assert_eq!(hits[0].end_seconds, Some(8.0));
    }
}

/// Repeated reconciliation retains only two recognized generations and preserves unexpected content.
#[test]
fn generations_are_bounded_without_deleting_unrecognized_data() {
    let (_dir, kb) = library();
    for _ in 0..4 {
        kb.index(&IndexOptions::default()).unwrap();
    }
    let count = fs::read_dir(kb.root.join(".omni/indexes")).unwrap().count();
    assert_eq!(count, 2);
    let orphan = kb.root.join(".omni/indexes/gen-000-unrecognized");
    fs::create_dir(&orphan).unwrap();
    fs::write(orphan.join("personal.txt"), "preserve me").unwrap();
    let report = kb.index(&IndexOptions::default()).unwrap();
    assert!(orphan.join("personal.txt").exists());
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("gen-000-unrecognized"))
    );
}

/// High-sorting unknown directories must not displace the actual previously published generation.
#[test]
fn retention_uses_actual_previous_pointer() {
    let (_dir, kb) = library();
    kb.index(&IndexOptions::default()).unwrap();
    let prior: String =
        serde_json::from_slice(&fs::read(kb.root.join(".omni/current.json")).unwrap()).unwrap();
    let unknown = kb.root.join(".omni/indexes/gen-z");
    fs::create_dir(&unknown).unwrap();
    fs::write(unknown.join("personal.txt"), "preserve me").unwrap();
    let report = kb.index(&IndexOptions::default()).unwrap();
    assert!(kb.root.join(".omni/indexes").join(prior).exists());
    assert!(unknown.join("personal.txt").exists());
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("gen-z"))
    );
}

/// Unexpected content inside Tantivy state must be preserved just like top-level retained content.
#[test]
fn retention_preserves_unexpected_nested_files() {
    let (_dir, kb) = library();
    kb.index(&IndexOptions::default()).unwrap();
    let prior: String =
        serde_json::from_slice(&fs::read(kb.root.join(".omni/current.json")).unwrap()).unwrap();
    let path = kb
        .root
        .join(".omni/indexes")
        .join(prior)
        .join("tantivy/personal.txt");
    fs::write(&path, "keep nested data").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let report = kb.index(&IndexOptions::default()).unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "keep nested data");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("unexpected retained files"))
    );
}

/// Cleanup must reject a symlinked inventory before reading or echoing external JSON content.
#[test]
#[cfg(unix)]
fn retirement_never_reads_symlinked_inventory() {
    let (_dir, kb) = library();
    kb.index(&IndexOptions::default()).unwrap();
    let prior: String =
        serde_json::from_slice(&fs::read(kb.root.join(".omni/current.json")).unwrap()).unwrap();
    let inventory = kb
        .root
        .join(".omni/indexes")
        .join(prior)
        .join("owned-files.json");
    fs::remove_file(&inventory).unwrap();
    let external = tempfile::tempdir().unwrap();
    let outside = external.path().join("private.json");
    fs::write(&outside, r#""EXTERNAL_PRIVATE_SENTINEL""#).unwrap();
    std::os::unix::fs::symlink(&outside, &inventory).unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let report = kb.index(&IndexOptions::default()).unwrap();
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("symlink"))
    );
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("EXTERNAL_PRIVATE_SENTINEL"))
    );
    assert!(inventory.is_symlink());
}

/// Case-sensitive literal trigram candidates must retain both Unicode and ASCII true matches.
#[test]
fn literal_trigram_candidates_are_verified_without_false_negatives() {
    let (_dir, kb) = library();
    fs::write(kb.root.join("raw/trigram.txt"), "AlphaBeta中文检索\n").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    for query in ["AlphaBeta", "中文检索"] {
        let hits = kb
            .search(&SearchOptions {
                query: query.into(),
                mode: SearchMode::Grep,
                case_sensitive: true,
                ..Default::default()
            })
            .unwrap();
        assert!(
            hits.iter()
                .any(|hit| hit.path == "raw/trigram.txt" && hit.line == Some(1))
        );
    }
    assert!(
        kb.search(&SearchOptions {
            query: "AlphaOther".into(),
            mode: SearchMode::Grep,
            case_sensitive: true,
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
}

/// Restoring authoritative files without derived index directories must remain rebuildable.
#[test]
fn missing_derived_index_directory_is_recreated() {
    let (_dir, kb) = library();
    fs::write(kb.root.join("raw/restore.txt"), "restoredsourceword").unwrap();
    kb.set_metadata("raw/restore.txt", &["preserved".into()], None)
        .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    fs::remove_dir_all(kb.root.join(".omni/indexes")).unwrap();
    let reopened = KnowledgeBase::open(&kb.root).unwrap();
    reopened.index(&IndexOptions::default()).unwrap();
    let hits = reopened
        .search(&SearchOptions {
            query: "restoredsourceword".into(),
            tags: vec!["preserved".into()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits[0].path, "raw/restore.txt");
}

/// Moving a complete library must preserve all lexical modes without creating a generation.
#[test]
fn moved_library_uses_existing_index_and_current_source_root() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("original");
    let kb = KnowledgeBase::init(&original, "Portable").unwrap();
    fs::write(kb.root.join("raw/portable.txt"), "portableevidenceword\n").unwrap();
    kb.set_metadata(
        "raw/portable.txt",
        &["travel".into()],
        Some("portable description"),
    )
    .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let pointer = fs::read(kb.root.join(".omni/current.json")).unwrap();
    let manifest = fs::read(manifest_path(&kb.root)).unwrap();
    let destination = directory.path().join("moved");
    fs::rename(&original, &destination).unwrap();
    let moved = KnowledgeBase::open(&destination).unwrap();
    assert_eq!(moved.config.library_id, kb.config.library_id);
    assert!(moved.config.library_id.is_some());
    for mode in [SearchMode::Bm25, SearchMode::Grep, SearchMode::Files] {
        let hits = moved
            .search(&SearchOptions {
                query: if matches!(mode, SearchMode::Files) {
                    "portable description"
                } else {
                    "portableevidenceword"
                }
                .into(),
                mode,
                tags: vec!["travel".into()],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].root, destination.canonicalize().unwrap());
        assert_eq!(hits[0].path, "raw/portable.txt");
        assert_eq!(
            fs::read_to_string(moved.resolve(&hits[0].path).unwrap()).unwrap(),
            "portableevidenceword\n"
        );
    }
    assert_eq!(
        fs::read(destination.join(".omni/current.json")).unwrap(),
        pointer
    );
    assert_eq!(fs::read(manifest_path(&destination)).unwrap(), manifest);
    for _ in 0..3 {
        moved.index(&IndexOptions::default()).unwrap();
    }
    assert_eq!(
        fs::read_dir(destination.join(".omni/indexes"))
            .unwrap()
            .count(),
        2
    );
}

/// A relocated semantic query uses cached source vectors and returns the new root with unchanged evidence.
#[test]
fn moved_library_preserves_cached_vector_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let original_path = directory.path().join("original");
    let kb = KnowledgeBase::init(&original_path, "Vectors").unwrap();
    fs::write(kb.root.join("raw/note.txt"), "cachedvectorword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let (config, service) = portable_query_service();
    let marker_path = kb.root.join("omni-knowledge.json");
    let mut marker: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    marker["embedding"] = serde_json::to_value(&config).unwrap();
    fs::write(marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
    let path = manifest_path(&kb.root);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut vector = vec![0.0_f32; config.dimension];
    vector[0] = 1.0;
    manifest["embedding"] = serde_json::to_value(&config).unwrap();
    manifest["vectors"] = serde_json::json!([{"path":"raw/note.txt","text":"cached source fragment","line":Some(1),"start_seconds":None::<f64>,"end_seconds":None::<f64>,"vector":vector}]);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let moved_path = directory.path().join("moved");
    fs::rename(original_path, &moved_path).unwrap();
    let moved = KnowledgeBase::open(&moved_path).unwrap();
    let pointer = fs::read(moved.root.join(".omni/current.json")).unwrap();
    let result = moved.search(&SearchOptions {
        query: "relocated vector question".into(),
        mode: SearchMode::Semantic,
        ..Default::default()
    });
    let requests = service.join().unwrap();
    let hits = result.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.starts_with("POST"))
            .count(),
        1
    );
    assert_eq!(hits[0].root, moved.root);
    assert_eq!(hits[0].path, "raw/note.txt");
    assert_eq!(hits[0].line, Some(1));
    assert_eq!(hits[0].snippet, "cached source fragment");
    assert_eq!(
        fs::read(moved.root.join(".omni/current.json")).unwrap(),
        pointer
    );
    let report = moved.index(&IndexOptions::default()).unwrap();
    assert_eq!(report.vectors, 1);
    let after: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest_path(&moved.root)).unwrap()).unwrap();
    assert_eq!(after["vectors"], manifest["vectors"]);
    assert_eq!(after["embedding"], manifest["embedding"]);
}

/// Whole-folder copies retain lineage, but another same-name initialized root rejects foreign state.
#[test]
fn copied_library_identity_is_distinct_from_an_independent_root() {
    let directory = tempfile::tempdir().unwrap();
    let original = KnowledgeBase::init(&directory.path().join("original"), "Same name").unwrap();
    fs::write(original.root.join("raw/note.txt"), "copiedlineageword").unwrap();
    original.index(&IndexOptions::default()).unwrap();
    let copied_path = directory.path().join("copy");
    copy_tree(&original.root, &copied_path);
    let copied = KnowledgeBase::open(&copied_path).unwrap();
    assert_eq!(copied.config.library_id, original.config.library_id);
    let hits = copied
        .search(&SearchOptions {
            query: "copiedlineageword".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits[0].root, copied.root);
    let independent =
        KnowledgeBase::init(&directory.path().join("independent"), "Same name").unwrap();
    assert_ne!(independent.config.library_id, original.config.library_id);
    copy_tree(
        &original.root.join(".omni"),
        &independent.root.join(".omni"),
    );
    assert!(independent.list().is_err());
    assert!(
        independent
            .search(&SearchOptions {
                query: "copiedlineageword".into(),
                ..Default::default()
            })
            .is_err()
    );
}

/// Opening or reading a legacy root must preserve its marker and require its recorded path.
#[test]
fn legacy_open_is_read_only_until_explicit_index_upgrade() {
    let directory = tempfile::tempdir().unwrap();
    let original_path = directory.path().join("legacy");
    let kb = KnowledgeBase::init(&original_path, "Legacy").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    make_legacy(&kb.root);
    let marker = fs::read(kb.root.join("omni-knowledge.json")).unwrap();
    let legacy = KnowledgeBase::open(&kb.root).unwrap();
    assert!(legacy.config.library_id.is_none());
    legacy.list().unwrap();
    legacy.status().unwrap();
    assert_eq!(
        fs::read(kb.root.join("omni-knowledge.json")).unwrap(),
        marker
    );
    let moved_path = directory.path().join("moved-legacy");
    fs::rename(original_path, &moved_path).unwrap();
    assert!(KnowledgeBase::open(&moved_path).unwrap().list().is_err());
    assert_eq!(
        fs::read(moved_path.join("omni-knowledge.json")).unwrap(),
        marker
    );
}

/// An explicit upgrade preserves unknown marker fields and works through a previously opened handle.
#[test]
fn legacy_index_upgrade_assigns_portable_identity_without_losing_settings() {
    let directory = tempfile::tempdir().unwrap();
    let original_path = directory.path().join("legacy");
    let kb = KnowledgeBase::init(&original_path, "Legacy").unwrap();
    fs::write(kb.root.join("raw/note.txt"), "upgradedportableword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    make_legacy(&kb.root);
    let marker_path = kb.root.join("omni-knowledge.json");
    let mut marker: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    marker["custom_field"] = serde_json::json!({"preserve": true});
    marker["whisper_model"] = serde_json::json!("/other-machine/model.bin");
    fs::write(&marker_path, serde_json::to_vec_pretty(&marker).unwrap()).unwrap();
    let legacy = KnowledgeBase::open(&kb.root).unwrap();
    legacy.index(&IndexOptions::default()).unwrap();
    assert!(legacy.config.library_id.is_none());
    legacy.list().unwrap();
    legacy.status().unwrap();
    let upgraded: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
    assert!(
        uuid::Uuid::parse_str(upgraded["library_id"].as_str().unwrap())
            .unwrap()
            .get_version_num()
            == 4
    );
    assert_eq!(upgraded["custom_field"], marker["custom_field"]);
    assert_eq!(upgraded["whisper_model"], marker["whisper_model"]);
    let pointer = fs::read(kb.root.join(".omni/current.json")).unwrap();
    let moved_path = directory.path().join("moved");
    fs::rename(original_path, &moved_path).unwrap();
    let moved = KnowledgeBase::open(&moved_path).unwrap();
    assert_eq!(
        moved
            .search(&SearchOptions {
                query: "upgradedportableword".into(),
                ..Default::default()
            })
            .unwrap()[0]
            .root,
        moved.root
    );
    assert_eq!(
        fs::read(moved.root.join(".omni/current.json")).unwrap(),
        pointer
    );
}

/// A failed legacy build can assign an identity without invalidating the last good path-bound snapshot.
#[test]
fn failed_legacy_upgrade_keeps_previous_generation_readable() {
    use sha2::{Digest, Sha256};
    let (_directory, kb) = library();
    fs::write(kb.root.join("raw/good.txt"), "lastgoodlegacyword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    make_legacy(&kb.root);
    let legacy = KnowledgeBase::open(&kb.root).unwrap();
    let pointer = fs::read(kb.root.join(".omni/current.json")).unwrap();
    fs::write(kb.root.join("raw/bad.txt"), "bad metadata").unwrap();
    legacy.set_metadata("raw/bad.txt", &[], None).unwrap();
    let metadata = kb.root.join(".omni/metadata").join(format!(
        "{}.json",
        hex::encode(Sha256::digest(b"raw/bad.txt"))
    ));
    fs::write(metadata, r#"{"version":2,"path":"raw/bad.txt"}"#).unwrap();
    assert!(legacy.index(&IndexOptions::default()).is_err());
    assert!(
        KnowledgeBase::open(&kb.root)
            .unwrap()
            .config
            .library_id
            .is_some()
    );
    assert_eq!(
        fs::read(kb.root.join(".omni/current.json")).unwrap(),
        pointer
    );
    assert_eq!(
        legacy
            .search(&SearchOptions {
                query: "lastgoodlegacyword".into(),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
}

/// Malformed or changed marker identities must be checked even by an already open handle.
#[test]
fn current_marker_identity_controls_snapshot_ownership() {
    let (_directory, kb) = library();
    kb.index(&IndexOptions::default()).unwrap();
    let marker = kb.root.join("omni-knowledge.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    value["library_id"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
    fs::write(&marker, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(kb.list().is_err());
    value["library_id"] = serde_json::json!("Same display name");
    fs::write(&marker, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(KnowledgeBase::open(&kb.root).is_err());
    assert!(kb.list().is_err());
}

/// Any marker entry isolates a subtree even when its target is absent and sources are deeper descendants.
#[cfg(unix)]
#[test]
fn dangling_child_marker_blocks_parent_index_and_deeper_path_management() {
    let (_directory, parent) = library();
    let child = parent.root.join("marked-child");
    fs::create_dir_all(child.join("grandchild")).unwrap();
    fs::write(child.join("grandchild/note.txt"), "isolatedchildtoken").unwrap();
    std::os::unix::fs::symlink(
        parent.root.join("missing-child-configuration"),
        child.join("omni-knowledge.json"),
    )
    .unwrap();
    assert!(KnowledgeBase::open(&child).is_err());
    assert!(KnowledgeBase::open(&child.join("grandchild")).is_err());
    assert!(KnowledgeBase::open(&child.join("grandchild/note.txt")).is_err());
    assert!(parent.resolve("marked-child/grandchild/note.txt").is_err());
    assert!(
        parent
            .set_metadata("marked-child/grandchild/note.txt", &[], None)
            .is_err()
    );
    parent.index(&IndexOptions::default()).unwrap();
    assert!(
        !parent
            .list()
            .unwrap()
            .iter()
            .any(|record| record.path.starts_with("marked-child/"))
    );
    assert!(
        parent
            .search(&SearchOptions {
                query: "isolatedchildtoken".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fs::read_to_string(child.join("grandchild/note.txt")).unwrap(),
        "isolatedchildtoken"
    );
}
