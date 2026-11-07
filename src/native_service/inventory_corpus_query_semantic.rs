//! SAME native tuple/model/Claim and mature vector engine over one actual bounded source set.
//! Not a whole-library/all-method controller, current authored tag projection, or model quality.
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
        self.result = Some(store.query_rows(vector, query.limit.get() as usize, |row| {
            self.owner.vector_eligible(row)
        })?);
        self.owner.check()
    }
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.failure.as_ref()
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
