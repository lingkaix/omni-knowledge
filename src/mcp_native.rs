//! Frozen native-owner ports for mechanical CLI/MCP adapters.
//!
//! Transport data is never authority. Establish the actual native `Owner`/`Selection`,
//! call `connect_selected` and obtain `native_scope` for the actual routed command.
//! Reuse the existing queue's real Claim and complete prepaid requirement; preserve Waiting.
//! Query responses retain their owning encoded bytes through the last transport consumer.
//! User-visible output uses `NativePublicationPlan`, its matching planned operation and `publish`;
//! raw M3 staging methods confer no destination authority. Keep the result/error owner in the
//! foreground until real native collection and settlement finish. No static residue is success.
//! Snapshot reads use `NativeTrustedArchive` and keep the same Claim live until the last reader.
//! Startup/watch refresh uses `prepare_corpus_refresh` and `activate_corpus`, then settles its
//! existing job only after the final source/tuple consumer. Foreground adapters below retain those owners.

pub use crate::native_service::{
    EncodedResponse, NativeAdmitted, NativeAdmittedError, NativeCorpusDraft, NativeCorpusError,
    NativeCorpusRetirement, NativeDerivedRecipe, NativeEpubMember, NativeEpubPlan,
    NativeFrozenReadPlan, NativeLibraryService, NativeProcessing, NativePublicationPlan,
    NativeQueryController, NativeQueryInputs, NativeRefreshJob, NativeSnapshotError,
    NativeSnapshotPolicy, NativeSnapshotRequest, NativeSource, NativeTrustedArchive,
    NativeTupleQuery, NativeVideoPlan,
};

use crate::{
    KnowledgeBase,
    native_service::{NativeJobPlan, NativeQueryLibraryInput, NativeTupleRead},
};
use anyhow::{Context, Result, bail};
use knowledge_core::{
    admission::{Account, Decision, Holding, Origin},
    backend::Budget,
    jobs::{Claim, Outcome, Priority, Reservation},
};
use knowledge_retrieval::Work;
use knowledge_storage::Preflight;
use omni_service::{
    self as wire,
    authority::{LocalPolicy, NativeAuthority, Owner, Session, TransportFacts},
};
use serde::Serialize;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Instant,
};

const MIB: u64 = 1024 * 1024;
// One foreground product executor; this lock grants no source or queue authority.
static FOREGROUND: Mutex<()> = Mutex::new(());

fn policy(read_only: bool) -> LocalPolicy {
    LocalPolicy {
        mode: if read_only {
            wire::ExposureMode::ReadOnly
        } else {
            wire::ExposureMode::ReadWrite
        },
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![wire::Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    }
}
fn reservation(memory_bytes: u64, temporary_bytes: u64) -> Reservation {
    Reservation {
        memory_bytes,
        temporary_bytes,
    }
}
#[derive(Debug)]
struct FaceRefusal(wire::ErrorCode, Option<wire::SearchMethod>);
impl std::fmt::Display for FaceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native query refused: {:?}", self.0)
    }
}
impl std::error::Error for FaceRefusal {}
fn refusal(code: wire::ErrorCode) -> anyhow::Error {
    FaceRefusal(code, None).into()
}
fn holding(need: Reservation) -> Result<Holding> {
    match Account::current()?.reserve(need, Origin::BeforeVerification)? {
        Decision::Ready(value) => Ok(value),
        Decision::Waiting(_) => Err(refusal(wire::ErrorCode::Busy)),
    }
}
fn work(need: Reservation) -> Result<Work> {
    match Work::reserve(need, Origin::BeforeVerification)? {
        Decision::Ready(value) => Ok(value),
        Decision::Waiting(_) => Err(refusal(wire::ErrorCode::Busy)),
    }
}
fn code(error: &anyhow::Error) -> wire::ErrorCode {
    error
        .downcast_ref::<FaceRefusal>()
        .map_or_else(|| NativeTupleQuery::failure_code(error), |error| error.0)
}

