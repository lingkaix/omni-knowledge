//! One volatile cooperative queue and logical reservation account, not an executor or authority.
//!
//! Callers select all capacities and supply current admitted context, source and processor facts.
//! Reservations are promises between cooperating callers, not measured RAM, disk or process limits.
//! A claim remains explicitly unsettled until a trusted executor reports actual settlement.
use crate::backend::Budget;
use crate::data::ContentDigest;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Explicit caller settings; zero closes the corresponding capacity, never means unlimited.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum queued jobs; offline and unsettled records still occupy retained-record capacity.
    pub max_pending: usize,
    /// Maximum retained submission receipts, including aliases; canonical jobs cannot exceed this.
    pub max_records: usize,
    /// Maximum queue-owned UTF-8 string bytes, including repeated IDs and digest text.
    /// Fixed record/vector overhead is bounded separately by max_records, not charged as text.
    pub max_descriptor_bytes: usize,
    /// Maximum unsettled jobs per (context, library, mount); None mount denotes home.
    pub per_mount: usize,
    /// Total simultaneous logical working-memory reservations, not an installed RAM ceiling.
    pub memory_bytes: u64,
    /// Total simultaneous logical temporary-output reservations, not retained cache/disk usage.
    pub temporary_bytes: u64,
}

/// Separate explicit resource promises; these do not redefine any Budget work maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reservation {
    /// Caller-admitted working-memory cost, including whatever concurrent costs it owns.
    pub memory_bytes: u64,
    /// Caller-admitted temporary-output cost; complete retained objects need their own accounting.
    pub temporary_bytes: u64,
}

/// Borrowed work identity; no field admits access, proves verification, or implies recency.
#[derive(Clone, Copy)]
pub struct Key<'a> {
    /// Stable local root/security namespace qualifier supplied by the trusted current owner.
    pub context: &'a str,
    /// Stable library identity inside that context, not a discovered root or permission.
    pub library: &'a str,
    /// Stable mount identity, or home; the caller must use consistent qualifiers.
    pub mount: Option<&'a str>,
    /// Stable source identity; equal bytes do not merge different sources.
    pub source: &'a str,
    /// Claimed verified input-byte version, obtained through current qualified source admission.
    pub version: &'a ContentDigest,
    /// Immutable recipe identity, not an executable instruction or global recipe catalog.
    pub recipe: &'a ContentDigest,
    /// Complete effective processor/dependency fingerprint, e.g. cache ProcessorIdentity::fingerprint.
    /// The owner must include effective options, models, packages, transcripts and annotations.
    pub processor: &'a ContentDigest,
}

/// Priority among currently eligible jobs; eligible ties keep original submission order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Ordinary admitted work.
    Normal,
    /// Explicitly pinned admitted work.
    Pinned,
}

/// One borrowed submission; strings are accounted before the queue copies them.
#[derive(Clone, Copy)]
pub struct Submission<'a> {
    /// Stable host operation identity; reuse with another intent is a conflict while retained.
    pub operation_id: &'a str,
    /// Stable host job identity; duplicate work can retain this as an alias of one actual job.
    pub job_id: &'a str,
    /// Scope/version/recipe/dependency binding; the queue does not independently verify these facts.
    pub key: Key<'a>,
    /// Existing Core work ceilings, passed unchanged to the executor, not silently implemented here.
    pub budget: Budget,
    /// Explicit simultaneous resource promises, separate from work/input-byte maxima.
    pub reservation: Reservation,
    /// Explicit scheduling priority, part of the retained intent.
    pub priority: Priority,
    /// Finite positive attempt bound, including explicit reconnect attempts; no automatic retry.
    pub max_attempts: u64,
}

/// A retained submission maps to this instance-local job number, never a durable/global ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Submitted {
    /// Canonical job number, also used by claim/status/cancel; stable host IDs remain retained.
    pub job: u64,
    /// True only when this call created work, not an operation/key alias.
    pub created: bool,
}

/// A cooperative request, never proof that a worker stopped or rolled back publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cancellation {
    /// Explicit host cancellation.
    Requested,
    /// Explicit context-matched obsolescence; no digest ordering was used.
    Superseded,
    /// The host polled an elapsed cooperative deadline.
    Deadline,
}

