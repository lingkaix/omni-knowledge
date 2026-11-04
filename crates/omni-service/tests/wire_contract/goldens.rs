use omni_service::{
    Command, CreationHistoryMode, ExposureMode, MountBackend, PackagePreset, PackageVerification,
    ProcessRecipe, SearchMethod, SearchRequest, SourceRef, SyncAction, TransferMode, Transport,
    Verification,
};

use crate::support::{
    DIGEST, budget, expected_version, golden, history_mode_signed, rel, request, root,
    search_request, source_ref, syn_id, target, uns,
};

const EXPECTED: &str = concat!(
    r#"{"digest":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","#,
    r#""size_bytes":"1","modified_ns":"0","changed_ns":null,"file_id":null,"backend_version":null}"#
);

const BUDGET: &str =
    r#"{"max_files":1,"max_bytes":"8388608","max_millis":30000,"max_memory_bytes":"1048576"}"#;

#[test]
fn library_list_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-library-list","command":{"op":"library_list"}}"#,
        request("syn-library-list", Command::LibraryList {}),
    );
}

#[test]
fn library_open_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-library-open","command":{"op":"library_open","path":"synthetic-os-folder/not-created"}}"#,
        request(
            "syn-library-open",
            Command::LibraryOpen {
                path: "synthetic-os-folder/not-created".to_owned(),
            },
        ),
    );
}

#[test]
fn library_create_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-library-create","command":{"op":"library_create","path":"synthetic-os-folder/not-created","name":"Synthetic","history_mode":"signed","operation_id":"syn-op"}}"#,
        request(
            "syn-library-create",
            Command::LibraryCreate {
                path: "synthetic-os-folder/not-created".to_owned(),
                name: "Synthetic".to_owned(),
                history_mode: history_mode_signed(),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn library_clone_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-library-clone","command":{"op":"library_clone","remote_url":"https://example.invalid/synthetic.git","destination":"synthetic-os-folder/not-created","name":"Synthetic","history_mode":"unsigned_explicit","operation_id":"syn-op"}}"#,
        request(
            "syn-library-clone",
            Command::LibraryClone {
                remote_url: "https://example.invalid/synthetic.git".to_owned(),
                destination: "synthetic-os-folder/not-created".to_owned(),
                name: "Synthetic".to_owned(),
                history_mode: CreationHistoryMode::UnsignedExplicit,
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn library_status_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-library-status","command":{"op":"library_status","library_id":"syn-library"}}"#,
        request(
            "syn-library-status",
            Command::LibraryStatus {
                library_id: syn_id("syn-library"),
            },
        ),
    );
}

#[test]
fn mount_attach_local_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-mount-attach","command":{"op":"mount_attach","library_id":"syn-library","name":"Synthetic mount","backend":"local","location":"synthetic-os-folder","destination":null,"operation_id":"syn-op"}}"#,
        request(
            "syn-mount-attach",
            Command::MountAttach {
                library_id: syn_id("syn-library"),
                name: "Synthetic mount".to_owned(),
                backend: MountBackend::Local,
                location: "synthetic-os-folder".to_owned(),
                destination: None,
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn mount_detach_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-mount-detach","command":{"op":"mount_detach","library_id":"syn-library","mount_id":"syn-mount","operation_id":"syn-op"}}"#,
        request(
            "syn-mount-detach",
            Command::MountDetach {
                library_id: syn_id("syn-library"),
                mount_id: syn_id("syn-mount"),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn mount_list_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-mount-list","command":{"op":"mount_list","library_id":"syn-library"}}"#,
        request(
            "syn-mount-list",
            Command::MountList {
                library_id: syn_id("syn-library"),
            },
        ),
    );
}

#[test]
fn browse_root_shape() {
    let bytes = r#"{"wire_version":1,"request_id":"syn-browse","command":{"op":"browse","library_id":"syn-library","mount_id":null,"path":"","cursor":null,"limit":50}}"#;
    let expected = request(
        "syn-browse",
        Command::Browse {
            library_id: syn_id("syn-library"),
            mount_id: None,
            path: root(""),
            cursor: None,
            limit: uns(50),
        },
    );
    golden(bytes, expected.clone());
    let decoded = omni_service::decode_request(bytes.as_bytes()).expect("browse root");
    let wire = serde_json::to_value(&decoded).expect("browse json");
    assert!(wire["command"].get("mount_id").is_some());
    assert!(wire["command"]["mount_id"].is_null());
    assert!(wire["command"].get("cursor").is_some());
    assert!(wire["command"]["cursor"].is_null());
    assert_eq!(wire["command"]["path"], "");
}

#[test]
fn source_details_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-source-details","command":{"op":"source_details","source":{"library_id":"syn-library","source_id":"syn-source"}}}"#,
        request(
            "syn-source-details",
            Command::SourceDetails {
                source: source_ref(),
            },
        ),
    );
}

#[test]
fn verify_source_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-verify-source","command":{{"op":"verify_source","source":{{"library_id":"syn-library","source_id":"syn-source"}},"budget":{BUDGET}}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-verify-source",
            Command::VerifySource {
                source: source_ref(),
                budget: budget(),
            },
        ),
    );
}

