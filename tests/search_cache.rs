//! Contract regressions for resident searches, publication leases, and live provider compatibility.

use omni_knowledge as fixture_library;
#[path = "support/retained.rs"]
mod retained;
use retained::legacy_init;

use omni_knowledge::{IndexOptions, KnowledgeBase, SearchMode, SearchOptions};
use std::fs::{self, FileTimes, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Create a retained v1 indexed source with unchanged metadata/publication component oracles.
fn library() -> (tempfile::TempDir, KnowledgeBase) {
    let directory = tempfile::tempdir().unwrap();
    let kb = legacy_init(directory.path(), "Resident search fixture").unwrap();
    fs::write(kb.root.join("raw/note.txt"), "cachewarmword\n").unwrap();
    kb.set_metadata("raw/note.txt", &["research".into()], Some("Authored note"))
        .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    (directory, kb)
}

/// Read the selected generation's manifest path without changing any durable state.
fn manifest_path(root: &Path) -> PathBuf {
    let generation: String =
        serde_json::from_slice(&fs::read(root.join(".omni/current.json")).unwrap()).unwrap();
    root.join(".omni/indexes")
        .join(generation)
        .join("manifest.json")
}

/// Query exactly the fixture source through the requested strategy, requiring its complete tag.
fn query(mode: SearchMode) -> SearchOptions {
    SearchOptions {
        query: "cachewarmword".into(),
        mode,
        tags: vec!["research".into()],
        ..Default::default()
    }
}

/// Warm every local strategy, then require each search to observe the newly published generation.
#[test]
fn repeated_searches_follow_publication_and_exact_tags() {
    let (_directory, kb) = library();
    for mode in [SearchMode::Files, SearchMode::Grep, SearchMode::Bm25] {
        let mut options = query(mode);
        if mode == SearchMode::Files {
            options.query = "note.txt".into();
        }
        assert_eq!(kb.search(&options).unwrap().len(), 1);
        options.tags = vec!["Research".into()];
        assert_eq!(kb.search(&options).unwrap().len(), 1);
        options.case_sensitive = true;
        assert!(kb.search(&options).unwrap().is_empty());
    }
    fs::write(kb.root.join("raw/note.txt"), "publishedreplacementword\n").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    for mode in [SearchMode::Grep, SearchMode::Bm25] {
        assert!(kb.search(&query(mode)).unwrap().is_empty());
        let options = SearchOptions {
            query: "publishedreplacementword".into(),
            ..query(mode)
        };
        assert_eq!(kb.search(&options).unwrap().len(), 1);
    }
}

/// Cached facts cannot hide an in-place path corruption, even after restoring length and modification time.
#[cfg(unix)]
#[test]
fn warm_search_revalidates_equal_size_manifest_changes() {
    use std::os::unix::fs::MetadataExt;
    let (_directory, kb) = library();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let path = manifest_path(&kb.root);
    let before = fs::metadata(&path).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let corrupted = original.replace("\"path\": \"raw/note.txt\"", "\"path\": \".omni/xx.txt\"");
    assert_ne!(original, corrupted);
    assert_eq!(original.len(), corrupted.len());
    fs::write(&path, corrupted).unwrap();
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    let after = fs::metadata(&path).unwrap();
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert!(
        kb.search(&query(SearchMode::Grep))
            .unwrap_err()
            .to_string()
            .contains("internal source")
    );
}

/// Replacing a warm manifest or marker with corrupt, foreign, or symbolic-link state never returns stale hits.
#[cfg(unix)]
#[test]
fn warm_search_refuses_replaced_manifest_and_marker() {
    let (_directory, kb) = library();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let path = manifest_path(&kb.root);
    let original = fs::read(&path).unwrap();
    let replacement = path.with_extension("replacement");
    fs::write(&replacement, b"{malformed}").unwrap();
    fs::rename(&replacement, &path).unwrap();
    assert!(kb.search(&query(SearchMode::Grep)).is_err());
    fs::write(&path, &original).unwrap();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), &original).unwrap();
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(outside.path(), &path).unwrap();
    assert!(kb.search(&query(SearchMode::Grep)).is_err());
    fs::remove_file(&path).unwrap();
    fs::write(&path, original).unwrap();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let marker = kb.root.join("omni-knowledge.json");
    let original_marker = fs::read(&marker).unwrap();
    let mut config: serde_json::Value = serde_json::from_slice(&original_marker).unwrap();
    config["library_id"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
    fs::write(&marker, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(kb.search(&query(SearchMode::Grep)).is_err());
    fs::write(&marker, &original_marker).unwrap();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let marker_backup = marker.with_extension("backup");
    fs::rename(&marker, &marker_backup).unwrap();
    std::os::unix::fs::symlink(&marker_backup, &marker).unwrap();
    assert!(kb.search(&query(SearchMode::Grep)).is_err());
}

/// A warm cache retains no old publication lock, and moving the root changes returned source identities.
#[test]
fn idle_resident_snapshot_does_not_block_publication_or_relocation() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("original");
    let kb = legacy_init(&source, "Portable resident search").unwrap();
    fs::write(kb.root.join("raw/note.txt"), "cachewarmword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let options = SearchOptions {
        query: "cachewarmword".into(),
        mode: SearchMode::Grep,
        ..Default::default()
    };
    assert_eq!(kb.search(&options).unwrap().len(), 1);
    let reopened = KnowledgeBase::open(&source).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        sender
            .send(reopened.index(&IndexOptions::default()).map(|_| ()))
            .unwrap();
    });
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("idle cache retained a reader lease")
        .unwrap();
    writer.join().unwrap();
    let destination = directory.path().join("moved");
    fs::rename(&source, &destination).unwrap();
    assert!(kb.search(&options).is_err());
    let moved = KnowledgeBase::open(&destination).unwrap();
    let hits = moved.search(&options).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].root, moved.root);
}

