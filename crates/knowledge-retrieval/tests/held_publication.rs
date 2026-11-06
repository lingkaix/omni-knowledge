//! Actual M5 held input -> SAME native Tantivy publisher; no loose input/extraction fallback.
#[path = "stored_generation/support.rs"]
mod support;
use knowledge_retrieval::{ByteDirectory, ByteHandle, StoredDirectory, StoredDirectoryPlan};
use std::sync::atomic::{AtomicUsize, Ordering};
use support::*;

#[derive(Debug)]
struct Inputs {
    actual: Arc<dyn ByteDirectory>,
    opens: Arc<AtomicUsize>,
    member: Option<&'static str>,
}
impl ByteDirectory for Inputs {
    fn check(&self) -> Result<(), retrieval_kernel::Error> {
        self.actual.check()
    }
    fn exists(&self, name: &str) -> Result<bool, retrieval_kernel::Error> {
        self.actual.exists(name)
    }
    fn open(&self, name: &str) -> Result<Arc<dyn ByteHandle>, retrieval_kernel::Error> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        // Fault input is still an actual M5 verified archive range, never canned read results.
        self.actual.open(self.member.unwrap_or(name))
    }
}

#[test]
fn held_archive_inputs_publish_without_reopen_or_late_global_setup() {
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
    let loose = tempfile::tempdir().unwrap();
    fs::create_dir_all(loose.path().join("indexes/native")).unwrap();
    let source_path = loose.path().join(SOURCE);
    fs::write(&source_path, TEXT).unwrap();
    let seed_store = cache.path().join("seed");
    let kernel = KernelAdapter::open(loose.path(), &seed_store, BUILD).unwrap();
    let control = Control::new(Instant::now() + Duration::from_secs(60));
    let mut seed_work = reserve(KernelAdapter::publish_requirement(4096).unwrap());
    let seed = kernel
        .publish(view(), None, &mut seed_work, &control)
        .unwrap();
    drop(seed_work);
    let seed_scope = fs::read_dir(&seed_store)
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
        &seed_scope.join(&seed),
        &source_path,
        &seed,
        account,
    );
    drop(kernel);
    fs::remove_dir_all(&seed_store).unwrap();
    fs::remove_file(&source_path).unwrap();
    assert!(!loose.path().join(SOURCE).exists());

    let (authority, owner) = NativeAuthority::new();
    let owner = Arc::new(owner);
    let selected_path = selected_root.clone();
    let library = Id::new("held-publication-library").unwrap();
    let selection = owner
        .select_current(
            library.clone(),
            File::open(&selected_root).unwrap(),
            LocalPolicy {
                mode: ExposureMode::ReadWrite,
                sensitive: false,
                excluded_paths: vec![],
                transports: vec![Transport::OwnerStdio],
                http_hosts: vec![],
                http_origins: vec![],
            },
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
    let command=Command::Refresh{
        library_id:library.clone(),mount_ids:vec![],verification:omni_service::Verification::FullHash,embeddings:false,
        budget:serde_json::from_value(serde_json::json!({"max_files":100_000,"max_bytes":"134217728","max_millis":60_000,"max_memory_bytes":"1073741824"})).unwrap(),
        operation_id:Id::new("held-publication").unwrap(),
    };
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
    let recipe = ContentDigest::new(digest(b"actual held archive publication")).unwrap();
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
    let plan = StoredDirectoryPlan {
        index_bytes: 2 * 1024 * 1024,
        table_bytes: 2 * 1024 * 1024,
        verification_memory: 4 * 1024 * 1024,
        read_window_bytes: 64 * 1024,
    };
    let directory_need = plan.requirement().unwrap();
    let publication_need = KernelAdapter::publish_requirement(4096).unwrap();
    let setup_need = Reservation {
        memory_bytes: 64 * 1024,
        temporary_bytes: 0,
    };
    let need = Reservation {
        memory_bytes: directory_need.memory_bytes
            + publication_need.memory_bytes
            + setup_need.memory_bytes,
        temporary_bytes: publication_need.temporary_bytes,
    };
    account
        .submit(Submission {
            operation_id: "held-publication",
            job_id: "held-publication-job",
            key,
            budget: bounds().budget,
            reservation: need,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let directory_work = Work::split_claim(&claim, key, directory_need).unwrap();
    let mut publication = Work::split_claim(&claim, key, publication_need).unwrap();
    let mut setup = Work::split_claim(&claim, key, setup_need).unwrap();
    let guarded = claim_control(source.clone(), claim.clone(), key);
    let pressure = reserve(Reservation {
        memory_bytes: account.status().unwrap().memory_available,
        temporary_bytes: 0,
    });
    let identity = StoredIdentity::new(BUILD).unwrap();
    let requirements = identity.requirements(&seed);
    let reader = StoredDirectory::open(
        source.clone(),
        &requirements,
        BUILD,
        plan,
        directory_work,
        &bounds(),
        &AtomicBool::new(false),
        &guarded,
    )
    .unwrap();
    let directory = reader.directory("").unwrap();
    let output = cache.path().join("held-output");
    let opens = Arc::new(AtomicUsize::new(0));
    let inputs = Arc::new(Inputs {
        actual: directory.clone(),
        opens: opens.clone(),
        member: None,
    });
    let native =
        KernelAdapter::open_held(loose.path(), &output, BUILD, inputs, &setup, &guarded).unwrap();
    let mut expected = None;
    for representation in [false, true] {
        let mut current = view();
        if representation {
            current.documents[0].content = Content::Representation {
                path: SOURCE.into(),
                digest: digest(TEXT),
                size: TEXT.len() as u64,
                processor: digest(BUILD.as_bytes()),
                fragments: vec![retrieval_kernel::RepresentationFragment {
                    start: 0,
                    end: TEXT.len() as u64,
                    locator: "pages/source".into(),
                }],
                complete: true,
            };
        }
        let before = opens.load(Ordering::SeqCst);
        let generation = native
            .publish(current, expected.clone(), &mut publication, &guarded)
            .unwrap();
        assert_eq!(
            opens.load(Ordering::SeqCst) - before,
            1,
            "one actual held handle, including final verification"
        );
        let scope = fs::read_dir(&output)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let index = tantivy::Index::open_in_dir(scope.join(&generation).join("index")).unwrap();
        let body = index.schema().get_field("body_unicode").unwrap();
        let reader: tantivy::IndexReader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::Manual)
            .try_into()
            .unwrap();
        let query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_text(body, "alpha"),
            tantivy::schema::IndexRecordOption::WithFreqs,
        );
        assert!(
            reader
                .searcher()
                .search(&query, &tantivy::collector::Count)
                .unwrap()
                > 0,
            "real held body reached actual SAME Tantivy postings"
        );
        assert_eq!(account.status().unwrap().memory_available, 0);
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        expected = Some(generation);
    }
    drop(native);
    let scope = fs::read_dir(&output)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let before = fs::read(scope.join("CURRENT")).unwrap();
    let mut current = view();
    current.documents[0].content = Content::Representation {
        path: SOURCE.into(),
        digest: digest(TEXT),
        size: TEXT.len() as u64,
        processor: digest(BUILD.as_bytes()),
        fragments: vec![retrieval_kernel::RepresentationFragment {
            start: 0,
            end: TEXT.len() as u64,
            locator: "pages/source".into(),
        }],
        complete: true,
    };
    // Unchanged labels cannot certify a NEW input owner whose actual bytes are wrong or absent.
    for member in ["indexes/native/manifest.json", "indexes/native/missing.txt"] {
        let calls = Arc::new(AtomicUsize::new(0));
        let inputs = Arc::new(Inputs {
            actual: directory.clone(),
            opens: calls.clone(),
            member: Some(member),
        });
        let native =
            KernelAdapter::open_held(loose.path(), &output, BUILD, inputs, &setup, &guarded)
                .unwrap();
        assert!(
            native
                .publish(
                    current.clone(),
                    expected.clone(),
                    &mut publication,
                    &guarded
                )
                .is_err(),
            "actual wrong/missing member must preserve the prior generation, not blind reuse or successful partial publication: {member}"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "actual native input inspected, not digest/DTO-only cache reuse"
        );
        assert_eq!(fs::read(scope.join("CURRENT")).unwrap(), before);
    }
    let inputs = Arc::new(Inputs {
        actual: directory.clone(),
        opens: Arc::new(AtomicUsize::new(0)),
        member: None,
    });
    let native =
        KernelAdapter::open_held(loose.path(), &output, BUILD, inputs, &setup, &guarded).unwrap();
    // Real selected-directory identity disappears AFTER binding; no synthetic revocation boolean.
    fs::rename(
        &selected_root,
        selected_parent.path().join("revoked-selected"),
    )
    .unwrap();
    assert!(
        native
            .publish(current, expected, &mut publication, &guarded)
            .is_err()
    );
    assert_eq!(fs::read(scope.join("CURRENT")).unwrap(), before);
    assert_eq!(account.status().unwrap().queue.unsettled, 1);
    // Actual synchronous kernel/source/check/FD owners die BEFORE returning their setup/execution loans.
    drop(native);
    drop(directory);
    drop(reader);
    drop(guarded);
    drop(source);
    publication.rejoin(&claim, key).unwrap();
    setup.rejoin(&claim, key).unwrap();
    account
        .queue()
        .settle(&claim, key, Outcome::Failed)
        .unwrap();
    drop(pressure);
}
