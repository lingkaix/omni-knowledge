//! Actual quiet/stable external-text Git batches through the existing ordinary backend.
//! This records Git-only outcomes; M1 still owns journal/file-domain publication and recovery.
//! Drift returns Conflict, policy revocation Denied, inadequate work Capacity and missing signer SigningRequired; all retain prepared recovery material.
#![deny(missing_docs)]
use super::*;
use knowledge_core::backend::{
    Availability, BackendError, Budget, ByteRange, ExpectedVersion, MountBackend, Unavailable,
};
use knowledge_core::data::ContentDigest;

/// One currently qualified backend byte version and its real native observation ID.
/// Construction supplies no read/write permission: the backend rechecks each actual read.
#[derive(Clone, Debug)]
pub struct ExternalText {
    /// Complete verified backend/path/stat/content version, rechecked against actual bytes.
    pub version: ExpectedVersion,
    /// Actual producer observation identity, never external-editor attribution.
    pub observation_id: String,
    /// True only for an actually observed deletion; backend absence is rechecked, never inferred from offline/denied.
    /// The version then identifies the prior qualified source, not bytes claimed to exist now.
    pub deleted: bool,
}
/// Observable waiting versus actual Git publication; never a claim of atomic saved-local state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalBatchState {
    /// Stable observations exist, but the actual monotonic quiet period has not elapsed.
    Waiting {
        /// Informational wall-clock projection; it never bypasses the monotonic wait.
        not_before_unix_millis: u64,
    },
    /// Actual existing Git transaction outcome, not file-domain publication.
    History(State),
}
/// Durable first stable observation, binding the batch identity across restart without storing source bodies.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct QuietIntent {
    /// Complete qualified versions, observation IDs, quiet policy and attribution binding.
    fingerprint: String,
    /// Actual accepted predecessor at the first stable observation.
    base: Option<Oid>,
    /// Original visible due fact; restart still requires a fresh monotonic observation.
    not_before_unix_millis: u64,
}
/// Complete qualified byte-version selection for a Core job key; this digest is data, not authority.
/// Order is significant; duplicate paths refuse in observe_external_batch.
pub fn external_batch_version(sources: &[ExternalText]) -> Result<ContentDigest, Error> {
    external_batch_version_bounded(sources, CONTROL_LIMIT)
}
/// Bound complete variable observation metadata before Serde can allocate its encoded fingerprint input.
fn external_batch_version_bounded(
    sources: &[ExternalText],
    limit: usize,
) -> Result<ContentDigest, Error> {
    let mut bound = sources.len().checked_mul(2048).ok_or(Error::Capacity)?;
    for source in sources {
        let snapshot = &source.version.snapshot;
        let mut charge = |text: &str| -> Result<(), Error> {
            bound = bound
                .checked_add(text.len().checked_mul(6).ok_or(Error::Capacity)?)
                .ok_or(Error::Capacity)?;
            if bound > limit {
                return Err(Error::Capacity);
            }
            Ok(())
        };
        for text in [
            source.observation_id.as_str(),
            snapshot.path.as_str(),
            snapshot.backend.library_id.as_str(),
        ]
        .into_iter()
        .chain(snapshot.backend.mount_id.as_deref())
        .chain(snapshot.backend_version.as_deref())
        {
            charge(text)?;
        }
        if let Some(file) = &snapshot.file_id {
            for text in [file.backend.library_id.as_str(), file.file_id.as_str()]
                .into_iter()
                .chain(file.backend.mount_id.as_deref())
                .chain(file.volume_id.as_deref())
                .chain(file.generation.as_deref())
            {
                charge(text)?;
            }
        }
    }
    if bound > limit {
        return Err(Error::Capacity);
    }
    let mut hasher = Sha256::new();
    for source in sources {
        validate_id(&source.observation_id)?;
        let bytes = serde_json::to_vec(&(&source.version, &source.observation_id, source.deleted))
            .map_err(|_| Error::Invalid)?;
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    ContentDigest::new(format!("{:x}", hasher.finalize())).map_err(|_| Error::Invalid)
}
impl History {
    /// Recheck the actual selected producer, including its held root FD and current read policy.
    fn check_external_backend(
        &self,
        backend: &knowledge_local::LocalMount<impl knowledge_local::LocalReadPolicy>,
        call: &Call,
    ) -> Result<(), Error> {
        call.check()?;
        self.validate()?;
        let actual = backend.root_identity().map_err(backend_error)?;
        if actual != (self.binding.home.device, self.binding.home.inode) {
            return Err(Error::ContextMismatch);
        }
        Ok(())
    }
    /// Observe actual selected text versions, wait a native elapsed quiet period, then commit exactly one batch.
    /// Reads verify full digests and stat/file identity before AND after candidate preparation.
    /// A changed source/base preserves the candidate and refuses publication; a completed retry is idempotent.
    /// Callers submit this operation to their existing queue using external_batch_version as key.version.
    pub fn observe_external_batch(
        &self,
        operation_id: &str,
        sources: &[ExternalText],
        backend: &knowledge_local::LocalMount<impl knowledge_local::LocalReadPolicy>,
        quiet: Duration,
        owner: &NativeHistoryOwner,
        work: HistoryWork<'_>,
    ) -> Result<ExternalBatchState, Error> {
        let call = work.call(self, owner)?;
        self.observe_external_batch_call(operation_id, sources, backend, quiet, work, &call)
    }

    /// Use the scheduling call's original accounting and current-selection checks, without renewing its ceilings.
    pub(crate) fn observe_external_batch_call(
        &self,
        operation_id: &str,
        sources: &[ExternalText],
        backend: &knowledge_local::LocalMount<impl knowledge_local::LocalReadPolicy>,
        quiet: Duration,
        work: HistoryWork<'_>,
        call: &Call,
    ) -> Result<ExternalBatchState, Error> {
        self.check_external_backend(backend, call)?;
        validate_id(operation_id)?;
        if self.owner.agent.is_some() || sources.is_empty() || quiet.is_zero() {
            return Err(Error::Invalid);
        }
        if sources.len() as u64 > call.files {
            return Err(Error::Capacity);
        }
        if sources.iter().any(|s| {
            s.version.snapshot.backend.library_id != work.key.library
                || s.version.snapshot.backend.mount_id.is_some()
        }) {
            return Err(Error::ContextMismatch);
        }
        let version = external_batch_version_bounded(sources, call.limit)?;
        if work.key.version != &version {
            return Err(Error::ContextMismatch);
        }
        let fingerprint = digest(
            &serde_json::to_vec(&(version.as_str(), quiet.as_millis(), self.context_digest()?))
                .map_err(|_| Error::Invalid)?,
        );
        let path = self
            .repo
            .join("omni-state/operations")
            .join(format!("{operation_id}.quiet.json"));
        let lock = self.lock(call)?;
        let scratch = self.scratch(call)?;
        let prior: Option<QuietIntent> = if path.exists() {
            Some(load(&path)?)
        } else {
            None
        };
        if let Some(prior) = &prior {
            if prior.fingerprint != fingerprint {
                return Err(Error::OperationMismatch);
            }
            if self.intent_path(operation_id).exists() {
                let intent = self.read_intent(operation_id)?;
                let state = self.inspect_intent(&intent, call)?;
                if matches!(state, State::Published(_) | State::Unchanged(_)) {
                    return Ok(ExternalBatchState::History(state));
                }
            }
        }
        let changes = match self.read_external(sources, backend, call) {
            Ok(changes) => changes,
            Err(error) => {
                self.quiet.lock().map_err(|_| Error::Recovery)?.take();
                return Err(error);
            }
        };
        let elapsed = {
            let mut clock = self.quiet.lock().map_err(|_| Error::Recovery)?;
            match clock.as_ref() {
                Some((id, since)) if id == operation_id => since.elapsed(),
                _ => {
                    *clock = Some((operation_id.into(), Instant::now()));
                    Duration::ZERO
                }
            }
        };
        let first = match prior {
            Some(first) => first,
            None => {
                let first = QuietIntent {
                    fingerprint,
                    base: self.reference(ACCEPTED, scratch.path(), call)?,
                    not_before_unix_millis: unix_millis()?
                        .checked_add(u64::try_from(quiet.as_millis()).map_err(|_| Error::Invalid)?)
                        .ok_or(Error::Invalid)?,
                };
                call.check()?;
                persist(&path, &first)?;
                first
            }
        };
        if elapsed < quiet {
            return Ok(ExternalBatchState::Waiting {
                not_before_unix_millis: unix_millis()?
                    .checked_add(
                        u64::try_from((quiet - elapsed).as_millis()).map_err(|_| Error::Invalid)?,
                    )
                    .ok_or(Error::Invalid)?,
            });
        }
        if self.reference(ACCEPTED, scratch.path(), call)? != first.base {
            return Err(Error::Conflict);
        }
        drop(lock);
        let request = Request {
            operation_id: operation_id.into(),
            base: first.base,
            destination: Destination::Accepted,
            changes,
        };
        let observations: Vec<_> = sources.iter().map(|s| s.observation_id.clone()).collect();
        self.prepare_kind_call(&request, &observations, call)?;
        // The producer may have changed while Git was signing/preparing. Re-read actual qualified versions before ref publication.
        self.read_external(sources, backend, call)?;
        self.check_external_backend(backend, call)?;
        Ok(ExternalBatchState::History(
            self.publish_call(operation_id, call)?,
        ))
    }

    /// Read real before/after stable versions through their actual backend and current policy.
    /// Bytes, repeated hashing, entries and retained strings consume the same whole-call work ceiling.
    fn read_external(
        &self,
        sources: &[ExternalText],
        backend: &knowledge_local::LocalMount<impl knowledge_local::LocalReadPolicy>,
        call: &Call,
    ) -> Result<Vec<Change>, Error> {
        let mut changes = Vec::new();
        let mut retained = 0usize;
        for source in sources {
            self.check_external_backend(backend, call)?;
            validate_id(&source.observation_id)?;
            let expected = &source.version;
            let path = expected.snapshot.path.as_str();
            self.validate_path(path)?;
            if expected.snapshot.backend.library_id.is_empty()
                || expected.snapshot.backend.mount_id.is_some()
                || changes.iter().any(|change: &Change| change.path == path)
            {
                return Err(Error::Invalid);
            }
            let size = if source.deleted {
                0
            } else {
                usize::try_from(expected.snapshot.size_bytes).map_err(|_| Error::Capacity)?
            };
            retained = retained
                .checked_add(size)
                .and_then(|n| n.checked_add(path.len()))
                .ok_or(Error::Capacity)?;
            if retained > call.limit {
                return Err(Error::Capacity);
            }
            call.account_files(3)?;
            let budget = || -> Result<Budget, Error> {
                call.check()?;
                Ok(Budget {
                    max_files: call.files_left.get(),
                    max_bytes: call.bytes_left.get(),
                    max_memory_bytes: call.limit.saturating_sub(retained.saturating_sub(size))
                        as u64,
                    max_millis: u64::try_from(
                        call.deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis(),
                    )
                    .unwrap_or(u64::MAX),
                })
            };
            self.check_external_backend(backend, call)?;
            let before = backend
                .stat(&expected.snapshot.path, budget()?)
                .map_err(backend_error)?;
            if source.deleted {
                if before.availability != Availability::Unavailable(Unavailable::Deleted) {
                    return Err(Error::Conflict);
                }
                self.check_external_backend(backend, call)?;
                let after = backend
                    .stat(&expected.snapshot.path, budget()?)
                    .map_err(backend_error)?;
                if after != before {
                    return Err(Error::Conflict);
                }
                changes.push(Change {
                    path: path.into(),
                    text: None,
                });
                continue;
            }
            if before.availability != Availability::Available
                || before.snapshot.as_ref() != Some(&expected.snapshot)
            {
                return Err(Error::Conflict);
            }
            self.check_external_backend(backend, call)?;
            let read = backend
                .open_range(
                    expected,
                    ByteRange::new(0, expected.snapshot.size_bytes.max(1))
                        .map_err(|_| Error::Invalid)?,
                    budget()?,
                )
                .map_err(backend_error)?;
            call.account_bytes(
                usize::try_from(read.progress.bytes_read).map_err(|_| Error::Capacity)?,
            )?;
            if read.version != *expected
                || !read.end_of_file
                || read.bytes.len() != size
                || digest(&read.bytes) != expected.digest.as_str()
            {
                return Err(Error::Conflict);
            }
            self.check_external_backend(backend, call)?;
            let after = backend
                .stat(&expected.snapshot.path, budget()?)
                .map_err(backend_error)?;
            if after != before {
                return Err(Error::Conflict);
            }
            let text = String::from_utf8(read.bytes).map_err(|_| Error::Invalid)?;
            changes.push(Change {
                path: path.into(),
                text: Some(text),
            });
        }
        Ok(changes)
    }
}
/// Use the actual wall clock only for visible due/wait facts, never permission or source identity.
pub(crate) fn unix_millis() -> Result<u64, Error> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Recovery)?
            .as_millis(),
    )
    .map_err(|_| Error::Recovery)
}
/// Preserve actual backend refusal distinctions relevant to safe publication.
fn backend_error(error: BackendError) -> Error {
    match error {
        BackendError::BudgetExhausted => Error::Capacity,
        BackendError::Unavailable(Unavailable::Changed) => Error::Conflict,
        BackendError::Unavailable(Unavailable::Denied) => Error::Denied,
        BackendError::InvalidRequest => Error::Invalid,
        _ => Error::Io,
    }
}
