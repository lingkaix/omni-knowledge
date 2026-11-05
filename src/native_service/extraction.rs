//! Funded native values keep their actual source, Claim, input leases and paid buffers together.

use super::{NativeProcessing, NativeSource};
use anyhow::Result;
use knowledge_core::admission::{Account, Holding};
use knowledge_core::jobs::{Claim, Outcome};
use knowledge_processing::admission::Admitted;
use knowledge_processing::extract::Extraction;
use std::fs::File;
use std::time::Duration;

/// Paid extraction with the original public source/Claim lifetime contract.
pub type NativeExtraction = NativeAdmitted<Extraction>;
/// Extraction failure retaining its actual native resource owner.
pub type NativeExtractionError = NativeAdmittedError<Extraction>;

/// Additional actual native sources and exclusively owned staging namespace, not another registry.
#[derive(Default)]
pub(super) struct Inputs {
    pub sources: Vec<NativeSource>,
    pub leases: Vec<File>,
    pub _scratch: Option<super::held::Scratch>,
    pub resources: Vec<Holding>,
}
impl Inputs {
    /// Check each independently admitted member, including its current namespace and read policy.
    fn check(&self) -> Result<()> {
        for source in &self.sources {
            source.check()?;
        }
        if let Some(scratch) = &self._scratch {
            scratch.check()?;
        }
        Ok(())
    }
}

