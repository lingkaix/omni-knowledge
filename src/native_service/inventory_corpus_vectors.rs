//! Saved actual M3 representations -> the existing affine provider/vector writer.
//! A prepared component never selects a whole-corpus pointer.
use super::*;
use knowledge_processing::embedding::funded::EmbeddingControl;
use knowledge_search::{
    AccountGrant, RepresentationIdentity, VectorBuildPlan, VectorCandidate, VectorError,
    VectorModality, VectorRow, VectorStore,
};

fn representation(part: &SourcePart) -> Result<RepresentationIdentity> {
    let Content::Representation { processor, .. } = &part.document.content else {
        bail!("vector rows require actual M3 representation bytes")
    };
    Ok(RepresentationIdentity {
        digest: part.representation_digest.clone(),
        processor: processor.clone(),
        modality: VectorModality::Text,
    })
}
fn locator(part: &SourcePart, start: u64, end: u64) -> String {
    format!(
        "representation:{}@{}..{}",
        part.representation_digest, start, end
    )
}

pub(super) fn prepare(
    owner: &mut NativeCorpusDraft,
    progress: &mut impl FnMut(NativeCorpusProgress),
) -> Result<()> {
    owner.check()?;
    anyhow::ensure!(
        owner.lexical_done && !owner.vectors_done,
        "vector phase requires unconsumed complete lexical preparation"
    );
    let Some(model) = owner.job.cause.model.as_ref() else {
        owner.vectors_done = true;
        return Ok(());
    };
    let identity = model.model()?.clone();
    anyhow::ensure!(
        owner.provider.is_some(),
        "actual provider must have been prepaid before M3"
    );
    let files = owner.files.as_ref().context("actual saved corpus")?;
    let version = stamp(&files.tape)?;
    let records = stamp(&files.sources)?;
    let end = files.sources.metadata()?.len();
    let mut position = 0;
    let mut plan = VectorBuildPlan {
        rows: 0,
        metadata_bytes: 0,
        max_row_bytes: 1,
    };
    while position < end {
        owner.check()?;
        let (part, next) =
            read_frame::<SourcePart>(&owner.files.as_ref().expect("files").sources, position)?;
        position = next;
        if part.embedding_rows == 0 {
            continue;
        }
        let repr = representation(&part)?;
        let loc = locator(&part, u64::MAX, u64::MAX);
        plan.include(
            VectorCandidate {
                source_id: &part.document.key.id,
                source_digest: &part.document.digest,
                representation: &repr,
                locator: &loc,
            },
            part.embedding_rows,
        )?;
    }
    anyhow::ensure!(
        plan.rows == owner.rows,
        "actual saved representation row count disagrees"
    );
    let need = plan.requirement(&identity)?;
    owner.phase_storage(need.temporary_bytes, progress)?;
    let work = Work::split_claim(owner.actual_claim()?, owner.job.key(), need)?;
    let grant = AccountGrant::from_work(work);
    let path = owner
        .files
        .as_ref()
        .expect("files")
        .stage
        .path()
        .join("vectors");
    let deadline = owner.actual_claim()?.deadline();
    let prepared = {
        let rows = Rows {
            owner,
            position: 0,
            end,
            version,
            records,
            page_offset: None,
            source: None,
            part: None,
            text: String::new(),
            cursor: 0,
            done: false,
        };
        VectorStore::prepare_stream(&path, &identity, &plan, rows, &grant, Some(deadline))
    };
    if let Some(error) = owner
        .source_errors
        .lock()
        .map_err(|_| anyhow::anyhow!("actual source errors unavailable"))?
        .pop()
    {
        return Err(error);
    }
    let prepared = prepared?;
    owner.check()?;
    prepared.check()?;
    owner.vectors = Some(Arc::new(prepared));
    owner.vectors_done = true;
    progress(NativeCorpusProgress::VectorsPrepared { rows: plan.rows });
    Ok(())
}

