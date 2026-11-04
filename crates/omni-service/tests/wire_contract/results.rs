use omni_service::{
    CancelState, CodecError, Digest, EntryKind, ErrorCode, Evidence, EvidenceKind, FileHit,
    HistoryEntry, HistoryOrigin, Integrity, Job, JobKind, JobPriority, JobState, Locator,
    MAX_CONTROL_FRAME_BYTES, MAX_SAFE_INTEGER, OperationState, PackageAction, PackageFormat,
    PackagePlan, PackageResult, PackageState, PlanSelection, PositiveInteger, Proposal,
    ProposalKind, ProposalState, ProviderState, Result as WireResult, SearchGroup, SearchMethod,
    SearchResult, ServiceError, Signature, SignerState, SyncOutcome, SyncResult, SyncState, U64,
    WriteResult, WriteState, encode_response,
};

use crate::support::{
    CANARY, DIGEST, coverage, exposure, library, mount, pos, response, safe, schedule,
    search_request, source, source_ref, stamp, syn_id, uns,
};

fn encoded(result: WireResult) -> serde_json::Value {
    let response = response("syn-res", result);
    let bytes = encode_response(&response).unwrap_or_else(|err| panic!("encode failed: {err}"));
    assert!(bytes.len() <= MAX_CONTROL_FRAME_BYTES);
    assert!(!bytes.contains(&b'\n'));
    assert_eq!(*bytes.first().expect("payload"), b'{');
    serde_json::from_slice(&bytes).expect("encoded json")
}

fn digest() -> Digest {
    Digest::new(DIGEST).expect("digest")
}

fn content(
    locator: Locator,
    next_offset: Option<U64>,
    next_line: Option<PositiveInteger>,
) -> WireResult {
    WireResult::Content {
        source: source_ref(),
        source_digest: digest(),
        representation_digest: None,
        locator,
        bytes_base64: "not*base64".to_owned(),
        truncated: false,
        next_offset,
        next_line,
    }
}

#[test]
fn empty_libraries_and_one_library_keep_null_keys() {
    let empty = encoded(WireResult::Libraries { libraries: vec![] });
    assert_eq!(
        empty,
        serde_json::json!({
            "wire_version": 1,
            "request_id": "syn-res",
            "result": {"type": "libraries", "libraries": []}
        })
    );

    let one = encoded(WireResult::Library {
        library: Box::new(library()),
    });
    assert_eq!(one["result"]["type"], "library");
    let library = &one["result"]["library"];
    assert_eq!(library["library_id"], "syn-library");
    assert_eq!(library["name"], "Synthetic");
    assert_eq!(library["home_path"], "synthetic-os-folder");
    assert_eq!(library["text_root"], "pages");
    assert!(library["manifest_version"].is_null());
    assert_eq!(library["state"], "unmarked");
    assert_eq!(library["history_mode"], "not_initialized");
    assert_eq!(library["exposure"]["mode"], "read_only");
    assert_eq!(library["exposure"]["sensitive"], false);
    assert_eq!(library["exposure"]["excluded_paths"], serde_json::json!([]));
    assert_eq!(
        library["exposure"]["transports"],
        serde_json::json!(["owner_ipc"])
    );
    assert_eq!(library["exposure"]["revision"], "syn-revision");
    assert_eq!(library["exposure"]["pending_expansion"], false);
    assert_eq!(library["mounts"], serde_json::json!([]));
    assert_eq!(library["sync_schedule"]["enabled"], false);
    assert_eq!(library["sync_schedule"]["interval_minutes"], 15);
    assert_eq!(library["sync_schedule"]["allow_push"], false);
    assert!(library["sync_schedule"]["next_run_at"].is_null());
    assert!(library["remote_url"].is_null());
    assert_eq!(library["remote_access"], "none");
    assert!(library.get("manifest_version").is_some());
    assert!(library.get("remote_url").is_some());
}

