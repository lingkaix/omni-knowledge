//! Visible durable due/dispatch facts over the host's existing Core queue, never a scheduler/executor pool.
//! Scope/descriptor mismatch, exhausted capacity and incomplete domain proof refuse; no error drops an actual native owner or settles unknown work.
#![deny(missing_docs)]
use super::*;
use knowledge_core::jobs::{
    JobError, Key, Outcome, Queue, State as QueueState, Submission, Submitted,
};

/// Live native schedule intention, separate from stored settings and ordinary remote consent.
/// Only explicit native enablement creates this capability; status/URL/ref/clock data cannot recreate it.
pub struct ScheduleConsent {
    /// Actual current native owner selection.
    owner: NativeHistoryOwner,
    /// Exact selected schedule identity.
    id: String,
    /// Exact selected remote scope, or external observation.
    endpoint: Option<String>,
    /// Native-selected cadence, never widened by durable settings.
    interval_minutes: u32,
    /// Native human scheduled-push intention, distinct from ordinary remote push rights.
    allow_push: bool,
    /// Native-selected complete operation recipe.
    recipe: knowledge_core::data::ContentDigest,
}
impl ScheduleConsent {
    /// Recheck current native capability and all originally selected scheduling effects, independently of stored flags.
    fn check(&self, history: &History, id: &str, state: &HistorySchedule) -> Result<(), Error> {
        self.owner.check(history)?;
        if self.id != id
            || self.endpoint != state.endpoint
            || self.interval_minutes != state.interval_minutes
            || self.allow_push != state.allow_push
            || state.base_recipe.as_ref() != Some(&self.recipe)
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    /// Keep remote and schedule intentions on the same actual native selection, not merely matching IDs.
    fn remote(&self, consent: &RemoteConsent) -> Result<(), Error> {
        if !std::sync::Arc::ptr_eq(&self.owner.selected, &consent.owner.selected)
            || self.endpoint.as_deref() != Some(&consent.fingerprint())
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
/// Local schedule facts, never permission to contact a stored endpoint or approve a proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistorySchedule {
    /// Local desired dispatch state, never a restored effect capability.
    pub enabled: bool,
    /// Explicit positive interval when enabled; zero only in the disabled default.
    pub interval_minutes: u32,
    /// Narrowed native scheduled push choice, still bounded by current remote consent.
    pub allow_push: bool,
    /// Informational due/backoff time; current native admission is checked separately.
    pub next_run_unix_millis: Option<u64>,
    /// Stable current occurrence identity used by the actual queue and Git receipts.
    pub operation_id: String,
    /// Last actual Core attempt count, not an inferred completion percentage.
    pub attempts: u64,
    /// Selected remote fingerprint, or external-text observation kind.
    pub(crate) endpoint: Option<String>,
    /// Full submitted key/budget/reservation digest, retained until domain completion.
    pub(crate) descriptor: Option<String>,
    /// Monotonic local occurrence sequence, not an authority epoch.
    pub(crate) generation: u64,
    /// Native-selected immutable operation recipe before occurrence qualification.
    pub(crate) base_recipe: Option<knowledge_core::data::ContentDigest>,
}
impl HistorySchedule {
    /// Qualify the actual immutable recipe by occurrence, so Core terminal-key dedup cannot swallow a later scheduled poll.
    /// This identifies work only; native effect permission is still checked independently.
    pub fn work_recipe(&self) -> Result<knowledge_core::data::ContentDigest, Error> {
        let base = self.base_recipe.as_ref().ok_or(Error::Invalid)?;
        knowledge_core::data::ContentDigest::new(digest(
            &serde_json::to_vec(&(
                base.as_str(),
                &self.operation_id,
                &self.endpoint,
                self.allow_push,
            ))
            .map_err(|_| Error::Invalid)?,
        ))
        .map_err(|_| Error::Invalid)
    }
}
/// Actual scheduled remote work, including any strictly leased permitted push.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledRemoteResult {
    /// Actual fetched proposal and bounded comparison.
    pub pull: RemotePull,
    /// Actual leased push only when explicitly enabled and local ancestry is ahead.
    pub push: Option<RemotePush>,
}
impl History {
    /// Read bounded durable scheduling facts. New libraries default disabled/no-push without remote contact.
    pub fn schedule_status(
        &self,
        id: &str,
        owner: &NativeHistoryOwner,
    ) -> Result<HistorySchedule, Error> {
        owner.check(self)?;
        let path = self.schedule_path(id)?;
        if path.exists() {
            let state: HistorySchedule = load(&path)?;
            validate_id(&state.operation_id).map_err(|_| Error::Recovery)?;
            if state.operation_id != scheduled_id(id, state.generation)
                || (state.enabled && (state.interval_minutes == 0 || state.base_recipe.is_none()))
                || (!state.enabled && state.next_run_unix_millis.is_some())
                || (state.allow_push && state.endpoint.is_none())
                || state
                    .endpoint
                    .as_ref()
                    .is_some_and(|value| !valid_digest(value))
                || state
                    .descriptor
                    .as_ref()
                    .is_some_and(|value| !valid_digest(value))
            {
                return Err(Error::Recovery);
            }
            Ok(state)
        } else {
            Ok(new_schedule(id))
        }
    }
    /// Explicit native enablement of one exact selected remote and its narrowed effects.
    /// This method does not itself fetch, submit work, confirm integration or grant wider roots.
    pub fn enable_remote_schedule(
        &self,
        id: &str,
        consent: &RemoteConsent,
        interval_minutes: u32,
        allow_push: bool,
        work: HistoryWork<'_>,
    ) -> Result<(HistorySchedule, ScheduleConsent), Error> {
        let call = work.call(self, &consent.owner)?;
        consent.check(self, allow_push, &call)?;
        self.configure_schedule(
            id,
            Some(consent.fingerprint()),
            (interval_minutes, allow_push),
            work.key.recipe,
            &consent.owner,
            &call,
        )
    }
    /// Explicit native enablement of external text observation through the same queue.
    pub fn enable_external_schedule(
        &self,
        id: &str,
        owner: &NativeHistoryOwner,
        interval_minutes: u32,
        work: HistoryWork<'_>,
    ) -> Result<(HistorySchedule, ScheduleConsent), Error> {
        let call = work.call(self, owner)?;
        self.configure_schedule(
            id,
            None,
            (interval_minutes, false),
            work.key.recipe,
            owner,
            &call,
        )
    }
    /// Stop future dispatch, without claiming to stop/settle an actual running queue claim.
    pub fn disable_schedule(
        &self,
        id: &str,
        owner: &NativeHistoryOwner,
        work: HistoryWork<'_>,
    ) -> Result<HistorySchedule, Error> {
        let call = work.call(self, owner)?;
        let _lock = self.lock(&call)?;
        let mut state = self.schedule_status(id, owner)?;
        state.enabled = false;
        state.next_run_unix_millis = None;
        replace_schedule(&self.schedule_path(id)?, &state, &call)?;
        Ok(state)
    }
    /// Submit/recover one due remote descriptor to the EXISTING queue; no claim or worker is manufactured here.
    /// Stable operation/job IDs come from schedule_status; full versions/recipes/resources come from actual native admission.
    pub fn submit_remote_schedule(
        &self,
        id: &str,
        schedule: &ScheduleConsent,
        consent: &RemoteConsent,
        queue: &Queue,
        submission: Submission<'_>,
    ) -> Result<Option<Submitted>, Error> {
        schedule.remote(consent)?;
        schedule.check(self, id, &self.schedule_status(id, &schedule.owner)?)?;
        self.submit_schedule(
            id,
            Some(&consent.fingerprint()),
            schedule,
            queue,
            submission,
        )
    }
    /// Submit actual externally verified batch work; its key.version must be external_batch_version at execution.
    pub fn submit_external_schedule(
        &self,
        id: &str,
        schedule: &ScheduleConsent,
        queue: &Queue,
        submission: Submission<'_>,
    ) -> Result<Option<Submitted>, Error> {
        schedule.check(self, id, &self.schedule_status(id, &schedule.owner)?)?;
        self.submit_schedule(id, None, schedule, queue, submission)
    }
    /// Execute a claimed scheduled remote job using actual native Git and the original complete queue binding.
    /// Fetch retains a proposal; this never supplies human confirmation or installs accepted/home bytes.
    pub fn run_remote_schedule(
        &self,
        id: &str,
        schedule: &ScheduleConsent,
        consent: &RemoteConsent,
        work: HistoryWork<'_>,
    ) -> Result<ScheduledRemoteResult, Error> {
        schedule.remote(consent)?;
        let call = work.call(self, &schedule.owner)?;
        let state = self.schedule_status(id, &schedule.owner)?;
        schedule.check(self, id, &state)?;
        self.check_scheduled_work(&state, Some(&consent.fingerprint()), work)?;
        let pull = self.pull_remote_call(&state.operation_id, consent, &call)?;
        call.check()?;
        let push = if state.allow_push && pull.relation == RemoteRelation::LocalAhead {
            let candidate = pull.accepted.as_ref().ok_or(Error::Recovery)?;
            let approval = consent.owner.approve_push(
                consent,
                &state.operation_id,
                candidate,
                Some(&pull.fetched),
            )?;
            Some(self.push_remote_call(&approval, consent, &call)?)
        } else {
            None
        };
        if push
            .as_ref()
            .is_some_and(|p| !matches!(p, RemotePush::Published(_)))
        {
            return Err(Error::Conflict);
        }
        Ok(ScheduledRemoteResult { pull, push })
    }
    /// Execute the actual quiet/stable observation for a claimed external-text schedule occurrence.
    /// Waiting is NOT queue completion; retain the original claim until its actual worker is settled.
    pub fn run_external_schedule(
        &self,
        id: &str,
        sources: &[ExternalText],
        backend: &knowledge_local::LocalMount<impl knowledge_local::LocalReadPolicy>,
        quiet: Duration,
        schedule: &ScheduleConsent,
        work: HistoryWork<'_>,
    ) -> Result<ExternalBatchState, Error> {
        let call = work.call(self, &schedule.owner)?;
        let state = self.schedule_status(id, &schedule.owner)?;
        schedule.check(self, id, &state)?;
        self.check_scheduled_work(&state, None, work)?;
        self.observe_external_batch_call(&state.operation_id, sources, backend, quiet, work, &call)
    }
    /// Prove actual domain completion while the causing claim still owns its native/result lifetime.
    /// Then durably advance due facts and settle that exact queue claim, never an unknown/dropped worker.
    pub fn finish_schedule(
        &self,
        id: &str,
        schedule: &ScheduleConsent,
        work: HistoryWork<'_>,
    ) -> Result<HistorySchedule, Error> {
        let call = work.call(self, &schedule.owner)?;
        let _lock = self.lock(&call)?;
        let mut state = self.schedule_status(id, &schedule.owner)?;
        schedule.check(self, id, &state)?;
        let snapshot = work
            .claim
            .owner()
            .status(work.claim.job(), work.key)
            .map_err(|_| Error::ContextMismatch)?;
        if state.descriptor.as_deref()
            != Some(&descriptor(
                work.key,
                snapshot.budget,
                snapshot.reservation,
            )?)
        {
            return Err(Error::ContextMismatch);
        }
        if self.runtime.pending()?.is_some() {
            return Err(Error::Busy);
        }
        if state.endpoint.is_some() {
            self.remote_completion(&state.operation_id, state.allow_push, &call)?;
        } else {
            let intent = self.read_intent(&state.operation_id)?;
            if !matches!(
                self.inspect_intent(&intent, &call)?,
                State::Published(_) | State::Unchanged(_)
            ) {
                return Err(Error::Recovery);
            }
        }
        state.attempts = snapshot.attempts;
        state.generation = state.generation.checked_add(1).ok_or(Error::Capacity)?;
        state.operation_id = scheduled_id(id, state.generation);
        state.descriptor = None;
        state.next_run_unix_millis = if state.enabled {
            Some(
                batch::unix_millis()?
                    .checked_add(interval_millis(state.interval_minutes)?)
                    .ok_or(Error::Capacity)?,
            )
        } else {
            None
        };
        replace_schedule(&self.schedule_path(id)?, &state, &call)?;
        work.claim
            .owner()
            .settle(work.claim, work.key, Outcome::Completed)
            .map_err(job_error)?;
        Ok(state)
    }
    /// Record visible finite backoff only for an actually settled offline job, never an unknown worker.
    pub fn backoff_schedule(
        &self,
        id: &str,
        schedule: &ScheduleConsent,
        queue: &Queue,
        job: u64,
        key: Key<'_>,
    ) -> Result<HistorySchedule, Error> {
        schedule.owner.check(self)?;
        let guard = self.control_lock()?;
        let mut state = self.schedule_status(id, &schedule.owner)?;
        schedule.check(self, id, &state)?;
        let snapshot = queue.status(job, key).map_err(|_| Error::ContextMismatch)?;
        if snapshot.state != QueueState::WaitingOffline
            || state.descriptor.as_deref()
                != Some(&descriptor(key, snapshot.budget, snapshot.reservation)?)
        {
            return Err(Error::Conflict);
        }
        state.attempts = snapshot.attempts;
        let delay = interval_millis(state.interval_minutes)?
            .checked_mul(snapshot.attempts.max(1))
            .ok_or(Error::Capacity)?;
        state.next_run_unix_millis = if state.enabled {
            Some(
                batch::unix_millis()?
                    .checked_add(delay)
                    .ok_or(Error::Capacity)?,
            )
        } else {
            None
        };
        replace_schedule_control(&self.schedule_path(id)?, &state)?;
        drop(guard);
        Ok(state)
    }
    /// Common bounded configuration, retaining any existing active occurrence instead of silently dropping it.
    fn configure_schedule(
        &self,
        id: &str,
        endpoint: Option<String>,
        cadence: (u32, bool),
        recipe: &knowledge_core::data::ContentDigest,
        owner: &NativeHistoryOwner,
        call: &Call,
    ) -> Result<(HistorySchedule, ScheduleConsent), Error> {
        let (interval_minutes, allow_push) = cadence;
        interval_millis(interval_minutes)?;
        let _lock = self.lock(call)?;
        let mut state = self.schedule_status(id, owner)?;
        if state.descriptor.is_some()
            && (state.endpoint != endpoint
                || state.interval_minutes != interval_minutes
                || state.allow_push != allow_push
                || state.base_recipe.as_ref() != Some(recipe))
        {
            return Err(Error::Conflict);
        }
        state.enabled = true;
        state.endpoint = endpoint.clone();
        state.interval_minutes = interval_minutes;
        state.allow_push = allow_push;
        state.base_recipe = Some(recipe.clone());
        if state.next_run_unix_millis.is_none() {
            state.next_run_unix_millis = Some(batch::unix_millis()?);
        }
        replace_schedule(&self.schedule_path(id)?, &state, call)?;
        Ok((
            state,
            ScheduleConsent {
                owner: owner.clone(),
                id: id.into(),
                endpoint,
                interval_minutes,
                allow_push,
                recipe: recipe.clone(),
            },
        ))
    }
    /// Admit only one current due occurrence; Core owns queue capacity, aliases, attempts and reservation waiting.
    fn submit_schedule(
        &self,
        id: &str,
        endpoint: Option<&str>,
        schedule: &ScheduleConsent,
        queue: &Queue,
        submission: Submission<'_>,
    ) -> Result<Option<Submitted>, Error> {
        let owner = &schedule.owner;
        owner.check(self)?;
        let guard = self.control_lock()?;
        let mut state = self.schedule_status(id, owner)?;
        schedule.check(self, id, &state)?;
        if !state.enabled || state.endpoint.as_deref() != endpoint {
            return Err(Error::Denied);
        }
        let now = batch::unix_millis()?;
        if state.next_run_unix_millis.is_none_or(|due| due > now) {
            return Ok(None);
        }
        if submission.operation_id != state.operation_id
            || submission.job_id != state.operation_id
            || submission.key.context != owner.selected.context
            || submission.key.library != owner.selected.library
            || submission.key.mount.is_some()
            || submission.key.recipe != &state.work_recipe()?
        {
            return Err(Error::ContextMismatch);
        }
        let required = Self::required_resources(&submission.budget)?;
        if submission.reservation.memory_bytes < required.memory_bytes
            || submission.reservation.temporary_bytes < required.temporary_bytes
        {
            return Err(Error::Capacity);
        }
        let version = descriptor(submission.key, submission.budget, submission.reservation)?;
        if state.descriptor.as_ref().is_some_and(|old| old != &version) {
            return Err(Error::OperationMismatch);
        }
        state.descriptor = Some(version);
        replace_schedule_control(&self.schedule_path(id)?, &state)?;
        let submitted = queue.submit(submission).map_err(job_error)?;
        let snapshot = queue
            .status(submitted.job, submission.key)
            .map_err(job_error)?;
        if snapshot.state == QueueState::WaitingOffline {
            queue
                .reconnect(submitted.job, submission.key)
                .map_err(job_error)?;
        }
        drop(guard);
        Ok(Some(submitted))
    }
    /// Match the complete descriptor of the actual claimed job before invoking domain work.
    fn check_scheduled_work(
        &self,
        state: &HistorySchedule,
        endpoint: Option<&str>,
        work: HistoryWork<'_>,
    ) -> Result<(), Error> {
        let snapshot = work
            .claim
            .owner()
            .status(work.claim.job(), work.key)
            .map_err(job_error)?;
        if !state.enabled
            || state.endpoint.as_deref() != endpoint
            || state.descriptor.as_deref()
                != Some(&descriptor(
                    work.key,
                    snapshot.budget,
                    snapshot.reservation,
                )?)
        {
            return Err(Error::ContextMismatch);
        }
        Ok(())
    }
    /// Fixed local settings file; imported manifest data is never read as scheduling authority.
    fn schedule_path(&self, id: &str) -> Result<PathBuf, Error> {
        validate_id(id)?;
        Ok(self
            .repo
            .join("omni-state")
            .join(format!("schedule-{id}.json")))
    }
    /// Short control-plane writer lock only; it starts no native process and cannot settle a worker.
    fn control_lock(&self) -> Result<OwnedWriterLock, Error> {
        self.validate()?;
        let path = self.repo.join("omni-state/writer.lock");
        if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
            return Err(Error::Invalid);
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        FileExt::try_lock_exclusive(&file).map_err(|_| Error::Busy)?;
        Ok(OwnedWriterLock::new(file))
    }
}
/// Complete descriptor fingerprint, preserving all Core qualifiers instead of a digest-only pseudo-key.
fn descriptor(
    key: Key<'_>,
    budget: knowledge_core::backend::Budget,
    reservation: knowledge_core::jobs::Reservation,
) -> Result<String, Error> {
    Ok(digest(
        &serde_json::to_vec(&(
            key.context,
            key.library,
            key.mount,
            key.source,
            key.version.as_str(),
            key.recipe.as_str(),
            key.processor.as_str(),
            budget,
            reservation.memory_bytes,
            reservation.temporary_bytes,
        ))
        .map_err(|_| Error::Invalid)?,
    ))
}
/// Deterministic occurrence identity survives queue restart without duplicate Git operations.
fn scheduled_id(id: &str, generation: u64) -> String {
    format!(
        "schedule-{}",
        &digest(format!("{id}:{generation}").as_bytes())[..40]
    )
}
/// Default disabled schedule, with no remote contact or inferred push rights.
fn new_schedule(id: &str) -> HistorySchedule {
    HistorySchedule {
        enabled: false,
        interval_minutes: 0,
        allow_push: false,
        next_run_unix_millis: None,
        operation_id: scheduled_id(id, 0),
        attempts: 0,
        endpoint: None,
        descriptor: None,
        generation: 0,
        base_recipe: None,
    }
}
/// Validate finite positive user interval; no data timestamp is an effect grant.
fn interval_millis(minutes: u32) -> Result<u64, Error> {
    if minutes == 0 {
        return Err(Error::Invalid);
    }
    u64::from(minutes).checked_mul(60_000).ok_or(Error::Invalid)
}
/// Recheck actual work before a controlled settings write.
fn replace_schedule(path: &Path, value: &HistorySchedule, call: &Call) -> Result<(), Error> {
    call.check()?;
    replace_schedule_control(path, value)
}
/// Atomically replace only our bounded local settings, never a home file or imported config.
fn replace_schedule_control(path: &Path, value: &HistorySchedule) -> Result<(), Error> {
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err(Error::Invalid);
    }
    let mut file = tempfile::NamedTempFile::new_in(path.parent().ok_or(Error::Invalid)?)?;
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Recovery)?;
    if bytes.len() > CONTROL_LIMIT {
        return Err(Error::Capacity);
    }
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|_| Error::Io)?;
    File::open(path.parent().ok_or(Error::Invalid)?)?.sync_all()?;
    Ok(())
}
/// Fixed Core admission classifications, never discard unknown native ownership.
fn job_error(error: JobError) -> Error {
    match error {
        JobError::Capacity => Error::Capacity,
        JobError::Conflict => Error::OperationMismatch,
        _ => Error::ContextMismatch,
    }
}