/// Foreground adapter over the existing selected owners, M1 producer and ONE native query queue.
/// Startup is an owner-launched local Refresh, before read-only agent requests are accepted.
pub struct NativeQueryHost {
    services: Vec<NativeLibraryService>,
    libraries: Vec<KnowledgeBase>,
    owner: Owner,
    session: Session,
}
impl NativeQueryHost {
    /// Select the actual local homes, activate their native corpus, then install agent policy.
    pub fn open(libraries: Vec<KnowledgeBase>, read_only: bool) -> Result<Self> {
        anyhow::ensure!(!libraries.is_empty(), "select a local library");
        let _foreground = FOREGROUND
            .lock()
            .map_err(|_| refusal(wire::ErrorCode::Internal))?;
        let (_, owner) = NativeAuthority::new();
        let mut services = Vec::new();
        let mut selections = Vec::new();
        for library in &libraries {
            let (service, selection) =
                NativeLibraryService::ordinary_local_owned(library.clone(), policy(false), &owner)?;
            let session = service
                .connect_selected(&selection, TransportFacts::OwnerStdio)
                .map_err(refusal)?;
            startup(&service, &owner, &session)?;
            services.push(service);
            selections.push(selection);
        }
        for selection in &selections {
            owner
                .set_policy(selection, policy(read_only))
                .map_err(refusal)?;
        }
        let session = owner
            .authority()
            .connect(&selections, TransportFacts::OwnerStdio)
            .map_err(refusal)?;
        Ok(Self {
            services,
            libraries,
            owner,
            session,
        })
    }
    /// Actual selected library IDs, for request construction and labelled results only.
    pub fn library_ids(&self) -> Vec<wire::Id> {
        self.services
            .iter()
            .map(|s| s.library_id().clone())
            .collect()
    }
    /// Dispatch another frozen command through the original runtime, retaining its Delivery owner.
    pub fn dispatch(&self, frame: &[u8]) -> Result<wire::runtime::Delivery> {
        self.services[0]
            .dispatch(frame, &self.session)
            .map_err(anyhow::Error::new)
    }
    /// Deliver original non-query runtime results, retaining all Delivery owners through writes.
    /// Library discovery combines actual per-library dispatcher data under the SAME selection session.
    pub fn write_command_mcp(
        &self,
        request: &wire::Request,
        output: &mut impl Write,
        id: &serde_json::Value,
    ) -> Result<()> {
        let all = matches!(request.command, wire::Command::LibraryList {});
        let count = if all { self.services.len() } else { 1 };
        let _transport = work(reservation(
            (wire::MAX_CONTROL_FRAME_BYTES as u64)
                .checked_mul(count as u64)
                .and_then(|n| n.checked_mul(32))
                .context("command delivery quote overflow")?,
            0,
        ))?;
        let frame = serde_json::to_vec(request)?;
        let mut deliveries = Vec::new();
        let mut failure = None;
        let targets: Vec<_> = if all {
            self.services.iter().collect()
        } else {
            let target = command_library(&request.command)
                .and_then(|id| {
                    self.services
                        .iter()
                        .find(|service| service.library_id() == id)
                })
                .unwrap_or(&self.services[0]);
            vec![target]
        };
        for service in targets {
            match service.dispatch(&frame, &self.session) {
                Ok(delivery) => deliveries.push(delivery),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        let encoded = if let Some(error) = &failure {
            let code = match error {
                wire::runtime::RuntimeError::Admission(code) => *code,
                _ => wire::ErrorCode::InvalidRequest,
            };
            wire::encode_response(&error_response(request.request_id.clone(), code, None))?
        } else if all {
            let mut libraries = Vec::new();
            let mut refused = None;
            for delivery in &deliveries {
                delivery.check().map_err(refusal)?;
                let bytes = delivery.encode()?;
                let response = serde_json::from_slice::<wire::Response>(&bytes)?;
                match response.result {
                    wire::Result::Libraries {
                        libraries: mut selected,
                    } => libraries.append(&mut selected),
                    other => {
                        refused = Some(other);
                        break;
                    }
                }
            }
            wire::encode_response(&wire::Response {
                wire_version: 1,
                request_id: request.request_id.clone(),
                result: refused.unwrap_or(wire::Result::Libraries { libraries }),
            })?
        } else {
            deliveries[0].encode()?
        };
        let is_error = matches!(
            serde_json::from_slice::<wire::Response>(&encoded)?.result,
            wire::Result::Error { .. }
        );
        let mut checked = CommandWrite {
            output,
            deliveries: &deliveries,
        };
        checked.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":")?;
        serde_json::to_writer(&mut checked, id)?;
        checked.write_all(b",\"result\":{\"content\":[{\"type\":\"text\",\"text\":")?;
        serde_json::to_writer(&mut checked, std::str::from_utf8(&encoded)?)?;
        checked.write_all(b"}],\"structuredContent\":")?;
        checked.write_all(&encoded)?;
        writeln!(checked, ",\"isError\":{is_error}}}}}")?;
        checked.flush()?;
        drop((encoded, failure, deliveries));
        Ok(())
    }
    /// Execute and deliver synchronously while every input, Claim and error owner stays foreground.
    pub fn with_query<T>(
        &self,
        request: &wire::Request,
        deliver: impl FnOnce(&mut NativeQueryReply<'_>) -> Result<T>,
    ) -> Result<T> {
        let _serial = FOREGROUND
            .lock()
            .map_err(|_| refusal(wire::ErrorCode::Internal))?;
        let wire::Command::Query { query: search } = &request.command else {
            bail!("query command required")
        };
        // This actual bounded write window is paid before input inspection and Claim execution.
        let transport = work(reservation(32 * 1024, 0))?;
        let selected: Vec<_> = self
            .services
            .iter()
            .filter(|s| search.library_ids.contains(s.library_id()))
            .collect();
        let setup = (|| {
            anyhow::ensure!(
                selected.len() == search.library_ids.len(),
                FaceRefusal(wire::ErrorCode::NotExposed, None)
            );
            let first = selected
                .first()
                .context(FaceRefusal(wire::ErrorCode::NotExposed, None))?;
            let permit = first
                .native_scope(&self.session, &request.command, None)
                .map_err(refusal)?;
            // This host has no native model selection. Refuse at that actual capability boundary,
            // before opening inputs; configured provider strings are never model capabilities.
            if search.methods.contains(&wire::SearchMethod::Semantic) {
                return Err(FaceRefusal(
                    wire::ErrorCode::ProviderUnavailable,
                    Some(wire::SearchMethod::Semantic),
                )
                .into());
            }
            let bindings: Vec<_> = selected.iter().map(|s| (*s, "wiki")).collect();
            let need = NativeQueryInputs::requirement(&permit, &bindings)?;
            anyhow::ensure!(
                need.memory_bytes
                    .checked_add(32 * 1024)
                    .is_some_and(|n| n <= search.budget.max_memory_bytes.get()),
                FaceRefusal(wire::ErrorCode::BudgetExhausted, None)
            );
            let descriptor = work(NativeTupleQuery::metadata_requirement())?;
            let mut plans = Vec::new();
            for service in &selected {
                let need = service.query_inventory_requirement(&permit, "wiki")?;
                let reader = work(NativeTupleRead::requirement())?;
                let inventory = holding(need)?;
                plans.push(NativeQueryLibraryInput::new(
                    service,
                    &self.owner,
                    "wiki",
                    None,
                    reader,
                    inventory,
                ));
            }
            // Holding is immediately consumed by the owning constructor; partial plans own refunds.
            let scratch = holding(reservation(4 * MIB, 0))?;
            Ok((permit, plans, scratch, descriptor))
        })();
        let (permit, plans, scratch, descriptor) = match setup {
            Ok(value) => value,
            Err(error) => {
                return deliver(&mut NativeQueryReply::error(
                    request.request_id.clone(),
                    &error,
                    transport,
                ));
            }
        };
        let inputs = NativeQueryInputs::open(permit, plans, scratch, descriptor);
        let query = match inputs.finish() {
            Ok(query) => query,
            Err(inputs) => {
                let error = inputs.failure().context("native query inputs retained")?;
                return deliver(&mut NativeQueryReply::error(
                    request.request_id.clone(),
                    error,
                    transport,
                ));
            }
        };
        let requirement = match query.requirement_all() {
            Ok(value) => value,
            Err(error) => {
                return deliver(&mut NativeQueryReply::error(
                    request.request_id.clone(),
                    &error,
                    transport,
                ));
            }
        };
        let usage = Account::current()?.queue().usage()?;
        if usage.pending != 0 || usage.unsettled != 0 {
            let error = refusal(wire::ErrorCode::Busy);
            return deliver(&mut NativeQueryReply::error(
                request.request_id.clone(),
                &error,
                transport,
            ));
        }
        let submitted = match query.submit(NativeJobPlan {
            operation_id: request.request_id.as_str(),
            job_id: request.request_id.as_str(),
            budget: Budget {
                max_files: search.budget.max_files.get(),
                max_bytes: search.budget.max_bytes.get(),
                max_millis: search.budget.max_millis.get(),
                max_memory_bytes: requirement.memory_bytes,
            },
            reservation: requirement,
            priority: Priority::Normal,
            max_attempts: 1,
        }) {
            Ok(value) => value,
            Err(error) => {
                return deliver(&mut NativeQueryReply::error(
                    request.request_id.clone(),
                    &error,
                    transport,
                ));
            }
        };
        let account = Account::current()?;
        let Some(claim) = account.queue().claim(Instant::now())? else {
            // Waiting remains an actual queued owner through delivery. This foreground invocation
            // then cancels its unstarted work explicitly; it never reports execution or settlement.
            let error = refusal(wire::ErrorCode::Busy);
            let result = deliver(&mut NativeQueryReply::error(
                request.request_id.clone(),
                &error,
                transport,
            ));
            account.queue().cancel(submitted.job, query.key())?;
            account.queue().retire(submitted.job, query.key())?;
            return result;
        };
        if claim.job() != submitted.job {
            account.queue().cancel(submitted.job, query.key())?;
            account.queue().retire(submitted.job, query.key())?;
            return Err(OwnedClaim(Some(Arc::new(claim))).into());
        }
        let claim = Arc::new(claim);
        let mut controller = match query.start_controller(claim.clone()) {
            Ok(controller) => controller,
            Err(error) => {
                let delivered = deliver(&mut NativeQueryReply::error(
                    request.request_id.clone(),
                    &error,
                    transport,
                ));
                let claim = Arc::try_unwrap(claim).map_err(|claim| OwnedClaim(Some(claim)))?;
                account
                    .queue()
                    .settle(&claim, query.key(), Outcome::Partial)?;
                account.queue().retire(submitted.job, query.key())?;
                return delivered;
            }
        };
        let mut completed = controller.failure().is_none();
        let delivered = if let Some(error) = controller.failure() {
            deliver(&mut NativeQueryReply::error(
                request.request_id.clone(),
                error,
                transport,
            ))
        } else {
            match controller.encode_response(request.request_id.clone()) {
                Ok(encoded) => deliver(&mut NativeQueryReply {
                    request_id: request.request_id.clone(),
                    content: ReplyContent::Search {
                        controller: &mut controller,
                        encoded,
                    },
                    libraries: &self.libraries,
                    _transport: transport,
                }),
                Err(error) => {
                    completed = false;
                    deliver(&mut NativeQueryReply::error(
                        request.request_id.clone(),
                        &error,
                        transport,
                    ))
                }
            }
        };
        // The callback and encoded Work have ended. Finish the actual retained source/page owners
        // before ending the execution Claim, including write/flush failures.
        let (returned, alias) = controller
            .finish()
            .map_err(|owner| OwnedController(Some(owner)))?;
        drop((returned, alias));
        let claim = Arc::try_unwrap(claim).map_err(|claim| OwnedClaim(Some(claim)))?;
        account.queue().settle(
            &claim,
            query.key(),
            if completed && delivered.is_ok() {
                Outcome::Completed
            } else {
                Outcome::Partial
            },
        )?;
        account.queue().retire(submitted.job, query.key())?;
        drop((query, claim));
        delivered
    }
}

/// Owning cleanup refusal stays in the original foreground error; it is never stringified away.
struct OwnedController(Option<Box<NativeQueryController>>);
impl std::fmt::Debug for OwnedController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnedController")
    }
}
impl std::fmt::Display for OwnedController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native query consumers remain live")
    }
}
impl std::error::Error for OwnedController {}
impl Drop for OwnedController {
    fn drop(&mut self) {
        drop(self.0.take());
    }
}
struct OwnedClaim(Option<Arc<Claim>>);
impl std::fmt::Debug for OwnedClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnedClaim")
    }
}
impl std::fmt::Display for OwnedClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native query Claim remains retained")
    }
}
impl std::error::Error for OwnedClaim {}
impl Drop for OwnedClaim {
    fn drop(&mut self) {
        drop(self.0.take());
    }
}

