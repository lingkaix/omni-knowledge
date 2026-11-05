//! One native dispatcher over the existing command catalog, shared by every transport.
//!
//! Concrete domain hosts own durable/current-version semantics. They receive live native permits,
//! not wire claims; unresolved hostcalls must return an honest typed error, never invented effects.
use crate::authority::{HumanApproval, NativeAuthority, Permit, Session};
use crate::{
    CodecError, Command, ErrorCode, Id, RelativePath, Request, Response, Result as WireResult,
    ServiceError, WIRE_VERSION, decode_request, encode_response,
};
use knowledge_core::admission::{Account, Decision, Holding, Origin};
use knowledge_core::jobs::Reservation;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Hostcall responsibility, not a second command registry or public wire operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    /// Native selected-library/mount/exposure administration and inspection.
    Registry,
    /// Safe source reads, apply, recovery, annotations and tracking.
    Storage,
    /// Text Git/proposal/remote transactions.
    History,
    /// Shared jobs and bounded extraction/media/model work.
    Processing,
    /// Shared actual lexical kernel and retrieval orchestration.
    Retrieval,
    /// Hostile archive inspection, plans, capture, export and restore.
    Package,
}
/// Native domain-resolved references. These facts never widen the caller's selections.
#[derive(Default)]
pub struct ResolvedScope {
    /// All resolved source/job/plan libraries, including cross-library input and destination.
    pub libraries: Vec<Id>,
    /// All resolved original/annotation/history paths before enumeration/stat/read/effect.
    pub paths: Vec<(Id, RelativePath)>,
}
/// Trusted composition of real domain implementations, not an alternate operation implementation.
pub trait DomainHost {
    /// Resolve opaque job/source/plan IDs only within `permit`'s native selections.
    /// This call must not contact an unselected root, expose excluded names, or start effects.
    fn resolve(
        &mut self,
        command: &Command,
        permit: &Permit,
    ) -> std::result::Result<ResolvedScope, ErrorCode>;
    /// Invoke the actual owner API, enforcing expected versions and truthful outcomes.
    /// Retained asynchronous jobs must retain/check a cloned permit before each effect and delivery.
    /// Agent text mutations must produce proposals, not human-authored saves or self-confirmation.
    fn execute(
        &mut self,
        domain: Domain,
        command: &Command,
        permit: &Permit,
    ) -> std::result::Result<WireResult, ErrorCode>;
}

/// Refusal before an admitted response exists; neither case implies any domain effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    /// Existing typed codec refusal, before native hostcalls.
    Codec(CodecError),
    /// Shared-account refusal before allocating decoded request state.
    Admission(ErrorCode),
}
impl From<CodecError> for RuntimeError {
    /// Retain the exact codec classification without input diagnostics.
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}
impl std::fmt::Display for RuntimeError {
    /// Emit only fixed error classes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native runtime: {self:?}")
    }
}
impl std::error::Error for RuntimeError {}

