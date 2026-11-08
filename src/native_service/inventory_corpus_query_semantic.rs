//! SAME native tuple/model/Claim and mature vector engine.
//! `NativeSemanticRun` covers one admitted source set. `NativeWholeSemanticRun` is one
//! whole-library semantic pass. This is not the all-method controller or model quality.
use super::super::super::reader::TuplePart;
use super::super::super::vectors::{locator, representation};
use super::super::keyword_embedding::NativeQueryEmbedding;
use super::*;
use knowledge_search::{
    VectorBuildPlan, VectorCandidate, VectorError, VectorEvidence, VectorResult, VectorStore,
};

impl NativeTupleQuery {
    /// SAME mature reader formula from actual source-row metadata before submission. This is
    /// DATA only; native source SDK/page/range/kernel/provider peaks are additional actual loans.
    pub fn vector_requirement(&self, library: usize) -> Result<Reservation> {
        self.cause.check()?;
        let lib = self
            .cause
            .libraries
            .get(library)
            .context("actual Query library")?;
        let model = lib
            .model
            .as_ref()
            .context("actual selected semantic model owner")?;
        model.check_permit(&lib.reader.permit)?;
        let wire::Command::Query { query } = lib
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        anyhow::ensure!(
            query.methods.contains(&wire::SearchMethod::Semantic),
            "actual selected semantic method required"
        );
        anyhow::ensure!(
            lib.reader.physical.data.model.as_ref() == Some(model.model()?),
            "actual tuple semantic dependency differs"
        );
        let file = lib.reader.physical.open_verified_admitted(
            "sources.rows",
            || lib.reader.check(),
            &|n| self.cause.admit_planning(n),
        )?;
        let mut plan = VectorBuildPlan {
            rows: 0,
            metadata_bytes: 0,
            max_row_bytes: 0,
        };
        let mut at = 0;
        while at < file.metadata()?.len() {
            self.cause.admit_planning_object()?;
            let (part, next) =
                read_frame_admitted::<SourcePart>(&file, at, &|n| self.cause.admit_planning(n))?;
            if part.embedding_rows > 0 {
                plan.include(
                    VectorCandidate {
                        source_id: &part.document.key.id,
                        source_digest: &part.document.digest,
                        representation: &representation(&part)?,
                        locator: &locator(&part, u64::MAX, u64::MAX),
                    },
                    part.embedding_rows,
                )?;
            }
            at = next;
            lib.reader.check()?;
        }
        anyhow::ensure!(
            plan.rows == lib.reader.physical.data.vector_rows,
            "actual complete tuple vector-row count differs"
        );
        if plan.rows == 0 {
            return Ok(Reservation {
                memory_bytes: 0,
                temporary_bytes: 0,
            });
        }
        Ok(plan.query_requirement(model.model()?, query.limit.get() as usize)?)
    }
}