impl Drop for Inputs {
    /// Destroy all actual source/FD/namespace records before finishing their detached metadata credit.
    fn drop(&mut self) {
        self.sources = Vec::new();
        self.leases = Vec::new();
        self._scratch.take();
        for holding in &mut self.resources {
            while holding.finish().is_err() {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

/// One actual paid native result; move this owner, not cloned raw buffers/FDs, between consumers.
/// Borrow through `value`, then finish only after the last read/write; no queue or executor is created.
pub struct NativeAdmitted<T> {
    output: Option<Admitted<T>>,
    // A true flag means the producer already established complete/stopped generated-write accounting.
    retained: Vec<(Holding, bool)>,
    pending: Option<anyhow::Error>,
    child_collected: bool,
    write_collected: bool,
    inputs: Inputs,
    processing: Option<NativeProcessing>,
    claim: Option<Claim>,
}
/// A failed native call still owns its actual source, Claim, typed error and unresolved capabilities.
/// `collect` makes one bounded native attempt; dropping unknown state stays in foreground shutdown.
pub struct NativeAdmittedError<T> {
    cause: anyhow::Error,
    owner: Box<NativeAdmitted<T>>,
}
impl<T> std::fmt::Debug for NativeAdmittedError<T> {
    /// Report classification without source paths, content, or child arguments.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeAdmittedError")
    }
}
impl<T> std::fmt::Display for NativeAdmittedError<T> {
    /// Pending resource ownership is not successful processing or job settlement.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "native processing failed; actual resource collection must settle before shutdown",
        )
    }
}
impl<T> std::error::Error for NativeAdmittedError<T> {
    /// Preserve the real typed failure while all native leases and Claim credit stay owned.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause().as_ref())
    }
}
impl<T> NativeAdmittedError<T> {
    /// Inspect the original typed error while this carrier retains all native ownership.
    pub fn cause(&self) -> &anyhow::Error {
        self.owner.pending.as_ref().unwrap_or(&self.cause)
    }
    /// Recover the actual source and Claim after collection, including a mismatched dispatch.
    /// This makes no queue settlement; return the Claim to its existing rightful owner.
    pub fn into_parts(self) -> (NativeProcessing, Claim, anyhow::Error) {
        let Self { cause, mut owner } = self;
        while owner.collect().is_err() {
            std::thread::sleep(Duration::from_millis(250));
        }
        let processing = owner.processing.take().expect("native processing owner");
        let claim = owner.claim.take().expect("native claim owner");
        let cause = owner.pending.take().unwrap_or(cause);
        (processing, claim, cause)
    }
    /// Inspect the real retained staging descriptor for native ownership recovery only, not disclosure authority.
    pub fn staging_directory(&self) -> Option<&File> {
        self.owner
            .inputs
            ._scratch
            .as_ref()
            .map(|scratch| &scratch.directory)
    }
    /// Retry actual bounded native collection, never marking the failed job completed.
    pub fn collect(&mut self) -> Result<()> {
        self.owner.collect()
    }
}
impl NativeProcessing {
    /// Call an existing funded producer while this SAME owner retains every input and native lease.
    pub(super) fn run<T>(
        self,
        claim: Claim,
        inputs: Inputs,
        work: impl FnOnce(&NativeProcessing, &mut Claim, &mut Inputs) -> Result<Admitted<T>>,
    ) -> std::result::Result<NativeAdmitted<T>, NativeAdmittedError<T>> {
        let mut owner = NativeAdmitted {
            output: None,
            retained: Vec::new(),
            pending: None,
            child_collected: false,
            write_collected: false,
            inputs,
            processing: Some(self),
            claim: Some(claim),
        };
        let result = (|| -> Result<()> {
            let processing = owner.processing.as_ref().expect("native processing owner");
            let claim = owner.claim.as_mut().expect("native claim owner");
            processing.check(claim)?;
            owner.inputs.check()?;
            owner.output = Some(work(processing, claim, &mut owner.inputs)?);
            owner.inputs.check()?;
            processing.check(claim)
        })();
        match result {
            Ok(()) => Ok(owner),
            Err(cause) => {
                owner.pending = Some(cause);
                Err(NativeAdmittedError {
                    cause: anyhow::anyhow!("native processing did not complete"),
                    owner: Box::new(owner),
                })
            }
        }
    }
    /// Extract the exact source using this actual scheduler Claim and its complete prepaid peak.
    /// None uses only THIS Claim's remaining credit after M3's complete input/output/capture buffers.
    /// All errors retain native ownership; no path reopen, alternate account or numeric grant is used.
    pub fn extract(
        self,
        claim: Claim,
        max_bytes: usize,
        child_rss_bytes: Option<u64>,
    ) -> std::result::Result<NativeExtraction, NativeExtractionError> {
        self.run(claim, Inputs::default(), |processing, claim, _| {
            knowledge_processing::extract::extract_file_funded(
                processing.source().file()?,
                processing.source().hint(),
                max_bytes,
                child_rss_bytes,
                &mut processing.funding(claim)?,
            )
        })
    }
}
/// Actual M3-produced text with source/Claim/processor binding retained through index consumption.
/// Identity fields are facts, not grants; this is NOT a provider, tuple publisher or new processor.
pub struct NativeIndexInput {
    representation: knowledge_search::RepresentationIdentity,
    source_id: String,
    source_digest: String,
    owner: NativeExtraction,
}
impl NativeIndexInput {
    /// Borrow the actual paid extraction; direct/extracted source coordinate semantics remain M3-owned.
    pub fn text(&self) -> Result<&str> {
        Ok(&self.owner.value()?.text)
    }
    /// Original source identity is the actual native processing key, not equal-content deduplication.
    pub fn source_id(&self) -> Result<&str> {
        self.owner.source()?;
        Ok(&self.source_id)
    }
    /// Complete original bytes were hashed from the SAME actual selected FD.
    pub fn source_digest(&self) -> Result<&str> {
        self.owner.source()?;
        Ok(&self.source_digest)
    }
    /// Digest of the real representation and effective consuming M3/native processor fingerprint.
    pub fn representation(&self) -> Result<&knowledge_search::RepresentationIdentity> {
        self.owner.source()?;
        Ok(&self.representation)
    }
    /// Retain/check the actual source through later controlled reads and delivery.
    pub fn source(&self) -> Result<&NativeSource> {
        self.owner.source()
    }
    /// Finish only after consumer buffers/artifacts and their split work have settled.
    /// Destroy these metadata allocations BEFORE their actual M3/native owner releases credit.
    pub fn complete(self) -> std::result::Result<NativeSource, NativeExtractionError> {
        let Self {
            representation,
            source_id,
            source_digest,
            owner,
        } = self;
        drop(representation);
        drop(source_id);
        drop(source_digest);
        owner.complete()
    }
}
impl NativeProcessing {
    /// Consume the existing actual Claim and M3 FD producer; reserve the metadata sibling
    /// BEFORE parsing/execution, so `None` child RSS cannot spend its retained share.
    /// The submitted complete NativeJobPlan must already include this64KiB sibling.
    pub fn extract_indexed(
        self,
        claim: Claim,
        max_bytes: usize,
        child_rss_bytes: Option<u64>,
    ) -> std::result::Result<NativeIndexInput, NativeExtractionError> {
        let mut owner = self.run(claim, Inputs::default(), |processing, claim, inputs| {
            processing
                .source()
                .indexed_extract_scope(claim, max_bytes)?;
            let metadata = Account::current()?.split_claim(
                claim,
                processing.key(),
                knowledge_core::jobs::Reservation {
                    memory_bytes: 64 * 1024,
                    temporary_bytes: 0,
                },
            )?;
            inputs.resources.push(metadata);
            knowledge_processing::extract::extract_file_funded(
                processing.source().file()?,
                processing.source().hint(),
                max_bytes,
                child_rss_bytes,
                &mut processing.funding(claim)?,
            )
        })?;
        let facts = (|| -> Result<_> {
            use sha2::{Digest, Sha256};
            let value = owner.value()?;
            anyhow::ensure!(
                matches!(value.status.as_str(), "extracted" | "partial"),
                "no actual text representation"
            );
            let mut hash = Sha256::new();
            for chunk in value.text.as_bytes().chunks(64 * 1024) {
                owner.source()?;
                hash.update(chunk);
            }
            owner.source()?;
            let key = owner
                .processing
                .as_ref()
                .expect("native processing owner")
                .key();
            Ok((
                knowledge_search::RepresentationIdentity {
                    digest: hex::encode(hash.finalize()),
                    processor: key.processor.as_str().to_owned(),
                    modality: knowledge_search::VectorModality::Text,
                },
                key.source.to_owned(),
                key.version.as_str().to_owned(),
            ))
        })();
        match facts {
            Ok((representation, source_id, source_digest)) => Ok(NativeIndexInput {
                representation,
                source_id,
                source_digest,
                owner,
            }),
            Err(cause) => {
                owner.pending = Some(cause);
                Err(NativeAdmittedError {
                    cause: anyhow::anyhow!("native index representation refused"),
                    owner: Box::new(owner),
                })
            }
        }
    }
}