/// Native dispatcher sharing Core's process owner; no transport-specific authority or resource pool.
pub struct Runtime {
    authority: NativeAuthority,
}
impl Runtime {
    /// Bind native authority. Process startup must already establish the sole Core `Account`.
    pub fn new(authority: NativeAuthority) -> Self {
        Self { authority }
    }
    /// Native CLI/domain composition over the SAME catalog, session, approval and account checks.
    /// This starts no effect. Resolve every actual library/path with this permit before IO; DTO
    /// IDs/flags are not source authority. Retain it and paid buffers through actual delivery.
    pub fn native_scope(
        &self,
        session: &Session,
        command: &Command,
        approval: Option<HumanApproval>,
    ) -> std::result::Result<Permit, ErrorCode> {
        let resources = std::sync::Arc::new(reserve_frame()?);
        crate::codec::validate_native_command(command).map_err(|_| ErrorCode::InvalidRequest)?;
        let (_, write, owner) = route(command);
        let mut permit = self.authority.admit(
            session,
            command,
            &declared_libraries(command),
            write,
            owner,
            approval,
        )?;
        permit.bind_resources(resources);
        permit.check()?;
        Ok(permit)
    }
    /// Decode one complete existing request and dispatch as an agent without human capability.
    pub fn dispatch(
        &self,
        bytes: &[u8],
        session: &Session,
        host: &mut impl DomainHost,
    ) -> std::result::Result<Delivery, RuntimeError> {
        self.dispatch_approved(bytes, session, host, None)
    }
    /// Dispatch an exact separately confirmed native intention; no JSON field can supply approval.
    pub fn dispatch_approved(
        &self,
        bytes: &[u8],
        session: &Session,
        host: &mut impl DomainHost,
        approval: Option<HumanApproval>,
    ) -> std::result::Result<Delivery, RuntimeError> {
        if bytes.len() > crate::MAX_CONTROL_FRAME_BYTES {
            return Err(CodecError::FrameTooLarge.into());
        }
        // The transport retains its input-frame charge; this charge precedes decoded allocations.
        let resources = std::sync::Arc::new(reserve_frame().map_err(RuntimeError::Admission)?);
        let request = decode_request(bytes)?;
        let (result, permit) = match self.run(&request, session, host, approval, resources.clone())
        {
            Ok((result, permit)) => (result, Some(permit)),
            Err(code) => (failure(code), None),
        };
        let response = Response {
            wire_version: WIRE_VERSION,
            request_id: request.request_id,
            result,
        };
        Ok(Delivery {
            response,
            permit,
            _resources: Some(resources),
        })
    }
    /// Authorize before native resolution, then before execution and result release.
    fn run(
        &self,
        request: &Request,
        session: &Session,
        host: &mut impl DomainHost,
        approval: Option<HumanApproval>,
        resources: std::sync::Arc<FrameResources>,
    ) -> std::result::Result<(WireResult, Permit), ErrorCode> {
        let (domain, write, owner) = route(&request.command);
        let libraries = declared_libraries(&request.command);
        let mut permit = self.authority.admit(
            session,
            &request.command,
            &libraries,
            write,
            owner,
            approval,
        )?;
        permit.bind_resources(resources);
        let resolved = host
            .resolve(&request.command, &permit)
            .map_err(redact_resolution)?;
        for id in &resolved.libraries {
            permit.check_library(id)?;
        }
        for (library, path) in &resolved.paths {
            permit.check_path(library, path)?;
        }
        permit.check()?;
        let result = host.execute(domain, &request.command, &permit)?;
        permit.check()?;
        Ok((result, permit))
    }
    /// Open only the existing HTTP job-status snapshot route; admission precedes capacity checking.
    /// EOF/capacity/deadline never asserts completion and does not cancel the underlying job.
    pub fn status_stream<'a>(
        &'a self,
        bytes: &[u8],
        session: &Session,
    ) -> std::result::Result<StatusStream<'a>, ErrorCode> {
        let resources = reserve_frame()?;
        let request = decode_request(bytes).map_err(|_| ErrorCode::InvalidRequest)?;
        if !matches!(request.command, Command::JobStatus { .. }) || !session.is_http() {
            return Err(ErrorCode::InvalidRequest);
        }
        self.authority
            .admit(session, &request.command, &[], false, false, None)?;
        STREAMS
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 16).then_some(n + 1)
            })
            .map_err(|_| ErrorCode::BudgetExhausted)?;
        Ok(StatusStream {
            runtime: self,
            session: session.clone(),
            request: bytes.to_vec(),
            started: Instant::now(),
            last: None,
            frames: 0,
            stopped: false,
            _resources: resources,
        })
    }
}