fn startup(service: &NativeLibraryService, owner: &Owner, session: &Session) -> Result<()> {
    let request = wire::decode_request(&serde_json::to_vec(
        &serde_json::json!({"wire_version":1,"request_id":"native-startup","command":{"op":"refresh","library_id":service.library_id(),"mount_ids":[],"verification":"full_hash","embeddings":false,"operation_id":"native-startup","budget":{"max_files":1000,"max_bytes":"67108864","max_millis":120000,"max_memory_bytes":"536870912"}}}),
    )?)?;
    let permit = service
        .native_scope(session, &request.command, None)
        .map_err(refusal)?;
    let mut scratch = holding(reservation(4 * MIB, 16 * MIB))?;
    let context = match holding(reservation(2 * MIB, 0)) {
        Ok(value) => value,
        Err(error) => {
            scratch.finish()?;
            return Err(error);
        }
    };
    let inventory = service.reconcile_inventory(
        owner,
        permit.clone(),
        "wiki",
        context,
        &Preflight {
            holding: &scratch,
            budget: Budget {
                max_files: 1000,
                max_bytes: 64 * MIB,
                max_millis: 120000,
                max_memory_bytes: 4 * MIB,
            },
        },
    );
    scratch.finish()?;
    let (inventory, report) = inventory?;
    drop(report);
    let job = service.prepare_corpus_refresh(
        owner,
        permit,
        inventory,
        None,
        holding(NativeRefreshJob::metadata_requirement())?,
    )?;
    let need = job.corpus_requirement()?;
    let usage = Account::current()?.queue().usage()?;
    anyhow::ensure!(
        usage.pending == 0 && usage.unsettled == 0,
        FaceRefusal(wire::ErrorCode::Busy, None)
    );
    let submitted = job.submit(NativeJobPlan {
        operation_id: "native-startup",
        job_id: "native-startup",
        budget: Budget {
            max_files: 1000,
            max_bytes: 64 * MIB,
            max_millis: 120000,
            max_memory_bytes: 512 * MIB,
        },
        reservation: reservation(256 * MIB, need.temporary_bytes),
        priority: Priority::Normal,
        max_attempts: 1,
    })?;
    let account = Account::current()?;
    let Some(claim) = account.queue().claim(Instant::now())? else {
        account.queue().cancel(submitted.job, job.key())?;
        account.queue().retire(submitted.job, job.key())?;
        bail!(FaceRefusal(wire::ErrorCode::Busy, None))
    };
    if claim.job() != submitted.job {
        account.queue().cancel(submitted.job, job.key())?;
        account.queue().retire(submitted.job, job.key())?;
        return Err(OwnedClaim(Some(Arc::new(claim))).into());
    }
    let (draft, _) = job
        .activate_corpus(claim, |_| {})
        .map_err(|error| StartupRetained(Mutex::new(Some(error))))?;
    let (returned, claim) = draft
        .into_job()
        .map_err(|draft| StartupRetained(Mutex::new(Some(draft))))?;
    account
        .queue()
        .settle(&claim, returned.key(), Outcome::Completed)?;
    account.queue().retire(submitted.job, returned.key())?;
    drop((returned, claim));
    Ok(())
}
struct StartupRetained(Mutex<Option<NativeCorpusError>>);
impl std::fmt::Debug for StartupRetained {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StartupRetained")
    }
}
impl std::fmt::Display for StartupRetained {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native startup consumers remain retained")
    }
}
impl std::error::Error for StartupRetained {}
impl Drop for StartupRetained {
    fn drop(&mut self) {
        drop(
            self.0
                .get_mut()
                .unwrap_or_else(|poison| poison.into_inner())
                .take(),
        );
    }
}