struct ProviderControl<'a> {
    processing: &'a NativeProcessing,
    claim: &'a Claim,
}
impl EmbeddingControl for ProviderControl<'_> {
    fn checkpoint(&self) -> Result<()> {
        self.processing.check(self.claim)
    }
    fn deadline(&self) -> Instant {
        self.claim.deadline()
    }
}
struct Rows<'a> {
    owner: &'a mut NativeCorpusDraft,
    position: u64,
    end: u64,
    version: [u64; 8],
    records: [u64; 8],
    page_offset: Option<u64>,
    source: Option<NativeProcessing>,
    part: Option<SourcePart>,
    text: String,
    cursor: usize,
    done: bool,
}
impl Rows<'_> {
    fn check(&self) -> Result<()> {
        self.owner.check()?;
        let files = self.owner.files.as_ref().context("actual saved files")?;
        anyhow::ensure!(
            stamp(&files.tape)? == self.version && stamp(&files.sources)? == self.records,
            "saved corpus changed"
        );
        if let Some(source) = &self.source {
            source.check(self.owner.actual_claim()?)?;
        }
        Ok(())
    }
    fn finish_source(&mut self) -> Result<()> {
        if let Some(source) = self.source.take() {
            let checked = source.check(self.owner.actual_claim()?);
            let finished = self.owner.finish_processing(source);
            checked?;
            finished?;
        }
        Ok(())
    }
    fn finish_page(&mut self) -> Result<()> {
        self.finish_source()?;
        if let Some(page) = self.owner.page.take() {
            let page = Arc::try_unwrap(page).map_err(|p| {
                self.owner.page = Some(p);
                anyhow::anyhow!("actual vector source page still owned")
            })?;
            let mut funding = page.into_funding().map_err(|p| {
                self.owner.page = Some(Arc::new(p));
                anyhow::anyhow!("actual vector page loan still owned")
            })?;
            Account::current()?.rejoin_claim(
                &mut funding,
                self.owner.actual_claim()?,
                self.owner.job.key(),
            )?;
        }
        self.page_offset = None;
        Ok(())
    }
    fn load(&mut self) -> Result<bool> {
        self.check()?;
        while self.position < self.end {
            let (part, next) = read_frame::<SourcePart>(
                &self.owner.files.as_ref().expect("files").sources,
                self.position,
            )?;
            self.position = next;
            if part.embedding_rows == 0 {
                continue;
            }
            let same = self.source.as_ref().is_some_and(|s| {
                let (id, digest, _) = s.source_facts();
                id == part.document.key.id && digest.as_str() == part.document.digest
            });
            if !same {
                self.finish_source()?;
            }
            if self.page_offset != Some(part.page_offset) {
                self.finish_page()?;
                let funding = Account::current()?.split_claim(
                    self.owner.actual_claim()?,
                    self.owner.job.key(),
                    Reservation {
                        memory_bytes: PAGE_MEMORY + 128 * 1024,
                        temporary_bytes: 0,
                    },
                )?;
                let budget = Budget {
                    max_files: 64,
                    max_bytes: PAGE_MEMORY,
                    max_millis: self.owner.actual_claim()?.budget().max_millis,
                    max_memory_bytes: PAGE_MEMORY,
                };
                self.owner.page = Some(Arc::new(self.owner.job.page(
                    part.page_offset,
                    budget,
                    funding,
                )?));
                self.page_offset = Some(part.page_offset);
            }
            if self.source.is_none() {
                let observed = self
                    .owner
                    .page
                    .as_ref()
                    .context("actual M1 vector page")?
                    .sources()
                    .nth(part.slot)
                    .context("actual M1 vector row absent")?;
                let source = self.owner.job.processing(
                    self.owner.actual_claim()?,
                    observed,
                    self.owner.actual_claim()?.budget(),
                )?;
                self.source = Some(source);
            }
            let source = self.source.as_ref().expect("actual source");
            let (id, digest, processor) = source.source_facts();
            let repr = representation(&part)?;
            anyhow::ensure!(
                id == part.document.key.id
                    && digest.as_str() == part.document.digest
                    && processor.as_str() == repr.processor,
                "actual source/processor differs from saved M3 representation"
            );
            anyhow::ensure!(
                part.tape_length <= TEXT_WINDOW as u64,
                "saved text window exceeds format"
            );
            let mut bytes = vec![0; usize::try_from(part.tape_length)?];
            self.check()?;
            self.owner
                .files
                .as_ref()
                .expect("files")
                .tape
                .read_exact_at(&mut bytes, part.tape_start)?;
            self.check()?;
            let Content::Representation { digest, size, .. } = &part.document.content else {
                bail!("missing actual saved text")
            };
            anyhow::ensure!(
                *size == bytes.len() as u64 && hex::encode(Sha256::digest(&bytes)) == *digest,
                "actual saved representation checksum differs"
            );
            self.text = String::from_utf8(bytes)?;
            self.cursor = 0;
            self.part = Some(part);
            return Ok(true);
        }
        self.finish_page()?;
        Ok(false)
    }
    fn row(&mut self) -> Result<Option<VectorRow>> {
        loop {
            self.check()?;
            if self.cursor == self.text.len() && !self.load()? {
                return Ok(None);
            }
            let start = self.cursor;
            let mut end = start.saturating_add(48 * 1024).min(self.text.len());
            while !self.text.is_char_boundary(end) {
                end -= 1;
            }
            self.cursor = end;
            let text = &self.text[start..end];
            if text.trim().is_empty() {
                continue;
            }
            let provider = self
                .owner
                .provider
                .take()
                .context("actual affine text provider unavailable")?;
            let source = self
                .source
                .as_ref()
                .context("actual original source owner")?;
            let control = ProviderControl {
                processing: source,
                claim: self.owner.actual_claim()?,
            };
            let embedded = provider.embed_text(text, &control)?;
            let vector = embedded.vector(&control)?.to_vec();
            self.owner
                .job
                .cause
                .model
                .as_ref()
                .context("actual selected model")?
                .model()?
                .check_vector(&vector)?;
            let part = self.part.as_ref().expect("actual representation part");
            let row = VectorRow {
                source_id: part.document.key.id.clone(),
                source_digest: part.document.digest.clone(),
                representation: representation(part)?,
                locator: locator(
                    part,
                    part.representation_start + start as u64,
                    part.representation_start + end as u64,
                ),
                vector,
            };
            self.owner.provider = Some(embedded.into_provider());
            self.check()?;
            return Ok(Some(row));
        }
    }
}
impl Iterator for Rows<'_> {
    type Item = std::result::Result<VectorRow, VectorError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.row() {
            Ok(Some(row)) => Some(Ok(row)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(error) => {
                self.done = true;
                self.owner
                    .source_errors
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(error);
                Some(Err(VectorError::Io(std::io::Error::other(
                    "actual native vector producer stopped",
                ))))
            }
        }
    }
}
impl Drop for Rows<'_> {
    fn drop(&mut self) {
        // The synchronous provider has either returned its actual runtime or collected it on error.
        // Source/FD callbacks die before their SDK/page loans are returned.
        if let Err(error) = self.finish_page() {
            self.owner
                .source_errors
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(error);
        }
    }
}
