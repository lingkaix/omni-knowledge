use omni_service::{
    Activity, Availability, Budget, CodecError, Coverage, CreationHistoryMode, Digest, DigestState,
    Enumeration, ExpectedVersion, Exposure, ExposureMode, FiniteNumber, HistoryMode, Id, Library,
    LibraryState, MAX_CONTROL_FRAME_BYTES, Mount, MountBackend, MountCapabilities, Reconciliation,
    RelativePath, RemoteAccess, Request, Residency, Response, ResultExtent, RootRelativePath,
    SafeInteger, SearchRequest, SignedDecimal, Source, SourceRef, SyncSchedule, Target, TextRoot,
    Time, U64, UnsignedInteger, decode_request,
};
use std::error::Error;

pub const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const CANARY: &str = "CANARY-wire-9f3c";

pub fn syn_id(value: &str) -> Id {
    Id::new(value).expect("fixture id")
}

pub fn rel(value: &str) -> RelativePath {
    RelativePath::new(value).expect("fixture relative path")
}

pub fn root(value: &str) -> RootRelativePath {
    RootRelativePath::new(value).expect("fixture browse path")
}

pub fn uns(value: u64) -> UnsignedInteger {
    UnsignedInteger::new(value).expect("fixture safe integer")
}

pub fn pos(value: u64) -> omni_service::PositiveInteger {
    omni_service::PositiveInteger::new(value).expect("fixture positive integer")
}

pub fn finite(value: f64) -> FiniteNumber {
    FiniteNumber::new(value).expect("fixture finite number")
}

pub fn safe(value: i64) -> SafeInteger {
    SafeInteger::new(value).expect("fixture signed integer")
}

pub fn budget() -> Budget {
    Budget {
        max_files: uns(1),
        max_bytes: U64::new(8_388_608),
        max_millis: uns(30_000),
        max_memory_bytes: U64::new(1_048_576),
    }
}

pub fn source_ref() -> SourceRef {
    SourceRef {
        library_id: syn_id("syn-library"),
        source_id: syn_id("syn-source"),
    }
}

pub fn expected_version() -> ExpectedVersion {
    ExpectedVersion {
        digest: Digest::new(DIGEST).expect("fixture digest"),
        size_bytes: U64::new(1),
        modified_ns: Some(SignedDecimal::new("0").expect("fixture decimal")),
        changed_ns: None,
        file_id: None,
        backend_version: None,
    }
}

pub fn target() -> Target {
    Target {
        library_id: syn_id("syn-library"),
        mount_id: None,
        path: rel("pages/index.md"),
    }
}

pub fn search_request() -> SearchRequest {
    SearchRequest {
        library_ids: vec![syn_id("syn-library")],
        keywords: vec![" alpha ".to_owned(), "alpha".to_owned(), "alpha".to_owned()],
        methods: vec![
            omni_service::SearchMethod::Bm25,
            omni_service::SearchMethod::Literal,
        ],
        required_tags: vec!["syn-tag".to_owned()],
        case_sensitive: false,
        limit: uns(10),
        budget: budget(),
        image_query: None,
    }
}

pub fn coverage() -> Coverage {
    Coverage {
        discovered: U64::new(0),
        verified: U64::new(0),
        pending_hash: U64::new(0),
        pending_extraction: U64::new(0),
        pending_embedding: U64::new(0),
        stale: U64::new(0),
        unavailable: U64::new(0),
        unsupported: U64::new(0),
        failed: U64::new(0),
        excluded: U64::new(0),
        enumeration: Enumeration::Complete,
        result_extent: ResultExtent::Enumeration,
        stopped_reason: None,
    }
}

pub fn exposure() -> Exposure {
    Exposure {
        mode: ExposureMode::ReadOnly,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![omni_service::Transport::OwnerIpc],
        revision: syn_id("syn-revision"),
        pending_expansion: false,
    }
}

pub fn schedule() -> SyncSchedule {
    SyncSchedule {
        enabled: false,
        interval_minutes: uns(15),
        allow_push: false,
        next_run_at: None,
    }
}