enum ReplyContent<'a> {
    Search {
        controller: &'a mut NativeQueryController,
        encoded: EncodedResponse,
    },
    Error {
        cause: &'a anyhow::Error,
        code: wire::ErrorCode,
        method: Option<wire::SearchMethod>,
    },
}
/// Borrowed foreground delivery: the host retains every native owner until this consumer ends.
pub struct NativeQueryReply<'a> {
    request_id: wire::Id,
    content: ReplyContent<'a>,
    libraries: &'a [KnowledgeBase],
    _transport: Work,
}
impl<'a> NativeQueryReply<'a> {
    fn error(request_id: wire::Id, cause: &'a anyhow::Error, transport: Work) -> Self {
        Self {
            request_id,
            content: ReplyContent::Error {
                cause,
                code: code(cause),
                method: cause
                    .downcast_ref::<FaceRefusal>()
                    .and_then(|error| error.1)
                    .or_else(|| NativeTupleQuery::failure_method(cause)),
            },
            libraries: &[],
            _transport: transport,
        }
    }
    /// Whether native execution refused, independent of JSON-RPC framing success.
    pub fn is_error(&self) -> bool {
        matches!(self.content, ReplyContent::Error { .. })
    }
    /// Write the canonical response, rechecking before each controlled chunk and final flush.
    pub fn write_wire(&mut self, output: &mut impl Write) -> Result<()> {
        match &mut self.content {
            ReplyContent::Search {
                controller,
                encoded,
            } => {
                let mut checked = Checked { output, controller };
                checked.write_all(encoded)?;
                checked.flush()?;
            }
            ReplyContent::Error {
                cause,
                code,
                method,
            } => {
                let _owning_error = cause;
                let response = error_response(self.request_id.clone(), *code, *method);
                serde_json::to_writer(&mut *output, &response)?;
                output.flush()?;
            }
        }
        Ok(())
    }
    /// Write one MCP tool response without cloning result bodies or dropping delivery guards.
    pub fn write_mcp(
        &mut self,
        output: &mut impl Write,
        id: &serde_json::Value,
        legacy: bool,
    ) -> Result<()> {
        match &mut self.content {
            ReplyContent::Search {
                controller,
                encoded,
            } => {
                let mut checked = Checked { output, controller };
                checked.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":")?;
                serde_json::to_writer(&mut checked, id)?;
                checked.write_all(b",\"result\":{\"content\":[{\"type\":\"text\",\"text\":")?;
                if legacy {
                    checked.write_all(b"\"")?;
                    let result = controller.search_result()?;
                    serde_json::to_writer(
                        Escaped {
                            output: &mut checked,
                        },
                        &LegacyHits {
                            result,
                            libraries: self.libraries,
                        },
                    )?;
                    checked.write_all(b"\"")?;
                } else {
                    serde_json::to_writer(&mut checked, std::str::from_utf8(encoded)?)?;
                }
                checked.write_all(b"}]")?;
                if !legacy {
                    checked.write_all(b",\"structuredContent\":")?;
                    checked.write_all(encoded)?;
                }
                checked.write_all(b",\"isError\":false}}\n")?;
                checked.flush()?;
            }
            ReplyContent::Error {
                cause,
                code,
                method,
            } => {
                let _owning_error = cause;
                let response = error_response(self.request_id.clone(), *code, *method);
                serde_json::to_writer(
                    &mut *output,
                    &serde_json::json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":serde_json::to_string(&response)?}],"structuredContent":response,"isError":true}}),
                )?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
        Ok(())
    }
    /// Write complete labelled SearchResult data for an explicitly canonical CLI request.
    pub fn write_search(&mut self, output: &mut impl Write) -> Result<()> {
        let ReplyContent::Search { controller, .. } = &mut self.content else {
            return self.write_wire(output);
        };
        let result = controller.search_result()?;
        let mut checked = Checked { output, controller };
        serde_json::to_writer(&mut checked, result)?;
        checked.flush()?;
        Ok(())
    }
    /// Print native per-library hits without sorting incomparable scores across groups.
    pub fn write_text(&mut self, output: &mut impl Write) -> Result<()> {
        let ReplyContent::Search { controller, .. } = &mut self.content else {
            return self.write_wire(output);
        };
        let result = controller.search_result()?;
        let mut checked = Checked { output, controller };
        for group in &result.groups {
            for hit in &group.files {
                writeln!(
                    checked,
                    "{}  {}  {}",
                    group.library_id.as_str(),
                    hit.source.path.as_str(),
                    hit.score.map_or(0.0, |score| score.get())
                )?;
                if let Some(evidence) = hit.evidence.first() {
                    writeln!(checked, "  {}", evidence.snippet)?;
                }
            }
        }
        checked.flush()?;
        Ok(())
    }
    /// Legacy array projection for the original CLI/MCP aliases; groups retain native ordering.
    pub fn write_legacy(&mut self, output: &mut impl Write) -> Result<()> {
        let ReplyContent::Search { controller, .. } = &mut self.content else {
            return self.write_wire(output);
        };
        let result = controller.search_result()?;
        let mut checked = Checked { output, controller };
        serde_json::to_writer(
            &mut checked,
            &LegacyHits {
                result,
                libraries: self.libraries,
            },
        )?;
        checked.flush()?;
        Ok(())
    }
}
/// Translate the original single-string aliases without changing whole tag or keyword values.
pub fn legacy_request(
    host: &NativeQueryHost,
    query: &str,
    mode: &str,
    tags: &[String],
    limit: usize,
    case_sensitive: bool,
    image: bool,
) -> Result<wire::Request> {
    let methods = match mode {
        "bm25" => vec!["bm25"],
        "literal" => vec!["literal"],
        "regex" | "grep" => vec!["regex"],
        "files" => vec!["files"],
        "tags" => vec!["tags"],
        "semantic" => vec!["semantic"],
        "hybrid" => vec!["bm25", "semantic"],
        _ => bail!("unknown search mode"),
    };
    anyhow::ensure!(
        !image || methods.contains(&"semantic"),
        "image query requires semantic mode"
    );
    let keywords: Vec<_> = if query.is_empty() {
        vec![]
    } else {
        vec![query]
    };
    wire::decode_request(&serde_json::to_vec(&serde_json::json!({"wire_version":1,"request_id":"foreground-query","command":{"op":"query","query":{"library_ids":host.library_ids(),"keywords":keywords,"methods":methods,"required_tags":tags,"case_sensitive":case_sensitive,"limit":limit,"image_query":null,"budget":{"max_files":1000,"max_bytes":"67108864","max_millis":60000,"max_memory_bytes":"536870912"}}}}))?).map_err(Into::into)
}

/// Fixed public error metadata; original owning failures remain with the synchronous reply.
pub fn error_response(
    request_id: wire::Id,
    code: wire::ErrorCode,
    method: Option<wire::SearchMethod>,
) -> wire::Response {
    wire::Response {
        wire_version: 1,
        request_id,
        result: wire::Result::Error {
            error: wire::ServiceError {
                code,
                message: if code == wire::ErrorCode::Busy {
                    "Native query is waiting for resources"
                } else {
                    "Native query refused"
                }
                .into(),
                retryable: code == wire::ErrorCode::Busy,
                operation_id: None,
                library_id: None,
                source_id: None,
                job_id: None,
                method,
                recovery_paths: vec![],
            },
        },
    }
}
struct Checked<'a, W> {
    output: &'a mut W,
    controller: &'a NativeQueryController,
}
impl<W: Write> Write for Checked<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.controller.search_result().map_err(io::Error::other)?;
        self.output.write(&bytes[..bytes.len().min(16 * 1024)])
    }
    fn flush(&mut self) -> io::Result<()> {
        self.controller.search_result().map_err(io::Error::other)?;
        self.output.flush()
    }
}
struct Escaped<'a, W> {
    output: &'a mut W,
}
impl<W: Write> Write for Escaped<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut start = 0;
        for (i, b) in bytes.iter().copied().enumerate() {
            let escape = match b {
                b'"' => Some(b"\\\"".as_slice()),
                b'\\' => Some(b"\\\\".as_slice()),
                b'\n' => Some(b"\\n".as_slice()),
                b'\r' => Some(b"\\r".as_slice()),
                b'\t' => Some(b"\\t".as_slice()),
                _ => None,
            };
            if let Some(escape) = escape {
                self.output.write_all(&bytes[start..i])?;
                self.output.write_all(escape)?;
                start = i + 1;
            } else if b < 0x20 {
                self.output.write_all(&bytes[start..i])?;
                let hex = b"0123456789abcdef";
                self.output.write_all(&[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    hex[(b >> 4) as usize],
                    hex[(b & 15) as usize],
                ])?;
                start = i + 1;
            }
        }
        self.output.write_all(&bytes[start..])?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
