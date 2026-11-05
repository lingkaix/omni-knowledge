//! Actual vector writer -> M5 capture -> held STORED directory -> exact vector reader.
//! Builder-owned numeric/authority/lifetime controls, NOT model quality or whole Root acceptance.
use super::*;
use knowledge_retrieval::{StoredDirectory, StoredDirectoryPlan};
use knowledge_search::{
    AccountGrant, ModelIdentity, VectorMetric, VectorNormalization, VectorRow, VectorStore,
};

fn fixture_representation() -> knowledge_search::RepresentationIdentity {
    knowledge_search::RepresentationIdentity {
        digest: "11".repeat(32),
        processor: "22".repeat(32),
        modality: knowledge_search::VectorModality::Text,
    }
}

#[test]
fn actual_vector_segments_read_from_the_same_held_archive_without_extraction() {
    let cache = tempfile::tempdir().unwrap();
    fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let account = Account::start(
        File::open(cache.path()).unwrap(),
        Configuration {
            memory_bytes: Some(2 * 1024 * 1024 * 1024),
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 16,
            max_records: 128,
            max_descriptor_bytes: 1024 * 1024,
            per_mount: 2,
        },
    )
    .unwrap();
    let inputs = reserve(Reservation {
        memory_bytes: 2 * 1024 * 1024,
        temporary_bytes: 0,
    });
    let model = ModelIdentity {
        provider: "precomputed-control".into(),
        model: "numeric-f32".into(),
        revision: "fixture-v1".into(),
        dimension: 2,
        metric: VectorMetric::Cosine,
        normalization: VectorNormalization::Raw,
        package_fingerprint: "cc".repeat(32),
        embedding_space: "two-dimensional-control".into(),
        preprocessing: "fixture-identity".into(),
    };
    let root = cache.path().join("vectors");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let rows: Vec<_> = (0..4096)
        .map(|n| VectorRow {
            representation: fixture_representation(),
            source_id: if n % 2 == 0 {
                "source-a".into()
            } else {
                "source-b".into()
            },
            source_digest: if n % 2 == 0 {
                "aa".repeat(32)
            } else {
                "bb".repeat(32)
            },
            locator: format!("row:{n}"),
            vector: if n % 2 == 0 {
                vec![1.0, 0.0]
            } else {
                vec![0.0, 1.0]
            },
        })
        .collect();
    let writer = VectorStore::build(
        &root,
        &model,
        &rows,
        &AccountGrant::from_work(reserve(Reservation {
            memory_bytes: 4 * 1024 * 1024,
            // Complete escaped metadata bound now includes two mandatory SHA identities.
            temporary_bytes: 8 * 1024 * 1024,
        })),
        None,
    )
    .unwrap();
    let generation = writer.generation().to_owned();
    let selected = tempfile::tempdir().unwrap();
    let text = cache.path().join("canonical.txt");
    fs::write(&text, TEXT).unwrap();
    let archive = archive_fixture_with_space(
        selected.path(),
        &root.join(&generation),
        &text,
        &generation,
        account,
        Some(&model),
    );
    drop(writer);
    drop(rows);
    fs::remove_dir_all(&root).unwrap();
    fs::remove_file(&text).unwrap();

    let (authority, owner) = NativeAuthority::new();
    let owner = Arc::new(owner);
    let selected_path = selected.path().to_path_buf();
    let policy = LocalPolicy {
        mode: ExposureMode::ReadOnly,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    };
    let library = Id::new("archive-library").unwrap();
    let selection = owner
        .select_current(
            library.clone(),
            File::open(selected.path()).unwrap(),
            policy.clone(),
            Arc::new(move |root| {
                let held = root.metadata().unwrap();
                fs::metadata(&selected_path)
                    .is_ok_and(|now| (now.dev(), now.ino()) == (held.dev(), held.ino()))
            }),
        )
        .unwrap();
    let session = authority
        .connect(std::slice::from_ref(&selection), TransportFacts::OwnerStdio)
        .unwrap();
    let runtime = Runtime::new(authority);
    let command:Command=serde_json::from_value(serde_json::json!({"op":"browse","library_id":library.as_str(),"mount_id":null,"path":"","cursor":null,"limit":10})).unwrap();
    let permit = runtime.native_scope(&session, &command, None).unwrap();
    let file = permit
        .with_root(&library, |root| {
            rustix::fs::openat(
                root,
                "frozen.zip",
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(|_| omni_service::ErrorCode::Changed)
        })
        .unwrap();
    let context = permit
        .with_selected_context(&library, |root, policy, revision| {
            let m = root.metadata().unwrap();
            Ok(format!("{}:{}:{policy:?}:{revision}", m.dev(), m.ino()))
        })
        .unwrap();
    let source = Arc::new(SelectedArchive {
        file,
        permit,
        library: library.clone(),
        path: RelativePath::new("frozen.zip").unwrap(),
    });
    let native = StoredIdentity::new(BUILD).unwrap();
    let mut requirements = native.requirements(&generation);
    requirements.model_space = Some(knowledge_package::VectorSpace {
        provider: &model.provider,
        model: &model.model,
        revision: &model.revision,
        dimension: 2,
        metric: "cosine",
        normalization: "raw",
        preprocessing: &model.preprocessing,
    });
    let plan = StoredDirectoryPlan {
        index_bytes: 2 * 1024 * 1024,
        table_bytes: 2 * 1024 * 1024,
        verification_memory: 16 * 1024 * 1024,
        read_window_bytes: 8192,
    };
    // A verifier cannot borrow its table/window siblings' already paid credit.
    let capped = StoredDirectoryPlan {
        verification_memory: 1,
        ..plan
    };
    assert!(
        StoredDirectory::open(
            source.clone(),
            &requirements,
            BUILD,
            capped,
            reserve(capped.requirement().unwrap()),
            &bounds(),
            &AtomicBool::new(false),
            &Control::new(Instant::now() + Duration::from_secs(60))
        )
        .is_err()
    );
    let version = ContentDigest::new(digest(&fs::read(&archive).unwrap())).unwrap();
    let recipe = ContentDigest::new(digest(b"actual archived vector ranges")).unwrap();
    let processor = ContentDigest::new(digest(BUILD.as_bytes())).unwrap();
    let key = Key {
        context: &context,
        library: library.as_str(),
        mount: None,
        source: "frozen.zip",
        version: &version,
        recipe: &recipe,
        processor: &processor,
    };
    let directory_need = plan.requirement().unwrap();
    let vector_need = Reservation {
        memory_bytes: 2 * 1024 * 1024,
        temporary_bytes: 0,
    };
    let need = Reservation {
        memory_bytes: directory_need.memory_bytes + vector_need.memory_bytes,
        temporary_bytes: 0,
    };
    account
        .submit(Submission {
            operation_id: "archived-vector",
            job_id: "archived-vector-job",
            key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let directory_work = Work::split_claim(&claim, key, directory_need).unwrap();
    let vector_work = Work::split_claim(&claim, key, vector_need).unwrap();
    let control = claim_control(source.clone(), claim.clone(), key);
    let pressure = reserve(Reservation {
        memory_bytes: account.status().unwrap().memory_available,
        temporary_bytes: 0,
    });
    let directory = StoredDirectory::open(
        source.clone(),
        &requirements,
        BUILD,
        plan,
        directory_work,
        &bounds(),
        &AtomicBool::new(false),
        &control,
    )
    .unwrap();
    let components = directory.directory("indexes/native").unwrap();
    let segment = components.open("segment.bin").unwrap();
    assert!(
        segment.read_bytes(0..8193).is_err(),
        "range ceiling must precede allocation, even inside a larger admitted extent"
    );
    let reader = VectorStore::open_directory(
        components.clone(),
        &generation,
        &model,
        &AccountGrant::from_work(vector_work),
    )
    .unwrap();
    let result = reader
        .query_filtered(&[1.0, 0.0], 1, |id| Ok(id == "source-b"))
        .unwrap();
    result.check().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "source-b");
    assert_eq!(result[0].1, 0.0);
    let mut locations = 0;
    result
        .visit_evidence(|hit| {
            assert_eq!(hit.source_id, "source-b");
            locations += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(locations, 2048);
    drop(pressure);
    owner
        .set_policy(
            &selection,
            LocalPolicy {
                excluded_paths: vec![RelativePath::new("frozen.zip").unwrap()],
                ..policy.clone()
            },
        )
        .unwrap();
    assert!(
        result.check().is_err(),
        "current actual policy blocks old vector bytes"
    );
    owner.set_policy(&selection, policy).unwrap();
    assert!(
        result.check().is_err(),
        "restoring policy data cannot resurrect the old native permit revision"
    );
    account.queue().cancel(claim.job(), key).unwrap();
    drop(result);
    drop(components);
    drop(directory);
    drop(control);
    assert!(
        account.status().unwrap().reserved.memory_bytes >= directory_need.memory_bytes,
        "a real remaining component handle retains archive/source/credit"
    );
    assert!(segment.read_bytes(0..4).is_err());
    drop(segment);
    drop(source);
    account
        .queue()
        .settle(&claim, key, Outcome::Cancelled)
        .unwrap();
    drop(inputs);
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
}
