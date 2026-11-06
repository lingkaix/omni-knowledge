//! ONE actual Refresh cause over an owned M1 inventory; source digests are separate facts.
#[path = "inventory_corpus.rs"]
mod corpus;
use super::*;
use crate::native_service::source::{refused_error, selected_context};
pub use corpus::{
    NativeBm25Run, NativeCorpusCoverage, NativeCorpusDraft, NativeCorpusError,
    NativeCorpusProgress, NativePartitionResult, NativeQueryLibrary, NativeQueryLoans,
    NativeQueryPartition, NativeQueryPartitionPage, NativeQuerySource, NativeQueryStage,
    NativeQueryStageRefusal, NativeQueryStageResources, NativeQueryStep, NativeTupleQuery,
    NativeTupleRead,
};
use knowledge_core::admission::Account;
use knowledge_core::jobs::{Claim, Key, Submission, Submitted};

/// Failed synchronous source setup whose actual SDK cleanup refusal still owns its scope.
/// No child/processor was started; callers may retain/retry the actual returned owner.
pub struct NativeRefreshScopeError {
    scope: wire::authority::ScopeRejoinError,
    cause: anyhow::Error,
}
impl NativeRefreshScopeError {
    /// Recover the actual owning SDK refusal and original setup error, never a numeric grant.
    pub fn into_parts(self) -> (wire::authority::ScopeRejoinError, anyhow::Error) {
        (self.scope, self.cause)
    }
}
impl std::fmt::Debug for NativeRefreshScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeRefreshScopeError")
    }
}
impl std::fmt::Display for NativeRefreshScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native funded source setup and owning scope cleanup refused")
    }
}
impl std::error::Error for NativeRefreshScopeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Opaque actual Refresh input and its existing Core queue submission.
/// Keep this owner through queueing, all source stages and final tuple settlement.
/// Descriptor hashes/labels are data; actual SDK/M1/model capabilities remain retained separately.
pub struct NativeRefreshJob {
    permit: Permit,
    cause: Arc<RefreshCause>,
}
pub(in crate::native_service) struct RefreshCause {
    descriptor: Vec<u8>,
    context: String,
    library: wire::Id,
    version: knowledge_core::data::ContentDigest,
    recipe: knowledge_core::data::ContentDigest,
    processor: knowledge_core::data::ContentDigest,
    inventory: NativeInventory,
    model: Option<NativeEmbeddingSelection>,
    // Actual descriptor/context/model fields die before known-settled metadata credit.
    _funding: InventoryCredit,
}
impl RefreshCause {
    pub(in crate::native_service) fn key(&self) -> Key<'_> {
        Key {
            context: &self.context,
            library: self.library.as_str(),
            mount: None,
            source: "native-M1-inventory-descriptor",
            version: &self.version,
            recipe: &self.recipe,
            processor: &self.processor,
        }
    }
    pub(in crate::native_service) fn check(&self) -> Result<()> {
        self.inventory
            .owner
            .policy
            .0
            .lease
            .check()
            .map_err(refused_error)?;
        if let Some(model) = &self.model {
            model.check()?;
        }
        Ok(())
    }
}
fn hash_json(value: &impl serde::Serialize) -> Result<knowledge_core::data::ContentDigest> {
    struct Sink(Sha256);
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Sink(Sha256::new());
    serde_json::to_writer(&mut sink, value)?;
    knowledge_core::data::ContentDigest::new(hex::encode(sink.0.finalize())).map_err(Into::into)
}
impl NativeRefreshJob {
    /// Protocol-bounded metadata ONLY: native command <=1MiB, model manifest <=64KiB,
    /// escaped descriptor/temporary encoder/key fields. No source-count or corpus-byte limit.
    pub fn metadata_requirement() -> Reservation {
        Reservation {
            memory_bytes: 4 * 1024 * 1024,
            temporary_bytes: 0,
        }
    }
    /// Actual descriptor bytes are retained, not a hash of an original FD followed by reopen.
    pub fn descriptor(&self) -> Result<&[u8]> {
        self.cause.check()?;
        Ok(&self.cause.descriptor)
    }
    /// Complete actual aggregate input key, NOT any individual source's UUID or content digest.
    pub fn key(&self) -> Key<'_> {
        self.cause.key()
    }
    /// Submit to the existing sole scheduler. No per-source child jobs or second queue exist.
    pub fn submit(&self, plan: NativeJobPlan<'_>) -> Result<Submitted> {
        self.cause.check()?;
        self.permit.check().map_err(refused_error)?;
        let wire::Command::Refresh { budget, .. } =
            self.permit.admitted_command().map_err(refused_error)?
        else {
            bail!("actual Refresh required")
        };
        anyhow::ensure!(
            plan.budget.max_files <= budget.max_files.get()
                && plan.budget.max_bytes <= budget.max_bytes.get()
                && plan.budget.max_millis <= budget.max_millis.get()
                && plan.budget.max_memory_bytes <= budget.max_memory_bytes.get(),
            "Refresh job exceeds actual admitted budget"
        );
        Ok(Account::current()?.submit(Submission {
            key: self.key(),
            operation_id: plan.operation_id,
            job_id: plan.job_id,
            budget: plan.budget,
            reservation: plan.reservation,
            priority: plan.priority,
            max_attempts: plan.max_attempts,
        })?)
    }
    /// Stream existing paid M1 pages without accumulating a source table or source FDs.
    pub fn page(
        &self,
        offset: u64,
        budget: Budget,
        funding: Holding,
    ) -> Result<NativeInventoryPage> {
        self.cause.check()?;
        self.cause.inventory.page(offset, budget, funding)
    }
    /// Destroy source/key/FD owners before returning its actual affine SDK scope loan.
    /// A retained scope refusal remains owned; it is never replaced by a global refund/reserve.
    pub fn finish_source(
        &self,
        processing: NativeProcessing,
        claim: &Claim,
    ) -> std::result::Result<(), wire::authority::ScopeRejoinError> {
        let (scope, matches) = processing.into_refresh_scope(&self.cause);
        if !matches {
            return Err(wire::authority::ScopeRejoinError::Retained {
                scope: Box::new(scope),
                code: wire::ErrorCode::InvalidRequest,
            });
        }
        scope.rejoin_processing_scope(claim, self.key())
    }
    /// Bind one real source from THIS actual inventory under the SAME aggregate Claim.
    /// Metadata is split before source FD inspection/hash. This never submits another job.
    pub fn processing(
        &self,
        claim: &Claim,
        observed: NativeObservedSource,
        verification: Budget,
    ) -> Result<NativeProcessing> {
        self.cause.check()?;
        anyhow::ensure!(
            claim.stop_requested(self.key())?.is_none(),
            "Refresh stopped"
        );
        anyhow::ensure!(
            Arc::ptr_eq(&observed.page.inventory, &self.cause.inventory.owner),
            "foreign native inventory owner"
        );
        let actual = claim.budget();
        let wire::Command::Refresh { budget, .. } =
            self.permit.admitted_command().map_err(refused_error)?
        else {
            bail!("actual Refresh required")
        };
        anyhow::ensure!(
            actual.max_files <= budget.max_files.get()
                && actual.max_bytes <= budget.max_bytes.get()
                && actual.max_millis <= budget.max_millis.get()
                && actual.max_memory_bytes <= budget.max_memory_bytes.get()
                && verification.max_files <= actual.max_files
                && verification.max_bytes <= actual.max_bytes
                && verification.max_millis <= actual.max_millis
                && verification.max_memory_bytes <= actual.max_memory_bytes,
            "source verification exceeds actual Refresh claim/request budget"
        );
        let bytes = (observed.observation().stat.path.as_str().len() as u64)
            .checked_mul(32)
            .and_then(|n| n.checked_add(1024 * 1024))
            .context("source metadata envelope overflow")?;
        let scope = self
            .permit
            .fork_processing_scope(
                claim,
                self.key(),
                Reservation {
                    memory_bytes: bytes,
                    temporary_bytes: 0,
                },
            )
            .map_err(refused_error)?;
        let result = (|| {
            let path = wire::RelativePath::new(observed.observation().stat.path.as_str())?;
            observed.check_permit(&scope)?;
            let source = NativeSource::open_claim(
                &self.cause.library,
                &path,
                &scope,
                verification,
                claim,
                self.key(),
            )?
            .bind_observed(observed)?;
            if let Some(model) = &self.cause.model {
                model.check_source(&source)?;
            }
            let mut processing =
                source.processing(self.cause.recipe.clone(), self.cause.processor.clone())?;
            processing.bind_refresh_cause(self.cause.clone());
            Ok(processing)
        })();
        match result {
            Ok(processing) => Ok(processing),
            Err(error) => {
                // No child/producer has started; synchronous source-construction locals died.
                // Preserve an actual owning SDK refusal instead of dropping a retained scope.
                if let Err(scope) = scope.rejoin_processing_scope(claim, self.key()) {
                    return Err(anyhow::Error::new(NativeRefreshScopeError {
                        scope,
                        cause: error,
                    }));
                }
                Err(error)
            }
        }
    }
}
impl NativeLibraryService {
    /// Capture the actual sealed Refresh, SAME selected M1 inventory and optional actual model.
    /// Complete incoming metadata funding is required before native inspection or descriptor copies.
    pub fn prepare_refresh(
        &self,
        owner: &Owner,
        permit: Permit,
        inventory: NativeInventory,
        model: Option<NativeEmbeddingSelection>,
        funding: Holding,
    ) -> Result<NativeRefreshJob> {
        let funding = InventoryCredit(Some(funding));
        anyhow::ensure!(
            funding.remaining()?.memory_bytes
                >= NativeRefreshJob::metadata_requirement().memory_bytes,
            "Refresh descriptor is not fully prepaid"
        );
        owner
            .pin_selection(&self.selection)
            .map_err(refused_error)?
            .check_permit(&permit)
            .map_err(refused_error)?;
        inventory
            .owner
            .policy
            .0
            .lease
            .check_permit(&permit)
            .map_err(refused_error)?;
        let command = permit.admitted_command().map_err(refused_error)?;
        let wire::Command::Refresh {
            library_id,
            embeddings,
            ..
        } = command
        else {
            bail!("actual sealed Refresh required")
        };
        anyhow::ensure!(
            library_id == &self.library
                && inventory.owner.policy.0.write.library_id == self.library.as_str(),
            "Refresh library differs from actual inventory"
        );
        anyhow::ensure!(
            *embeddings == model.is_some(),
            "actual Refresh embedding choice requires matching actual model selection"
        );
        if let Some(model) = &model {
            model.check_permit(&permit)?;
        }
        let context = selected_context(&self.library, &permit)?;
        let receiver = model.as_ref().map(|m| hash_json(m.config()?)).transpose()?;
        let model_facts = model.as_ref().map(|m| m.model()).transpose()?;
        let descriptor = serde_json::to_vec(&(
            "native-M1-refresh-input-v1",
            &inventory.owner.scan,
            inventory.owner.root,
            &context,
            command,
            model_facts,
            receiver.as_ref().map(|d| d.as_str()),
        ))?;
        let version =
            knowledge_core::data::ContentDigest::new(hex::encode(Sha256::digest(&descriptor)))?;
        let recipe = hash_json(command)?;
        let processor = knowledge_core::data::ContentDigest::new(hex::encode(Sha256::digest(
            [
                b"native-refresh-processor-v1\0".as_slice(),
                admitted::processor_identity().as_bytes(),
                include_bytes!("inventory_job.rs"),
                include_bytes!("inventory_corpus.rs"),
                recipe.as_str().as_bytes(),
            ]
            .concat(),
        )))?;
        let cause = Arc::new(RefreshCause {
            descriptor,
            context,
            library: self.library.clone(),
            version,
            recipe,
            processor,
            inventory,
            model,
            _funding: funding,
        });
        cause.check()?;
        Ok(NativeRefreshJob { permit, cause })
    }
}