#[test]
fn safe_manifest_versions_encode_as_numbers() {
    let mut high_library = library();
    high_library.manifest_version = Some(safe(MAX_SAFE_INTEGER as i64));
    let high = encoded(WireResult::Library {
        library: Box::new(high_library),
    });
    assert_eq!(
        high["result"]["library"]["manifest_version"],
        MAX_SAFE_INTEGER
    );

    let mut low_library = library();
    low_library.manifest_version = Some(safe(-(MAX_SAFE_INTEGER as i64)));
    let low = encoded(WireResult::Library {
        library: Box::new(low_library),
    });
    assert_eq!(
        low["result"]["library"]["manifest_version"],
        serde_json::json!(-(MAX_SAFE_INTEGER as i64))
    );
}

#[test]
fn status_mount_source_and_listing_shapes() {
    let status = omni_service::Status {
        library: library(),
        coverage: coverage(),
        jobs: vec![],
        generation_id: None,
        cache_path: "synthetic-os-folder/cache".to_owned(),
        history_path: None,
        cache_used_bytes: U64::new(0),
        cache_budget_bytes: U64::new(1),
        provider: ProviderState::NotConfigured,
        signer: SignerState::UnsignedExplicit,
        recovery_operation_ids: vec![],
        warnings: vec![],
    };
    let value = encoded(WireResult::Status {
        status: Box::new(status),
    });
    assert_eq!(value["result"]["type"], "status");
    assert!(value["result"]["status"]["generation_id"].is_null());
    assert!(value["result"]["status"]["history_path"].is_null());
    assert_eq!(value["result"]["status"]["cache_used_bytes"], "0");
    assert_eq!(value["result"]["status"]["cache_budget_bytes"], "1");
    assert_eq!(value["result"]["status"]["provider"], "not_configured");
    assert_eq!(value["result"]["status"]["signer"], "unsigned_explicit");
    assert_eq!(
        value["result"]["status"]["coverage"]["enumeration"],
        "complete"
    );
    assert_eq!(
        value["result"]["status"]["coverage"]["result_extent"],
        "enumeration"
    );
    assert_eq!(value["result"]["status"]["coverage"]["excluded"], "0");
    assert!(value["result"]["status"]["coverage"]["stopped_reason"].is_null());

    let mounts = encoded(WireResult::Mounts {
        mounts: vec![mount()],
    });
    assert_eq!(mounts["result"]["type"], "mounts");
    assert_eq!(mounts["result"]["mounts"][0]["backend"], "local");
    assert_eq!(mounts["result"]["mounts"][0]["availability"], "available");
    assert_eq!(mounts["result"]["mounts"][0]["reconciliation"], "manual");
    assert!(mounts["result"]["mounts"][0]["last_complete_scan"].is_null());
    assert_eq!(
        mounts["result"]["mounts"][0]["capabilities"]["range_read"],
        true
    );
    assert_eq!(mounts["result"]["mounts"][0]["writable"], false);

    let source_value = encoded(WireResult::Source {
        source: Box::new(source()),
    });
    let source_json = &source_value["result"]["source"];
    assert_eq!(source_value["result"]["type"], "source");
    assert!(source_json["mount_id"].is_null());
    assert!(source_json["digest"].is_null());
    assert!(source_json["version"].is_null());
    assert_eq!(source_json["digest_state"], "unverified");
    assert_eq!(source_json["path"], "pages/index.md");
    assert_eq!(source_json["logical_path"], "pages/index.md");
    assert_eq!(source_json["residency"], "resident");
    assert_eq!(source_json["activity"], "stable");
    assert!(source_json.get("digest").is_some());
    assert!(source_json.get("version").is_some());

    let listing = encoded(WireResult::Listing {
        entries: vec![
            omni_service::ListingEntry {
                name: "pages".to_owned(),
                path: crate::support::rel("pages"),
                kind: EntryKind::Directory,
                source: None,
            },
            omni_service::ListingEntry {
                name: "index.md".to_owned(),
                path: crate::support::rel("pages/index.md"),
                kind: EntryKind::File,
                source: Some(source()),
            },
        ],
        next_cursor: None,
        complete: true,
    });
    assert_eq!(listing["result"]["type"], "listing");
    assert!(listing["result"]["next_cursor"].is_null());
    assert_eq!(listing["result"]["complete"], true);
    assert!(listing["result"]["entries"][0]["source"].is_null());
    assert_eq!(listing["result"]["entries"][0]["kind"], "directory");
    assert_eq!(listing["result"]["entries"][1]["kind"], "file");
    assert_eq!(
        listing["result"]["entries"][1]["source"]["source_id"],
        "syn-source"
    );
}