/// One bounded response with its resource and current native-policy lifetime.
/// The transport must retain this object until its complete bounded write has settled.
pub struct Delivery {
    response: Response,
    permit: Option<Permit>,
    _resources: Option<std::sync::Arc<FrameResources>>,
}
impl Delivery {
    /// Move a genuine native scope into the existing delivery owner. The caller must reserve the
    /// complete response/encoding peak BEFORE constructing it; no bare buffer settles this owner.
    pub fn from_native(response: Response, permit: Permit) -> std::result::Result<Self, ErrorCode> {
        permit.reserve_result(0)?;
        Ok(Self {
            response,
            permit: Some(permit),
            _resources: None,
        })
    }
    /// Recheck native context immediately before serialization/delivery; revocation redacts all data.
    pub fn encode(&self) -> std::result::Result<Vec<u8>, CodecError> {
        if self.permit.as_ref().is_some_and(|p| p.check().is_err()) {
            return encode_response(&Response {
                wire_version: WIRE_VERSION,
                request_id: self.response.request_id.clone(),
                result: failure(ErrorCode::NotExposed),
            });
        }
        match encode_response(&self.response) {
            Err(CodecError::FrameTooLarge) => encode_response(&Response {
                wire_version: WIRE_VERSION,
                request_id: self.response.request_id.clone(),
                result: failure(ErrorCode::BudgetExhausted),
            }),
            result => result,
        }
    }
    /// Recheck before each controllable transport write/chunk; do not deliver old encoded bytes after refusal.
    pub fn check(&self) -> std::result::Result<(), ErrorCode> {
        self.permit.as_ref().map_or(Ok(()), Permit::check)
    }
    /// Whether the delivered result is a typed error, independent of transport/MCP protocol errors.
    pub fn is_error(&self) -> bool {
        matches!(self.response.result, WireResult::Error { .. })
            || self.check().is_err()
            || encode_response(&self.response).is_err()
    }
}

