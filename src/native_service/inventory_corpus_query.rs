//! ONE actual Query invocation over selected complete tuples; no child source jobs/new scheduler.
use super::*;
#[path = "inventory_corpus_partition.rs"]
mod partition;
use knowledge_core::jobs::{Key, Submission, Submitted};
pub use partition::{
    NativePartitionResult, NativeQueryLoans, NativeQueryPartition, NativeQueryStage,
    NativeQueryStageRefusal, NativeQueryStageResources,
};
use std::collections::BTreeSet;
/// Actual selected tuple/M1/model owners for one labelled library; no manifest or ID reconstruction.
pub struct NativeQueryLibrary {
    reader: NativeTupleRead,
    inventory: NativeInventory,
    model: Option<NativeEmbeddingSelection>,
}
impl NativeQueryLibrary {
    /// Move already paid actual owners, not source-count-sized snapshots or permission DTOs.
    pub fn new(
        reader: NativeTupleRead,
        inventory: NativeInventory,
        model: Option<NativeEmbeddingSelection>,
    ) -> Self {
        Self {
            reader,
            inventory,
            model,
        }
    }
}
pub struct NativeTupleQuery {
    cause: Arc<QueryCause>,
}
struct QueryCause {
    descriptor: Vec<u8>,
    context: String,
    version: knowledge_core::data::ContentDigest,
    recipe: knowledge_core::data::ContentDigest,
    processor: knowledge_core::data::ContentDigest,
    libraries: Vec<NativeQueryLibrary>,
    request_budget: Budget,
    deadline: Instant,
    _funding: Work,
}
impl QueryCause {
    fn key(&self) -> Key<'_> {
        Key {
            context: &self.context,
            library: "native-query-selected-libraries",
            mount: None,
            source: "native-complete-tuple-query-descriptor",
            version: &self.version,
            recipe: &self.recipe,
            processor: &self.processor,
        }
    }
    fn check(&self) -> Result<()> {
        for library in &self.libraries {
            library.reader.check()?;
            library
                .inventory
                .owner
                .policy
                .0
                .lease
                .check()
                .map_err(refused_error)?;
            if let Some(model) = &library.model {
                model.check()?;
            }
        }
        Ok(())
    }
    fn check_claim(&self, claim: &Claim) -> Result<()> {
        self.check()?;
        self.check_scope_claim(claim)
    }
    // Called INSIDE the actual funded source Permit's live native check. That Permit already
    // checks the actual SDK selections/configuration/path; its opaque TuplePart/M1 callbacks
    // check current metadata/root/original FD dependencies. Do not recursively repeat all of
    // those SAME checks here. No permission/epoch TTL or boolean grant cache is introduced.
    fn check_scope_claim(&self, claim: &Claim) -> Result<()> {
        let b = &self.request_budget;
        let a = claim.budget();
        anyhow::ensure!(
            a.max_files <= b.max_files
                && a.max_bytes <= b.max_bytes
                && a.max_millis <= b.max_millis
                && a.max_memory_bytes <= b.max_memory_bytes,
            "actual Query claim exceeds sealed request budget"
        );
        anyhow::ensure!(
            Instant::now() < self.deadline
                && Instant::now() < claim.deadline()
                && claim.stop_requested(self.key())?.is_none(),
            "actual aggregate Query claim stopped"
        );
        for scope in &self.libraries {
            if let Some(model) = &scope.model {
                model.check()?;
            }
        }
        Ok(())
    }
}
/// One actual M1/source/Query-scope loan. Keep it through its final component/read/delivery owner.
pub struct NativeQuerySource {
    source: NativeSource,
    part: SourcePart,
    cause: Arc<QueryCause>,
    library: usize,
    claim_owner: std::sync::Weak<Claim>,
    next: u64,
}
impl NativeQuerySource {
    /// Actual current original FD/scope, never a display path or naked cached version grant.
    pub fn source(&self) -> &NativeSource {
        &self.source
    }
    /// Exact next source-table cursor, DATA rather than source permission or completeness.
    pub fn next(&self) -> u64 {
        self.next
    }
    /// Actual labelled input slot, not a grant to another library.
    pub fn library(&self) -> usize {
        self.library
    }
}
#[derive(Serialize)]
struct QueryDescriptor<'a> {
    format: &'static str,
    command: &'a wire::Command,
    roots: Vec<(&'a str, (u64, u64), &'a str, &'a str)>,
    contexts: Vec<String>,
    models: Vec<Option<&'a knowledge_search::ModelIdentity>>,
}
impl NativeTupleQuery {
    /// Bounded command/context/descriptor/key copies only; tuple/M1/model owners are separately paid.
    pub fn metadata_requirement() -> Reservation {
        Reservation {
            memory_bytes: 4 * 1024 * 1024,
            temporary_bytes: 0,
        }
    }
    /// All libraries must come from the SAME opaque SDK invocation, not byte-equal requests/sessions.
    /// Actual context/FD/policy/model owners remain retained independently of descriptor hashes.
    pub fn prepare(libraries: Vec<NativeQueryLibrary>, funding: Work) -> Result<Self> {
        anyhow::ensure!(
            funding.remaining()?.memory_bytes >= Self::metadata_requirement().memory_bytes,
            "complete Query descriptor envelope is not prepaid"
        );
        struct Inputs {
            libraries: Vec<NativeQueryLibrary>,
            funding: Work,
        }
        let inputs = Inputs { libraries, funding };
        anyhow::ensure!(
            !inputs.libraries.is_empty(),
            "actual Query libraries required"
        );
        let base = &inputs.libraries[0].reader.permit;
        let command = base.admitted_command().map_err(refused_error)?;
        let wire::Command::Query { query } = command else {
            bail!("actual Query required")
        };
        let semantic = query.methods.contains(&wire::SearchMethod::Semantic);
        let mut seen = BTreeSet::new();
        let mut roots = Vec::new();
        let mut contexts = Vec::new();
        let mut models = Vec::new();
        for scope in &inputs.libraries {
            scope.reader.check()?;
            let physical = &scope.reader.physical;
            anyhow::ensure!(
                std::ptr::eq(
                    command,
                    scope
                        .reader
                        .permit
                        .admitted_command()
                        .map_err(refused_error)?
                ),
                "Query tuple owners came from different actual SDK requests"
            );
            anyhow::ensure!(
                seen.insert(physical.library.as_str()),
                "duplicate Query library owner"
            );
            anyhow::ensure!(
                scope.inventory.owner.root == physical.data.original_root,
                "foreign Query M1 inventory root"
            );
            scope
                .inventory
                .owner
                .policy
                .0
                .lease
                .check_permit(base)
                .map_err(refused_error)?;
            if semantic {
                let selected = scope
                    .model
                    .as_ref()
                    .context("selected semantic model owner required")?;
                selected.check_permit(base)?;
                anyhow::ensure!(
                    physical.data.model.as_ref() == Some(selected.model()?),
                    "selected semantic method dependency differs from complete tuple"
                );
            }
            roots.push((
                physical.library.as_str(),
                physical.data.original_root,
                physical.pointer.header_sha256.as_str(),
                scope.inventory.scan(),
            ));
            contexts.push(selected_context(&physical.library, base)?);
            models.push(scope.model.as_ref().map(|m| m.model()).transpose()?);
        }
        anyhow::ensure!(
            seen.len() == query.library_ids.len()
                && query
                    .library_ids
                    .iter()
                    .all(|id| seen.contains(id.as_str())),
            "actual Query selected-library set is incomplete"
        );
        let context = super::super::hash_json(&contexts)?.as_str().to_owned();
        let recipe = super::super::hash_json(command)?;
        let processor = knowledge_core::data::ContentDigest::new(admitted::build_identity())?;
        let descriptor = serde_json::to_vec(&QueryDescriptor {
            format: "native-complete-tuple-query-v1",
            command,
            roots,
            contexts,
            models,
        })?;
        let version =
            knowledge_core::data::ContentDigest::new(hex::encode(Sha256::digest(&descriptor)))?;
        Ok(Self {
            cause: Arc::new(QueryCause {
                descriptor,
                context,
                version,
                recipe,
                processor,
                request_budget: Budget {
                    max_files: query.budget.max_files.get(),
                    max_bytes: query.budget.max_bytes.get(),
                    max_millis: query.budget.max_millis.get(),
                    max_memory_bytes: query.budget.max_memory_bytes.get(),
                },
                deadline: inputs
                    .libraries
                    .iter()
                    .map(|scope| scope.reader.deadline)
                    .min()
                    .expect("actual nonempty Query libraries"),
                libraries: inputs.libraries,
                _funding: inputs.funding,
            }),
        })
    }
    /// Actual retained invocation description, not permission to reopen any tuple/source.
    pub fn descriptor(&self) -> Result<&[u8]> {
        self.cause.check()?;
        Ok(&self.cause.descriptor)
    }
    pub fn key(&self) -> Key<'_> {
        self.cause.key()
    }
    /// Existing sole Core scheduler; no nested per-library/per-source Query jobs.
    pub fn submit(&self, plan: NativeJobPlan<'_>) -> Result<Submitted> {
        self.cause.check()?;
        let wire::Command::Query { query } = self.cause.libraries[0]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        let b = &query.budget;
        anyhow::ensure!(
            plan.budget.max_files <= b.max_files.get()
                && plan.budget.max_bytes <= b.max_bytes.get()
                && plan.budget.max_millis <= b.max_millis.get()
                && plan.budget.max_memory_bytes <= b.max_memory_bytes.get(),
            "Query job exceeds actual admitted request budget"
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
    /// Reuse existing paid M1 page credits; no resident corpus/source FD collection.
    pub fn page(
        &self,
        claim: &Claim,
        library: usize,
        offset: u64,
        budget: Budget,
        funding: Holding,
    ) -> Result<NativeInventoryPage> {
        let funding = InventoryCredit(Some(funding));
        self.cause.check_claim(claim)?;
        let scope = self
            .cause
            .libraries
            .get(library)
            .context("actual Query library index")?;
        let b = claim.budget();
        anyhow::ensure!(
            budget.max_files <= b.max_files
                && budget.max_bytes <= b.max_bytes
                && budget.max_millis <= b.max_millis
                && budget.max_memory_bytes <= b.max_memory_bytes,
            "Query page exceeds actual claim budget"
        );
        scope.inventory.page(offset, budget, funding.into_holding())
    }
    /// Fresh funded SAME-Claim SDK scope precedes metadata copies and actual source FD inspection.
    pub fn source(
        &self,
        service: &NativeLibraryService,
        claim: &Arc<Claim>,
        library: usize,
        observed: NativeObservedSource,
        offset: u64,
    ) -> Result<Option<NativeQuerySource>> {
        self.cause.check_claim(claim)?;
        let selected = self
            .cause
            .libraries
            .get(library)
            .context("actual Query library index")?;
        anyhow::ensure!(
            Arc::ptr_eq(&observed.page.inventory, &selected.inventory.owner),
            "foreign Query inventory owner"
        );
        let bytes = (observed.observation().stat.path.as_str().len() as u64)
            .checked_mul(32)
            .and_then(|n| n.checked_add(4 * FRAME as u64 + 1024 * 1024))
            .context("Query source scope envelope overflow")?;
        let scope = selected
            .reader
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
        let checked = self.cause.clone();
        let live = claim.clone();
        let result = (|| -> Result<Option<NativeQuerySource>> {
            scope
                .retain_check(Arc::new(move || checked.check_scope_claim(&live).is_ok()))
                .map_err(refused_error)?;
            let Some((source, part, next)) = selected
                .reader
                .checked_part_at(service, &scope, observed, offset)?
            else {
                return Ok(None);
            };
            Ok(Some(NativeQuerySource {
                source,
                part,
                cause: self.cause.clone(),
                library,
                claim_owner: Arc::downgrade(claim),
                next,
            }))
        })();
        match result {
            Ok(Some(source)) => Ok(Some(source)),
            Ok(None) => {
                scope
                    .rejoin_processing_scope(claim, self.key())
                    .map_err(|scope| {
                        anyhow::Error::new(super::super::NativeRefreshScopeError {
                            scope,
                            cause: anyhow::anyhow!("Query source EOF cleanup refused"),
                        })
                    })?;
                Ok(None)
            }
            Err(cause) => match scope.rejoin_processing_scope(claim, self.key()) {
                Ok(()) => Err(cause),
                Err(scope) => Err(anyhow::Error::new(super::super::NativeRefreshScopeError {
                    scope,
                    cause,
                })),
            },
        }
    }
    /// Destroy data/FD consumers before exact affine SDK scope rejoin; live clones return owner.
    pub fn finish_source(
        &self,
        source: NativeQuerySource,
        claim: &Claim,
    ) -> std::result::Result<(), wire::authority::ScopeRejoinError> {
        let NativeQuerySource {
            source,
            part,
            cause,
            library: _,
            claim_owner,
            next: _,
        } = source;
        let matches = Arc::ptr_eq(&cause, &self.cause)
            && claim_owner
                .upgrade()
                .is_some_and(|owner| std::ptr::eq(owner.as_ref(), claim));
        drop((part, cause));
        let scope = source.into_query_scope();
        if !matches {
            return Err(wire::authority::ScopeRejoinError::Retained {
                scope: Box::new(scope),
                code: wire::ErrorCode::InvalidRequest,
            });
        }
        scope.rejoin_processing_scope(claim, self.key())
    }
}
