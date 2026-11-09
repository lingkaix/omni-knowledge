//! Actual Query/readonly Owner policy, existing complete M1 journal and unchanged durable bytes.
use super::*;
fn tree(path: &std::path::Path) -> Vec<(std::path::PathBuf, u64, u64, i64, i64, String)> {
    use std::os::unix::fs::MetadataExt;
    fn walk(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut Vec<(std::path::PathBuf, u64, u64, i64, i64, String)>,
    ) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for p in entries {
            let m = fs::symlink_metadata(&p).unwrap();
            out.push((
                p.strip_prefix(root).unwrap().into(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                if m.is_file() {
                    hex::encode(Sha256::digest(fs::read(&p).unwrap()))
                } else {
                    String::new()
                },
            ));
            if m.is_dir() {
                walk(root, &p, out)
            }
        }
    }
    let mut out = vec![];
    walk(path, path, &mut out);
    out
}
#[test]
fn actual_readonly_query_binds_existing_complete_inventory_without_writable_context() {
    let home = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(home.path(), "readonly query").unwrap();
    fs::write(kb.root.join("pages/a.md"), "# 日本語 read-only source\n").unwrap();
    let mut policy = LocalPolicy {
        mode: wire::ExposureMode::ReadWrite,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![wire::Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    };
    let (service, owner, selection) =
        NativeLibraryService::ordinary_local(kb.clone(), policy.clone()).unwrap();
    let mut fixture = holding(2 * 1024 * 1024, 65536);
    let session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let inventory = reconcile(
        &service,
        &owner,
        refresh(&service, &session, false, "readonly-seed"),
    );
    let scan = inventory.scan().to_owned();
    drop(inventory);
    policy.mode = wire::ExposureMode::ReadOnly;
    owner.set_policy(&selection, policy).unwrap();
    let command=wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"readonly-query","command":{"op":"query","query":{"library_ids":[service.library_id().as_str()],"keywords":["日本語"],"methods":["files"],"required_tags":[],"case_sensitive":false,"limit":10,"budget":{"max_files":1000,"max_bytes":"8388608","max_millis":60000,"max_memory_bytes":"134217728"},"image_query":null}}})).unwrap()).unwrap().command;
    let permit = service.native_scope(&session, &command, None).unwrap();
    let mut scratch = holding(4 * 1024 * 1024, 0);
    let before = tree(&kb.root);
    let budget = Budget {
        max_files: 1000,
        max_bytes: 8 * 1024 * 1024,
        max_millis: 60000,
        max_memory_bytes: 4 * 1024 * 1024,
    };
    let inventory = service
        .inventory_for_query(
            &owner,
            &permit,
            "pages",
            &scan,
            holding(2 * 1024 * 1024, 0),
            &Preflight {
                holding: &scratch,
                budget,
            },
        )
        .unwrap();
    assert!(
        !inventory.owner.policy.0.write.writable,
        "actual readonly context, not forged writable Refresh"
    );
    let source = observed(&inventory, "pages/a.md");
    assert_eq!(source.observation().scan, scan);
    assert_eq!(
        tree(&kb.root),
        before,
        "Query binding and bounded replay cannot create or mutate a journal, projection, lane or source"
    );
    assert!(
        service
            .inventory_for_query(
                &owner,
                &permit,
                "pages",
                &uuid::Uuid::new_v4().to_string(),
                holding(2 * 1024 * 1024, 0),
                &Preflight {
                    holding: &scratch,
                    budget
                }
            )
            .is_err(),
        "arbitrary scan labels cannot assert a complete snapshot"
    );
    let status_command=wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"not-query","command":{"op":"library_status","library_id":service.library_id().as_str()}})).unwrap()).unwrap().command;
    let status = service
        .native_scope(&session, &status_command, None)
        .unwrap();
    assert!(
        service
            .inventory_for_query(
                &owner,
                &status,
                "pages",
                &scan,
                holding(2 * 1024 * 1024, 0),
                &Preflight {
                    holding: &scratch,
                    budget
                }
            )
            .is_err(),
        "Status is not Query intent"
    );
    owner.revoke(&selection).unwrap();
    assert!(
        inventory
            .page(0, budget, holding(5 * 1024 * 1024, 0))
            .is_err(),
        "actual live owner revocation applies before replay"
    );
    drop(source);
    drop(inventory);
    drop(permit);
    drop(status);
    scratch.finish().unwrap();
    fixture.finish().unwrap();
}
