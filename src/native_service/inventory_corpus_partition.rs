//! FD-qualified partition adapters over the SAME mature kernel/range readers, not another index.
use super::*;
#[path = "inventory_corpus_query_semantic.rs"]
mod semantic;
use knowledge_retrieval::{ArchiveSource, HeldRangeReaders, SearchResult, SourceGeneration};
pub use semantic::NativeSemanticRun;
/// Complete mature kernel and range peaks. Source SDK/page/tuple/descriptor loans are additional.
pub struct NativeQueryStageResources {
    /// Data-only bound; the actual combined loan comes atomically from the live Claim.
    pub window: u64,
}
impl NativeQueryStageResources {
    pub fn requirement(window: u64) -> Result<Reservation> {
        let a = KernelAdapter::query_requirement();
        let b = HeldRangeReaders::requirement(window)?;
        Ok(Reservation {
            memory_bytes: a
                .memory_bytes
                .checked_add(b.memory_bytes)
                .context("partition quote overflow")?,
            temporary_bytes: 0,
        })
    }
}
/// Actual stopped/unused stage resources, returned without a global release/reserve gap.
pub struct NativeQueryLoans {
    pub job: NativeTupleQuery,
    pub sources: Vec<NativeQuerySource>,
    /// Actual combined query/range loan; None only if preflight acquired no loan.
    pub work: Option<Work>,
    pub claim: Arc<Claim>,
}
pub struct NativeQueryStageRefusal {
    cause: anyhow::Error,
    loans: NativeQueryLoans,
}
impl std::fmt::Debug for NativeQueryStageRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeQueryStageRefusal")
    }
}
impl std::fmt::Display for NativeQueryStageRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("actual Query partition preflight refused; resources retained")
    }
}
impl std::error::Error for NativeQueryStageRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
impl NativeQueryStageRefusal {
    pub fn into_parts(self) -> (anyhow::Error, NativeQueryLoans) {
        (self.cause, self.loans)
    }
}
struct StageOwner {
    sources: Vec<NativeQuerySource>,
    readers: HeldRangeReaders,
    cause: Arc<QueryCause>,
    library: usize,
    claim: Arc<Claim>,
}
#[cfg(test)]
thread_local! {static TEST_CHECK_PROFILE:std::cell::Cell<(u64,u128)>=const{std::cell::Cell::new((0,0))};}
impl StageOwner {
    fn check(&self) -> Result<()> {
        #[cfg(test)]
        let started = Instant::now();
        let result = (|| {
            // Each opaque source's actual funded SDK scope already runs this SAME cause/Claim
            // guard plus its current path/original/M1 dependency checks (issued only by source()).
            // Do not recursively repeat the identical global check before those checks.
            if self.sources.is_empty() {
                self.cause.check_claim(&self.claim)?;
            }
            for source in &self.sources {
                source.source.check()?;
            }
            Ok(())
        })();
        #[cfg(test)]
        TEST_CHECK_PROFILE.with(|p| {
            let (n, t) = p.get();
            p.set((n + 1, t + started.elapsed().as_micros()))
        });
        result
    }
    // Immutable index statistics use the admitted source snapshot. Revalidate ALL source
    // dependencies before/after that operation and on result access. Every actual IO boundary
    // still checks current native Query authority, model, metadata FDs and real Claim/stop.
    // Original representation ranges additionally check their OWN actual source at every refill.
    fn check_io(&self) -> Result<()> {
        #[cfg(test)]
        let started = Instant::now();
        let result = (|| {
            self.cause.check_scope_claim(&self.claim)?;
            let library = &self.cause.libraries[self.library];
            library.reader.check()?;
            library
                .inventory
                .owner
                .policy
                .0
                .lease
                .check()
                .map_err(refused_error)?;
            Ok(())
        })();
        #[cfg(test)]
        TEST_CHECK_PROFILE.with(|p| {
            let (n, t) = p.get();
            p.set((n + 1, t + started.elapsed().as_micros()))
        });
        result
    }
    fn physical(&self) -> &reader::Physical {
        &self.cause.libraries[self.library].reader.physical
    }
    fn control(self: &Arc<Self>) -> Control {
        let owner = self.clone();
        Control::guarded(
            self.claim.deadline().min(self.cause.deadline),
            Arc::new(move || owner.cause.check_execution(&owner.claim).is_ok()),
        )
    }
}
/// A bounded source set plus ONE already paid mature query/read envelope. Keep through delivery.
pub struct NativeQueryStage(Arc<StageOwner>);
impl NativeTupleQuery {
    /// Validate actual source/Claim identities, then atomically split the COMPLETE query+range
    /// envelope from this SAME live Claim. No caller-supplied global Work can substitute.
    pub fn stage(
        &self,
        claim: Arc<Claim>,
        library: usize,
        sources: Vec<NativeQuerySource>,
        resources: NativeQueryStageResources,
    ) -> std::result::Result<NativeQueryStage, NativeQueryStageRefusal> {
        let window = resources.window;
        let mut loans = NativeQueryLoans {
            job: NativeTupleQuery {
                cause: self.cause.clone(),
            },
            sources,
            work: None,
            claim,
        };
        let checked = (|| -> Result<()> {
            let need = NativeQueryStageResources::requirement(window)?;
            self.cause.check_claim(&loans.claim)?;
            anyhow::ensure!(
                library < self.cause.libraries.len(),
                "actual partition library required"
            );
            for source in &loans.sources {
                anyhow::ensure!(
                    Arc::ptr_eq(&source.cause, &self.cause)
                        && source.library == library
                        && source
                            .claim_owner
                            .upgrade()
                            .as_ref()
                            .is_some_and(|owner| Arc::ptr_eq(owner, &loans.claim)),
                    "foreign partition source/Claim owner"
                );
                source.source.check()?;
            }
            loans.work = Some(Work::split_claim(&loans.claim, self.key(), need)?);
            Ok(())
        })();
        if let Err(cause) = checked {
            return Err(NativeQueryStageRefusal { cause, loans });
        }
        let NativeQueryLoans {
            job,
            sources,
            work,
            claim,
        } = loans;
        let readers = match HeldRangeReaders::new_owning(
            work.expect("actual combined loan"),
            window,
            Control::new(claim.deadline()),
        ) {
            Ok(readers) => readers,
            Err((work, cause)) => {
                return Err(NativeQueryStageRefusal {
                    cause: cause.into(),
                    loans: NativeQueryLoans {
                        job,
                        sources,
                        work: Some(work),
                        claim,
                    },
                });
            }
        };
        Ok(NativeQueryStage(Arc::new(StageOwner {
            sources,
            readers,
            cause: job.cause,
            library,
            claim,
        })))
    }
}
#[cfg(test)]
thread_local! {
    static TEST_COMPONENT_OPENS:std::cell::Cell<u64>=const{std::cell::Cell::new(0)};
    static TEST_INDEX_OPEN_HOOK:std::cell::RefCell<Option<Box<dyn FnOnce()>>>=const{std::cell::RefCell::new(None)};
}
#[cfg(test)]
pub(super) fn test_component_open_count() -> u64 {
    TEST_COMPONENT_OPENS.with(std::cell::Cell::get)
}
/// One bounded catalog advance; an empty eligible view skips payload without conflating EOF.
pub struct NativeQueryPartitionPage {
    pub partition: Option<NativeQueryPartition>,
    pub next: u64,
}
impl NativeQueryStage {
    #[cfg(test)]
    pub(in crate::native_service) fn test_vector_bounds(&self) -> Vec<(String, String, u64, u64)> {
        self.0
            .sources
            .iter()
            .filter(|s| s.part.embedding_rows > 0)
            .map(|s| {
                (
                    s.part.document.key.id.clone(),
                    s.part.representation_digest.clone(),
                    s.part.representation_start,
                    s.part.representation_start + s.part.tape_length,
                )
            })
            .collect()
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_component_opens(&self) -> u64 {
        TEST_COMPONENT_OPENS.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_profile(&self) -> (u64, u128) {
        TEST_CHECK_PROFILE.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_on_index_open(&self, hook: impl FnOnce() + 'static) {
        TEST_INDEX_OPEN_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
    }
    pub fn check(&self) -> Result<()> {
        self.0.check()
    }
    /// One sealed catalog frame; source membership is exact held producer data plus actual scopes.
    pub fn partition(&self, offset: u64) -> Result<Option<NativeQueryPartitionPage>> {
        self.check()?;
        let physical = self.0.physical();
        let catalog = physical.open_verified_admitted("lexical.rows", || self.check(), &|n| {
            self.0.cause.admit_io(&self.0.claim, n)
        })?;
        let size = catalog.metadata()?.len();
        if offset == size {
            return Ok(None);
        }
        anyhow::ensure!(offset < size, "partition cursor outside actual catalog");
        let (shard, next) = read_frame_admitted::<Shard>(&catalog, offset, &|n| {
            self.0.cause.admit_io(&self.0.claim, n)
        })?;
        if self.0.sources.is_empty() {
            return Ok(Some(NativeQueryPartitionPage {
                partition: None,
                next,
            }));
        }
        let mut documents = Vec::new();
        for source in &self.0.sources {
            anyhow::ensure!(
                shard.parts.iter().any(|part| part == &source.part),
                "source receipt does not belong to selected partition"
            );
            documents.push(source.part.document.clone());
        }
        let prefix = format!(
            "kernel/{}/{}/",
            hex::encode(Sha256::digest(shard.scope.as_bytes())),
            shard.generation
        );
        let components = Arc::new(Components {
            owner: self.0.clone(),
            prefix,
        });
        let originals = Arc::new(Originals(self.0.clone()));
        let control = self.0.control();
        let generation = SourceGeneration::open(
            &shard.scope,
            &shard.generation,
            &admitted::build_identity(),
            components,
            originals,
            &control,
        )?;
        self.check()?;
        Ok(Some(NativeQueryPartitionPage {
            partition: Some(NativeQueryPartition {
                generation,
                view: ScopeView {
                    scope: shard.scope,
                    documents,
                    unavailable_objects: 0,
                },
                owner: self.0.clone(),
            }),
            next,
        }))
    }
    /// No consumer may outlive the returned exact loans. Live FD/bytes/result returns THIS owner.
    pub fn into_loans(self) -> std::result::Result<NativeQueryLoans, Self> {
        let inner = Arc::try_unwrap(self.0).map_err(Self)?;
        let StageOwner {
            sources,
            readers,
            cause,
            library,
            claim,
        } = inner;
        match readers.into_work() {
            Ok(ranges) => Ok(NativeQueryLoans {
                job: NativeTupleQuery { cause },
                sources,
                work: Some(ranges),
                claim,
            }),
            Err(readers) => Err(Self(Arc::new(StageOwner {
                sources,
                readers,
                cause,
                library,
                claim,
            }))),
        }
    }
}
struct HeldFile {
    file: File,
    version: [u64; 8],
    owner: Arc<StageOwner>,
    original: Option<usize>,
}
fn check_range(owner: &StageOwner, original: Option<usize>) -> Result<()> {
    owner.check_io()?;
    if let Some(index) = original {
        owner.sources[index].source.check()?;
    }
    Ok(())
}
impl ArchiveSource for HeldFile {
    fn check(&self) -> std::result::Result<(), knowledge_retrieval::Error> {
        check_range(&self.owner, self.original)
            .map_err(|_| knowledge_retrieval::Error::Inventory)?;
        if stamp(&self.file).map_err(|_| knowledge_retrieval::Error::Inventory)? != self.version {
            return Err(knowledge_retrieval::Error::Inventory);
        }
        Ok(())
    }
    fn file(&self) -> std::result::Result<&File, knowledge_retrieval::Error> {
        // SAME range engine checks ArchiveSource::check before borrowing this exact immutable FD
        // and again after reading. Borrowing its descriptor must not repeat that entire check.
        Ok(&self.file)
    }
    fn admit_read(&self, bytes: u64) -> std::result::Result<(), knowledge_retrieval::Error> {
        #[cfg(test)]
        if std::env::var_os("OMNI_TEST_BYPASS_RANGE_DATA_BUDGET").is_some() {
            return Ok(());
        }
        self.owner
            .cause
            .admit_io(&self.owner.claim, bytes)
            .map_err(|error| {
                if error.is::<QueryBudgetExhausted>() {
                    knowledge_retrieval::Error::ReadBudget
                } else {
                    knowledge_retrieval::Error::Inventory
                }
            })
    }
}
fn range(
    owner: &Arc<StageOwner>,
    name: &str,
    offset: u64,
    length: Option<u64>,
    original: Option<usize>,
) -> Result<Arc<dyn ByteHandle>> {
    check_range(owner, original)?;
    let file =
        owner
            .physical()
            .open_verified_admitted(name, || check_range(owner, original), &|n| {
                owner.cause.admit_io(&owner.claim, n)
            })?;
    let version = stamp(&file)?;
    let length = length.unwrap_or(version[2]);
    let source = Arc::new(HeldFile {
        file,
        version,
        owner: owner.clone(),
        original,
    });
    Ok(owner.readers.open(source, offset, length)?)
}
struct Components {
    owner: Arc<StageOwner>,
    prefix: String,
}
impl std::fmt::Debug for Components {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActualTupleComponents")
    }
}
impl Components {
    fn name(&self, name: &str) -> std::result::Result<String, KernelError> {
        if name.is_empty()
            || std::path::Path::new(name)
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err(kernel_error());
        }
        Ok(format!("{}{name}", self.prefix))
    }
}
impl ByteDirectory for Components {
    fn check(&self) -> std::result::Result<(), KernelError> {
        self.owner.check_io().map_err(|_| kernel_error())
    }
    fn exists(&self, name: &str) -> std::result::Result<bool, KernelError> {
        self.check()?;
        self.owner
            .physical()
            .member_exists_admitted(&self.name(name)?, || self.owner.check_io(), &|n| {
                self.owner.cause.admit_io(&self.owner.claim, n)
            })
            .map_err(|_| kernel_error())
    }
    fn open(&self, name: &str) -> std::result::Result<Arc<dyn ByteHandle>, KernelError> {
        self.check()?;
        #[cfg(test)]
        TEST_COMPONENT_OPENS.with(|n| n.set(n.get() + 1));
        #[cfg(test)]
        if name.starts_with("index/") {
            TEST_INDEX_OPEN_HOOK.with(|h| {
                let hook = h.borrow_mut().take();
                if let Some(hook) = hook {
                    hook()
                }
            });
        }
        range(&self.owner, &self.name(name)?, 0, None, None).map_err(|_| kernel_error())
    }
}
struct Originals(Arc<StageOwner>);
impl std::fmt::Debug for Originals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActualTupleRepresentations")
    }
}
impl ByteDirectory for Originals {
    fn check(&self) -> std::result::Result<(), KernelError> {
        self.0.check_io().map_err(|_| kernel_error())
    }
    fn exists(&self, name: &str) -> std::result::Result<bool, KernelError> {
        self.check()?;
        Ok(self.0.sources.iter().any(
            |s| matches!(&s.part.document.content,Content::Representation{path,..}if path==name),
        ))
    }
    fn open(&self, name: &str) -> std::result::Result<Arc<dyn ByteHandle>, KernelError> {
        self.check()?;
        let index=self.0.sources.iter().position(|s|matches!(&s.part.document.content,Content::Representation{path,..}if path==name)).ok_or_else(kernel_error)?;
        let part = &self.0.sources[index].part;
        range(
            &self.0,
            "representations.data",
            part.tape_start,
            Some(part.tape_length),
            Some(index),
        )
        .map_err(|_| kernel_error())
    }
}
/// One exact held generation and its eligible view, not independent component CURRENT selection.
pub struct NativeQueryPartition {
    generation: SourceGeneration,
    view: ScopeView,
    owner: Arc<StageOwner>,
}
impl NativeQueryPartition {
    /// Consume only a BM25/keyword explicitly present in the SAME actual sealed Query.
    pub fn lexical(self, keyword: usize) -> Result<NativePartitionResult> {
        self.owner.check()?;
        let wire::Command::Query { query } = self.owner.cause.libraries[self.owner.library]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        anyhow::ensure!(
            query.methods.contains(&wire::SearchMethod::Bm25),
            "BM25 not requested by actual Query"
        );
        anyhow::ensure!(
            self.owner.sources.iter().all(|s| s
                .annotations()
                .matches_required_tags(&query.required_tags, query.case_sensitive)),
            "actual current native tag dependency required"
        );
        let text = query
            .keywords
            .get(keyword)
            .context("actual Query keyword required")?;
        let control = self.owner.control();
        let result = self.generation.search(
            knowledge_retrieval::Query {
                mode: knowledge_retrieval::Mode::Lexical,
                query: text.clone(),
                scopes: vec![self.view.clone()],
                limit: query.limit.get() as usize,
            },
            &control,
        )?;
        self.owner.check()?;
        Ok(NativePartitionResult {
            result,
            partition: self,
        })
    }
    /// Scan one ACTUAL already-admitted source through the SAME mature Exact/Regex engine.
    /// Raw occurrence limits and coverage stay explicit; they are not distinct-file ranking.
    /// The outer controller must visit every admitted representation and retain real locators.
    pub fn scan(
        self,
        method: wire::SearchMethod,
        keyword: usize,
        source: usize,
    ) -> Result<NativePartitionResult> {
        self.owner.check()?;
        let wire::Command::Query { query } = self.owner.cause.libraries[self.owner.library]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        anyhow::ensure!(
            matches!(
                method,
                wire::SearchMethod::Literal | wire::SearchMethod::Regex
            ) && query.methods.contains(&method),
            "actual selected literal/regex method required"
        );
        anyhow::ensure!(
            self.owner.sources.get(source).is_some_and(|s| s
                .annotations()
                .matches_required_tags(&query.required_tags, query.case_sensitive)),
            "actual current native tag dependency required"
        );
        let keyword = query
            .keywords
            .get(keyword)
            .context("actual Query keyword required")?;
        let (mode, text) = match method {
            wire::SearchMethod::Literal if query.case_sensitive => {
                (knowledge_retrieval::Mode::Exact, keyword.clone())
            }
            wire::SearchMethod::Literal => (
                knowledge_retrieval::Mode::Regex,
                format!("(?i:{})", regex::escape(keyword)),
            ),
            wire::SearchMethod::Regex if query.case_sensitive => {
                (knowledge_retrieval::Mode::Regex, keyword.clone())
            }
            wire::SearchMethod::Regex => {
                (knowledge_retrieval::Mode::Regex, format!("(?i:{keyword})"))
            }
            _ => unreachable!("closed actual scan method"),
        };
        let document = self
            .view
            .documents
            .get(source)
            .context("actual admitted source slot")?
            .clone();
        let result = self.generation.search(
            knowledge_retrieval::Query {
                mode,
                query: text,
                scopes: vec![ScopeView {
                    scope: self.view.scope.clone(),
                    documents: vec![document],
                    unavailable_objects: 0,
                }],
                limit: query.limit.get() as usize,
            },
            &self.owner.control(),
        )?;
        self.owner.check()?;
        Ok(NativePartitionResult {
            result,
            partition: self,
        })
    }
}
/// Results retain every actual source, FD, whole-tuple pin, Claim and query/read credit.
pub struct NativePartitionResult {
    result: SearchResult,
    partition: NativeQueryPartition,
}
impl NativePartitionResult {
    pub fn result(&self) -> Result<&SearchResult> {
        self.partition.owner.check()?;
        Ok(&self.result)
    }
    pub fn check(&self) -> Result<()> {
        self.partition.owner.check()
    }
    pub fn into_partition(self) -> NativeQueryPartition {
        let Self { result, partition } = self;
        drop(result);
        partition
    }
}