#[test]
fn read_range_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-read-range","command":{{"op":"read_range","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"offset":"0","length":1}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-read-range",
            Command::ReadRange {
                source: source_ref(),
                expected: expected_version(),
                offset: omni_service::U64::new(0),
                length: uns(1),
            },
        ),
    );
}

#[test]
fn read_representation_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-read-representation","command":{{"op":"read_representation","source":{{"library_id":"syn-library","source_id":"syn-source"}},"representation_digest":"{DIGEST}","start_line":1,"max_lines":20}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-read-representation",
            Command::ReadRepresentation {
                source: source_ref(),
                representation_digest: omni_service::Digest::new(DIGEST).expect("digest"),
                start_line: crate::support::pos(1),
                max_lines: uns(20),
            },
        ),
    );
}

#[test]
fn annotate_null_expected_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-annotate","command":{"op":"annotate","source":{"library_id":"syn-library","source_id":"syn-source"},"expected_annotation":null,"tags":["syn-tag"],"description":"A synthetic description","operation_id":"syn-op"}}"#,
        request(
            "syn-annotate",
            Command::Annotate {
                source: source_ref(),
                expected_annotation: None,
                tags: vec!["syn-tag".to_owned()],
                description: "A synthetic description".to_owned(),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn edit_text_null_expected_shape() {
    let bytes = r#"{"wire_version":1,"request_id":"syn-edit-text","command":{"op":"edit_text","target":{"library_id":"syn-library","mount_id":null,"path":"pages/index.md"},"expected":null,"text":"say \"hello\" \\ 中\n","operation_id":"syn-op"}}"#;
    golden(
        bytes,
        request(
            "syn-edit-text",
            Command::EditText {
                target: target(),
                expected: None,
                text: "say \"hello\" \\ 中\n".to_owned(),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn import_file_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-import-file","command":{"op":"import_file","library_id":"syn-library","input":{"library_id":"syn-library","source_id":"syn-source"},"destination":{"library_id":"syn-library","mount_id":null,"path":"pages/index.md"},"operation_id":"syn-op"}}"#,
        request(
            "syn-import-file",
            Command::ImportFile {
                library_id: syn_id("syn-library"),
                input: source_ref(),
                destination: target(),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn transfer_copy_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-transfer","command":{{"op":"transfer","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"destination":{{"library_id":"syn-library","mount_id":null,"path":"pages/index.md"}},"mode":"copy","operation_id":"syn-op"}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-transfer",
            Command::Transfer {
                source: source_ref(),
                expected: expected_version(),
                destination: target(),
                mode: TransferMode::Copy,
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn trash_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-trash","command":{{"op":"trash","source":{{"library_id":"syn-library","source_id":"syn-source"}},"expected":{EXPECTED},"operation_id":"syn-op"}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-trash",
            Command::Trash {
                source: source_ref(),
                expected: expected_version(),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn hydrate_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-hydrate","command":{"op":"hydrate","library_id":"syn-library","source_ids":["syn-source"],"make_available_offline":false,"operation_id":"syn-op"}}"#,
        request(
            "syn-hydrate",
            Command::Hydrate {
                library_id: syn_id("syn-library"),
                source_ids: vec![syn_id("syn-source")],
                make_available_offline: false,
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn refresh_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-refresh","command":{{"op":"refresh","library_id":"syn-library","mount_ids":[],"verification":"stat_first","embeddings":false,"budget":{BUDGET},"operation_id":"syn-op"}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-refresh",
            Command::Refresh {
                library_id: syn_id("syn-library"),
                mount_ids: vec![],
                verification: Verification::StatFirst,
                embeddings: false,
                budget: budget(),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn pin_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-pin","command":{"op":"pin","library_id":"syn-library","source_ids":["syn-source"],"pinned":true,"operation_id":"syn-op"}}"#,
        request(
            "syn-pin",
            Command::Pin {
                library_id: syn_id("syn-library"),
                source_ids: vec![syn_id("syn-source")],
                pinned: true,
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn process_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-process","command":{{"op":"process","library_id":"syn-library","source_ids":["syn-source"],"recipe":"extract","budget":{BUDGET},"operation_id":"syn-op"}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-process",
            Command::Process {
                library_id: syn_id("syn-library"),
                source_ids: vec![syn_id("syn-source")],
                recipe: ProcessRecipe::Extract,
                budget: budget(),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn query_shape_preserves_keyword_bytes() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-query","command":{{"op":"query","query":{{"library_ids":["syn-library"],"keywords":[" alpha ","alpha","alpha"],"methods":["bm25","literal"],"required_tags":["syn-tag"],"case_sensitive":false,"limit":10,"budget":{BUDGET},"image_query":null}}}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-query",
            Command::Query {
                query: search_request(),
            },
        ),
    );
}

#[test]
fn job_status_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-job-status","command":{"op":"job_status","job_id":"syn-job"}}"#,
        request(
            "syn-job-status",
            Command::JobStatus {
                job_id: syn_id("syn-job"),
            },
        ),
    );
}

#[test]
fn cancel_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-cancel","command":{"op":"cancel","job_id":"syn-job","operation_id":"syn-op"}}"#,
        request(
            "syn-cancel",
            Command::Cancel {
                job_id: syn_id("syn-job"),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn operation_status_null_library_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-operation-status","command":{"op":"operation_status","operation_id":"syn-op","library_id":null}}"#,
        request(
            "syn-operation-status",
            Command::OperationStatus {
                operation_id: syn_id("syn-op"),
                library_id: None,
            },
        ),
    );
}

#[test]
fn history_list_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-history-list","command":{"op":"history_list","library_id":"syn-library","cursor":null,"limit":20}}"#,
        request(
            "syn-history-list",
            Command::HistoryList {
                library_id: syn_id("syn-library"),
                cursor: None,
                limit: uns(20),
            },
        ),
    );
}

#[test]
fn history_diff_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-history-diff","command":{"op":"history_diff","library_id":"syn-library","base_commit_id":"syn-base","candidate_commit_id":"syn-candidate","offset":"0","length":64}}"#,
        request(
            "syn-history-diff",
            Command::HistoryDiff {
                library_id: syn_id("syn-library"),
                base_commit_id: "syn-base".to_owned(),
                candidate_commit_id: "syn-candidate".to_owned(),
                offset: omni_service::U64::new(0),
                length: uns(64),
            },
        ),
    );
}

#[test]
fn proposal_list_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-proposal-list","command":{"op":"proposal_list","library_id":"syn-library"}}"#,
        request(
            "syn-proposal-list",
            Command::ProposalList {
                library_id: syn_id("syn-library"),
            },
        ),
    );
}

#[test]
fn proposal_diff_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-proposal-diff","command":{"op":"proposal_diff","library_id":"syn-library","proposal_id":"syn-proposal","offset":"0","length":64}}"#,
        request(
            "syn-proposal-diff",
            Command::ProposalDiff {
                library_id: syn_id("syn-library"),
                proposal_id: syn_id("syn-proposal"),
                offset: omni_service::U64::new(0),
                length: uns(64),
            },
        ),
    );
}

#[test]
fn confirm_merge_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-confirm-merge","command":{{"op":"confirm_merge","library_id":"syn-library","proposal_id":"syn-proposal","base_commit_id":"syn-base","diff_digest":"{DIGEST}","operation_id":"syn-op"}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-confirm-merge",
            Command::ConfirmMerge {
                library_id: syn_id("syn-library"),
                proposal_id: syn_id("syn-proposal"),
                base_commit_id: "syn-base".to_owned(),
                diff_digest: omni_service::Digest::new(DIGEST).expect("digest"),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn sync_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-sync","command":{"op":"sync","library_id":"syn-library","action":"fetch","operation_id":"syn-op"}}"#,
        request(
            "syn-sync",
            Command::Sync {
                library_id: syn_id("syn-library"),
                action: SyncAction::Fetch,
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn sync_schedule_enabled_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-sync-schedule","command":{"op":"sync_schedule","library_id":"syn-library","enabled":true,"interval_minutes":15,"allow_push":false,"operation_id":"syn-op"}}"#,
        request(
            "syn-sync-schedule",
            Command::SyncSchedule {
                library_id: syn_id("syn-library"),
                enabled: true,
                interval_minutes: uns(15),
                allow_push: false,
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn epub_plan_shape() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-epub-plan","command":{{"op":"epub_plan","library_id":"syn-library","page_paths":["pages/index.md"],"media_source_ids":[],"destination":"/synthetic/book.epub","budget":{BUDGET}}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-epub-plan",
            Command::EpubPlan {
                library_id: syn_id("syn-library"),
                page_paths: vec![rel("pages/index.md")],
                media_source_ids: vec![],
                destination: "/synthetic/book.epub".to_owned(),
                budget: budget(),
            },
        ),
    );
}

#[test]
fn epub_export_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-epub-export","command":{"op":"epub_export","library_id":"syn-library","plan_id":"syn-plan","operation_id":"syn-op"}}"#,
        request(
            "syn-epub-export",
            Command::EpubExport {
                library_id: syn_id("syn-library"),
                plan_id: syn_id("syn-plan"),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn snapshot_plan_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-snapshot-plan","command":{"op":"snapshot_plan","library_id":"syn-library","selection":{"preset":"frozen","source_ids":[],"page_paths":[],"originals":"references","include_indexes":false,"include_history":false,"include_journal":false,"require_complete":true},"destination":"/synthetic/snapshot.zip"}}"#,
        request(
            "syn-snapshot-plan",
            Command::SnapshotPlan {
                library_id: syn_id("syn-library"),
                selection: omni_service::PackageSelection {
                    preset: PackagePreset::Frozen,
                    source_ids: vec![],
                    page_paths: vec![],
                    originals: omni_service::OriginalSelection::References,
                    include_indexes: false,
                    include_history: false,
                    include_journal: false,
                    require_complete: true,
                },
                destination: "/synthetic/snapshot.zip".to_owned(),
            },
        ),
    );
}

#[test]
fn snapshot_create_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-snapshot-create","command":{"op":"snapshot_create","library_id":"syn-library","plan_id":"syn-plan","operation_id":"syn-op"}}"#,
        request(
            "syn-snapshot-create",
            Command::SnapshotCreate {
                library_id: syn_id("syn-library"),
                plan_id: syn_id("syn-plan"),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn package_inspect_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-package-inspect","command":{"op":"package_inspect","path":"synthetic-os-folder/archive.zip","verification":"manifest"}}"#,
        request(
            "syn-package-inspect",
            Command::PackageInspect {
                path: "synthetic-os-folder/archive.zip".to_owned(),
                verification: PackageVerification::Manifest,
            },
        ),
    );
}

#[test]
fn open_frozen_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-open-frozen","command":{"op":"open_frozen","path":"synthetic-os-folder/archive.zip"}}"#,
        request(
            "syn-open-frozen",
            Command::OpenFrozen {
                path: "synthetic-os-folder/archive.zip".to_owned(),
            },
        ),
    );
}

#[test]
fn restore_plan_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-restore-plan","command":{"op":"restore_plan","path":"synthetic-os-folder/archive.zip","destination":"synthetic-os-folder/restored","mount_map":[{"mount_id":"syn-mount","location":"synthetic-os-folder"}]}}"#,
        request(
            "syn-restore-plan",
            Command::RestorePlan {
                path: "synthetic-os-folder/archive.zip".to_owned(),
                destination: "synthetic-os-folder/restored".to_owned(),
                mount_map: vec![omni_service::MountMapping {
                    mount_id: syn_id("syn-mount"),
                    location: "synthetic-os-folder".to_owned(),
                }],
            },
        ),
    );
}

