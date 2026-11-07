//! ONE actual Query invocation over selected complete tuples; no child source jobs/new scheduler.
use super::*;
#[path = "inventory_corpus_query_input.rs"]
mod input;
pub use input::{NativeQueryInputs, NativeQueryLibraryInput};
#[path = "inventory_corpus_query_budget.rs"]
mod budget;
pub(super) use budget::{DataAmount, QueryBudgetExhausted, ReadAllowance};
#[path = "inventory_corpus_controller.rs"]
mod controller;
pub use controller::{
    NativeBm25Run, NativeMetadataRun, NativeQueryController, NativeQueryStep, NativeScanOccurrence,
    NativeScanRun,
};
#[path = "inventory_corpus_query_embedding.rs"]
mod keyword_embedding;
pub use keyword_embedding::NativeQueryEmbedding;
#[path = "inventory_corpus_partition.rs"]
mod partition;
use knowledge_core::jobs::{Key, Submission, Submitted};
pub use partition::{
    NativePartitionResult, NativeQueryLoans, NativeQueryPartition, NativeQueryPartitionPage,
    NativeQueryStage, NativeQueryStageRefusal, NativeQueryStageResources, NativeSemanticRun,
    NativeWholeSemanticRun,
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
/// Actual page allocation loan, never an owning-work-free error code.
pub enum NativeQueryPageLoan {
    /// Decoder never published a page; its actual incoming credit remains owned.
    Funding(Holding),
    /// Actual page/data/pins remain owned until its last consumer dies.
    Page(NativeInventoryPage),
}
/// A failed page factory retains its actual input credit or its actual produced page.
pub struct NativeQueryPageRefusal {
    cause: anyhow::Error,
    loan: NativeQueryPageLoan,
}
impl std::fmt::Debug for NativeQueryPageRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::fmt::Display for NativeQueryPageRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "native Query page refused while retaining its actual loan: {}",
            self.cause
        )
    }
}
impl std::error::Error for NativeQueryPageRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
impl NativeQueryPageRefusal {
    /// Return the actual owner; destroy page data before exact same-Claim rejoin.
    pub fn into_parts(self) -> (anyhow::Error, NativeQueryPageLoan) {
        (self.cause, self.loan)
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
    input_memory: u64,
    deadline: Instant,
    reads: ReadAllowance,
    // DATA baseline of separately funded pre-submission inputs, never a permission/epoch cache.
    execution_base: std::sync::OnceLock<DataAmount>,
    _funding: Work,
}
impl QueryCause {
    fn admit_io(&self, claim: &Claim, bytes: u64) -> Result<()> {
        self.check_scope_claim(claim)?;
        self.reads
            .admit(bytes, self.execution_limit(claim)?.bytes)?;
        self.check_scope_claim(claim)
    }
    fn admit_object(&self, claim: &Claim) -> Result<()> {
        self.check_scope_claim(claim)?;
        self.reads
            .admit_object(self.execution_limit(claim)?.objects)?;
        self.check_scope_claim(claim)
    }
    fn admit_planning(&self, bytes: u64) -> Result<()> {
        anyhow::ensure!(
            self.execution_base.get().is_none(),
            "query quote metadata requires pre-submission ownership"
        );
        self.reads.admit(bytes, self.request_budget.max_bytes)
    }
    fn admit_planning_object(&self) -> Result<()> {
        anyhow::ensure!(
            self.execution_base.get().is_none(),
            "query quote metadata requires pre-submission ownership"
        );
        self.reads.admit_object(self.request_budget.max_files)
    }
    fn execution_limit(&self, claim: &Claim) -> Result<DataAmount> {
        let base = *self
            .execution_base
            .get()
            .context("actual aggregate Query submission required")?;
        Ok(DataAmount {
            bytes: self
                .request_budget
                .max_bytes
                .min(base.bytes.saturating_add(claim.budget().max_bytes)),
            objects: self
                .request_budget
                .max_files
                .min(base.objects.saturating_add(claim.budget().max_files)),
        })
    }
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
        self.check_execution(claim)?;
        for scope in &self.libraries {
            if let Some(model) = &scope.model {
                model.check()?;
            }
        }
        Ok(())
    }
    // Mature CPU ticks poll the REAL execution token/budget/stop/deadline without issuing
    // duplicate native metadata IO. Full current model/SDK/member/source checks remain at
    // every actual Directory/range IO and before/after operations and result exposure.
    fn check_execution(&self, claim: &Claim) -> Result<()> {
        self.reads.check()?;
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
        Ok(())
    }
}
/// Canonical authored metadata bound to an actual current source or pointer FD scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeQueryAnnotations {
    pub source_id: wire::Id,
    pub path: wire::RelativePath,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub annotation_path: Option<wire::RelativePath>,
    pub digest: Option<wire::Digest>,
    pub digest_state: wire::DigestState,
    pub availability: wire::Availability,
}
impl NativeQueryAnnotations {
    pub fn matches_required_tags(&self, required_tags: &[String], case_sensitive: bool) -> bool {
        required_tags.iter().all(|required| {
            self.tags
                .iter()
                .any(|tag| eq_text(tag, required, case_sensitive))
        })
    }
    pub fn matches_tag_keyword(&self, keyword: &str, case_sensitive: bool) -> bool {
        keyword
            .split([',', '，'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .any(|part| {
                self.tags
                    .iter()
                    .any(|tag| eq_text(tag, part, case_sensitive))
            })
    }
    pub fn matches_file_keyword(&self, keyword: &str, case_sensitive: bool) -> bool {
        if keyword.trim().is_empty() {
            return true;
        }
        contains_text(self.path.as_str(), keyword, case_sensitive)
            || contains_text(&self.title, keyword, case_sensitive)
            || contains_text(&self.description, keyword, case_sensitive)
    }
}
pub(super) struct QueryAnnotationDraft {
    pub(super) source_id: String,
    pub(super) path: wire::RelativePath,
    pub(super) page_title: Option<String>,
    pub(super) page_descriptions: Vec<String>,
    pub(super) page_tags: Vec<String>,
    pub(super) has_frontmatter: bool,
    pub(super) pointer_title: Option<String>,
    pub(super) pointer_descriptions: Vec<String>,
    pub(super) pointer_tags: Vec<String>,
    pub(super) pointer_path: Option<wire::RelativePath>,
    pub(super) digest: Option<wire::Digest>,
    pub(super) digest_state: wire::DigestState,
    pub(super) availability: wire::Availability,
}
pub(super) fn query_annotations(draft: QueryAnnotationDraft) -> Result<NativeQueryAnnotations> {
    let mut descriptions = draft.page_descriptions;
    for part in draft.pointer_descriptions {
        if !descriptions.iter().any(|item| item == &part) {
            descriptions.push(part);
        }
    }
    let mut tags = draft.page_tags;
    for tag in draft.pointer_tags {
        if !tags.iter().any(|item| item == &tag) {
            tags.push(tag);
        }
    }
    let fallback = draft
        .path
        .as_str()
        .rsplit('/')
        .next()
        .unwrap_or(draft.path.as_str())
        .to_owned();
    let title = draft
        .pointer_title
        .filter(|title| !title.is_empty())
        .or(draft.page_title.filter(|title| !title.is_empty()))
        .unwrap_or(fallback);
    let annotation_path = if let Some(pointer) = draft.pointer_path {
        Some(pointer)
    } else if draft.path.as_str().starts_with("wiki/")
        || draft.path.as_str().starts_with("pages/")
        || draft.has_frontmatter
    {
        Some(draft.path.clone())
    } else {
        None
    };
    Ok(NativeQueryAnnotations {
        source_id: wire::Id::new(draft.source_id)?,
        path: draft.path,
        title,
        description: descriptions.join("\n"),
        tags,
        annotation_path,
        digest: draft.digest,
        digest_state: draft.digest_state,
        availability: draft.availability,
    })
}
fn eq_text(left: &str, right: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        left == right
    } else {
        left.to_lowercase() == right.to_lowercase()
    }
}
fn contains_text(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        haystack.contains(needle)
    } else {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    }
}
fn canonical_annotations(
    document: &Document,
    pointer_title: Option<String>,
    pointer_descriptions: Vec<String>,
    pointer_tags: Vec<String>,
    pointer_path: Option<wire::RelativePath>,
) -> Result<NativeQueryAnnotations> {
    let meta = &document.metadata;
    let page_descriptions = meta
        .get("frontmatter_descriptions")
        .map(|text| serde_json::from_str::<Vec<String>>(text))
        .transpose()?
        .unwrap_or_default();
    let page_tags = meta
        .get("frontmatter_tags")
        .map(|text| serde_json::from_str::<Vec<String>>(text))
        .transpose()?
        .unwrap_or_default();
    let path = wire::RelativePath::new(
        document
            .locator
            .as_deref()
            .context("actual source logical path absent")?,
    )?;
    query_annotations(QueryAnnotationDraft {
        source_id: document.key.id.clone(),
        path,
        page_title: meta.get("frontmatter_title").cloned(),
        page_descriptions,
        page_tags,
        has_frontmatter: meta.contains_key("frontmatter_page"),
        pointer_title,
        pointer_descriptions,
        pointer_tags,
        pointer_path,
        digest: Some(wire::Digest::new(document.digest.clone())?),
        digest_state: wire::DigestState::Verified,
        availability: wire::Availability::Available,
    })
}
/// One actual M1/source/Query-scope loan. Keep it through its final component/read/delivery owner.
pub struct NativeQuerySource {
    annotations: NativeQueryAnnotations,
    part: SourcePart,
    cause: Arc<QueryCause>,
    library: usize,
    claim_owner: std::sync::Weak<Claim>,
    next: u64,
    source: NativeSource,
}
impl NativeQuerySource {
    /// Actual current original FD/scope, never a display path or naked cached version grant.
    pub fn source(&self) -> &NativeSource {
        &self.source
    }
    /// Canonical title, description, and tags bound to this actual source scope.
    pub fn annotations(&self) -> &NativeQueryAnnotations {
        &self.annotations
    }
    /// Whether this source's canonical tags satisfy the admitted Query.
    pub fn matches_required_tags(&self) -> Result<bool> {
        self.source.check()?;
        let wire::Command::Query { query } = self.cause.libraries[self.library]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required");
        };
        Ok(self
            .annotations
            .matches_required_tags(&query.required_tags, query.case_sensitive))
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
        let input_memory = inputs.libraries.iter().try_fold(
            inputs.funding.remaining()?.memory_bytes,
            |n, lib| {
                let inventory = lib.inventory.owner._funding.remaining()?.memory_bytes;
                n.checked_add(lib.reader.physical.metadata_memory()?)
                    .and_then(|n| n.checked_add(inventory))
                    .ok_or_else(|| anyhow::anyhow!("actual Query input memory overflow"))
            },
        )?;
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
                base.same_input_scope(&scope.reader.permit),
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
        let spent = inputs.libraries.iter().try_fold(0u64, |n, scope| {
            n.checked_add(scope.reader.physical.input_reads.used())
                .and_then(|n| n.checked_add(scope.inventory.owner.input_bytes))
                .ok_or(QueryBudgetExhausted)
        })?;
        let objects = inputs.libraries.iter().try_fold(0u64, |n, scope| {
            n.checked_add(scope.inventory.owner.input_objects)
                .and_then(|n| n.checked_add(scope.reader.physical.input_reads.amount().objects))
                .ok_or(QueryBudgetExhausted)
        })?;
        anyhow::ensure!(
            spent <= query.budget.max_bytes.get() && objects <= query.budget.max_files.get(),
            QueryBudgetExhausted
        );
        Ok(Self {
            cause: Arc::new(QueryCause {
                descriptor,
                context,
                version,
                recipe,
                processor,
                input_memory,
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
                reads: ReadAllowance::from_amount(DataAmount {
                    bytes: spent,
                    objects,
                }),
                execution_base: std::sync::OnceLock::new(),
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
        let input_memory = self
            .cause
            .input_memory
            .checked_add(
                self.cause.libraries[0]
                    .reader
                    .permit
                    .retained_input_memory()
                    .map_err(refused_error)?,
            )
            .context("actual SDK/input memory overflow")?;
        anyhow::ensure!(
            plan.budget.max_files <= b.max_files.get()
                && plan.budget.max_bytes <= b.max_bytes.get()
                && plan.budget.max_millis <= b.max_millis.get()
                && plan
                    .budget
                    .max_memory_bytes
                    .checked_add(input_memory)
                    .is_some_and(|n| n <= b.max_memory_bytes.get())
                && plan
                    .reservation
                    .memory_bytes
                    .checked_add(input_memory)
                    .is_some_and(|n| n <= b.max_memory_bytes.get()),
            "Query job exceeds actual admitted request budget"
        );
        self.cause
            .execution_base
            .get_or_init(|| self.cause.reads.amount());
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
        let mut funding = Some(funding);
        let mut retained: Option<NativeInventoryPage> = None;
        let result = (|| -> Result<NativeInventoryPage> {
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
            self.cause
                .reads
                .with_remaining(self.cause.execution_limit(claim)?, |remaining| {
                    self.cause.check_scope_claim(claim)?;
                    match scope.inventory.page_owning(
                        offset,
                        Budget {
                            max_bytes: budget.max_bytes.min(remaining.bytes),
                            max_files: budget.max_files.min(remaining.objects),
                            ..budget
                        },
                        funding.take().expect("actual incoming page credit"),
                    ) {
                        Ok(page) => retained = Some(page),
                        Err((cause, credit)) => {
                            funding = Some(credit);
                            return Err(cause);
                        }
                    }
                    let page = retained.as_ref().expect("actual decoder page");
                    if page.issue() == Some(&knowledge_storage::Error::Budget)
                        || (!page.complete() && page.next_offset() == offset)
                    {
                        return Err(QueryBudgetExhausted.into());
                    }
                    let spent = DataAmount {
                        bytes: page.read_bytes(),
                        objects: page.read_objects(),
                    };
                    self.cause.check_scope_claim(claim)?;
                    Ok((retained.take().expect("actual successful page"), spent))
                })
        })();
        result.map_err(|cause| {
            let budget = cause.is::<QueryBudgetExhausted>()
                || cause.downcast_ref::<knowledge_storage::Error>()
                    == Some(&knowledge_storage::Error::Budget);
            let loan = match retained {
                Some(page) => NativeQueryPageLoan::Page(page),
                None => NativeQueryPageLoan::Funding(funding.expect("actual refused page credit")),
            };
            let error = anyhow::Error::new(NativeQueryPageRefusal { cause, loan });
            if budget {
                error.context(QueryBudgetExhausted)
            } else {
                error
            }
        })
    }
    /// Already prepaid tuple metadata supplies the exact framed quote. A fresh SAME-Claim SDK
    /// scope precedes retained metadata copies and actual original-source FD inspection.
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
        self.cause.admit_object(claim)?;
        let Some((part, next)) = selected
            .reader
            .part_admitted(offset, &|n| self.cause.admit_io(claim, n))?
        else {
            return Ok(None);
        };
        self.source_part(service, claim, library, observed, part, next)
            .map(Some)
    }
    // Reuse the SAME actual decoded FD/receipt, never cached permission or a path/hash reopen.
    fn source_part(
        &self,
        service: &NativeLibraryService,
        claim: &Arc<Claim>,
        library: usize,
        observed: NativeObservedSource,
        part: super::reader::TuplePart,
        next: u64,
    ) -> Result<NativeQuerySource> {
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
        part.check_reader(&selected.reader)?;
        let pointer_path = format!("sources/{}.md", observed.source_id());
        let pointer_rel = wire::RelativePath::new(&pointer_path)?;
        let pointer = match selected
            .reader
            .permit
            .check_path(&service.library, &pointer_rel)
        {
            Err(wire::ErrorCode::NotExposed) => None,
            Err(code) => return Err(refused_error(code)),
            Ok(()) => match selected
                .reader
                .permit
                .with_root(&service.library, |root| {
                    Ok(LibraryHost::open_root(root, pointer_rel.as_str()))
                })
                .map_err(refused_error)?
            {
                Ok(file) => {
                    let meta = file.metadata()?;
                    anyhow::ensure!(
                        meta.is_file() && meta.len() <= wire::MAX_CONTROL_FRAME_BYTES as u64,
                        "actual pointer metadata refused"
                    );
                    Some((file, meta, pointer_rel))
                }
                Err(wire::ErrorCode::NotFound) => None,
                Err(code) => return Err(refused_error(code)),
            },
        };
        let mut bytes = part
            .framed_bytes()
            .checked_mul(16)
            .and_then(|n| {
                n.checked_add(
                    (observed.observation().stat.path.as_str().len() as u64).checked_mul(32)?,
                )
            })
            .and_then(|n| n.checked_add(1024 * 1024))
            .context("Query source scope envelope overflow")?;
        if let Some((_, meta, _)) = &pointer
            && meta.len() > 16 * 1024
        {
            bytes = bytes
                .checked_add(
                    meta.len()
                        .checked_mul(16)
                        .context("Query source scope envelope overflow")?,
                )
                .context("Query source scope envelope overflow")?;
        }
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
        let result = (|| -> Result<NativeQuerySource> {
            scope
                .retain_check(Arc::new(move || checked.check_scope_claim(&live).is_ok()))
                .map_err(refused_error)?;
            let data = part.data.clone();
            let source = part.source(service, &scope, observed)?;
            let (pointer_title, pointer_descriptions, pointer_tags, pointer_path) =
                if let Some((pointer_file, pointer_meta, pointer_rel)) = pointer {
                    self.cause.admit_object(claim)?;
                    self.cause.admit_io(claim, pointer_meta.len())?;
                    anyhow::ensure!(
                        stamp(&pointer_file)? == stamp_from_meta(&pointer_meta),
                        "pointer changed before read"
                    );
                    let mut pointer_bytes = vec![0u8; pointer_meta.len() as usize];
                    std::os::unix::fs::FileExt::read_exact_at(
                        &pointer_file,
                        &mut pointer_bytes,
                        0,
                    )?;
                    anyhow::ensure!(
                        stamp(&pointer_file)? == stamp_from_meta(&pointer_meta),
                        "pointer changed after read"
                    );
                    LibraryHost::track_version(
                        &service.library,
                        pointer_rel.as_str(),
                        &pointer_file,
                        &pointer_meta,
                        &scope,
                    )
                    .map_err(refused_error)?;
                    let (title, descriptions, tags) =
                        parse_pointer_markdown(&String::from_utf8(pointer_bytes)?);
                    (title, descriptions, tags, Some(pointer_rel))
                } else {
                    (None, Vec::new(), Vec::new(), None)
                };
            let annotations = canonical_annotations(
                &data.document,
                pointer_title,
                pointer_descriptions,
                pointer_tags,
                pointer_path,
            )?;
            Ok(NativeQuerySource {
                annotations,
                part: data,
                cause: self.cause.clone(),
                library,
                claim_owner: Arc::downgrade(claim),
                next,
                source,
            })
        })();
        match result {
            Ok(source) => Ok(source),
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
            annotations,
            part,
            cause,
            library: _,
            claim_owner,
            next: _,
            source,
        } = source;
        let matches = Arc::ptr_eq(&cause, &self.cause)
            && claim_owner
                .upgrade()
                .is_some_and(|owner| std::ptr::eq(owner.as_ref(), claim));
        drop((part, annotations, cause));
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