#[test]
fn locator_shapes_and_unchecked_base64_are_preserved() {
    let original = encoded(content(
        Locator::OriginalLines {
            start_line: pos(1),
            end_line: pos(2),
            start_byte: None,
            end_byte: None,
        },
        Some(U64::new(2)),
        None,
    ));
    assert_eq!(original["result"]["type"], "content");
    assert_eq!(original["result"]["bytes_base64"], "not*base64");
    assert_eq!(original["result"]["locator"]["type"], "original_lines");
    assert_eq!(original["result"]["locator"]["start_line"], 1);
    assert_eq!(original["result"]["locator"]["end_line"], 2);
    assert!(original["result"]["locator"]["start_byte"].is_null());
    assert!(original["result"]["locator"]["end_byte"].is_null());
    assert_eq!(original["result"]["next_offset"], "2");
    assert!(original["result"]["next_line"].is_null());
    assert!(original["result"]["representation_digest"].is_null());

    let lines = encoded(content(
        Locator::RepresentationLines {
            representation_digest: digest(),
            start_line: pos(3),
            end_line: pos(4),
        },
        None,
        Some(pos(5)),
    ));
    assert_eq!(lines["result"]["locator"]["type"], "representation_lines");
    assert_eq!(lines["result"]["locator"]["representation_digest"], DIGEST);
    assert!(lines["result"]["next_offset"].is_null());
    assert_eq!(lines["result"]["next_line"], 5);

    let page = encoded(content(
        Locator::Page {
            representation_digest: digest(),
            page: pos(6),
            start_line: None,
            end_line: None,
        },
        None,
        None,
    ));
    assert_eq!(page["result"]["locator"]["type"], "page");
    assert_eq!(page["result"]["locator"]["page"], 6);
    assert!(page["result"]["locator"]["start_line"].is_null());
    assert!(page["result"]["locator"]["end_line"].is_null());

    let slide = encoded(content(
        Locator::Slide {
            representation_digest: digest(),
            slide: pos(7),
        },
        None,
        None,
    ));
    assert_eq!(slide["result"]["locator"]["type"], "slide");
    assert_eq!(slide["result"]["locator"]["slide"], 7);

    let sheet = encoded(content(
        Locator::Sheet {
            representation_digest: digest(),
            sheet: "Sheet 1".to_owned(),
            cell_range: None,
        },
        None,
        None,
    ));
    assert_eq!(sheet["result"]["locator"]["type"], "sheet");
    assert_eq!(sheet["result"]["locator"]["sheet"], "Sheet 1");
    assert!(sheet["result"]["locator"].get("cell_range").is_some());
    assert!(sheet["result"]["locator"]["cell_range"].is_null());

    let time = encoded(content(
        Locator::Time {
            start_ms: U64::new(0),
            end_ms: U64::new(u64::MAX),
            representation_digest: None,
        },
        None,
        None,
    ));
    assert_eq!(time["result"]["locator"]["type"], "time");
    assert_eq!(time["result"]["locator"]["start_ms"], "0");
    assert_eq!(time["result"]["locator"]["end_ms"], "18446744073709551615");
    assert!(time["result"]["locator"]["representation_digest"].is_null());

    let unlocated = encoded(content(Locator::Unlocated, None, None));
    assert_eq!(
        unlocated["result"]["locator"],
        serde_json::json!({"type": "unlocated"})
    );
}

