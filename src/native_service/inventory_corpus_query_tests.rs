//! Actual cold Query Core job/SDK/M1 ownership; this does not activate application payload queries.
use super::*;
pub(super) fn run(
    service: &NativeLibraryService,
    owner: &Owner,
    permit: &Permit,
    scan_and_path: (&str, &str),
    foreign: &NativeInventory,
    mutation: Option<std::path::PathBuf>,
    model: Option<NativeEmbeddingSelection>,
) {
    let (scan, path) = scan_and_path;
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
    let partition_sources = reader.test_partition_sources(partition_at).unwrap();
    eprintln!(
        "actual native partition distinct source receipts={}",
        partition_sources.len()
    );
    let need = partition_sources
        .iter()
        .map(|(at, next, _, path)| (next - at) * 16 + path.len() as u64 * 32 + 1024 * 1024)
        .sum::<u64>()
        + NativeQueryStageResources::requirement(1024 * 1024)
            .unwrap()
            .memory_bytes
        + 18 * 1024 * 1024;
    let need = need.max(192 * 1024 * 1024);
    let semantic = model.is_some();
    let query = NativeTupleQuery::prepare(
        vec![NativeQueryLibrary::new(reader, inventory, model)],
        ready(NativeTupleQuery::metadata_requirement()),
    )
    .unwrap();
    assert!(!query.descriptor().unwrap().is_empty());
    let vector_need = semantic.then(|| query.vector_requirement(0).unwrap());
    query
        .submit(NativeJobPlan {
            operation_id: "actual-cold-query",
            job_id: "actual-cold-query-job",
            budget: Budget {
                max_files: 1000,
                max_bytes: 64 * 1024 * 1024,
                max_millis: 60000,
                max_memory_bytes: need,
            },
            reservation: Reservation {
                memory_bytes: need,
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
    let refused = match query.page(
        &claim,
        usize::MAX,
        0,
        Budget {
            max_files: 1,
            max_bytes: 1024,
            max_millis: 1000,
            max_memory_bytes: 1024,
        },
        bad,
    ) {
        Ok(_) => panic!("foreign library page must refuse"),
        Err(error) => error.downcast::<NativeQueryPageRefusal>().unwrap(),
    };
    assert_eq!(
        account.status().unwrap().reserved.memory_bytes,
        before_bad + 1024,
        "actual input credit must remain owned until explicit caller cleanup"
    );
    let (_, loan) = refused.into_parts();
    match loan {
        NativeQueryPageLoan::Funding(mut actual) => actual.finish().unwrap(),
        NativeQueryPageLoan::Page(_) => panic!("foreign library refusal cannot publish a page"),
    }
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
    let embedding_before = account
        .job_status(claim.job(), query.key())
        .unwrap()
        .execution;
    let mut embedding = query.embed_keyword(claim.clone(), 0, 0);
    if semantic {
        assert!(
            embedding.failure().is_none(),
            "actual selected Query/model/Claim embedding: {:?}",
            embedding.failure()
        );
        assert_eq!(
            embedding.vector().unwrap(),
            &[5.0, 1.0],
            "numeric contract-only ALPHA, not semantic quality/loaded WeMM"
        );
        let available = account
            .job_status(claim.job(), query.key())
            .unwrap()
            .execution;
        let mut blocked = account
            .split_claim(
                &claim,
                query.key(),
                Reservation {
                    memory_bytes: available.memory_bytes - 1,
                    temporary_bytes: 0,
                },
            )
            .unwrap();
        assert!(
            embedding.materialize().is_err(),
            "one byte cannot pay the next vector buffer"
        );
        assert_eq!(
            account
                .job_status(claim.job(), query.key())
                .unwrap()
                .execution
                .memory_bytes,
            1
        );
        assert_eq!(embedding.vector().unwrap(), &[5.0, 1.0]);
        account
            .rejoin_claim(&mut blocked, &claim, query.key())
            .unwrap();
        embedding.materialize().unwrap();
        assert_eq!(embedding.vector().unwrap(), &[5.0, 1.0]);
        let copied = account
            .job_status(claim.job(), query.key())
            .unwrap()
            .execution;
        assert_eq!(
            copied.memory_bytes,
            embedding_before.memory_bytes - 4112,
            "actual provider dies/rejoins while the new live vector keeps its own prepaid buffer"
        );
        embedding.materialize().unwrap();
        assert_eq!(
            account
                .job_status(claim.job(), query.key())
                .unwrap()
                .execution,
            copied
        );
        assert_eq!(account.status().unwrap().memory_available, 0);
    } else {
        assert!(
            embedding.failure().is_some(),
            "an unselected semantic method cannot contact a receiver"
        );
        assert!(embedding.vector().is_err());
    }
    assert_eq!(account.status().unwrap().memory_available, 0);
    let mut semantic_embedding = if semantic {
        Some(embedding)
    } else {
        let (actual_query, actual_claim) = embedding
            .finish()
            .unwrap_or_else(|_| panic!("actual Query embedding owner remains live"));
        assert!(Arc::ptr_eq(&actual_claim, &claim));
        assert_eq!(
            account
                .job_status(claim.job(), query.key())
                .unwrap()
                .execution,
            embedding_before,
            "actual failed provider loan returns to the SAME Claim"
        );
        drop((actual_query, actual_claim));
        None
    };
    assert_eq!(account.status().unwrap().memory_available, 0);
    let remaining = account
        .job_status(claim.job(), query.key())
        .unwrap()
        .execution
        .memory_bytes;
    let mut spare = Work::split_claim(
        &claim,
        query.key(),
        Reservation {
            memory_bytes: remaining - 2 * 1024 * 1024,
            temporary_bytes: 0,
        },
    )
    .unwrap();
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
    spare.rejoin(&claim, query.key()).unwrap();
    let empty = query
        .stage(
            claim.clone(),
            0,
            vec![],
            NativeQueryStageResources {
                window: 1024 * 1024,
            },
        )
        .unwrap();
    let opened = empty.test_component_opens();
    let skipped = empty.partition(partition_at).unwrap().unwrap();
    assert!(skipped.partition.is_none());
    assert!(skipped.next > partition_at);
    drop(skipped);
    if let Some(embedding) = semantic_embedding.as_ref() {
        let run = empty.semantic(embedding, vector_need.unwrap());
        assert!(run.failure().is_none());
        assert!(run.hits().unwrap().is_empty());
        run.finish()
            .unwrap_or_else(|_| panic!("actual empty semantic stage still retained"));
    }
    assert_eq!(
        empty.test_component_opens(),
        opened,
        "empty current source eligibility must not open generation payload/manifest/components"
    );
    let loans = empty
        .into_loans()
        .unwrap_or_else(|_| panic!("empty stage retained a payload owner"));
    loans.work.unwrap().rejoin(&claim, query.key()).unwrap();
    drop((loans.job, loans.claim, loans.sources));
    let mut sources = vec![source];
    for (at, _, id, _) in partition_sources {
        if at == source_at {
            continue;
        }
        let observed = page
            .sources()
            .find(|s| s.source_id() == id)
            .expect("actual same-page source row");
        sources.push(
            query
                .source(service, &claim, 0, observed, at)
                .unwrap()
                .unwrap(),
        );
    }
    let stage = query
        .stage(
            claim.clone(),
            0,
            sources,
            NativeQueryStageResources {
                window: 1024 * 1024,
            },
        )
        .unwrap();
    let semantic_run = if let Some(embedding) = semantic_embedding.take() {
        let before_reader = account
            .job_status(claim.job(), query.key())
            .unwrap()
            .execution;
        let need = vector_need.unwrap();
        let actual_bounds = stage.test_vector_bounds();
        let run = stage.semantic(&embedding, need);
        assert!(
            run.failure().is_none(),
            "actual native model/vector/source bridge: {:?}",
            run.failure()
        );
        assert!(
            !run.hits().unwrap().is_empty(),
            "actual selected numeric model and original scopes must reach mature cosine"
        );
        assert!(
            run.hits()
                .unwrap()
                .iter()
                .all(|(id, _)| actual_bounds.iter().any(|b| &b.0 == id)),
            "a mature vector rank from outside the ACTUAL admitted native source set is not eligible"
        );
        let mut evidence = 0;
        run.visit_evidence(|hit| {
            assert!(hit.locator.starts_with("representation:"));
            let (start, end) = hit
                .locator
                .rsplit_once('@')
                .unwrap()
                .1
                .split_once("..")
                .unwrap();
            let (start, end) = (start.parse::<u64>().unwrap(), end.parse::<u64>().unwrap());
            assert!(
                actual_bounds
                    .iter()
                    .any(|(id, digest, a, b)| id == &hit.source_id
                        && digest == &hit.representation.digest
                        && *a <= start
                        && start < end
                        && end <= *b),
                "equal-byte representation windows must keep their ACTUAL admitted global span"
            );
            evidence += 1;
            Ok(())
        })
        .unwrap();
        assert!(evidence >= run.hits().unwrap().len());
        assert_eq!(
            account
                .job_status(claim.job(), query.key())
                .unwrap()
                .execution
                .memory_bytes,
            before_reader.memory_bytes - need.memory_bytes
        );
        assert_eq!(account.status().unwrap().memory_available, 0);
        // The mature result has its OWN paid query-vector copy. Only now may this earlier
        // materialized input die and its actual buffer loan return to the continuing Claim.
        let (actual_query, actual_claim) = embedding
            .finish()
            .unwrap_or_else(|_| panic!("actual native semantic input still retained"));
        assert!(Arc::ptr_eq(&actual_claim, &claim));
        drop((actual_query, actual_claim));
        Some(run)
    } else {
        None
    };
    let started = Instant::now();
    let partition = stage
        .partition(partition_at)
        .unwrap()
        .unwrap()
        .partition
        .expect("actual eligible source partition");
    eprintln!(
        "actual native partition open elapsed={:?} guard-profile={:?}",
        started.elapsed(),
        stage.test_profile()
    );
    let mutating = mutation.is_some();
    let changed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Some(path) = mutation {
        let changed = changed.clone();
        stage.test_on_index_open(move || {
            let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
            std::io::Write::write_all(&mut file, b"\nactual mid-query source mutation\n").unwrap();
            file.sync_all().unwrap();
            changed.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }
    let searched = partition.lexical(0);
    eprintln!(
        "actual native lexical elapsed={:?} guard-profile={:?} owner-check={:?}",
        started.elapsed(),
        stage.test_profile(),
        stage.check()
    );
    let stage = if mutating {
        assert!(
            changed.load(std::sync::atomic::Ordering::SeqCst),
            "actual index IO mutation hook must execute"
        );
        assert!(
            searched.is_err(),
            "a source changed DURING mature index IO must revoke the whole native result before delivery"
        );
        drop(searched);
        account.queue().cancel(claim.job(), query.key()).unwrap();
        stage
    } else {
        let result = searched.unwrap();
        assert!(
            !result.result().unwrap().hits.is_empty(),
            "actual same-FD/native-source/tuple/kernel query must retrieve admitted alpha: {:?}",
            result.result().unwrap()
        );
        // SAME actual SourceGeneration, admitted source/FD/Claim and working envelope. Destroy
        // the previous result before another scan; neither raw occurrences nor this one-source
        // primitive claim to be a complete all-method/library response.
        let literal = result
            .into_partition()
            .scan(wire::SearchMethod::Literal, 0, 0)
            .unwrap();
        assert!(
            !literal.result().unwrap().hits.is_empty(),
            "actual case-insensitive ALPHA literal"
        );
        assert_eq!(
            literal.result().unwrap().mode,
            knowledge_retrieval::Mode::Regex
        );
        let escaped = literal
            .into_partition()
            .scan(wire::SearchMethod::Literal, 1, 0)
            .unwrap();
        assert!(
            escaped.result().unwrap().hits.is_empty(),
            "literal alpha.* must be escaped, not treated as a pattern"
        );
        let result = escaped
            .into_partition()
            .scan(wire::SearchMethod::Regex, 1, 0)
            .unwrap();
        assert!(
            !result.result().unwrap().hits.is_empty(),
            "actual raw alpha.* regex must match the same admitted source"
        );
        assert!(
            result
                .result()
                .unwrap()
                .hits
                .iter()
                .any(|hit| matches!(hit.locator, knowledge_retrieval::Locator::Text { .. })),
            "mature original representation occurrence coordinates survive"
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
        stage
    };
    if let Some(run) = semantic_run {
        assert!(
            run.check().is_err(),
            "actual cancellation revokes native semantic delivery"
        );
        run.finish()
            .unwrap_or_else(|_| panic!("actual semantic reader still retained"));
    }
    let NativeQueryLoans {
        job: stage_job,
        sources,
        work,
        claim: stage_claim,
    } = stage
        .into_loans()
        .unwrap_or_else(|_| panic!("actual native partition still retained after last result"));
    drop((stage_job, stage_claim));
    let mut sources = sources.into_iter();
    let source = sources.next().unwrap();
    for source in sources {
        query
            .finish_source(source, &claim)
            .unwrap_or_else(|_| panic!("actual partition scope retained"));
    }
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

pub(super) fn run_whole_bm25(
    service: &NativeLibraryService,
    owner: &Owner,
    permit: &Permit,
    scan: &str,
    mode: &str,
    first_ranked: Option<&str>,
) {
    let changed = mode == "query-source-change";
    let tiny = mode == "query-byte-budget";
    let range_limited = mode == "query-range-budget";
    let object_limited = mode == "query-object-budget";
    let limited = tiny || range_limited || object_limited;
    let input_limited = mode == "query-input-object-budget";
    let input_before = NativeInventory::test_input_objects();
    let byte_budget = match mode {
        "query-byte-budget" => 1,
        "query-byte-phase" => 22 * 1024 * 1024,
        "query-range-budget" => 20 * 1024 * 1024,
        _ => 64 * 1024 * 1024,
    };
    let account = Account::current().unwrap();
    let mut scratch = holding(4 * 1024 * 1024, 0);
    let inventory = service.inventory_for_query(
        owner,
        permit,
        "wiki",
        scan,
        holding(2 * 1024 * 1024, 0),
        &Preflight {
            holding: &scratch,
            budget: Budget {
                max_files: if input_limited { 70 } else { 1000 },
                max_bytes: 64 * 1024 * 1024,
                max_millis: 60000,
                max_memory_bytes: 4 * 1024 * 1024,
            },
        },
    );
    if input_limited {
        let examined = NativeInventory::test_input_objects() - input_before;
        let error = match inventory {
            Err(error) => error,
            Ok(_) => panic!("actual input object ceiling cannot authorize a complete inventory"),
        };
        scratch.finish().unwrap();
        eprintln!("actual native M1 input objects decoded={examined} under ceiling70");
        assert!(
            examined <= 70,
            "native input pages must admit remaining actual metadata objects BEFORE replay, not inspect over budget then refuse"
        );
        assert_eq!(
            error.downcast_ref::<knowledge_storage::Error>(),
            Some(&knowledge_storage::Error::Budget),
            "input ceiling needs a fixed typed budget refusal, not recovery/source absence"
        );
        return;
    }
    let inventory = inventory.unwrap();
    let reader = service
        .open_corpus_tuple(owner, permit.clone(), ready(NativeTupleRead::requirement()))
        .unwrap();
    let shards = reader.test_header().unwrap()["shards"].as_u64().unwrap();
    let query = NativeTupleQuery::prepare(
        vec![NativeQueryLibrary::new(reader, inventory, None)],
        ready(NativeTupleQuery::metadata_requirement()),
    )
    .unwrap();
    let need = query.bm25_requirement(0).unwrap();
    eprintln!("actual whole BM25 conservative SAME-Claim requirement={need:?} shards={shards}");
    assert!(
        need.memory_bytes <= 512 * 1024 * 1024,
        "actual fixture must fit its unchanged sealed512MiB budget"
    );
    query
        .submit(NativeJobPlan {
            operation_id: "actual-whole-bm25",
            job_id: "actual-whole-bm25-job",
            budget: Budget {
                max_files: if object_limited { 70 } else { 1000 },
                max_bytes: byte_budget,
                max_millis: 60000,
                max_memory_bytes: need.memory_bytes,
            },
            reservation: need,
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
                eprintln!("actual whole Query pressure waiting: {reason:?}");
                assert!(Instant::now() < claim.deadline());
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    };
    let mut walk = query.start_bm25(claim.clone(), 0, 0).unwrap();
    if mode == "query-drop-tail" {
        let observed = Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX));
        assert_eq!(account.status().unwrap().memory_available, 0);
        walk.test_paid_admin_tail(observed.clone());
        drop(walk);
        let available_with_live_data = observed.load(std::sync::atomic::Ordering::SeqCst);
        eprintln!(
            "actual Core global bytes available while the paid8KiB admin buffer was STILL LIVE: {available_with_live_data}"
        );
        account.queue().cancel(claim.job(), query.key()).unwrap();
        pressure.finish().unwrap();
        let claim =
            Arc::try_unwrap(claim).unwrap_or_else(|_| panic!("actual probe Claim still retained"));
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
        assert_eq!(
            available_with_live_data, 0,
            "actual paid administrative bytes must DIE before their Core credit is released"
        );
        return;
    }
    let opened = walk.test_component_opens();
    let metadata_reads = walk.test_frame_read_bytes();
    let part_reads = walk.test_part_reads();
    let object_base = walk.objects_admitted();
    let start = Instant::now();
    let mut stopped = false;
    loop {
        let step = walk.step(service);
        if step.is_failed() {
            if limited {
                stopped = true;
                break;
            }
            panic!(
                "actual whole native BM25 failed while retaining its real owner: {:?}",
                walk.failure()
            )
        }
        if step.is_complete() {
            break;
        }
    }
    eprintln!(
        "actual whole-library BM25 elapsed={:?} partitions={} unavailable_parts={} admitted-data-bytes={} admitted-objects={}",
        start.elapsed(),
        walk.partitions(),
        walk.unavailable_parts(),
        walk.bytes_admitted(),
        walk.objects_admitted()
    );
    if limited {
        assert!(
            stopped,
            "actual Claim data ceiling must refuse before over-budget result delivery"
        );
        if tiny {
            assert!(
                walk.test_frame_read_bytes() - metadata_reads <= 1,
                "actual metadata FD reads must be admitted BEFORE read/retained allocation under the same one-byte Claim"
            );
        }
        assert!(
            walk.budget_exhausted(),
            "actual Claim data limit must be a typed budget refusal, not fabricated source absence"
        );
        if tiny {
            assert_eq!(
                walk.test_component_opens(),
                opened,
                "no generation payload may open after insufficient actual byte admission"
            );
        } else if object_limited {
            let decoded = walk.test_part_reads() - part_reads;
            let pages = walk.test_owned_page_objects();
            let admitted = walk.objects_admitted() - object_base;
            eprintln!(
                "actual runtime source-frame decodes={decoded}, owned M1 metadataobjects={pages}, admitted={admitted}"
            );
            assert!(
                decoded + pages <= admitted,
                "actual source metadata object must be admitted BEFORE native FD decoding, not after duplicate inspections"
            );
            assert!(
                walk.test_frame_read_bytes() > metadata_reads,
                "object control must actually examine native metadata before exhaustion"
            );
            assert!(
                walk.partitions() < shards,
                "insufficient object allowance cannot claim the full walk"
            );
        } else {
            assert!(
                walk.test_component_opens() > opened,
                "range quota control must actually reach mature generation IO"
            );
        }
        let observed = (
            walk.test_component_opens(),
            walk.test_frame_read_bytes(),
            walk.bytes_admitted(),
            walk.objects_admitted(),
        );
        assert!(
            walk.step(service).is_failed(),
            "failed step must never retry effects"
        );
        assert_eq!(
            (
                walk.test_component_opens(),
                walk.test_frame_read_bytes(),
                walk.bytes_admitted(),
                walk.objects_admitted()
            ),
            observed
        );
        account.queue().cancel(claim.job(), query.key()).unwrap();
    } else {
        assert_eq!(
            walk.partitions(),
            shards,
            "every admitted partition must be visited"
        );
        assert!(
            shards > 1,
            "actual fixture must cross source fragments/partitions"
        );
        assert_eq!(
            walk.unavailable_parts(),
            u64::from(changed),
            "real stale noncandidate must be removed BEFORE eligible-only statistics"
        );
        let hits = walk.hits().unwrap().collect::<Vec<_>>();
        assert!(!hits.is_empty());
        assert!(hits.len() <= 10);
        if let Some(expected) = first_ranked {
            assert_eq!(
                hits[0].key.id, expected,
                "an original repeated across partitions must never gain extra fragment votes over the earlier-path single original"
            );
        }
        let unique: std::collections::BTreeSet<_> = hits.iter().map(|hit| &hit.key).collect();
        assert_eq!(
            unique.len(),
            hits.len(),
            "original fragments can never multiply a returned file"
        );
        drop((unique, hits));
        assert_eq!(
            account.status().unwrap().memory_available,
            0,
            "actual whole Query must never reserve or release global credit between partitions"
        );
        let live = walk
            .test_scope_clone()
            .expect("actual retained winning original scope");
        account.queue().cancel(claim.job(), query.key()).unwrap();
        assert!(
            walk.hits().is_err(),
            "actual cancellation must revoke whole-library result access"
        );
        walk = match walk.finish() {
            Err(owner) => owner,
            Ok(_) => panic!("actual live winning SDK clone must retain whole-result cleanup owner"),
        };
        assert_eq!(account.status().unwrap().queue.unsettled, 1);
        drop(live);
    }
    let (returned, alias) = walk.finish().unwrap_or_else(|_| {
        panic!("actual whole Query remained pinned after last winning source consumer")
    });
    drop((returned, alias));
    assert_eq!(
        account
            .job_status(claim.job(), query.key())
            .unwrap()
            .execution
            .memory_bytes,
        before,
        "every whole-query/page/source/output loan must return exactly to SAME Claim"
    );
    assert_eq!(account.status().unwrap().memory_available, 0);
    pressure.finish().unwrap();
    let claim = Arc::try_unwrap(claim).unwrap_or_else(|_| {
        panic!("whole Query Claim still retained after actual final consumers")
    });
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

struct MetadataFixtureCleanup<'a>(&'a std::path::Path);
impl Drop for MetadataFixtureCleanup<'_> {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.join("sources"));
        let _ = fs::remove_dir_all(self.0.join(".omni/metadata"));
    }
}
struct LiveQuery {
    query: NativeTupleQuery,
    claim: Arc<knowledge_core::jobs::Claim>,
    scratch: Holding,
    job: u64,
}
impl LiveQuery {
    fn finish(self) {
        let LiveQuery {
            query,
            claim,
            mut scratch,
            job,
        } = self;
        let claim =
            Arc::try_unwrap(claim).unwrap_or_else(|_| panic!("metadata query claim retained"));
        let account = Account::current().unwrap();
        account
            .queue()
            .settle(
                &claim,
                query.key(),
                knowledge_core::jobs::Outcome::Cancelled,
            )
            .unwrap();
        account
            .queue()
            .retire(job, query.key())
            .unwrap_or_else(|error| panic!("canonical query job was not terminal: {error:?}"));
        drop(query);
        scratch.finish().unwrap();
    }
}
fn metadata_command(
    service: &NativeLibraryService,
    request: &LiveQueryOptions<'_>,
) -> wire::Command {
    wire::decode_request(
        &serde_json::to_vec(&serde_json::json!({
            "wire_version": 1,
            "request_id": "canonical-metadata-controls",
            "command": {
                "op": "query",
                "query": {
                    "library_ids": [service.library_id().as_str()],
                    "keywords": request.keywords,
                    "methods": request.methods,
                    "required_tags": request.required_tags,
                    "case_sensitive": request.case_sensitive,
                    "limit": request.limit,
                    "budget": {
                        "max_files": request.max_files,
                        "max_bytes": request.max_bytes.to_string(),
                        "max_millis": 120000,
                        "max_memory_bytes": "1073741824"
                    },
                    "image_query": null
                }
            }
        }))
        .unwrap(),
    )
    .unwrap()
    .command
}
struct LiveQueryOptions<'a> {
    keywords: &'a [&'a str],
    methods: &'a [&'a str],
    required_tags: &'a [&'a str],
    case_sensitive: bool,
    limit: u64,
    max_files: u64,
    max_bytes: u64,
    max_millis: u64,
    bm25: bool,
    job: &'a str,
}
fn open_live(
    service: &NativeLibraryService,
    owner: &Owner,
    session: &Session,
    scan: &str,
    request: LiveQueryOptions<'_>,
) -> LiveQuery {
    let command = metadata_command(service, &request);
    let permit = service.native_scope(session, &command, None).unwrap();
    let scratch = holding(4 * 1024 * 1024, 0);
    let inventory = service
        .inventory_for_query(
            owner,
            &permit,
            "wiki",
            scan,
            holding(2 * 1024 * 1024, 0),
            &Preflight {
                holding: &scratch,
                budget: Budget {
                    max_files: request.max_files,
                    max_bytes: request.max_bytes.min(8 * 1024 * 1024),
                    max_millis: 60000,
                    max_memory_bytes: 4 * 1024 * 1024,
                },
            },
        )
        .unwrap();
    let reader = service
        .open_corpus_tuple(owner, permit, ready(NativeTupleRead::requirement()))
        .unwrap();
    let query = NativeTupleQuery::prepare(
        vec![NativeQueryLibrary::new(reader, inventory, None)],
        ready(NativeTupleQuery::metadata_requirement()),
    )
    .unwrap();
    let reservation = if request.bm25 {
        query.bm25_requirement(0).unwrap()
    } else {
        Reservation {
            memory_bytes: (8 * 1024 * 1024u64)
                .saturating_add(request.limit.saturating_mul(2 * 1024 * 1024))
                .saturating_add(16 * 1024 * 1024),
            temporary_bytes: 0,
        }
    };
    let submitted = query
        .submit(NativeJobPlan {
            operation_id: request.job,
            job_id: request.job,
            budget: Budget {
                max_files: request.max_files,
                max_bytes: request.max_bytes,
                max_millis: request.max_millis,
                max_memory_bytes: reservation.memory_bytes,
            },
            reservation,
            priority: knowledge_core::jobs::Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    assert!(
        submitted.created,
        "canonical query {} must admit a new job",
        request.job
    );
    let claim = Arc::new(
        Account::current()
            .unwrap()
            .queue()
            .claim(Instant::now())
            .unwrap()
            .unwrap_or_else(|| panic!("canonical metadata query was not claimable")),
    );
    LiveQuery {
        query,
        claim,
        scratch,
        job: submitted.job,
    }
}
fn drive_metadata(
    service: &NativeLibraryService,
    live: &LiveQuery,
) -> (Vec<NativeQueryAnnotations>, u64) {
    let mut run = live.query.start_metadata(live.claim.clone(), 0).unwrap();
    loop {
        let step = run.step(service);
        if step.is_failed() {
            panic!("canonical metadata failed: {:#}", run.failure().unwrap());
        }
        if step.is_complete() {
            break;
        }
    }
    let hits = run.hits().unwrap().cloned().collect::<Vec<_>>();
    let unavailable = run.unavailable_sources();
    let (query, claim) = run
        .finish()
        .unwrap_or_else(|_| panic!("metadata finish retained a scope or page"));
    drop((query, claim));
    (hits, unavailable)
}
fn drive_bm25(
    service: &NativeLibraryService,
    live: &LiveQuery,
    source_id: Option<&str>,
) -> (Vec<String>, u64, Option<NativeQueryAnnotations>) {
    let mut run = live.query.start_bm25(live.claim.clone(), 0, 0).unwrap();
    loop {
        let step = run.step(service);
        if step.is_failed() {
            panic!(
                "canonical bm25 failed: {:#} bytes={} objects={}",
                run.failure().unwrap(),
                run.bytes_admitted(),
                run.objects_admitted()
            );
        }
        if step.is_complete() {
            break;
        }
    }
    let ids = run
        .hits()
        .unwrap()
        .map(|hit| hit.key.id.clone())
        .collect::<Vec<_>>();
    let unavailable = run.unavailable_parts();
    let annotations = source_id.map(|id| {
        run.source_annotations(id)
            .unwrap()
            .cloned()
            .unwrap_or_else(|| panic!("missing annotations for {id}; hits={ids:?}"))
    });
    let (query, claim) = run
        .finish()
        .unwrap_or_else(|_| panic!("bm25 finish retained a scope or page"));
    drop((query, claim));
    (ids, unavailable, annotations)
}
fn observation_id(inventory: &NativeInventory, wanted: &str) -> String {
    let mut offset = 0u64;
    loop {
        let page = inventory
            .page(
                offset,
                Budget {
                    max_files: 1000,
                    max_bytes: 8 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 1024 * 1024,
                },
                holding(2 * 1024 * 1024, 0),
            )
            .unwrap();
        let next = page.next_offset();
        let complete = page.complete();
        let found = page.observations().find_map(|row| {
            (row.observation().stat.path.as_str() == wanted).then(|| row.source_id().to_owned())
        });
        if let Some(id) = found {
            return id;
        }
        assert!(!complete, "actual observation {wanted} absent");
        assert!(next > offset, "inventory replay made no progress");
        offset = next;
    }
}
fn inventory_page_keys(inventory: &NativeInventory) -> Vec<Vec<(String, String)>> {
    let mut offset = 0u64;
    let mut pages = Vec::new();
    loop {
        let page = inventory
            .page(
                offset,
                Budget {
                    max_files: 1000,
                    max_bytes: 8 * 1024 * 1024,
                    max_millis: 60000,
                    max_memory_bytes: 1024 * 1024,
                },
                holding(2 * 1024 * 1024, 0),
            )
            .unwrap();
        let next = page.next_offset();
        let complete = page.complete();
        pages.push(
            page.observations()
                .map(|row| {
                    (
                        row.observation().stat.path.as_str().to_owned(),
                        row.source_id().to_owned(),
                    )
                })
                .collect(),
        );
        if complete {
            break;
        }
        assert!(next > offset, "inventory replay made no progress");
        offset = next;
    }
    pages
}
fn later_page_has_earlier_key(pages: &[Vec<(String, String)>]) -> bool {
    pages.iter().enumerate().any(|(index, page)| {
        page.iter().any(|key| {
            pages
                .iter()
                .take(index)
                .any(|earlier| earlier.iter().any(|previous| key < previous))
        })
    })
}
pub(super) fn run_semantic_tag_exclusion(
    service: &NativeLibraryService,
    owner: &Owner,
    session: &Session,
    scan: &str,
    model: &NativeEmbeddingSelection,
) {
    let query_rows_before = NativeSemanticRun::test_semantic_query_rows();
    let command = wire::decode_request(
        &serde_json::to_vec(&serde_json::json!({
            "wire_version": 1,
            "request_id": "canonical-semantic-tag-exclusion",
            "command": {
                "op": "query",
                "query": {
                    "library_ids": [service.library_id().as_str()],
                    "keywords": ["alpha"],
                    "methods": ["semantic"],
                    "required_tags": ["missing-semantic-tag"],
                    "case_sensitive": true,
                    "limit": 10,
                    "budget": {
                        "max_files": 1000,
                        "max_bytes": "67108864",
                        "max_millis": 120000,
                        "max_memory_bytes": "1073741824"
                    },
                    "image_query": null
                }
            }
        }))
        .unwrap(),
    )
    .unwrap()
    .command;
    let permit = service.native_scope(session, &command, None).unwrap();
    let mut scratch = holding(4 * 1024 * 1024, 0);
    let inventory = service
        .inventory_for_query(
            owner,
            &permit,
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
    let reader = service
        .open_corpus_tuple(owner, permit, ready(NativeTupleRead::requirement()))
        .unwrap();
    let (source_at, _) = reader.test_source_partition("raw/one.md").unwrap();
    let query = NativeTupleQuery::prepare(
        vec![NativeQueryLibrary::new(
            reader,
            inventory,
            Some(model.clone()),
        )],
        ready(NativeTupleQuery::metadata_requirement()),
    )
    .unwrap();
    let vector_need = query.vector_requirement(0).unwrap();
    let stage_need = NativeQueryStageResources::requirement(1024 * 1024)
        .unwrap()
        .memory_bytes;
    // Stage quote plus the live embedding provider loan, the pinned page, and the source scope.
    let memory = stage_need.saturating_add(160 * 1024 * 1024);
    let account = Account::current().unwrap();
    let submitted = query
        .submit(NativeJobPlan {
            operation_id: "canonical-semantic-tag-exclusion",
            job_id: "canonical-semantic-tag-exclusion-job",
            budget: Budget {
                max_files: 1000,
                max_bytes: 64 * 1024 * 1024,
                max_millis: 120000,
                max_memory_bytes: memory,
            },
            reservation: Reservation {
                memory_bytes: memory,
                temporary_bytes: 0,
            },
            priority: knowledge_core::jobs::Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    assert!(submitted.created);
    let claim = Arc::new(
        account
            .queue()
            .claim(Instant::now())
            .unwrap()
            .expect("semantic tag exclusion was not claimable"),
    );
    let mut cursor = 0u64;
    let (page, observed) = loop {
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
            .find(|row| row.observation().stat.path.as_str() == "raw/one.md");
        if let Some(observed) = found {
            break (page, observed);
        }
        assert!(
            !page.complete(),
            "raw/one.md absent from semantic inventory"
        );
        cursor = page.next_offset();
        let mut funded = page
            .into_funding()
            .unwrap_or_else(|_| panic!("unconsumed semantic page remained pinned"));
        account
            .rejoin_claim(&mut funded, &claim, query.key())
            .unwrap();
    };
    let source = query
        .source(service, &claim, 0, observed, source_at)
        .unwrap()
        .unwrap();
    let embedding = query.embed_keyword(claim.clone(), 0, 0);
    assert!(
        embedding.failure().is_none(),
        "semantic tag exclusion embedding: {:?}",
        embedding.failure()
    );
    let stage = query
        .stage(
            claim.clone(),
            0,
            vec![source],
            NativeQueryStageResources {
                window: 1024 * 1024,
            },
        )
        .unwrap_or_else(|error| panic!("semantic tag exclusion stage: {error}"));
    let run = stage.semantic(&embedding, vector_need);
    let failure = run
        .failure()
        .map(|error| format!("{error:#}"))
        .unwrap_or_else(|| panic!("tagged semantic exclusion reached delivery"));
    assert!(
        failure.contains("tag"),
        "semantic exclusion must refuse before cosine, got {failure}"
    );
    assert_eq!(
        NativeSemanticRun::test_semantic_query_rows(),
        query_rows_before,
        "required-tag mismatch must not reach cosine"
    );
    run.finish()
        .unwrap_or_else(|_| panic!("refused semantic run retained a reader"));
    let (_query, embedding_claim) = embedding
        .finish()
        .unwrap_or_else(|_| panic!("semantic embedding owner remains live"));
    drop(embedding_claim);
    let loans = stage
        .into_loans()
        .unwrap_or_else(|_| panic!("semantic stage retained after the refused run"));
    drop((loans.job, loans.claim));
    for source in loans.sources {
        query
            .finish_source(source, &claim)
            .unwrap_or_else(|_| panic!("semantic source scope retained"));
    }
    loans
        .work
        .expect("semantic stage loan")
        .rejoin(&claim, query.key())
        .unwrap();
    let mut funded = page
        .into_funding()
        .unwrap_or_else(|_| panic!("semantic page still retained after its source"));
    account
        .rejoin_claim(&mut funded, &claim, query.key())
        .unwrap();
    let claim = Arc::try_unwrap(claim).unwrap_or_else(|_| panic!("semantic claim retained"));
    account
        .queue()
        .settle(
            &claim,
            query.key(),
            knowledge_core::jobs::Outcome::Cancelled,
        )
        .unwrap();
    account
        .queue()
        .retire(submitted.job, query.key())
        .unwrap_or_else(|error| panic!("semantic tag exclusion job was not terminal: {error:?}"));
    drop(query);
    scratch.finish().unwrap();
}
pub(super) fn run_canonical_metadata_controls(
    service: &NativeLibraryService,
    owner: &Owner,
    session: &Session,
    source_root: &std::path::Path,
    query_inventory: &NativeInventory,
    stat_inventory: &NativeInventory,
) {
    let _cleanup = MetadataFixtureCleanup(source_root);
    let scan = query_inventory.scan();
    let llm_wiki_id = observed(query_inventory, "wiki/sources/llm-wiki.md")
        .source_id()
        .to_owned();
    let page_001_id = observed(query_inventory, "raw/page-001.md")
        .source_id()
        .to_owned();
    let sources = source_root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    fs::write(
        sources.join(format!("{llm_wiki_id}.md")),
        "---\ntitle: Pointer LLM Wiki\ndescription: Pointer workflow note\ntags: [PointerTag, MixedCase]\n---\nAuthored pointer summary.\n",
    )
    .unwrap();
    let sidecar_name = format!(
        "{}.json",
        hex::encode(Sha256::digest("wiki/sources/llm-wiki.md".as_bytes()))
    );
    let sidecar_dir = source_root.join(".omni/metadata");
    fs::create_dir_all(&sidecar_dir).unwrap();
    fs::write(
        sidecar_dir.join(sidecar_name),
        r#"{"version":1,"path":"wiki/sources/llm-wiki.md","tags":["hidden-sidecar-tag"],"description":"hidden sidecar"}"#,
    )
    .unwrap();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["PointerTag"],
            methods: &["files", "tags"],
            required_tags: &["reference", "pointertag"],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-pointer-union",
        },
    );
    let (hits, _) = drive_metadata(service, &live);
    live.finish();
    let hit = hits
        .iter()
        .find(|hit| hit.path.as_str() == "wiki/sources/llm-wiki.md")
        .unwrap_or_else(|| panic!("pointer wiki missing from metadata hits"));
    assert_eq!(hit.title, "Pointer LLM Wiki");
    assert_eq!(
        hit.tags,
        vec![
            "reference".to_owned(),
            "knowledge".to_owned(),
            "agent".to_owned(),
            "PointerTag".to_owned(),
            "MixedCase".to_owned(),
        ]
    );
    assert!(
        hit.description
            .contains("A workflow for accumulating an agent-maintained personal wiki.")
    );
    assert!(hit.description.contains("Pointer workflow note"));
    assert!(hit.description.contains("Authored pointer summary."));
    let annotation = format!("sources/{llm_wiki_id}.md");
    assert_eq!(
        hit.annotation_path.as_ref().map(|path| path.as_str()),
        Some(annotation.as_str())
    );
    assert_eq!(hit.digest_state, wire::DigestState::Verified);
    assert_eq!(hit.availability, wire::Availability::Available);
    assert!(!hit.tags.iter().any(|tag| tag == "hidden-sidecar-tag"));
    let bytes_before_frontmatter = NativeMetadataRun::test_bytes_before_frontmatter();
    let objects_before_pointer = NativeMetadataRun::test_objects_before_pointer();
    assert!(NativeMetadataRun::test_frontmatter_entries() >= 1);
    assert!(NativeMetadataRun::test_pointer_entries() >= 1);
    let frontmatter_entries = NativeMetadataRun::test_frontmatter_entries();
    let frontmatter_read = NativeMetadataRun::test_frontmatter_max_read();
    let frontmatter_io = NativeMetadataRun::test_frontmatter_io_bytes();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &[],
            methods: &["files", "tags"],
            required_tags: &[],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: bytes_before_frontmatter + 1,
            max_millis: 120000,
            bm25: false,
            job: "canonical-frontmatter-byte-boundary",
        },
    );
    let mut run = live.query.start_metadata(live.claim.clone(), 0).unwrap();
    loop {
        let step = run.step(service);
        if step.is_failed() {
            break;
        }
        assert!(
            !step.is_complete(),
            "frontmatter byte ceiling completed before the reader refused"
        );
    }
    assert!(
        run.budget_exhausted(),
        "frontmatter byte boundary: {:?}",
        run.failure()
    );
    assert!(NativeMetadataRun::test_frontmatter_entries() > frontmatter_entries);
    assert_eq!(
        NativeMetadataRun::test_frontmatter_io_bytes(),
        frontmatter_io,
        "refused frontmatter attempt performed IO"
    );
    assert_eq!(
        NativeMetadataRun::test_frontmatter_last_read(),
        0,
        "refused frontmatter admission performed IO"
    );
    assert!(
        NativeMetadataRun::test_frontmatter_max_read().saturating_sub(frontmatter_read) < 65536,
        "refused frontmatter read jumped by {}",
        NativeMetadataRun::test_frontmatter_max_read().saturating_sub(frontmatter_read)
    );
    assert!(
        run.bytes_admitted() < bytes_before_frontmatter + 65536,
        "refused frontmatter call admitted {}",
        run.bytes_admitted()
    );
    let (query, claim) = run
        .finish()
        .unwrap_or_else(|_| panic!("frontmatter byte boundary finish retained a scope"));
    drop((query, claim));
    live.finish();
    let pointer_entries = NativeMetadataRun::test_pointer_entries();
    let pointer_read = NativeMetadataRun::test_pointer_read_bytes();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &[],
            methods: &["files", "tags"],
            required_tags: &[],
            case_sensitive: false,
            limit: 10,
            max_files: objects_before_pointer,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-pointer-object-boundary",
        },
    );
    let mut run = live.query.start_metadata(live.claim.clone(), 0).unwrap();
    loop {
        let step = run.step(service);
        if step.is_failed() {
            break;
        }
        assert!(
            !step.is_complete(),
            "pointer object ceiling completed without refusing the pointer"
        );
    }
    assert!(
        run.budget_exhausted(),
        "pointer object boundary: {:?}",
        run.failure()
    );
    assert!(NativeMetadataRun::test_pointer_entries() > pointer_entries);
    assert_eq!(NativeMetadataRun::test_pointer_read_bytes(), pointer_read);
    let (query, claim) = run
        .finish()
        .unwrap_or_else(|_| panic!("pointer object boundary finish retained a scope"));
    drop((query, claim));
    live.finish();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["PointerTag"],
            methods: &["files", "tags"],
            required_tags: &["pointertag"],
            case_sensitive: true,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-pointer-case-miss",
        },
    );
    let (hits, _) = drive_metadata(service, &live);
    live.finish();
    assert!(
        hits.is_empty(),
        "case-sensitive required tag must keep exact spelling"
    );
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["PointerTag"],
            methods: &["files", "tags"],
            required_tags: &["PointerTag"],
            case_sensitive: true,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-pointer-case-hit",
        },
    );
    let (hits, _) = drive_metadata(service, &live);
    live.finish();
    assert!(
        hits.iter()
            .any(|hit| hit.path.as_str() == "wiki/sources/llm-wiki.md")
    );
    fs::write(
        sources.join(format!("{page_001_id}.md")),
        "---\ntags: [LiveEditTag]\ndescription: Initial live description\n---\n",
    )
    .unwrap();
    let before_hash = NativeSource::test_hash_read_calls();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["alpha"],
            methods: &["bm25"],
            required_tags: &["LiveEditTag"],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: true,
            job: "canonical-live-edit-initial",
        },
    );
    let (ids, unavailable, annotations) = drive_bm25(service, &live, Some(&page_001_id));
    live.finish();
    assert_eq!(ids, vec![page_001_id.clone()]);
    assert_eq!(
        unavailable, 1,
        "the parent same-inode edit is the only withheld part; tag filtering adds none"
    );
    let annotations = annotations.unwrap();
    assert_eq!(annotations.tags, vec!["LiveEditTag".to_owned()]);
    assert_eq!(annotations.description, "Initial live description");
    fs::write(
        sources.join(format!("{page_001_id}.md")),
        "---\ntags: [UpdatedLiveTag]\ndescription: Updated live description\n---\n",
    )
    .unwrap();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["alpha"],
            methods: &["bm25"],
            required_tags: &["LiveEditTag"],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: true,
            job: "canonical-live-edit-stale",
        },
    );
    let (ids, _, _) = drive_bm25(service, &live, None);
    live.finish();
    assert!(
        ids.is_empty(),
        "stale required tag must not survive a pointer edit"
    );
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["alpha"],
            methods: &["bm25"],
            required_tags: &["UpdatedLiveTag"],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: true,
            job: "canonical-live-edit-current",
        },
    );
    let (ids, _, annotations) = drive_bm25(service, &live, Some(&page_001_id));
    live.finish();
    assert_eq!(ids, vec![page_001_id.clone()]);
    let annotations = annotations.unwrap();
    assert_eq!(annotations.tags, vec!["UpdatedLiveTag".to_owned()]);
    assert_eq!(annotations.description, "Updated live description");
    assert_eq!(NativeSource::test_hash_read_calls(), before_hash);
    let (excluded, _selection) = NativeLibraryService::ordinary_local_owned(
        KnowledgeBase::open(source_root).unwrap(),
        LocalPolicy {
            mode: wire::ExposureMode::ReadWrite,
            sensitive: false,
            excluded_paths: vec![wire::RelativePath::new("raw/page-001.md").unwrap()],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        },
        owner,
    )
    .unwrap();
    let excluded_session = excluded.connect(TransportFacts::OwnerStdio).unwrap();
    let live = open_live(
        &excluded,
        owner,
        &excluded_session,
        scan,
        LiveQueryOptions {
            keywords: &["alpha"],
            methods: &["bm25"],
            required_tags: &["UpdatedLiveTag"],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: true,
            job: "canonical-excluded-bm25",
        },
    );
    let (ids, unavailable, _) = drive_bm25(&excluded, &live, None);
    live.finish();
    assert!(ids.is_empty(), "excluded source must not reach BM25 hits");
    assert_eq!(
        unavailable, 1,
        "excluding page-001 must not add an unavailable part beyond the parent edit"
    );
    let live = open_live(
        &excluded,
        owner,
        &excluded_session,
        scan,
        LiveQueryOptions {
            keywords: &[],
            methods: &["files", "tags"],
            required_tags: &["UpdatedLiveTag"],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-excluded-metadata",
        },
    );
    let (hits, _) = drive_metadata(&excluded, &live);
    live.finish();
    assert!(
        hits.is_empty(),
        "excluded source must not reach metadata hits"
    );
    drop(excluded_session);
    drop(excluded);
    let denied_id = observation_id(stat_inventory, "raw/actually-denied.md");
    fs::write(
        sources.join(format!("{denied_id}.md")),
        "---\ntags: [DeniedAssetTag]\ndescription: Denied asset pointer\n---\n",
    )
    .unwrap();
    let two = source_root.join("raw/two.html");
    let hidden_two = source_root.join("raw/fixture-missing-two");
    fs::rename(&two, &hidden_two).unwrap();
    let live = open_live(
        service,
        owner,
        session,
        stat_inventory.scan(),
        LiveQueryOptions {
            keywords: &[],
            methods: &["files", "tags"],
            required_tags: &[],
            case_sensitive: false,
            limit: 100,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-stat-discovery",
        },
    );
    let (hits, unavailable) = drive_metadata(service, &live);
    live.finish();
    fs::rename(&hidden_two, &two).unwrap();
    let one = hits
        .iter()
        .find(|hit| hit.path.as_str() == "raw/one.md")
        .unwrap_or_else(|| panic!("stat-first raw/one.md missing from metadata hits"));
    assert_eq!(one.digest_state, wire::DigestState::Unverified);
    assert!(one.digest.is_none());
    assert_eq!(one.availability, wire::Availability::Available);
    let denied = hits
        .iter()
        .find(|hit| hit.path.as_str() == "raw/actually-denied.md")
        .unwrap_or_else(|| panic!("denied original missing from metadata hits"));
    assert_eq!(denied.digest_state, wire::DigestState::Unverified);
    assert!(denied.digest.is_none());
    assert_eq!(denied.availability, wire::Availability::Denied);
    assert_eq!(denied.tags, vec!["DeniedAssetTag".to_owned()]);
    assert!(!hits.iter().any(|hit| hit.path.as_str() == "raw/two.html"));
    assert!(unavailable >= 2, "unavailable sources={unavailable}");
    let bound_description = "First\n  indented\nFirst";
    let bound_path = source_root.join("raw/page-002.md");
    let original_page = fs::read(&bound_path).unwrap();
    let bound_id = observed(query_inventory, "raw/page-002.md")
        .source_id()
        .to_owned();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["BoundTag"],
            methods: &["files", "tags"],
            required_tags: &["BoundTag"],
            case_sensitive: true,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-bound-metadata",
        },
    );
    let mut run = live.query.start_metadata(live.claim.clone(), 0).unwrap();
    loop {
        let step = run.step(service);
        if step.is_failed() {
            panic!("bound metadata failed: {:#}", run.failure().unwrap());
        }
        if step.is_complete() {
            break;
        }
    }
    let hits = run.hits().unwrap().cloned().collect::<Vec<_>>();
    let bound = hits
        .iter()
        .find(|hit| hit.path.as_str() == "raw/page-002.md")
        .unwrap_or_else(|| panic!("edited page-002 missing from metadata hits"));
    assert_eq!(bound.description, bound_description);
    assert_eq!(bound.title, "Bound Title");
    assert_eq!(
        bound.tags,
        vec!["BoundTag".to_owned(), "legacy-kept".to_owned()]
    );
    let cloned = run.test_scope_clone().expect("retained metadata scope");
    let run = match run.finish() {
        Err(run) => run,
        Ok(_) => panic!("live metadata scope must block finish"),
    };
    drop(cloned);
    let (query, claim) = run
        .finish()
        .unwrap_or_else(|_| panic!("metadata finish after the scope clone died"));
    drop((query, claim));
    live.finish();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["frontmatter"],
            methods: &["bm25"],
            required_tags: &["BoundTag"],
            case_sensitive: true,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: true,
            job: "canonical-bound-bm25",
        },
    );
    let (ids, _, annotations) = drive_bm25(service, &live, Some(&bound_id));
    live.finish();
    assert!(ids.iter().any(|id| id == &bound_id));
    let annotations = annotations.unwrap();
    assert_eq!(annotations.description, bound_description);
    assert_eq!(annotations.title, "Bound Title");
    assert_eq!(
        annotations.tags,
        vec!["BoundTag".to_owned(), "legacy-kept".to_owned()]
    );
    fs::write(
        sources.join(format!("{bound_id}.md")),
        "---\ndescription: \"First\\n  indented\"\n---\n",
    )
    .unwrap();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["BoundTag"],
            methods: &["files", "tags"],
            required_tags: &["BoundTag"],
            case_sensitive: true,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-bound-pointer-metadata",
        },
    );
    let (hits, _) = drive_metadata(service, &live);
    live.finish();
    let bound = hits
        .iter()
        .find(|hit| hit.path.as_str() == "raw/page-002.md")
        .unwrap_or_else(|| panic!("pointer union lost page-002"));
    assert_eq!(bound.description, bound_description);
    assert_eq!(bound.title, "Bound Title");
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &["frontmatter"],
            methods: &["bm25"],
            required_tags: &["BoundTag"],
            case_sensitive: true,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: true,
            job: "canonical-bound-pointer-bm25",
        },
    );
    let (_, _, annotations) = drive_bm25(service, &live, Some(&bound_id));
    live.finish();
    let annotations = annotations.unwrap();
    assert_eq!(annotations.description, bound_description);
    assert_eq!(annotations.title, "Bound Title");
    let current = fs::read_to_string(&bound_path).unwrap();
    assert!(current.starts_with("---"));
    let mut edited = current.replacen("title: Bound Title\n", "title: Bound Title Edited\n", 1);
    edited.push_str(&"x".repeat(65543usize.saturating_sub(edited.len())));
    assert!(edited.starts_with("---"));
    assert!(edited.len() > 65542);
    assert!(edited.contains("title: Bound Title Edited\n"));
    fs::write(&bound_path, edited).unwrap();
    let refreshed =
        super::refresh_reconciled_inventory(service, owner, session, "canonical-bound-title");
    let edited_scan = refreshed.scan();
    let live = open_live(
        service,
        owner,
        session,
        edited_scan,
        LiveQueryOptions {
            keywords: &["BoundTag"],
            methods: &["files", "tags"],
            required_tags: &["BoundTag"],
            case_sensitive: true,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-bound-edited-metadata",
        },
    );
    let (hits, _) = drive_metadata(service, &live);
    live.finish();
    let bound = hits
        .iter()
        .find(|hit| hit.path.as_str() == "raw/page-002.md")
        .unwrap_or_else(|| panic!("padded page-002 missing from metadata hits"));
    assert_eq!(bound.title, "Bound Title Edited");
    assert_eq!(bound.description, bound_description);
    assert!(NativeMetadataRun::test_frontmatter_entries() >= 1);
    assert_eq!(NativeMetadataRun::test_frontmatter_max_read(), 65536);
    assert_eq!(NativeMetadataRun::test_frontmatter_max_admitted(), 65536);
    drop(refreshed);
    fs::write(&bound_path, original_page).unwrap();
    let pages = inventory_page_keys(query_inventory);
    assert!(pages.len() > 1, "corpus inventory pages={}", pages.len());
    assert!(
        later_page_has_earlier_key(&pages),
        "metadata reclamation needs a later page with an earlier path key"
    );
    let reclaim_scan = scan.to_owned();
    let live = open_live(
        service,
        owner,
        session,
        &reclaim_scan,
        LiveQueryOptions {
            keywords: &[],
            methods: &["files", "tags"],
            required_tags: &[],
            case_sensitive: false,
            limit: 1,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            bm25: false,
            job: "canonical-page-reclamation",
        },
    );
    let mut run = live.query.start_metadata(live.claim.clone(), 0).unwrap();
    let evictions_before = NativeMetadataRun::test_evictions();
    let mut visited = 0u64;
    loop {
        let step = run.step(service);
        visited += 1;
        if step.is_failed() {
            panic!("page reclamation failed: {:#}", run.failure().unwrap());
        }
        if step.is_complete() {
            break;
        }
    }
    assert!(visited > 1, "metadata pages visited={visited}");
    assert!(
        NativeMetadataRun::test_evictions() > evictions_before,
        "a later smaller path must evict the previous metadata candidate"
    );
    assert_eq!(
        run.test_reclaimable_pages(),
        0,
        "evicted metadata pages must be collected while a real candidate remains"
    );
    assert!(
        run.test_pinned_pages() <= 1,
        "pinned metadata pages={}",
        run.test_pinned_pages()
    );
    eprintln!(
        "metadata page reclamation visited={visited} evicted={} pinned={}",
        NativeMetadataRun::test_evictions() - evictions_before,
        run.test_pinned_pages()
    );
    let (query, claim) = run
        .finish()
        .unwrap_or_else(|_| panic!("page reclamation finish retained a scope or page"));
    drop((query, claim));
    live.finish();
    let live = open_live(
        service,
        owner,
        session,
        scan,
        LiveQueryOptions {
            keywords: &[],
            methods: &["files", "tags"],
            required_tags: &[],
            case_sensitive: false,
            limit: 10,
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 1,
            bm25: false,
            job: "canonical-deadline",
        },
    );
    let mut run = live.query.start_metadata(live.claim.clone(), 0).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    assert!(run.step(service).is_failed());
    let (query, claim) = run
        .finish()
        .unwrap_or_else(|_| panic!("deadline metadata finish retained a scope"));
    drop((query, claim));
    live.finish();
}