struct LegacyHits<'a> {
    result: &'a wire::SearchResult,
    libraries: &'a [KnowledgeBase],
}
impl Serialize for LegacyHits<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(None)?;
        for group in &self.result.groups {
            let root = self
                .libraries
                .iter()
                .find(|kb| kb.config.library_id.as_deref() == Some(group.library_id.as_str()))
                .map(|kb| &kb.root);
            for hit in &group.files {
                let evidence = hit.evidence.first();
                let (line, start_seconds, end_seconds) = match evidence.map(|e| &e.locator) {
                    Some(
                        wire::Locator::OriginalLines { start_line, .. }
                        | wire::Locator::RepresentationLines { start_line, .. },
                    ) => (Some(start_line.get()), None, None),
                    Some(wire::Locator::Page { start_line, .. }) => {
                        (start_line.map(|line| line.get()), None, None)
                    }
                    Some(wire::Locator::Time {
                        start_ms, end_ms, ..
                    }) => (
                        None,
                        Some(start_ms.get() as f64 / 1000.0),
                        Some(end_ms.get() as f64 / 1000.0),
                    ),
                    _ => (None, None, None),
                };
                let mode = if self.result.applied.methods.len() > 1 {
                    "hybrid"
                } else {
                    match self.result.applied.methods[0] {
                        wire::SearchMethod::Bm25 => "bm25",
                        wire::SearchMethod::Literal => "literal",
                        wire::SearchMethod::Regex => "grep",
                        wire::SearchMethod::Files => "files",
                        wire::SearchMethod::Tags => "tags",
                        wire::SearchMethod::Semantic => "semantic",
                    }
                };
                seq.serialize_element(&LegacyHit {
                    root: root.map(|root| root.as_path()),
                    path: hit.source.path.as_str(),
                    title: &hit.source.title,
                    tags: &hit.source.tags,
                    score: hit.score.map_or(0.0, |score| score.get()),
                    snippet: evidence.map(|e| e.snippet.as_str()).unwrap_or(""),
                    mode,
                    line,
                    start_seconds,
                    end_seconds,
                })?;
            }
        }
        seq.end()
    }
}

