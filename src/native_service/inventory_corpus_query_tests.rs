//! Actual cold Query Core job/SDK/M1 ownership; this does not activate application payload queries.
use super::*;
pub(super) fn run(
    service: &NativeLibraryService,
    owner: &Owner,
    permit: &Permit,
    scan: &str,
    path: &str,
    foreign: &NativeInventory,
) {
    let account = Account::current().unwrap();
    let mut scratch = holding(4 * 1024 * 1024, 0);
    let inventory = service
        .inventory_for_query(
            owner,
            permit,
            "wiki",
            scan,
            holding(2 * 1024 * 1024, 0),
            &Preflight {
                holding: &scratch,
                budget: Budget {
                    max_files: 1000,
                    max_bytes: 8 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 4 * 1024 * 1024,
                },
            },
        )
        .unwrap();
    let wrong = observed(foreign, path);
    let reader = service
        .open_corpus_tuple(owner, permit.clone(), ready(NativeTupleRead::requirement()))
        .unwrap();
    let (source_at, partition_at) = reader.test_source_partition(path).unwrap();
    let query = NativeTupleQuery::prepare(
        vec![NativeQueryLibrary::new(reader, inventory, None)],
        ready(NativeTupleQuery::metadata_requirement()),
    )
    .unwrap();
    assert!(!query.descriptor().unwrap().is_empty());
    query
        .submit(NativeJobPlan {
            operation_id: "actual-cold-query",
            job_id: "actual-cold-query-job",
            budget: Budget {
                max_files: 1000,
                max_bytes: 64 * 1024 * 1024,
                max_millis: 60000,
                max_memory_bytes: 192 * 1024 * 1024,
            },
            reservation: Reservation {
                memory_bytes: 192 * 1024 * 1024,
                temporary_bytes: 0,
            },
            priority: knowledge_core::jobs::Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let before = account
        .job_status(claim.job(), query.key())
        .unwrap()
        .execution
        .memory_bytes;
    // Even a synchronous failed preflight must not lose its actual Holding via Holding::drop.
    let before_bad = account.status().unwrap().reserved.memory_bytes;
    let bad = holding(1024, 0);
    assert!(
        query
            .page(
                &claim,
                usize::MAX,
                0,
                Budget {
                    max_files: 1,
                    max_bytes: 1024,
                    max_millis: 1000,
                    max_memory_bytes: 1024
                },
                bad
            )
            .is_err()
    );
    assert_eq!(account.status().unwrap().reserved.memory_bytes, before_bad);
    let mut cursor = 0;
    let (page, actual) = loop {
        let funded = account
            .split_claim(
                &claim,
                query.key(),
                Reservation {
                    memory_bytes: 2 * 1024 * 1024,
                    temporary_bytes: 0,
                },
            )
            .unwrap();
        let page = query
            .page(
                &claim,
                0,
                cursor,
                Budget {
                    max_files: 1000,
                    max_bytes: 8 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 1024 * 1024,
                },
                funded,
            )
            .unwrap();
        let found = page
            .sources()
            .find(|row| row.observation().stat.path.as_str() == path);
        if let Some(actual) = found {
            break (page, actual);
        }
        assert!(!page.complete());
        cursor = page.next_offset();
        let mut funded = page
            .into_funding()
            .unwrap_or_else(|_| panic!("unconsumed Query page remained pinned"));
        account
            .rejoin_claim(&mut funded, &claim, query.key())
            .unwrap();
    };
    // Release the rejected foreign row's separate paid page BEFORE the pressure boundary.
    // Its legitimate earlier metadata refund is not a late reservation by the actual source.
    let before_hash = NativeSource::test_hash_read_calls();
    assert!(
        query.source(service, &claim, 0, wrong, 0).is_err(),
        "foreign opaque M1 owner is not substitutable by equal root/scan data"
    );
    // Actual Ready, not a numeric status quote, is the pressure grant. No effects/errors are retried.
    let mut pressure = loop {
        let s = account.status().unwrap();
        match account
            .reserve(
                Reservation {
                    memory_bytes: s.memory_available,
                    temporary_bytes: s.storage_available,
                },
                Origin::BeforeVerification,
            )
            .unwrap()
        {
            Decision::Ready(owner) => break owner,
            Decision::Waiting(reason) => {
                eprintln!("actual Query fixture pressure waiting: {reason:?}");
                assert!(Instant::now() < claim.deadline());
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    };
    assert_eq!(account.status().unwrap().memory_available, 0);
    let source = query
        .source(service, &claim, 0, actual, source_at)
        .unwrap()
        .unwrap();
    assert_eq!(source.library(), 0);
    assert!(source.next() > 0);
    source.source().check().unwrap();
    assert!(
        Arc::strong_count(&claim) > 1,
        "actual live Query source must retain the real Claim, not only key/number facts"
    );
    assert_eq!(NativeSource::test_hash_read_calls(), before_hash);
    assert_eq!(
        account.status().unwrap().memory_available,
        0,
        "actual Query source cannot require a late global reservation"
    );
    let page = match page.into_funding() {
        Err(page) => page,
        Ok(_) => panic!("actual Query source lost its paid M1 page callback pin"),
    };
    let live = source.source().test_query_scope_clone();
    let stage = query
        .stage(
            claim.clone(),
            0,
            vec![source],
            NativeQueryStageResources {
                window: 1024 * 1024,
            },
        )
        .unwrap();
    let started = Instant::now();
    let (partition, _next) = stage.partition(partition_at).unwrap().unwrap();
    eprintln!(
        "actual native partition open elapsed={:?} guard-profile={:?}",
        started.elapsed(),
        stage.test_profile()
    );
    let searched = partition.lexical(0);
    eprintln!(
        "actual native lexical elapsed={:?} guard-profile={:?} owner-check={:?}",
        started.elapsed(),
        stage.test_profile(),
        stage.check()
    );
    let result = searched.unwrap();
    assert!(
        !result.result().unwrap().hits.is_empty(),
        "actual same-FD/native-source/tuple/kernel query must retrieve admitted alpha: {:?}",
        result.result().unwrap()
    );
    assert_eq!(
        account.status().unwrap().memory_available,
        0,
        "real kernel/component query cannot introduce a global-credit gap"
    );
    let stage = match stage.into_loans() {
        Err(stage) => stage,
        Ok(_) => panic!("live native result must retain actual query/range/source resources"),
    };
    result.check().unwrap();
    account.queue().cancel(claim.job(), query.key()).unwrap();
    assert!(
        result.check().is_err(),
        "actual cancellation must revoke final controllable native result access"
    );
    drop(result);
    let NativeQueryLoans {
        job: stage_job,
        sources,
        work,
        claim: stage_claim,
    } = stage
        .into_loans()
        .unwrap_or_else(|_| panic!("actual native partition still retained after last result"));
    drop((stage_job, stage_claim));
    let source = sources.into_iter().next().unwrap();
    work.expect("actual combined stage loan")
        .rejoin(&claim, query.key())
        .unwrap();
    let scope = match query.finish_source(source, &claim) {
        Err(wire::authority::ScopeRejoinError::Retained { scope, .. }) => scope,
        _ => panic!("live actual SDK scope clone must return owning refusal"),
    };
    assert_eq!(
        account.status().unwrap().queue.unsettled,
        1,
        "source return cannot settle aggregate Query"
    );
    drop(live);
    scope
        .rejoin_processing_scope(&claim, query.key())
        .unwrap_or_else(|_| panic!("actual scope still retained after last SDK clone"));
    let mut funded = page
        .into_funding()
        .unwrap_or_else(|_| panic!("actual Query page still retained after last source/scope"));
    account
        .rejoin_claim(&mut funded, &claim, query.key())
        .unwrap();
    assert_eq!(
        account
            .job_status(claim.job(), query.key())
            .unwrap()
            .execution
            .memory_bytes,
        before,
        "exact source/page rejoin cannot leak or globally release Claim credit"
    );
    pressure.finish().unwrap();
    let claim = Arc::try_unwrap(claim)
        .unwrap_or_else(|_| panic!("actual Query Claim retained after last source/scope"));
    account
        .queue()
        .settle(
            &claim,
            query.key(),
            knowledge_core::jobs::Outcome::Cancelled,
        )
        .unwrap();
    drop(query);
    scratch.finish().unwrap();
}
