//! Actual native snapshot trust and frozen-generation activation over existing paid owners.
use super::*;
use anyhow::{Context, ensure};
use knowledge_core::admission::Account;
use knowledge_core::backend::Budget;
use knowledge_core::jobs::Claim;
use knowledge_history::{
    History, HistoryWork, KnownSigner, NativeHistoryOwner, SignatureState, Verification,
};
use knowledge_package::{Bounds, FrozenArchive, ManifestVerificationError};
use knowledge_retrieval::{ArchiveSource, Control, StoredGeneration, StoredIdentity, StoredPlan};
use omni_service::authority::SelectionLease;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

/// Explicit native signer policy. This data alone grants nothing: verification also requires
/// the actual owner, current source selection, human-approved OpenFrozen and live Claim.
#[derive(Clone, Copy, Debug)]
pub enum NativeSnapshotPolicy {
    /// Admit only an actual cryptographically valid signature from the selected known policy.
    KnownSignerRequired,
    /// Admit known-valid or explicitly chosen unsigned input; invalid/unknown never downgrade.
    UnsignedExplicit,
}
/// Borrowed existing native owners and finite verification settings; never a wire approval DTO.
pub struct NativeSnapshotRequest<'a> {
    pub owner: &'a Owner,
    pub selection: &'a Selection,
    pub known: Vec<KnownSigner>,
    pub policy: NativeSnapshotPolicy,
    pub history: &'a History,
    pub history_owner: &'a NativeHistoryOwner,
    pub bounds: &'a Bounds,
    pub cancel: &'a AtomicBool,
}
/// Exact immutable generation/range data for the existing mature reader; no pathname authority.
pub struct NativeFrozenReadPlan<'a> {
    pub generation: &'a str,
    pub scope: &'a str,
    pub prefix: &'a str,
    pub ranges: StoredPlan,
    pub bounds: &'a Bounds,
}
/// Actual selected source/Claim/trust epoch retained through the last native range/result owner.
/// It opens no declared external root and never unpacks an index or starts another search engine.
pub struct NativeTrustedArchive {
    processing: NativeProcessing,
    claim: Arc<Claim>,
    lease: Option<SelectionLease>,
    policy: NativeSnapshotPolicy,
    verification: Option<Verification>,
    snapshot: Option<knowledge_core::data::ContentDigest>,
    known: Vec<KnownSigner>,
    reads: AtomicU64,
    verifier_credit: Option<knowledge_core::admission::Holding>,
    // Actual source/key/policy/verification/Claim metadata dies before this independent lifetime.
    _metadata: Work,
}
/// A verifier failure still owns its actual source, Claim and typed native collection capability.
pub struct NativeSnapshotError {
    cause: Option<anyhow::Error>,
    archive: Option<Box<NativeTrustedArchive>>,
    collected: bool,
}
impl std::fmt::Debug for NativeSnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeSnapshotError")
    }
}
impl std::fmt::Display for NativeSnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native snapshot activation refused; actual owner retained")
    }
}
impl std::error::Error for NativeSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause().as_ref())
    }
}
impl NativeSnapshotError {
    /// Borrow the typed failure while its native collection capability remains owned.
    pub fn cause(&self) -> &anyhow::Error {
        self.cause.as_ref().expect("actual snapshot error")
    }
    /// Observed cryptographic state, if verification completed before refusal.
    pub fn verification(&self) -> Option<&Verification> {
        self.archive.as_ref()?.verification.as_ref()
    }
    /// One bounded native attempt, independent of a now-revoked source policy.
    pub fn collect(&mut self) -> Result<()> {
        if !self.collected {
            if let Some(ManifestVerificationError::Native(knowledge_history::Error::Unsettled(
                child,
            ))) = self
                .cause
                .as_ref()
                .and_then(|error| error.downcast_ref::<ManifestVerificationError>())
            {
                child.collect()?;
            }
            self.archive
                .as_mut()
                .context("actual snapshot owner")?
                .release_verifier_credit()?;
            self.collected = true;
        }
        Ok(())
    }
    /// Recover the actual dispatch after real collection; never settle a foreign/stale Claim.
    pub fn into_parts(mut self) -> (NativeProcessing, Arc<Claim>, anyhow::Error) {
        while self.collect().is_err() {
            std::thread::sleep(Duration::from_millis(250));
        }
        let archive = *self.archive.take().expect("actual snapshot source owner");
        let NativeTrustedArchive {
            processing,
            claim,
            lease,
            policy: _,
            verification,
            snapshot,
            known,
            reads: _,
            verifier_credit,
            _metadata,
        } = archive;
        drop((lease, verification, snapshot, known, verifier_credit));
        drop(_metadata);
        (
            processing,
            claim,
            self.cause.take().expect("actual snapshot failure"),
        )
    }
}
impl Drop for NativeSnapshotError {
    fn drop(&mut self) {
        if self.archive.is_none() {
            return;
        }
        while self.collect().is_err() {
            std::thread::sleep(Duration::from_millis(250));
        }
        let archive = self.archive.as_ref().expect("actual snapshot owner");
        let outcome = match archive.claim.stop_requested(archive.processing.key()) {
            Ok(Some(knowledge_core::jobs::Cancellation::Superseded)) => {
                knowledge_core::jobs::Outcome::Superseded
            }
            Ok(Some(_)) => knowledge_core::jobs::Outcome::Cancelled,
            _ => knowledge_core::jobs::Outcome::Failed,
        };
        while Account::current()
            .and_then(|account| {
                account
                    .queue()
                    .settle(&archive.claim, archive.processing.key(), outcome)
                    .map_err(Into::into)
            })
            .is_err()
        {
            // A mismatched dispatch must use into_parts, as with the existing native result owner.
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}
impl NativeTrustedArchive {
    /// Quote all retained native signer strings before moving them into this owner.
    pub fn metadata_requirement(known: &Vec<KnownSigner>) -> Result<Reservation> {
        let spare = (known.capacity() - known.len()) as u64;
        let base = spare
            .checked_mul(std::mem::size_of::<KnownSigner>() as u64)
            .and_then(|n| n.checked_add(256 * 1024))
            .context("native trust metadata overflow")?;
        let memory = known.iter().try_fold(base, |total, signer| {
            (signer.retained_bytes() as u64)
                .checked_add(1024)
                .and_then(|n| n.checked_mul(32))
                .and_then(|n| total.checked_add(n))
                .context("native trust metadata overflow")
        })?;
        Ok(Reservation {
            memory_bytes: memory,
            temporary_bytes: 0,
        })
    }
    /// Compose the simultaneous native verifier/package peak and sequential reader peak before submission.
    pub fn requirement(
        budget: Budget,
        package_memory: u64,
        ranges: StoredPlan,
    ) -> Result<Reservation> {
        let verifier = History::required_resources(&budget)?;
        let reader = ranges.requirement()?;
        Ok(Reservation {
            memory_bytes: verifier
                .memory_bytes
                .checked_add(package_memory)
                .context("native snapshot peak overflow")?
                .max(reader.memory_bytes),
            temporary_bytes: verifier.temporary_bytes.max(reader.temporary_bytes),
        })
    }
    /// Exact consuming Root/lock/vendor fingerprint, not a package-declared compatible flag.
    pub fn build_identity() -> String {
        admitted::build_identity()
    }
    /// Consume the real selected OpenFrozen dispatch and verify its archive and native signer policy.
    /// Errors retain the original source, Arc<Claim>, typed native child and paid metadata.
    pub fn verify(
        processing: NativeProcessing,
        claim: Arc<Claim>,
        request: NativeSnapshotRequest<'_>,
        metadata: Work,
    ) -> std::result::Result<Arc<Self>, NativeSnapshotError> {
        let mut archive = Self {
            processing,
            claim,
            lease: None,
            policy: request.policy,
            verification: None,
            snapshot: None,
            known: request.known,
            reads: AtomicU64::new(0),
            verifier_credit: None,
            _metadata: metadata,
        };
        let result = (|| -> Result<()> {
            ensure!(
                archive._metadata.remaining()?.memory_bytes
                    >= Self::metadata_requirement(&archive.known)?.memory_bytes,
                "native trust metadata is not prepaid"
            );
            archive.lease = Some(
                request
                    .owner
                    .pin_selection(request.selection)
                    .map_err(source::refused_error)?,
            );
            archive.check_source()?;
            let wire::Command::OpenFrozen { path } = archive
                .processing
                .source()
                .permit()
                .admitted_command()
                .map_err(source::refused_error)?
            else {
                bail!("actual OpenFrozen native intention required");
            };
            ensure!(
                Path::new(path) == archive.processing.source().hint(),
                "actual frozen source differs from approved archive"
            );
            let budget = archive.claim.budget();
            let limits = request.bounds.budget;
            ensure!(
                limits.max_bytes <= budget.max_bytes
                    && limits.max_files <= budget.max_files
                    && limits.max_millis <= budget.max_millis
                    && limits.max_memory_bytes <= budget.max_memory_bytes,
                "native snapshot verification exceeds actual Claim budget"
            );
            let account = Account::current()?;
            let mut credit = account.split_claim(
                &archive.claim,
                archive.processing.key(),
                Reservation {
                    memory_bytes: limits.max_memory_bytes,
                    temporary_bytes: 0,
                },
            )?;
            // All borrowed package buffers/readers are destroyed before returning this exact loan.
            let verified = (|| {
                let current = || {
                    archive
                        .check_source()
                        .map_err(|_| knowledge_package::Error::Drift)
                };
                let read = |bytes| {
                    archive
                        .admit_raw(bytes)
                        .map_err(|_| knowledge_package::Error::Quota)
                };
                let mut work = knowledge_package::Work::guarded_reader_metered(
                    request.bounds,
                    &mut credit,
                    request.cancel,
                    &current,
                    &read,
                )?;
                let file = archive.processing.source().file()?.try_clone()?;
                let mut frozen = FrozenArchive::open(file, &mut work)?;
                let snapshot = frozen.snapshot_id().clone();
                let verification = frozen.verify_manifest_signature(
                    request.history,
                    &archive.known,
                    request.history_owner,
                    HistoryWork {
                        claim: &archive.claim,
                        key: archive.processing.key(),
                    },
                )?;
                frozen.revalidate()?;
                Ok::<_, anyhow::Error>((snapshot, verification))
            })();
            // Retain the real loan before any fallible return/finish; preserve an actual unknown
            // native verifier error even if its independent dead package loan cannot yet return.
            archive.verifier_credit = Some(credit);
            let returned = archive.release_verifier_credit();
            let (snapshot, verification) = verified?;
            returned?;
            archive.snapshot = Some(snapshot);
            archive.verification = Some(verification);
            archive.check()?;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(Arc::new(archive)),
            Err(cause) => Err(NativeSnapshotError {
                cause: Some(cause),
                archive: Some(Box::new(archive)),
                collected: false,
            }),
        }
    }
    fn release_verifier_credit(&mut self) -> Result<()> {
        if let Some(credit) = &mut self.verifier_credit {
            if Account::current()?
                .rejoin_claim(credit, &self.claim, self.processing.key())
                .is_err()
            {
                // All borrowed package data/FDs died before this point. A stale generation must
                // never rejoin another attempt; only this known-dead actual loan may finish.
                credit.finish()?;
            }
            self.verifier_credit.take();
        }
        Ok(())
    }
    fn check_source(&self) -> Result<()> {
        ensure!(
            Instant::now() < self.claim.deadline(),
            "native frozen Claim deadline elapsed"
        );
        self.processing.check(&self.claim)?;
        self.lease
            .as_ref()
            .context("actual native trust selection")?
            .check_permit(self.processing.source().permit())
            .map_err(source::refused_error)?;
        Ok(())
    }
    /// Recheck actual source, policy epoch, full Claim and accepted cryptographic state.
    pub fn check(&self) -> Result<()> {
        self.check_source()?;
        let state = self
            .verification
            .as_ref()
            .context("native snapshot is not verified")?
            .state;
        ensure!(
            state == SignatureState::ValidKnown
                || (matches!(self.policy, NativeSnapshotPolicy::UnsignedExplicit)
                    && state == SignatureState::Unsigned),
            "native snapshot signer trust refused"
        );
        ensure!(
            self.snapshot.is_some(),
            "native snapshot identity unavailable"
        );
        Ok(())
    }
    fn admit_raw(&self, bytes: u64) -> Result<()> {
        self.check_source()?;
        loop {
            let used = self.reads.load(Ordering::Acquire);
            let next = used
                .checked_add(bytes)
                .filter(|next| *next <= self.claim.budget().max_bytes)
                .context("native frozen aggregate byte budget exhausted")?;
            if self
                .reads
                .compare_exchange_weak(used, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
        Ok(())
    }
    /// Cryptographic facts from the actual native verifier; these do not grant other effects.
    pub fn verification(&self) -> &Verification {
        self.verification
            .as_ref()
            .expect("actual verified snapshot")
    }
    /// Observed canonical manifest identity from the same pinned archive.
    pub fn snapshot_id(&self) -> &knowledge_core::data::ContentDigest {
        self.snapshot.as_ref().expect("actual verified snapshot")
    }
    /// Activate the existing native kernel against verified STORED ranges under the same live owner.
    pub fn open_generation(
        self: Arc<Self>,
        plan: NativeFrozenReadPlan<'_>,
        work: Work,
        cancel: &AtomicBool,
        control: &Control,
    ) -> std::result::Result<StoredGeneration, knowledge_retrieval::Error> {
        self.check()
            .map_err(|_| knowledge_retrieval::Error::Inventory)?;
        let build = Self::build_identity();
        let identity = StoredIdentity::new(&build)?;
        let native = identity.requirements(plan.generation);
        StoredGeneration::open(
            self,
            &native,
            plan.scope,
            &build,
            plan.prefix,
            plan.ranges,
            work,
            plan.bounds,
            cancel,
            control,
        )
    }
}
impl ArchiveSource for NativeTrustedArchive {
    fn check(&self) -> std::result::Result<(), knowledge_retrieval::Error> {
        NativeTrustedArchive::check(self).map_err(|_| knowledge_retrieval::Error::Inventory)
    }
    fn file(&self) -> std::result::Result<&File, knowledge_retrieval::Error> {
        self.check()
            .map_err(|_| knowledge_retrieval::Error::Inventory)?;
        self.processing
            .source()
            .file()
            .map_err(|_| knowledge_retrieval::Error::Inventory)
    }
    fn admit_read(&self, bytes: u64) -> std::result::Result<(), knowledge_retrieval::Error> {
        self.check()
            .map_err(|_| knowledge_retrieval::Error::Inventory)?;
        self.admit_raw(bytes)
            .map_err(|_| knowledge_retrieval::Error::ReadBudget)
    }
}

impl NativeLibraryService {
    /// Prepare the actual complete-corpus job and persistent kernel runtime BEFORE queue submission.
    /// Startup/watch callers retain the returned job under its actual Submitted.job; there is no extra queue.
    pub fn prepare_corpus_refresh(
        &self,
        owner: &Owner,
        permit: Permit,
        inventory: NativeInventory,
        model: Option<NativeEmbeddingSelection>,
        funding: knowledge_core::admission::Holding,
    ) -> Result<NativeRefreshJob> {
        let job = self.prepare_refresh(owner, permit, inventory, model, funding)?;
        knowledge_retrieval::KernelAdapter::initialize_runtime()?;
        Ok(job)
    }
}
