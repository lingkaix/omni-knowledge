//! Actual two-source M1/Refresh/cause/Claim/FD/M3 composition, not metadata-as-permission.
use super::*;
#[test]
fn actual_refresh_claim_processes_distinct_native_sources_without_child_jobs() {
    if std::env::var_os("OMNI_REFRESH_CAUSE_CHILD").is_none() {
        for mode in ["normal", "premature", "pressure-race"] {
            let out=std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact","native_service::inventory::tests::job::actual_refresh_claim_processes_distinct_native_sources_without_child_jobs","--nocapture"])
                .env("OMNI_REFRESH_CAUSE_CHILD","1").env("OMNI_REFRESH_CAUSE_MODE",mode).output().unwrap();
            assert!(String::from_utf8_lossy(&out.stdout).contains("running 1 test"));
            assert!(
                out.status.success(),
                "actual {mode} Refresh cause child: {} {}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(home.path(), "Actual aggregate Refresh").unwrap();
    fs::write(
        kb.root.join("raw/first.md"),
        "# First actual source\n日本語 first\n",
    )
    .unwrap();
    fs::write(
        kb.root.join("raw/second.html"),
        "<h1>Second actual source</h1><p>日本語 second</p>",
    )
    .unwrap();
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
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let inventory = reconcile(
        &service,
        &owner,
        refresh(&service, &session, false, "aggregate-scan"),
    );
    let page_budget = Budget {
        max_files: 1000,
        max_bytes: 8 * 1024 * 1024,
        max_millis: 60000,
        max_memory_bytes: 1024 * 1024,
    };
    let page = inventory
        .page(0, page_budget, holding(2 * 1024 * 1024, 0))
        .unwrap();
    assert!(page.complete());
    assert!(page.issue().is_none());
    let first = page
        .sources()
        .find(|s| s.observation().stat.path.as_str() == "raw/first.md")
        .unwrap();
    let second = page
        .sources()
        .find(|s| s.observation().stat.path.as_str() == "raw/second.html")
        .unwrap();
    let expected = [
        (
            first.source_id().to_owned(),
            first.observation().version.as_ref().unwrap().digest.clone(),
        ),
        (
            second.source_id().to_owned(),
            second
                .observation()
                .version
                .as_ref()
                .unwrap()
                .digest
                .clone(),
        ),
    ];
    assert_ne!(expected[0].0, expected[1].0);
    assert_ne!(expected[0].1, expected[1].1);
    // Another real scan/owner, same selected root and source bytes: DTO equality is insufficient.
    let foreign = reconcile(
        &service,
        &owner,
        refresh(&service, &session, false, "foreign-scan"),
    );
    let mut offset = 0;
    let (foreign_page, foreign_row) = loop {
        let page = foreign
            .page(offset, page_budget, holding(2 * 1024 * 1024, 0))
            .unwrap();
        assert!(page.issue().is_none());
        let found = page
            .sources()
            .find(|s| s.observation().stat.path.as_str() == "raw/first.md");
        if let Some(row) = found {
            break (page, row);
        }
        assert!(
            !page.complete(),
            "actual foreign scan row must be found across bounded pages"
        );
        assert!(page.next_offset() > offset);
        offset = page.next_offset();
    };
    // Constructor refusals return the SAME actual metadata credit; existing opaque inventory
    // and command owners remain alive, so this isolated account oracle is exact.
    let account = Account::current().unwrap();
    let same_inventory = || NativeInventory {
        owner: inventory.owner.clone(),
    };
    let allowed = refresh(&service, &session, false, "descriptor-too-small");
    let before = account.status().unwrap().memory_available;
    assert!(
        service
            .prepare_refresh(
                &owner,
                allowed.clone(),
                same_inventory(),
                None,
                holding(1, 0)
            )
            .is_err()
    );
    assert_eq!(account.status().unwrap().memory_available, before);
    let status=wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"not-refresh","command":{"op":"library_status","library_id":service.library_id().as_str()}})).unwrap()).unwrap().command;
    let status = service.native_scope(&session, &status, None).unwrap();
    let before = account.status().unwrap().memory_available;
    assert!(
        service
            .prepare_refresh(
                &owner,
                status.clone(),
                same_inventory(),
                None,
                holding(NativeRefreshJob::metadata_requirement().memory_bytes, 0)
            )
            .is_err(),
        "status metadata cannot authorize an aggregate Refresh"
    );
    assert_eq!(account.status().unwrap().memory_available, before);
    let embed = refresh(&service, &session, true, "missing-actual-model");
    let before = account.status().unwrap().memory_available;
    assert!(
        service
            .prepare_refresh(
                &owner,
                embed.clone(),
                same_inventory(),
                None,
                holding(NativeRefreshJob::metadata_requirement().memory_bytes, 0)
            )
            .is_err(),
        "actual true embedding intent cannot be satisfied by model labels or None"
    );
    assert_eq!(account.status().unwrap().memory_available, before);
    drop((allowed, status, embed));
    let job = service
        .prepare_refresh(
            &owner,
            refresh(&service, &session, false, "aggregate-build"),
            inventory,
            None,
            holding(NativeRefreshJob::metadata_requirement().memory_bytes, 0),
        )
        .unwrap();
    assert_eq!(
        hex::encode(Sha256::digest(job.descriptor().unwrap())),
        job.key().version.as_str()
    );
    assert!(
        expected
            .iter()
            .all(|(id, sha)| id != job.key().source && sha.as_str() != job.key().version.as_str())
    );
    let budget = Budget {
        max_files: 1000,
        max_bytes: 8 * 1024 * 1024,
        max_millis: 60000,
        max_memory_bytes: 128 * 1024 * 1024,
    };
    let submitted = job
        .submit(NativeJobPlan {
            operation_id: "aggregate-build",
            job_id: "aggregate-build",
            budget,
            reservation: Reservation {
                memory_bytes: 128 * 1024 * 1024,
                temporary_bytes: 2 * 1024 * 1024,
            },
            priority: knowledge_core::jobs::Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let account = Account::current().unwrap();
    let mut claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    assert_eq!(claim.job(), submitted.job);
    let available = account.status().unwrap();
    // A real independent ONE-account owner arrives after the numeric status quote.
    let mut raced = std::env::var("OMNI_REFRESH_CAUSE_MODE")
        .is_ok_and(|mode| mode == "pressure-race")
        .then(|| holding(65536, 0));
    // Status is a quote, never a grant. Native volume facts/other owners may change it.
    // Only Waiting (which acquires nothing) is resampled; no effect/error is retried.
    let mut quote = available;
    let mut saw_wait = false;
    let mut pressure = loop {
        assert!(
            Instant::now() < claim.deadline(),
            "actual pressure admission deadline; no native phase started"
        );
        match account
            .reserve(
                Reservation {
                    memory_bytes: quote.memory_available,
                    temporary_bytes: quote.storage_available,
                },
                knowledge_core::admission::Origin::BeforeVerification,
            )
            .unwrap()
        {
            knowledge_core::admission::Decision::Ready(owner) => break owner,
            knowledge_core::admission::Decision::Waiting(reason) => {
                saw_wait = true;
                eprintln!("actual pressure fixture waiting: {reason:?}");
                std::thread::sleep(Duration::from_millis(1));
                quote = account.status().unwrap();
            }
        }
    };
    assert!(
        raced.is_none() || saw_wait,
        "forced real owner must invalidate the earlier status quote"
    );
    let verify = Budget {
        max_files: 1,
        max_bytes: 65536,
        max_millis: 60000,
        max_memory_bytes: 1024 * 1024,
    };
    let before = account
        .job_status(claim.job(), job.key())
        .unwrap()
        .execution;
    assert!(
        job.processing(&claim, foreign_row, verify).is_err(),
        "only SAME actual inventory owner may supply source stages"
    );
    assert_eq!(
        account
            .job_status(claim.job(), job.key())
            .unwrap()
            .execution,
        before
    );
    for (row, (expected_id, expected_digest)) in [first, second].into_iter().zip(expected) {
        let processing = job.processing(&claim, row, verify).unwrap();
        assert!(claim.stop_requested(processing.key()).unwrap().is_none());
        assert!(
            processing
                .submit(NativeJobPlan {
                    operation_id: "forbidden-child",
                    job_id: "forbidden-child",
                    budget,
                    reservation: Reservation {
                        memory_bytes: 1,
                        temporary_bytes: 0
                    },
                    priority: knowledge_core::jobs::Priority::Normal,
                    max_attempts: 1
                })
                .is_err()
        );
        let input = processing.extract_indexed(claim, 65536, None).unwrap();
        assert_eq!(
            input.source_id().unwrap(),
            expected_id,
            "aggregate subject must NEVER replace real M1 UUID"
        );
        assert_eq!(
            input.source_digest().unwrap(),
            expected_digest.as_str(),
            "metadata descriptor is not original bytes"
        );
        assert!(input.text().unwrap().contains("actual source"));
        assert!(input.text().unwrap().contains("日本語"));
        assert_ne!(
            input.representation().unwrap().processor,
            job.key().processor.as_str()
        );
        if std::env::var("OMNI_REFRESH_CAUSE_MODE").is_ok_and(|mode| mode == "premature") {
            let error = match input.complete() {
                Err(error) => error,
                Ok(_) => panic!("one source must NEVER settle the aggregate Refresh as Completed"),
            };
            let (processing, returned, _cause) = error.into_parts();
            assert_eq!(account.status().unwrap().queue.unsettled, 1);
            assert!(job.finish_source(processing, &returned).is_ok());
            pressure.finish().unwrap();
            account
                .queue()
                .settle(&returned, job.key(), knowledge_core::jobs::Outcome::Failed)
                .unwrap();
            return;
        }
        let (processing, returned) = input.finish_stage().unwrap();
        claim = returned;
        match job.finish_source(processing, &claim) {
            Ok(()) => (),
            Err(_) => panic!("actual source scope must rejoin after FD owners die"),
        }
        assert_eq!(
            account.status().unwrap().memory_available,
            0,
            "NO global gap between actual sources"
        );
        assert_eq!(
            account.status().unwrap().queue.unsettled,
            1,
            "ONE aggregate job remains active"
        );
        assert_eq!(
            account
                .job_status(claim.job(), job.key())
                .unwrap()
                .execution,
            before,
            "complete actual phase envelope is reusable"
        );
    }
    pressure.finish().unwrap();
    if let Some(raced) = &mut raced {
        raced.finish().unwrap();
    }
    account
        .queue()
        .settle(&claim, job.key(), knowledge_core::jobs::Outcome::Completed)
        .unwrap();
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    drop((page, foreign_page, foreign, job));
}