/// Actual executor-reported settlement; none proves filesystem retirement or domain truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Executor reports fully completed work, even if a cancellation request raced completion.
    Completed,
    /// Executor reports settled partial work with truthful coverage owned elsewhere.
    Partial,
    /// Executor confirms stopped cancellation, not merely a requested stop.
    Cancelled,
    /// Cancellation settled an explicitly superseded job.
    Superseded,
    /// Executor stopped offline; only a noncancelled settlement with attempts remaining waits.
    /// The final allowed attempt settles as Finished(Offline), not WaitingOffline.
    Offline,
    /// Invalid model/configuration; terminal until explicit host intervention/new submission.
    InvalidModel,
    /// Other settled failure; there is no automatic retry or raw diagnostic payload.
    Failed,
}

/// Current accounting state, not a copy of the service Job DTO or an execution observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Awaiting claim without reservations.
    Queued,
    /// Claimed and not settled, including an abandoned token or unknown worker liveness.
    Unsettled {
        /// First request is retained; no request frees reservations.
        cancellation: Option<Cancellation>,
    },
    /// Actual noncancelled Offline settlement released execution reservations with attempts remaining.
    /// Reconnect is explicit; final-attempt Offline settles as Finished(Offline), not this state.
    WaitingOffline,
    /// Terminal record retained for idempotency until explicit retirement.
    Finished(Outcome),
}

/// Fixed-size inspection; caller-owned copies do not grow queue history or release resources.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    /// Exact current queue/accounting state.
    pub state: State,
    /// The unchanged admitted work maxima, not measured usage.
    pub budget: Budget,
    /// Promised resources held only while unsettled.
    pub reservation: Reservation,
    /// Attempts actually claimed, not queued submissions or polling calls.
    pub attempts: u64,
    /// Current cooperative deadline, present only while unsettled.
    pub deadline: Option<Instant>,
}

/// Distinct actual queue counts and promised resources; never physical RAM/volume usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usage {
    /// Canonical retained jobs, including terminal records.
    pub jobs: usize,
    /// Retained operation/job alias receipts; no silent idempotency eviction.
    pub records: usize,
    /// Jobs presently queued, not waiting offline or already claimed.
    pub pending: usize,
    /// Claimed jobs still awaiting trusted actual settlement, including dropped claims.
    pub unsettled: usize,
    /// Stopped offline jobs awaiting explicit reconnect.
    pub waiting_offline: usize,
    /// Queue-owned UTF-8 lengths; allocator overhead and caller buffers are not physical quotas.
    pub descriptor_bytes: usize,
    /// Exactly the sum of logical promises held by unsettled jobs.
    pub reserved: Reservation,
}

/// Fixed input-free errors; no rejected paths, identities, model strings or secrets are reflected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobError {
    /// Empty required identity, zero attempt bound, or inconsistent capacity settings.
    Invalid,
    /// A finite record/pending/descriptor/reservation capacity cannot admit the request.
    Capacity,
    /// A retained operation/job/key was reused with different intent.
    Conflict,
    /// Required retained job is absent; explicit retirement ends that idempotency window.
    Unknown,
    /// Expected context/key does not match the retained record.
    Context,
    /// Claim belongs to another volatile owner instance.
    WrongInstance,
    /// Claim is missing, already settled, retired or has a different claim generation.
    StaleClaim,
    /// The requested transition is not allowed from the current state.
    State,
    /// A lifetime sequence or attempt counter cannot advance without overflow.
    Exhausted,
    /// Instant::checked_add actually failed on this host; no arbitrary max_millis threshold applies.
    /// Even u64::MAX milliseconds may be representable, so a large integer alone does not imply Clock.
    Clock,
    /// Owned state was poisoned; fail closed without resetting reservations.
    Poisoned,
}
impl fmt::Display for JobError {
    /// Write only a fixed classification, never submitted data or underlying lock details.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Invalid => "invalid job data or capacities",
            Self::Capacity => "job capacity unavailable",
            Self::Conflict => "retained job intent conflict",
            Self::Unknown => "job not retained",
            Self::Context => "job context mismatch",
            Self::WrongInstance => "wrong queue instance",
            Self::StaleClaim => "stale job claim",
            Self::State => "job transition refused",
            Self::Exhausted => "job lifetime counter exhausted",
            Self::Clock => "job deadline unrepresentable",
            Self::Poisoned => "job accounting unavailable",
        })
    }
}
impl std::error::Error for JobError {}

