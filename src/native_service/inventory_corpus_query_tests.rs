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
