//! Actual native whole-source producer under SAME one Claim; not tuple/product acceptance.
use super::*;
#[path = "inventory_corpus_query_tests.rs"]
mod query_job;
use std::io::Read;
use std::os::unix::fs::{FileExt, PermissionsExt};
struct ContractFixture {
    server: crate::native_service::embedding_tests::server::Server,
    _files: tempfile::TempDir,
    _work: knowledge_retrieval::Work,
}
fn contract_model(
    service: &NativeLibraryService,
    owner: &Owner,
) -> (NativeEmbeddingSelection, ContractFixture) {
    use crate::native_service::embedding_tests::{ready, server};
    use knowledge_search::{ModelIdentity, VectorMetric, VectorNormalization};
    let work = ready(Reservation {
        memory_bytes: 2 * 1024 * 1024,
        temporary_bytes: 65536,
    });
    let files = tempfile::tempdir().unwrap();
    let code =
        include_bytes!("../../crates/knowledge-processing/tests/support/embedding_server.rs");
    let package = files.path().join("numeric-contract.rs");
    let manifest = files.path().join("identity.json");
    fs::write(&package, code).unwrap();
    let mut config = crate::embedding::EmbeddingConfig {
        endpoint: "http://127.0.0.1:1".into(),
        model: "bounded-corpus-contract".into(),
        dimension: 2,
        package_fingerprint: hex::encode(Sha256::digest(code)),
        embedding_space: "contract-only-not-WeMM".into(),
        timeout_seconds: 5,
    };
    let server = server::Server::start_for(&config, None, false, Duration::from_secs(120));
    config.endpoint = server.endpoint.clone();
    let identity = ModelIdentity {
        provider: "deterministic-contract-only".into(),
        model: config.model.clone(),
        revision: "corpus-fixture-v1".into(),
        dimension: 2,
        metric: VectorMetric::Cosine,
        normalization: VectorNormalization::Raw,
        package_fingerprint: config.package_fingerprint.clone(),
        embedding_space: config.embedding_space.clone(),
        preprocessing: "UTF8-length-contract-not-semantic-quality".into(),
    };
    fs::write(&manifest, serde_json::to_vec(&identity).unwrap()).unwrap();
    let selected = service
        .select_embedding(
            owner,
            File::open(package).unwrap(),
            File::open(manifest).unwrap(),
            config.clone(),
            Budget {
                max_files: 2,
                max_bytes: 1024 * 1024,
                max_millis: 30000,
                max_memory_bytes: 2 * 1024 * 1024,
            },
            ready(NativeEmbeddingSelection::requirement(&config).unwrap()),
        )
        .unwrap();
    (
        selected,
        ContractFixture {
            server,
            _files: files,
            _work: work,
        },
    )
}
fn corpus_refresh(
    service: &NativeLibraryService,
    session: &Session,
    id: &str,
    embeddings: bool,
) -> Permit {
    let command=wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"corpus","command":{"op":"refresh","library_id":service.library_id().as_str(),"mount_ids":[],"verification":"full_hash","embeddings":embeddings,"budget":{"max_files":1000,"max_bytes":"67108864","max_millis":120000,"max_memory_bytes":"536870912"},"operation_id":id}})).unwrap()).unwrap().command;
    service.native_scope(session, &command, None).unwrap()
}
fn file_at(root: &File, name: &str) -> File {
    File::from(
        rustix::fs::openat(
            root,
            name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    )
}
fn frames(mut file: impl Read) -> Vec<serde_json::Value> {
    let mut result = Vec::new();
    loop {
        let mut len = [0; 4];
        match file.read_exact(&mut len) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => panic!("{e}"),
        };
        let len = u32::from_le_bytes(len) as usize;
        assert!(len <= 1024 * 1024);
        let mut data = vec![0; len];
        file.read_exact(&mut data).unwrap();
        result.push(serde_json::from_slice(&data).unwrap());
    }
    result
}
#[test]
fn actual_corpus_m3_persists_and_publishes_all_sources_without_resident_body_or_fd_set() {
    if std::env::var_os("OMNI_CORPUS_CHILD").is_none() {
        let cache = tempfile::tempdir().unwrap();
        fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
        for mode in [
            "normal",
            "query-source-change",
            "ranking",
            "query-byte-budget",
            "query-byte-phase",
            "query-range-budget",
            "query-object-budget",
            "query-input-object-budget",
            "query-drop-tail",
            "waiting",
            "waiting-after-progress",
            "vectors",
            "vectors-source-change",
            "vectors-tape-change",
        ] {
            let output=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","native_service::inventory::tests::corpus::actual_corpus_m3_persists_and_publishes_all_sources_without_resident_body_or_fd_set","--nocapture"]).env("OMNI_CORPUS_CHILD","1").env("OMNI_CORPUS_MODE",mode).env("OMNI_CACHE_DIR",cache.path()).output().unwrap();
            assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
            assert!(
                output.status.success(),
                "actual corpus {mode} child: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(home.path(), "Actual bounded native corpus").unwrap();
    let source_root = kb.root.clone();
    fs::write(
        source_root.join("raw/actually-deleted.md"),
        "actual prior source, not a fabricated tombstone",
    )
    .unwrap();
    fs::write(kb.root.join("raw/one.md"), "# Native first\n日本語 alpha\n").unwrap();
    fs::write(
        kb.root.join("raw/two.html"),
        "<h1>Actual second</h1><p>日本語 beta</p>",
    )
    .unwrap();
    fs::write(kb.root.join("raw/unavailable.unknown"), [0xff, 0, 0xfe, 1]).unwrap();
    let mode = std::env::var("OMNI_CORPUS_MODE").unwrap();
    let ranking = mode == "ranking";
    // Keep EVERY original hostile/high-frequency alpha fixture unchanged. The additional
    // ranking mode makes a rare-term long original repeat across partitions and gives a
    // single short original the same best rank plus the earlier stable logical path.
    let long = if ranking {
        ("corpus 日本語\n".repeat(12000) + "alpha\n").repeat(21)
    } else {
        "corpus alpha 日本語\n".repeat(180_000)
    };
    assert!(long.len() > 2 * 1024 * 1024);
    if ranking {
        assert!(long.len() >= "corpus alpha 日本語\n".len() * 180_000);
    }
    let long_path = if ranking {
        "raw/zzz-long.md"
    } else {
        "raw/long.md"
    };
    fs::write(kb.root.join(long_path), &long).unwrap();
    drop(long);
    for n in 0..70 {
        let path = if ranking && n == 0 {
            "raw/aaa-page-000.md".to_owned()
        } else {
            format!("raw/page-{n:03}.md")
        };
        let text = if ranking && n == 0 {
            "alpha\n".to_owned()
        } else {
            format!("actual page source {n} alpha\n")
        };
        fs::write(kb.root.join(path), text).unwrap();
    }
    let (service, owner, _selection) = NativeLibraryService::ordinary_local(
        kb,
        LocalPolicy {
            mode: wire::ExposureMode::ReadWrite,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        },
    )
    .unwrap();
    let embeddings = mode.starts_with("vectors");
    let (model, fixture) = if embeddings {
        let (model, fixture) = contract_model(&service, &owner);
        (Some(model), Some(fixture))
    } else {
        (None, None)
    };
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let seed = reconcile(
        &service,
        &owner,
        corpus_refresh(&service, &session, "actual-metadata-seed", false),
    );
    drop(seed);
    fs::remove_file(source_root.join("raw/actually-deleted.md")).unwrap();
    fs::write(
        source_root.join("raw/actually-denied.md"),
        "no original digest may be fabricated for this unreadable source",
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            source_root.join("raw/actually-denied.md"),
            fs::Permissions::from_mode(0o000),
        )
        .unwrap();
    }
    let permit = corpus_refresh(&service, &session, "actual-corpus", embeddings);
    let inventory = reconcile(&service, &owner, permit.clone());
    let expected = observed(&inventory, long_path).source_id().to_owned();
    let first_ranked = ranking.then(|| {
        observed(&inventory, "raw/aaa-page-000.md")
            .source_id()
            .to_owned()
    });
    {
        let page = inventory
            .page(
                0,
                Budget {
                    max_files: 1000,
                    max_bytes: 8 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 1024 * 1024,
                },
                holding(2 * 1024 * 1024, 0),
            )
            .unwrap();
        assert!(
            page.sources()
                .all(|s| !s.observation().stat.path.as_str().starts_with("journal/")),
            "actual M1's own canonical journal is bookkeeping, never original source content"
        );
    }
    knowledge_retrieval::KernelAdapter::initialize_runtime().unwrap();
    let job = service
        .prepare_refresh(
            &owner,
            permit,
            inventory,
            model.clone(),
            holding(NativeRefreshJob::metadata_requirement().memory_bytes, 0),
        )
        .unwrap();
    let account = Account::current().unwrap();
    let need = job.corpus_requirement().unwrap();
    assert!(need.memory_bytes < 256 * 1024 * 1024);
    job.submit(NativeJobPlan {
        operation_id: "actual-corpus",
        job_id: "actual-corpus-job",
        budget: Budget {
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            max_memory_bytes: 512 * 1024 * 1024,
        },
        reservation: Reservation {
            memory_bytes: 256 * 1024 * 1024,
            temporary_bytes: need.temporary_bytes,
        },
        priority: knowledge_core::jobs::Priority::Normal,
        max_attempts: 1,
    })
    .unwrap();
    let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    let number = claim.job();
    let waiting = mode.starts_with("waiting");
    let mut test_metadata = holding(8 * 1024 * 1024, 0);
    let k = job.key();
    let facts = (
        k.context.to_owned(),
        k.library.to_owned(),
        k.mount.map(str::to_owned),
        k.source.to_owned(),
        k.version.clone(),
        k.recipe.clone(),
        k.processor.clone(),
    );
    let generated = admitted::generated_path().unwrap().to_owned();
    let status = account.status().unwrap();
    let mut pressure = holding(
        status.memory_available,
        if waiting {
            status
                .storage_available
                .saturating_sub(if mode == "waiting-after-progress" {
                    256 * 1024
                } else {
                    0
                })
        } else {
            0
        },
    );
    let draft = job.begin_corpus(claim, &generated).unwrap();
    if waiting {
        let directory = draft.staging_directory().unwrap().try_clone().unwrap();
        let mut waits = 0;
        let mut admitted_writes = 0;
        let mut at_wait = None;
        let lengths = || {
            [
                "representations.data",
                "sources.rows",
                "lexical.rows",
                "inventory.rows",
            ]
            .map(|name| file_at(&directory, name).metadata().unwrap().len())
        };
        let error = match draft.build_lexical(|event| {
            if let NativeCorpusProgress::WriteAdmitted { bytes, available } = event {
                assert!(available >= bytes + 64 * 1024);
                admitted_writes += 1;
            }
            if let NativeCorpusProgress::Waiting(reason) = event {
                at_wait = Some(lengths());
                assert_eq!(reason, knowledge_core::admission::Waiting::Storage);
                waits += 1;
                account
                    .queue()
                    .cancel(
                        number,
                        knowledge_core::jobs::Key {
                            context: &facts.0,
                            library: &facts.1,
                            mount: facts.2.as_deref(),
                            source: &facts.3,
                            version: &facts.4,
                            recipe: &facts.5,
                            processor: &facts.6,
                        },
                    )
                    .unwrap();
            }
        }) {
            Err(error) => error,
            Ok(_) => panic!("resource wait cannot become successful publication"),
        };
        assert_eq!(
            waits, 1,
            "actual Core waiting is visible BEFORE the append which requires its credit"
        );
        let before = at_wait.expect("actual waiting boundary");
        if mode == "waiting-after-progress" {
            assert!(
                before[0] > 0 && admitted_writes > 0,
                "exercise actual previously prepaid successful appends, not an empty-corpus special case"
            );
        }
        assert_eq!(
            lengths(),
            before,
            "cancellation while Waiting cannot perform ANY additional payload or metadata append"
        );
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        drop(directory);
        let (draft, _cause) = error.into_parts();
        let (job, claim) = draft.into_job().unwrap();
        account
            .queue()
            .settle(&claim, job.key(), knowledge_core::jobs::Outcome::Cancelled)
            .unwrap();
        drop(facts);
        test_metadata.finish().unwrap();
        pressure.finish().unwrap();
        return;
    }
    let mut reports = 0;
    let mut admitted_writes = 0;
    let draft = draft
        .build_lexical(|event| {
            if let NativeCorpusProgress::WriteAdmitted { bytes, available } = event {
                assert!(available >= bytes + 64 * 1024);
                admitted_writes += 1;
            }
            if let NativeCorpusProgress::Prepared { .. } = event {
                reports += 1;
            }
        })
        .unwrap_or_else(|error| panic!("actual corpus producer: {:#}", error.cause()));
    assert!(reports >= 4);
    let (count, shards, rows) = draft.counts();
    assert!(count >= 74 && shards >= 4 && rows > 0);
    assert_eq!(account.status().unwrap().queue.unsettled, 1);
    assert_eq!(account.status().unwrap().memory_available, 0);
    let records = frames(file_at(draft.staging_directory().unwrap(), "sources.rows"));
    let catalog = frames(file_at(draft.staging_directory().unwrap(), "lexical.rows"));
    let inventory = frames(file_at(
        draft.staging_directory().unwrap(),
        "inventory.rows",
    ));
    assert_eq!(draft.coverage().observations, inventory.len() as u64);
    assert_eq!(draft.coverage().tombstones, 1);
    assert_eq!(draft.coverage().unavailable, 1);
    let denied = inventory
        .iter()
        .find(|r| r["stat"]["path"] == "raw/actually-denied.md")
        .expect("actual unreadable metadata remains discoverable");
    assert!(
        denied["version"].is_null(),
        "unreadable original has NO manufactured digest/version"
    );
    assert!(
        inventory
            .iter()
            .any(|r| r["stat"]["path"] == "raw/actually-deleted.md"),
        "actual M1 tombstone remains discoverable"
    );
    assert!(
        !records
            .iter()
            .any(|r| r["document"]["key"]["id"] == denied["source_id"]),
        "unreadable metadata cannot become an invented representation/vector source"
    );
    assert_eq!(catalog.len() as u64, shards);
    assert_eq!(
        admitted_writes,
        count + records.len() as u64 + catalog.len() as u64 + inventory.len() as u64,
        "EVERY tape/source/catalog/inventory append must have the actual complete prepaid write event"
    );
    let chunks: Vec<_> = records
        .iter()
        .filter(|v| v["document"]["key"]["id"] == expected)
        .collect();
    assert!(
        chunks.len() >= 3,
        "actual >2MiB representation is windowed, not a corpus/source cap"
    );
    assert!(
        chunks
            .iter()
            .all(|v| v["tape_length"].as_u64().unwrap() <= 1024 * 1024)
    );
    assert!(
        records
            .iter()
            .any(|v| v["document"]["name"] == "unavailable.unknown" && v["tape_length"] == 0)
    );
    assert!(
        records
            .iter()
            .all(|v| v["document"]["key"]["id"] != "native-M1-inventory-descriptor")
    );
    let full_long_digest = chunks[0]["representation_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let first_source = records
        .iter()
        .find(|r| r["embedding_rows"].as_u64().unwrap() > 0)
        .unwrap()["document"]["locator"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(chunks);
    drop(catalog);
    drop(records);
    let draft = if embeddings {
        let server = &fixture.as_ref().unwrap().server;
        assert_eq!(
            server.accepted.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "provider setup is prepaid, not contacted before actual text production"
        );
        if mode == "vectors-source-change" {
            fs::write(
                home.path().join(&first_source),
                "replaced original after actual M3",
            )
            .unwrap();
        }
        if mode == "vectors-tape-change" {
            let tape = File::from(
                rustix::fs::openat(
                    draft.staging_directory().unwrap(),
                    "representations.data",
                    rustix::fs::OFlags::RDWR
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .unwrap(),
            );
            tape.write_all_at(b"!", 0).unwrap();
        }
        let built = draft.build_vectors(|_| {});
        if mode != "vectors" {
            let error = match built {
                Err(error) => error,
                Ok(_) => panic!("changed actual source/tape cannot prepare vectors"),
            };
            assert_eq!(
                server.accepted.load(std::sync::atomic::Ordering::SeqCst),
                0,
                "changed actual owner is refused BEFORE receiver contact"
            );
            let (draft, _cause) = error.into_parts();
            assert!(draft.test_vectors().is_none());
            let (job, claim) = draft.into_job().unwrap();
            account
                .queue()
                .settle(&claim, job.key(), knowledge_core::jobs::Outcome::Failed)
                .unwrap();
            drop(job);
            drop(model);
            drop(fixture);
            drop(facts);
            test_metadata.finish().unwrap();
            pressure.finish().unwrap();
            return;
        }
        let draft =
            built.unwrap_or_else(|e| panic!("actual saved M3 vector producer: {:#}", e.cause()));
        assert_eq!(account.status().unwrap().memory_available, 0);
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        assert!(server.accepted.load(std::sync::atomic::Ordering::SeqCst) >= rows as usize);
        let values = frames(draft.test_vectors().unwrap().component("rows.bin").unwrap());
        assert_eq!(values.len() as u64, rows);
        assert!(
            values
                .iter()
                .all(|r| r["source_id"] != "native-M1-inventory-descriptor")
        );
        let long_rows: Vec<_> = values
            .iter()
            .filter(|r| r["source_id"] == expected)
            .collect();
        assert!(long_rows.len() > 40);
        assert!(
            long_rows
                .iter()
                .all(|r| r["representation"]["digest"] == full_long_digest)
        );
        drop(long_rows);
        drop(values);
        assert!(
            rustix::fs::statat(
                draft.staging_directory().unwrap(),
                "vectors/CURRENT",
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW
            )
            .is_err(),
            "prepared components cannot independently select whole tuple"
        );
        let pin = draft
            .test_vectors()
            .unwrap()
            .component("manifest.json")
            .unwrap();
        let error = match draft.into_job() {
            Err(error) => error,
            Ok(_) => panic!("live actual component must retain its owning corpus"),
        };
        let (draft, _cause) = error.into_parts();
        drop(pin);
        draft
    } else {
        draft
    };
    let draft = draft
        .prepare_tuple(|_| {})
        .unwrap_or_else(|e| panic!("actual whole tuple seal: {:#}", e.cause()));
    let header = draft.test_tuple_header().unwrap();
    assert_eq!(
        header["coverage"]["observations"],
        draft.coverage().observations
    );
    assert_eq!(header["coverage"]["unavailable"], 1);
    assert_eq!(header["coverage"]["tombstones"], 1);
    assert_eq!(header["sources"], count);
    assert_eq!(header["shards"], shards);
    assert_eq!(header["model"].is_null(), !embeddings);
    assert_eq!(header["vector_generation"].is_null(), !embeddings);
    assert_eq!(header["vector_rows"], if embeddings { rows } else { 0 });
    let members = frames(file_at(draft.staging_directory().unwrap(), "members.rows"));
    assert_eq!(members.len() as u64, header["members"].as_u64().unwrap());
    assert!(
        members.iter().any(|m| m["name"] == "inventory.rows"
            && m["sha256"].as_str().is_some_and(|s| s.len() == 64))
    );
    assert!(members.iter().any(|m| {
        m["name"]
            .as_str()
            .is_some_and(|n| n.starts_with("kernel/") && n.ends_with("manifest.json"))
    }));
    let aggregate_pin = file_at(draft.staging_directory().unwrap(), "READERS");
    assert!(
        fs2::FileExt::try_lock_exclusive(&aggregate_pin).is_err(),
        "actual aggregate reader pin protects not-yet-opened components"
    );
    drop(aggregate_pin);
    assert!(
        rustix::fs::statat(
            draft.staging_directory().unwrap(),
            "CURRENT",
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW
        )
        .is_err(),
        "a prepared whole tuple still cannot independently select library CURRENT"
    );
    drop(members);
    drop(header);
    let mut draft = draft;
    if mode == "normal" {
        draft.test_switch_fault();
    }
    let selected = draft.select_tuple(|_| {});
    let draft = if mode == "normal" {
        let error = match selected {
            Err(error) => error,
            Ok(_) => {
                panic!("post-switch uncertainty must return the actual owning publication phase")
            }
        };
        assert!(format!("{:#}", error.cause()).contains("post-switch"));
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        let (draft, _cause) = error.into_parts();
        draft
            .select_tuple(|_| {})
            .unwrap_or_else(|e| panic!("actual foreground publication recovery: {:#}", e.cause()))
    } else {
        selected.unwrap_or_else(|e| panic!("actual whole selector publication: {:#}", e.cause()))
    };
    let pointer = draft.test_selected_pointer().unwrap();
    assert_eq!(pointer["format"], 1);
    assert!(
        pointer["stage"]
            .as_str()
            .unwrap()
            .starts_with("native-corpus-")
    );
    assert_eq!(pointer["header_sha256"].as_str().unwrap().len(), 64);
    drop(pointer);
    let mut draft = draft;
    let loan = if mode == "normal" {
        draft.test_retained_scope().unwrap()
    } else {
        0
    };
    let before_cleanup = account
        .job_status(number, draft.job_key_for_test())
        .unwrap()
        .execution
        .memory_bytes;
    let mutation_directory = draft.staging_directory().unwrap().try_clone().unwrap();
    let (job, claim) = draft.into_job().unwrap();
    assert_eq!(
        account
            .job_status(number, job.key())
            .unwrap()
            .execution
            .memory_bytes,
        before_cleanup + loan,
        "actual retained SDK scope must return to SAME Claim, not leak or release globally"
    );
    assert_eq!(
        account.status().unwrap().queue.unsettled,
        1,
        "draft consumption cannot complete aggregate tuple job"
    );
    account
        .queue()
        .settle(&claim, job.key(), knowledge_core::jobs::Outcome::Completed)
        .unwrap();
    drop(job);
    // The Refresh pressure oracle ends after actual aggregate settlement. The independent
    // cold Query below prepays its own complete peak, then applies its own FULL pressure.
    pressure.finish().unwrap();
    let methods = if embeddings {
        vec!["files", "bm25", "literal", "regex", "semantic"]
    } else {
        vec!["files", "bm25", "literal", "regex"]
    };
    let query_command=wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"actual-cold-tuple-query","command":{"op":"query","query":{"library_ids":[service.library_id().as_str()],"keywords":["ALPHA","alpha.*"],"methods":methods,"required_tags":[],"case_sensitive":false,"limit":10,"budget":{"max_files":1000,"max_bytes":"67108864","max_millis":60000,"max_memory_bytes":"536870912"},"image_query":null}}})).unwrap()).unwrap().command;
    let query_permit = service
        .native_scope(&session, &query_command, None)
        .unwrap();
    let reader = service
        .open_corpus_tuple(&owner, query_permit, ready(NativeTupleRead::requirement()))
        .unwrap();
    let reopened = reader.test_header().unwrap();
    assert_eq!(
        reader.test_unpinned_header_opens(),
        0,
        "actual aggregate pin must precede every native header open, not only the returned reader"
    );
    assert_eq!(reopened["sources"], count);
    assert_eq!(reopened["coverage"]["unavailable"], 1);
    assert_eq!(reopened["coverage"]["tombstones"], 1);
    assert!(
        !reader.test_aggregate_exclusive().unwrap(),
        "actual reopened aggregate pin protects components which this Query has not opened"
    );
    assert_eq!(
        account.status().unwrap().queue.unsettled,
        0,
        "readonly tuple open cannot fabricate a Refresh/job"
    );
    let source_permit = service
        .native_scope(&session, &query_command, None)
        .unwrap();
    let mut query_scratch = holding(4 * 1024 * 1024, 0);
    let query_inventory = service
        .inventory_for_query(
            &owner,
            &source_permit,
            "wiki",
            reopened["scan"].as_str().unwrap(),
            holding(2 * 1024 * 1024, 0),
            &Preflight {
                holding: &query_scratch,
                budget: Budget {
                    max_files: 1000,
                    max_bytes: 8 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 4 * 1024 * 1024,
                },
            },
        )
        .unwrap();
    let first_path = reader.test_part_path(0).unwrap();
    let actual = observed(&query_inventory, &first_path);
    let original_id = actual.source_id().to_owned();
    let before_hash = NativeSource::test_hash_read_calls();
    let (cached_source, _next) = reader
        .current_source_at(&service, &source_permit, actual, 0)
        .unwrap()
        .unwrap();
    cached_source.check().unwrap();
    assert_eq!(
        NativeSource::test_hash_read_calls(),
        before_hash,
        "current Query eligibility must use actual pinned original FD version, not synchronously hash source bodies"
    );
    // Independent actual Query scope, not the earlier primitive's already pinned source checks.
    let job_permit = service
        .native_scope(&session, &query_command, None)
        .unwrap();
    query_job::run(
        &service,
        &owner,
        &job_permit,
        (
            reopened["scan"].as_str().unwrap(),
            if mode == "query-source-change" {
                "raw/two.html"
            } else {
                "raw/one.md"
            },
        ),
        &query_inventory,
        (mode == "query-source-change").then(|| source_root.join("raw/two.html")),
        model.clone(),
    );
    let mut whole_command = query_command.clone();
    if let wire::Command::Query { query } = &mut whole_command {
        query.methods = vec![wire::SearchMethod::Bm25];
    }
    let whole_permit = service
        .native_scope(&session, &whole_command, None)
        .unwrap();
    query_job::run_whole_bm25(
        &service,
        &owner,
        &whole_permit,
        reopened["scan"].as_str().unwrap(),
        &mode,
        first_ranked.as_deref(),
    );
    // A real same-inode edit followed by actual stat-first M1 discovery leaves no verified
    // new digest. The opaque producer FD version must reject it without a query-time rehash.
    {
        let mut edit = fs::OpenOptions::new()
            .append(true)
            .open(source_root.join(&first_path))
            .unwrap();
        std::io::Write::write_all(&mut edit, b"\nactual changed original\n").unwrap();
        edit.sync_all().unwrap();
    }
    let stat_command=wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"actual-stat-first-current","command":{"op":"refresh","library_id":service.library_id().as_str(),"mount_ids":[],"verification":"stat_first","embeddings":false,"budget":{"max_files":1000,"max_bytes":"67108864","max_millis":60000,"max_memory_bytes":"536870912"},"operation_id":"actual-stat-first-current"}})).unwrap()).unwrap().command;
    let stat_permit = service.native_scope(&session, &stat_command, None).unwrap();
    let mut stat_scratch = holding(4 * 1024 * 1024, 16 * 1024 * 1024);
    let (stat_inventory, stat_report) = service
        .reconcile_inventory(
            &owner,
            stat_permit,
            "wiki",
            holding(2 * 1024 * 1024, 0),
            &Preflight {
                holding: &stat_scratch,
                budget: Budget {
                    max_files: 1000,
                    max_bytes: 64 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 4 * 1024 * 1024,
                },
            },
        )
        .unwrap();
    assert!(stat_report.complete);
    let current = observed(&stat_inventory, &first_path);
    assert_eq!(
        current.source_id(),
        original_id,
        "real same-inode edit retains actual independently tracked M1 source UUID"
    );
    assert_eq!(
        current.observation().verification,
        knowledge_storage::Verification::Pending
    );
    assert!(
        current.observation().version.is_none(),
        "real stat-first changed original cannot invent a verified new digest"
    );
    let current_query = service
        .native_scope(&session, &query_command, None)
        .unwrap();
    let before_stale_hash = NativeSource::test_hash_read_calls();
    let stale = match reader.current_source_at(&service, &current_query, current, 0) {
        Err(error) => error,
        Ok(_) => panic!(
            "a current M1 metadata row with changed original version cannot certify old cached payload"
        ),
    };
    assert!(
        stale
            .chain()
            .any(|cause| cause.is::<crate::native_service::source::CachedQuerySourceUnavailable>()),
        "actual source ineligibility must be typed separately from global policy/Claim/cache failure: {stale:#}"
    );
    assert_eq!(
        NativeSource::test_hash_read_calls(),
        before_stale_hash,
        "stale Query dependency must not synchronously hash the changed original"
    );
    drop(current_query);
    // Explicit fixture actor removes the actual original name. Outer native authorization
    // remains current; the confined open refusal, not its text, identifies unavailable input.
    let missing = observed(&stat_inventory, &first_path);
    let missing_query = service
        .native_scope(&session, &query_command, None)
        .unwrap();
    let original_path = source_root.join(&first_path);
    let hidden_path = source_root.join("raw/fixture-missing-original");
    fs::rename(&original_path, &hidden_path).unwrap();
    let refused = match reader.current_source_at(&service, &missing_query, missing, 0) {
        Err(error) => error,
        Ok(_) => panic!("missing actual original cannot authorize cached payload"),
    };
    fs::rename(&hidden_path, &original_path).unwrap();
    if mode == "normal" {
        query_job::run_canonical_metadata_controls(
            &service,
            &owner,
            &session,
            &source_root,
            &query_inventory,
            &stat_inventory,
        );
    }
    assert!(
        refused
            .chain()
            .any(|cause| cause.is::<crate::native_service::source::CachedQuerySourceUnavailable>()),
        "confined original absence must not be confused with a global grant failure: {refused:#}"
    );
    assert_eq!(NativeSource::test_hash_read_calls(), before_stale_hash);
    drop(missing_query);
    drop(stat_inventory);
    drop(stat_report);
    stat_scratch.finish().unwrap();
    // Explicit fixture actor replaces an equal-byte cache header, not a Query write capability.
    let original = File::from(
        rustix::fs::openat(
            &mutation_directory,
            "tuple.json",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    );
    let mut equal = vec![0; original.metadata().unwrap().len() as usize];
    std::os::unix::fs::FileExt::read_exact_at(&original, &mut equal, 0).unwrap();
    rustix::fs::renameat(
        &mutation_directory,
        "tuple.json",
        &mutation_directory,
        "fixture-original-header",
    )
    .unwrap();
    let mut replacement = File::from(
        rustix::fs::openat(
            &mutation_directory,
            "tuple.json",
            rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_bits_truncate(0o600),
        )
        .unwrap(),
    );
    std::io::Write::write_all(&mut replacement, &equal).unwrap();
    replacement.sync_all().unwrap();
    assert!(
        reader.check().is_err(),
        "equal-byte replacement cannot resurrect the held aggregate header owner"
    );
    let new_permit = service
        .native_scope(&session, &query_command, None)
        .unwrap();
    assert!(
        service
            .open_corpus_tuple(&owner, new_permit, ready(NativeTupleRead::requirement()))
            .is_err(),
        "cold reader cannot accept equal-byte foreign header identity"
    );
    assert!(
        cached_source.check().is_err(),
        "current Query source must retain aggregate metadata dependency checks"
    );
    drop(cached_source);
    drop(query_inventory);
    drop(source_permit);
    drop(first_path);
    query_scratch.finish().unwrap();
    drop(replacement);
    drop(original);
    drop(equal);
    drop(mutation_directory);
    drop(reopened);
    drop(reader);
    drop(model);
    drop(fixture);
    drop(facts);
    test_metadata.finish().unwrap();
}