/// One shared cooperating owner; cloning shares exactly the same bounded mutex-protected state.
#[derive(Clone)]
pub struct Queue(pub(crate) Arc<Mutex<Inner>>, pub(crate) Option<u32>);

/// Non-forgeable instance/job/generation binding, not an authorization or worker-stop token.
/// Drop performs NO settlement: the record remains Unsettled and all promises remain held.
/// The claim retains its owner, even after the original Queue handle drops.
/// Losing all claims for unsettled work deliberately strands capacity in this slice.
pub struct Claim {
    /// Same volatile accounting owner, retained throughout claim lifetime.
    owner: Queue,
    /// Canonical lifetime job number.
    job: u64,
    /// Unique lifetime claim generation, never reused after retirement/reconnect.
    generation: u64,
    /// Unchanged admitted work ceilings, without owned variable-sized data.
    budget: Budget,
    /// Monotonic cooperative deadline, not an installed timer or process limit.
    deadline: Instant,
}
impl Claim {
    /// Return the canonical instance-local job number for host dispatch mapping.
    pub fn job(&self) -> u64 {
        self.job
    }
    /// Return the exact admitted Core work maxima for actual executor enforcement.
    pub fn budget(&self) -> Budget {
        self.budget
    }
    /// Return the cooperative deadline; passing it does not prove worker termination.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Borrow this same owner, permitting actual settlement after the original handle drops.
    /// This is ordinary trusted in-process access, not a portable authorization grant.
    pub fn owner(&self) -> &Queue {
        &self.owner
    }
}

/// Queue-owned identity; each copied UTF-8 length is included in descriptor accounting.
struct OwnedKey {
    /// Stable local scope qualifier.
    context: String,
    /// Library data ID.
    library: String,
    /// Home/mount data qualifier.
    mount: Option<String>,
    /// Source data ID, separate from content identity.
    source: String,
    /// Claimed verified source byte version.
    version: ContentDigest,
    /// Effective immutable recipe.
    recipe: ContentDigest,
    /// Complete effective processor/dependency identity.
    processor: ContentDigest,
}
impl OwnedKey {
    /// Copy only after admission has charged every resulting string length.
    fn copy(key: Key<'_>) -> Self {
        Self {
            context: key.context.to_owned(),
            library: key.library.to_owned(),
            mount: key.mount.map(str::to_owned),
            source: key.source.to_owned(),
            version: key.version.clone(),
            recipe: key.recipe.clone(),
            processor: key.processor.clone(),
        }
    }
    /// Match every retained fact without inferring recency or permission.
    fn matches(&self, key: Key<'_>) -> bool {
        self.same_source(key)
            && self.version == *key.version
            && self.recipe == *key.recipe
            && self.processor == *key.processor
    }
    /// Check explicit supersession remains inside the same subject and local namespace.
    fn same_source(&self, key: Key<'_>) -> bool {
        self.context == key.context
            && self.library == key.library
            && self.mount.as_deref() == key.mount
            && self.source == key.source
    }
    /// Group per-mount concurrency by trusted scope/library/mount, including home.
    fn same_mount(&self, other: &Self) -> bool {
        self.context == other.context && self.library == other.library && self.mount == other.mount
    }
}

