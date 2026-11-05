//! Builder-owned actual lexical archive composition; not whole native acceptance.
#[path = "stored_generation/support.rs"]
mod support;
use support::*;

/// Compose actual writer/M5 ranges/Tantivy queries, native policy and Claim/resource lifetimes.
#[test]
fn actual_stored_generation_queries_and_current_native_lifetimes() {
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
    let source_root = tempfile::tempdir().unwrap();
    fs::create_dir_all(source_root.path().join("indexes/native")).unwrap();
    let source_path = source_root.path().join(SOURCE);
    fs::write(&source_path, TEXT).unwrap();
    let native_store = cache.path().join("kernel");
    let kernel = KernelAdapter::open(source_root.path(), &native_store, BUILD).unwrap();
    let control = Control::new(Instant::now() + Duration::from_secs(60));
    let mut publication = reserve(KernelAdapter::publish_requirement(4096).unwrap());
    let generation_id = kernel
        .publish(view(), None, &mut publication, &control)
        .unwrap();
    drop(publication);
    let scope_dir = fs::read_dir(&native_store)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let selected_parent = tempfile::tempdir().unwrap();
    let selected_root = selected_parent.path().join("selected");
    fs::create_dir(&selected_root).unwrap();
    let archive = archive_fixture(
        &selected_root,
        &scope_dir.join(&generation_id),
        &source_path,
        &generation_id,
        account,
    );
    drop(kernel);
    fs::remove_dir_all(&native_store).unwrap();
    fs::remove_file(&source_path).unwrap();
    // No unpack/scratch/rebuild is possible: both the native store and canonical loose source are gone.
    let (authority, owner) = NativeAuthority::new();
    let owner = Arc::new(owner);
    let selected_path = selected_root.clone();
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
            File::open(&selected_root).unwrap(),
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
    let command: Command = serde_json::from_value(serde_json::json!({"op":"browse", "library_id":library.as_str(), "mount_id":null, "path":"", "cursor":null, "limit":10})).unwrap();
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
    let version = ContentDigest::new(digest(&fs::read(&archive).unwrap())).unwrap();
    let recipe = ContentDigest::new(digest(b"real native archive query")).unwrap();
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
    let plan = StoredPlan {
        index_bytes: 2 * 1024 * 1024,
        table_bytes: 2 * 1024 * 1024,
    };
    let need = plan.requirement().unwrap();
    account
        .submit(Submission {
            operation_id: "archive-query",
            job_id: "archive-query-job",
            key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let work = Work::split_claim(&claim, key, need).unwrap();
    let identity = StoredIdentity::new(BUILD).unwrap();
    let requirements = identity.requirements(&generation_id);
    let guarded = claim_control(source.clone(), claim.clone(), key);
    // Spend all otherwise free global credit after the complete real query plan
    // was paid. Verification must borrow that plan, not make a second late charge.
    let pressure = reserve(Reservation {
        memory_bytes: account.status().unwrap().memory_available,
        temporary_bytes: 0,
    });
    let mut reader = StoredGeneration::open(
        source.clone(),
        &requirements,
        "archive-shard",
        BUILD,
        "indexes/native",
        plan,
        work,
        &bounds(),
        &AtomicBool::new(false),
        &guarded,
    )
    .unwrap();
    assert_eq!(reader.id(), generation_id);
    drop(pressure);
    let baseline = account.status().unwrap().reserved.memory_bytes;
    for (mode, text) in [
        (Mode::Lexical, "alpha"),
        (Mode::Lexical, "机器学习"),
        (Mode::Lexical, "日本語"),
        (Mode::Exact, "shared archive"),
        (Mode::Regex, "^alpha.*archive$"),
    ] {
        let result = query(reader, mode, text, &control);
        assert!(
            result.result().complete,
            "{mode:?}: {:?}",
            result.result().coverage
        );
        assert_eq!(result.result().hits.len(), 1);
        assert_eq!(result.result().hits[0].key.id, "first-source");
        assert_eq!(result.result().hits[0].digest, digest(TEXT));
        assert!(matches!(
            &result.result().hits[0].locator,
            Locator::Text { original: Some(original), .. } if original == "pages/source"
        ));
        if mode == Mode::Exact {
            assert_eq!(
                result.result().hits[0].locator,
                Locator::Text {
                    start: 6,
                    end: 20,
                    original: Some("pages/source".into())
                }
            );
        }
        if mode == Mode::Regex {
            assert_eq!(
                result.result().hits[0].locator,
                Locator::Text {
                    start: 0,
                    end: 20,
                    original: Some("pages/source".into())
                }
            );
        }
        assert_eq!(result.result().generations["archive-shard"], generation_id);
        result.check().unwrap();
        reader = result.into_generation();
    }
    let cancellation_recipe =
        ContentDigest::new(digest(b"real cancellable native archive query")).unwrap();
    let cancellation_key = Key {
        recipe: &cancellation_recipe,
        ..key
    };
    account
        .submit(Submission {
            operation_id: "archive-stop",
            job_id: "archive-stop-job",
            key: cancellation_key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let stopping_claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let stopping_work = Work::split_claim(&stopping_claim, cancellation_key, need).unwrap();
    let stopping_control = claim_control(source.clone(), stopping_claim.clone(), cancellation_key);
    let stopping_reader = StoredGeneration::open(
        source.clone(),
        &requirements,
        "archive-shard",
        BUILD,
        "indexes/native",
        plan,
        stopping_work,
        &bounds(),
        &AtomicBool::new(false),
        &stopping_control,
    )
    .unwrap();
    let stopping_result = query(stopping_reader, Mode::Lexical, "alpha", &stopping_control);
    account
        .queue()
        .cancel(stopping_claim.job(), cancellation_key)
        .unwrap();
    source.check().unwrap();
    assert!(
        stopping_result.check().is_err(),
        "actual Claim cancellation, not a changed source or policy, blocks delivery"
    );
    let stopping_reader = stopping_result.into_generation();
    assert!(
        stopping_reader
            .search(
                Query {
                    mode: Mode::Lexical,
                    query: "alpha".into(),
                    scopes: vec![view()],
                    limit: 10
                },
                &control
            )
            .is_err()
    );
    drop(stopping_control);
    account
        .queue()
        .settle(&stopping_claim, cancellation_key, Outcome::Cancelled)
        .unwrap();
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        baseline,
        "stopped native reader costs settle only after their real owners drop"
    );
    let delivered = query(reader, Mode::Lexical, "alpha", &control);
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        baseline,
        "result owner retains the actual native reader charge"
    );
    let renamed = selected_parent.path().join("old-selected");
    fs::rename(&selected_root, &renamed).unwrap();
    fs::create_dir(&selected_root).unwrap();
    assert!(
        delivered.check().is_err(),
        "held FDs do not authorize a replaced selected parent"
    );
    fs::remove_dir(&selected_root).unwrap();
    fs::rename(&renamed, &selected_root).unwrap();
    delivered.check().unwrap();

    // Revoke a real native selection DURING initial M5 verification, not just
    // before open or after query. A later bad CRC makes an unguarded full pass
    // observably different: it fails checksum without ever reaching revocation.
    let bad_path = selected_root.join("bad.zip");
    fs::copy(&archive, &bad_path).unwrap();
    let offset = {
        let mut zip = zip::ZipArchive::new(File::open(&bad_path).unwrap()).unwrap();
        zip.by_name(SOURCE).unwrap().data_start()
    };
    {
        use std::os::unix::fs::FileExt;
        let file = fs::OpenOptions::new().write(true).open(&bad_path).unwrap();
        file.write_all_at(b"!", offset).unwrap();
        file.sync_all().unwrap();
    }
    let permit = runtime.native_scope(&session, &command, None).unwrap();
    let bad_source = Arc::new(SelectedArchive {
        file: File::open(&bad_path).unwrap(),
        permit,
        library: library.clone(),
        path: RelativePath::new("bad.zip").unwrap(),
    });
    let bad_version = ContentDigest::new(digest(&fs::read(&bad_path).unwrap())).unwrap();
    let bad_key = Key {
        source: "bad.zip",
        version: &bad_version,
        ..key
    };
    account
        .submit(Submission {
            operation_id: "archive-refill-revoke",
            job_id: "archive-refill-revoke-job",
            key: bad_key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let bad_claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let bad_work = Work::split_claim(&bad_claim, bad_key, need).unwrap();
    let checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let check_count = checks.clone();
    let native_owner = owner.clone();
    let native_selection = selection.clone();
    let mut revoked_policy = policy.clone();
    revoked_policy.excluded_paths = vec![RelativePath::new("bad.zip").unwrap()];
    let bad_current = claim_control(bad_source.clone(), bad_claim.clone(), bad_key);
    let revoke = Control::guarded(
        bad_claim.deadline(),
        Arc::new(move || {
            let n = check_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if n == 16 {
                native_owner
                    .set_policy(&native_selection, revoked_policy.clone())
                    .unwrap();
            }
            bad_current.check().is_ok()
        }),
    );
    let opening = StoredGeneration::open(
        bad_source.clone(),
        &requirements,
        "archive-shard",
        BUILD,
        "indexes/native",
        plan,
        bad_work,
        &bounds(),
        &AtomicBool::new(false),
        &revoke,
    );
    assert!(
        matches!(
            opening,
            Err(Error::Package(knowledge_package::Error::Cancelled))
        ),
        "initial verification returned {:?}",
        opening.as_ref().err(),
    );
    assert_eq!(checks.load(std::sync::atomic::Ordering::SeqCst), 16);
    assert!(
        bad_source.check().is_err(),
        "actual owner revocation took effect inside verification"
    );
    account.queue().cancel(bad_claim.job(), bad_key).unwrap();
    account
        .queue()
        .settle(&bad_claim, bad_key, Outcome::Cancelled)
        .unwrap();
    drop(revoke);
    drop(bad_source);
    drop(bad_claim);
    owner
        .set_policy(
            &selection,
            LocalPolicy {
                excluded_paths: vec![RelativePath::new("frozen.zip").unwrap()],
                ..policy
            },
        )
        .unwrap();
    assert!(
        delivered.check().is_err(),
        "current native policy revocation withholds old result bytes"
    );
    drop(delivered);
    drop(source);
    drop(guarded);
    account
        .queue()
        .settle(&claim, key, Outcome::Completed)
        .unwrap();
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    assert!(account.status().unwrap().reserved.memory_bytes < baseline - need.memory_bytes / 2);
    assert!(matches!(
        Work::reserve(
            Reservation {
                memory_bytes: 2 * 1024 * 1024 * 1024 + 1,
                temporary_bytes: 0
            },
            Origin::BeforeVerification
        ),
        Err(Error::Admission(
            knowledge_core::admission::Error::TooLargeMemory
        ))
    ));
    let used = account.status().unwrap().reserved.memory_bytes;
    assert!(matches!(
        Work::reserve(
            Reservation {
                memory_bytes: 2 * 1024 * 1024 * 1024 - used + 1,
                temporary_bytes: 0
            },
            Origin::BeforeVerification
        )
        .unwrap(),
        Decision::Waiting(_)
    ));
}
