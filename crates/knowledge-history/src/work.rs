//! Native-only current selection and the existing queue's complete work binding.
//! These capabilities are never deserialized from a manifest, URL, attribution or service DTO.
//! Invalid scope returns ContextMismatch/Denied; stale, cancelled, underfunded or elapsed actual work refuses before effects.
#![deny(missing_docs)]
use super::*;
use knowledge_core::jobs::{Key, State as JobState};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A native host's live selection, separate from commit attribution and portable library data.
/// The trusted host creates this only after its current owner/root/effect admission.
/// Revoke it on selection, policy or owner-session change; stored schedules cannot recreate it.
#[derive(Clone)]
pub struct NativeHistoryOwner {
    /// Actual shared selection and revocation lifetime, never a stored context flag.
    pub(crate) selected: Arc<Selection>,
}
/// One live native selection; clones share revocation, not a global runtime flag.
pub(crate) struct Selection {
    /// Current native policy/session permission.
    pub(crate) live: AtomicBool,
    /// Native-selected repository, not an imported locator.
    pub(crate) repo: PathBuf,
    /// Native-selected authoritative home.
    pub(crate) home: PathBuf,
    /// Bound actual repository directory identity.
    pub(crate) repo_id: Identity,
    /// Bound actual home directory identity.
    pub(crate) home_id: Identity,
    /// Existing admitted security namespace.
    pub(crate) context: String,
    /// Existing admitted library within that namespace.
    pub(crate) library: String,
}
impl NativeHistoryOwner {
    /// Bind an already admitted native owner selection to this actual repository/home.
    /// This trusted in-process constructor is NOT a wire grant or a human-confirmation decoder.
    pub fn select(history: &History, context: &str, library: &str) -> Result<Self, Error> {
        validate_id(context)?;
        validate_id(library)?;
        history.validate()?;
        Ok(Self {
            selected: Arc::new(Selection {
                live: AtomicBool::new(true),
                repo: history.repo.clone(),
                home: history.home.clone(),
                repo_id: history.binding.repo.clone(),
                home_id: history.binding.home.clone(),
                context: context.into(),
                library: library.into(),
            }),
        })
    }
    /// Stop every later controllable effect using this selection; already completed effects remain facts.
    pub fn revoke(&self) {
        self.selected.live.store(false, Ordering::Release);
    }
    /// Verify current native selection without treating matching IDs as permission.
    pub(crate) fn check(&self, history: &History) -> Result<(), Error> {
        if self.selected.repo != history.repo || self.selected.home != history.home {
            return Err(Error::ContextMismatch);
        }
        self.selected.check()
    }
}
impl Selection {
    /// Recheck the actual live capability and opened native directory identities.
    pub(crate) fn check(&self) -> Result<(), Error> {
        if !self.live.load(Ordering::Acquire) {
            return Err(Error::Denied);
        }
        if identity(&self.repo)? != self.repo_id || identity(&self.home)? != self.home_id {
            return Err(Error::ContextMismatch);
        }
        Ok(())
    }
}

/// Original nonforgeable claim plus its complete verified work descriptor.
/// No URL, timestamp, digest alone, operation name or attribution can substitute for this binding.
#[derive(Clone, Copy)]
pub struct HistoryWork<'a> {
    /// Actual original live claim; the host retains its causing credit through unknown ownership.
    pub claim: &'a Arc<Claim>,
    /// Every verified source/recipe/dependency qualifier, not a digest-only pseudo-key.
    pub key: Key<'a>,
}
impl HistoryWork<'_> {
    /// Establish a guarded call before allocating buffers, generating files or spawning native Git.
    pub(crate) fn call(self, history: &History, owner: &NativeHistoryOwner) -> Result<Call, Error> {
        owner.check(history)?;
        if self.key.context != owner.selected.context
            || self.key.library != owner.selected.library
            || self.key.mount.is_some()
        {
            return Err(Error::ContextMismatch);
        }
        match self
            .claim
            .stop_requested(self.key)
            .map_err(|_| Error::ContextMismatch)?
        {
            None => {}
            Some(_) => return Err(Error::Cancelled),
        }
        let job = self.claim.job();
        let deadline = self.claim.deadline();
        let budget = self.claim.budget();
        let required = History::required_resources(&budget)?;
        // Quarantine/bundle/patch/source versions can coexist with two capture vectors.
        // Keep sixteen complete control slots plus native observation scratch inside, not above, the original memory promise.
        let control = budget
            .max_bytes
            .min(budget.max_memory_bytes / 32)
            .min(CONTROL_LIMIT as u64);
        let rss = budget
            .max_memory_bytes
            .saturating_sub(16 * control + (1 << 20));
        if control == 0 || rss < (1 << 20) {
            return Err(Error::Capacity);
        }
        let initial = self
            .claim
            .owner()
            .status(job, self.key)
            .map_err(|_| Error::ContextMismatch)?;
        if initial.budget != budget || initial.deadline != Some(deadline) {
            return Err(Error::ContextMismatch);
        }
        match initial.state {
            JobState::Unsettled { cancellation: None } => {}
            JobState::Unsettled {
                cancellation: Some(_),
            } => return Err(Error::Cancelled),
            _ => return Err(Error::ContextMismatch),
        }
        if initial.reservation.memory_bytes < required.memory_bytes
            || initial.reservation.temporary_bytes < required.temporary_bytes
        {
            return Err(Error::Capacity);
        }
        let descriptor_bytes = self
            .key
            .context
            .len()
            .checked_add(self.key.library.len())
            .and_then(|n| n.checked_add(self.key.source.len()))
            .and_then(|n| n.checked_add(3 * 64))
            .ok_or(Error::Capacity)?;
        if descriptor_bytes as u64 > control {
            return Err(Error::Capacity);
        }
        // Split the SAME live generation before copying descriptor strings or creating native work.
        let mut call = Call::new(self.claim)?;
        let holding = knowledge_core::admission::Account::current()
            .map_err(|_| Error::ContextMismatch)?
            .split_claim(self.claim, self.key, required)
            .map_err(|_| Error::Capacity)?;
        call.funding = Some(holding);
        call.live_claim = Some(self.claim.clone());
        let claim = self.claim.clone();
        let context = self.key.context.to_owned();
        let library = self.key.library.to_owned();
        let source = self.key.source.to_owned();
        let version = self.key.version.clone();
        let recipe = self.key.recipe.clone();
        let processor = self.key.processor.clone();
        let selected = owner.selected.clone();
        let check = move |credit: Option<&mut knowledge_core::admission::Holding>| {
            let key = Key {
                context: &context,
                library: &library,
                mount: None,
                source: &source,
                version: &version,
                recipe: &recipe,
                processor: &processor,
            };
            if let Some(credit) = credit {
                // Cleanup returns actual credit; revoked policy cannot prevent resource collection.
                return knowledge_core::admission::Account::current()
                    .map_err(|_| Error::ContextMismatch)?
                    .rejoin_claim(credit, &claim, key)
                    .map_err(|_| Error::ContextMismatch);
            }
            selected.check()?;
            if Instant::now() >= claim.deadline() {
                return Err(Error::Deadline);
            }
            match claim
                .stop_requested(key)
                .map_err(|_| Error::ContextMismatch)?
            {
                None => Ok(()),
                Some(_) => Err(Error::Cancelled),
            }
        };
        call.limit = control as usize;
        call.rss_limit = rss;
        call.current = Some(Box::new(check));
        call.check()?;
        Ok(call)
    }
}