/// One canonical job; retries reuse it rather than growing a stale-entry heap/history.
struct Job {
    /// Monotonic instance-local record identity.
    number: u64,
    /// Canonical stable host job ID, retained even if later submissions name aliases.
    id: String,
    /// Complete key facts supplied by the host.
    key: OwnedKey,
    /// Work maxima, never used as measured usage.
    budget: Budget,
    /// Explicit promises to take atomically at claim.
    reservation: Reservation,
    /// Execution-owned portion after explicit same-account lifetime splits.
    held: Reservation,
    /// Explicit scheduling preference.
    priority: Priority,
    /// Finite caller attempt ceiling.
    max_attempts: u64,
    /// Actually claimed attempts.
    attempts: u64,
    /// Owned lifecycle state.
    state: State,
    /// Live claim generation, absent unless unsettled.
    generation: Option<u64>,
    /// Live cooperative deadline, absent unless unsettled.
    deadline: Option<Instant>,
    /// Accounted key/canonical-ID UTF-8 bytes, for explicit retirement.
    bytes: usize,
}
impl Job {
    /// Compare all work intent; duplicates do not silently promote priority or replace budgets.
    fn intent(&self, s: Submission<'_>) -> bool {
        self.key.matches(s.key)
            && self.reservation == s.reservation
            && self.priority == s.priority
            && self.max_attempts == s.max_attempts
            && self.budget.max_files == s.budget.max_files
            && self.budget.max_bytes == s.budget.max_bytes
            && self.budget.max_millis == s.budget.max_millis
            && self.budget.max_memory_bytes == s.budget.max_memory_bytes
    }
    /// Return fixed inspection without cloning any variable-sized input.
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: self.state,
            budget: self.budget,
            reservation: self.reservation,
            attempts: self.attempts,
            deadline: self.deadline,
        }
    }
}

/// Retained operation and proposed job alias; bounded independently of canonical work deduplication.
struct Receipt {
    /// Stable host operation ID; unique while retained.
    operation: String,
    /// Proposed host job ID; aliases cannot be reused for other work while retained.
    job_id: String,
    /// Canonical job number; canonical record cannot disappear while receipts remain.
    job: u64,
    /// Accounted UTF-8 bytes for both owned strings.
    bytes: usize,
}

/// All owned scheduling/accounting state; no separate controller, heap tombstones or callbacks.
pub(crate) struct Inner {
    /// Explicit immutable caller capacities.
    pub(crate) limits: Limits,
    /// Canonical retained jobs, bounded by receipt capacity.
    jobs: Vec<Job>,
    /// Every retained submission identity, including duplicate-key aliases.
    receipts: Vec<Receipt>,
    /// Combined charged variable-size descriptor bytes.
    bytes: usize,
    /// Exact logical promises held until settlement.
    pub(crate) reserved: Reservation,
    /// Shared lifetime sequence for record and claim identities; retirement never resets it.
    pub(crate) sequence: u64,
    /// Optional process-native facts and holdings in this SAME account; standalone fixtures omit it.
    pub(crate) extra: Option<crate::admission::Extra>,
}

impl Queue {
    /// Create empty state without capacity-sized allocation or defaults.
    /// Returns Invalid if pending capacity exceeds retained-record capacity; zero capacities close admission.
    pub fn new(limits: Limits) -> Result<Self, JobError> {
        if limits.max_pending > limits.max_records {
            return Err(JobError::Invalid);
        }
        Ok(Self(
            Arc::new(Mutex::new(Inner {
                limits,
                jobs: Vec::new(),
                receipts: Vec::new(),
                bytes: 0,
                reserved: Reservation {
                    memory_bytes: 0,
                    temporary_bytes: 0,
                },
                sequence: 0,
                extra: None,
            })),
            None,
        ))
    }
    /// Acquire the sole accounting lock; poison never resets or releases reservations.
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Inner>, JobError> {
        if self.1.is_some_and(|pid| pid != std::process::id()) {
            return Err(JobError::WrongInstance);
        }
        self.0.lock().map_err(|_| JobError::Poisoned)
    }

    /// Check a capability's same-account origin before private lifetime transfers.
    pub(crate) fn owns(&self, claim: &Claim) -> bool {
        Arc::ptr_eq(&self.0, &claim.owner.0)
    }

    /// Admit or deduplicate work and retain each newly accepted operation/job alias.
    /// Returns Conflict for different intent under any retained operation/job/key, Capacity before copying
    /// unadmitted strings, Invalid for empty identities/zero attempts, or Exhausted without counter wrap.
    /// Identical retained operation delivery needs no new capacity; key aliases require receipt/text capacity.
    pub fn submit(&self, s: Submission<'_>) -> Result<Submitted, JobError> {
        self.lock()?.submit(s)
    }

    /// Return bounded owned counts, including abandoned-but-unsettled promises; poison fails closed.
    pub fn usage(&self) -> Result<Usage, JobError> {
        Ok(self.lock()?.usage())
    }