#[test]
fn restore_apply_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-restore-apply","command":{"op":"restore_apply","plan_id":"syn-plan","operation_id":"syn-op"}}"#,
        request(
            "syn-restore-apply",
            Command::RestoreApply {
                plan_id: syn_id("syn-plan"),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn migration_plan_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-migration-plan","command":{"op":"migration_plan","library_id":"syn-library"}}"#,
        request(
            "syn-migration-plan",
            Command::MigrationPlan {
                library_id: syn_id("syn-library"),
            },
        ),
    );
}

#[test]
fn migration_apply_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-migration-apply","command":{"op":"migration_apply","library_id":"syn-library","plan_id":"syn-plan","operation_id":"syn-op"}}"#,
        request(
            "syn-migration-apply",
            Command::MigrationApply {
                library_id: syn_id("syn-library"),
                plan_id: syn_id("syn-plan"),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn v1_import_plan_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-v1-import","command":{"op":"v1_import_plan","path":"synthetic-os-folder/foreign","destination":"synthetic-os-folder/imported"}}"#,
        request(
            "syn-v1-import",
            Command::V1ImportPlan {
                path: "synthetic-os-folder/foreign".to_owned(),
                destination: "synthetic-os-folder/imported".to_owned(),
            },
        ),
    );
}

#[test]
fn v1_export_plan_null_host_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-v1-export","command":{"op":"v1_export_plan","library_id":"syn-library","destination":"synthetic-os-folder/exported","host_context_id":null}}"#,
        request(
            "syn-v1-export",
            Command::V1ExportPlan {
                library_id: syn_id("syn-library"),
                destination: "synthetic-os-folder/exported".to_owned(),
                host_context_id: None,
            },
        ),
    );
}

