//! Actual canonical M1 journal/asset identity→native source→SAME Claim/M3/provider controls.
#[path = "inventory_credit_tests.rs"]
mod credit;
use super::*;
use crate::native_service::embedding_tests::{claim, digest, ready, server};
use knowledge_core::admission::Account;
use knowledge_search::{ModelIdentity, VectorMetric, VectorNormalization};
use std::sync::atomic::Ordering;

fn holding(memory_bytes: u64, temporary_bytes: u64) -> Holding {
    match Account::current()
        .unwrap()
        .reserve(
            Reservation {
                memory_bytes,
                temporary_bytes,
            },
            Origin::BeforeVerification,
        )
        .unwrap()
    {
        Decision::Ready(h) => h,
        _ => panic!("actual ONE-account fixture admission"),
    }
}
fn refresh(
    service: &NativeLibraryService,
    session: &Session,
    embeddings: bool,
    id: &str,
) -> Permit {
    let command = wire::decode_request(&serde_json::to_vec(&serde_json::json!({
        "wire_version":1,"request_id":"inventory","command":{
            "op":"refresh","library_id":service.library_id().as_str(),"mount_ids":[],
            "verification":"full_hash","embeddings":embeddings,
            "budget":{"max_files":1000,"max_bytes":"8388608","max_millis":60000,"max_memory_bytes":"134217728"},
            "operation_id":id
        }
    })).unwrap()).unwrap().command;
    service.native_scope(session, &command, None).unwrap()
}
fn reconcile(service: &NativeLibraryService, owner: &Owner, permit: Permit) -> NativeInventory {
    let mut scratch = holding(4 * 1024 * 1024, 16 * 1024 * 1024);
    let context = holding(2 * 1024 * 1024, 0);
    let (inventory, result) = service
        .reconcile_inventory(
            owner,
            permit,
            "wiki",
            context,
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
    assert!(result.complete);
    assert!(result.verified > 0);
    drop(result);
    scratch.finish().unwrap(); // Actual synchronous report/scratch died; Holding::drop is not release.
    inventory
}
fn observed(inventory: &NativeInventory, wanted: &str) -> NativeObservedSource {
    let mut offset = 0;
    let mut wanted_source = None;
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
        assert!(page.issue().is_none());
        let next = page.next_offset();
        let complete = page.complete();
        for source in page.into_sources() {
            let path = source.observation().stat.path.as_str();
            assert!(
                !path.starts_with("nested/"),
                "actual child-owned home is outside parent inventory"
            );
            assert!(
                !path.starts_with(".omni/"),
                "actual bookkeeping is not original content"
            );
            if path == wanted {
                assert!(
                    wanted_source.is_none(),
                    "only the actual complete scan is consumed"
                );
                wanted_source = Some(source);
            }
        }
        if complete {
            break;
        }
        assert!(
            next > offset,
            "partial replay must progress without skipping history"
        );
        offset = next;
    }
    wanted_source.expect("actual bounded M1 scan source")
}
#[test]
fn canonical_inventory_asset_rename_and_actual_refresh_embedding_use_the_held_descriptor() {
    if std::env::var_os("OMNI_NATIVE_INVENTORY_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","native_service::inventory::tests::canonical_inventory_asset_rename_and_actual_refresh_embedding_use_the_held_descriptor","--nocapture"])
            .env("OMNI_NATIVE_INVENTORY_CHILD","1").output().unwrap();
        assert!(
            output.status.success(),
            "native inventory child: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(home.path(), "actual M1 native source").unwrap();
    let html = "<h1>Actual M1 日本語 asset</h1><p>Stable identity is not a path hash</p>";
    fs::write(kb.root.join("raw/native.html"), html).unwrap();
    let child = KnowledgeBase::init(&kb.root.join("nested"), "child-owned").unwrap();
    fs::write(child.root.join("raw/secret.html"), "never a parent source").unwrap();
    let policy = LocalPolicy {
        mode: wire::ExposureMode::ReadWrite,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![wire::Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    };
    let (service, owner, _selection) =
        NativeLibraryService::ordinary_local(kb.clone(), policy).unwrap();
    let _fixture = ready(Reservation {
        memory_bytes: 2 * 1024 * 1024,
        temporary_bytes: 65536,
    });
    let artifacts = tempfile::tempdir().unwrap();
    let package = artifacts.path().join("actual-contract-package.rs");
    let manifest = artifacts.path().join("actual-model.json");
    let code =
        include_bytes!("../../crates/knowledge-processing/tests/support/embedding_server.rs");
    fs::write(&package, code).unwrap();
    let mut config = crate::embedding::EmbeddingConfig {
        endpoint: "http://127.0.0.1:1".into(),
        model: "m1-contract-only".into(),
        dimension: 2,
        package_fingerprint: hex::encode(Sha256::digest(code)),
        embedding_space: "m1-native-contract:not-quality".into(),
        timeout_seconds: 5,
    };
    let server = server::Server::start(&config, None, false);
    config.endpoint = server.endpoint.clone();
    let model = ModelIdentity {
        provider: "deterministic-contract-only".into(),
        model: config.model.clone(),
        revision: "fixture-v1".into(),
        dimension: 2,
        metric: VectorMetric::Cosine,
        normalization: VectorNormalization::Raw,
        package_fingerprint: config.package_fingerprint.clone(),
        embedding_space: config.embedding_space.clone(),
        preprocessing: "actual-UTF8-byte-length:not-quality".into(),
    };
    fs::write(&manifest, serde_json::to_vec(&model).unwrap()).unwrap();
    let selected = service
        .select_embedding(
            &owner,
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
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let actual_refresh = refresh(&service, &session, true, "inventory-first");
    // Actual stat-first Refresh consumes M1 metadata only; producer verification is separate.
    let mut metadata_command = actual_refresh.admitted_command().unwrap().clone();
    if let wire::Command::Refresh {
        verification,
        embeddings,
        operation_id,
        ..
    } = &mut metadata_command
    {
        *verification = wire::Verification::StatFirst;
        *embeddings = false;
        *operation_id = wire::Id::new("inventory-stat-first").unwrap();
    }
    let metadata_permit = service
        .native_scope(&session, &metadata_command, None)
        .unwrap();
    let mut scratch = holding(4 * 1024 * 1024, 16 * 1024 * 1024);
    let (metadata_inventory, metadata_result) = service
        .reconcile_inventory(
            &owner,
            metadata_permit,
            "wiki",
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
    assert!(metadata_result.complete && metadata_result.pending > 0);
    assert_eq!(metadata_result.verified, 0);
    let metadata_source = observed(&metadata_inventory, "raw/native.html");
    assert!(metadata_source.observation().version.is_none());
    let metadata_asset = metadata_source.source_id().to_owned();
    drop(metadata_source);
    drop(metadata_inventory);
    drop(metadata_result);
    scratch.finish().unwrap();
    assert_eq!(server.accepted.load(Ordering::SeqCst), 0);
    // Reusing the SAME admitted Refresh frame must neither recursively reenter nor form a pin cycle.
    let inventory = reconcile(&service, &owner, actual_refresh.clone());
    let first = observed(&inventory, "raw/native.html");
    let asset = first.source_id().to_owned();
    uuid::Uuid::parse_str(&asset).unwrap();
    assert_eq!(asset, metadata_asset);
    assert!(
        LocalMount::from_selected_directory(
            &kb.root,
            File::open(&child.root).unwrap(),
            service.library_id().as_str(),
            None,
            inventory.owner.policy.clone()
        )
        .is_err(),
        "never discard/substitute the actual selected descriptor"
    );
    assert!(
        Store::from_selected_directory(
            &kb.root,
            File::open(&child.root).unwrap(),
            service.library_id().as_str(),
            "native",
            inventory.owner.policy.clone()
        )
        .is_err(),
        "M1 must retain the actual native selected root"
    );
    assert_ne!(asset, hex::encode(Sha256::digest(b"raw/native.html")));
    let wrong = artifacts.path().join("same-bytes-wrong-inode.html");
    fs::write(&wrong, html).unwrap();
    assert!(
        inventory
            .owner
            .mount
            .check_held_snapshot(
                &File::open(&wrong).unwrap(),
                &first.observation().stat,
                inventory.owner.check_budget
            )
            .is_err(),
        "path metadata does not qualify another equal-byte descriptor"
    );
    let budget = Budget {
        max_files: 1,
        max_bytes: 65536,
        max_millis: 60000,
        max_memory_bytes: 128 * 1024 * 1024,
    };
    let status = service
        .native_scope(
            &session,
            &wire::Command::LibraryStatus {
                library_id: service.library_id().clone(),
            },
            None,
        )
        .unwrap();
    let mut no_body_budget = budget;
    no_body_budget.max_bytes = 0;
    let wrong_scope = service
        .source_for_observed(
            &status,
            observed(&inventory, "raw/native.html"),
            no_body_budget,
        )
        .err()
        .expect("status never selects a source processing body");
    assert!(
        wrong_scope
            .to_string()
            .contains("actual command does not select"),
        "refuse before source inspection/hash: {wrong_scope}"
    );
    drop(status);
    let source = service
        .source_for_observed(&actual_refresh, first, budget)
        .unwrap();
    let processing = source
        .processing(
            digest(b"actual-refresh-embed"),
            digest(include_bytes!("inventory_tests.rs")),
        )
        .unwrap();
    assert_eq!(processing.key().source, asset);
    let original_processor = processing.key().processor.as_str().to_owned();
    let c = claim(&processing, budget, "m1-actual-refresh-embed");
    let producer = processing
        .extract_for_embedding(c, selected.clone(), 65536, None)
        .unwrap();
    let text = producer.text().unwrap().to_owned();
    assert!(text.contains("日本語"));
    let account = Account::current().unwrap();
    let mut pressure = holding(account.status().unwrap().memory_available, 0);
    let output = producer.embed(0..text.len()).unwrap();
    assert_eq!(output.source_id().unwrap(), asset);
    assert_eq!(
        output.source_digest().unwrap(),
        digest(html.as_bytes()).as_str()
    );
    assert_eq!(output.vector().unwrap(), &[text.len() as f32, 1.0]);
    assert_eq!(server.seen.lock().unwrap().as_deref(), Some(text.as_str()));
    pressure.finish().unwrap();
    output.into_producer().complete().unwrap();
    drop(actual_refresh);
    let accepted_after_embedding = server.accepted.load(Ordering::SeqCst);
    assert!(accepted_after_embedding > 0);
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    // Actual Refresh without embeddings is metadata/extraction intention, NOT receiver consent.
    let no_embed = refresh(&service, &session, false, "inventory-no-embed");
    let source = service
        .source_for_observed(&no_embed, observed(&inventory, "raw/native.html"), budget)
        .unwrap();
    let processing = source
        .processing(digest(b"refresh-without-embedding"), digest(b"fixture"))
        .unwrap();
    let c = claim(&processing, budget, "m1-no-embed");
    let error = processing
        .extract_for_embedding(c, selected, 65536, None)
        .err()
        .expect("actual false embedding choice must refuse");
    let (processing, c, _) = error.into_parts();
    account
        .queue()
        .settle(&c, processing.key(), knowledge_core::jobs::Outcome::Failed)
        .unwrap();
    drop(processing);
    drop(no_embed);
    assert_eq!(
        server.accepted.load(Ordering::SeqCst),
        accepted_after_embedding
    );
    // Native rename evidence, not content equality, preserves the independent asset ID.
    fs::rename(
        kb.root.join("raw/native.html"),
        kb.root.join("raw/renamed.html"),
    )
    .unwrap();
    let renamed = reconcile(
        &service,
        &owner,
        refresh(&service, &session, false, "inventory-rename"),
    );
    let row = observed(&renamed, "raw/renamed.html");
    assert_eq!(row.source_id(), asset);
    assert_eq!(row.observation().change, ChangeKind::Renamed);
    let renamed_permit = refresh(&service, &session, false, "inventory-renamed-input");
    let renamed_processing = service
        .source_for_observed(&renamed_permit, row, budget)
        .unwrap()
        .processing(
            digest(b"actual-refresh-embed"),
            digest(include_bytes!("inventory_tests.rs")),
        )
        .unwrap();
    assert_ne!(
        renamed_processing.key().processor.as_str(),
        original_processor,
        "stable M1 identity must not erase the actual M3 hint from effective dependencies"
    );
    drop(renamed_processing);
    drop(renamed_permit);
    let before_replacement = observed(&renamed, "raw/renamed.html");
    let equal = artifacts.path().join("replacement.html");
    fs::write(&equal, html).unwrap();
    fs::rename(equal, kb.root.join("raw/renamed.html")).unwrap();
    let current = refresh(&service, &session, false, "inventory-replacement-open");
    assert!(
        service
            .source_for_observed(&current, before_replacement, budget)
            .is_err(),
        "equal bytes cannot reuse another native identity's M1 mapping"
    );
    drop(current);
    let replaced = reconcile(
        &service,
        &owner,
        refresh(&service, &session, false, "inventory-replacement-scan"),
    );
    let new_source = observed(&replaced, "raw/renamed.html");
    assert_ne!(new_source.source_id(), asset);
    drop(server);
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
}