#[derive(Serialize)]
struct LegacyHit<'a> {
    root: Option<&'a std::path::Path>,
    path: &'a str,
    title: &'a str,
    tags: &'a [String],
    score: f64,
    snippet: &'a str,
    mode: &'a str,
    line: Option<u64>,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
}

struct CommandWrite<'a, W> {
    output: &'a mut W,
    deliveries: &'a [wire::runtime::Delivery],
}
impl<W: Write> Write for CommandWrite<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for delivery in self.deliveries {
            delivery
                .check()
                .map_err(|code| io::Error::other(format!("native delivery {code:?}")))?;
        }
        self.output.write(&bytes[..bytes.len().min(16 * 1024)])
    }
    fn flush(&mut self) -> io::Result<()> {
        for delivery in self.deliveries {
            delivery
                .check()
                .map_err(|code| io::Error::other(format!("native delivery {code:?}")))?;
        }
        self.output.flush()
    }
}

// Routing DATA from the frozen command. The chosen runtime still performs actual scope admission.
fn command_library(command: &wire::Command) -> Option<&wire::Id> {
    use wire::Command::*;
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
        | ExposureSet { library_id, .. }
        | ImportFile { library_id, .. } => Some(library_id),
        SourceDetails { source }
        | VerifySource { source, .. }
        | ReadRange { source, .. }
        | ReadRepresentation { source, .. }
        | Annotate { source, .. }
        | Trash { source, .. }
        | Transfer { source, .. } => Some(&source.library_id),
        EditText { target, .. } => Some(&target.library_id),
        OperationStatus { library_id, .. } => library_id.as_ref(),
        _ => None,
    }
}