#[test]
fn exposure_get_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-exposure-get","command":{"op":"exposure_get","library_id":"syn-library"}}"#,
        request(
            "syn-exposure-get",
            Command::ExposureGet {
                library_id: syn_id("syn-library"),
            },
        ),
    );
}

#[test]
fn exposure_set_shape() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-exposure-set","command":{"op":"exposure_set","library_id":"syn-library","expected_revision":"syn-revision","mode":"read_only","sensitive":false,"excluded_paths":["private/notes.md"],"transports":["owner_ipc"],"operation_id":"syn-op"}}"#,
        request(
            "syn-exposure-set",
            Command::ExposureSet {
                library_id: syn_id("syn-library"),
                expected_revision: syn_id("syn-revision"),
                mode: ExposureMode::ReadOnly,
                sensitive: false,
                excluded_paths: vec![rel("private/notes.md")],
                transports: vec![Transport::OwnerIpc],
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn git_mount_keeps_destination_string() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-mount-git","command":{"op":"mount_attach","library_id":"syn-library","name":"Synthetic git","backend":"git","location":"https://example.invalid/synthetic.git","destination":"synthetic-os-folder/work","operation_id":"syn-op"}}"#,
        request(
            "syn-mount-git",
            Command::MountAttach {
                library_id: syn_id("syn-library"),
                name: "Synthetic git".to_owned(),
                backend: MountBackend::Git,
                location: "https://example.invalid/synthetic.git".to_owned(),
                destination: Some("synthetic-os-folder/work".to_owned()),
                operation_id: syn_id("syn-op"),
            },
        ),
    );
}

