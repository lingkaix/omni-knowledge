//! Real M1 row/context resource lifetime, using one isolated native account and actual library.
use super::*;
#[test]
fn native_inventory_credit_survives_last_row_then_returns_after_payloads_die() {
    if std::env::var_os("OMNI_INVENTORY_CREDIT_CHILD").is_none() {
        for format in ["md", "html"] {
            let output=std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact","native_service::inventory::tests::credit::native_inventory_credit_survives_last_row_then_returns_after_payloads_die","--nocapture"])
                .env("OMNI_INVENTORY_CREDIT_CHILD","1").env("OMNI_INVENTORY_CREDIT_FORMAT",format).output().unwrap();
            assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
            assert!(
                output.status.success(),
                "actual {format} inventory credit child: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(home.path(), "actual M1 ownership").unwrap();
    let format = std::env::var("OMNI_INVENTORY_CREDIT_FORMAT").unwrap_or_else(|_| "md".into());
    let source_path = format!("raw/owned.{format}");
    let body = if format == "html" {
        "<h1>Actual owned source</h1><p>日本語</p>"
    } else {
        "# Actual owned source\n日本語\n"
    };
    fs::write(kb.root.join(&source_path), body).unwrap();
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
        refresh(&service, &session, false, "actual-credit-scan"),
    );
    let account = Account::current().unwrap();
    let before = account.status().unwrap().memory_available;
    let budget = Budget {
        max_files: 1000,
        max_bytes: 8 * 1024 * 1024,
        max_millis: 60000,
        max_memory_bytes: 1024 * 1024,
    };
    let page = inventory
        .page(0, budget, holding(2 * 1024 * 1024, 0))
        .unwrap();
    assert!(page.issue().is_none());
    let mut rows = page.into_sources();
    let row = rows.next().expect("actual complete scan source");
    drop(rows);
    assert_eq!(
        account.status().unwrap().memory_available,
        before - 2 * 1024 * 1024,
        "an escaping opaque row still owns its full actual page"
    );
    assert!(!row.source_id().is_empty());
    drop(row);
    assert_eq!(
        account.status().unwrap().memory_available,
        before,
        "last opaque row must destroy actual page metadata and FINISH its real Holding; Holding::drop is not settlement"
    );
    assert!(inventory.page(0, budget, holding(1, 0)).is_err());
    assert_eq!(
        account.status().unwrap().memory_available,
        before,
        "refused preflight returns empty input funding before any metadata inspection"
    );
    let page = inventory
        .page(0, budget, holding(2 * 1024 * 1024, 0))
        .unwrap();
    let row = page
        .sources()
        .find(|r| r.observation().stat.path.as_str() == source_path)
        .unwrap();
    let page = match page.into_funding() {
        Err(page) => page,
        Ok(_) => panic!("live actual row must prevent page-credit recovery"),
    };
    let source_permit = refresh(&service, &session, false, "actual-credit-source");
    let source = service
        .source_for_observed(
            &source_permit,
            row,
            Budget {
                max_files: 1,
                max_bytes: 8 * 1024 * 1024,
                max_millis: 60000,
                max_memory_bytes: 1024 * 1024,
            },
        )
        .unwrap();
    let mut processing = source
        .processing(
            digest(b"actual-source-metadata-scope"),
            digest(include_bytes!("inventory_credit_tests.rs")),
        )
        .unwrap();
    let mut claim = claim(
        &processing,
        Budget {
            max_files: 1,
            max_bytes: 8 * 1024 * 1024,
            max_millis: 60000,
            max_memory_bytes: 128 * 1024 * 1024,
        },
        "actual-m1-scoped-metadata",
    );
    let mut scoped_pressure = holding(account.status().unwrap().memory_available, 0);
    let fork = processing
        .source()
        .permit()
        .fork_processing_scope(
            &claim,
            processing.key(),
            Reservation {
                memory_bytes: 1024 * 1024,
                temporary_bytes: 0,
            },
        )
        .unwrap();
    let fresh = page
        .sources()
        .find(|r| r.observation().stat.path.as_str() == source_path)
        .unwrap();
    let scoped = service
        .source_for_observed(
            &fork,
            fresh,
            Budget {
                max_files: 1,
                max_bytes: 8 * 1024 * 1024,
                max_millis: 60000,
                max_memory_bytes: 1024 * 1024,
            },
        )
        .unwrap();
    assert_eq!(scoped.version(), processing.source().version());
    assert_eq!(
        account.status().unwrap().memory_available,
        0,
        "actual M1→native FD/hash/binding is inside the SAME real source Claim, no late global credit"
    );
    drop(scoped);
    assert!(
        fork.rejoin_processing_scope(&claim, processing.key())
            .is_ok()
    );
    assert_eq!(account.status().unwrap().memory_available, 0);
    for _ in 0..8 {
        let input = processing.extract_indexed(claim, 65536, None).unwrap();
        assert!(input.text().unwrap().contains("Actual owned source"));
        assert!(input.text().unwrap().contains("日本語"));
        assert_ne!(input.source_id().unwrap(), "actual-metadata-source");
        (processing, claim) = input.finish_stage().unwrap();
        assert_eq!(
            account.status().unwrap().memory_available,
            0,
            "all actual M3 output/metadata loans rejoin the same live claim before the next source phase"
        );
        assert_eq!(
            account.status().unwrap().queue.unsettled,
            1,
            "read-stage return is not job completion"
        );
    }
    scoped_pressure.finish().unwrap();
    account
        .queue()
        .settle(
            &claim,
            processing.key(),
            knowledge_core::jobs::Outcome::Completed,
        )
        .unwrap();
    drop(processing);
    let page = match page.into_funding() {
        Err(page) => page,
        Ok(_) => panic!("actual SDK FD/delivery check still owns the M1 row/page"),
    };
    drop(source_permit); // Its actual FD/version callback, not a boolean, releases the last row.
    let mut funding = match page.into_funding() {
        Ok(h) => h,
        Err(_) => panic!("all actual row/source owners are gone"),
    };
    let mut pressure = holding(account.status().unwrap().memory_available, 0);
    for _ in 0..16 {
        let page = inventory.page(0, budget, funding).unwrap();
        assert!(page.issue().is_none());
        let row = page.sources().next().unwrap();
        let page = match page.into_funding() {
            Err(page) => page,
            Ok(_) => panic!("a live opaque handle cannot return credit"),
        };
        drop(row);
        funding = match page.into_funding() {
            Ok(h) => h,
            Err(_) => panic!("same fully destroyed page must return the same actual holding"),
        };
        assert_eq!(
            account.status().unwrap().memory_available,
            0,
            "page iteration never releases/re-reserves global credit"
        );
    }
    funding.finish().unwrap();
    pressure.finish().unwrap();
    assert_eq!(account.status().unwrap().memory_available, before);
    let mut refused_scratch = holding(4 * 1024 * 1024, 16 * 1024 * 1024);
    let refused_before = account.status().unwrap().memory_available;
    assert!(
        service
            .reconcile_inventory(
                &owner,
                refresh(&service, &session, false, "refused-layout"),
                "invalid-layout",
                holding(2 * 1024 * 1024, 0),
                &Preflight {
                    holding: &refused_scratch,
                    budget: Budget {
                        max_files: 1000,
                        max_bytes: 8 * 1024 * 1024,
                        max_millis: 60000,
                        max_memory_bytes: 4 * 1024 * 1024
                    }
                }
            )
            .is_err()
    );
    assert_eq!(
        account.status().unwrap().memory_available,
        refused_before,
        "a refused actual context constructor returns its empty owned funding"
    );
    refused_scratch.finish().unwrap();
    drop(inventory);
    assert_eq!(
        account.status().unwrap().memory_available,
        before + 2 * 1024 * 1024,
        "actual context credit ends after its last page/source dependent"
    );
}