#[test]
fn search_preserves_null_and_finite_scores() {
    let hit = FileHit {
        source: source(),
        rank: pos(1),
        score: None,
        evidence: vec![Evidence {
            method: SearchMethod::Bm25,
            keyword: None,
            locator: Locator::Unlocated,
            snippet: "synthetic".to_owned(),
            kind: EvidenceKind::MetadataObservation,
            source_digest: None,
            representation_digest: None,
            processor_id: None,
            model_space: None,
            score: Some(crate::support::finite(1.5)),
        }],
        retained_evidence_count: U64::new(1),
        useful_location_count: U64::new(0),
    };
    let value = encoded(WireResult::Search {
        search: Box::new(SearchResult {
            applied: search_request(),
            groups: vec![SearchGroup {
                library_id: syn_id("syn-library"),
                generation_id: None,
                partitioning_id: None,
                files: vec![hit],
                coverage: coverage(),
                returned_files: uns(1),
                limit_reached: false,
            }],
        }),
    });
    assert_eq!(value["result"]["type"], "search");
    let file = &value["result"]["search"]["groups"][0]["files"][0];
    assert!(file["score"].is_null());
    assert_eq!(file["rank"], 1);
    assert_eq!(file["evidence"][0]["score"], serde_json::json!(1.5));
    assert!(file["evidence"][0]["keyword"].is_null());
    assert_eq!(file["evidence"][0]["kind"], "metadata_observation");
    assert_eq!(file["evidence"][0]["method"], "bm25");
    assert_eq!(
        value["result"]["search"]["applied"]["keywords"][0],
        " alpha "
    );
    assert_eq!(
        value["result"]["search"]["applied"]["keywords"]
            .as_array()
            .expect("keywords")
            .len(),
        3
    );
    assert!(value["result"]["search"]["applied"]["image_query"].is_null());
    assert!(value["result"]["search"]["groups"][0]["generation_id"].is_null());
}

#[test]
fn finite_zero_score_does_not_encode_as_null() {
    let mut evidence = Evidence {
        method: SearchMethod::Literal,
        keyword: Some("syn".to_owned()),
        locator: Locator::Unlocated,
        snippet: "synthetic".to_owned(),
        kind: EvidenceKind::VerifiedMatch,
        source_digest: None,
        representation_digest: None,
        processor_id: None,
        model_space: None,
        score: Some(crate::support::finite(0.0)),
    };
    evidence.score = Some(crate::support::finite(-1.5));
    let hit = FileHit {
        source: source(),
        rank: pos(1),
        score: Some(crate::support::finite(0.0)),
        evidence: vec![evidence],
        retained_evidence_count: U64::new(1),
        useful_location_count: U64::new(1),
    };
    let value = encoded(WireResult::Sources { sources: vec![] });
    assert_eq!(
        value["result"],
        serde_json::json!({"type": "sources", "sources": []})
    );
    let value = encoded(WireResult::Search {
        search: Box::new(SearchResult {
            applied: search_request(),
            groups: vec![SearchGroup {
                library_id: syn_id("syn-library"),
                generation_id: None,
                partitioning_id: None,
                files: vec![hit],
                coverage: coverage(),
                returned_files: uns(1),
                limit_reached: true,
            }],
        }),
    });
    let file = &value["result"]["search"]["groups"][0]["files"][0];
    assert_eq!(file["score"], serde_json::json!(0.0));
    assert!(file["score"].is_number());
    assert_eq!(file["evidence"][0]["score"], serde_json::json!(-1.5));
    assert_ne!(file["score"], serde_json::Value::Null);
}