/// Service-wide bound, shared even by multiple native Runtime values.
static STREAMS: AtomicUsize = AtomicUsize::new(0);
/// Bounded coalescing snapshots of one admitted job; this is not a durable subscription/replay log.
pub struct StatusStream<'a> {
    runtime: &'a Runtime,
    session: Session,
    request: Vec<u8>,
    started: Instant,
    last: Option<Instant>,
    frames: u8,
    stopped: bool,
    _resources: FrameResources,
}
impl StatusStream<'_> {
    /// Return an immediate snapshot, then at most one per second, for at most 30s/32 frames.
    /// None means no frame now or budget-ended, never job completion; inspect `ended` separately.
    pub fn next(
        &mut self,
        host: &mut impl DomainHost,
    ) -> std::result::Result<Option<Delivery>, RuntimeError> {
        let now = Instant::now();
        if self.stopped
            || now.duration_since(self.started) >= Duration::from_secs(30)
            || self.frames >= 32
        {
            self.stopped = true;
            return Ok(None);
        }
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs(1))
        {
            return Ok(None);
        }
        let delivery = match self.runtime.dispatch(&self.request, &self.session, host) {
            Ok(delivery) => delivery,
            Err(error) => {
                self.stopped = true;
                return Err(error);
            }
        };
        self.frames += 1;
        self.last = Some(now);
        self.stopped = delivery.is_error() || terminal(&delivery.response.result);
        Ok(Some(delivery))
    }
    /// Whether this connection has stopped; it makes no assertion about the underlying job outcome.
    pub fn ended(&self) -> bool {
        self.stopped || self.started.elapsed() >= Duration::from_secs(30) || self.frames >= 32
    }
}
impl Drop for StatusStream<'_> {
    /// Release only the exact connection slot; never cancel a job or release its resource holding.
    fn drop(&mut self) {
        STREAMS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// An actual frame-lifetime charge from Core, not another transport admission controller.
pub(crate) struct FrameResources {
    holding: std::sync::Mutex<Holding>,
    checks: std::sync::Mutex<Vec<crate::authority::DeliveryCheck>>,
}
impl FrameResources {
    pub(crate) fn grow(&self, bytes: u64) -> std::result::Result<(), ErrorCode> {
        match self
            .holding
            .lock()
            .map_err(|_| ErrorCode::Internal)?
            .grow(Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            })
            .map_err(|_| ErrorCode::BudgetExhausted)?
        {
            Decision::Ready(()) => Ok(()),
            Decision::Waiting(_) => Err(ErrorCode::Busy),
        }
    }
}
impl FrameResources {
    pub(crate) fn retain_check(
        &self,
        check: crate::authority::DeliveryCheck,
    ) -> std::result::Result<(), ErrorCode> {
        self.grow(1024)?;
        self.checks
            .lock()
            .map_err(|_| ErrorCode::Internal)?
            .push(check);
        Ok(())
    }
    pub(crate) fn check(&self) -> std::result::Result<(), ErrorCode> {
        if self
            .checks
            .lock()
            .map_err(|_| ErrorCode::Internal)?
            .iter()
            .all(|check| check())
        {
            Ok(())
        } else {
            Err(ErrorCode::Changed)
        }
    }
}
impl Drop for FrameResources {
    /// Payload and host execution have settled before the owning Delivery releases this resource.
    fn drop(&mut self) {
        // Captured native pins/FDs and their actual state checks die before their aggregate charge.
        if let Ok(checks) = self.checks.get_mut() {
            checks.clear();
        }
        if let Ok(holding) = self.holding.get_mut() {
            let _ = holding.finish();
        }
    }
}
/// Reserve conservative bounded request/result/serialization space before calling domain owners.
fn reserve_frame() -> std::result::Result<FrameResources, ErrorCode> {
    let account = Account::current().map_err(|_| ErrorCode::Busy)?;
    match account
        .reserve(
            Reservation {
                memory_bytes: 64 * 1024 * 1024,
                temporary_bytes: 0,
            },
            Origin::BeforeVerification,
        )
        .map_err(|_| ErrorCode::BudgetExhausted)?
    {
        Decision::Ready(holding) => Ok(FrameResources {
            holding: std::sync::Mutex::new(holding),
            checks: std::sync::Mutex::new(Vec::new()),
        }),
        Decision::Waiting(_) => Err(ErrorCode::Busy),
    }
}
/// Redact existence-bearing opaque ID/path resolution errors at the exposure boundary.
fn redact_resolution(code: ErrorCode) -> ErrorCode {
    match code {
        ErrorCode::NotFound | ErrorCode::Excluded | ErrorCode::Denied => ErrorCode::NotExposed,
        code => code,
    }
}
/// Produce fixed, non-disclosing diagnostics without echoing a root, query or argument.
fn failure(code: ErrorCode) -> WireResult {
    WireResult::Error {
        error: ServiceError {
            code,
            message: "native request refused".into(),
            retryable: matches!(code, ErrorCode::Busy),
            operation_id: None,
            library_id: None,
            source_id: None,
            job_id: None,
            method: None,
            recovery_paths: vec![],
        },
    }
}
/// Inspect only actual terminal job states; EOF and a requested cancellation are not terminal.
fn terminal(result: &WireResult) -> bool {
    if let WireResult::Job { job } = result {
        matches!(
            job.state,
            crate::JobState::Completed
                | crate::JobState::Failed
                | crate::JobState::Cancelled
                | crate::JobState::Partial
                | crate::JobState::Superseded
        )
    } else {
        false
    }
}

