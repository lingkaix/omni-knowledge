//! Actual two-root tuple/M1 preparation; input counters observe native IO/decodes, not quotes.
use super::*;
use std::os::unix::fs::PermissionsExt;

fn command(
    services: &[NativeLibraryService],
    files: u64,
    bytes: u64,
    millis: u64,
) -> wire::Command {
    wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"two-actual-inputs","command":{"op":"query","query":{
        "library_ids":services.iter().map(|s|s.library_id().as_str()).collect::<Vec<_>>(),"keywords":["alpha"],"methods":["files","bm25"],"required_tags":[],"case_sensitive":false,"limit":10,
        "budget":{"max_files":files,"max_bytes":bytes.to_string(),"max_millis":millis,"max_memory_bytes":"536870912"},"image_query":null}}})).unwrap()).unwrap().command
}
fn publish(service: &NativeLibraryService, owner: &Owner, session: &Session, names: (&str, &str)) {
    let command=wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":names.0,"command":{"op":"refresh","library_id":service.library_id().as_str(),"mount_ids":[],"verification":"full_hash","embeddings":false,
        "budget":{"max_files":1000,"max_bytes":"67108864","max_millis":120000,"max_memory_bytes":"536870912"},"operation_id":names.0}})).unwrap()).unwrap().command;
    let permit = service.native_scope(session, &command, None).unwrap();
    let inventory = reconcile(service, owner, permit.clone());
    let job = service
        .prepare_refresh(
            owner,
            permit,
            inventory,
            None,
            holding(NativeRefreshJob::metadata_requirement().memory_bytes, 0),
        )
        .unwrap();
    let need = job.corpus_requirement().unwrap();
    job.submit(NativeJobPlan {
        operation_id: names.0,
        job_id: names.1,
        budget: Budget {
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 120000,
            max_memory_bytes: 512 * 1024 * 1024,
        },
        reservation: Reservation {
            memory_bytes: need.memory_bytes.max(256 * 1024 * 1024),
            temporary_bytes: need.temporary_bytes,
        },
        priority: knowledge_core::jobs::Priority::Normal,
        max_attempts: 1,
    })
    .unwrap();
    let account = Account::current().unwrap();
    let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    let draft = job
        .begin_corpus(claim, admitted::generated_path().unwrap())
        .unwrap()
        .build_lexical(|_| {})
        .unwrap_or_else(|e| panic!("actual native M3: {}", e.cause()))
        .prepare_tuple(|_| {})
        .unwrap_or_else(|e| panic!("actual tuple: {}", e.cause()))
        .select_tuple(|_| {})
        .unwrap_or_else(|e| panic!("actual selector: {}", e.cause()));
    let (job, claim) = draft.into_job().unwrap();
    account
        .queue()
        .settle(&claim, job.key(), knowledge_core::jobs::Outcome::Completed)
        .unwrap();
}
fn inputs<'a>(
    services: &'a [NativeLibraryService],
    owner: &'a Owner,
    permit: Permit,
    underpaid: u8,
) -> NativeQueryInputs<'a> {
    // ALL real loans are obtained before the constructor can open even its FIRST input FD.
    let scratch = holding(4 * 1024 * 1024, 0);
    let descriptor = ready(NativeTupleQuery::metadata_requirement());
    let plans = services
        .iter()
        .enumerate()
        .map(|(i, s)| {
            NativeQueryLibraryInput::new(
                s,
                owner,
                "wiki",
                None,
                ready(Reservation {
                    memory_bytes: if underpaid == 1 && i == services.len() - 1 {
                        1
                    } else {
                        NativeTupleRead::requirement().memory_bytes
                    },
                    temporary_bytes: 0,
                }),
                holding(
                    if underpaid == 2 && i == services.len() - 1 {
                        1024 * 1024
                    } else {
                        2 * 1024 * 1024
                    },
                    0,
                ),
            )
        })
        .collect();
    let account = Account::current().unwrap();
    let mut pressure = holding(account.status().unwrap().memory_available, 0);
    let prepared = NativeQueryInputs::open(permit, plans, scratch, descriptor);
    pressure.finish().unwrap();
    prepared
}
#[test]
fn actual_two_library_inputs_share_prepaid_span_and_original_deadline() {
    if std::env::var_os("OMNI_QUERY_INPUT_CHILD").is_none() {
        let cache = tempfile::tempdir().unwrap();
        fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let out=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","native_service::inventory::tests::query_inputs::actual_two_library_inputs_share_prepaid_span_and_original_deadline","--nocapture"])
            .env("OMNI_QUERY_INPUT_CHILD","1").env("OMNI_CACHE_DIR",cache.path()).output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("running 1 test"));
        assert!(
            out.status.success(),
            "actual two-library child: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let homes = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let (_, owner) = NativeAuthority::new();
    let mut services = Vec::new();
    let mut selections = Vec::new();
    for (i, home) in homes.iter().enumerate() {
        let kb = KnowledgeBase::init(home.path(), "actual multi-library input").unwrap();
        fs::write(
            kb.root.join("raw/native.md"),
            format!("# Actual root {i}\nalpha 日本語\n"),
        )
        .unwrap();
        let (service, selection) = NativeLibraryService::ordinary_local_owned(
            kb,
            LocalPolicy {
                mode: wire::ExposureMode::ReadWrite,
                sensitive: false,
                excluded_paths: vec![],
                transports: vec![wire::Transport::OwnerStdio],
                http_hosts: vec![],
                http_origins: vec![],
            },
            &owner,
        )
        .unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        if i == 0 {
            knowledge_retrieval::KernelAdapter::initialize_runtime().unwrap();
        }
        publish(
            &service,
            &owner,
            &session,
            if i == 0 {
                ("input-build-a", "input-job-a")
            } else {
                ("input-build-b", "input-job-b")
            },
        );
        services.push(service);
        selections.push(selection);
    }
    assert_ne!(services[0].library_id(), services[1].library_id());
    let account = Account::current().unwrap();
    let session = owner
        .authority()
        .connect(&selections, TransportFacts::OwnerStdio)
        .unwrap();
    let full = command(&services, 1000, 64 * 1024 * 1024, 60000);
    let permit = services[0].native_scope(&session, &full, None).unwrap();
    let expected_deadline = permit.query_deadline().unwrap();
    std::thread::sleep(Duration::from_millis(10));
    let before = NativeTupleRead::test_input_probes();
    let before_m1 = NativeInventory::test_input_objects();
    let other = services[0].native_scope(&session, &full, None).unwrap();
    assert!(permit.same_input_scope(&permit.clone()));
    assert!(
        !permit.same_input_scope(&other),
        "equal bytes cannot replace actual invocation/frame identity"
    );
    let quote =
        NativeQueryInputs::requirement(&permit, &[(&services[0], "wiki"), (&services[1], "wiki")])
            .unwrap();
    assert!(
        quote.memory_bytes > 24 * 1024 * 1024
            && quote.memory_bytes < 28 * 1024 * 1024 + permit.retained_input_memory().unwrap()
    );
    let prepared = inputs(&services, &owner, permit.clone(), 0);
    assert!(
        prepared.failure().is_none(),
        "actual all-funded preparation: {:?}",
        prepared.failure()
    );
    let first = prepared.test_first_input();
    let total = prepared.input_amount();
    let after = NativeTupleRead::test_input_probes();
    assert_eq!(after.0 - before.0, 2, "two actual namespace input FD opens");
    assert_eq!(
        total.1,
        after.2 - before.2 + NativeInventory::test_input_objects() - before_m1
    );
    assert_eq!(
        first.2, expected_deadline,
        "time spent before input cannot be renewed"
    );
    assert!(total.0 > first.0 && total.1 > first.1);
    let query = prepared
        .finish()
        .unwrap_or_else(|e| panic!("actual shared input finish: {:?}", e.failure()));
    // The retained input peak cannot be ignored just because the job alone fits the wire cap.
    let bad = query.submit(NativeJobPlan {
        operation_id: "too-large-input-sum",
        job_id: "too-large-input-sum",
        budget: Budget {
            max_files: 1000,
            max_bytes: 64 * 1024 * 1024,
            max_millis: 60000,
            max_memory_bytes: 512 * 1024 * 1024,
        },
        reservation: Reservation {
            memory_bytes: 512 * 1024 * 1024,
            temporary_bytes: 0,
        },
        priority: knowledge_core::jobs::Priority::Normal,
        max_attempts: 1,
    });
    assert!(bad.is_err());
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    // Quote one sequential peak plus every retained list inside the unchanged wire cap.
    let need = query.requirement_all().unwrap();
    let submitted = query
        .submit(NativeJobPlan {
            operation_id: "two-library-delivery",
            job_id: "two-library-delivery",
            budget: Budget {
                max_files: 1000,
                max_bytes: 64 * 1024 * 1024,
                max_millis: 60000,
                max_memory_bytes: need.memory_bytes,
            },
            reservation: need,
            priority: knowledge_core::jobs::Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
    let mut controller = query.start_controller(claim.clone()).unwrap();
    assert!(
        controller.failure().is_none(),
        "actual two-library delivery: {:?}",
        controller.failure()
    );
    let result = controller.search_result().unwrap();
    assert_eq!(result.groups.len(), 2);
    for (group, service) in result.groups.iter().zip(&services) {
        assert_eq!(&group.library_id, service.library_id());
        assert!(
            !group.files.is_empty(),
            "library lost its own matching files"
        );
        assert!(
            group
                .files
                .iter()
                .all(|file| &file.source.library_id == service.library_id())
        );
    }
    let first_path = homes[0].path().join("raw/native.md");
    let original = fs::read(&first_path).unwrap();
    fs::write(&first_path, "changed after both groups completed").unwrap();
    assert!(
        controller.search_result().is_err(),
        "earlier-library winner lost its final dependency check"
    );
    assert!(
        controller
            .encode_response(wire::Id::new("two-library-stale").unwrap())
            .is_err()
    );
    let (returned_query, returned_claim) = controller
        .finish()
        .unwrap_or_else(|_| panic!("two-library delivery retained scopes"));
    drop((returned_query, returned_claim));
    fs::write(&first_path, original).unwrap();
    let claim =
        Arc::try_unwrap(claim).unwrap_or_else(|_| panic!("two-library delivery retained Claim"));
    account
        .queue()
        .settle(
            &claim,
            query.key(),
            knowledge_core::jobs::Outcome::Completed,
        )
        .unwrap();
    account.queue().retire(submitted.job, query.key()).unwrap();
    drop(query);
    // Independent native FD observation: only ONE tuple may open when the second has one
    // remaining object. Passing the original full limit to each child actually opens TWO.
    let limited = command(&services, first.1 + 1, 64 * 1024 * 1024, 60000);
    let permit = services[0].native_scope(&session, &limited, None).unwrap();
    let before = NativeTupleRead::test_input_probes();
    let refused = inputs(&services, &owner, permit, 0);
    let after = NativeTupleRead::test_input_probes();
    assert!(refused.failure().is_some());
    assert_eq!(
        after.0 - before.0,
        1,
        "second actual input FD opened after aggregate object exhaustion"
    );
    assert!(after.2 - before.2 <= first.1 + 1);
    drop(refused.finish().err().unwrap());
    let limited = command(&services, 1000, first.0 + 1, 60000);
    let permit = services[0].native_scope(&session, &limited, None).unwrap();
    let before = NativeTupleRead::test_input_probes();
    let refused = inputs(&services, &owner, permit, 0);
    let after = NativeTupleRead::test_input_probes();
    assert!(refused.failure().is_some());
    assert!(
        after.1 - before.1 <= first.0 + 1,
        "actual native tuple bytes exceeded shared span"
    );
    drop(refused);
    for underpaid in [1, 2] {
        let permit = services[0].native_scope(&session, &full, None).unwrap();
        let before = NativeTupleRead::test_input_probes();
        let refused = inputs(&services, &owner, permit, underpaid);
        assert!(refused.failure().is_some());
        assert_eq!(
            NativeTupleRead::test_input_probes().0,
            before.0,
            "late unpaid reader/policy context cannot permit earlier input reads"
        );
        drop(refused);
    }
    let expired = command(&services, 1000, 64 * 1024 * 1024, 1);
    let permit = services[0].native_scope(&session, &expired, None).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let before = NativeTupleRead::test_input_probes();
    let refused = inputs(&services, &owner, permit, 0);
    assert!(refused.failure().is_some());
    assert_eq!(
        NativeTupleRead::test_input_probes().0,
        before.0,
        "original expired Query cannot restart its input timer"
    );
    drop(refused);
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    eprintln!(
        "actual two-library input controls PASS: normal={total:?} first={first:?}, shared object/byte spans, unpaid-late root, original deadline, retained-input+execution sum; no product acceptance claim"
    );
}