    /// Inspect one context-matched retained job without cloning strings.
    /// Returns Unknown/Context for missing or different facts, or Poisoned.
    pub fn status(&self, job: u64, expected: Key<'_>) -> Result<Snapshot, JobError> {
        let inner = self.lock()?;
        Ok(inner.get(job, expected)?.snapshot())
    }

    /// Claim the highest-priority eligible job, FIFO by original admission among eligible ties.
    /// Blocked jobs stay queued; polling returns None without spinning or acquiring partial resources.
    /// All global promises and one mount slot are taken atomically, never from three controllers.
    /// The caller supplies monotonic now; Clock/Exhausted leave all claims/counters unchanged.
    /// Clock requires actual checked_add failure, not a guessed host duration range.
    pub fn claim(&self, now: Instant) -> Result<Option<Claim>, JobError> {
        let mut i = self.lock()?;
        if let Some(extra) = &mut i.extra {
            extra.refresh().map_err(|_| JobError::Capacity)?;
        }
        let candidate = i
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| {
                j.state == State::Queued
                    && j.attempts < j.max_attempts
                    && i.fits(j.reservation)
                    && i.mount_count(&j.key) < i.limits.per_mount
            })
            .max_by_key(|(_, j)| (j.priority, std::cmp::Reverse(j.number)))
            .map(|(n, _)| n);
        let Some(n) = candidate else {
            return Ok(None);
        };
        let deadline = now
            .checked_add(Duration::from_millis(i.jobs[n].budget.max_millis))
            .ok_or(JobError::Clock)?;
        let generation = i.sequence.checked_add(1).ok_or(JobError::Exhausted)?;
        let attempts = i.jobs[n]
            .attempts
            .checked_add(1)
            .ok_or(JobError::Exhausted)?;
        let reservation = i.jobs[n].reservation;
        // fits established both ceilings; keep checked additions before any state mutation.
        let memory_bytes = i
            .reserved
            .memory_bytes
            .checked_add(reservation.memory_bytes)
            .ok_or(JobError::Exhausted)?;
        let temporary_bytes = i
            .reserved
            .temporary_bytes
            .checked_add(reservation.temporary_bytes)
            .ok_or(JobError::Exhausted)?;
        i.reserved = Reservation {
            memory_bytes,
            temporary_bytes,
        };
        i.sequence = generation;
        let job = &mut i.jobs[n];
        job.attempts = attempts;
        job.state = State::Unsettled { cancellation: None };
        job.held = reservation;
        job.generation = Some(generation);
        job.deadline = Some(deadline);
        Ok(Some(Claim {
            owner: self.clone(),
            job: job.number,
            generation,
            budget: job.budget,
            deadline,
        }))
    }

    /// Cancel queued/offline work immediately; unsettled work only records its first stop request.
    /// Terminal calls are idempotent; Unknown/Context/Poisoned never change accounting.
    pub fn cancel(&self, job: u64, expected: Key<'_>) -> Result<State, JobError> {
        self.lock()?.cancel(job, expected, Cancellation::Requested)
    }

    /// Explicitly obsolete work inside the same context/library/mount/source and a different full key.
    /// The trusted host must validate replacement facts; digests are never ordered as versions.
    /// This does not submit replacement work or retire bytes/pins; invalid context/identity returns Context.
    pub fn supersede(
        &self,
        job: u64,
        expected: Key<'_>,
        replacement: Key<'_>,
    ) -> Result<State, JobError> {
        let mut i = self.lock()?;
        let old = i.get(job, expected)?;
        if !valid_key(replacement)
            || !old.key.same_source(replacement)
            || old.key.matches(replacement)
        {
            return Err(JobError::Context);
        }
        i.cancel(job, expected, Cancellation::Superseded)
    }

    /// Record newly elapsed active deadlines without releasing anything or claiming worker stop.
    /// Returns the bounded number of first requests; no thread, timer, kill or automatic retry is installed.
    pub fn request_expired(&self, now: Instant) -> Result<usize, JobError> {
        let mut i = self.lock()?;
        let mut count = 0;
        for j in &mut i.jobs {
            if j.state == (State::Unsettled { cancellation: None })
                && j.deadline.is_some_and(|d| now >= d)
            {
                j.state = State::Unsettled {
                    cancellation: Some(Cancellation::Deadline),
                };
                count += 1;
            }
        }
        Ok(count)
    }

    /// Report actual worker settlement and release exactly its complete reservation and mount slot.
    /// The trusted executor must first establish stopped children/settled output/owned I/O; this API cannot.
    /// Cancellation/deadlines alone are not settlement; Completed may legitimately win a cancellation race.
    /// Returns WrongInstance, StaleClaim, Context, State or Poisoned without any release on refusal.
    pub fn settle(
        &self,
        claim: &Claim,
        expected: Key<'_>,
        mut outcome: Outcome,
    ) -> Result<State, JobError> {
        if !Arc::ptr_eq(&self.0, &claim.owner.0) {
            return Err(JobError::WrongInstance);
        }
        let mut i = self.lock()?;
        let n = i
            .jobs
            .iter()
            .position(|j| j.number == claim.job)
            .ok_or(JobError::StaleClaim)?;
        let j = &i.jobs[n];
        if j.generation != Some(claim.generation) {
            return Err(JobError::StaleClaim);
        }
        if !j.key.matches(expected) {
            return Err(JobError::Context);
        }
        let State::Unsettled { cancellation } = j.state else {
            return Err(JobError::StaleClaim);
        };
        if outcome == Outcome::Superseded && cancellation != Some(Cancellation::Superseded) {
            return Err(JobError::State);
        }
        if outcome == Outcome::Cancelled && cancellation == Some(Cancellation::Superseded) {
            outcome = Outcome::Superseded;
        }
        let state =
            if outcome == Outcome::Offline && cancellation.is_none() && j.attempts < j.max_attempts
            {
                State::WaitingOffline
            } else {
                State::Finished(outcome)
            };
        let reservation = j.held;
        let memory_bytes = i
            .reserved
            .memory_bytes
            .checked_sub(reservation.memory_bytes)
            .ok_or(JobError::State)?;
        let temporary_bytes = i
            .reserved
            .temporary_bytes
            .checked_sub(reservation.temporary_bytes)
            .ok_or(JobError::State)?;
        i.reserved = Reservation {
            memory_bytes,
            temporary_bytes,
        };
        let j = &mut i.jobs[n];
        j.state = state;
        j.held = Reservation {
            memory_bytes: 0,
            temporary_bytes: 0,
        };
        j.generation = None;
        j.deadline = None;
        Ok(state)
    }

    /// Explicit host reconnect queues a stopped offline job, within pending/attempt bounds.
    /// Repeated delivery while already queued is a no-op; terminal/active work returns State.
    /// Invalid models and other terminal failures cannot be retried through this method.
    pub fn reconnect(&self, job: u64, expected: Key<'_>) -> Result<(), JobError> {
        let mut i = self.lock()?;
        let j = i.get(job, expected)?;
        if j.state == State::Queued {
            return Ok(());
        }
        if j.state != State::WaitingOffline || j.attempts >= j.max_attempts {
            return Err(JobError::State);
        }
        if i.usage().pending >= i.limits.max_pending {
            return Err(JobError::Capacity);
        }
        let j = i
            .jobs
            .iter_mut()
            .find(|j| j.number == job)
            .ok_or(JobError::Unknown)?;
        j.state = State::Queued;
        Ok(())
    }

    /// Explicitly forget one terminal job AND all its retained operation/job aliases.
    /// This ends their volatile idempotency window; never retires filesystem bytes, journals or pins.
    /// Active, queued and offline-waiting work returns State; lifetime counters never reset.
    pub fn retire(&self, job: u64, expected: Key<'_>) -> Result<(), JobError> {
        let mut i = self.lock()?;
        let j = i.get(job, expected)?;
        if !matches!(j.state, State::Finished(_)) {
            return Err(JobError::State);
        }
        let bytes = i
            .receipts
            .iter()
            .filter(|r| r.job == job)
            .try_fold(j.bytes, |n, r| n.checked_add(r.bytes))
            .ok_or(JobError::State)?;
        i.bytes = i.bytes.checked_sub(bytes).ok_or(JobError::State)?;
        i.receipts.retain(|r| r.job != job);
        let n = i
            .jobs
            .iter()
            .position(|j| j.number == job)
            .ok_or(JobError::Unknown)?;
        i.jobs.remove(n);
        Ok(())
    }
}