#[cfg(test)]
thread_local! {
    static TEST_SEMANTIC_QUERY_ROWS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
/// Actual vector result/error, reader Work and same source/range/model/Claim owners. Data dies
/// before explicit same-Claim rejoin. No status quote or bare numeric vector grants permission.
pub struct NativeSemanticRun {
    failure: Option<anyhow::Error>,
    result: Option<VectorResult>,
    work: Option<Arc<Work>>,
    owner: Arc<StageOwner>,
    empty: bool,
    closing: bool,
}
impl NativeQueryStage {
    /// Run the SAME mature exact cosine/distinct-file algorithm over this actual admitted source
    /// set. Other representations/libraries must be visited by the outer controller. All simultaneous
    /// stage/source/buffer/reader peaks must already fit the enclosing original Query Claim.
    pub fn semantic(
        &self,
        embedding: &NativeQueryEmbedding,
        reader_need: Reservation,
    ) -> NativeSemanticRun {
        let mut run = NativeSemanticRun {
            failure: None,
            result: None,
            work: None,
            owner: self.0.clone(),
            empty: false,
            closing: false,
        };
        if let Err(error) = run.begin(embedding, reader_need) {
            run.failure = Some(error);
        }
        run
    }
}
impl StageOwner {
    fn vector_eligible(&self, row: VectorCandidate<'_>) -> std::result::Result<bool, VectorError> {
        for source in &self.sources {
            let part = &source.part;
            if row.source_id != part.document.key.id
                || row.source_digest != part.document.digest
                || part.embedding_rows == 0
            {
                continue;
            }
            let expected = representation(part).map_err(|_| {
                VectorError::Identity("actual native representation dependency".into())
            })?;
            if row.representation != &expected {
                continue;
            }
            let prefix = format!("representation:{}@", part.representation_digest);
            let span = row
                .locator
                .strip_prefix(&prefix)
                .and_then(|s| s.split_once(".."))
                .and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)))
                .ok_or_else(|| {
                    VectorError::Identity("actual native vector locator dependency".into())
                })?;
            let end = part
                .representation_start
                .checked_add(part.tape_length)
                .ok_or_else(|| {
                    VectorError::Identity("actual native representation overflow".into())
                })?;
            if span.0 >= span.1 {
                return Err(VectorError::Identity(
                    "actual native vector locator is empty or reversed".into(),
                ));
            }
            // Equal-byte windows may legitimately share a representation digest. Their actual
            // global spans still differ: another window is ineligible, not corrupt or admitted.
            if span.0 < part.representation_start || span.1 > end {
                continue;
            }
            source.source.check().map_err(|_| {
                VectorError::Identity("actual native vector source unavailable".into())
            })?;
            if !source
                .matches_required_tags()
                .map_err(|_| VectorError::Identity("actual native vector tag dependency".into()))?
            {
                continue;
            }
            return Ok(true);
        }
        Ok(false)
    }
}
impl NativeSemanticRun {
    fn admit_rows(&self) -> Result<()> {
        let count = self.owner.physical().data.vector_rows;
        // Exact immutable tuple count is checked against the manifest BEFORE opening rows.bin.
        // Successful mature query/evidence passes decode every one of these metadata records.
        // This is DATA admission, not a second resource account or a source permission cache.
        for _ in 0..count {
            self.owner.cause.admit_object(&self.owner.claim)?;
        }
        Ok(())
    }
    fn begin(&mut self, embedding: &NativeQueryEmbedding, reader_need: Reservation) -> Result<()> {
        self.owner.check()?;
        let vector =
            embedding.vector_for(&self.owner.cause, &self.owner.claim, self.owner.library)?;
        let lib = &self.owner.cause.libraries[self.owner.library];
        let wire::Command::Query { query } = lib
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        anyhow::ensure!(
            query.methods.contains(&wire::SearchMethod::Semantic),
            "actual selected semantic method required"
        );
        anyhow::ensure!(
            self.owner.sources.iter().all(|s| s
                .annotations()
                .matches_required_tags(&query.required_tags, query.case_sensitive)),
            "actual current native tag dependency required"
        );
        if self
            .owner
            .sources
            .iter()
            .all(|s| s.part.embedding_rows == 0)
        {
            self.empty = true;
            return Ok(());
        }
        anyhow::ensure!(
            reader_need.temporary_bytes == 0,
            "read-only vector resources required"
        );
        let generation = lib
            .reader
            .physical
            .data
            .vector_generation
            .as_ref()
            .context("actual tuple vector generation")?;
        let model = lib
            .reader
            .physical
            .data
            .model
            .as_ref()
            .context("actual tuple semantic model")?;
        self.owner.cause.admit_object(&self.owner.claim)?;
        self.admit_rows()?;
        self.work = Some(Arc::new(Work::split_claim(
            &self.owner.claim,
            self.owner.cause.key(),
            reader_need,
        )?));
        let directory: Arc<dyn ByteDirectory> = Arc::new(Components {
            owner: self.owner.clone(),
            prefix: format!("vectors/{generation}/"),
        });
        let store = VectorStore::open_directory_held(
            &directory,
            generation,
            model,
            self.work.as_ref().expect("actual vector reader loan"),
            Some(lib.reader.physical.data.vector_rows),
        )?;
        #[cfg(test)]
        TEST_SEMANTIC_QUERY_ROWS.with(|count| count.set(count.get() + 1));
        self.result = Some(store.query_rows(vector, query.limit.get() as usize, |row| {
            self.owner.vector_eligible(row)
        })?);
        self.owner.check()
    }
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.failure.as_ref()
    }
    #[cfg(test)]
    pub fn test_semantic_query_rows() -> u64 {
        TEST_SEMANTIC_QUERY_ROWS.with(std::cell::Cell::get)
    }
    pub fn check(&self) -> Result<()> {
        anyhow::ensure!(
            !self.closing && self.failure.is_none(),
            "native semantic result is not deliverable"
        );
        self.owner.check()?;
        if let Some(result) = &self.result {
            result.check()?;
        }
        anyhow::ensure!(
            self.empty || self.result.is_some(),
            "actual native semantic outcome required"
        );
        Ok(())
    }
    /// Bounded distinct original file ranks, NOT a complete library/all-method response.
    pub fn hits(&self) -> Result<&[(String, f32)]> {
        self.check()?;
        Ok(self.result.as_ref().map(|r| r.as_slice()).unwrap_or(&[]))
    }
    /// All mature evidence locations for winning originals in this ACTUAL admitted source set.
    /// The consumer must separately prepay copied/serialized data and retain its delivery owner.
    pub fn visit_evidence(
        &self,
        mut visit: impl FnMut(&VectorEvidence) -> Result<()>,
    ) -> Result<()> {
        self.check()?;
        if let Some(result) = &self.result {
            self.admit_rows()?;
            result.visit_rows(
                |row| self.owner.vector_eligible(row),
                |hit| {
                    self.owner.check().map_err(|_| {
                        VectorError::Identity("actual native evidence source unavailable".into())
                    })?;
                    visit(hit).map_err(|_| {
                        VectorError::Identity("actual native evidence consumer refused".into())
                    })
                },
            )?;
        }
        self.check()
    }
    /// Destroy error/result/vector/FD consumers BEFORE exact same-Claim rejoin. A failed rejoin
    /// returns THIS actual reader/source/range owner, never an error code or settlement marker.
    pub fn finish(mut self) -> std::result::Result<(), Box<Self>> {
        self.closing = true;
        drop(self.failure.take());
        drop(self.result.take());
        if let Some(work) = self.work.take() {
            match Arc::try_unwrap(work) {
                Ok(mut work) => {
                    if let Err(error) = work.rejoin(&self.owner.claim, self.owner.cause.key()) {
                        self.failure = Some(error.into());
                        self.work = Some(Arc::new(work));
                        return Err(Box::new(self));
                    }
                }
                Err(work) => {
                    self.work = Some(work);
                    return Err(Box::new(self));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
thread_local! {
    static TEST_WHOLE_SEMANTIC_PASSES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
/// One whole-library semantic pass: keyword vector, one `rows.bin` scan, and the winning sources.
/// Evidence, the vector result, and errors die before source scopes and Claim loans rejoin.
pub struct NativeWholeSemanticRun {
    query: NativeTupleQuery,
    claim: Arc<Claim>,
    library: usize,
    data: Option<Box<WholeData>>,
    embedding: Option<Box<NativeQueryEmbedding>>,
    // Administrative reservation. Result bytes and the embedding buffer die before this loan.
    base: Option<Work>,
}
struct WholeData {
    evidence: std::collections::BTreeMap<String, Vec<VectorEvidence>>,
    result: Option<VectorResult>,
    sources: std::collections::BTreeMap<String, NativeQuerySource>,
    pending: Vec<NativeQuerySource>,
    scopes: Vec<Permit>,
    page: Option<(u64, NativeInventoryPage)>,
    pinned_pages: Vec<NativeInventoryPage>,
    vectors: Option<Arc<WholeVectorOwner>>,
    reader_work: Option<Arc<Work>>,
    extra: Vec<Work>,
    credits: Vec<Holding>,
    metadata_capacity: u64,
    metadata_used: u64,
    unavailable_parts: u64,
    coverage: QueryCoverage,
    empty: bool,
    closing: bool,
    failure: Option<anyhow::Error>,
}
impl WholeData {
    fn new() -> Self {
        Self {
            evidence: std::collections::BTreeMap::new(),
            result: None,
            sources: std::collections::BTreeMap::new(),
            pending: Vec::new(),
            scopes: Vec::new(),
            page: None,
            pinned_pages: Vec::new(),
            vectors: None,
            reader_work: None,
            extra: Vec::new(),
            credits: Vec::new(),
            metadata_capacity: 0,
            metadata_used: 0,
            unavailable_parts: 0,
            coverage: QueryCoverage::default(),
            empty: false,
            closing: false,
            failure: None,
        }
    }
}
struct CurrentPart {
    part: SourcePart,
    remaining: u64,
    eligible: bool,
}
struct WholeVectorOwner {
    readers: std::sync::Mutex<Option<HeldRangeReaders>>,
    cause: Arc<QueryCause>,
    library: usize,
    claim: Arc<Claim>,
    prefix: String,
}
impl WholeVectorOwner {
    /// Same IO guard as a stage range with no original-source index (`check_range(owner, None)`).
    fn check_range(&self) -> Result<()> {
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
    }
    fn physical(&self) -> &reader::Physical {
        &self.cause.libraries[self.library].reader.physical
    }
}
struct WholeVectorComponents {
    owner: Arc<WholeVectorOwner>,
}
impl std::fmt::Debug for WholeVectorComponents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WholeVectorComponents")
    }
}
impl WholeVectorComponents {
    fn component_name(&self, name: &str) -> std::result::Result<String, KernelError> {
        if name.is_empty()
            || std::path::Path::new(name)
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(kernel_error());
        }
        Ok(format!("{}{name}", self.owner.prefix))
    }
}
struct VectorComponentFile {
    file: std::fs::File,
    version: [u64; 8],
    owner: Arc<WholeVectorOwner>,
}
impl ArchiveSource for VectorComponentFile {
    fn check(&self) -> std::result::Result<(), knowledge_retrieval::Error> {
        self.owner
            .check_range()
            .map_err(|_| knowledge_retrieval::Error::Inventory)?;
        if stamp(&self.file).map_err(|_| knowledge_retrieval::Error::Inventory)? != self.version {
            return Err(knowledge_retrieval::Error::Inventory);
        }
        Ok(())
    }
    fn file(&self) -> std::result::Result<&std::fs::File, knowledge_retrieval::Error> {
        Ok(&self.file)
    }
    fn admit_read(&self, bytes: u64) -> std::result::Result<(), knowledge_retrieval::Error> {
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
impl ByteDirectory for WholeVectorComponents {
    fn check(&self) -> std::result::Result<(), KernelError> {
        self.owner.check_range().map_err(|_| kernel_error())
    }
    fn exists(&self, name: &str) -> std::result::Result<bool, KernelError> {
        self.check()?;
        let full = self.component_name(name)?;
        self.owner
            .physical()
            .member_exists_admitted(&full, || self.owner.check_range(), &|n| {
                self.owner.cause.admit_io(&self.owner.claim, n)
            })
            .map_err(|_| kernel_error())
    }
    fn open(&self, name: &str) -> std::result::Result<Arc<dyn ByteHandle>, KernelError> {
        self.open_component(name).map_err(|_| kernel_error())
    }
}
impl WholeVectorComponents {
    fn open_component(&self, name: &str) -> Result<Arc<dyn ByteHandle>> {
        self.owner.check_range()?;
        #[cfg(test)]
        super::TEST_COMPONENT_OPENS.with(|n| n.set(n.get() + 1));
        let full = self.component_name(name).map_err(|_| kernel_error())?;
        let file = self.owner.physical().open_verified_admitted(
            &full,
            || self.owner.check_range(),
            &|n| self.owner.cause.admit_io(&self.owner.claim, n),
        )?;
        let version = stamp(&file)?;
        let length = version[2];
        let source = Arc::new(VectorComponentFile {
            file,
            version,
            owner: self.owner.clone(),
        });
        let guard = self
            .owner
            .readers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let readers = guard.as_ref().context("actual vector range readers")?;
        Ok(readers.open(source, 0, length)?)
    }
}
fn same_observed(source: &NativeObservedSource, id: &str, locator: &str) -> bool {
    source.source_id() == id && source.observation().stat.path.as_str() == locator
}
fn row_matches(
    part: &SourcePart,
    row: VectorCandidate<'_>,
) -> std::result::Result<(), VectorError> {
    if row.source_id != part.document.key.id || row.source_digest != part.document.digest {
        return Err(VectorError::Identity(
            "actual native vector row source differs".into(),
        ));
    }
    let expected = representation(part)
        .map_err(|_| VectorError::Identity("actual native representation dependency".into()))?;
    if row.representation != &expected {
        return Err(VectorError::Identity(
            "actual native vector representation differs".into(),
        ));
    }
    let prefix = format!("representation:{}@", part.representation_digest);
    let span = row
        .locator
        .strip_prefix(&prefix)
        .and_then(|tail| tail.split_once(".."))
        .and_then(|(start, end)| Some((start.parse::<u64>().ok()?, end.parse::<u64>().ok()?)))
        .ok_or_else(|| VectorError::Identity("actual native vector locator dependency".into()))?;
    let end = part
        .representation_start
        .checked_add(part.tape_length)
        .ok_or_else(|| VectorError::Identity("actual native representation overflow".into()))?;
    if span.0 >= span.1 {
        return Err(VectorError::Identity(
            "actual native vector locator is empty or reversed".into(),
        ));
    }
    if span.0 < part.representation_start || span.1 > end {
        return Err(VectorError::Identity(
            "actual native vector locator is outside its source part".into(),
        ));
    }
    Ok(())
}
struct Stream<'a> {
    run: &'a mut NativeWholeSemanticRun,
    library_id: &'a wire::Id,
    cursor: u64,
    sources_len: u64,
    current: Option<CurrentPart>,
    active: Option<(String, NativeQuerySource)>,
    callback_error: Option<anyhow::Error>,
}
impl Drop for Stream<'_> {
    fn drop(&mut self) {
        if let Some((_, source)) = self.active.take() {
            self.run
                .data
                .as_mut()
                .expect("actual owner")
                .pending
                .push(source);
        }
    }
}
impl Stream<'_> {
    fn read_part(&mut self) -> Result<(u64, TuplePart, u64)> {
        anyhow::ensure!(
            self.cursor < self.sources_len,
            "actual source cursor is exhausted"
        );
        self.run.query.cause.admit_object(&self.run.claim)?;
        let part_offset = self.cursor;
        let (tuple, after) = self.run.query.cause.libraries[self.run.library]
            .reader
            .part_admitted(part_offset, &|n| {
                self.run.query.cause.admit_io(&self.run.claim, n)
            })?
            .context("actual source part absent")?;
        self.cursor = after;
        Ok((part_offset, tuple, after))
    }
    fn consider(&mut self, _part_offset: u64, tuple: TuplePart, after: u64) -> Result<bool> {
        let id = tuple.data.document.key.id.clone();
        if self
            .active
            .as_ref()
            .is_some_and(|(active_id, _)| active_id == &id)
        {
            return Ok(true);
        }
        if let Some((_, source)) = self.active.take() {
            self.run.close_source(source)?;
        }
        let locator = tuple
            .data
            .document
            .locator
            .clone()
            .context("actual source logical path absent")?;
        let excluded = {
            let library = &self.run.query.cause.libraries[self.run.library];
            if library.inventory.owner.path_excluded(&locator) {
                true
            } else {
                match library
                    .reader
                    .permit
                    .check_path(self.library_id, &wire::RelativePath::new(&locator)?)
                {
                    Err(wire::ErrorCode::NotExposed) => true,
                    Err(code) => return Err(refused_error(code)),
                    Ok(()) => false,
                }
            }
        };
        if excluded {
            return Ok(false);
        }
        let first_source = self
            .run
            .data
            .as_mut()
            .expect("actual owner")
            .coverage
            .begin(&id, &tuple.data.status);
        let page_offset = tuple.data.page_offset;
        let embedding_rows = tuple.data.embedding_rows;
        let observed = self.run.observed_for(page_offset, &id, &locator)?;
        let Some(observed) = observed else {
            if first_source {
                self.run
                    .data
                    .as_mut()
                    .expect("actual owner")
                    .coverage
                    .unavailable += 1;
            }
            self.run
                .data
                .as_mut()
                .expect("actual owner")
                .unavailable_parts += 1;
            return Ok(false);
        };
        match self.run.query.source_part(
            self.library_id,
            &self.run.claim,
            self.run.library,
            observed,
            tuple,
            after,
        ) {
            Ok(source) => {
                if first_source {
                    self.run
                        .data
                        .as_mut()
                        .expect("actual owner")
                        .coverage
                        .verified += 1;
                }
                match source.matches_required_tags() {
                    Ok(true) if embedding_rows == 0 => {
                        self.run.close_source(source)?;
                        Ok(false)
                    }
                    Ok(true) => {
                        self.active = Some((id, source));
                        Ok(true)
                    }
                    Ok(false) => {
                        self.run.close_source(source)?;
                        Ok(false)
                    }
                    Err(error) => {
                        self.run.close_source(source)?;
                        Err(error)
                    }
                }
            }
            Err(error) => {
                let unavailable = error
                    .downcast_ref::<crate::native_service::NativeRefreshScopeError>()
                    .is_none()
                    && error.chain().any(|cause| {
                        cause.is::<crate::native_service::source::CachedQuerySourceUnavailable>()
                    });
                if unavailable {
                    if first_source {
                        self.run
                            .data
                            .as_mut()
                            .expect("actual owner")
                            .coverage
                            .unavailable += 1;
                    }
                    self.run
                        .data
                        .as_mut()
                        .expect("actual owner")
                        .unavailable_parts += 1;
                    Ok(false)
                } else {
                    Err(error)
                }
            }
        }
    }
    fn prepare_part(&mut self) -> Result<()> {
        if self.current.as_ref().is_some_and(|part| part.remaining > 0) {
            return Ok(());
        }
        loop {
            if self.cursor >= self.sources_len {
                bail!("actual vector rows exceed admitted source parts");
            }
            let (part_offset, tuple, after) = self.read_part()?;
            let part = tuple.data.clone();
            let rows = part.embedding_rows;
            let eligible = self.consider(part_offset, tuple, after)?;
            if rows > 0 {
                self.current = Some(CurrentPart {
                    part,
                    remaining: rows,
                    eligible,
                });
                return Ok(());
            }
        }
    }
    fn accept(&mut self, row: VectorCandidate<'_>) -> std::result::Result<bool, VectorError> {
        if let Err(error) = self.prepare_part() {
            self.callback_error = Some(error);
            return Err(VectorError::Identity(
                "actual native source eligibility refused".into(),
            ));
        }
        let eligible = {
            let Some(current) = self.current.as_mut() else {
                return Err(VectorError::Identity(
                    "actual native vector row has no source part".into(),
                ));
            };
            if current.remaining == 0 {
                return Err(VectorError::Identity(
                    "actual native vector row exceeds its source part".into(),
                ));
            }
            current.remaining -= 1;
            row_matches(&current.part, row)?;
            current.eligible
        };
        if !eligible {
            return Ok(false);
        }
        let Some((_, source)) = self.active.as_ref() else {
            return Err(VectorError::Identity(
                "actual native vector source is not open".into(),
            ));
        };
        source
            .source()
            .check()
            .map_err(|_| VectorError::Identity("actual native vector source unavailable".into()))?;
        Ok(true)
    }
    fn drain_tail(&mut self) -> Result<()> {
        while self.cursor < self.sources_len {
            let (part_offset, tuple, after) = self.read_part()?;
            anyhow::ensure!(
                tuple.data.embedding_rows == 0,
                "actual source parts exceed vector rows"
            );
            let _eligible = self.consider(part_offset, tuple, after)?;
        }
        Ok(())
    }
    fn finish_pass(&mut self, result: &VectorResult) -> Result<()> {
        if self
            .current
            .as_ref()
            .is_some_and(|part| part.remaining != 0)
        {
            bail!("actual vector rows do not cover admitted source parts");
        }
        self.drain_tail()?;
        if let Some((id, source)) = self.active.take() {
            if result.iter().any(|(hit, _)| hit == &id) {
                self.run
                    .data
                    .as_mut()
                    .expect("actual owner")
                    .sources
                    .insert(id, source);
            } else {
                self.run.close_source(source)?;
            }
        }
        let mut winners: std::collections::BTreeSet<String> = result
            .iter()
            .filter(|(id, _)| {
                !self
                    .run
                    .data
                    .as_ref()
                    .expect("actual owner")
                    .sources
                    .contains_key(id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        // Only the bounded winners stay resident. Rewalk source metadata once, never the
        // vector index once per winner, instead of retaining an unpaid offset map for all sources.
        self.cursor = 0;
        while !winners.is_empty() && self.cursor < self.sources_len {
            let (_, tuple, after) = self.read_part()?;
            let id = tuple.data.document.key.id.clone();
            if winners.remove(&id) {
                self.reopen(&id, tuple, after)?;
            }
        }
        anyhow::ensure!(
            winners.is_empty(),
            "actual winning semantic source part is absent"
        );
        self.run.release_page()?;
        Ok(())
    }
    fn reopen(&mut self, id: &str, tuple: TuplePart, after: u64) -> Result<()> {
        let page_offset = tuple.data.page_offset;
        anyhow::ensure!(
            tuple.data.document.key.id == id,
            "winning source part differs"
        );
        let locator = tuple
            .data
            .document
            .locator
            .clone()
            .context("actual source logical path absent")?;
        let observed = self
            .run
            .observed_for(page_offset, id, &locator)?
            .context("actual winning M1 source absent")?;
        let source = self.run.query.source_part(
            self.library_id,
            &self.run.claim,
            self.run.library,
            observed,
            tuple,
            after,
        )?;
        match source.matches_required_tags() {
            Ok(true) => {
                self.run
                    .data
                    .as_mut()
                    .expect("actual owner")
                    .sources
                    .insert(id.to_owned(), source);
                Ok(())
            }
            Ok(false) => {
                self.run.close_source(source)?;
                bail!("actual winning source lost required tags");
            }
            Err(error) => {
                self.run.close_source(source)?;
                Err(error)
            }
        }
    }
}
impl NativeTupleQuery {
    /// Same reader reservation as `vector_requirement`, charged to the submitted execution
    /// allowance. The public quote admits planning bytes only before submission.
    fn live_vector_requirement(&self, claim: &Claim, library: usize) -> Result<Reservation> {
        self.cause.check_claim(claim)?;
        let lib = self
            .cause
            .libraries
            .get(library)
            .context("actual Query library")?;
        let model = lib
            .model
            .as_ref()
            .context("actual selected semantic model owner")?;
        model.check_permit(&lib.reader.permit)?;
        let wire::Command::Query { query } = lib
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        anyhow::ensure!(
            query.methods.contains(&wire::SearchMethod::Semantic),
            "actual selected semantic method required"
        );
        anyhow::ensure!(
            lib.reader.physical.data.model.as_ref() == Some(model.model()?),
            "actual tuple semantic dependency differs"
        );
        let limit = query.limit.get() as usize;
        let model_id = model.model()?.clone();
        let file = lib.reader.physical.open_verified_admitted(
            "sources.rows",
            || lib.reader.check(),
            &|n| self.cause.admit_io(claim, n),
        )?;
        let mut plan = VectorBuildPlan {
            rows: 0,
            metadata_bytes: 0,
            max_row_bytes: 0,
        };
        let mut at = 0;
        while at < file.metadata()?.len() {
            self.cause.admit_object(claim)?;
            let (part, next) =
                read_frame_admitted::<SourcePart>(&file, at, &|n| self.cause.admit_io(claim, n))?;
            if part.embedding_rows > 0 {
                plan.include(
                    VectorCandidate {
                        source_id: &part.document.key.id,
                        source_digest: &part.document.digest,
                        representation: &representation(&part)?,
                        locator: &locator(&part, u64::MAX, u64::MAX),
                    },
                    part.embedding_rows,
                )?;
            }
            at = next;
            lib.reader.check()?;
        }
        anyhow::ensure!(
            plan.rows == lib.reader.physical.data.vector_rows,
            "actual complete tuple vector-row count differs"
        );
        if plan.rows == 0 {
            return Ok(Reservation {
                memory_bytes: 0,
                temporary_bytes: 0,
            });
        }
        Ok(plan.query_requirement(&model_id, limit)?)
    }
    /// Execute one whole-library semantic pass for a requested keyword, streaming `sources.rows`
    /// and M1 pages in lockstep with `rows.bin` without rescanning the vector index per partition.
    pub fn start_semantic(
        &self,
        service: &NativeLibraryService,
        claim: Arc<Claim>,
        library: usize,
        keyword: usize,
    ) -> NativeWholeSemanticRun {
        self.start_semantic_for(&service.library, claim, library, keyword)
    }
    /// Use the already-held selected library for controller routing without a host mutex.
    pub(in crate::native_service) fn start_semantic_for(
        &self,
        library_id: &wire::Id,
        claim: Arc<Claim>,
        library: usize,
        keyword: usize,
    ) -> NativeWholeSemanticRun {
        let mut run = NativeWholeSemanticRun {
            query: NativeTupleQuery {
                cause: self.cause.clone(),
            },
            claim,
            library,
            data: Some(Box::new(WholeData::new())),
            embedding: None,
            base: None,
        };
        if let Err(error) = run.execute(library_id, keyword) {
            run.note_failure(error);
        }
        run
    }
}
impl NativeWholeSemanticRun {
    /// Return the selected complete tuple's checked model-space label as response data.
    pub(in crate::native_service) fn model_space(&self) -> Result<&str> {
        self.check()?;
        Ok(&self.query.cause.libraries[self.library]
            .reader
            .physical
            .data
            .model
            .as_ref()
            .context("actual tuple model identity")?
            .embedding_space)
    }
    /// Coverage from this one source stream, before tag filtering or distance work.
    pub(in crate::native_service) fn coverage(&self) -> Result<wire::Coverage> {
        self.check()?;
        Ok(self
            .data
            .as_ref()
            .context("actual semantic owner")?
            .coverage
            .wire())
    }
    /// Resolve one winning representation span through its actual held tuple/source/Claim.
    pub(in crate::native_service) fn evidence_locator(
        &self,
        id: &str,
        digest: &str,
        start: u64,
        end: u64,
    ) -> Result<wire::Locator> {
        let source = self
            .data
            .as_ref()
            .context("actual semantic owner")?
            .sources
            .get(id)
            .context("actual semantic winner")?;
        super::super::controller::representation_locator(
            &self.query,
            &self.claim,
            source,
            digest,
            start,
            end,
        )
    }
    fn note_failure(&mut self, error: anyhow::Error) {
        let error = if self.query.cause.reads.check().is_err() {
            error.context(QueryBudgetExhausted)
        } else {
            error
        };
        let data = self.data.as_mut().expect("actual owner");
        if data.failure.is_none() {
            data.failure = Some(error);
        }
    }
    fn capture_failed_embedding(&mut self, embedding: NativeQueryEmbedding) -> anyhow::Error {
        let exhausted = embedding
            .failure()
            .is_some_and(|error| error.is::<QueryBudgetExhausted>());
        let message = embedding
            .failure()
            .map(|error| format!("{error:#}"))
            .unwrap_or_else(|| "actual native embedding refused".to_owned());
        let recorded = if exhausted {
            anyhow::Error::msg(message).context(QueryBudgetExhausted)
        } else {
            anyhow::Error::msg(message)
        };
        match embedding.finish() {
            Ok(_) => recorded,
            Err(embedding) => {
                self.embedding = Some(embedding);
                recorded
            }
        }
    }
    fn execute(&mut self, library_id: &wire::Id, keyword: usize) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        anyhow::ensure!(
            self.library < self.query.cause.libraries.len(),
            "actual Query library"
        );
        let wire::Command::Query { query } = self.query.cause.libraries[self.library]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required");
        };
        anyhow::ensure!(
            *library_id
                == self.query.cause.libraries[self.library]
                    .reader
                    .physical
                    .library,
            "actual library_id/library owner differs"
        );
        anyhow::ensure!(
            query.methods.contains(&wire::SearchMethod::Semantic),
            "actual selected semantic method required"
        );
        anyhow::ensure!(
            keyword < query.keywords.len(),
            "actual Query keyword required"
        );
        let bytes = query
            .limit
            .get()
            .checked_add(64)
            .and_then(|n| n.checked_mul(2048))
            .and_then(|n| n.checked_add(64 * 1024))
            .context("semantic administration overflow")?;
        self.base = Some(Work::split_claim(
            &self.claim,
            self.query.key(),
            Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            },
        )?);
        let mut embedding = self
            .query
            .embed_keyword(self.claim.clone(), self.library, keyword);
        if embedding.failure().is_some() {
            return Err(self.capture_failed_embedding(embedding));
        }
        if let Err(error) = embedding.materialize() {
            self.embedding = Some(Box::new(embedding));
            return Err(error);
        }
        let searched = self.search(library_id, &embedding);
        match embedding.finish() {
            Ok(_) => searched,
            Err(embedding) => {
                self.embedding = Some(embedding);
                match searched {
                    Ok(()) => Err(anyhow::anyhow!(
                        "actual native query embedding remains foreground-owned"
                    )),
                    Err(error) => Err(error),
                }
            }
        }
    }
    fn sources_len(&self) -> Result<u64> {
        let library = &self.query.cause.libraries[self.library];
        let file = library.reader.physical.open_verified_admitted(
            "sources.rows",
            || library.reader.check(),
            &|n| self.query.cause.admit_io(&self.claim, n),
        )?;
        Ok(file.metadata()?.len())
    }
    fn search(&mut self, library_id: &wire::Id, embedding: &NativeQueryEmbedding) -> Result<()> {
        let vector_rows = self.query.cause.libraries[self.library]
            .reader
            .physical
            .data
            .vector_rows;
        let sources_len = self.sources_len()?;
        if vector_rows == 0 {
            self.walk_sources(library_id, sources_len)?;
            self.data.as_mut().expect("actual owner").empty = true;
            return Ok(());
        }
        let reader_need = if self.query.cause.execution_base.get().is_none() {
            self.query.vector_requirement(self.library)?
        } else {
            self.query
                .live_vector_requirement(&self.claim, self.library)?
        };
        let range_need = HeldRangeReaders::requirement(1024 * 1024)?;
        let range_work = Work::split_claim(&self.claim, self.query.key(), range_need)?;
        let readers = match HeldRangeReaders::new_owning(
            range_work,
            1024 * 1024,
            Control::new(self.claim.deadline().min(self.query.cause.deadline)),
        ) {
            Ok(readers) => readers,
            Err((work, error)) => {
                self.data.as_mut().expect("actual owner").extra.push(work);
                return Err(error.into());
            }
        };
        let header = &self.query.cause.libraries[self.library]
            .reader
            .physical
            .data;
        let generation = header
            .vector_generation
            .clone()
            .context("actual tuple vector generation")?;
        let model = header
            .model
            .clone()
            .context("actual tuple semantic model")?;
        let owner = Arc::new(WholeVectorOwner {
            readers: std::sync::Mutex::new(Some(readers)),
            cause: self.query.cause.clone(),
            library: self.library,
            claim: self.claim.clone(),
            prefix: format!("vectors/{generation}/"),
        });
        self.data.as_mut().expect("actual owner").vectors = Some(owner.clone());
        let reader_work = Arc::new(Work::split_claim(
            &self.claim,
            self.query.key(),
            reader_need,
        )?);
        self.data.as_mut().expect("actual owner").reader_work = Some(reader_work.clone());
        self.query.cause.admit_object(&self.claim)?;
        for _ in 0..vector_rows {
            self.query.cause.admit_object(&self.claim)?;
        }
        let directory: Arc<dyn ByteDirectory> = Arc::new(WholeVectorComponents { owner });
        let store = VectorStore::open_directory_held(
            &directory,
            &generation,
            &model,
            &reader_work,
            Some(vector_rows),
        )?;
        let limit = {
            let wire::Command::Query { query } = self.query.cause.libraries[self.library]
                .reader
                .permit
                .admitted_command()
                .map_err(refused_error)?
            else {
                bail!("actual Query required");
            };
            query.limit.get() as usize
        };
        let vector = embedding.vector_for(&self.query.cause, &self.claim, self.library)?;
        let result = {
            let mut stream = Stream {
                run: self,
                library_id,
                cursor: 0,
                sources_len,
                current: None,
                active: None,
                callback_error: None,
            };
            #[cfg(test)]
            TEST_WHOLE_SEMANTIC_PASSES.with(|count| count.set(count.get() + 1));
            let queried = store.query_rows(vector, limit, |row| stream.accept(row));
            if let Some(error) = stream.callback_error.take() {
                return Err(error);
            }
            let result = queried?;
            stream.finish_pass(&result)?;
            result
        };
        self.data.as_mut().expect("actual owner").result = Some(result);
        self.collect_evidence()?;
        Ok(())
    }
    fn walk_sources(&mut self, library_id: &wire::Id, sources_len: u64) -> Result<()> {
        let mut stream = Stream {
            run: self,
            library_id,
            cursor: 0,
            sources_len,
            current: None,
            active: None,
            callback_error: None,
        };
        stream.drain_tail()?;
        if let Some((_, source)) = stream.active.take() {
            stream.run.close_source(source)?;
        }
        Ok(())
    }
    fn collect_evidence(&mut self) -> Result<()> {
        let rows = self.query.cause.libraries[self.library]
            .reader
            .physical
            .data
            .vector_rows;
        for _ in 0..rows {
            self.query.cause.admit_object(&self.claim)?;
        }
        let sources = std::mem::take(&mut self.data.as_mut().expect("actual owner").sources);
        let result = self
            .data
            .as_mut()
            .expect("actual owner")
            .result
            .take()
            .context("actual semantic result")?;
        let mut deferred = None;
        let visited = result.visit_rows(
            |row| match sources.get(row.source_id) {
                Some(source) => source.source().check().map(|_| true).map_err(|_| {
                    VectorError::Identity("actual native evidence source unavailable".into())
                }),
                None => Ok(false),
            },
            |hit| {
                if deferred.is_some() {
                    return Err(VectorError::Identity(
                        "actual native evidence consumer refused".into(),
                    ));
                }
                if let Err(error) = self.charge_evidence(hit) {
                    deferred = Some(error);
                    return Err(VectorError::Identity(
                        "actual native evidence consumer refused".into(),
                    ));
                }
                Ok(())
            },
        );
        {
            let data = self.data.as_mut().expect("actual owner");
            data.sources = sources;
            data.result = Some(result);
        }
        if let Some(error) = deferred {
            return Err(error);
        }
        visited?;
        Ok(())
    }
    fn charge_evidence(&mut self, hit: &VectorEvidence) -> Result<()> {
        let bytes = (hit.source_id.len()
            + hit.source_digest.len()
            + hit.representation.digest.len()
            + hit.representation.processor.len()
            + hit.locator.len()) as u64;
        let charge = bytes
            .checked_mul(16)
            .and_then(|n| n.checked_add(std::mem::size_of::<VectorEvidence>() as u64 + 1024))
            .context("evidence copy overflow")?;
        let (used, capacity) = {
            let data = self.data.as_ref().expect("actual owner");
            (data.metadata_used, data.metadata_capacity)
        };
        let peak = used
            .checked_add(charge)
            .context("evidence metadata overflow")?;
        if peak > capacity {
            let next = peak
                .max(
                    capacity
                        .checked_add(capacity / 2)
                        .context("metadata growth overflow")?,
                )
                .max(64 * 1024);
            let work = Work::split_claim(
                &self.claim,
                self.query.key(),
                Reservation {
                    memory_bytes: next - capacity,
                    temporary_bytes: 0,
                },
            )?;
            let data = self.data.as_mut().expect("actual owner");
            data.extra.push(work);
            data.metadata_capacity = next;
        }
        let owned = hit.clone();
        let data = self.data.as_mut().expect("actual owner");
        data.metadata_used = data
            .metadata_used
            .checked_add(charge)
            .context("evidence metadata overflow")?;
        data.evidence
            .entry(owned.source_id.clone())
            .or_default()
            .push(owned);
        Ok(())
    }
    fn read_inventory_page(&mut self, offset: u64) -> Result<NativeInventoryPage> {
        let budget = self.claim.budget();
        let funding = Account::current()?.split_claim(
            &self.claim,
            self.query.key(),
            Reservation {
                memory_bytes: PAGE_MEMORY + 128 * 1024,
                temporary_bytes: 0,
            },
        )?;
        self.query.page(
            &self.claim,
            self.library,
            offset,
            Budget {
                max_files: 64.min(budget.max_files),
                max_bytes: budget.max_bytes,
                max_millis: budget.max_millis,
                max_memory_bytes: PAGE_MEMORY,
            },
            funding,
        )
    }
    fn release_page(&mut self) -> Result<()> {
        let data = self.data.as_mut().expect("actual owner");
        if let Some((_, page)) = data.page.take() {
            match page.into_funding() {
                Ok(mut credit) => {
                    if let Err(error) = Account::current().and_then(|account| {
                        account.rejoin_claim(&mut credit, &self.claim, self.query.key())
                    }) {
                        data.credits.push(credit);
                        return Err(error.into());
                    }
                }
                Err(page) => data.pinned_pages.push(page),
            }
        }
        Ok(())
    }
    fn collect_pages(&mut self) -> Result<()> {
        let pages = std::mem::take(&mut self.data.as_mut().expect("actual owner").pinned_pages);
        let mut pages = pages.into_iter();
        while let Some(page) = pages.next() {
            match page.into_funding() {
                Err(page) => self
                    .data
                    .as_mut()
                    .expect("actual owner")
                    .pinned_pages
                    .push(page),
                Ok(mut credit) => {
                    if let Err(error) = Account::current().and_then(|account| {
                        account.rejoin_claim(&mut credit, &self.claim, self.query.key())
                    }) {
                        let data = self.data.as_mut().expect("actual owner");
                        data.credits.push(credit);
                        data.pinned_pages.extend(pages);
                        return Err(error.into());
                    }
                }
            }
        }
        Ok(())
    }
    fn observed_for(
        &mut self,
        page_offset: u64,
        id: &str,
        locator: &str,
    ) -> Result<Option<NativeObservedSource>> {
        let scan_drift = {
            let library = &self.query.cause.libraries[self.library];
            library.inventory.scan() != library.reader.physical.data.scan
        };
        let mismatched = self
            .data
            .as_ref()
            .expect("actual owner")
            .page
            .as_ref()
            .is_none_or(|(offset, _)| *offset != page_offset);
        if mismatched {
            self.release_page()?;
            if !scan_drift {
                let page = self.read_inventory_page(page_offset)?;
                self.data.as_mut().expect("actual owner").page = Some((page_offset, page));
            }
        }
        if let Some(found) = self.find_observed(id, locator) {
            return Ok(Some(found));
        }
        if !scan_drift {
            bail!("actual M1 page source absent");
        }
        self.release_page()?;
        let mut offset = 0u64;
        loop {
            let page = self.read_inventory_page(offset)?;
            let found = page
                .sources()
                .find(|source| same_observed(source, id, locator));
            let complete = page.complete();
            let next_offset = page.next_offset();
            self.data.as_mut().expect("actual owner").page = Some((offset, page));
            if found.is_some() {
                return Ok(found);
            }
            if complete {
                return Ok(None);
            }
            anyhow::ensure!(next_offset > offset, "inventory replay made no progress");
            self.release_page()?;
            offset = next_offset;
        }
    }
    fn find_observed(&self, id: &str, locator: &str) -> Option<NativeObservedSource> {
        self.data
            .as_ref()?
            .page
            .as_ref()?
            .1
            .sources()
            .find(|source| same_observed(source, id, locator))
    }
    fn close_source(&mut self, source: NativeQuerySource) -> Result<()> {
        match self.query.finish_source(source, &self.claim) {
            Ok(()) => Ok(()),
            Err(wire::authority::ScopeRejoinError::Retained { scope, code }) => {
                self.data
                    .as_mut()
                    .expect("actual owner")
                    .scopes
                    .push(*scope);
                Err(refused_error(code))
            }
            Err(wire::authority::ScopeRejoinError::Released(code)) => Err(refused_error(code)),
        }
    }
    fn rejoin_work(&mut self, mut work: Work) -> bool {
        if work.rejoin(&self.claim, self.query.key()).is_err() {
            self.data.as_mut().expect("actual owner").extra.push(work);
            false
        } else {
            true
        }
    }
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.data.as_ref()?.failure.as_ref()
    }
    /// Fixed typed global data-budget refusal, never an unavailable source row.
    pub fn budget_exhausted(&self) -> bool {
        self.failure()
            .is_some_and(|error| error.is::<QueryBudgetExhausted>())
    }
    pub fn check(&self) -> Result<()> {
        let data = self
            .data
            .as_ref()
            .context("native semantic result consumed")?;
        anyhow::ensure!(
            !data.closing && data.failure.is_none(),
            "native semantic result is not deliverable"
        );
        self.query.cause.check_claim(&self.claim)?;
        if let Some(result) = &data.result {
            result.check()?;
        }
        anyhow::ensure!(
            data.empty || data.result.is_some(),
            "actual native semantic outcome required"
        );
        for source in data.sources.values() {
            source.source().check()?;
        }
        Ok(())
    }
    /// Bounded distinct original file ranks for this whole-library semantic pass.
    pub fn hits(&self) -> Result<&[(String, f32)]> {
        self.check()?;
        Ok(self
            .data
            .as_ref()
            .expect("actual owner")
            .result
            .as_ref()
            .map(|result| result.as_slice())
            .unwrap_or(&[]))
    }
    /// Mature evidence locations retained for one winning original file.
    pub fn evidence_for(&self, source_id: &str) -> Result<&[VectorEvidence]> {
        self.check()?;
        Ok(self
            .data
            .as_ref()
            .expect("actual owner")
            .evidence
            .get(source_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]))
    }
    /// Canonical annotations for one retained source, after current permit checks.
    pub fn source_annotations(&self, source_id: &str) -> Result<Option<&NativeQueryAnnotations>> {
        self.check()?;
        Ok(self
            .data
            .as_ref()
            .expect("actual owner")
            .sources
            .get(source_id)
            .map(NativeQuerySource::annotations))
    }
    /// Actual stale/missing cached representation parts omitted before cosine distance.
    pub fn unavailable_parts(&self) -> u64 {
        self.data.as_ref().map_or(0, |data| data.unavailable_parts)
    }
    /// Conservative admitted data bytes, including prior input metadata, NOT completed-IO/RSS.
    pub fn bytes_admitted(&self) -> u64 {
        self.query.cause.reads.used()
    }
    /// Conservative source/metadata examinations, including unavailable entries.
    pub fn objects_admitted(&self) -> u64 {
        self.query.cause.reads.amount().objects
    }
    #[cfg(test)]
    pub fn test_whole_passes() -> u64 {
        TEST_WHOLE_SEMANTIC_PASSES.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_component_opens(&self) -> u64 {
        super::test_component_open_count()
    }
    #[cfg(test)]
    pub fn test_scope_clone(&self) -> Option<Permit> {
        self.data
            .as_ref()?
            .sources
            .values()
            .next()
            .map(|source| source.source.test_query_scope_clone())
    }
    /// Transfer winning source owners to the all-method controller before `finish`.
    pub(in crate::native_service) fn take_sources(
        &mut self,
    ) -> std::collections::BTreeMap<String, NativeQuerySource> {
        self.data
            .as_mut()
            .map(|data| std::mem::take(&mut data.sources))
            .unwrap_or_default()
    }
    /// Pages kept alive by transferred semantic observers. `finish` cannot recover them.
    pub(in crate::native_service) fn take_retained_pages(&mut self) -> Vec<NativeInventoryPage> {
        let Some(data) = self.data.as_mut() else {
            return Vec::new();
        };
        let mut pages = std::mem::take(&mut data.pinned_pages);
        if let Some((_, page)) = data.page.take() {
            pages.push(page);
        }
        pages
    }
    /// Destroy evidence, the vector result, and errors, then close sources and rejoin loans.
    /// A live scope, page, reader, or embedding returns this owner.
    pub fn finish(mut self) -> std::result::Result<(NativeTupleQuery, Arc<Claim>), Box<Self>> {
        if let Some(data) = self.data.as_mut() {
            data.closing = true;
            data.evidence.clear();
            drop(data.result.take());
            if let Some(error) = data.failure.take() {
                match error.downcast::<NativeQueryPageRefusal>() {
                    Ok(owned) => {
                        let (cause, loan) = owned.into_parts();
                        data.failure = Some(cause);
                        match loan {
                            NativeQueryPageLoan::Funding(holding) => data.credits.push(holding),
                            NativeQueryPageLoan::Page(page) => data.pinned_pages.push(page),
                        }
                    }
                    Err(error) => {
                        match error.downcast::<crate::native_service::NativeRefreshScopeError>() {
                            Ok(owned) => match owned.scope {
                                wire::authority::ScopeRejoinError::Retained { scope, .. } => {
                                    data.scopes.push(*scope)
                                }
                                wire::authority::ScopeRejoinError::Released(_) => {}
                            },
                            Err(error) => drop(error),
                        }
                    }
                }
            }
        }
        if self.data.is_some() {
            let mut pending =
                std::mem::take(&mut self.data.as_mut().expect("actual owner").pending);
            pending.extend(self.take_sources().into_values());
            for source in pending {
                if let Err(error) = self.close_source(source) {
                    self.data.as_mut().expect("actual owner").failure = Some(error);
                }
            }
            let scopes = std::mem::take(&mut self.data.as_mut().expect("actual owner").scopes);
            for scope in scopes {
                match scope.rejoin_processing_scope(&self.claim, self.query.key()) {
                    Ok(()) => {}
                    Err(wire::authority::ScopeRejoinError::Retained { scope, .. }) => self
                        .data
                        .as_mut()
                        .expect("actual owner")
                        .scopes
                        .push(*scope),
                    Err(wire::authority::ScopeRejoinError::Released(_)) => {}
                }
            }
            if !self.data.as_ref().expect("actual owner").scopes.is_empty() {
                return Err(Box::new(self));
            }
            if self.release_page().is_err() || self.collect_pages().is_err() {
                return Err(Box::new(self));
            }
            if !self
                .data
                .as_ref()
                .expect("actual owner")
                .pinned_pages
                .is_empty()
            {
                return Err(Box::new(self));
            }
            if let Some(work) = self.data.as_mut().expect("actual owner").reader_work.take() {
                match Arc::try_unwrap(work) {
                    Ok(work) => {
                        if !self.rejoin_work(work) {
                            return Err(Box::new(self));
                        }
                    }
                    Err(work) => {
                        self.data.as_mut().expect("actual owner").reader_work = Some(work);
                        return Err(Box::new(self));
                    }
                }
            }
            if let Some(owner) = self.data.as_mut().expect("actual owner").vectors.take() {
                match Arc::try_unwrap(owner) {
                    Ok(owner) => match recover_ranges(owner) {
                        Ok(Some(work)) => {
                            if !self.rejoin_work(work) {
                                return Err(Box::new(self));
                            }
                        }
                        Ok(None) => {}
                        Err(owner) => {
                            self.data.as_mut().expect("actual owner").vectors =
                                Some(Arc::from(owner));
                            return Err(Box::new(self));
                        }
                    },
                    Err(owner) => {
                        self.data.as_mut().expect("actual owner").vectors = Some(owner);
                        return Err(Box::new(self));
                    }
                }
            }
            let mut data = self.data.take().expect("actual owner");
            drop(data.failure.take());
            let mut extra = std::mem::take(&mut data.extra).into_iter();
            while let Some(mut work) = extra.next() {
                if work.rejoin(&self.claim, self.query.key()).is_err() {
                    data.extra.push(work);
                    data.extra.extend(extra);
                    self.data = Some(data);
                    return Err(Box::new(self));
                }
            }
            let mut credits = std::mem::take(&mut data.credits).into_iter();
            while let Some(mut credit) = credits.next() {
                if Account::current()
                    .and_then(|account| {
                        account.rejoin_claim(&mut credit, &self.claim, self.query.key())
                    })
                    .is_err()
                {
                    data.credits.push(credit);
                    data.credits.extend(credits);
                    self.data = Some(data);
                    return Err(Box::new(self));
                }
            }
            drop(data);
        }
        if let Some(embedding) = self.embedding.take()
            && let Err(embedding) = embedding.finish()
        {
            self.embedding = Some(embedding);
            return Err(Box::new(self));
        }
        if let Some(mut base) = self.base.take()
            && base.rejoin(&self.claim, self.query.key()).is_err()
        {
            self.base = Some(base);
            return Err(Box::new(self));
        }
        Ok((self.query, self.claim))
    }
}
fn recover_ranges(
    owner: WholeVectorOwner,
) -> std::result::Result<Option<Work>, Box<WholeVectorOwner>> {
    let WholeVectorOwner {
        readers,
        cause,
        library,
        claim,
        prefix,
    } = owner;
    let readers = readers
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match readers {
        Some(readers) => match readers.into_work() {
            Ok(work) => Ok(Some(work)),
            Err(readers) => Err(Box::new(WholeVectorOwner {
                readers: std::sync::Mutex::new(Some(readers)),
                cause,
                library,
                claim,
                prefix,
            })),
        },
        None => Ok(None),
    }
}
