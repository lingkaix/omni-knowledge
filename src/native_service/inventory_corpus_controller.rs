//! Bounded, owning ordinary-library BM25 walk over the SAME actual tuple/Claim/partition engine.
//! This is not activation or the all-method controller: semantic/scan/tag/metadata composition and
//! aggregate byte/object budget reporting still need integration before the application uses it.
use super::*;
use knowledge_retrieval::Hit;
use knowledge_search::FileRankWindow;
use std::num::NonZeroUsize;

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
        let Some(data) = self.data.as_ref() else {
            return NativeQueryStep::Failed;
        };
        if data.failure.is_some() || data.closing {
            return NativeQueryStep::Failed;
        }
        if data.done {
            return NativeQueryStep::Complete;
        }
        match self.advance(service) {
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
    fn advance(&mut self, service: &NativeLibraryService) -> Result<bool> {
        self.check()?;
        let library = &self.query.cause.libraries[self.library];
        anyhow::ensure!(
            service.library == library.reader.physical.library,
            "actual service/library owner differs"
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
                    let found = page.sources().find(|s| same_observed(s, source_id, &locator));
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
                    self.data.as_mut().expect("actual owner").unavailable_parts += 1;
                    self.data.as_mut().expect("actual owner").source = after;
                    continue;
                }
                bail!("actual M1 page source absent");
            };
            let result =
                self.query
                    .source_part(service, &self.claim, self.library, observed, part, after);
            match result {
                Ok(source) => match source.matches_required_tags() {
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
                },
                Err(error) => {
                    // An actual owning scope refusal is NEVER downgraded to an omitted DTO row.
                    if error
                        .downcast_ref::<crate::native_service::NativeRefreshScopeError>()
                        .is_none()
                        && error.chain().any(|e| {
                            e.is::<crate::native_service::source::CachedQuerySourceUnavailable>()
                        })
                    {
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