/// A symbolic-link replacement of an already opened canonical root must not reuse cached ownership.
#[cfg(unix)]
#[test]
fn warm_search_refuses_root_location_replaced_by_symlink() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("original");
    let kb = legacy_init(&source, "Original root").unwrap();
    fs::write(kb.root.join("raw/note.txt"), "cachewarmword").unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let options = SearchOptions {
        tags: vec![],
        ..query(SearchMode::Grep)
    };
    assert_eq!(kb.search(&options).unwrap().len(), 1);
    let destination = directory.path().join("moved");
    fs::rename(&source, &destination).unwrap();
    std::os::unix::fs::symlink(&destination, &source).unwrap();
    assert!(kb.search(&options).is_err());
    assert_eq!(
        KnowledgeBase::open(&source)
            .unwrap()
            .search(&options)
            .unwrap()[0]
            .root,
        fs::canonicalize(destination).unwrap()
    );
}

/// Serve one validated embedding query, optionally followed by a live incompatible health reply.
/// Every connection and expected request has a finite deadline, and observed content transmissions are returned.
fn query_service(
    incompatible_after_first: bool,
) -> (
    omni_knowledge::embedding::EmbeddingConfig,
    std::thread::JoinHandle<Vec<String>>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = omni_knowledge::embedding::EmbeddingConfig {
        endpoint: format!("http://{}", listener.local_addr().unwrap()),
        dimension: 2,
        timeout_seconds: 2,
        ..Default::default()
    };
    let identity = config.clone();
    let worker = std::thread::spawn(move || {
        let mut requests = vec![];
        let mut expected = vec!["GET /health", "GET /v1/models", "POST /v1/embeddings"];
        if incompatible_after_first {
            expected.push("GET /health");
        }
        for (index, request) in expected.into_iter().enumerate() {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(
                    Instant::now() < deadline,
                    "semantic query did not perform fresh preflight"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            stream.set_nonblocking(false).unwrap();
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
            assert_eq!(
                headers.lines().next().unwrap(),
                format!("{request} HTTP/1.1")
            );
            requests.push(request.to_owned());
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(length < 65536);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let reply = match request {
                "GET /health" => {
                    serde_json::json!({"status":"ready", "backend":"mock", "package_fingerprint":if index == 3 { "incompatible" } else { &identity.package_fingerprint }})
                }
                "GET /v1/models" => {
                    serde_json::json!({"data":[{"id":identity.model,"dimension":identity.dimension,"embedding_space":identity.embedding_space}]})
                }
                _ => {
                    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(payload["input"], "semantic fixture query");
                    serde_json::json!({"model":identity.model,"data":[{"index":0,"embedding":[1.0,0.0]}],"indexed":{"dimension":identity.dimension,"embedding_space":identity.embedding_space}})
                }
            };
            let body = serde_json::to_vec(&reply).unwrap();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
        requests
    });
    (config, worker)
}

/// Attach deterministic finite vectors and a compatible loopback identity without invoking a model during indexing.
fn install_vector_fixture(kb: &KnowledgeBase, config: &omni_knowledge::embedding::EmbeddingConfig) {
    let marker = kb.root.join("omni-knowledge.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    value["embedding"] = serde_json::to_value(config).unwrap();
    fs::write(marker, serde_json::to_vec(&value).unwrap()).unwrap();
    let manifest = manifest_path(&kb.root);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["embedding"] = serde_json::to_value(config).unwrap();
    value["vectors"] = serde_json::json!([{"path":"raw/note.txt","text":"Cached finite evidence","line":1,"start_seconds":null,"end_seconds":null,"vector":[1.0,0.0]}]);
    fs::write(manifest, serde_json::to_vec(&value).unwrap()).unwrap();
}

/// Warm resident vectors never suppress a stopped or incompatible provider's fresh preflight failure.
#[test]
fn warm_semantic_search_rechecks_live_provider_identity() {
    for incompatible in [false, true] {
        let (_directory, kb) = library();
        let (config, service) = query_service(incompatible);
        install_vector_fixture(&kb, &config);
        assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
        let options = SearchOptions {
            query: "semantic fixture query".into(),
            mode: SearchMode::Semantic,
            ..Default::default()
        };
        let hits = kb.search(&options).unwrap();
        assert_eq!(hits[0].snippet, "Cached finite evidence");
        if !incompatible {
            assert_eq!(service.join().unwrap().len(), 3);
            assert!(kb.search(&options).is_err());
        } else {
            assert!(kb.search(&options).is_err());
            let requests = service.join().unwrap();
            assert_eq!(
                requests,
                [
                    "GET /health",
                    "GET /v1/models",
                    "POST /v1/embeddings",
                    "GET /health"
                ]
            );
        }
    }
}

/// Even lexical warm-cache reads must reject newly malformed vectors instead of reusing prior valid facts.
#[test]
fn warm_local_search_revalidates_all_vectors_after_manifest_mutation() {
    let (_directory, kb) = library();
    let config = omni_knowledge::embedding::EmbeddingConfig {
        dimension: 2,
        ..Default::default()
    };
    install_vector_fixture(&kb, &config);
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let path = manifest_path(&kb.root);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["vectors"][0]["vector"] = serde_json::json!([1.0]);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        kb.search(&query(SearchMode::Grep))
            .unwrap_err()
            .to_string()
            .contains("vector dimension")
    );
}

/// Fresh pointer and read-gate confinement checks cannot be skipped merely because records are resident.
#[cfg(unix)]
#[test]
fn warm_search_still_validates_pointer_and_read_gate() {
    let (_directory, kb) = library();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let pointer = kb.root.join(".omni/current.json");
    let original = fs::read(&pointer).unwrap();
    fs::write(&pointer, "\"../escape\"").unwrap();
    assert!(kb.search(&query(SearchMode::Grep)).is_err());
    fs::write(&pointer, original).unwrap();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let gate = kb.root.join(".omni/read.lock");
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::remove_file(&gate).unwrap();
    std::os::unix::fs::symlink(outside.path(), &gate).unwrap();
    assert!(kb.search(&query(SearchMode::Grep)).is_err());
}

/// A stale public handle must compare semantic vectors with the current marker's provider identity.
#[test]
fn warm_search_uses_current_configuration_instead_of_handle_configuration() {
    let (_directory, kb) = library();
    let config = omni_knowledge::embedding::EmbeddingConfig {
        dimension: 2,
        ..Default::default()
    };
    install_vector_fixture(&kb, &config);
    let opened = KnowledgeBase::open(&kb.root).unwrap();
    assert_eq!(opened.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let marker = kb.root.join("omni-knowledge.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    value["embedding"]["embedding_space"] = serde_json::json!("changed-local-space");
    fs::write(marker, serde_json::to_vec(&value).unwrap()).unwrap();
    let options = SearchOptions {
        query: "semantic fixture query".into(),
        mode: SearchMode::Semantic,
        ..Default::default()
    };
    assert!(
        opened
            .search(&options)
            .unwrap_err()
            .to_string()
            .contains("configured embedding identity differs")
    );
}

/// Removing the selected generation cannot leave a resident snapshot available as successful search state.
#[test]
fn warm_search_refuses_a_missing_generation() {
    let (_directory, kb) = library();
    assert_eq!(kb.search(&query(SearchMode::Grep)).unwrap().len(), 1);
    let manifest = manifest_path(&kb.root);
    fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
    for mode in [SearchMode::Files, SearchMode::Grep, SearchMode::Bm25] {
        assert!(kb.search(&query(mode)).is_err());
    }
}
