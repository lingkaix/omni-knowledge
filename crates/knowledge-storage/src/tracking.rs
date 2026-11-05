//! Streaming source reconciliation and visible-journal replay; private projections are disposable.
use crate::{
    bootstrap::{Bounds, Preflight},
    *,
};
use knowledge_core::{
    admission::{Decision, Origin},
    backend::*,
};
use knowledge_local::{LocalMount, LocalReadPolicy};
#[path = "inventory.rs"]
mod inventory;
use inventory::Inventory;
/// Observed identity disposition, never a content-hash association or permission grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// A new native qualified object, with a new independent source ID.
    Created,
    /// Current native identity remains the same.
    Current,
    /// Actual current bytes changed while qualified identity remains the same.
    Changed,
    /// Reuse-protected identity moved and its old path was independently observed absent.
    Renamed,
    /// Actual relevant absence was confirmed after complete enumeration.
    Deleted,
    /// No sufficient reuse/continuity evidence exists; annotations are not transplanted.
    IdentityUncertain,
}
/// Independent body-work result; metadata discovery is never collapsed into extraction/verification success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verification {
    /// Eager metadata is available; body work was deliberately deferred.
    Pending,
    /// Complete admitted bytes were actually checked at this observation boundary.
    Verified,
    /// Explicit work budget stopped before verification, without treating it as absence.
    Budget,
    /// Actual typed backend failure, including unsupported/denied/offline/changed.
    Failed(BackendError),
}
/// Versioned metadata-only observation; equality does not grant access to any source bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Explicit persisted observation encoding version.
    pub format_version: u32,
    /// Physical selected root; copied homes cannot reuse this private projection as native evidence.
    pub root: (u64, u64),
    /// Unique scan identity; not a causal/global ordering clock.
    pub scan: String,
    /// Stable independent asset data identity.
    pub source_id: String,
    /// Native hard-link/identity ambiguity is retained; later equal identity cannot pick an annotation owner.
    pub identity_ambiguous: bool,
    /// Actual current (or last-known tombstone) qualified metadata.
    pub stat: StatSnapshot,
    /// Complete actually verified bytes, null when metadata-only/unknown/deleted.
    pub version: Option<ExpectedVersion>,
    /// Body work outcome is separate from metadata availability/current identity.
    pub verification: Verification,
    /// Honest actual identity result, separate from availability.
    pub change: ChangeKind,
    /// Current actual availability, not inferred from a partial listing.
    pub availability: Availability,
}
/// Actual qualified complete/partial scan boundary; never a native event replay or absence grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    /// Explicit metadata encoding version.
    pub format_version: u32,
    /// Actual selected native root identity.
    pub root: (u64, u64),
    /// Exact library/mount/backend qualifier, including multiple mounts of the same physical root.
    pub backend: BackendIdentity,
    /// Unique scan identity, not global ordering.
    pub scan: String,
    /// Actual read-policy revision/exclusion fingerprint; not an approval or content digest.
    pub read_scope: String,
    /// Actual relevant scan completeness.
    pub complete: bool,
    /// Actual UTC completion/partial observation timestamp.
    pub observed_at: String,
    /// Current native scheduling classification.
    pub schedule: Schedule,
}
/// Preserve watch refusal classifications without converting continuity loss into empty success.
fn watch_error(e: knowledge_local::WatchError) -> Error {
    use knowledge_local::WatchError;
    match e {
        WatchError::Admission(e) => Error::Admission(e),
        WatchError::WaitingMemory => Error::Waiting(knowledge_core::admission::Waiting::Memory),
        WatchError::Budget => Error::Budget,
        WatchError::Backend(e) => Error::Backend(e),
        WatchError::Unsupported => Error::Unsupported,
        WatchError::QuiescenceUnknown => Error::Indeterminate,
        WatchError::Native | WatchError::Exhausted | WatchError::LeaseExpired => {
            Error::Backend(BackendError::Unavailable(Unavailable::Changed))
        }
    }
}
/// Native cursor retained only after its corresponding complete inventory checkpoint is durable.
/// It is diagnostic replay state, not source identity, read permission, or permission to prune on restart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventCursor {
    /// Exact held native root (device, inode).
    pub root: (u64, u64),
    /// Selected backend/library/mount qualifier.
    pub backend: BackendIdentity,
    /// Current policy and exclusion scope fingerprint.
    pub read_scope: String,
    /// Actual complete durable inventory checkpoint this record follows.
    pub scan: String,
    /// Actual device event-store UUID.
    pub volume: [u8; 16],
    /// Observed device-relative FSEvent ID, captured before that inventory scan.
    pub event_id: u64,
}
/// Bounded durable replay output; caller retains pre-verification memory through these owned values.
pub struct Replay {
    /// Actual validated metadata records, never source bodies.
    pub observations: Vec<Observation>,
    /// Actual complete/partial boundaries let restart retain the last complete scan independently.
    pub checkpoints: Vec<Checkpoint>,
    /// Post-inventory native event cursors; restart still requires actual full bounded catch-up.
    pub event_cursors: Vec<EventCursor>,
    /// Next exact retained byte position; malformed bytes are never skipped.
    pub next_offset: u64,
    /// The selected current stream EOF was actually reached.
    pub complete: bool,
    /// Malformed/truncated/contradictory bytes remain visible for explicit recovery.
    pub issue: Option<Error>,
}
/// Explicit current reconciliation controls; no timer/daemon or manifest-derived read grant.
pub struct ReconcileOptions {
    /// Hash admitted sources now; false performs metadata only and schedules later body work.
    pub verify_bytes: bool,
    /// Native caller's actual schedule classification, shown separately from scan completion.
    pub schedule: Schedule,
}
/// Native scheduling intent/status; NAS is never silently advertised as reliable local events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schedule {
    /// Ordinary local startup/periodic catch-up.
    StartupPeriodic,
    /// OS-mounted NAS nightly/manual reconciliation, not invented event continuity.
    NightlyManual,
    /// Explicit owner-requested reconciliation only.
    Manual,
}
/// Current bounded work result; previous complete generations/pins remain independently retained.
pub struct Reconciled {
    /// Actual metadata objects observed by the native stream.
    pub observed: u64,
    /// Sources whose complete bytes were actually verified now.
    pub verified: u64,
    /// Sources withheld from current byte claims because verification failed/was not requested.
    pub pending: u64,
    /// Actual failed body checks, independently visible from metadata matches.
    pub failed: u64,
    /// Actual unsupported body checks, not empty successful extraction.
    pub unsupported: u64,
    /// Only complete relevant successful enumeration can establish absence.
    pub complete: bool,
    /// Last-complete journal checkpoint was actually synchronized for this scan.
    pub last_complete: Option<String>,
    /// Explicit native schedule, not an executing hidden scheduler.
    pub schedule: Schedule,
}
/// Disposable qualified projection frame anchored to exact actual visible journal bytes.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Cached {
    /// Metadata-only current state.
    observation: Observation,
    /// Exact durable journal anchor.
    proof: journal::Proof,
}
/// Immutable per-key frames on the actual account-selected generated volume; no annotation authority.
struct Projection {
    /// Private qualified selected cache directory.
    dir: File,
    /// Exact native namespace, not inferred from cached bytes.
    root: (u64, u64),
    /// Exact selected qualifier, including mount/backend separation.
    backend: BackendIdentity,
    /// Independent stream identity; offsets are never compared between writers.
    writer: String,
}
impl Projection {
    /// Namespace by actual root/library/writer and the current visible stream incarnation.
    /// Reused native root inodes cannot revive disposable frames from a different journal history.
    fn open(
        root: (u64, u64),
        backend: &BackendIdentity,
        writer: &str,
        generation: &str,
    ) -> Result<Self, Error> {
        let parent = Account::current()?.generated_directory()?;
        let parent = native::private_child(&parent, "inventory-v1")?;
        let key = hash(
            serde_json::to_string(&(root, backend, writer))
                .map_err(|_| Error::Invalid)?
                .as_bytes(),
        );
        let namespace = native::private_child(&parent, &key)?;
        Ok(Self {
            dir: native::private_child(&namespace, generation)?,
            root,
            backend: backend.clone(),
            writer: writer.to_owned(),
        })
    }
    /// Locate highest writer-local byte-offset frame without loading a whole corpus/history.
    fn latest(&self, key: &str, b: &Bounds) -> Result<Option<Cached>, Error> {
        let dir = match native::directory(&self.dir, key) {
            Ok(d) => d,
            Err(Error::Missing) => return Ok(None),
            Err(e) => return Err(e),
        };
        let Some(name) = latest_name(&dir, b)? else {
            return Ok(None);
        };
        let cached: Cached = native::control(&dir, &name, b.control())?;
        if cached.observation.root != self.root || cached.observation.stat.backend != self.backend {
            return Err(Error::Recovery);
        }
        Ok(Some(cached))
    }
    /// Lookup actual qualified path state; cached data is revalidated against its visible journal anchor.
    fn path(&self, path: &str, b: &Bounds, journal_dir: &File) -> Result<Option<Cached>, Error> {
        let v = self.latest(&format!("p-{}", hash(path.as_bytes())), b)?;
        if let Some(c) = &v {
            if c.observation.stat.path.as_str() != path {
                return Err(Error::Recovery);
            }
            self.verify(c, b, journal_dir)?;
        }
        Ok(v)
    }
    /// Lookup only reuse-protected qualified native identity; equal content is deliberately irrelevant.
    fn identity(
        &self,
        stat: &StatSnapshot,
        b: &Bounds,
        journal_dir: &File,
    ) -> Result<Option<Cached>, Error> {
        let Some(k) = identity_key(stat)? else {
            return Ok(None);
        };
        let v = self.latest(&k, b)?;
        if let Some(c) = &v {
            self.verify(c, b, journal_dir)?;
        }
        Ok(v)
    }
    /// Verify a writer-qualified visible anchor rather than a foreign stream offset.
    fn verify(&self, cached: &Cached, b: &Bounds, dir: &File) -> Result<(), Error> {
        if cached.proof.writer != self.writer {
            return Err(Error::Recovery);
        }
        journal::Journal::read(dir, &self.writer, b.control())?
            .verify_tracking(&cached.proof, &cached.observation)
    }
    /// Publish immutable generated metadata with actual same-volume promises/write reconciliation.
    fn save(&self, c: &Cached, b: &Bounds) -> Result<(), Error> {
        let mut keys = vec![format!(
            "p-{}",
            hash(c.observation.stat.path.as_str().as_bytes())
        )];
        if let Some(k) = identity_key(&c.observation.stat)? {
            keys.push(k)
        }
        for key in keys {
            b.check()?;
            let dir = native::private_child(&self.dir, &key)?;
            let name = format!("{:020}.json", c.proof.position());
            if native::regular(&dir, &name)?.is_some() {
                let old: Cached = native::control(&dir, &name, b.control())?;
                if old != *c {
                    return Err(Error::Recovery);
                }
                continue;
            }
            generated_json(&dir, &name, c, b)?;
        }
        Ok(())
    }
}
/// Hash only reuse-protected qualified file identity, never inode/mtime/content alone.
fn identity_key(stat: &StatSnapshot) -> Result<Option<String>, Error> {
    let Some(id) = &stat.file_id else {
        return Ok(None);
    };
    if id.volume_id.is_none() || (id.birth_ns.is_none() && id.generation.is_none()) {
        return Ok(None);
    }
    Ok(Some(format!(
        "i-{}",
        hash(
            serde_json::to_string(id)
                .map_err(|_| Error::Invalid)?
                .as_bytes()
        )
    )))
}
/// Enumerate immutable metadata frame names under deadline, without an unbounded list allocation.
fn latest_name(dir: &File, b: &Bounds) -> Result<Option<String>, Error> {
    let mut latest = None::<String>;
    let fd = rustix::fs::openat(
        dir,
        ".",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| Error::Io)?;
    let iter = rustix::fs::Dir::read_from(fd).map_err(|_| Error::Io)?;
    for e in iter {
        b.check()?;
        let e = e.map_err(|_| Error::Io)?;
        let name = e.file_name().to_str().map_err(|_| Error::Recovery)?;
        if name == "." || name == ".." {
            continue;
        }
        if name.len() != 25
            || !name.ends_with(".json")
            || !name[..20].bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Error::Recovery);
        }
        if latest.as_ref().is_none_or(|n| name > n.as_str()) {
            latest = Some(name.to_owned())
        }
    }
    Ok(latest)
}
/// Reserve generated bytes before exclusive metadata publication, then reconcile confirmed actual writes.
fn generated_json<T: Serialize>(
    dir: &File,
    name: &str,
    value: &T,
    b: &Bounds,
) -> Result<(), Error> {
    let mut size = JsonSize {
        bytes: 0,
        limit: b.control(),
    };
    serde_json::to_writer(&mut size, value).map_err(|_| Error::Budget)?;
    let bytes = size.bytes.checked_add(1).ok_or(Error::Budget)?;
    let mut holding = match Account::current()?.reserve(
        Reservation {
            memory_bytes: 0,
            temporary_bytes: bytes,
        },
        Origin::BeforeVerification,
    )? {
        Decision::Ready(h) => h,
        Decision::Waiting(w) => return Err(Error::Waiting(w)),
    };
    let result = native::control_new(dir, name, value);
    if result.is_ok() {
        holding.written(bytes)?;
        holding.finish()?;
    } else {
        // The exclusive writer returned and its exact FD closed: no async/native work remains.
        // Keep partial bytes untouched; fresh native free space accounts them, not an invented deletion.
        holding.finish()?;
        return result;
    }
    Ok(())
}
impl<P: WritePolicy> Store<P> {
    /// Reconcile the exact watched mount, then persist its pre-scan native ID only after durability.
    /// Gaps, concurrent hints, partial scans and current root/scope drift never advance a cursor.
    /// Restart deliberately uses a fresh stream plus full bounded catch-up, not assumed replay continuity.
    /// The host still owns debounce scheduling and the lifetime of its prepaid input/result holding.
    ///
    /// Compiled consumer shape only; actual native durability requires the native integration test.
    /// ```no_run
    /// use knowledge_local::{HintWatch, LocalReadPolicy};
    /// use knowledge_storage::{Error, Preflight, ReconcileOptions, Reconciled, Schedule, Store, WritePolicy};
    /// fn catch_up<P: WritePolicy, R: LocalReadPolicy>(
    ///     store: &Store<P>, watch: &mut HintWatch<'_, R>, preflight: &Preflight<'_>,
    /// ) -> Result<Reconciled, Error> {
    ///     store.reconcile_watched(watch, ReconcileOptions {
    ///         verify_bytes: true, schedule: Schedule::StartupPeriodic,
    ///     }, preflight)
    /// }
    /// ```
    pub fn reconcile_watched<R: LocalReadPolicy>(
        &self,
        watch: &mut knowledge_local::HintWatch<'_, R>,
        options: ReconcileOptions,
        preflight: &Preflight<'_>,
    ) -> Result<Reconciled, Error> {
        let b = Bounds::new(preflight)?;
        let before = watch.poll().map_err(watch_error)?;
        let cursor = watch.native_cursor().map_err(watch_error)?;
        let mount = watch.mount();
        let scope = mount
            .read_scope(preflight.budget, preflight.holding)
            .map_err(Error::Backend)?;
        let result = self.reconcile_local(mount, options, preflight)?;
        let after = watch.poll().map_err(watch_error)?;
        if result.complete && before.epoch == after.epoch && after.coverage_complete {
            if let Some(cursor) = cursor {
                if cursor.root != mount.root_identity().map_err(Error::Backend)?
                    || scope
                        != mount
                            .read_scope(preflight.budget, preflight.holding)
                            .map_err(Error::Backend)?
                {
                    return Err(Error::Context);
                }
                let c = self.gate()?;
                self.live(&c)?;
                b.check()?;
                let lane = self.lane()?;
                let mut journal = journal::Journal::open(&lane.journal, &self.writer, b.control())?;
                journal.append_event_cursor(&EventCursor {
                    root: cursor.root,
                    backend: mount.context(),
                    read_scope: scope,
                    scan: result.last_complete.clone().ok_or(Error::Recovery)?,
                    volume: cursor.volume,
                    event_id: cursor.event_id,
                })?;
            }
            watch
                .acknowledge_reconciliation(after.epoch)
                .map_err(watch_error)?;
        }
        Ok(result)
    }
    /// Replay validated visible observations with a bounded working set and an exact byte cursor.
    /// Root/policy/writer checks remain native; malformed bytes are retained and reported, not skipped.
    pub fn replay_tracking(&self, offset: u64, preflight: &Preflight<'_>) -> Result<Replay, Error> {
        let b = Bounds::new(preflight)?;
        let c = self.gate()?;
        let lane = self.existing_lane()?;
        let mut journal = journal::Journal::read(&lane.journal, &self.writer, b.control())?;
        let mut observations = Vec::new();
        let mut checkpoints = Vec::new();
        let mut event_cursors = Vec::new();
        let mut cursor = offset;
        let mut consumed = 0u64;
        let mut issue = None;
        let mut complete = false;
        while consumed < b.bytes
            && ((observations.len() + checkpoints.len() + event_cursors.len()) as u64)
                < preflight.budget.max_files
        {
            b.check()?;
            match journal.next_tracking(cursor, b.control().min(b.memory / 64)) {
                Ok(Some((next, item))) => {
                    consumed = consumed.checked_add(next - cursor).ok_or(Error::Budget)?;
                    cursor = next;
                    if let Some(journal::ReplayItem::Checkpoint(checkpoint)) = &item {
                        if checkpoint.backend.library_id != self.library {
                            return Err(Error::Recovery);
                        }
                        checkpoints.push(checkpoint.clone());
                    }
                    if let Some(journal::ReplayItem::EventCursor(cursor)) = &item {
                        if cursor.backend.library_id != self.library {
                            return Err(Error::Recovery);
                        }
                        event_cursors.push(cursor.clone());
                    }
                    if let Some(journal::ReplayItem::Source(observation, _)) = item {
                        if observation.stat.backend.library_id != self.library {
                            return Err(Error::Recovery);
                        }
                        observations.push(*observation)
                    }
                }
                Ok(None) => {
                    complete = true;
                    break;
                }
                Err(e) => {
                    issue = Some(e);
                    break;
                }
            }
            if (observations.len() + checkpoints.len() + event_cursors.len()) as u64 * b.control()
                > b.memory / 4
            {
                break;
            }
        }
        self.live(&c)?;
        Ok(Replay {
            observations,
            checkpoints,
            event_cursors,
            next_offset: cursor,
            complete,
            issue,
        })
    }
    /// Stream complete/partial metadata, verify selected actual bytes and append durable identity observations.
    /// Cache projections are rebuilt from the visible journal before reuse; incomplete scans never tombstone.
    pub fn reconcile_local<R: LocalReadPolicy>(
        &self,
        mount: &LocalMount<R>,
        options: ReconcileOptions,
        preflight: &Preflight<'_>,
    ) -> Result<Reconciled, Error> {
        let b = Bounds::new(preflight)?;
        let c = self.gate()?;
        let root = mount.root_identity().map_err(Error::Backend)?;
        let read_scope = mount
            .read_scope(preflight.budget, preflight.holding)
            .map_err(Error::Backend)?;
        let lane = self.lane()?;
        let mut journal = journal::Journal::open(&lane.journal, &self.writer, b.control())?;
        if mount.context_bytes().is_none_or(|n| n > b.control() / 16) {
            return Err(Error::Budget);
        }
        let backend = mount.context();
        let selected_cache = Account::current()?.generated_directory()?;
        if native::within(&selected_cache, &self.root)?
            || mount
                .contains_directory(&selected_cache, preflight.budget, preflight.holding)
                .map_err(Error::Backend)?
        {
            return Err(Error::Context);
        }
        let inventory = Inventory::rebuild(root, &backend, &self.writer, &lane.journal, &b)?;
        let scan = uuid::Uuid::new_v4().to_string();
        journal.append(&scan, &hash(scan.as_bytes()), None, "reconcile_started")?;
        let mut result = Reconciled {
            observed: 0,
            verified: 0,
            pending: 0,
            failed: 0,
            unsupported: 0,
            complete: false,
            last_complete: None,
            schedule: options.schedule,
        };
        let mut failure = None;
        let mut bytes = 0u64;
        let budget = Budget {
            max_memory_bytes: b.memory / 4,
            ..preflight.budget
        };
        let listing = mount.visit(budget, preflight.holding, |entry| {
            let run = (|| -> Result<(), Error> {
                b.check()?;
                self.live(&c)?;
                if entry.kind != EntryKind::File || entry.path.as_str() == "omni-knowledge.json" {
                    return Ok(());
                }
                let stat = entry.stat.snapshot.ok_or(Error::Recovery)?;
                if stat.backend != backend || stat.backend.library_id != self.library {
                    return Err(Error::Context);
                }
                let old = inventory.path(&stat, &b)?;
                let identity = inventory.identity(&stat, &b)?;
                let matched = old.as_ref().filter(|v| {
                    v.observation.change != ChangeKind::Deleted
                        && identity_key(&v.observation.stat).ok().flatten().is_some()
                        && identity_key(&v.observation.stat).ok() == identity_key(&stat).ok()
                });
                let moved = identity.as_ref().filter(|v| {
                    !v.observation.identity_ambiguous
                        && v.observation.change != ChangeKind::Deleted
                        && v.observation.stat.path != stat.path
                        && mount.link_count(&stat.path, budget, preflight.holding).ok() == Some(1)
                        && mount
                            .stat(&v.observation.stat.path, budget)
                            .ok()
                            .is_some_and(|o| {
                                o.availability == Availability::Unavailable(Unavailable::Deleted)
                            })
                });
                let prior = matched.or(moved);
                let source_id = prior
                    .map(|p| p.observation.source_id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let mut change = if moved.is_some() {
                    ChangeKind::Renamed
                } else if matched.is_some() {
                    ChangeKind::Current
                } else if old.is_some() {
                    ChangeKind::IdentityUncertain
                } else {
                    ChangeKind::Created
                };
                let mut verification = Verification::Pending;
                let version = if options.verify_bytes {
                    bytes = bytes.checked_add(stat.size_bytes).ok_or(Error::Budget)?;
                    if bytes > b.bytes {
                        verification = Verification::Budget;
                        result.pending += 1;
                        None
                    } else {
                        match mount.verify(&stat.path, budget, preflight.holding) {
                            Ok(v) if v.snapshot == stat => {
                                result.verified += 1;
                                verification = Verification::Verified;
                                Some(v)
                            }
                            Ok(_) => {
                                verification = Verification::Failed(BackendError::Unavailable(
                                    Unavailable::Changed,
                                ));
                                result.failed += 1;
                                None
                            }
                            Err(e) => {
                                verification = Verification::Failed(e);
                                result.failed += 1;
                                if e == BackendError::Unavailable(Unavailable::Unsupported) {
                                    result.unsupported += 1;
                                }
                                None
                            }
                        }
                    }
                } else {
                    result.pending += 1;
                    None
                };
                if prior.is_some_and(|p| {
                    p.observation.version.as_ref().map(|v| &v.digest)
                        != version.as_ref().map(|v| &v.digest)
                }) && version.is_some()
                    && change == ChangeKind::Current
                {
                    change = ChangeKind::Changed;
                }
                let observation = Observation {
                    format_version: 1,
                    root,
                    scan: scan.clone(),
                    source_id,
                    identity_ambiguous: identity
                        .as_ref()
                        .is_some_and(|v| v.observation.identity_ambiguous)
                        || mount.link_count(&stat.path, budget, preflight.holding).ok() != Some(1),
                    stat,
                    version,
                    verification,
                    change,
                    availability: entry.stat.availability,
                };
                let proof = journal.append_tracking(&scan, &observation)?;
                inventory.save(&Cached { observation, proof }, &b)?;
                result.observed += 1;
                Ok(())
            })();
            if let Err(e) = run {
                failure = Some(e);
                return Err(BackendError::Io);
            }
            Ok(())
        });
        if let Some(e) = failure {
            return Err(e);
        }
        result.complete = matches!(listing, Ok(Enumeration::Complete { .. }));
        if let Err(e) = listing {
            journal.append(&scan, &hash(scan.as_bytes()), None, "reconcile_unavailable")?;
            return Err(Error::Backend(e));
        }
        if mount
            .read_scope(preflight.budget, preflight.holding)
            .map_err(Error::Backend)?
            != read_scope
        {
            return Err(Error::Backend(BackendError::Unavailable(
                Unavailable::Changed,
            )));
        }
        if result.complete {
            // Inspect every known qualified path; neither partial coverage nor scope denial can infer deletion.
            for projection in &inventory.projections {
                let fd = rustix::fs::openat(
                    &projection.dir,
                    ".",
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|_| Error::Io)?;
                for e in rustix::fs::Dir::read_from(fd).map_err(|_| Error::Io)? {
                    b.check()?;
                    let e = e.map_err(|_| Error::Io)?;
                    let key = e.file_name().to_str().map_err(|_| Error::Recovery)?;
                    if !key.starts_with("p-") {
                        continue;
                    }
                    let Some(frame) = projection.latest(key, &b)? else {
                        continue;
                    };
                    let Some(old) = inventory.path(&frame.observation.stat, &b)? else {
                        continue;
                    };
                    if old.observation.scan == scan || old.observation.change == ChangeKind::Deleted
                    {
                        continue;
                    }
                    if inventory
                        .identity(&old.observation.stat, &b)?
                        .is_some_and(|v| {
                            v.observation.scan == scan
                                && v.observation.source_id == old.observation.source_id
                                && v.observation.stat.path != old.observation.stat.path
                        })
                    {
                        continue;
                    }
                    let current = match mount.stat(&old.observation.stat.path, budget) {
                        Ok(s) => s,
                        Err(BackendError::Unavailable(Unavailable::Denied)) => continue,
                        Err(e) => return Err(Error::Backend(e)),
                    };
                    if current.availability == Availability::Unavailable(Unavailable::Deleted) {
                        let mut observation = old.observation;
                        observation.scan = scan.clone();
                        observation.version = None;
                        observation.verification = Verification::Pending;
                        observation.change = ChangeKind::Deleted;
                        observation.availability = Availability::Unavailable(Unavailable::Deleted);
                        let proof = journal.append_tracking(&scan, &observation)?;
                        inventory.save(&Cached { observation, proof }, &b)?;
                    }
                }
            }
            self.live(&c)?;
            if mount
                .read_scope(preflight.budget, preflight.holding)
                .map_err(Error::Backend)?
                != read_scope
            {
                return Err(Error::Backend(BackendError::Unavailable(
                    Unavailable::Changed,
                )));
            }
            journal.append_checkpoint(&Checkpoint {
                format_version: 1,
                root,
                backend: mount.context(),
                scan: scan.clone(),
                read_scope: read_scope.clone(),
                complete: true,
                observed_at: time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(|_| Error::Io)?,
                schedule: options.schedule,
            })?;
            result.last_complete = Some(scan);
        } else {
            journal.append_checkpoint(&Checkpoint {
                format_version: 1,
                root,
                backend: mount.context(),
                scan: scan.clone(),
                read_scope,
                complete: false,
                observed_at: time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(|_| Error::Io)?,
                schedule: options.schedule,
            })?;
        }
        Ok(result)
    }
}
