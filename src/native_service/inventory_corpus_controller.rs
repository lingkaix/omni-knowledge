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
    metadata_capacity: u64,
    metadata_used: u64,
    done: bool,
    closing: bool,
    failure: Option<anyhow::Error>,
}
enum RowBuild {
    Skip {
        unavailable: bool,
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
}
#[cfg(test)]
fn record_frontmatter_read(read: u64, admitted: u64) {
    TEST_FRONTMATTER_MAX_READ.with(|max_read| {
        if read > max_read.get() {
            max_read.set(read);
            TEST_FRONTMATTER_MAX_ADMITTED.with(|max_admitted| max_admitted.set(admitted));
        }
    });
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
    fn advance(&mut self, service: &NativeLibraryService) -> Result<bool> {
        self.query.cause.check_claim(&self.claim)?;
        anyhow::ensure!(
            service.library
                == self.query.cause.libraries[self.library]
                    .reader
                    .physical
                    .library,
            "actual service/library owner differs"
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
            self.consider(service, observed)?;
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
    fn consider(
        &mut self,
        service: &NativeLibraryService,
        observed: NativeObservedSource,
    ) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        let path = wire::RelativePath::new(observed.observation().stat.path.as_str())?;
        let permit = &self.query.cause.libraries[self.library].reader.permit;
        match permit.check_path(&service.library, &path) {
            Err(wire::ErrorCode::NotExposed) => return Ok(()),
            Err(code) => return Err(refused_error(code)),
            Ok(()) => {}
        }
        let pointer_rel = wire::RelativePath::new(format!("sources/{}.md", observed.source_id()))?;
        let pointer = open_pointer(permit, &service.library, pointer_rel)?;
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
        match self.classify(service, &scope, observed, path, pointer) {
            Ok(RowBuild::Skip { unavailable }) => {
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
                if unavailable {
                    self.data
                        .as_mut()
                        .expect("actual owner")
                        .unavailable_sources += 1;
                }
                self.retain(annotations, scope)
            }
            Err(cause) => Err(self.abandon_scope(scope, cause)),
        }
    }
    fn classify(
        &self,
        service: &NativeLibraryService,
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
        let library_id = service.library.clone();
        let watched = path.clone();
        scope
            .retain_check(Arc::new(move || {
                checked.check_scope_claim(&live).is_ok()
                    && path_permit.check_path(&library_id, &watched).is_ok()
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
                .with_root(&service.library, |root| {
                    Ok(LibraryHost::open_root(root, path.as_str()))
                })
                .map_err(refused_error)?;
            match opened {
                Err(wire::ErrorCode::NotFound) => {
                    return Ok(RowBuild::Skip { unavailable: true });
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
                        return Ok(RowBuild::Skip { unavailable: true });
                    }
                    LibraryHost::track_version(
                        &service.library,
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
                let parsed = self.read_pointer(service, scope, pointer)?;
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
            Ok(RowBuild::Skip { unavailable })
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
        }
        let head_len = len.min(6);
        self.query.cause.admit_io(&self.claim, head_len)?;
        let mut head_bytes = vec![0u8; head_len as usize];
        file.read_exact_at(&mut head_bytes, 0)?;
        #[cfg(test)]
        let mut read = head_len;
        let matched =
            head_bytes.starts_with(b"---") || head_bytes.starts_with("\u{feff}---".as_bytes());
        if matched {
            let extra = len.min(64 * 1024) - head_len;
            if extra > 0 {
                self.query.cause.admit_io(&self.claim, extra)?;
                let mut rest = vec![0u8; extra as usize];
                file.read_exact_at(&mut rest, head_len)?;
                #[cfg(test)]
                {
                    read += extra;
                }
                head_bytes.extend_from_slice(&rest);
            }
        }
        #[cfg(test)]
        record_frontmatter_read(read, read);
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
        service: &NativeLibraryService,
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
            &service.library,
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
