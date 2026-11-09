//! Bounded, owning ordinary-library BM25 walk over the SAME actual tuple/Claim/partition engine.
//! This is not activation or the all-method controller: semantic/scan/tag/metadata composition and
//! aggregate byte/object budget reporting still need integration before the application uses it.
use super::*;
use knowledge_retrieval::Hit;
use knowledge_search::FileRankWindow;
use std::num::NonZeroUsize;
use std::os::unix::fs::FileExt;

/// A synchronous step never relinquishes this actual foreground owner on failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeQueryStep {
    /// One actual catalog partition advanced; owner remains foreground.
    Progress,
    /// Catalog/source EOF established; retained results still require currentness checks.
    Complete,
    /// Terminal failure retained by the actual owner; do not retry this step.
    Failed,
}
impl NativeQueryStep {
    /// Data-only step classification, not a result or delivery grant.
    pub fn is_complete(self) -> bool {
        self == Self::Complete
    }
    /// The owner retains its actual failure/resources until explicit cleanup.
    pub fn is_failed(self) -> bool {
        self == Self::Failed
    }
}
struct Candidate {
    hit: Hit,
    charged: u64,
}
struct Data {
    window: FileRankWindow,
    rows: BTreeMap<String, Candidate>,
    sources: BTreeMap<String, NativeQuerySource>,
    pending: Vec<NativeQuerySource>,
    scopes: Vec<Permit>,
    page: Option<(u64, NativeInventoryPage)>,
    pinned_pages: Vec<NativeInventoryPage>,
    stage: Option<NativeQueryStage>,
    extra: Vec<Work>,
    credits: Vec<Holding>,
    closing: bool,
    metadata_capacity: u64,
    metadata_used: u64,
    catalog: u64,
    source: u64,
    partitions: u64,
    unavailable_parts: u64,
    coverage: QueryCoverage,
    done: bool,
    failure: Option<anyhow::Error>,
    // Ablation ONLY: actual mature partition lists over this 75-source fixture. Base admin
    // credit pays these small test records; no production collector retains partition lists.
    #[cfg(test)]
    test_partition_lists: Vec<Vec<String>>,
    #[cfg(test)]
    test_raw_order: Option<Vec<String>>,
}
/// Actual source/page/tuple/model/Claim and scalar-result owner, not a list of detached DTOs.
/// `finish` is mandatory after last result delivery; it returns THIS owner if a loan remains live.
pub struct NativeBm25Run {
    query: NativeTupleQuery,
    claim: Arc<Claim>,
    library: usize,
    keyword: usize,
    data: Option<Box<Data>>,
    // Rust drops fields in declaration order: even administrative error bytes die first.
    base: Option<Work>,
}
fn same_observed(source: &NativeObservedSource, id: &str, locator: &str) -> bool {
    source.source_id() == id && source.observation().stat.path.as_str() == locator
}
impl NativeTupleQuery {
    /// Complete conservative BM25 working peak for one library/keyword. Metadata already has its
    /// actual prepaid owner. This quote is DATA: it does not submit, allocate, open originals or grant.
    pub fn bm25_requirement(&self, library: usize) -> Result<Reservation> {
        self.cause.check()?;
        let lib = self
            .cause
            .libraries
            .get(library)
            .context("actual Query library")?;
        let wire::Command::Query { query } = lib
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        let slots = query.limit.get();
        let file = lib.reader.physical.open_verified_admitted(
            "sources.rows",
            || lib.reader.check(),
            &|n| self.cause.admit_planning(n),
        )?;
        let mut at = 0;
        let mut max_scope = 0;
        let mut max_row = 0;
        while at < file.metadata()?.len() {
            lib.reader.check()?;
            self.cause.admit_planning_object()?;
            let (part, next) =
                read_frame_admitted::<SourcePart>(&file, at, &|n| self.cause.admit_planning(n))?;
            let path = part
                .document
                .locator
                .as_ref()
                .context("actual original display path")?;
            let bytes = next.checked_sub(at).context("actual source frame")?;
            let quote = bytes
                .checked_mul(16)
                .and_then(|n| n.checked_add((path.len() as u64).checked_mul(32)?))
                .and_then(|n| n.checked_add(1024 * 1024))
                .context("source scope quote overflow")?;
            max_scope = max_scope.max(quote);
            max_row = max_row.max(bytes);
            at = next;
        }
        let catalog = lib.reader.physical.open_verified_admitted(
            "lexical.rows",
            || lib.reader.check(),
            &|n| self.cause.admit_planning(n),
        )?;
        let mut at = 0;
        let mut width = 0;
        while at < catalog.metadata()?.len() {
            let (shard, next) =
                read_frame_admitted::<Shard>(&catalog, at, &|n| self.cause.admit_planning(n))?;
            width = width.max(shard.parts.len() as u64);
            at = next;
            lib.reader.check()?;
        }
        let winners = slots.min(lib.reader.physical.data.sources);
        let current = max_scope
            .checked_mul(width)
            .context("current source peak overflow")?;
        let retained = max_scope
            .checked_add(PAGE_MEMORY + 128 * 1024)
            .and_then(|n| n.checked_mul(winners))
            .context("retained source peak overflow")?;
        let metadata = max_row
            .checked_add(500 * 4 + 4096)
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| n.checked_mul(winners.checked_add(1)?))
            .and_then(|n| n.checked_mul(2))
            .context("rank metadata peak overflow")?;
        let total = current
            .checked_add(retained)
            .and_then(|n| n.checked_add(metadata))
            .and_then(|n| {
                n.checked_add(
                    NativeQueryStageResources::requirement(1024 * 1024)
                        .ok()?
                        .memory_bytes,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    PAGE_MEMORY
                        + 128 * 1024
                        + Self::metadata_requirement().memory_bytes
                        + NativeTupleRead::requirement().memory_bytes
                        + 4 * 1024 * 1024,
                )
            })
            .context("whole native query peak overflow")?;
        Ok(Reservation {
            memory_bytes: total,
            temporary_bytes: 0,
        })
    }
    /// Start ONLY a requested BM25 keyword in an actual selected ordinary library. Whole caller
    /// admission must include the requirement above; every later copy still splits real Claim credit.
    pub fn start_bm25(
        &self,
        claim: Arc<Claim>,
        library: usize,
        keyword: usize,
    ) -> Result<NativeBm25Run> {
        self.cause.check_claim(&claim)?;
        let lib = self
            .cause
            .libraries
            .get(library)
            .context("actual Query library")?;
        let wire::Command::Query { query } = lib
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        anyhow::ensure!(
            query.methods.contains(&wire::SearchMethod::Bm25) && keyword < query.keywords.len(),
            "requested BM25 keyword required"
        );
        let limit = query.limit.get() as usize;
        let bytes = (std::mem::size_of::<NativeQuerySource>() as u64
            + std::mem::size_of::<Candidate>() as u64
            + 1024)
            .checked_mul(
                (limit as u64)
                    .checked_add(64)
                    .context("rank slots overflow")?,
            )
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(64 * 1024))
            .context("rank administration overflow")?;
        let base = Work::split_claim(
            &claim,
            self.key(),
            Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            },
        )?;
        Ok(NativeBm25Run {
            query: NativeTupleQuery {
                cause: self.cause.clone(),
            },
            claim,
            library,
            keyword,
            base: Some(base),
            data: Some(Box::new(Data {
                window: FileRankWindow::new(limit),
                rows: BTreeMap::new(),
                sources: BTreeMap::new(),
                pending: Vec::new(),
                scopes: Vec::new(),
                page: None,
                pinned_pages: Vec::new(),
                stage: None,
                extra: Vec::new(),
                credits: Vec::new(),
                closing: false,
                metadata_capacity: 0,
                metadata_used: 0,
                catalog: 0,
                source: 0,
                partitions: 0,
                unavailable_parts: 0,
                coverage: QueryCoverage::default(),
                done: false,
                failure: None,
                #[cfg(test)]
                test_partition_lists: Vec::new(),
                #[cfg(test)]
                test_raw_order: None,
            })),
        })
    }
}
impl NativeBm25Run {
    #[cfg(test)]
    pub fn test_frame_read_bytes(&self) -> u64 {
        TEST_FRAME_READ_BYTES.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_component_opens(&self) -> u64 {
        partition::test_component_open_count()
    }
    #[cfg(test)]
    pub fn test_part_reads(&self) -> u64 {
        NativeTupleRead::test_part_read_count()
    }
    #[cfg(test)]
    pub fn test_owned_page_objects(&self) -> u64 {
        self.data.as_ref().map_or(0, |d| {
            d.page.as_ref().map_or(0, |(_, p)| p.read_objects())
                + d.pinned_pages
                    .iter()
                    .map(NativeInventoryPage::read_objects)
                    .sum::<u64>()
        })
    }
    #[cfg(test)]
    pub fn test_scope_clone(&self) -> Option<Permit> {
        self.data
            .as_ref()?
            .sources
            .values()
            .next()
            .map(|s| s.source.test_query_scope_clone())
    }
    #[cfg(test)]
    pub fn test_paid_admin_tail(&mut self, observed: Arc<std::sync::atomic::AtomicU64>) {
        #[derive(Debug)]
        struct PaidTail {
            bytes: Box<[u8]>,
            observed: Arc<std::sync::atomic::AtomicU64>,
        }
        impl std::fmt::Display for PaidTail {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("actual paid administrative error buffer")
            }
        }
        impl std::error::Error for PaidTail {}
        impl Drop for PaidTail {
            fn drop(&mut self) {
                std::hint::black_box(self.bytes.as_ref());
                self.observed.store(
                    Account::current()
                        .unwrap()
                        .status()
                        .unwrap()
                        .memory_available,
                    std::sync::atomic::Ordering::SeqCst,
                );
            }
        }
        assert!(
            self.base
                .as_ref()
                .unwrap()
                .remaining()
                .unwrap()
                .memory_bytes
                >= 64 * 1024
        );
        self.data.as_mut().unwrap().failure = Some(anyhow::Error::new(PaidTail {
            bytes: vec![1u8; 8 * 1024].into_boxed_slice(),
            observed,
        }));
    }
    /// Fixed typed global data-budget refusal, never an unavailable source row.
    pub fn budget_exhausted(&self) -> bool {
        self.failure()
            .is_some_and(|error| error.is::<QueryBudgetExhausted>())
    }
    /// Borrow the retained actual failure without detaching any owning refusal.
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.data.as_ref()?.failure.as_ref()
    }
    /// Conservative admitted data bytes, including prior input metadata, NOT completed-IO/RSS.
    pub fn bytes_admitted(&self) -> u64 {
        self.query.cause.reads.used()
    }
    /// Conservative source/metadata examinations, including input work and unavailable entries.
    /// This is not a distinct returned-file count; failed opaque spans retain their possible work.
    pub fn objects_admitted(&self) -> u64 {
        self.query.cause.reads.amount().objects
    }
    /// Actual catalog partitions examined, not an invented original-file count.
    pub fn partitions(&self) -> u64 {
        self.data.as_ref().map_or(0, |d| d.partitions)
    }
    /// Actual stale/missing cached representation parts omitted before native statistics.
    pub fn unavailable_parts(&self) -> u64 {
        self.data.as_ref().map_or(0, |d| d.unavailable_parts)
    }
    fn check(&self) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        let data = self.data.as_ref().context("native result consumed")?;
        for source in data.sources.values() {
            source.source.check()?;
        }
        Ok(())
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
    pub(in crate::native_service) fn take_sources(
        &mut self,
    ) -> BTreeMap<String, NativeQuerySource> {
        self.data
            .as_mut()
            .map(|data| std::mem::take(&mut data.sources))
            .unwrap_or_default()
    }
    /// Pages kept alive by transferred source observers. `finish` cannot recover them.
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
    /// Borrow approved current data only; the actual owner and its credit must outlive delivery.
    pub fn hits(&self) -> Result<impl Iterator<Item = &Hit>> {
        self.check()?;
        let data = self.data.as_ref().context("native result consumed")?;
        anyhow::ensure!(
            data.done && data.failure.is_none() && !data.closing,
            "native query not complete or cleanup started"
        );
        #[cfg(test)]
        let ids = data
            .test_raw_order
            .as_ref()
            .into_iter()
            .flat_map(|ids| ids.iter().map(String::as_str))
            .chain(
                data.window
                    .ranked()
                    .filter(|_| data.test_raw_order.is_none()),
            );
        #[cfg(not(test))]
        let ids = data.window.ranked();
        Ok(ids.filter_map(|id| data.rows.get(id).map(|row| &row.hit)))
    }
    /// A failed step is terminal, not an invitation to retry effects or release actual owners.
    pub fn step(&mut self, service: &NativeLibraryService) -> NativeQueryStep {
        self.step_library(&service.library)
    }
    /// Route through this run's actual selected reader; the ID alone grants no access.
    fn step_library(&mut self, library_id: &wire::Id) -> NativeQueryStep {
        let Some(data) = self.data.as_ref() else {
            return NativeQueryStep::Failed;
        };
        if data.failure.is_some() || data.closing {
            return NativeQueryStep::Failed;
        }
        if data.done {
            return NativeQueryStep::Complete;
        }
        match self.advance(library_id) {
            Ok(true) => NativeQueryStep::Complete,
            Ok(false) => NativeQueryStep::Progress,
            Err(error) => {
                let error = if self.query.cause.reads.check().is_err() {
                    // Preserve any original owning refusal beneath the typed budget context.
                    error.context(QueryBudgetExhausted)
                } else {
                    error
                };
                self.data.as_mut().expect("actual owner").failure = Some(error);
                NativeQueryStep::Failed
            }
        }
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
                    if let Err(error) = Account::current()
                        .and_then(|a| a.rejoin_claim(&mut credit, &self.claim, self.query.key()))
                    {
                        data.credits.push(credit);
                        return Err(error.into());
                    }
                }
                Err(page) => data.pinned_pages.push(page),
            }
        }
        Ok(())
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
    fn advance(&mut self, library_id: &wire::Id) -> Result<bool> {
        self.check()?;
        let library = &self.query.cause.libraries[self.library];
        anyhow::ensure!(
            *library_id == library.reader.physical.library,
            "actual library_id/library owner differs"
        );
        let at = self.data.as_ref().expect("actual owner").catalog;
        let catalog = library.reader.physical.open_verified_admitted(
            "lexical.rows",
            || library.reader.check(),
            &|n| self.query.cause.admit_io(&self.claim, n),
        )?;
        if at == catalog.metadata()?.len() {
            let source = library.reader.physical.open_verified_admitted(
                "sources.rows",
                || library.reader.check(),
                &|n| self.query.cause.admit_io(&self.claim, n),
            )?;
            anyhow::ensure!(
                self.data.as_ref().expect("actual owner").source == source.metadata()?.len(),
                "complete catalog omitted source receipts"
            );
            self.release_page()?;
            #[cfg(test)]
            if std::env::var_os("OMNI_TEST_PARTITION_LIST_FUSION").is_some() {
                let data = self.data.as_mut().expect("actual owner");
                data.test_raw_order = Some(
                    knowledge_search::fuse_ranks(&data.test_partition_lists)
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect(),
                );
            }
            self.data.as_mut().expect("actual owner").done = true;
            return Ok(true);
        }
        let (shard, next) = read_frame_admitted::<Shard>(&catalog, at, &|n| {
            self.query.cause.admit_io(&self.claim, n)
        })?;
        if self
            .data
            .as_ref()
            .expect("actual owner")
            .page
            .as_ref()
            .is_none_or(|(offset, _)| *offset != shard.page_offset)
        {
            self.release_page()?;
            let page = self.read_inventory_page(shard.page_offset)?;
            self.data.as_mut().expect("actual owner").page = Some((shard.page_offset, page));
        }
        for expected in &shard.parts {
            let cursor = self.data.as_ref().expect("actual owner").source;
            self.query.cause.admit_object(&self.claim)?;
            let (part, after, locator, excluded, scan_drift) = {
                let library = &self.query.cause.libraries[self.library];
                let (part, after) = library
                    .reader
                    .part_admitted(cursor, &|n| self.query.cause.admit_io(&self.claim, n))?
                    .context("selected source cursor absent")?;
                anyhow::ensure!(
                    part.data == *expected && expected.page_offset == shard.page_offset,
                    "source/catalog producer receipts differ"
                );
                let locator = expected
                    .document
                    .locator
                    .as_deref()
                    .context("actual source logical path absent")?
                    .to_owned();
                let excluded = library.inventory.owner.path_excluded(&locator)
                    || matches!(
                        library.reader.permit.check_path(
                            &library.reader.physical.library,
                            &wire::RelativePath::new(&locator)?,
                        ),
                        Err(wire::ErrorCode::NotExposed)
                    );
                let scan_drift = library.inventory.scan() != library.reader.physical.data.scan;
                (part, after, locator, excluded, scan_drift)
            };
            if excluded {
                self.data.as_mut().expect("actual owner").source = after;
                continue;
            }
            let source_id = expected.document.key.id.as_str();
            let first_source = self
                .data
                .as_mut()
                .expect("actual owner")
                .coverage
                .begin(source_id, &expected.status);
            let mut observed = self
                .data
                .as_ref()
                .expect("actual owner")
                .page
                .as_ref()
                .expect("actual page")
                .1
                .sources()
                .find(|s| same_observed(s, source_id, &locator));
            if observed.is_none() && scan_drift {
                self.release_page()?;
                let mut offset = 0u64;
                loop {
                    let page = self.read_inventory_page(offset)?;
                    let found = page
                        .sources()
                        .find(|s| same_observed(s, source_id, &locator));
                    let complete = page.complete();
                    let next_offset = page.next_offset();
                    self.data.as_mut().expect("actual owner").page = Some((offset, page));
                    if found.is_some() {
                        observed = found;
                        break;
                    }
                    if complete {
                        break;
                    }
                    anyhow::ensure!(next_offset > offset, "inventory replay made no progress");
                    self.release_page()?;
                    offset = next_offset;
                }
            }
            let Some(observed) = observed else {
                if scan_drift {
                    if first_source {
                        self.data
                            .as_mut()
                            .expect("actual owner")
                            .coverage
                            .unavailable += 1;
                    }
                    self.data.as_mut().expect("actual owner").unavailable_parts += 1;
                    self.data.as_mut().expect("actual owner").source = after;
                    continue;
                }
                bail!("actual M1 page source absent");
            };
            let result = self.query.source_part(
                library_id,
                &self.claim,
                self.library,
                observed,
                part,
                after,
            );
            match result {
                Ok(source) => {
                    if first_source {
                        self.data.as_mut().expect("actual owner").coverage.verified += 1;
                    }
                    match source.matches_required_tags() {
                        Ok(true) => self
                            .data
                            .as_mut()
                            .expect("actual owner")
                            .pending
                            .push(source),
                        Ok(false) => {
                            self.close_source(source)?;
                            self.data.as_mut().expect("actual owner").source = after;
                            continue;
                        }
                        Err(error) => {
                            self.close_source(source)?;
                            return Err(error);
                        }
                    }
                }
                Err(error) => {
                    // An actual owning scope refusal is NEVER downgraded to an omitted DTO row.
                    if error
                        .downcast_ref::<crate::native_service::NativeRefreshScopeError>()
                        .is_none()
                        && error.chain().any(|e| {
                            e.is::<crate::native_service::source::CachedQuerySourceUnavailable>()
                        })
                    {
                        if first_source {
                            self.data
                                .as_mut()
                                .expect("actual owner")
                                .coverage
                                .unavailable += 1;
                        }
                        self.data.as_mut().expect("actual owner").unavailable_parts += 1;
                    } else {
                        return Err(error);
                    }
                }
            }
            self.data.as_mut().expect("actual owner").source = after;
        }
        if self.data.as_ref().expect("actual owner").pending.is_empty() {
            let data = self.data.as_mut().expect("actual owner");
            data.catalog = next;
            data.partitions += 1;
            return Ok(false);
        }
        let pending = std::mem::take(&mut self.data.as_mut().expect("actual owner").pending);
        let stage = match self.query.stage(
            self.claim.clone(),
            self.library,
            pending,
            NativeQueryStageResources {
                window: 1024 * 1024,
            },
        ) {
            Ok(stage) => stage,
            Err(refused) => {
                let (cause, loans) = refused.into_parts();
                let data = self.data.as_mut().expect("actual owner");
                data.pending = loans.sources;
                if let Some(work) = loans.work {
                    data.extra.push(work)
                };
                drop((loans.job, loans.claim));
                return Err(cause);
            }
        };
        self.data.as_mut().expect("actual owner").stage = Some(stage);
        let selected = self
            .data
            .as_ref()
            .expect("actual owner")
            .stage
            .as_ref()
            .expect("actual stage")
            .partition(at)?
            .context("actual catalog cursor absent")?;
        anyhow::ensure!(selected.next == next, "catalog cursor changed");
        if let Some(partition) = selected.partition {
            let result = partition.lexical(self.keyword)?;
            #[cfg(test)]
            if std::env::var_os("OMNI_TEST_PARTITION_LIST_FUSION").is_some() {
                self.data
                    .as_mut()
                    .expect("actual owner")
                    .test_partition_lists
                    .push(
                        result
                            .result()?
                            .hits
                            .iter()
                            .map(|hit| hit.key.id.clone())
                            .collect(),
                    );
            }
            for (index, hit) in result.result()?.hits.iter().enumerate() {
                let rank = index + 1;
                let path = shard
                    .parts
                    .iter()
                    .find(|p| p.document.key == hit.key)
                    .and_then(|p| p.document.locator.as_deref())
                    .context("actual source logical path absent")?;
                let mut count = Count(0);
                serde_json::to_writer(&mut count, hit)?;
                let charge = count
                    .0
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(std::mem::size_of::<Hit>() as u64 + 1024))
                    .context("result copy overflow")?;
                let data = self.data.as_mut().expect("actual owner");
                // Reserve real SAME-Claim capacity BEFORE even the rank key copy, and cover
                // old+replacement bytes together until the actual old allocation dies.
                let peak = data
                    .metadata_used
                    .checked_add(charge)
                    .context("result metadata overflow")?;
                if peak > data.metadata_capacity {
                    let capacity = peak
                        .max(
                            data.metadata_capacity
                                .checked_add(data.metadata_capacity / 2)
                                .context("metadata growth overflow")?,
                        )
                        .max(64 * 1024);
                    let work = Work::split_claim(
                        &self.claim,
                        self.query.key(),
                        Reservation {
                            memory_bytes: capacity - data.metadata_capacity,
                            temporary_bytes: 0,
                        },
                    )?;
                    data.extra.push(work);
                    data.metadata_capacity = capacity;
                }
                let prior = data.window.rank(&hit.key.id);
                data.window.observe(
                    &hit.key.id,
                    path,
                    NonZeroUsize::new(rank).expect("one based mature rank"),
                );
                if data.window.contains(&hit.key.id) && prior.is_none_or(|old| rank < old) {
                    let id = hit.key.id.clone();
                    let row = Candidate {
                        hit: hit.clone(),
                        charged: charge,
                    };
                    let old = data.rows.remove(&id);
                    let released = old.as_ref().map_or(0, |old| old.charged);
                    drop(old);
                    data.metadata_used = data
                        .metadata_used
                        .checked_sub(released)
                        .and_then(|n| n.checked_add(charge))
                        .context("metadata replacement overflow")?;
                    data.rows.insert(id, row);
                }
            }
            drop(result);
        }
        let stage = self
            .data
            .as_mut()
            .expect("actual owner")
            .stage
            .take()
            .expect("actual stage");
        let loans = match stage.into_loans() {
            Ok(loans) => loans,
            Err(stage) => {
                self.data.as_mut().expect("actual owner").stage = Some(stage);
                bail!("actual native consumer still pins partition")
            }
        };
        if let Some(mut work) = loans.work
            && let Err(error) = work.rejoin(&self.claim, self.query.key())
        {
            self.data.as_mut().expect("actual owner").extra.push(work);
            self.data
                .as_mut()
                .expect("actual owner")
                .pending
                .extend(loans.sources);
            drop((loans.job, loans.claim));
            return Err(error.into());
        }
        let data = self.data.as_mut().expect("actual owner");
        let evicted: Vec<_> = data
            .rows
            .keys()
            .filter(|id| !data.window.contains(id))
            .cloned()
            .collect();
        for id in evicted {
            if let Some(row) = data.rows.remove(&id) {
                let charged = row.charged;
                drop(row);
                data.metadata_used -= charged;
            }
        }
        let evicted: Vec<_> = data
            .sources
            .keys()
            .filter(|id| !data.window.contains(id))
            .cloned()
            .collect();
        let mut closing = Vec::new();
        for id in evicted {
            closing.push(data.sources.remove(&id).expect("actual evicted source"));
        }
        for source in loans.sources {
            let id = source.part.document.key.id.clone();
            if data.window.contains(&id) && !data.sources.contains_key(&id) {
                data.sources.insert(id, source);
            } else {
                closing.push(source)
            }
        }
        drop((loans.job, loans.claim));
        self.data
            .as_mut()
            .expect("actual owner")
            .pending
            .extend(closing);
        while let Some(source) = self.data.as_mut().expect("actual owner").pending.pop() {
            self.close_source(source)?;
        }
        self.collect_pages()?;
        let data = self.data.as_mut().expect("actual owner");
        data.catalog = next;
        data.partitions += 1;
        Ok(false)
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
                    if let Err(error) = Account::current()
                        .and_then(|a| a.rejoin_claim(&mut credit, &self.claim, self.query.key()))
                    {
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
    /// After last actual consumer/delivery, destroy data/FDs then return the exact loans. A live
    /// scope/page/stage returns this actual owner, never silently drops or globally re-reserves it.
    pub fn finish(mut self) -> std::result::Result<(NativeTupleQuery, Arc<Claim>), Self> {
        if let Some(data) = self.data.as_mut() {
            data.closing = true;
            data.rows.clear();
            data.window = FileRankWindow::new(0);
            if let Some(stage) = data.stage.take() {
                match stage.into_loans() {
                    Ok(loans) => {
                        data.pending.extend(loans.sources);
                        if let Some(work) = loans.work {
                            data.extra.push(work)
                        }
                        drop((loans.job, loans.claim));
                    }
                    Err(stage) => {
                        data.stage = Some(stage);
                        return Err(self);
                    }
                }
            }
            data.pending
                .extend(std::mem::take(&mut data.sources).into_values());
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
            while let Some(source) = self.data.as_mut().and_then(|d| d.pending.pop()) {
                // A retained scope is kept in data.scopes; a Released refusal owns no live data.
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
                return Err(self);
            }
            if self.release_page().is_err() || self.collect_pages().is_err() {
                return Err(self);
            }
            if !self
                .data
                .as_ref()
                .expect("actual owner")
                .pinned_pages
                .is_empty()
            {
                return Err(self);
            }
            let mut data = self.data.take().expect("actual owner");
            // Kill ALL output paid allocations before rejoining their loans. The administrative
            // base separately pays the empty containers/loan handles until this box itself dies.
            drop(std::mem::take(&mut data.rows));
            drop(std::mem::replace(&mut data.window, FileRankWindow::new(0)));
            drop(data.failure.take());
            let mut extra = std::mem::take(&mut data.extra).into_iter();
            while let Some(mut work) = extra.next() {
                if work.rejoin(&self.claim, self.query.key()).is_err() {
                    data.extra.push(work);
                    data.extra.extend(extra);
                    self.data = Some(data);
                    return Err(self);
                }
            }
            let mut credits = std::mem::take(&mut data.credits).into_iter();
            while let Some(mut credit) = credits.next() {
                if Account::current()
                    .and_then(|a| a.rejoin_claim(&mut credit, &self.claim, self.query.key()))
                    .is_err()
                {
                    data.credits.push(credit);
                    data.credits.extend(credits);
                    self.data = Some(data);
                    return Err(self);
                }
            }
            drop(data);
        }
        let mut base = self.base.take().expect("actual administrative loan");
        if base.rejoin(&self.claim, self.query.key()).is_err() {
            self.base = Some(base);
            return Err(self);
        }
        Ok((self.query, self.claim))
    }
}
struct Count(u64);
impl std::io::Write for Count {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("query result quote overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

type PointerOpen = (File, std::fs::Metadata, wire::RelativePath);
struct PageFrontmatter {
    title: Option<String>,
    descriptions: Vec<String>,
    tags: Vec<String>,
    present: bool,
}
struct PointerMarkdown {
    title: Option<String>,
    descriptions: Vec<String>,
    tags: Vec<String>,
    path: wire::RelativePath,
}
struct MetadataCandidate {
    annotations: NativeQueryAnnotations,
    scope: Permit,
    charged: u64,
}
struct MetadataData {
    rows: BTreeMap<(String, String), MetadataCandidate>,
    scopes: Vec<Permit>,
    page: Option<NativeInventoryPage>,
    pinned_pages: Vec<NativeInventoryPage>,
    extra: Vec<Work>,
    credits: Vec<Holding>,
    offset: u64,
    unavailable_sources: u64,
    discovered: u64,
    verified: u64,
    pending_hash: u64,
    metadata_capacity: u64,
    metadata_used: u64,
    done: bool,
    closing: bool,
    failure: Option<anyhow::Error>,
}
enum RowBuild {
    Skip {
        unavailable: bool,
        verified: bool,
        pending_hash: bool,
    },
    Keep {
        annotations: NativeQueryAnnotations,
        unavailable: bool,
    },
}
#[cfg(test)]
thread_local! {
    static TEST_FRONTMATTER_MAX_READ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_FRONTMATTER_MAX_ADMITTED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_FRONTMATTER_ENTRIES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_POINTER_ENTRIES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_POINTER_READ_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_BYTES_BEFORE_FRONTMATTER: std::cell::Cell<Option<u64>> =
        const { std::cell::Cell::new(None) };
    static TEST_OBJECTS_BEFORE_POINTER: std::cell::Cell<Option<u64>> =
        const { std::cell::Cell::new(None) };
    static TEST_METADATA_EVICTIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_FRONTMATTER_IO_BYTES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_FRONTMATTER_LAST_READ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_FRONTMATTER_CALL_READ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_FRONTMATTER_CALL_ADMITTED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn begin_frontmatter_observation() {
    TEST_FRONTMATTER_CALL_READ.with(|cell| cell.set(0));
    TEST_FRONTMATTER_CALL_ADMITTED.with(|cell| cell.set(0));
    TEST_FRONTMATTER_LAST_READ.with(|cell| cell.set(0));
}
#[cfg(test)]
fn observe_frontmatter_admission(bytes: u64) {
    TEST_FRONTMATTER_CALL_ADMITTED.with(|cell| cell.set(cell.get().saturating_add(bytes)));
}
#[cfg(test)]
fn observe_frontmatter_buffer(bytes: u64) {
    TEST_FRONTMATTER_IO_BYTES.with(|cell| cell.set(cell.get().saturating_add(bytes)));
    let read = TEST_FRONTMATTER_CALL_READ.with(|cell| {
        let total = cell.get().saturating_add(bytes);
        cell.set(total);
        total
    });
    TEST_FRONTMATTER_LAST_READ.with(|cell| cell.set(read));
    let admitted = TEST_FRONTMATTER_CALL_ADMITTED.with(std::cell::Cell::get);
    TEST_FRONTMATTER_MAX_READ.with(|max_read| {
        if read > max_read.get() {
            max_read.set(read);
            TEST_FRONTMATTER_MAX_ADMITTED.with(|max_admitted| max_admitted.set(admitted));
        }
    });
}
fn read_frontmatter_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    file.read_exact_at(buf, offset)?;
    #[cfg(test)]
    observe_frontmatter_buffer(buf.len() as u64);
    Ok(())
}
/// Actual M1 metadata discovery owner. Annotations and scopes die before page and Claim credit.
pub struct NativeMetadataRun {
    query: NativeTupleQuery,
    claim: Arc<Claim>,
    library: usize,
    method: Option<(wire::SearchMethod, usize)>,
    data: Option<Box<MetadataData>>,
    // Rust drops fields in declaration order: paid result bytes die before this administrative loan.
    base: Option<Work>,
}
impl NativeTupleQuery {
    /// Start metadata discovery for every admitted file/tag keyword in one actual library.
    pub fn start_metadata(&self, claim: Arc<Claim>, library: usize) -> Result<NativeMetadataRun> {
        self.start_metadata_for(claim, library, None)
    }
    /// Start metadata discovery for one file/tag keyword, or for the whole admitted keyword set.
    pub fn start_metadata_for(
        &self,
        claim: Arc<Claim>,
        library: usize,
        method: Option<(wire::SearchMethod, usize)>,
    ) -> Result<NativeMetadataRun> {
        self.cause.check_claim(&claim)?;
        anyhow::ensure!(library < self.cause.libraries.len(), "actual Query library");
        let wire::Command::Query { query } = self.cause.libraries[library]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required");
        };
        if let Some((selected, keyword)) = method {
            anyhow::ensure!(
                matches!(
                    selected,
                    wire::SearchMethod::Files | wire::SearchMethod::Tags
                ) && query.methods.contains(&selected)
                    && keyword < query.keywords.len(),
                "requested metadata method required"
            );
        }
        let bytes = query
            .limit
            .get()
            .checked_add(64)
            .and_then(|n| n.checked_mul(2048))
            .and_then(|n| n.checked_add(64 * 1024))
            .context("metadata administration overflow")?;
        let base = Work::split_claim(
            &claim,
            self.key(),
            Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            },
        )?;
        Ok(NativeMetadataRun {
            query: NativeTupleQuery {
                cause: self.cause.clone(),
            },
            claim,
            library,
            method,
            data: Some(Box::new(MetadataData {
                rows: BTreeMap::new(),
                scopes: Vec::new(),
                page: None,
                pinned_pages: Vec::new(),
                extra: Vec::new(),
                credits: Vec::new(),
                offset: 0,
                unavailable_sources: 0,
                discovered: 0,
                verified: 0,
                pending_hash: 0,
                metadata_capacity: 0,
                metadata_used: 0,
                done: false,
                closing: false,
                failure: None,
            })),
            base: Some(base),
        })
    }
}
impl NativeMetadataRun {
    /// Fixed typed global data-budget refusal, never an unavailable source row.
    pub fn budget_exhausted(&self) -> bool {
        self.failure()
            .is_some_and(|error| error.is::<QueryBudgetExhausted>())
    }
    /// Borrow the retained actual failure without detaching any owning refusal.
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.data.as_ref()?.failure.as_ref()
    }
    /// Conservative admitted data bytes, including prior input metadata, NOT completed-IO/RSS.
    pub fn bytes_admitted(&self) -> u64 {
        self.query.cause.reads.used()
    }
    /// Conservative source/metadata examinations, including unavailable entries.
    pub fn objects_admitted(&self) -> u64 {
        self.query.cause.reads.amount().objects
    }
    /// Actual missing or unreadable originals withheld or labeled unavailable. Exclusions are not counted.
    pub fn unavailable_sources(&self) -> u64 {
        self.data
            .as_ref()
            .map_or(0, |data| data.unavailable_sources)
    }
    #[cfg(test)]
    pub fn test_frontmatter_max_read() -> u64 {
        TEST_FRONTMATTER_MAX_READ.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_frontmatter_max_admitted() -> u64 {
        TEST_FRONTMATTER_MAX_ADMITTED.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_frontmatter_entries() -> u64 {
        TEST_FRONTMATTER_ENTRIES.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_frontmatter_io_bytes() -> u64 {
        TEST_FRONTMATTER_IO_BYTES.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_frontmatter_last_read() -> u64 {
        TEST_FRONTMATTER_LAST_READ.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_pointer_entries() -> u64 {
        TEST_POINTER_ENTRIES.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_pointer_read_bytes() -> u64 {
        TEST_POINTER_READ_BYTES.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_bytes_before_frontmatter() -> u64 {
        TEST_BYTES_BEFORE_FRONTMATTER.with(|slot| slot.get().unwrap_or(0))
    }
    #[cfg(test)]
    pub fn test_objects_before_pointer() -> u64 {
        TEST_OBJECTS_BEFORE_POINTER.with(|slot| slot.get().unwrap_or(0))
    }
    #[cfg(test)]
    pub fn test_evictions() -> u64 {
        TEST_METADATA_EVICTIONS.with(std::cell::Cell::get)
    }
    #[cfg(test)]
    pub fn test_pinned_pages(&self) -> usize {
        self.data.as_ref().map_or(0, |data| data.pinned_pages.len())
    }
    #[cfg(test)]
    pub fn test_reclaimable_pages(&self) -> usize {
        self.data.as_ref().map_or(0, |data| {
            data.pinned_pages
                .iter()
                .filter(|page| Arc::strong_count(&page.page) == 1)
                .count()
        })
    }
    #[cfg(test)]
    pub fn test_scope_clone(&self) -> Option<Permit> {
        self.data
            .as_ref()?
            .rows
            .values()
            .next()
            .map(|row| row.scope.clone())
    }
    fn check(&self) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        let data = self.data.as_ref().context("native result consumed")?;
        for row in data.rows.values() {
            row.scope.check().map_err(refused_error)?;
        }
        Ok(())
    }
    pub(in crate::native_service) fn take_scopes(
        &mut self,
    ) -> Vec<(NativeQueryAnnotations, Permit)> {
        self.data
            .as_mut()
            .map(|data| {
                std::mem::take(&mut data.rows)
                    .into_values()
                    .map(|row| (row.annotations, row.scope))
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Pages kept alive by transferred metadata scopes. `finish` cannot recover them.
    pub(in crate::native_service) fn take_retained_pages(&mut self) -> Vec<NativeInventoryPage> {
        let Some(data) = self.data.as_mut() else {
            return Vec::new();
        };
        let mut pages = std::mem::take(&mut data.pinned_pages);
        if let Some(page) = data.page.take() {
            pages.push(page);
        }
        pages
    }
    /// Borrow current matching annotations after every retained permit is rechecked.
    pub fn hits(&self) -> Result<impl Iterator<Item = &NativeQueryAnnotations>> {
        self.check()?;
        let data = self.data.as_ref().context("native result consumed")?;
        anyhow::ensure!(
            data.done && data.failure.is_none() && !data.closing,
            "native metadata query not complete"
        );
        Ok(data.rows.values().map(|row| &row.annotations))
    }
    /// A failed step is terminal. The actual owner keeps its page, scope, and Claim until finish.
    pub fn step(&mut self, service: &NativeLibraryService) -> NativeQueryStep {
        self.step_library(&service.library)
    }
    /// Route through this run's actual selected reader; the ID alone grants no access.
    fn step_library(&mut self, library_id: &wire::Id) -> NativeQueryStep {
        let Some(data) = self.data.as_ref() else {
            return NativeQueryStep::Failed;
        };
        if data.failure.is_some() || data.closing {
            return NativeQueryStep::Failed;
        }
        if data.done {
            return NativeQueryStep::Complete;
        }
        match self.advance(library_id) {
            Ok(true) => NativeQueryStep::Complete,
            Ok(false) => NativeQueryStep::Progress,
            Err(error) => {
                let error = if self.query.cause.reads.check().is_err() {
                    error.context(QueryBudgetExhausted)
                } else {
                    error
                };
                self.data.as_mut().expect("actual owner").failure = Some(error);
                NativeQueryStep::Failed
            }
        }
    }
    /// Destroy annotations, then rejoin scopes, pages, and Work. A live scope returns this owner.
    pub fn finish(mut self) -> std::result::Result<(NativeTupleQuery, Arc<Claim>), Self> {
        if let Some(data) = self.data.as_mut() {
            data.closing = true;
            let rows = std::mem::take(&mut data.rows);
            for (_, row) in rows {
                let MetadataCandidate {
                    annotations, scope, ..
                } = row;
                drop(annotations);
                data.scopes.push(scope);
            }
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
                            Err(error) => data.failure = Some(error),
                        }
                    }
                }
            }
        }
        if self.data.is_some() {
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
                return Err(self);
            }
            if self.release_page().is_err() || self.collect_pages().is_err() {
                return Err(self);
            }
            if !self
                .data
                .as_ref()
                .expect("actual owner")
                .pinned_pages
                .is_empty()
            {
                return Err(self);
            }
            let mut data = self.data.take().expect("actual owner");
            drop(std::mem::take(&mut data.rows));
            drop(data.failure.take());
            let mut extra = std::mem::take(&mut data.extra).into_iter();
            while let Some(mut work) = extra.next() {
                if work.rejoin(&self.claim, self.query.key()).is_err() {
                    data.extra.push(work);
                    data.extra.extend(extra);
                    self.data = Some(data);
                    return Err(self);
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
                    return Err(self);
                }
            }
            drop(data);
        }
        let mut base = self.base.take().expect("actual administrative loan");
        if base.rejoin(&self.claim, self.query.key()).is_err() {
            self.base = Some(base);
            return Err(self);
        }
        Ok((self.query, self.claim))
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
        if let Some(page) = data.page.take() {
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
    fn rejoin_scope(&mut self, scope: Permit) -> Result<()> {
        match scope.rejoin_processing_scope(&self.claim, self.query.key()) {
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
    fn abandon_scope(&mut self, scope: Permit, cause: anyhow::Error) -> anyhow::Error {
        match scope.rejoin_processing_scope(&self.claim, self.query.key()) {
            Ok(()) => cause,
            Err(wire::authority::ScopeRejoinError::Retained { scope, .. }) => {
                self.data
                    .as_mut()
                    .expect("actual owner")
                    .scopes
                    .push(*scope);
                cause
            }
            Err(wire::authority::ScopeRejoinError::Released(_)) => cause,
        }
    }
    fn query_limit(&self) -> Result<usize> {
        let wire::Command::Query { query } = self.query.cause.libraries[self.library]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required");
        };
        Ok(query.limit.get() as usize)
    }
    fn advance(&mut self, library_id: &wire::Id) -> Result<bool> {
        self.query.cause.check_claim(&self.claim)?;
        anyhow::ensure!(
            *library_id
                == self.query.cause.libraries[self.library]
                    .reader
                    .physical
                    .library,
            "actual library_id/library owner differs"
        );
        if self.data.as_ref().expect("actual owner").done {
            return Ok(true);
        }
        let offset = self.data.as_ref().expect("actual owner").offset;
        self.release_page()?;
        self.collect_pages()?;
        let page = self.read_inventory_page(offset)?;
        let observations: Vec<_> = page.observations().collect();
        let complete = page.complete();
        let next_offset = page.next_offset();
        self.data.as_mut().expect("actual owner").page = Some(page);
        for observed in observations {
            self.consider(library_id, observed)?;
        }
        if complete {
            self.release_page()?;
            self.collect_pages()?;
            self.data.as_mut().expect("actual owner").done = true;
            return Ok(true);
        }
        anyhow::ensure!(next_offset > offset, "inventory replay made no progress");
        self.data.as_mut().expect("actual owner").offset = next_offset;
        Ok(false)
    }
    fn consider(&mut self, library_id: &wire::Id, observed: NativeObservedSource) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        let path = wire::RelativePath::new(observed.observation().stat.path.as_str())?;
        let permit = &self.query.cause.libraries[self.library].reader.permit;
        match permit.check_path(library_id, &path) {
            Err(wire::ErrorCode::NotExposed) => return Ok(()),
            Err(code) => return Err(refused_error(code)),
            Ok(()) => {}
        }
        self.data.as_mut().expect("actual owner").discovered += 1;
        let pointer_rel = wire::RelativePath::new(format!("sources/{}.md", observed.source_id()))?;
        let pointer = open_pointer(permit, library_id, pointer_rel)?;
        let pointer_extra = pointer_scope_extra(pointer.as_ref())?;
        let memory_bytes = (1024 * 1024u64)
            .checked_add(
                (path.as_str().len() as u64)
                    .checked_mul(32)
                    .context("metadata scope quote overflow")?,
            )
            .and_then(|bytes| bytes.checked_add(pointer_extra))
            .context("metadata scope quote overflow")?;
        let scope = permit
            .fork_processing_scope(
                &self.claim,
                self.query.key(),
                Reservation {
                    memory_bytes,
                    temporary_bytes: 0,
                },
            )
            .map_err(refused_error)?;
        match self.classify(library_id, &scope, observed, path, pointer) {
            Ok(RowBuild::Skip {
                unavailable,
                verified,
                pending_hash,
            }) => {
                let data = self.data.as_mut().expect("actual owner");
                data.verified += u64::from(verified);
                data.pending_hash += u64::from(pending_hash);
                if unavailable {
                    self.data
                        .as_mut()
                        .expect("actual owner")
                        .unavailable_sources += 1;
                }
                self.rejoin_scope(scope)
            }
            Ok(RowBuild::Keep {
                annotations,
                unavailable,
            }) => {
                let data = self.data.as_mut().expect("actual owner");
                data.verified += u64::from(annotations.digest_state == wire::DigestState::Verified);
                data.pending_hash += u64::from(
                    annotations.availability == wire::Availability::Available
                        && annotations.digest_state == wire::DigestState::Unverified,
                );
                if unavailable {
                    data.unavailable_sources += 1;
                }
                self.retain(annotations, scope)
            }
            Err(cause) => Err(self.abandon_scope(scope, cause)),
        }
    }
    fn classify(
        &self,
        library_id: &wire::Id,
        scope: &Permit,
        observed: NativeObservedSource,
        path: wire::RelativePath,
        pointer: Option<PointerOpen>,
    ) -> Result<RowBuild> {
        let checked = self.query.cause.clone();
        let live = self.claim.clone();
        let path_permit = self.query.cause.libraries[self.library]
            .reader
            .permit
            .clone();
        let watched_library = library_id.clone();
        let watched = path.clone();
        scope
            .retain_check(Arc::new(move || {
                checked.check_scope_claim(&live).is_ok()
                    && path_permit.check_path(&watched_library, &watched).is_ok()
            }))
            .map_err(refused_error)?;
        let source_id = observed.source_id().to_owned();
        let obs = observed.observation().clone();
        let mut observed = Some(observed);
        let available = obs.availability == knowledge_core::backend::Availability::Available
            && !matches!(obs.verification, knowledge_storage::Verification::Failed(_));
        let mut page_title = None;
        let mut page_descriptions = Vec::new();
        let mut page_tags = Vec::new();
        let mut has_frontmatter = false;
        let digest;
        let digest_state;
        let availability;
        let unavailable;
        if available {
            let opened = scope
                .with_root(library_id, |root| {
                    Ok(LibraryHost::open_root(root, path.as_str()))
                })
                .map_err(refused_error)?;
            match opened {
                Err(wire::ErrorCode::NotFound) => {
                    return Ok(RowBuild::Skip {
                        unavailable: true,
                        verified: false,
                        pending_hash: false,
                    });
                }
                Err(wire::ErrorCode::Denied) => {
                    digest = None;
                    digest_state = wire::DigestState::Unverified;
                    availability = wire::Availability::Denied;
                    unavailable = true;
                }
                Err(code) => return Err(refused_error(code)),
                Ok(orig_file) => {
                    let orig_meta = orig_file.metadata()?;
                    if !orig_meta.is_file()
                        || observed
                            .as_ref()
                            .expect("actual observation")
                            .check_file(&orig_file)
                            .is_err()
                    {
                        return Ok(RowBuild::Skip {
                            unavailable: true,
                            verified: false,
                            pending_hash: false,
                        });
                    }
                    LibraryHost::track_version(
                        library_id,
                        path.as_str(),
                        &orig_file,
                        &orig_meta,
                        scope,
                    )
                    .map_err(refused_error)?;
                    let pinned = orig_file.try_clone()?;
                    let watched_source = observed.take().expect("actual observation");
                    scope
                        .retain_check(Arc::new(move || watched_source.check_file(&pinned).is_ok()))
                        .map_err(refused_error)?;
                    if path.as_str().ends_with(".md") && orig_meta.len() > 0 {
                        let parsed = self.read_page_frontmatter(&orig_file, orig_meta.len())?;
                        page_title = parsed.title;
                        page_descriptions = parsed.descriptions;
                        page_tags = parsed.tags;
                        has_frontmatter = parsed.present;
                    }
                    let verified = verified_digest(&obs)?;
                    digest = verified.0;
                    digest_state = verified.1;
                    availability = wire::Availability::Available;
                    unavailable = false;
                }
            }
        } else {
            digest = None;
            digest_state = wire::DigestState::Unverified;
            availability = map_unavailable(&obs);
            unavailable = true;
        }
        let (pointer_title, pointer_descriptions, pointer_tags, pointer_path) =
            if let Some(pointer) = pointer {
                let parsed = self.read_pointer(library_id, scope, pointer)?;
                (
                    parsed.title,
                    parsed.descriptions,
                    parsed.tags,
                    Some(parsed.path),
                )
            } else {
                (None, Vec::new(), Vec::new(), None)
            };
        let annotations = super::query_annotations(super::QueryAnnotationDraft {
            source_id,
            path,
            page_title,
            page_descriptions,
            page_tags,
            has_frontmatter,
            pointer_title,
            pointer_descriptions,
            pointer_tags,
            pointer_path,
            digest,
            digest_state,
            availability,
        })?;
        if self.metadata_matches(&annotations)? {
            Ok(RowBuild::Keep {
                annotations,
                unavailable,
            })
        } else {
            Ok(RowBuild::Skip {
                unavailable,
                verified: annotations.digest_state == wire::DigestState::Verified,
                pending_hash: annotations.availability == wire::Availability::Available
                    && annotations.digest_state == wire::DigestState::Unverified,
            })
        }
    }
    fn read_page_frontmatter(&self, file: &File, len: u64) -> Result<PageFrontmatter> {
        #[cfg(test)]
        {
            TEST_FRONTMATTER_ENTRIES.with(|count| count.set(count.get() + 1));
            TEST_BYTES_BEFORE_FRONTMATTER.with(|slot| {
                if slot.get().is_none() {
                    slot.set(Some(self.query.cause.reads.used()));
                }
            });
            begin_frontmatter_observation();
        }
        let head_len = len.min(6);
        #[cfg(test)]
        let admitted_before_head = self.query.cause.reads.used();
        self.query.cause.admit_io(&self.claim, head_len)?;
        #[cfg(test)]
        observe_frontmatter_admission(
            self.query
                .cause
                .reads
                .used()
                .saturating_sub(admitted_before_head),
        );
        let mut head_bytes = vec![0u8; head_len as usize];
        read_frontmatter_at(file, &mut head_bytes, 0)?;
        let matched =
            head_bytes.starts_with(b"---") || head_bytes.starts_with("\u{feff}---".as_bytes());
        if matched {
            let extra = len.min(64 * 1024) - head_len;
            if extra > 0 {
                #[cfg(test)]
                let admitted_before_rest = self.query.cause.reads.used();
                self.query.cause.admit_io(&self.claim, extra)?;
                #[cfg(test)]
                observe_frontmatter_admission(
                    self.query
                        .cause
                        .reads
                        .used()
                        .saturating_sub(admitted_before_rest),
                );
                let mut rest = vec![0u8; extra as usize];
                read_frontmatter_at(file, &mut rest, head_len)?;
                head_bytes.extend_from_slice(&rest);
            }
        }
        let (title, descriptions, tags, present) = if matched {
            parse_frontmatter_fields(&String::from_utf8_lossy(&head_bytes))
        } else {
            (None, Vec::new(), Vec::new(), false)
        };
        Ok(PageFrontmatter {
            title,
            descriptions,
            tags,
            present,
        })
    }
    fn read_pointer(
        &self,
        library_id: &wire::Id,
        scope: &Permit,
        pointer: PointerOpen,
    ) -> Result<PointerMarkdown> {
        let (pointer_file, pointer_meta, pointer_rel) = pointer;
        #[cfg(test)]
        {
            TEST_POINTER_ENTRIES.with(|count| count.set(count.get() + 1));
            TEST_OBJECTS_BEFORE_POINTER.with(|slot| {
                if slot.get().is_none() {
                    slot.set(Some(self.query.cause.reads.amount().objects));
                }
            });
        }
        self.query.cause.admit_object(&self.claim)?;
        self.query.cause.admit_io(&self.claim, pointer_meta.len())?;
        anyhow::ensure!(
            stamp(&pointer_file)? == stamp_from_meta(&pointer_meta),
            "pointer changed before read"
        );
        let mut pointer_bytes = vec![0u8; pointer_meta.len() as usize];
        #[cfg(test)]
        TEST_POINTER_READ_BYTES.with(|count| count.set(count.get() + pointer_meta.len()));
        pointer_file.read_exact_at(&mut pointer_bytes, 0)?;
        anyhow::ensure!(
            stamp(&pointer_file)? == stamp_from_meta(&pointer_meta),
            "pointer changed after read"
        );
        LibraryHost::track_version(
            library_id,
            pointer_rel.as_str(),
            &pointer_file,
            &pointer_meta,
            scope,
        )
        .map_err(refused_error)?;
        let (title, descriptions, tags) =
            parse_pointer_markdown(&String::from_utf8(pointer_bytes)?);
        Ok(PointerMarkdown {
            title,
            descriptions,
            tags,
            path: pointer_rel,
        })
    }
    fn metadata_matches(&self, annotations: &NativeQueryAnnotations) -> Result<bool> {
        let wire::Command::Query { query } = self.query.cause.libraries[self.library]
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required");
        };
        if !annotations.matches_required_tags(&query.required_tags, query.case_sensitive) {
            return Ok(false);
        }
        let matched =
            if let Some((method, keyword)) = self.method {
                match method {
                    wire::SearchMethod::Files => annotations
                        .matches_file_keyword(&query.keywords[keyword], query.case_sensitive),
                    wire::SearchMethod::Tags => annotations
                        .matches_tag_keyword(&query.keywords[keyword], query.case_sensitive),
                    _ => false,
                }
            } else if query.keywords.is_empty() {
                true
            } else {
                (query.methods.contains(&wire::SearchMethod::Files)
                    && query.keywords.iter().any(|keyword| {
                        annotations.matches_file_keyword(keyword, query.case_sensitive)
                    }))
                    || (query.methods.contains(&wire::SearchMethod::Tags)
                        && query.keywords.iter().any(|keyword| {
                            annotations.matches_tag_keyword(keyword, query.case_sensitive)
                        }))
                    || (!query.methods.contains(&wire::SearchMethod::Files)
                        && !query.methods.contains(&wire::SearchMethod::Tags))
            };
        Ok(matched)
    }
    fn retain(&mut self, annotations: NativeQueryAnnotations, scope: Permit) -> Result<()> {
        let key = (
            annotations.path.as_str().to_owned(),
            annotations.source_id.as_str().to_owned(),
        );
        let limit = self.query_limit()?;
        let exists = self
            .data
            .as_ref()
            .expect("actual owner")
            .rows
            .contains_key(&key);
        let len = self.data.as_ref().expect("actual owner").rows.len();
        if len == limit && !exists {
            let last = self
                .data
                .as_ref()
                .expect("actual owner")
                .rows
                .last_key_value()
                .expect("bounded metadata rows")
                .0
                .clone();
            if key >= last {
                return self.rejoin_scope(scope);
            }
        }
        let charge = annotation_charge(&annotations)?;
        if let Err(error) = self.ensure_metadata_capacity(charge) {
            return Err(self.abandon_scope(scope, error));
        }
        let evict = if exists {
            Some(key.clone())
        } else if self.data.as_ref().expect("actual owner").rows.len() == limit {
            self.data
                .as_ref()
                .expect("actual owner")
                .rows
                .last_key_value()
                .map(|(last, _)| last.clone())
        } else {
            None
        };
        #[cfg(test)]
        let displaced = evict.as_ref().is_some_and(|previous| previous != &key);
        if let Some(evict) = evict
            && let Some(old) = self
                .data
                .as_mut()
                .expect("actual owner")
                .rows
                .remove(&evict)
        {
            let released = old.charged;
            drop(old.annotations);
            if let Err(error) = self.rejoin_scope(old.scope) {
                return Err(self.abandon_scope(scope, error));
            }
            #[cfg(test)]
            if displaced {
                TEST_METADATA_EVICTIONS.with(|count| count.set(count.get() + 1));
            }
            let data = self.data.as_mut().expect("actual owner");
            data.metadata_used = data.metadata_used.saturating_sub(released);
            if let Err(error) = self.collect_pages() {
                return Err(self.abandon_scope(scope, error));
            }
        }
        let used = match self
            .data
            .as_ref()
            .expect("actual owner")
            .metadata_used
            .checked_add(charge)
        {
            Some(used) => used,
            None => {
                return Err(self.abandon_scope(scope, anyhow::anyhow!("metadata result overflow")));
            }
        };
        let data = self.data.as_mut().expect("actual owner");
        data.metadata_used = used;
        data.rows.insert(
            key,
            MetadataCandidate {
                annotations,
                scope,
                charged: charge,
            },
        );
        Ok(())
    }
    fn ensure_metadata_capacity(&mut self, charge: u64) -> Result<()> {
        let data = self.data.as_mut().expect("actual owner");
        let peak = data
            .metadata_used
            .checked_add(charge)
            .context("metadata result overflow")?;
        if peak <= data.metadata_capacity {
            return Ok(());
        }
        let capacity = peak
            .max(
                data.metadata_capacity
                    .checked_add(data.metadata_capacity / 2)
                    .context("metadata growth overflow")?,
            )
            .max(64 * 1024);
        let work = Work::split_claim(
            &self.claim,
            self.query.key(),
            Reservation {
                memory_bytes: capacity - data.metadata_capacity,
                temporary_bytes: 0,
            },
        )?;
        data.extra.push(work);
        data.metadata_capacity = capacity;
        Ok(())
    }
}
fn annotation_charge(annotations: &NativeQueryAnnotations) -> Result<u64> {
    let mut bytes = annotations.path.as_str().len()
        + annotations.source_id.as_str().len()
        + annotations.title.len()
        + annotations.description.len()
        + annotations
            .annotation_path
            .as_ref()
            .map_or(0, |path| path.as_str().len());
    for tag in &annotations.tags {
        bytes += tag.len();
    }
    (bytes as u64)
        .checked_mul(16)
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<NativeQueryAnnotations>() as u64 + 1024)
        })
        .context("metadata result quote overflow")
}
fn pointer_scope_extra(pointer: Option<&PointerOpen>) -> Result<u64> {
    let Some((_, meta, _)) = pointer else {
        return Ok(0);
    };
    if meta.len() > 16 * 1024 {
        meta.len()
            .checked_mul(16)
            .context("pointer scope quote overflow")
    } else {
        Ok(0)
    }
}
fn open_pointer(
    permit: &Permit,
    library: &wire::Id,
    pointer_rel: wire::RelativePath,
) -> Result<Option<PointerOpen>> {
    match permit.check_path(library, &pointer_rel) {
        Err(wire::ErrorCode::NotExposed) => Ok(None),
        Err(code) => Err(refused_error(code)),
        Ok(()) => match permit
            .with_root(library, |root| {
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
                Ok(Some((file, meta, pointer_rel)))
            }
            Err(wire::ErrorCode::NotFound) => Ok(None),
            Err(code) => Err(refused_error(code)),
        },
    }
}
fn verified_digest(
    obs: &knowledge_storage::Observation,
) -> Result<(Option<wire::Digest>, wire::DigestState)> {
    if obs.verification == knowledge_storage::Verification::Verified
        && let Some(version) = &obs.version
    {
        Ok((
            Some(wire::Digest::new(version.digest.as_str())?),
            wire::DigestState::Verified,
        ))
    } else {
        Ok((None, wire::DigestState::Unverified))
    }
}
fn map_unavailable(obs: &knowledge_storage::Observation) -> wire::Availability {
    use knowledge_core::backend::{Availability, BackendError, Unavailable};
    let kind = match obs.availability {
        Availability::Unavailable(kind) => Some(kind),
        Availability::Available => match obs.verification {
            knowledge_storage::Verification::Failed(BackendError::Unavailable(kind)) => Some(kind),
            _ => None,
        },
    };
    match kind {
        Some(Unavailable::Denied) => wire::Availability::Denied,
        Some(Unavailable::Offline) => wire::Availability::Offline,
        Some(Unavailable::Placeholder) => wire::Availability::Placeholder,
        Some(Unavailable::Unsupported) => wire::Availability::Unsupported,
        Some(Unavailable::Changed) => wire::Availability::Changed,
        Some(Unavailable::Deleted) => wire::Availability::Deleted,
        Some(Unavailable::Unknown) | None => wire::Availability::Unknown,
    }
}
/// One shifted literal/regex occurrence, or an unlocated filename/title/description observation.
#[derive(Clone, Debug)]
pub struct NativeScanOccurrence {
    /// Global representation byte start, when the match has a text coordinate.
    pub start: Option<u64>,
    /// Global representation byte end, when the match has a text coordinate.
    pub end: Option<u64>,
    pub snippet: String,
    pub metadata: bool,
}
struct ScanOccurrence {
    start: Option<u64>,
    end: Option<u64>,
    snippet: String,
    metadata: bool,
    representation_digest: String,
    processor: Option<String>,
    source_digest: String,
}
struct ScanFile {
    occurrences: Vec<ScanOccurrence>,
}
struct AdmittedScan {
    id: String,
    path: String,
    origin: u64,
    representation_digest: String,
    annotations: NativeQueryAnnotations,
}
struct ScanData {
    window: FileRankWindow,
    files: BTreeMap<String, ScanFile>,
    sources: BTreeMap<String, NativeQuerySource>,
    pending: Vec<NativeQuerySource>,
    scopes: Vec<Permit>,
    page: Option<(u64, NativeInventoryPage)>,
    pinned_pages: Vec<NativeInventoryPage>,
    stage: Option<NativeQueryStage>,
    extra: Vec<Work>,
    credits: Vec<Holding>,
    catalog: u64,
    source: u64,
    unavailable_parts: u64,
    coverage: QueryCoverage,
    done: bool,
    closing: bool,
    failure: Option<anyhow::Error>,
}
/// Whole-library literal/regex scan over every admitted representation in catalog order.
pub struct NativeScanRun {
    query: NativeTupleQuery,
    claim: Arc<Claim>,
    library: usize,
    method: wire::SearchMethod,
    keyword: usize,
    data: Option<Box<ScanData>>,
    base: Option<Work>,
}
impl NativeTupleQuery {
    /// Start one requested literal or regex keyword. The caller prepays the BM25-sized working peak.
    pub fn start_scan(
        &self,
        claim: Arc<Claim>,
        library: usize,
        method: wire::SearchMethod,
        keyword: usize,
    ) -> Result<NativeScanRun> {
        self.cause.check_claim(&claim)?;
        let lib = self
            .cause
            .libraries
            .get(library)
            .context("actual Query library")?;
        let wire::Command::Query { query } = lib
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required");
        };
        anyhow::ensure!(
            matches!(
                method,
                wire::SearchMethod::Literal | wire::SearchMethod::Regex
            ) && query.methods.contains(&method)
                && keyword < query.keywords.len(),
            "requested literal/regex keyword required"
        );
        let limit = query.limit.get();
        let bytes = (std::mem::size_of::<NativeQuerySource>() as u64 + 1024)
            .checked_mul(limit.checked_add(64).context("scan slots overflow")?)
            .and_then(|n| n.checked_add(64 * 1024))
            .context("scan administration overflow")?;
        let base = Work::split_claim(
            &claim,
            self.key(),
            Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            },
        )?;
        Ok(NativeScanRun {
            query: NativeTupleQuery {
                cause: self.cause.clone(),
            },
            claim,
            library,
            method,
            keyword,
            base: Some(base),
            data: Some(Box::new(ScanData {
                window: FileRankWindow::new(query.limit.get() as usize),
                files: BTreeMap::new(),
                sources: BTreeMap::new(),
                pending: Vec::new(),
                scopes: Vec::new(),
                page: None,
                pinned_pages: Vec::new(),
                stage: None,
                extra: Vec::new(),
                credits: Vec::new(),
                catalog: 0,
                source: 0,
                unavailable_parts: 0,
                coverage: QueryCoverage::default(),
                done: false,
                closing: false,
                failure: None,
            })),
        })
    }
}
impl NativeScanRun {
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.data.as_ref()?.failure.as_ref()
    }
    pub fn budget_exhausted(&self) -> bool {
        self.failure()
            .is_some_and(|error| error.is::<QueryBudgetExhausted>())
    }
    pub fn unavailable_parts(&self) -> u64 {
        self.data.as_ref().map_or(0, |data| data.unavailable_parts)
    }
    pub fn bytes_admitted(&self) -> u64 {
        self.query.cause.reads.used()
    }
    pub fn objects_admitted(&self) -> u64 {
        self.query.cause.reads.amount().objects
    }
    fn check(&self) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        let data = self.data.as_ref().context("native scan consumed")?;
        for source in data.sources.values() {
            source.source().check()?;
        }
        Ok(())
    }
    /// Distinct winning files in best-rank, path, source-id order.
    pub fn ranked(&self) -> Result<Vec<String>> {
        self.check()?;
        let data = self.data.as_ref().context("native scan consumed")?;
        anyhow::ensure!(
            data.done && data.failure.is_none() && !data.closing,
            "native scan not complete"
        );
        Ok(data.window.ranked().map(str::to_owned).collect())
    }
    /// Shifted body matches and metadata fallbacks retained for one winning file.
    pub fn occurrences(&self, source_id: &str) -> Result<Vec<NativeScanOccurrence>> {
        self.check()?;
        let data = self.data.as_ref().context("native scan consumed")?;
        anyhow::ensure!(
            data.done && data.failure.is_none() && !data.closing,
            "native scan not complete"
        );
        Ok(data
            .files
            .get(source_id)
            .map(|file| {
                file.occurrences
                    .iter()
                    .map(|item| NativeScanOccurrence {
                        start: item.start,
                        end: item.end,
                        snippet: item.snippet.clone(),
                        metadata: item.metadata,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
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
    pub(in crate::native_service) fn take_sources(
        &mut self,
    ) -> BTreeMap<String, NativeQuerySource> {
        self.data
            .as_mut()
            .map(|data| std::mem::take(&mut data.sources))
            .unwrap_or_default()
    }
    /// Pages kept alive by transferred scan observers. `finish` cannot recover them.
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
    pub fn step(&mut self, service: &NativeLibraryService) -> NativeQueryStep {
        self.step_library(&service.library)
    }
    /// Route through this run's actual selected reader; the ID alone grants no access.
    fn step_library(&mut self, library_id: &wire::Id) -> NativeQueryStep {
        let Some(data) = self.data.as_ref() else {
            return NativeQueryStep::Failed;
        };
        if data.failure.is_some() || data.closing {
            return NativeQueryStep::Failed;
        }
        if data.done {
            return NativeQueryStep::Complete;
        }
        match self.advance(library_id) {
            Ok(true) => NativeQueryStep::Complete,
            Ok(false) => NativeQueryStep::Progress,
            Err(error) => {
                let error = if self.query.cause.reads.check().is_err() {
                    error.context(QueryBudgetExhausted)
                } else {
                    error
                };
                self.data.as_mut().expect("actual owner").failure = Some(error);
                NativeQueryStep::Failed
            }
        }
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
    fn advance(&mut self, library_id: &wire::Id) -> Result<bool> {
        self.check()?;
        let library = &self.query.cause.libraries[self.library];
        anyhow::ensure!(
            *library_id == library.reader.physical.library,
            "actual library_id/library owner differs"
        );
        let at = self.data.as_ref().expect("actual owner").catalog;
        let catalog = library.reader.physical.open_verified_admitted(
            "lexical.rows",
            || library.reader.check(),
            &|n| self.query.cause.admit_io(&self.claim, n),
        )?;
        if at == catalog.metadata()?.len() {
            let source = library.reader.physical.open_verified_admitted(
                "sources.rows",
                || library.reader.check(),
                &|n| self.query.cause.admit_io(&self.claim, n),
            )?;
            anyhow::ensure!(
                self.data.as_ref().expect("actual owner").source == source.metadata()?.len(),
                "complete catalog omitted source receipts"
            );
            self.release_page()?;
            self.collect_pages()?;
            self.data.as_mut().expect("actual owner").done = true;
            return Ok(true);
        }
        let (shard, next) = read_frame_admitted::<Shard>(&catalog, at, &|n| {
            self.query.cause.admit_io(&self.claim, n)
        })?;
        if self
            .data
            .as_ref()
            .expect("actual owner")
            .page
            .as_ref()
            .is_none_or(|(offset, _)| *offset != shard.page_offset)
        {
            self.release_page()?;
            let page = self.read_inventory_page(shard.page_offset)?;
            self.data.as_mut().expect("actual owner").page = Some((shard.page_offset, page));
        }
        for expected in &shard.parts {
            let cursor = self.data.as_ref().expect("actual owner").source;
            self.query.cause.admit_object(&self.claim)?;
            let (part, after, locator, excluded, scan_drift) = {
                let library = &self.query.cause.libraries[self.library];
                let (part, after) = library
                    .reader
                    .part_admitted(cursor, &|n| self.query.cause.admit_io(&self.claim, n))?
                    .context("selected source cursor absent")?;
                anyhow::ensure!(
                    part.data == *expected && expected.page_offset == shard.page_offset,
                    "source/catalog producer receipts differ"
                );
                let locator = expected
                    .document
                    .locator
                    .as_deref()
                    .context("actual source logical path absent")?
                    .to_owned();
                let excluded = library.inventory.owner.path_excluded(&locator)
                    || matches!(
                        library.reader.permit.check_path(
                            &library.reader.physical.library,
                            &wire::RelativePath::new(&locator)?,
                        ),
                        Err(wire::ErrorCode::NotExposed)
                    );
                let scan_drift = library.inventory.scan() != library.reader.physical.data.scan;
                (part, after, locator, excluded, scan_drift)
            };
            if excluded {
                self.data.as_mut().expect("actual owner").source = after;
                continue;
            }
            let source_id = expected.document.key.id.as_str();
            let first_source = self
                .data
                .as_mut()
                .expect("actual owner")
                .coverage
                .begin(source_id, &expected.status);
            let mut observed = self
                .data
                .as_ref()
                .expect("actual owner")
                .page
                .as_ref()
                .expect("actual page")
                .1
                .sources()
                .find(|source| same_observed(source, source_id, &locator));
            if observed.is_none() && scan_drift {
                self.release_page()?;
                let mut offset = 0u64;
                loop {
                    let page = self.read_inventory_page(offset)?;
                    let found = page
                        .sources()
                        .find(|source| same_observed(source, source_id, &locator));
                    let complete = page.complete();
                    let next_offset = page.next_offset();
                    self.data.as_mut().expect("actual owner").page = Some((offset, page));
                    if found.is_some() {
                        observed = found;
                        break;
                    }
                    if complete {
                        break;
                    }
                    anyhow::ensure!(next_offset > offset, "inventory replay made no progress");
                    self.release_page()?;
                    offset = next_offset;
                }
            }
            let Some(observed) = observed else {
                if scan_drift {
                    if first_source {
                        self.data
                            .as_mut()
                            .expect("actual owner")
                            .coverage
                            .unavailable += 1;
                    }
                    self.data.as_mut().expect("actual owner").unavailable_parts += 1;
                    self.data.as_mut().expect("actual owner").source = after;
                    continue;
                }
                bail!("actual M1 page source absent");
            };
            let result = self.query.source_part(
                library_id,
                &self.claim,
                self.library,
                observed,
                part,
                after,
            );
            match result {
                Ok(source) => {
                    if first_source {
                        self.data.as_mut().expect("actual owner").coverage.verified += 1;
                    }
                    match source.matches_required_tags() {
                        Ok(true) => self
                            .data
                            .as_mut()
                            .expect("actual owner")
                            .pending
                            .push(source),
                        Ok(false) => {
                            self.close_source(source)?;
                            self.data.as_mut().expect("actual owner").source = after;
                            continue;
                        }
                        Err(error) => {
                            self.close_source(source)?;
                            return Err(error);
                        }
                    }
                }
                Err(error) => {
                    if error
                        .downcast_ref::<crate::native_service::NativeRefreshScopeError>()
                        .is_none() && error.chain().any(|cause| {
                        cause.is::<crate::native_service::source::CachedQuerySourceUnavailable>()
                    }) {
                        if first_source {
                            self.data
                                .as_mut()
                                .expect("actual owner")
                                .coverage
                                .unavailable += 1;
                        }
                        self.data.as_mut().expect("actual owner").unavailable_parts += 1;
                    } else {
                        return Err(error);
                    }
                }
            }
            self.data.as_mut().expect("actual owner").source = after;
        }
        if self.data.as_ref().expect("actual owner").pending.is_empty() {
            self.data.as_mut().expect("actual owner").catalog = next;
            return Ok(false);
        }
        let pending = std::mem::take(&mut self.data.as_mut().expect("actual owner").pending);
        let admitted: Vec<AdmittedScan> = pending
            .iter()
            .map(|source| AdmittedScan {
                id: source.part.document.key.id.clone(),
                path: source.part.document.locator.clone().unwrap_or_default(),
                origin: source.part.representation_start,
                representation_digest: source.part.representation_digest.clone(),
                annotations: source.annotations().clone(),
            })
            .collect();
        let stage = match self.query.stage(
            self.claim.clone(),
            self.library,
            pending,
            NativeQueryStageResources {
                window: 1024 * 1024,
            },
        ) {
            Ok(stage) => stage,
            Err(refused) => {
                let (cause, loans) = refused.into_parts();
                let data = self.data.as_mut().expect("actual owner");
                data.pending = loans.sources;
                if let Some(work) = loans.work {
                    data.extra.push(work);
                }
                drop((loans.job, loans.claim));
                return Err(cause);
            }
        };
        self.data.as_mut().expect("actual owner").stage = Some(stage);
        let selected = self
            .data
            .as_ref()
            .expect("actual owner")
            .stage
            .as_ref()
            .expect("actual stage")
            .partition(at)?
            .context("actual catalog cursor absent")?;
        anyhow::ensure!(selected.next == next, "catalog cursor changed");
        let mut found: BTreeMap<String, Vec<ScanOccurrence>> = BTreeMap::new();
        if let Some(mut partition) = selected.partition {
            for (slot, source) in admitted.iter().enumerate() {
                let result = partition.scan(self.method, self.keyword, slot)?;
                let hits = result.result()?.hits.clone();
                partition = result.into_partition();
                let origin = source.origin;
                let id = source.id.clone();
                for hit in hits {
                    let (start, end) = shifted_span(&hit.locator, origin);
                    found.entry(id.clone()).or_default().push(ScanOccurrence {
                        start,
                        end,
                        snippet: hit.snippet.chars().take(500).collect(),
                        metadata: false,
                        representation_digest: source.representation_digest.clone(),
                        processor: hit.processor,
                        source_digest: hit.digest,
                    });
                }
            }
            drop(partition);
        }
        let (keyword, case_sensitive, limit) = {
            let wire::Command::Query { query } = self.query.cause.libraries[self.library]
                .reader
                .permit
                .admitted_command()
                .map_err(refused_error)?
            else {
                bail!("actual Query required");
            };
            (
                query.keywords[self.keyword].clone(),
                query.case_sensitive,
                query.limit.get() as usize,
            )
        };
        for item in &admitted {
            let empty = found.get(&item.id).is_none_or(|items| items.is_empty());
            if empty
                && item
                    .annotations
                    .matches_file_keyword(&keyword, case_sensitive)
            {
                found
                    .entry(item.id.clone())
                    .or_default()
                    .push(ScanOccurrence {
                        start: None,
                        end: None,
                        snippet: item.annotations.title.chars().take(500).collect(),
                        metadata: true,
                        representation_digest: String::new(),
                        processor: None,
                        source_digest: item
                            .annotations
                            .digest
                            .as_ref()
                            .map(|digest| digest.as_str().to_owned())
                            .unwrap_or_default(),
                    });
            }
        }
        let mut ranked: Vec<&AdmittedScan> = admitted
            .iter()
            .filter(|item| found.get(&item.id).is_some_and(|items| !items.is_empty()))
            .collect();
        ranked.sort_by(|left, right| left.path.cmp(&right.path).then(left.id.cmp(&right.id)));
        for (index, item) in ranked.iter().enumerate() {
            self.data.as_mut().expect("actual owner").window.observe(
                &item.id,
                &item.path,
                NonZeroUsize::new(index + 1).expect("one based scan rank"),
            );
            if !self
                .data
                .as_ref()
                .expect("actual owner")
                .window
                .contains(&item.id)
            {
                continue;
            }
            let incoming = found.remove(&item.id).unwrap_or_default();
            let data = self.data.as_mut().expect("actual owner");
            let file = data
                .files
                .entry(item.id.clone())
                .or_insert_with(|| ScanFile {
                    occurrences: Vec::new(),
                });
            for occurrence in incoming {
                if file.occurrences.len() >= limit {
                    break;
                }
                file.occurrences.push(occurrence);
            }
        }
        let stage = self
            .data
            .as_mut()
            .expect("actual owner")
            .stage
            .take()
            .expect("actual stage");
        let loans = match stage.into_loans() {
            Ok(loans) => loans,
            Err(stage) => {
                self.data.as_mut().expect("actual owner").stage = Some(stage);
                bail!("actual native consumer still pins partition");
            }
        };
        if let Some(mut work) = loans.work
            && let Err(error) = work.rejoin(&self.claim, self.query.key())
        {
            self.data.as_mut().expect("actual owner").extra.push(work);
            self.data
                .as_mut()
                .expect("actual owner")
                .pending
                .extend(loans.sources);
            drop((loans.job, loans.claim));
            return Err(error.into());
        }
        let data = self.data.as_mut().expect("actual owner");
        let evicted: Vec<_> = data
            .files
            .keys()
            .filter(|id| !data.window.contains(id))
            .cloned()
            .collect();
        for id in evicted {
            data.files.remove(&id);
        }
        let evicted: Vec<_> = data
            .sources
            .keys()
            .filter(|id| !data.window.contains(id))
            .cloned()
            .collect();
        let mut closing = Vec::new();
        for id in evicted {
            closing.push(data.sources.remove(&id).expect("actual evicted source"));
        }
        for source in loans.sources {
            let id = source.part.document.key.id.clone();
            if data.window.contains(&id) && !data.sources.contains_key(&id) {
                data.sources.insert(id, source);
            } else {
                closing.push(source);
            }
        }
        drop((loans.job, loans.claim));
        self.data
            .as_mut()
            .expect("actual owner")
            .pending
            .extend(closing);
        while let Some(source) = self.data.as_mut().expect("actual owner").pending.pop() {
            self.close_source(source)?;
        }
        self.collect_pages()?;
        self.data.as_mut().expect("actual owner").catalog = next;
        Ok(false)
    }
    pub fn finish(mut self) -> std::result::Result<(NativeTupleQuery, Arc<Claim>), Self> {
        if let Some(data) = self.data.as_mut() {
            data.closing = true;
            data.files.clear();
            data.window = FileRankWindow::new(0);
            if let Some(stage) = data.stage.take() {
                match stage.into_loans() {
                    Ok(loans) => {
                        data.pending.extend(loans.sources);
                        if let Some(work) = loans.work {
                            data.extra.push(work);
                        }
                        drop((loans.job, loans.claim));
                    }
                    Err(stage) => {
                        data.stage = Some(stage);
                        return Err(self);
                    }
                }
            }
            data.pending
                .extend(std::mem::take(&mut data.sources).into_values());
        }
        if self.data.is_some() {
            while let Some(source) = self.data.as_mut().and_then(|data| data.pending.pop()) {
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
                return Err(self);
            }
            if self.release_page().is_err() || self.collect_pages().is_err() {
                return Err(self);
            }
            if !self
                .data
                .as_ref()
                .expect("actual owner")
                .pinned_pages
                .is_empty()
            {
                return Err(self);
            }
            let mut data = self.data.take().expect("actual owner");
            drop(std::mem::take(&mut data.files));
            drop(std::mem::replace(&mut data.window, FileRankWindow::new(0)));
            drop(data.failure.take());
            let mut extra = std::mem::take(&mut data.extra).into_iter();
            while let Some(mut work) = extra.next() {
                if work.rejoin(&self.claim, self.query.key()).is_err() {
                    data.extra.push(work);
                    data.extra.extend(extra);
                    self.data = Some(data);
                    return Err(self);
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
                    return Err(self);
                }
            }
            drop(data);
        }
        let mut base = self.base.take().expect("actual administrative loan");
        if base.rejoin(&self.claim, self.query.key()).is_err() {
            self.base = Some(base);
            return Err(self);
        }
        Ok((self.query, self.claim))
    }
}
fn shifted_span(locator: &knowledge_retrieval::Locator, origin: u64) -> (Option<u64>, Option<u64>) {
    match locator {
        knowledge_retrieval::Locator::Text { start, end, .. }
        | knowledge_retrieval::Locator::Title { start, end, .. }
        | knowledge_retrieval::Locator::Metadata { start, end, .. } => (
            Some(start.saturating_add(origin)),
            Some(end.saturating_add(origin)),
        ),
    }
}
use std::collections::BTreeSet;
struct DeliveryFile {
    path: String,
    score: Option<f32>,
    evidence: Vec<wire::Evidence>,
    annotations: NativeQueryAnnotations,
}
struct DeliveryList {
    ids: Vec<String>,
    files: BTreeMap<String, DeliveryFile>,
}
/// Context at the actual selected method boundary, without replacing its owning error.
#[derive(Debug)]
struct FailedMethod(wire::SearchMethod);
impl std::fmt::Display for FailedMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native query method {:?}", self.0)
    }
}
impl std::error::Error for FailedMethod {}
struct ControllerData {
    sources: Vec<NativeQuerySource>,
    permits: Vec<(usize, String, Permit)>,
    pages: Vec<NativeInventoryPage>,
    credits: Vec<Holding>,
    extra: Vec<Work>,
    failure: Option<anyhow::Error>,
    closing: bool,
    unavailable: u64,
    coverage: Option<wire::Coverage>,
}
/// Encoded response bytes and their existing prepaid Work loan.
/// A transport can move this value into a queue or Arc; bytes die before credit on last drop.
/// It contains no authority and must be produced only after controller delivery rechecks.
pub struct EncodedResponse {
    bytes: Vec<u8>,
    _work: Work,
}
impl std::ops::Deref for EncodedResponse {
    type Target = [u8];
    /// Borrow the bytes while their actual serialization loan remains owned.
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}
/// All-method fusion and final delivery for one admitted tuple query.
/// Winning source and metadata scopes stay live until `finish`.
pub struct NativeQueryController {
    query: NativeTupleQuery,
    claim: Arc<Claim>,
    result: Option<wire::SearchResult>,
    data: Option<Box<ControllerData>>,
    base: Option<Work>,
}
impl NativeTupleQuery {
    /// Method recorded at the actual failing boundary, including preflight. DATA, never a grant.
    pub fn failure_method(error: &anyhow::Error) -> Option<wire::SearchMethod> {
        error.downcast_ref::<FailedMethod>().map(|failed| failed.0)
    }
    /// Typed authority/budget failures remain observable without diagnostic parsing.
    pub fn failure_code(error: &anyhow::Error) -> wire::ErrorCode {
        if let Some(refusal) = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<crate::native_service::source::NativeRefusal>())
        {
            return refusal.0;
        }
        if error
            .chain()
            .any(|cause| cause.is::<QueryBudgetExhausted>())
        {
            return wire::ErrorCode::BudgetExhausted;
        }
        if let Some(error) = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<knowledge_retrieval::Error>())
        {
            match error {
                knowledge_retrieval::Error::ReadBudget
                | knowledge_retrieval::Error::Reservation => {
                    return wire::ErrorCode::BudgetExhausted;
                }
                // These are the kernel's structured error codes, never its diagnostic message.
                knowledge_retrieval::Error::Kernel(error) => {
                    return match error.code.as_str() {
                        "invalid-request" => wire::ErrorCode::InvalidRequest,
                        "resource-limited" => wire::ErrorCode::BudgetExhausted,
                        "cancelled" => wire::ErrorCode::Cancelled,
                        "changed" => wire::ErrorCode::Changed,
                        _ => wire::ErrorCode::Internal,
                    };
                }
                _ => {}
            }
        }
        if let Some(error) = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<knowledge_retrieval::KernelError>())
        {
            return match error.code.as_str() {
                "invalid-request" => wire::ErrorCode::InvalidRequest,
                "resource-limited" => wire::ErrorCode::BudgetExhausted,
                "cancelled" => wire::ErrorCode::Cancelled,
                "changed" => wire::ErrorCode::Changed,
                _ => wire::ErrorCode::Internal,
            };
        }
        wire::ErrorCode::Internal
    }
    /// Conservative peak for sequential whole-library methods plus retained winning scopes.
    pub fn requirement_all(&self) -> Result<Reservation> {
        self.cause.check()?;
        let mut memory = Self::metadata_requirement().memory_bytes;
        let mut execution_peak = 0u64;
        for (index, lib) in self.cause.libraries.iter().enumerate() {
            let wire::Command::Query { query } = lib
                .reader
                .permit
                .admitted_command()
                .map_err(refused_error)?
            else {
                bail!("actual Query required");
            };
            execution_peak = execution_peak.max(self.bm25_requirement(index)?.memory_bytes);
            if query.methods.contains(&wire::SearchMethod::Semantic) {
                // Missing actual native model selection is a provider refusal, not a lexical fallback.
                if lib.model.is_none() {
                    return Err(refused_error(wire::ErrorCode::ProviderUnavailable)
                        .context(FailedMethod(wire::SearchMethod::Semantic)));
                }
                execution_peak = execution_peak.max(
                    self.vector_requirement(index)
                        .with_context(|| FailedMethod(wire::SearchMethod::Semantic))?
                        .memory_bytes
                        .checked_add(64 * 1024 * 1024)
                        .context("semantic peak overflow")?,
                );
            }
            let methods = (query.methods.len().max(1) as u64)
                .checked_mul(query.keywords.len().max(1) as u64)
                .and_then(|n| n.checked_mul(query.limit.get()));
            let extra = methods
                .and_then(|n| n.checked_mul(2 * 1024 * 1024))
                .context("retained source quote overflow")?;
            memory = memory
                .checked_add(extra)
                .context("controller requirement overflow")?;
        }
        Ok(Reservation {
            memory_bytes: memory
                .checked_add(execution_peak)
                .context("controller peak overflow")?,
            temporary_bytes: 0,
        })
    }
    /// Run every requested method and fuse one labelled group per library.
    pub fn start_controller(&self, claim: Arc<Claim>) -> Result<NativeQueryController> {
        self.cause.check_claim(&claim)?;
        anyhow::ensure!(
            !self.cause.libraries.is_empty(),
            "actual Query libraries required"
        );
        let base = Work::split_claim(
            &claim,
            self.key(),
            Reservation {
                memory_bytes: 1024 * 1024,
                temporary_bytes: 0,
            },
        )?;
        let mut controller = NativeQueryController {
            query: NativeTupleQuery {
                cause: self.cause.clone(),
            },
            claim,
            result: None,
            data: Some(Box::new(ControllerData {
                sources: Vec::new(),
                permits: Vec::new(),
                pages: Vec::new(),
                credits: Vec::new(),
                extra: Vec::new(),
                failure: None,
                closing: false,
                unavailable: 0,
                coverage: None,
            })),
            base: Some(base),
        };
        if let Err(error) = controller.execute() {
            controller.data.as_mut().expect("actual owner").failure = Some(error);
        }
        Ok(controller)
    }
}
impl NativeQueryController {
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.data.as_ref()?.failure.as_ref()
    }
    /// Actual failed method; absence means failure before a selected method boundary.
    pub fn failure_method(&self) -> Option<wire::SearchMethod> {
        self.failure().and_then(NativeTupleQuery::failure_method)
    }
    /// Actual typed failure code, retaining the original owning error in this controller.
    pub fn failure_code(&self) -> Option<wire::ErrorCode> {
        self.failure().map(NativeTupleQuery::failure_code)
    }
    /// Recheck the claim and every retained winner, then borrow the fused result.
    pub fn search_result(&self) -> Result<&wire::SearchResult> {
        self.recheck()?;
        self.result.as_ref().context("actual search result absent")
    }
    /// Recheck winners and prepay both DTO copies and serialization before allocation.
    /// The returned carrier retains the existing loan through the transport's last consumer.
    pub fn encode_response(&mut self, request_id: wire::Id) -> Result<EncodedResponse> {
        self.recheck()?;
        let search = self
            .result
            .as_ref()
            .context("actual search result absent")?;
        let mut counted = Count(0);
        serde_json::to_writer(&mut counted, search)?;
        let memory = counted
            .0
            .checked_mul(16)
            .and_then(|n| n.checked_add((request_id.as_str().len() as u64).checked_mul(8)?))
            .and_then(|n| n.checked_add(4096))
            .context("response serialization peak overflow")?;
        let mut work = Work::split_claim(
            &self.claim,
            self.query.key(),
            Reservation {
                memory_bytes: memory,
                temporary_bytes: 0,
            },
        )?;
        let response = wire::Response {
            wire_version: 1,
            request_id,
            result: wire::Result::Search {
                search: Box::new(search.clone()),
            },
        };
        let encoded = wire::encode_response(&response)
            .map_err(|error| anyhow::anyhow!("actual search response refused: {error}"));
        drop(response);
        let bytes = match encoded {
            Ok(bytes) => bytes,
            Err(error) => {
                if work.rejoin(&self.claim, self.query.key()).is_err() {
                    self.data.as_mut().expect("actual owner").extra.push(work);
                }
                return Err(error);
            }
        };
        if let Err(error) = self.recheck() {
            drop(bytes);
            if work.rejoin(&self.claim, self.query.key()).is_err() {
                self.data.as_mut().expect("actual owner").extra.push(work);
            }
            return Err(error);
        }
        Ok(EncodedResponse { bytes, _work: work })
    }
    #[cfg(test)]
    pub fn test_scope_clone(&self) -> Option<Permit> {
        let data = self.data.as_ref()?;
        data.sources
            .first()
            .map(|source| source.source().test_query_scope_clone())
            .or_else(|| data.permits.first().map(|(_, _, permit)| permit.clone()))
    }
    pub fn finish(mut self) -> std::result::Result<(NativeTupleQuery, Arc<Claim>), Box<Self>> {
        drop(self.result.take());
        if self.data.is_some() {
            self.data.as_mut().expect("actual owner").closing = true;
            let sources = std::mem::take(&mut self.data.as_mut().expect("actual owner").sources);
            for source in sources {
                let _ = self.close_source(source);
            }
            let permits = std::mem::take(&mut self.data.as_mut().expect("actual owner").permits);
            for (_, _, permit) in permits {
                let _ = self.rejoin_permit(permit);
            }
            if !self.data.as_ref().expect("actual owner").permits.is_empty() {
                return Err(Box::new(self));
            }
            let mut pages =
                std::mem::take(&mut self.data.as_mut().expect("actual owner").pages).into_iter();
            while let Some(page) = pages.next() {
                match page.into_funding() {
                    Ok(mut credit) => {
                        if Account::current()
                            .and_then(|account| {
                                account.rejoin_claim(&mut credit, &self.claim, self.query.key())
                            })
                            .is_err()
                        {
                            let data = self.data.as_mut().expect("actual owner");
                            data.credits.push(credit);
                            data.pages.extend(pages);
                            return Err(Box::new(self));
                        }
                    }
                    Err(page) => {
                        let data = self.data.as_mut().expect("actual owner");
                        data.pages.push(page);
                        data.pages.extend(pages);
                        return Err(Box::new(self));
                    }
                }
            }
            if !self.data.as_ref().expect("actual owner").credits.is_empty() {
                return Err(Box::new(self));
            }
            let mut extra = std::mem::take(&mut self.data.as_mut().expect("actual owner").extra);
            while let Some(mut work) = extra.pop() {
                if work.rejoin(&self.claim, self.query.key()).is_err() {
                    self.data.as_mut().expect("actual owner").extra.push(work);
                    self.data
                        .as_mut()
                        .expect("actual owner")
                        .extra
                        .extend(extra);
                    return Err(Box::new(self));
                }
            }
            drop(self.data.take());
        }
        let mut base = self.base.take().expect("actual administrative loan");
        if base.rejoin(&self.claim, self.query.key()).is_err() {
            self.base = Some(base);
            return Err(Box::new(self));
        }
        Ok((self.query, self.claim))
    }
    fn recheck(&self) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        let data = self.data.as_ref().context("native controller consumed")?;
        anyhow::ensure!(
            data.failure.is_none() && !data.closing,
            "native controller result is not deliverable"
        );
        for source in &data.sources {
            source.source().check()?;
        }
        for (_, _, permit) in &data.permits {
            permit.check().map_err(refused_error)?;
        }
        Ok(())
    }
    fn close_source(&mut self, source: NativeQuerySource) -> Result<()> {
        match self.query.finish_source(source, &self.claim) {
            Ok(()) => Ok(()),
            Err(wire::authority::ScopeRejoinError::Retained { scope, .. }) => {
                self.data.as_mut().expect("actual owner").permits.push((
                    usize::MAX,
                    "retained".into(),
                    *scope,
                ));
                bail!("winning scope remains live")
            }
            Err(wire::authority::ScopeRejoinError::Released(code)) => Err(refused_error(code)),
        }
    }
    fn rejoin_permit(&mut self, permit: Permit) -> Result<()> {
        match permit.rejoin_processing_scope(&self.claim, self.query.key()) {
            Ok(()) => Ok(()),
            Err(wire::authority::ScopeRejoinError::Retained { scope, .. }) => {
                self.data.as_mut().expect("actual owner").permits.push((
                    usize::MAX,
                    "retained".into(),
                    *scope,
                ));
                bail!("winning scope remains live")
            }
            Err(wire::authority::ScopeRejoinError::Released(code)) => Err(refused_error(code)),
        }
    }
    fn execute(&mut self) -> Result<()> {
        let applied = {
            let wire::Command::Query { query } = self.query.cause.libraries[0]
                .reader
                .permit
                .admitted_command()
                .map_err(refused_error)?
            else {
                bail!("actual Query required");
            };
            query.clone()
        };
        let mut groups = Vec::new();
        for library in 0..self.query.cause.libraries.len() {
            let library_id = self.query.cause.libraries[library]
                .reader
                .physical
                .library
                .clone();
            groups.push(self.execute_library(&library_id, library)?);
        }
        self.result = Some(wire::SearchResult { applied, groups });
        Ok(())
    }
    fn execute_library(
        &mut self,
        library_id: &wire::Id,
        library: usize,
    ) -> Result<wire::SearchGroup> {
        let lib = &self.query.cause.libraries[library];
        anyhow::ensure!(
            *library_id == lib.reader.physical.library,
            "actual library_id/library owner differs"
        );
        let wire::Command::Query { query } = lib
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required");
        };
        let query = query.clone();
        let library_id = lib.reader.physical.library.clone();
        let generation_id = wire::Id::new(lib.reader.physical.data.scan.clone()).ok();
        let partitioning_id = wire::Id::new(lib.reader.physical.data.descriptor.clone()).ok();
        self.data.as_mut().expect("actual owner").coverage = None;
        let jobs = method_jobs(&query);
        let mut lists: Vec<Vec<String>> = Vec::new();
        let mut files: BTreeMap<String, DeliveryFile> = BTreeMap::new();
        for job in jobs {
            let method = match &job {
                MethodJob::Browse => None,
                MethodJob::Meta(method, _) | MethodJob::Scan(method, _) => Some(*method),
                MethodJob::Bm25(_) => Some(wire::SearchMethod::Bm25),
                MethodJob::Semantic(_) => Some(wire::SearchMethod::Semantic),
            };
            let list =
                self.run_job(&library_id, library, &query, job)
                    .map_err(|error| match method {
                        Some(method) => error.context(FailedMethod(method)),
                        None => error,
                    })?;
            lists.push(list.ids);
            for (id, file) in list.files {
                if let Some(existing) = files.get_mut(&id) {
                    existing.evidence.extend(file.evidence);
                } else {
                    files.insert(id, file);
                }
            }
        }
        let limit = query.limit.get() as usize;
        let (order, fused_scores) = fuse_delivery(&files, &lists);
        if lists.len() != 1 {
            for (id, score) in fused_scores {
                if let Some(file) = files.get_mut(&id) {
                    file.score = Some(score);
                }
            }
        }
        let winners: Vec<String> = order.into_iter().take(limit).collect();
        let winner_set: BTreeSet<String> = winners.iter().cloned().collect();
        let limit_reached = files.len() > limit;
        self.retain_winners(library, &winner_set)?;
        let mut group_files = Vec::new();
        for (index, id) in winners.iter().enumerate() {
            let Some(file) = files.get_mut(id) else {
                continue;
            };
            dedup_evidence(&mut file.evidence);
            let located = file
                .evidence
                .iter()
                .filter(|item| !matches!(item.locator, wire::Locator::Unlocated))
                .count() as u64;
            let rank = (index as u64) + 1;
            group_files.push(wire::FileHit {
                source: wire_source(library_id.clone(), &file.annotations)?,
                rank: wire::PositiveInteger::new(rank).map_err(|_| anyhow::anyhow!("file rank"))?,
                score: file
                    .score
                    .map(|score| {
                        wire::FiniteNumber::new(score as f64)
                            .map_err(|_| anyhow::anyhow!("file score"))
                    })
                    .transpose()?,
                retained_evidence_count: wire::U64::new(file.evidence.len() as u64),
                useful_location_count: wire::U64::new(located),
                evidence: file.evidence.clone(),
            });
        }
        let coverage = self
            .data
            .as_mut()
            .expect("actual owner")
            .coverage
            .take()
            .context("actual method coverage absent")?;
        Ok(wire::SearchGroup {
            library_id,
            generation_id,
            partitioning_id,
            files: group_files,
            coverage,
            returned_files: wire::UnsignedInteger::new(winners.len() as u64)
                .map_err(|_| anyhow::anyhow!("returned files"))?,
            limit_reached,
        })
    }
    fn retain_winners(&mut self, library: usize, winners: &BTreeSet<String>) -> Result<()> {
        let sources = std::mem::take(&mut self.data.as_mut().expect("actual owner").sources);
        let mut kept = Vec::new();
        let mut sources = sources.into_iter();
        while let Some(source) = sources.next() {
            if source.library != library
                || winners.contains(source.annotations().source_id.as_str())
            {
                kept.push(source);
            } else if let Err(error) = self.close_source(source) {
                self.data
                    .as_mut()
                    .expect("actual owner")
                    .sources
                    .extend(kept);
                self.data
                    .as_mut()
                    .expect("actual owner")
                    .sources
                    .extend(sources);
                return Err(error);
            }
        }
        self.data.as_mut().expect("actual owner").sources = kept;
        let permits = std::mem::take(&mut self.data.as_mut().expect("actual owner").permits);
        let mut kept_permits = Vec::new();
        let mut permits = permits.into_iter();
        while let Some((slot, id, permit)) = permits.next() {
            if slot != library || winners.contains(&id) {
                kept_permits.push((slot, id, permit));
            } else if let Err(error) = self.rejoin_permit(permit) {
                self.data
                    .as_mut()
                    .expect("actual owner")
                    .permits
                    .extend(kept_permits);
                self.data
                    .as_mut()
                    .expect("actual owner")
                    .permits
                    .extend(permits);
                return Err(error);
            }
        }
        self.data.as_mut().expect("actual owner").permits = kept_permits;
        Ok(())
    }
    /// Preserve processing observations even when a metadata method ran first.
    /// Independent method visits do not add duplicate file counts.
    fn keep_indexed_coverage(&mut self, indexed: wire::Coverage) {
        let slot = &mut self.data.as_mut().expect("actual owner").coverage;
        if let Some(current) = slot {
            current.unsupported =
                wire::U64::new(current.unsupported.get().max(indexed.unsupported.get()));
            current.failed = wire::U64::new(current.failed.get().max(indexed.failed.get()));
            current.pending_extraction = wire::U64::new(
                current
                    .pending_extraction
                    .get()
                    .max(indexed.pending_extraction.get()),
            );
            current.unavailable =
                wire::U64::new(current.unavailable.get().max(indexed.unavailable.get()));
        } else {
            *slot = Some(indexed);
        }
    }
    fn run_job(
        &mut self,
        library_id: &wire::Id,
        library: usize,
        query: &wire::SearchRequest,
        job: MethodJob,
    ) -> Result<DeliveryList> {
        match job {
            MethodJob::Browse => self.run_metadata(library_id, library, None, query),
            MethodJob::Meta(method, keyword) => {
                self.run_metadata(library_id, library, Some((method, keyword)), query)
            }
            MethodJob::Bm25(keyword) => self.run_bm25(library_id, library, keyword, query),
            MethodJob::Scan(method, keyword) => {
                self.run_scan(library_id, library, method, keyword, query)
            }
            MethodJob::Semantic(keyword) => self.run_semantic(library_id, library, keyword, query),
        }
    }
    fn run_bm25(
        &mut self,
        library_id: &wire::Id,
        library: usize,
        keyword: usize,
        query: &wire::SearchRequest,
    ) -> Result<DeliveryList> {
        let mut run = self
            .query
            .start_bm25(self.claim.clone(), library, keyword)?;
        loop {
            let step = run.step_library(library_id);
            if step.is_failed() {
                self.data.as_mut().expect("actual owner").unavailable += run.unavailable_parts();
                return Err(MethodRunFailure(Some(MethodRun::Bm25(Box::new(run)))).into());
            }
            if step.is_complete() {
                break;
            }
        }
        self.data.as_mut().expect("actual owner").unavailable += run.unavailable_parts();
        self.keep_indexed_coverage(run.data.as_ref().expect("actual owner").coverage.wire());
        let list = capture_bm25(&run, &query.keywords[keyword])?;
        let sources = run.take_sources();
        let pages = run.take_retained_pages();
        run.finish()
            .map_err(|_| anyhow::anyhow!("bm25 finish retained a scope"))?;
        let data = self.data.as_mut().expect("actual owner");
        data.sources.extend(sources.into_values());
        data.pages.extend(pages);
        Ok(list)
    }
    fn run_scan(
        &mut self,
        library_id: &wire::Id,
        library: usize,
        method: wire::SearchMethod,
        keyword: usize,
        query: &wire::SearchRequest,
    ) -> Result<DeliveryList> {
        let mut run = self
            .query
            .start_scan(self.claim.clone(), library, method, keyword)?;
        loop {
            let step = run.step_library(library_id);
            if step.is_failed() {
                self.data.as_mut().expect("actual owner").unavailable += run.unavailable_parts();
                return Err(MethodRunFailure(Some(MethodRun::Scan(Box::new(run)))).into());
            }
            if step.is_complete() {
                break;
            }
        }
        self.data.as_mut().expect("actual owner").unavailable += run.unavailable_parts();
        self.keep_indexed_coverage(run.data.as_ref().expect("actual owner").coverage.wire());
        let list = capture_scan(&run, method, &query.keywords[keyword])?;
        let sources = run.take_sources();
        let pages = run.take_retained_pages();
        run.finish()
            .map_err(|_| anyhow::anyhow!("scan finish retained a scope"))?;
        let data = self.data.as_mut().expect("actual owner");
        data.sources.extend(sources.into_values());
        data.pages.extend(pages);
        Ok(list)
    }
    fn run_metadata(
        &mut self,
        library_id: &wire::Id,
        library: usize,
        method: Option<(wire::SearchMethod, usize)>,
        query: &wire::SearchRequest,
    ) -> Result<DeliveryList> {
        let mut run = self
            .query
            .start_metadata_for(self.claim.clone(), library, method)?;
        loop {
            let step = run.step_library(library_id);
            if step.is_failed() {
                self.data.as_mut().expect("actual owner").unavailable += run.unavailable_sources();
                return Err(MethodRunFailure(Some(MethodRun::Metadata(Box::new(run)))).into());
            }
            if step.is_complete() {
                break;
            }
        }
        self.data.as_mut().expect("actual owner").unavailable += run.unavailable_sources();
        let keyword = method.map(|(_, index)| query.keywords[index].as_str());
        let method = method.map(|(method, _)| method);
        let counts = run.data.as_ref().expect("actual owner");
        let mut coverage = QueryCoverage {
            discovered: counts.discovered,
            verified: counts.verified,
            unavailable: counts.unavailable_sources,
            ..QueryCoverage::default()
        }
        .wire();
        coverage.pending_hash = wire::U64::new(counts.pending_hash);
        if let Some(indexed) = &self.data.as_ref().expect("actual owner").coverage {
            coverage.unsupported = indexed.unsupported;
            coverage.failed = indexed.failed;
            coverage.pending_extraction = indexed.pending_extraction;
        }
        self.data.as_mut().expect("actual owner").coverage = Some(coverage);
        let hits: Vec<NativeQueryAnnotations> = run.hits()?.cloned().collect();
        let scopes = run.take_scopes();
        let pages = run.take_retained_pages();
        run.finish()
            .map_err(|_| anyhow::anyhow!("metadata finish retained a scope"))?;
        self.data
            .as_mut()
            .expect("actual owner")
            .pages
            .extend(pages);
        let mut ids = Vec::new();
        let mut files = BTreeMap::new();
        for annotations in hits {
            let id = annotations.source_id.as_str().to_owned();
            ids.push(id.clone());
            files.insert(
                id,
                DeliveryFile {
                    path: annotations.path.as_str().to_owned(),
                    score: None,
                    evidence: vec![metadata_evidence(&annotations, method, keyword)?],
                    annotations,
                },
            );
        }
        self.data
            .as_mut()
            .expect("actual owner")
            .permits
            .extend(scopes.into_iter().map(|(annotations, permit)| {
                (library, annotations.source_id.as_str().to_owned(), permit)
            }));
        Ok(DeliveryList { ids, files })
    }
    fn run_semantic(
        &mut self,
        library_id: &wire::Id,
        library: usize,
        keyword: usize,
        query: &wire::SearchRequest,
    ) -> Result<DeliveryList> {
        let mut run =
            self.query
                .start_semantic_for(library_id, self.claim.clone(), library, keyword);
        if run.failure().is_some() {
            self.data.as_mut().expect("actual owner").unavailable += run.unavailable_parts();
            return Err(MethodRunFailure(Some(MethodRun::Semantic(Box::new(run)))).into());
        }
        self.data.as_mut().expect("actual owner").unavailable += run.unavailable_parts();
        self.keep_indexed_coverage(run.coverage()?);
        let keyword = query.keywords[keyword].as_str();
        let mut ids = Vec::new();
        let mut files = BTreeMap::new();
        for (id, score) in run.hits()?.iter() {
            let Some(annotations) = run.source_annotations(id)?.cloned() else {
                continue;
            };
            let evidence = run
                .evidence_for(id)?
                .iter()
                .map(|item| semantic_evidence(&run, id, item, keyword, *score))
                .collect::<Result<Vec<_>>>()?;
            ids.push(id.clone());
            files.insert(
                id.clone(),
                DeliveryFile {
                    path: annotations.path.as_str().to_owned(),
                    score: Some(*score),
                    evidence,
                    annotations,
                },
            );
        }
        let sources = run.take_sources();
        let pages = run.take_retained_pages();
        run.finish()
            .map_err(|_| anyhow::anyhow!("semantic finish retained a scope"))?;
        let data = self.data.as_mut().expect("actual owner");
        data.sources.extend(sources.into_values());
        data.pages.extend(pages);
        Ok(DeliveryList { ids, files })
    }
}
// Retain original owning failures until their existing finish/adoption path runs. In particular,
// page/scope errors must remain inside the run so its real loans can be recovered and rejoined.
enum MethodRun {
    Bm25(Box<NativeBm25Run>),
    Scan(Box<NativeScanRun>),
    Metadata(Box<NativeMetadataRun>),
    Semantic(Box<NativeWholeSemanticRun>),
}
struct MethodRunFailure(Option<MethodRun>);
impl std::fmt::Debug for MethodRunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MethodRunFailure")
    }
}
impl std::fmt::Display for MethodRunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native query method failed")
    }
}
impl std::error::Error for MethodRunFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.0.as_ref()? {
            MethodRun::Bm25(run) => run.failure(),
            MethodRun::Scan(run) => run.failure(),
            MethodRun::Metadata(run) => run.failure(),
            MethodRun::Semantic(run) => run.failure(),
        }
        .map(|error| error.as_ref())
    }
}
impl Drop for MethodRunFailure {
    fn drop(&mut self) {
        if let Some(run) = self.0.take() {
            match run {
                MethodRun::Bm25(run) => {
                    let _ = (*run).finish();
                }
                MethodRun::Scan(run) => {
                    let _ = (*run).finish();
                }
                MethodRun::Metadata(run) => {
                    let _ = (*run).finish();
                }
                MethodRun::Semantic(run) => {
                    let _ = (*run).finish();
                }
            }
        }
    }
}
enum MethodJob {
    Browse,
    Meta(wire::SearchMethod, usize),
    Bm25(usize),
    Scan(wire::SearchMethod, usize),
    Semantic(usize),
}
fn method_jobs(query: &wire::SearchRequest) -> Vec<MethodJob> {
    if query.keywords.is_empty() {
        return vec![MethodJob::Browse];
    }
    let mut jobs = Vec::new();
    for method in &query.methods {
        for keyword in 0..query.keywords.len() {
            jobs.push(match method {
                wire::SearchMethod::Bm25 => MethodJob::Bm25(keyword),
                wire::SearchMethod::Literal | wire::SearchMethod::Regex => {
                    MethodJob::Scan(*method, keyword)
                }
                wire::SearchMethod::Semantic => MethodJob::Semantic(keyword),
                wire::SearchMethod::Files | wire::SearchMethod::Tags => {
                    MethodJob::Meta(*method, keyword)
                }
            });
        }
    }
    jobs
}
fn fuse_delivery(
    files: &BTreeMap<String, DeliveryFile>,
    lists: &[Vec<String>],
) -> (Vec<String>, Vec<(String, f32)>) {
    if lists.len() <= 1 {
        return (lists.first().cloned().unwrap_or_default(), Vec::new());
    }
    let mut fused = knowledge_search::fuse_ranks(lists);
    fused.sort_by(|left, right| {
        right.1.total_cmp(&left.1).then_with(|| {
            let left_path = files
                .get(&left.0)
                .map(|file| file.path.as_str())
                .unwrap_or("");
            let right_path = files
                .get(&right.0)
                .map(|file| file.path.as_str())
                .unwrap_or("");
            left_path.cmp(right_path).then(left.0.cmp(&right.0))
        })
    });
    let order = fused.iter().map(|(id, _)| id.clone()).collect();
    (order, fused)
}
fn capture_bm25(run: &NativeBm25Run, keyword: &str) -> Result<DeliveryList> {
    let mut ids = Vec::new();
    let mut files = BTreeMap::new();
    for hit in run.hits()? {
        let id = hit.key.id.clone();
        let Some(annotations) = run.source_annotations(&id)?.cloned() else {
            continue;
        };
        ids.push(id.clone());
        files.insert(
            id,
            DeliveryFile {
                path: annotations.path.as_str().to_owned(),
                score: hit.score,
                evidence: vec![bm25_evidence(run, hit, keyword)?],
                annotations,
            },
        );
    }
    Ok(DeliveryList { ids, files })
}
fn capture_scan(
    run: &NativeScanRun,
    method: wire::SearchMethod,
    keyword: &str,
) -> Result<DeliveryList> {
    let mut ids = Vec::new();
    let mut files = BTreeMap::new();
    for id in run.ranked()? {
        let Some(annotations) = run.source_annotations(&id)?.cloned() else {
            continue;
        };
        let evidence = scan_wire_evidence(run, &id, method, keyword)?;
        ids.push(id.clone());
        files.insert(
            id,
            DeliveryFile {
                path: annotations.path.as_str().to_owned(),
                score: None,
                evidence,
                annotations,
            },
        );
    }
    Ok(DeliveryList { ids, files })
}
fn scan_wire_evidence(
    run: &NativeScanRun,
    source_id: &str,
    method: wire::SearchMethod,
    keyword: &str,
) -> Result<Vec<wire::Evidence>> {
    let data = run.data.as_ref().context("native scan consumed")?;
    let Some(file) = data.files.get(source_id) else {
        return Ok(Vec::new());
    };
    file.occurrences
        .iter()
        .map(|item| {
            let locator = match (item.start, item.end, item.metadata) {
                (Some(start), Some(end), false) => {
                    let source = data.sources.get(source_id).context("actual scan source")?;
                    representation_locator(
                        &run.query,
                        &run.claim,
                        source,
                        &item.representation_digest,
                        start,
                        end,
                    )?
                }
                _ => wire::Locator::Unlocated,
            };
            Ok(wire::Evidence {
                method,
                keyword: Some(keyword.to_owned()),
                locator,
                snippet: item.snippet.clone(),
                kind: if item.metadata {
                    wire::EvidenceKind::MetadataObservation
                } else {
                    wire::EvidenceKind::VerifiedMatch
                },
                source_digest: wire::Digest::new(item.source_digest.clone()).ok(),
                representation_digest: wire::Digest::new(item.representation_digest.clone()).ok(),
                processor_id: item.processor.clone(),
                model_space: None,
                score: None,
            })
        })
        .collect()
}
fn bm25_evidence(run: &NativeBm25Run, hit: &Hit, keyword: &str) -> Result<wire::Evidence> {
    let source = run
        .data
        .as_ref()
        .context("actual bm25 owner")?
        .sources
        .get(&hit.key.id)
        .context("actual bm25 source")?;
    let (locator, metadata) = match &hit.locator {
        knowledge_retrieval::Locator::Text {
            start,
            end,
            original: Some(original),
        } => {
            let (digest, origin) = original
                .strip_prefix("representation:")
                .and_then(|text| text.split_once('@'))
                .context("actual BM25 representation coordinate")?;
            let origin = origin.parse::<u64>()?;
            (
                representation_locator(
                    &run.query,
                    &run.claim,
                    source,
                    digest,
                    origin
                        .checked_add(*start)
                        .context("BM25 coordinate overflow")?,
                    origin
                        .checked_add(*end)
                        .context("BM25 coordinate overflow")?,
                )?,
                false,
            )
        }
        _ => (wire::Locator::Unlocated, true),
    };
    Ok(wire::Evidence {
        method: wire::SearchMethod::Bm25,
        keyword: Some(keyword.to_owned()),
        locator,
        snippet: hit.snippet.chars().take(500).collect(),
        kind: if metadata {
            wire::EvidenceKind::MetadataObservation
        } else {
            wire::EvidenceKind::VerifiedMatch
        },
        source_digest: wire::Digest::new(hit.digest.clone()).ok(),
        representation_digest: wire::Digest::new(source.part.representation_digest.clone()).ok(),
        processor_id: hit.processor.clone(),
        model_space: None,
        score: hit
            .score
            .map(|score| {
                wire::FiniteNumber::new(score as f64).map_err(|_| anyhow::anyhow!("score"))
            })
            .transpose()?,
    })
}
fn metadata_evidence(
    annotations: &NativeQueryAnnotations,
    method: Option<wire::SearchMethod>,
    keyword: Option<&str>,
) -> Result<wire::Evidence> {
    Ok(wire::Evidence {
        method: method.unwrap_or(wire::SearchMethod::Files),
        keyword: keyword.map(str::to_owned),
        locator: wire::Locator::Unlocated,
        snippet: annotations.title.chars().take(500).collect(),
        kind: wire::EvidenceKind::MetadataObservation,
        source_digest: annotations.digest.clone(),
        representation_digest: None,
        processor_id: None,
        model_space: None,
        score: None,
    })
}
fn semantic_evidence(
    run: &NativeWholeSemanticRun,
    id: &str,
    item: &knowledge_search::VectorEvidence,
    keyword: &str,
    score: f32,
) -> Result<wire::Evidence> {
    Ok(wire::Evidence {
        method: wire::SearchMethod::Semantic,
        keyword: Some(keyword.to_owned()),
        locator: vector_locator(run, id, &item.locator)?,
        snippet: String::new(),
        kind: wire::EvidenceKind::SemanticCandidate,
        source_digest: wire::Digest::new(item.source_digest.clone()).ok(),
        representation_digest: wire::Digest::new(item.representation.digest.clone()).ok(),
        processor_id: Some(item.representation.processor.clone()),
        model_space: Some(run.model_space()?.to_owned()),
        score: Some(
            wire::FiniteNumber::new(score as f64).map_err(|_| anyhow::anyhow!("semantic score"))?,
        ),
    })
}
fn vector_locator(run: &NativeWholeSemanticRun, id: &str, text: &str) -> Result<wire::Locator> {
    let Some(rest) = text.strip_prefix("representation:") else {
        return Ok(wire::Locator::Unlocated);
    };
    let Some((digest, span)) = rest.split_once('@') else {
        return Ok(wire::Locator::Unlocated);
    };
    let Some((start, end)) = span.split_once("..") else {
        return Ok(wire::Locator::Unlocated);
    };
    let (Ok(start), Ok(end)) = (start.parse::<u64>(), end.parse::<u64>()) else {
        return Ok(wire::Locator::Unlocated);
    };
    if wire::Digest::new(digest.to_owned()).is_err() {
        return Ok(wire::Locator::Unlocated);
    }
    run.evidence_locator(id, digest, start, end)
}
/// Map admitted global representation bytes to real lines with bounded, prepaid tape reads.
/// Only an identical full original/representation digest permits original byte coordinates.
pub(super) fn representation_locator(
    query: &NativeTupleQuery,
    claim: &Arc<Claim>,
    source: &NativeQuerySource,
    digest: &str,
    start: u64,
    end: u64,
) -> Result<wire::Locator> {
    source.source().check()?;
    anyhow::ensure!(
        digest == source.part.representation_digest && start <= end,
        "representation identity/span differs"
    );
    let mut work = Work::split_claim(
        claim,
        query.key(),
        Reservation {
            memory_bytes: 64 * 1024,
            temporary_bytes: 0,
        },
    )?;
    let located = (|| {
        let reader = &query.cause.libraries[source.library].reader;
        let file = reader.physical.open_verified_admitted(
            "representations.data",
            || source.source().check(),
            &|n| query.cause.admit_io(claim, n),
        )?;
        let base = source
            .part
            .tape_start
            .checked_sub(source.part.representation_start)
            .context("representation base underflow")?;
        anyhow::ensure!(
            base.checked_add(end)
                .is_some_and(|n| n <= file.metadata().map_or(0, |m| m.len())),
            "representation coordinate outside held tape"
        );
        let mut bytes = vec![0u8; 64 * 1024];
        let mut at = 0u64;
        let mut first = 1u64;
        let mut last = 1u64;
        while at < end {
            source.source().check()?;
            let n = (end - at).min(bytes.len() as u64) as usize;
            query.cause.admit_io(claim, n as u64)?;
            file.read_exact_at(&mut bytes[..n], base + at)?;
            for (index, byte) in bytes[..n].iter().enumerate() {
                if *byte == b'\n' {
                    if at + (index as u64) < start {
                        first += 1;
                    }
                    if at + (index as u64) < end.saturating_sub(1) {
                        last += 1;
                    }
                }
            }
            at += n as u64;
        }
        source.source().check()?;
        let start_line = wire::PositiveInteger::new(first).map_err(|_| anyhow::anyhow!("line"))?;
        let end_line =
            wire::PositiveInteger::new(last.max(first)).map_err(|_| anyhow::anyhow!("line"))?;
        if source
            .annotations()
            .digest
            .as_ref()
            .is_some_and(|value| value.as_str() == digest)
        {
            Ok(wire::Locator::OriginalLines {
                start_line,
                end_line,
                start_byte: Some(wire::U64::new(start)),
                end_byte: Some(wire::U64::new(end)),
            })
        } else {
            Ok(wire::Locator::RepresentationLines {
                representation_digest: wire::Digest::new(digest.to_owned())?,
                start_line,
                end_line,
            })
        }
    })();
    // All tape descriptors and scratch have died before this synchronous exact-loan rejoin.
    work.rejoin(claim, query.key())?;
    located
}
fn wire_source(library_id: wire::Id, annotations: &NativeQueryAnnotations) -> Result<wire::Source> {
    Ok(wire::Source {
        library_id,
        source_id: annotations.source_id.clone(),
        mount_id: None,
        path: annotations.path.clone(),
        logical_path: annotations.path.clone(),
        title: annotations.title.clone(),
        description: annotations.description.clone(),
        tags: annotations.tags.clone(),
        annotation_path: annotations.annotation_path.clone(),
        digest: annotations.digest.clone(),
        digest_state: annotations.digest_state,
        version: None,
        size_bytes: None,
        media_type: None,
        created_at: None,
        modified_at: None,
        availability: annotations.availability,
        residency: wire::Residency::Unknown,
        activity: wire::Activity::Stable,
        conflict_source_id: None,
    })
}
fn dedup_evidence(items: &mut Vec<wire::Evidence>) {
    let located = items
        .iter()
        .any(|item| !matches!(item.locator, wire::Locator::Unlocated));
    if located {
        items.retain(|item| !matches!(item.locator, wire::Locator::Unlocated));
    }
    let mut at = 0;
    while at < items.len() {
        if items[..at].iter().any(|prior| {
            prior.method == items[at].method
                && prior.keyword == items[at].keyword
                && prior.locator == items[at].locator
                && prior.representation_digest == items[at].representation_digest
        }) {
            items.remove(at);
        } else {
            at += 1;
        }
    }
}
