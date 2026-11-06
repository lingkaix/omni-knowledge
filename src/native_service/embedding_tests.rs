//! Actual owner-selected code package/receiver/source/Claim→M3→HTTP→prepared vector controls.
//! The explicit deterministic contract model proves NO WeMM/semantic quality/native human acceptance.
use super::*;
use knowledge_core::{backend::Budget, data::ContentDigest, jobs::Priority};
use knowledge_search::{
    AccountGrant, ModelIdentity, VectorMetric, VectorNormalization, VectorRow, VectorStore,
};
use std::{os::unix::fs::PermissionsExt, sync::atomic::Ordering};
#[path = "../../crates/knowledge-processing/tests/support/embedding_server.rs"]
pub(super) mod server;
pub(super) fn digest(bytes: &[u8]) -> ContentDigest {
    ContentDigest::new(hex::encode(Sha256::digest(bytes))).unwrap()
}
pub(super) fn ready(need: Reservation) -> Work {
    match Work::reserve(need, Origin::BeforeVerification).unwrap() {
        Decision::Ready(w) => w,
        _ => panic!("actual fixture admission"),
    }
}
fn command(service: &NativeLibraryService, source_id: &str, operation_id: &str) -> wire::Command {
    wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"embed","command":{"op":"process","library_id":service.library_id().as_str(),"source_ids":[source_id],"recipe":"embed","budget":{"max_files":1,"max_bytes":"65536","max_millis":60000,"max_memory_bytes":"134217728"},"operation_id":operation_id}})).unwrap()).unwrap().command
}
fn processing(
    service: &NativeLibraryService,
    session: &Session,
    command: &wire::Command,
    budget: Budget,
) -> NativeProcessing {
    let permit = service.native_scope(session, command, None).unwrap();
    let recipe = match command {
        wire::Command::Process { recipe, .. } => {
            digest(format!("actual-native-Process:{recipe:?};M3:max-bytes65536").as_bytes())
        }
        _ => panic!("actual process recipe"),
    };
    service
        .source(
            &permit,
            &wire::RelativePath::new("raw/native.html").unwrap(),
            budget,
        )
        .unwrap()
        .processing(recipe, digest(include_bytes!("embedding_tests.rs")))
        .unwrap()
}
pub(super) fn claim(
    processing: &NativeProcessing,
    budget: Budget,
    id: &str,
) -> knowledge_core::jobs::Claim {
    let job = processing
        .submit(NativeJobPlan {
            operation_id: id,
            job_id: id,
            budget,
            reservation: Reservation {
                memory_bytes: 128 * 1024 * 1024,
                temporary_bytes: 2 * 1024 * 1024,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let account = knowledge_core::admission::Account::current().unwrap();
    let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    assert_eq!(claim.job(), job.job);
    claim
}
#[test]
fn actual_owner_package_source_claim_embedding_and_model_revocation() {
    // Isolate this actual selected-Claim assertion from unrelated global queue subjects.
    if std::env::var_os("OMNI_NATIVE_EMBED_CHILD").is_none() {
        let out=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","native_service::embedding_tests::actual_owner_package_source_claim_embedding_and_model_revocation","--nocapture"]).env("OMNI_NATIVE_EMBED_CHILD","1").output().unwrap();
        assert!(
            out.status.success(),
            "actual native embed child: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(home.path(), "Actual native embedding fixture").unwrap();
    let html = "<h1>Actual 日本語 native source</h1><p>Scope and package proof</p>";
    fs::write(kb.root.join("raw/native.html"), html).unwrap();
    let policy = LocalPolicy {
        mode: wire::ExposureMode::ReadWrite,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![wire::Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    };
    let (service, owner, _selection) =
        NativeLibraryService::ordinary_local(kb.clone(), policy.clone()).unwrap();
    let account = knowledge_core::admission::Account::current().unwrap();
    let fixture = ready(Reservation {
        memory_bytes: 2 * 1024 * 1024,
        temporary_bytes: 65536,
    });
    let artifacts = tempfile::tempdir().unwrap();
    let package = artifacts.path().join("contract-package.rs");
    let metadata = artifacts.path().join("identity.json");
    let code =
        include_bytes!("../../crates/knowledge-processing/tests/support/embedding_server.rs");
    fs::write(&package, code).unwrap();
    let mut config = crate::embedding::EmbeddingConfig {
        endpoint: "http://127.0.0.1:1".into(),
        model: "native-contract-v1".into(),
        dimension: 2,
        package_fingerprint: hex::encode(Sha256::digest(code)),
        embedding_space: "native-contract-only".into(),
        timeout_seconds: 5,
    };
    let server = server::Server::start(&config, None, false);
    config.endpoint = server.endpoint.clone();
    let identity = ModelIdentity {
        provider: "deterministic-contract-only".into(),
        model: config.model.clone(),
        revision: "actual-fixture-source-v1".into(),
        dimension: 2,
        metric: VectorMetric::Cosine,
        normalization: VectorNormalization::Raw,
        package_fingerprint: config.package_fingerprint.clone(),
        embedding_space: config.embedding_space.clone(),
        preprocessing: "actual-UTF8-byte-length-v1:not-semantic-quality".into(),
    };
    fs::write(&metadata, serde_json::to_vec(&identity).unwrap()).unwrap();
    let model_budget = Budget {
        max_files: 2,
        max_bytes: 1024 * 1024,
        max_memory_bytes: 2 * 1024 * 1024,
        max_millis: 30000,
    };
    let selected = service
        .select_embedding(
            &owner,
            File::open(&package).unwrap(),
            File::open(&metadata).unwrap(),
            config.clone(),
            model_budget,
            ready(NativeEmbeddingSelection::requirement(&config).unwrap()),
        )
        .unwrap();
    assert_eq!(selected.model().unwrap(), &identity);
    let mut tiny_model_budget = model_budget;
    tiny_model_budget.max_memory_bytes = 1;
    assert!(
        service
            .select_embedding(
                &owner,
                File::open(&package).unwrap(),
                File::open(&metadata).unwrap(),
                config.clone(),
                tiny_model_budget,
                ready(NativeEmbeddingSelection::requirement(&config).unwrap())
            )
            .is_err()
    );
    selected.model().unwrap();
    // Invalid preparation must not commit the native configuration epoch or revoke the good choice.
    let bad = artifacts.path().join("invalid.json");
    let mut invalid = identity.clone();
    invalid.package_fingerprint = "ff".repeat(32);
    fs::write(&bad, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let mut invalid_config = config.clone();
    invalid_config.package_fingerprint = invalid.package_fingerprint;
    assert!(
        service
            .select_embedding(
                &owner,
                File::open(&package).unwrap(),
                File::open(&bad).unwrap(),
                invalid_config.clone(),
                model_budget,
                ready(NativeEmbeddingSelection::requirement(&invalid_config).unwrap())
            )
            .is_err()
    );
    selected.model().unwrap();
    assert_eq!(server.accepted.load(Ordering::SeqCst), 0);
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let browse=serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"browse","command":{"op":"browse","library_id":service.library_id().as_str(),"mount_id":null,"path":"raw","cursor":null,"limit":100}})).unwrap();
    let listing = service.dispatch(&browse, &session).unwrap();
    let response: serde_json::Value = serde_json::from_slice(&listing.encode().unwrap()).unwrap();
    let source_id = response["result"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "native.html")
        .unwrap()["source"]["source_id"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(response);
    drop(listing);
    let cmd = command(&service, &source_id, "actual-embed");
    let budget = Budget {
        max_files: 1,
        max_bytes: 65536,
        max_memory_bytes: 128 * 1024 * 1024,
        max_millis: 60000,
    };
    // Equal ID/path data in another actual authority cannot borrow the first owner's model context.
    let (foreign, _, _) = NativeLibraryService::ordinary_local(kb.clone(), policy).unwrap();
    let fses = foreign.connect(TransportFacts::OwnerStdio).unwrap();
    let fp = processing(
        &foreign,
        &fses,
        &command(&foreign, &source_id, "foreign-model"),
        budget,
    );
    let fc = claim(&fp, budget, "foreign-model");
    let error = fp
        .extract_for_embedding(fc, selected.clone(), 65536, None)
        .err()
        .expect("equal labels are not native model selection authority");
    let (p, c, _) = error.into_parts();
    account
        .queue()
        .settle(&c, p.key(), knowledge_core::jobs::Outcome::Failed)
        .unwrap();
    drop(p);
    assert_eq!(server.accepted.load(Ordering::SeqCst), 0);
    let mut extraction_only = command(&service, &source_id, "extract-not-embed");
    if let wire::Command::Process { recipe, .. } = &mut extraction_only {
        *recipe = wire::ProcessRecipe::Extract;
    }
    let p = processing(&service, &session, &extraction_only, budget);
    let c = claim(&p, budget, "extract-not-embed");
    let error = p
        .extract_for_embedding(c, selected.clone(), 65536, None)
        .err()
        .expect("an actual Extract grant is not Embed receiver intention");
    let (p, c, _) = error.into_parts();
    account
        .queue()
        .settle(&c, p.key(), knowledge_core::jobs::Outcome::Failed)
        .unwrap();
    drop(p);
    assert_eq!(server.accepted.load(Ordering::SeqCst), 0);
    let p = processing(&service, &session, &cmd, budget);
    let c = claim(&p, budget, "actual-embed");
    let consumer = Work::split_claim(
        &c,
        p.key(),
        Reservation {
            memory_bytes: 2 * 1024 * 1024,
            temporary_bytes: 2 * 1024 * 1024,
        },
    )
    .unwrap();
    let producer = p
        .extract_for_embedding(c, selected.clone(), 65536, None)
        .unwrap();
    let extracted = producer.text().unwrap().to_owned();
    assert!(extracted.contains("日本語"));
    let mut pressure = match account
        .reserve(
            Reservation {
                memory_bytes: account.status().unwrap().memory_available,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(h) => h,
        _ => panic!("actual all-free-credit pressure"),
    };
    let result = producer.embed(0..extracted.len()).unwrap();
    assert_eq!(
        server.seen.lock().unwrap().as_deref(),
        Some(extracted.as_str())
    );
    assert_eq!(result.vector().unwrap(), &[extracted.len() as f32, 1.0]);
    assert_eq!(
        result.source_digest().unwrap(),
        digest(html.as_bytes()).as_str()
    );
    assert_eq!(
        result.representation().unwrap().digest,
        hex::encode(Sha256::digest(extracted.as_bytes()))
    );
    assert_eq!(result.model().unwrap(), &identity);
    assert_eq!(
        account.status().unwrap().queue.unsettled,
        1,
        "actual source/Claim remains owned through output delivery"
    );
    let row = VectorRow {
        source_id: result.source_id().unwrap().into(),
        source_digest: result.source_digest().unwrap().into(),
        representation: result.representation().unwrap().clone(),
        locator: format!(
            "extracted-byte:{}..{}",
            result.range().unwrap().start,
            result.range().unwrap().end
        ),
        vector: result.vector().unwrap().to_vec(),
    };
    let output = tempfile::tempdir().unwrap();
    fs::set_permissions(output.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let rows = [row];
    let prepared = std::sync::Arc::new(
        VectorStore::prepare(
            output.path(),
            result.model().unwrap(),
            &rows,
            &AccountGrant::from_work(consumer),
            None,
        )
        .unwrap(),
    );
    assert!(!output.path().join("CURRENT").exists());
    let mut component = prepared.component("rows.bin").unwrap();
    let mut encoded = Vec::new();
    std::io::Read::read_to_end(&mut component, &mut encoded).unwrap();
    assert!(
        String::from_utf8_lossy(&encoded)
            .contains(result.representation().unwrap().digest.as_str())
    );
    drop(encoded);
    drop(component);
    drop(prepared);
    drop(rows);
    pressure.finish().unwrap();
    result.into_producer().complete().unwrap();
    // A new real source version obtains another actual Claim; identical metadata replacement still
    // revokes the old selected FD and cannot be resurrected by restoring equal model DATA.
    fs::write(
        kb.root.join("raw/native.html"),
        "<p>Second source 日本語</p>",
    )
    .unwrap();
    let p = processing(
        &service,
        &session,
        &command(&service, &source_id, "model-replacement"),
        budget,
    );
    let c = claim(&p, budget, "model-replacement");
    let producer = p
        .extract_for_embedding(c, selected.clone(), 65536, None)
        .unwrap();
    let length = producer.text().unwrap().len();
    let result = producer.embed(0..length).unwrap();
    let replacement = artifacts.path().join("replacement.json");
    fs::write(&replacement, fs::read(&metadata).unwrap()).unwrap();
    fs::rename(replacement, &metadata).unwrap();
    assert!(
        result.vector().is_err(),
        "equal bytes do not replace the actual selected model metadata FD"
    );
    let renewed = service
        .select_embedding(
            &owner,
            File::open(&package).unwrap(),
            File::open(&metadata).unwrap(),
            config.clone(),
            model_budget,
            ready(NativeEmbeddingSelection::requirement(&config).unwrap()),
        )
        .unwrap();
    renewed.model().unwrap();
    assert!(
        result.vector().is_err(),
        "equal identity data cannot resurrect old native source/model epochs"
    );
    let error = result
        .into_producer()
        .complete()
        .err()
        .expect("revoked model/source context cannot complete");
    let (p, c, _) = error.into_parts();
    account
        .queue()
        .settle(&c, p.key(), knowledge_core::jobs::Outcome::Failed)
        .unwrap();
    drop(p);
    let mut l2_identity = identity.clone();
    l2_identity.normalization = VectorNormalization::L2;
    fs::write(&metadata, serde_json::to_vec(&l2_identity).unwrap()).unwrap();
    let l2 = service
        .select_embedding(
            &owner,
            File::open(&package).unwrap(),
            File::open(&metadata).unwrap(),
            config.clone(),
            model_budget,
            ready(NativeEmbeddingSelection::requirement(&config).unwrap()),
        )
        .unwrap();
    let p = processing(
        &service,
        &session,
        &command(&service, &source_id, "bad-l2"),
        budget,
    );
    let c = claim(&p, budget, "bad-l2");
    let producer = p.extract_for_embedding(c, l2.clone(), 65536, None).unwrap();
    let length = producer.text().unwrap().len();
    let error = producer
        .embed(0..length)
        .err()
        .expect("declared L2 model cannot expose an actual raw/nonunit provider vector");
    let (p, c, _) = error.into_parts();
    account
        .queue()
        .settle(&c, p.key(), knowledge_core::jobs::Outcome::InvalidModel)
        .unwrap();
    drop(p);
    drop(l2);
    assert!(
        !output.path().join("CURRENT").exists(),
        "failed provider/model never selects a partial index tuple"
    );
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    drop(renewed);
    drop(selected);
    drop(server);
    drop(fixture);
}