impl Inner {
    /// Compute live state counts without a growing history or stale priority entries.
    pub(crate) fn usage(&self) -> Usage {
        Usage {
            jobs: self.jobs.len(),
            records: self.receipts.len(),
            pending: self
                .jobs
                .iter()
                .filter(|j| j.state == State::Queued)
                .count(),
            unsettled: self
                .jobs
                .iter()
                .filter(|j| matches!(j.state, State::Unsettled { .. }))
                .count(),
            waiting_offline: self
                .jobs
                .iter()
                .filter(|j| j.state == State::WaitingOffline)
                .count(),
            descriptor_bytes: self.bytes,
            reserved: self.jobs.iter().fold(
                Reservation {
                    memory_bytes: 0,
                    temporary_bytes: 0,
                },
                |sum, j| Reservation {
                    memory_bytes: sum.memory_bytes + j.held.memory_bytes,
                    temporary_bytes: sum.temporary_bytes + j.held.temporary_bytes,
                },
            ),
        }
    }
    /// Inspect under the same admission mutex so state and resource waits cannot race each other.
    pub(crate) fn inspect(&self, job: u64, expected: Key<'_>) -> Result<Snapshot, JobError> {
        Ok(self.get(job, expected)?.snapshot())
    }
    /// Resolve a retained record and validate every expected key fact before mutations.
    fn get(&self, job: u64, expected: Key<'_>) -> Result<&Job, JobError> {
        let j = self
            .jobs
            .iter()
            .find(|j| j.number == job)
            .ok_or(JobError::Unknown)?;
        if !j.key.matches(expected) {
            return Err(JobError::Context);
        }
        Ok(j)
    }
    /// Count unsettled work for one trusted mount qualifier, including abandoned claims.
    fn mount_count(&self, key: &OwnedKey) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.state, State::Unsettled { .. }) && j.key.same_mount(key))
            .count()
    }
    /// Check both complete reservation additions and capacities before taking either.
    fn fits(&self, r: Reservation) -> bool {
        self.reserved
            .memory_bytes
            .checked_add(r.memory_bytes)
            .is_some_and(|v| v <= self.limits.memory_bytes)
            && self
                .reserved
                .temporary_bytes
                .checked_add(r.temporary_bytes)
                .is_some_and(|v| {
                    v <= self.limits.temporary_bytes
                        && self.extra.as_ref().is_none_or(|x| v <= x.ceiling())
                })
    }
    /// Validate private claim/key/generation facts without exposing another public token constructor.
    pub(crate) fn claim_resources(
        &mut self,
        claim: &Claim,
        expected: Key<'_>,
    ) -> Result<&mut Reservation, JobError> {
        self.get(claim.job, expected)?;
        let j = self
            .jobs
            .iter_mut()
            .find(|j| j.number == claim.job)
            .ok_or(JobError::StaleClaim)?;
        if j.generation != Some(claim.generation) || !matches!(j.state, State::Unsettled { .. }) {
            return Err(JobError::StaleClaim);
        }
        Ok(&mut j.held)
    }
    /// Derive currently execution-owned resources without maintaining a parallel counter.
    pub(crate) fn job_resources(&self, job: u64) -> Reservation {
        self.jobs.iter().find(|j| j.number == job).map_or(
            Reservation {
                memory_bytes: 0,
                temporary_bytes: 0,
            },
            |j| j.held,
        )
    }
    /// Apply cancellation without pretending a possibly live worker settled.
    fn cancel(
        &mut self,
        job: u64,
        expected: Key<'_>,
        reason: Cancellation,
    ) -> Result<State, JobError> {
        self.get(job, expected)?;
        let j = self
            .jobs
            .iter_mut()
            .find(|j| j.number == job)
            .ok_or(JobError::Unknown)?;
        j.state = match j.state {
            State::Queued | State::WaitingOffline => {
                State::Finished(if reason == Cancellation::Superseded {
                    Outcome::Superseded
                } else {
                    Outcome::Cancelled
                })
            }
            State::Unsettled { cancellation } => State::Unsettled {
                cancellation: cancellation.or(Some(reason)),
            },
            terminal => terminal,
        };
        Ok(j.state)
    }
    /// Admit with bounded aliases and charge all text before copying; no partial logical admission.
    fn submit(&mut self, s: Submission<'_>) -> Result<Submitted, JobError> {
        if s.operation_id.is_empty()
            || s.job_id.is_empty()
            || !valid_key(s.key)
            || s.max_attempts == 0
        {
            return Err(JobError::Invalid);
        }
        if let Some(r) = self.receipts.iter().find(|r| r.operation == s.operation_id) {
            let j = self
                .jobs
                .iter()
                .find(|j| j.number == r.job)
                .ok_or(JobError::Unknown)?;
            if r.job_id != s.job_id || !j.intent(s) {
                return Err(JobError::Conflict);
            }
            return Ok(Submitted {
                job: j.number,
                created: false,
            });
        }
        let existing = self.jobs.iter().position(|j| j.key.matches(s.key));
        if let Some(n) = existing
            && !self.jobs[n].intent(s)
        {
            return Err(JobError::Conflict);
        }
        // Canonical IDs and all accepted aliases remain bound until explicit group retirement.
        if self
            .jobs
            .iter()
            .any(|j| j.id == s.job_id && Some(j.number) != existing.map(|n| self.jobs[n].number))
            || self.receipts.iter().any(|r| {
                r.job_id == s.job_id && Some(r.job) != existing.map(|n| self.jobs[n].number)
            })
        {
            return Err(JobError::Conflict);
        }
        if self
            .receipts
            .len()
            .checked_add(self.extra.as_ref().map_or(0, |x| x.held.len()))
            .is_none_or(|n| n >= self.limits.max_records)
        {
            return Err(JobError::Capacity);
        }
        let receipt_bytes = s
            .operation_id
            .len()
            .checked_add(s.job_id.len())
            .ok_or(JobError::Capacity)?;
        let job_bytes = if existing.is_none() {
            if self.usage().pending >= self.limits.max_pending
                || s.reservation.memory_bytes > self.limits.memory_bytes
                || s.reservation.temporary_bytes > self.limits.temporary_bytes
            {
                return Err(JobError::Capacity);
            }
            [
                s.key.context.len(),
                s.key.library.len(),
                s.key.mount.map_or(0, str::len),
                s.key.source.len(),
                s.key.version.as_str().len(),
                s.key.recipe.as_str().len(),
                s.key.processor.as_str().len(),
                s.job_id.len(),
            ]
            .into_iter()
            .try_fold(0usize, |n, bytes| n.checked_add(bytes))
            .ok_or(JobError::Capacity)?
        } else {
            0
        };
        let total = self
            .bytes
            .checked_add(receipt_bytes)
            .and_then(|n| n.checked_add(job_bytes))
            .filter(|n| *n <= self.limits.max_descriptor_bytes)
            .ok_or(JobError::Capacity)?;
        let number = if let Some(n) = existing {
            self.jobs[n].number
        } else {
            self.sequence.checked_add(1).ok_or(JobError::Exhausted)?
        };
        self.receipts
            .try_reserve_exact(1)
            .map_err(|_| JobError::Capacity)?;
        if existing.is_none() {
            self.jobs
                .try_reserve_exact(1)
                .map_err(|_| JobError::Capacity)?;
            self.jobs.push(Job {
                number,
                id: s.job_id.to_owned(),
                key: OwnedKey::copy(s.key),
                budget: s.budget,
                reservation: s.reservation,
                held: Reservation {
                    memory_bytes: 0,
                    temporary_bytes: 0,
                },
                priority: s.priority,
                max_attempts: s.max_attempts,
                attempts: 0,
                state: State::Queued,
                generation: None,
                deadline: None,
                bytes: job_bytes,
            });
            self.sequence = number;
        }
        self.receipts.push(Receipt {
            operation: s.operation_id.to_owned(),
            job_id: s.job_id.to_owned(),
            job: number,
            bytes: receipt_bytes,
        });
        self.bytes = total;
        Ok(Submitted {
            job: number,
            created: existing.is_none(),
        })
    }
}

/// Validate only required nonempty data qualifiers, without imposing UUID/path/source-format policy.
fn valid_key(k: Key<'_>) -> bool {
    !k.context.is_empty()
        && !k.library.is_empty()
        && !k.source.is_empty()
        && k.mount.is_none_or(|m| !m.is_empty())
}