#[test]
fn browse_child_and_selected_mount() {
    golden(
        r#"{"wire_version":1,"request_id":"syn-browse-child","command":{"op":"browse","library_id":"syn-library","mount_id":"syn-mount","path":"pages","cursor":"syn-cursor","limit":1}}"#,
        request(
            "syn-browse-child",
            Command::Browse {
                library_id: syn_id("syn-library"),
                mount_id: Some(syn_id("syn-mount")),
                path: root("pages"),
                cursor: Some("syn-cursor".to_owned()),
                limit: uns(1),
            },
        ),
    );
}

#[test]
fn image_query_keeps_semantic_source_ref() {
    let bytes = format!(
        r#"{{"wire_version":1,"request_id":"syn-image-query","command":{{"op":"query","query":{{"library_ids":["syn-library"],"keywords":[],"methods":["semantic"],"required_tags":[],"case_sensitive":false,"limit":1,"budget":{BUDGET},"image_query":{{"library_id":"syn-library","source_id":"syn-source"}}}}}}}}"#
    );
    golden(
        &bytes,
        request(
            "syn-image-query",
            Command::Query {
                query: SearchRequest {
                    library_ids: vec![syn_id("syn-library")],
                    keywords: vec![],
                    methods: vec![SearchMethod::Semantic],
                    required_tags: vec![],
                    case_sensitive: false,
                    limit: uns(1),
                    budget: budget(),
                    image_query: Some(SourceRef {
                        library_id: syn_id("syn-library"),
                        source_id: syn_id("syn-source"),
                    }),
                },
            },
        ),
    );
}