#[test]
fn snippet_bound_counts_unicode_scalars() {
    let short = "中".repeat(500);
    assert_eq!(short.chars().count(), 500);
    assert!(short.len() > 500);
    let mut evidence = Evidence {
        method: SearchMethod::Literal,
        keyword: Some("syn".to_owned()),
        locator: Locator::Unlocated,
        snippet: short,
        kind: EvidenceKind::VerifiedMatch,
        source_digest: None,
        representation_digest: None,
        processor_id: None,
        model_space: None,
        score: None,
    };
    let accepted = response(
        "syn-res",
        WireResult::Search {
            search: Box::new(SearchResult {
                applied: search_request(),
                groups: vec![SearchGroup {
                    library_id: syn_id("syn-library"),
                    generation_id: None,
                    partitioning_id: None,
                    files: vec![FileHit {
                        source: source(),
                        rank: pos(1),
                        score: None,
                        evidence: vec![evidence.clone()],
                        retained_evidence_count: U64::new(1),
                        useful_location_count: U64::new(0),
                    }],
                    coverage: coverage(),
                    returned_files: uns(1),
                    limit_reached: false,
                }],
            }),
        },
    );
    let bytes = encode_response(&accepted).expect("500 unicode scalars fit");
    assert!(bytes.len() <= MAX_CONTROL_FRAME_BYTES);
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(
        value["result"]["search"]["groups"][0]["files"][0]["evidence"][0]["snippet"],
        "中".repeat(500)
    );

    evidence.snippet = "a".repeat(501);
    let rejected = response(
        "syn-res",
        WireResult::Search {
            search: Box::new(SearchResult {
                applied: search_request(),
                groups: vec![SearchGroup {
                    library_id: syn_id("syn-library"),
                    generation_id: None,
                    partitioning_id: None,
                    files: vec![FileHit {
                        source: source(),
                        rank: pos(1),
                        score: None,
                        evidence: vec![evidence],
                        retained_evidence_count: U64::new(1),
                        useful_location_count: U64::new(0),
                    }],
                    coverage: coverage(),
                    returned_files: uns(1),
                    limit_reached: false,
                }],
            }),
        },
    );
    let err = encode_response(&rejected).expect_err("501 scalars exceed the snippet bound");
    assert_ne!(err, CodecError::FrameTooLarge);
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn remaining_result_tags_keep_required_nulls() {
    let job = Job {
        job_id: syn_id("syn-job"),
        operation_id: syn_id("syn-op"),
        library_id: syn_id("syn-library"),
        kind: JobKind::Reconcile,
        state: JobState::Queued,
        priority: JobPriority::Normal,
        coverage: coverage(),
        error: None,
    };
    let value = encoded(WireResult::Job {
        job: Box::new(job.clone()),
    });
    assert_eq!(value["result"]["type"], "job");
    assert_eq!(value["result"]["job"]["kind"], "reconcile");
    assert_eq!(value["result"]["job"]["state"], "queued");
    assert_eq!(value["result"]["job"]["priority"], "normal");
    assert!(value["result"]["job"]["error"].is_null());

    let cancelled = encoded(WireResult::Cancelled {
        job_id: syn_id("syn-job"),
        state: CancelState::AlreadyTerminal,
    });
    assert_eq!(
        cancelled["result"],
        serde_json::json!({"type": "cancelled", "job_id": "syn-job", "state": "already_terminal"})
    );

    let pending = encoded(WireResult::Operation {
        operation_id: syn_id("syn-op"),
        state: OperationState::Pending,
        outcome: None,
        job: None,
    });
    assert_eq!(
        pending["result"],
        serde_json::json!({
            "type": "operation",
            "operation_id": "syn-op",
            "state": "pending",
            "outcome": null,
            "job": null
        })
    );
    assert!(pending["result"].get("outcome").is_some());

    let completed = encoded(WireResult::Operation {
        operation_id: syn_id("syn-op"),
        state: OperationState::Completed,
        outcome: Some(Box::new(WireResult::Libraries { libraries: vec![] })),
        job: Some(Box::new(job)),
    });
    assert_eq!(completed["result"]["outcome"]["type"], "libraries");
    assert_eq!(completed["result"]["job"]["job_id"], "syn-job");

    let write = encoded(WireResult::Write {
        write: Box::new(WriteResult {
            operation_id: syn_id("syn-op"),
            state: WriteState::Unchanged,
            source: None,
            commit_id: None,
            proposal: None,
            recovery_paths: vec![],
            sync_state: SyncState::NotApplicable,
        }),
    });
    assert_eq!(write["result"]["type"], "write");
    assert_eq!(write["result"]["write"]["state"], "unchanged");
    assert!(write["result"]["write"]["source"].is_null());
    assert!(write["result"]["write"]["commit_id"].is_null());
    assert!(write["result"]["write"]["proposal"].is_null());
    assert_eq!(write["result"]["write"]["sync_state"], "not_applicable");

    let history = encoded(WireResult::History {
        entries: vec![HistoryEntry {
            commit_id: "syn-commit".to_owned(),
            parent_ids: vec![],
            operation_id: None,
            observed_at: stamp(),
            origin: HistoryOrigin::Human,
            author: "Synthetic Author".to_owned(),
            committer: "Synthetic Committer".to_owned(),
            agent: None,
            tool: None,
            session_id: None,
            signature: Signature::Unsigned,
            affected_paths: vec![],
        }],
        next_cursor: None,
    });
    assert_eq!(history["result"]["type"], "history");
    assert_eq!(
        history["result"]["entries"][0]["observed_at"],
        "2026-11-04T00:00:00Z"
    );
    assert_eq!(history["result"]["entries"][0]["origin"], "human");
    assert_eq!(history["result"]["entries"][0]["signature"], "unsigned");
    assert!(history["result"]["entries"][0]["agent"].is_null());
    assert!(history["result"]["next_cursor"].is_null());

    let diff = encoded(WireResult::Diff {
        base_commit_id: "syn-base".to_owned(),
        candidate_commit_id: "syn-candidate".to_owned(),
        diff_digest: digest(),
        unified_diff: "line \"quoted\"\n".to_owned(),
        offset: U64::new(0),
        next_offset: None,
        truncated: false,
    });
    assert_eq!(diff["result"]["type"], "diff");
    assert_eq!(diff["result"]["unified_diff"], "line \"quoted\"\n");
    assert_eq!(diff["result"]["offset"], "0");
    assert!(diff["result"]["next_offset"].is_null());
    assert_eq!(diff["result"]["truncated"], false);

    let proposals = encoded(WireResult::Proposals {
        proposals: vec![Proposal {
            proposal_id: syn_id("syn-proposal"),
            library_id: syn_id("syn-library"),
            operation_id: syn_id("syn-op"),
            kind: ProposalKind::Text,
            base_commit_id: "syn-base".to_owned(),
            candidate_commit_id: "syn-candidate".to_owned(),
            diff_digest: digest(),
            affected_paths: vec![crate::support::rel("pages/index.md")],
            state: ProposalState::Pending,
        }],
    });
    assert_eq!(proposals["result"]["type"], "proposals");
    assert_eq!(proposals["result"]["proposals"][0]["kind"], "text");
    assert_eq!(proposals["result"]["proposals"][0]["state"], "pending");
    assert_eq!(proposals["result"]["proposals"][0]["diff_digest"], DIGEST);

    let sync = encoded(WireResult::Sync {
        sync: Box::new(SyncResult {
            operation_id: syn_id("syn-op"),
            state: SyncOutcome::Offline,
            local_head: None,
            remote_head: None,
            proposal: None,
            message: "synthetic refusal".to_owned(),
        }),
    });
    assert_eq!(sync["result"]["type"], "sync");
    assert_eq!(sync["result"]["sync"]["state"], "offline");
    assert!(sync["result"]["sync"]["local_head"].is_null());
    assert!(sync["result"]["sync"]["remote_head"].is_null());
    assert!(sync["result"]["sync"]["proposal"].is_null());
    assert_eq!(sync["result"]["sync"]["message"], "synthetic refusal");

    let exposure = encoded(WireResult::Exposure {
        exposure: Box::new(exposure()),
    });
    assert_eq!(exposure["result"]["type"], "exposure");
    assert_eq!(exposure["result"]["exposure"]["mode"], "read_only");

    let error = encoded(WireResult::Error {
        error: ServiceError {
            code: ErrorCode::OwnerRequired,
            message: format!("say \"hello\" \\ {CANARY}\n中"),
            retryable: false,
            operation_id: None,
            library_id: None,
            source_id: None,
            job_id: None,
            method: Some(SearchMethod::Regex),
            recovery_paths: vec![crate::support::rel("pages/index.md")],
        },
    });
    assert_eq!(error["result"]["type"], "error");
    assert_eq!(error["result"]["error"]["code"], "owner_required");
    assert_eq!(error["result"]["error"]["method"], "regex");
    assert!(error["result"]["error"]["operation_id"].is_null());
    assert_eq!(
        error["result"]["error"]["message"],
        format!("say \"hello\" \\ {CANARY}\n中")
    );
    assert_eq!(
        error["result"]["error"]["recovery_paths"][0],
        "pages/index.md"
    );
    let bytes = serde_json::to_vec(&error).expect("re-encode observer");
    assert!(!bytes.contains(&b'\n'));
    assert!(
        bytes
            .windows(CANARY.len())
            .any(|window| window == CANARY.as_bytes())
    );
}

#[test]
fn package_selection_must_agree_with_format() {
    let plan = |format, selection| PackagePlan {
        plan_id: syn_id("syn-plan"),
        library_id: syn_id("syn-library"),
        format,
        action: PackageAction::Export,
        selection,
        estimated_bytes: U64::new(0),
        external_reference_count: U64::new(0),
        uncovered_count: U64::new(0),
        coverage: coverage(),
        destination: Some("/synthetic/out".to_owned()),
        conflicts: vec![],
        warnings: vec![],
    };
    let package = PlanSelection::Package(omni_service::PackageSelection {
        preset: omni_service::PackagePreset::Backup,
        source_ids: vec![],
        page_paths: vec![],
        originals: omni_service::OriginalSelection::References,
        include_indexes: false,
        include_history: false,
        include_journal: false,
        require_complete: true,
    });
    let epub = PlanSelection::Epub(omni_service::EpubSelection {
        page_paths: vec![crate::support::rel("pages/index.md")],
        media_source_ids: vec![],
    });
    let native = encoded(WireResult::PackagePlan {
        plan: Box::new(plan(PackageFormat::NativeZip, package.clone())),
    });
    assert_eq!(native["result"]["type"], "package_plan");
    assert_eq!(native["result"]["plan"]["format"], "native_zip");
    assert_eq!(native["result"]["plan"]["selection"]["preset"], "backup");
    assert!(
        native["result"]["plan"]["selection"]
            .get("page_paths")
            .is_some()
    );

    let foreign = encoded(WireResult::PackagePlan {
        plan: Box::new(plan(PackageFormat::OpenkitV1, package.clone())),
    });
    assert_eq!(foreign["result"]["plan"]["format"], "openkit_v1");

    let publication = encoded(WireResult::PackagePlan {
        plan: Box::new(plan(PackageFormat::Epub, epub.clone())),
    });
    assert_eq!(publication["result"]["plan"]["format"], "epub");
    assert_eq!(
        publication["result"]["plan"]["selection"]["page_paths"][0],
        "pages/index.md"
    );
    assert!(
        publication["result"]["plan"]["selection"]
            .get("preset")
            .is_none()
    );

    let mismatch = response(
        "syn-res",
        WireResult::PackagePlan {
            plan: Box::new(plan(PackageFormat::Epub, package)),
        },
    );
    let err = encode_response(&mismatch).expect_err("epub format rejects package selection");
    assert_ne!(err, CodecError::FrameTooLarge);
    let mismatch = response(
        "syn-res",
        WireResult::PackagePlan {
            plan: Box::new(plan(PackageFormat::NativeZip, epub)),
        },
    );
    assert!(encode_response(&mismatch).is_err());

    let migration = encoded(WireResult::MigrationPlan {
        plan: Box::new(omni_service::MigrationPlan {
            plan_id: syn_id("syn-plan"),
            library_id: syn_id("syn-library"),
            from_version: safe(1),
            to_version: safe(MAX_SAFE_INTEGER as i64),
            path_map: vec![omni_service::PathMapping {
                from: crate::support::rel("pages/old.md"),
                to: crate::support::rel("pages/index.md"),
            }],
            orphan_annotation_paths: vec![],
            conflicts: vec![],
            warnings: vec![],
        }),
    });
    assert_eq!(migration["result"]["type"], "migration_plan");
    assert_eq!(migration["result"]["plan"]["from_version"], 1);
    assert_eq!(migration["result"]["plan"]["to_version"], MAX_SAFE_INTEGER);
    assert_eq!(
        migration["result"]["plan"]["path_map"][0]["from"],
        "pages/old.md"
    );

    let package = encoded(WireResult::Package {
        package: Box::new(PackageResult {
            operation_id: None,
            state: PackageState::Inspected,
            snapshot_id: None,
            path: None,
            library_id: None,
            signature: Signature::Unsigned,
            integrity: Integrity::ManifestOnly,
            coverage: coverage(),
            omitted_indexes: vec![],
            warnings: vec![],
        }),
    });
    assert_eq!(package["result"]["type"], "package");
    assert_eq!(package["result"]["package"]["state"], "inspected");
    assert_eq!(package["result"]["package"]["signature"], "unsigned");
    assert_eq!(package["result"]["package"]["integrity"], "manifest_only");
    assert!(package["result"]["package"]["snapshot_id"].is_null());
    assert!(package["result"]["package"]["operation_id"].is_null());
}

#[test]
fn nested_operation_outcome_is_refused() {
    let nested = response(
        "syn-res",
        WireResult::Operation {
            operation_id: syn_id("syn-op"),
            state: OperationState::Completed,
            outcome: Some(Box::new(WireResult::Operation {
                operation_id: syn_id("syn-inner"),
                state: OperationState::Pending,
                outcome: None,
                job: None,
            })),
            job: None,
        },
    );
    let err = encode_response(&nested).expect_err("nested operation");
    assert_ne!(err, CodecError::FrameTooLarge);
    assert!(std::error::Error::source(&err).is_none());
    crate::support::assert_input_free(&err, "syn-inner");
}

#[test]
fn diff_bound_is_not_the_frame_bound() {
    let diff = |text: String| {
        response(
            "syn-res",
            WireResult::Diff {
                base_commit_id: "syn-base".to_owned(),
                candidate_commit_id: "syn-candidate".to_owned(),
                diff_digest: digest(),
                unified_diff: text,
                offset: U64::new(0),
                next_offset: None,
                truncated: true,
            },
        )
    };
    let raw_len = 524_288;
    assert!(raw_len < MAX_CONTROL_FRAME_BYTES);
    let accepted = encode_response(&diff("a".repeat(raw_len))).expect("declared diff length fits");
    assert!(accepted.len() + 64 < MAX_CONTROL_FRAME_BYTES);
    let rejected = diff("a".repeat(raw_len + 1));
    let err = encode_response(&rejected).expect_err("one byte over the diff length");
    assert_ne!(err, CodecError::FrameTooLarge);
}

#[test]
fn escaped_field_can_exceed_the_frame_after_the_raw_field_fits() {
    let raw_len = 600_000;
    assert!(raw_len <= MAX_CONTROL_FRAME_BYTES);
    let error = |message: String| {
        response(
            "syn-res",
            WireResult::Error {
                error: ServiceError {
                    code: ErrorCode::InvalidRequest,
                    message,
                    retryable: false,
                    operation_id: None,
                    library_id: None,
                    source_id: None,
                    job_id: None,
                    method: None,
                    recovery_paths: vec![],
                },
            },
        )
    };
    let plain =
        encode_response(&error("a".repeat(raw_len))).expect("raw message fits in the frame");
    assert!(plain.len() <= MAX_CONTROL_FRAME_BYTES);
    let quotes = format!("{CANARY}{}", "\"".repeat(raw_len - CANARY.len()));
    assert_eq!(quotes.len(), raw_len);
    assert!(quotes.len() <= MAX_CONTROL_FRAME_BYTES);
    let err = encode_response(&error(quotes)).expect_err("escaping pushes the frame over");
    assert_eq!(err, CodecError::FrameTooLarge);
    crate::support::assert_input_free(&err, CANARY);
}

#[test]
fn unsupported_response_version_is_input_free() {
    let mut response = response(CANARY, WireResult::Libraries { libraries: vec![] });
    response.wire_version = 2;
    let err = encode_response(&response).expect_err("version");
    assert_eq!(err, CodecError::UnsupportedVersion);
    crate::support::assert_input_free(&err, CANARY);
}

#[test]
fn disabled_schedule_encodes_null_next_run() {
    let value = encoded(WireResult::Library {
        library: Box::new(library()),
    });
    assert!(
        value["result"]["library"]["sync_schedule"]
            .get("next_run_at")
            .is_some()
    );
    assert!(value["result"]["library"]["sync_schedule"]["next_run_at"].is_null());
    assert_eq!(schedule().interval_minutes.get(), 15);
}