pub fn capabilities() -> MountCapabilities {
    MountCapabilities {
        range_read: true,
        stable_file_id: false,
        reliable_watch: false,
        conditional_write: false,
        residency: true,
    }
}

pub fn mount() -> Mount {
    Mount {
        mount_id: syn_id("syn-mount"),
        name: "Synthetic mount".to_owned(),
        backend: MountBackend::Local,
        location: "synthetic-os-folder".to_owned(),
        availability: Availability::Available,
        writable: false,
        capabilities: capabilities(),
        last_complete_scan: None,
        reconciliation: Reconciliation::Manual,
    }
}

pub fn source() -> Source {
    Source {
        library_id: syn_id("syn-library"),
        source_id: syn_id("syn-source"),
        mount_id: None,
        path: rel("pages/index.md"),
        logical_path: rel("pages/index.md"),
        title: "Synthetic note".to_owned(),
        description: "A synthetic description".to_owned(),
        tags: vec!["syn-tag".to_owned()],
        annotation_path: None,
        digest: None,
        digest_state: DigestState::Unverified,
        version: None,
        size_bytes: None,
        media_type: None,
        created_at: None,
        modified_at: None,
        availability: Availability::Available,
        residency: Residency::Resident,
        activity: Activity::Stable,
        conflict_source_id: None,
    }
}

pub fn library() -> Library {
    Library {
        library_id: syn_id("syn-library"),
        name: "Synthetic".to_owned(),
        home_path: "synthetic-os-folder".to_owned(),
        text_root: TextRoot::Pages,
        manifest_version: None,
        state: LibraryState::Unmarked,
        history_mode: HistoryMode::NotInitialized,
        exposure: exposure(),
        mounts: vec![],
        sync_schedule: schedule(),
        remote_url: None,
        remote_access: RemoteAccess::None,
    }
}

pub fn request(request_id: &str, command: omni_service::Command) -> Request {
    Request {
        wire_version: 1,
        request_id: syn_id(request_id),
        command,
    }
}

pub fn response(request_id: &str, result: omni_service::Result) -> Response {
    Response {
        wire_version: 1,
        request_id: syn_id(request_id),
        result,
    }
}

pub fn golden(bytes: &str, expected: Request) {
    let raw = bytes.as_bytes();
    assert!(
        raw.len() <= MAX_CONTROL_FRAME_BYTES,
        "precondition: golden frame fits"
    );
    let decoded = decode_request(raw).unwrap_or_else(|err| panic!("decode failed: {err}"));
    assert_eq!(decoded.wire_version, 1);
    assert_eq!(decoded, expected);
    let reserialized =
        serde_json::to_vec(&decoded).unwrap_or_else(|err| panic!("reserialize: {err}"));
    let again =
        decode_request(&reserialized).unwrap_or_else(|err| panic!("redecode failed: {err}"));
    assert_eq!(again, expected);
}

pub fn refuse(bytes: &[u8]) -> CodecError {
    assert!(
        bytes.len() <= MAX_CONTROL_FRAME_BYTES,
        "precondition: refusal is inside the frame bound"
    );
    match decode_request(bytes) {
        Ok(value) => panic!("owner-invalid request decoded: {value:?}"),
        Err(err) => {
            assert!(err.source().is_none());
            err
        }
    }
}

pub fn assert_input_free(err: &impl Error, canary: &str) {
    let shown = format!("{err} {err:?}");
    assert!(!shown.contains(canary), "diagnostic retained caller input");
    assert!(err.source().is_none());
}

pub fn envelope(body: &str) -> String {
    format!(r#"{{"wire_version":1,"request_id":"syn-bound","command":{body}}}"#)
}

pub fn history_mode_signed() -> CreationHistoryMode {
    CreationHistoryMode::Signed
}

pub fn stamp() -> Time {
    Time::new("2026-11-04T00:00:00Z").expect("fixture timestamp")
}