impl<T> NativeAdmitted<T> {
    /// Borrow the SAME checked source for prepaid response construction and controlled output boundaries.
    pub fn source(&self) -> Result<&NativeSource> {
        let processing = self.processing.as_ref().expect("native processing owner");
        processing.check(self.claim.as_ref().expect("native claim owner"))?;
        self.inputs.check()?;
        Ok(processing.source())
    }
    /// Recheck every current native source/policy and Claim before exposing the borrowed paid value.
    pub fn value(&self) -> Result<&T> {
        self.source()?;
        Ok(&self.output.as_ref().expect("paid native output").value)
    }
    /// Destroy every paid field/FD before finishing its costs and completing the actual Claim.
    /// Returns the SAME source for delivery; staging outputs grant no user-visible publication authority.
    pub fn complete(mut self) -> std::result::Result<NativeSource, NativeAdmittedError<T>> {
        let result = (|| -> Result<()> {
            self.value()?;
            self.collect()?;
            let processing = self.processing.as_ref().expect("native processing owner");
            let claim = self.claim.as_ref().expect("native claim owner");
            self.inputs.check()?;
            processing.check(claim)?;
            Account::current()?
                .queue()
                .settle(claim, processing.key(), Outcome::Completed)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.claim.take();
                Ok(self
                    .processing
                    .take()
                    .expect("native processing owner")
                    .into_source())
            }
            Err(cause) => Err(NativeAdmittedError {
                cause,
                owner: Box::new(self),
            }),
        }
    }
    /// Collect real unknown work, destroy complete/stopped output records, then finish only dead costs.
    /// Unknown generated-write deltas remain charged; artifact names are not reconciliation receipts.
    fn collect(&mut self) -> Result<()> {
        if !self.child_collected
            && let Some(pending) = self.pending.as_mut()
            && let Some(child) =
                pending.downcast_mut::<knowledge_processing::worker::UnsettledChild>()
        {
            self.retained
                .extend(child.collect()?.into_iter().map(|holding| (holding, false)));
            self.child_collected = true;
        }
        if let Some(pending) = self.pending.as_mut()
            && let Some(write) =
                pending.downcast_mut::<knowledge_processing::funding::UnsettledWrite>()
        {
            if !self.write_collected {
                write.retry()?;
                self.write_collected = true;
            }
            let retained = write.take_retained()?;
            // Destroy the typed owner's actual file/lease/name records before their credit finishes.
            self.pending.take();
            self.retained
                .extend(retained.into_iter().map(|holding| (holding, true)));
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|error| error.is::<knowledge_processing::held::PartialOutputs>())
        {
            let partial = self
                .pending
                .take()
                .expect("typed partial output")
                .downcast::<knowledge_processing::held::PartialOutputs>()
                .expect("typed partial output");
            let knowledge_processing::held::PartialOutputs {
                cause,
                directory,
                artifacts,
                sources,
                retained,
            } = partial;
            drop(artifacts);
            drop(sources);
            drop(directory);
            self.retained
                .extend(retained.into_iter().map(|holding| (holding, true)));
            self.pending = Some(cause);
        }
        if let Some(scratch) = &self.inputs._scratch {
            scratch.check()?;
        }
        if let Some(Admitted { value, retained }) = self.output.take() {
            drop(value);
            self.retained
                .extend(retained.into_iter().map(|holding| (holding, true)));
        }
        while let Some((holding, writes_known)) = self.retained.last_mut() {
            if !*writes_known && holding.remaining()?.temporary_bytes != 0 {
                anyhow::bail!("native generated-write delta remains unresolved");
            }
            holding.finish()?;
            self.retained.pop();
        }
        Ok(())
    }
}
impl<T> Drop for NativeAdmitted<T> {
    /// Abandonment/cancellation settles only after actual collection and last-reader destruction.
    /// The existing queue keeps this Claim visible while bounded collection remains pending.
    fn drop(&mut self) {
        while self.collect().is_err() {
            std::thread::sleep(Duration::from_millis(250));
        }
        if let (Some(processing), Some(claim)) = (&self.processing, &self.claim)
            && let Ok(account) = Account::current()
        {
            let outcome = match claim.stop_requested(processing.key()) {
                Ok(Some(knowledge_core::jobs::Cancellation::Superseded)) => Outcome::Superseded,
                Ok(Some(_)) => Outcome::Cancelled,
                _ => Outcome::Failed,
            };
            while account
                .queue()
                .settle(claim, processing.key(), outcome)
                .is_err()
            {
                // Recover a mismatched Claim with into_parts; never discard or settle it under another key.
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}