/// Exhaustive responsibility/admission match over the existing version-1 Command enum.
/// Adding an operation cannot silently acquire a default read or owner exemption.
pub fn route(command: &Command) -> (Domain, bool, bool) {
    use Command::*;
    match command {
        LibraryList {} | LibraryStatus { .. } | MountList { .. } => {
            (Domain::Registry, false, false)
        }
        LibraryOpen { .. }
        | LibraryCreate { .. }
        | LibraryClone { .. }
        | MountAttach { .. }
        | MountDetach { .. }
        | ExposureSet { .. } => (Domain::Registry, true, true),
        ExposureGet { .. } => (Domain::Registry, false, true),
        Browse { .. }
        | SourceDetails { .. }
        | VerifySource { .. }
        | ReadRange { .. }
        | ReadRepresentation { .. } => (Domain::Storage, false, false),
        Annotate { .. } | EditText { .. } | ImportFile { .. } | Transfer { .. } => {
            (Domain::Storage, true, false)
        }
        Trash { .. } => (Domain::Storage, true, true),
        Refresh { .. } | Process { .. } | Pin { .. } | Cancel { .. } => {
            (Domain::Processing, true, false)
        }
        Hydrate { .. } => (Domain::Processing, true, true),
        JobStatus { .. } | OperationStatus { .. } => (Domain::Processing, false, false),
        Query { .. } => (Domain::Retrieval, false, false),
        HistoryList { .. } | HistoryDiff { .. } | ProposalList { .. } | ProposalDiff { .. } => {
            (Domain::History, false, false)
        }
        ConfirmMerge { .. } | Sync { .. } | SyncSchedule { .. } => (Domain::History, true, true),
        EpubPlan { .. }
        | EpubExport { .. }
        | SnapshotPlan { .. }
        | SnapshotCreate { .. }
        | PackageInspect { .. }
        | OpenFrozen { .. }
        | RestorePlan { .. }
        | RestoreApply { .. }
        | MigrationPlan { .. }
        | MigrationApply { .. }
        | V1ImportPlan { .. }
        | V1ExportPlan { .. } => (Domain::Package, true, true),
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod native_tests;

/// Collect only explicit declared library labels; native resolved IDs are checked separately.
fn declared_libraries(command: &Command) -> Vec<Id> {
    use Command::*;
    match command {
        LibraryStatus { library_id }
        | MountAttach { library_id, .. }
        | MountDetach { library_id, .. }
        | MountList { library_id }
        | Browse { library_id, .. }
        | Hydrate { library_id, .. }
        | Refresh { library_id, .. }
        | Pin { library_id, .. }
        | Process { library_id, .. }
        | HistoryList { library_id, .. }
        | HistoryDiff { library_id, .. }
        | ProposalList { library_id }
        | ProposalDiff { library_id, .. }
        | ConfirmMerge { library_id, .. }
        | Sync { library_id, .. }
        | SyncSchedule { library_id, .. }
        | EpubPlan { library_id, .. }
        | EpubExport { library_id, .. }
        | SnapshotPlan { library_id, .. }
        | SnapshotCreate { library_id, .. }
        | MigrationPlan { library_id }
        | MigrationApply { library_id, .. }
        | V1ExportPlan { library_id, .. }
        | ExposureGet { library_id }
        | ExposureSet { library_id, .. } => vec![library_id.clone()],
        SourceDetails { source }
        | VerifySource { source, .. }
        | ReadRange { source, .. }
        | ReadRepresentation { source, .. }
        | Annotate { source, .. }
        | Trash { source, .. } => vec![source.library_id.clone()],
        EditText { target, .. } => vec![target.library_id.clone()],
        ImportFile {
            library_id,
            input,
            destination,
            ..
        } => vec![
            library_id.clone(),
            input.library_id.clone(),
            destination.library_id.clone(),
        ],
        Transfer {
            source,
            destination,
            ..
        } => vec![source.library_id.clone(), destination.library_id.clone()],
        Query { query } => query.library_ids.clone(),
        OperationStatus { library_id, .. } => library_id.iter().cloned().collect(),
        LibraryList {}
        | LibraryOpen { .. }
        | LibraryCreate { .. }
        | LibraryClone { .. }
        | JobStatus { .. }
        | Cancel { .. }
        | PackageInspect { .. }
        | OpenFrozen { .. }
        | RestorePlan { .. }
        | RestoreApply { .. }
        | V1ImportPlan { .. } => vec![],
    }
}
