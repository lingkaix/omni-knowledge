//! Actual bounded M1 -> M3 -> held representation -> SAME kernel corpus producer.
//! Drafts retain the real job/Claim; preparing lexical components is NOT tuple activation.
use super::*;
use knowledge_core::admission::{Decision, Waiting};
use knowledge_processing::embedding::funded::FundedTextProvider;
use knowledge_retrieval::KernelError;
use knowledge_retrieval::{
    ByteDirectory, ByteHandle, ByteLength, Content, Control, Document, KernelAdapter, ScopeView,
    SourceBytes, SourceKey, Work,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Seek, SeekFrom, Write};
use std::ops::Range;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[path = "inventory_corpus_query.rs"]
mod query;
pub use query::{
    NativePartitionResult, NativeQueryLibrary, NativeQueryLoans, NativeQueryPartition,
    NativeQuerySource, NativeQueryStage, NativeQueryStageRefusal, NativeQueryStageResources,
    NativeTupleQuery,
};
#[path = "inventory_corpus_reader.rs"]
mod reader;
#[path = "inventory_corpus_selection.rs"]
mod selection;
pub use reader::NativeTupleRead;
#[path = "inventory_corpus_tuple.rs"]
mod tuple;
#[path = "inventory_corpus_vectors.rs"]
mod vectors;

const METADATA: u64 = 8 * 1024 * 1024;
const METADATA_DISK: u64 = 64 * 1024;
const PAGE_MEMORY: u64 = 2 * 1024 * 1024;
const FRAME: usize = 1024 * 1024;
const TEXT_WINDOW: usize = 1024 * 1024;
const SHARD_TEXT: u64 = 8 * 1024 * 1024;
const READ_WINDOW: usize = 64 * 1024;

/// Actual Core resource contention or completed producer progress; never permission or settlement.
#[derive(Clone, Copy, Debug)]
pub enum NativeCorpusProgress {
    /// The SAME foreground owner retains buffers/FDs/Claim while Core cannot fund the next phase.
    Waiting(Waiting),
    /// Actual original sources consumed and actual lexical shards prepared so far.
    Prepared { sources: u64, shards: u64 },
    /// Actual saved representations produced a prepared, unselected vector component.
    VectorsPrepared { rows: u64 },
    /// Actual existing credit immediately before a tape/catalog append; DATA, never a grant.
    WriteAdmitted { bytes: u64, available: u64 },
    /// The actual same-library writer lease is contended; this foreground owner remains live.
    WriterWaiting,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourcePart {
    original_version: [u64; 8],
    page_offset: u64,
    slot: usize,
    embedding_rows: u64,
    tape_start: u64,
    tape_length: u64,
    representation_start: u64,
    representation_digest: String,
    format: String,
    status: String,
    document: Document,
}
#[derive(Serialize, Deserialize)]
struct Shard {
    scope: String,
    generation: String,
    page_offset: u64,
    parts: Vec<SourcePart>,
}
struct Stage {
    temporary: Option<tempfile::TempDir>,
    path: PathBuf,
}
impl Stage {
    fn new(temporary: tempfile::TempDir) -> Self {
        Self {
            path: temporary.path().to_owned(),
            temporary: Some(temporary),
        }
    }
    fn path(&self) -> &Path {
        &self.path
    }
    // Only the actual durable selector owner calls this, never a path/phase DTO.
    fn adopt(&mut self) {
        if let Some(actual) = self.temporary.take() {
            let _ = actual.keep();
        }
    }
}
struct Files {
    tape: Arc<File>,
    sources: File,
    catalog: File,
    inventory: File,
    directory: File,
    parent: File,
    volume_parent: File,
    parent_identity: (u64, u64),
    parent_path: PathBuf,
    stage: Stage,
}
impl Files {
    fn check(&self) -> Result<()> {
        let m = std::fs::symlink_metadata(&self.parent_path)?;
        anyhow::ensure!(
            m.is_dir() && (m.dev(), m.ino()) == self.parent_identity,
            "generated parent changed"
        );
        let actual = Account::current()?.generated_directory()?.metadata()?;
        let volume = self.volume_parent.metadata()?;
        anyhow::ensure!(
            (actual.dev(), actual.ino()) == (volume.dev(), volume.ino()),
            "different generated volume owner"
        );
        let held = self.directory.metadata()?;
        let now = std::fs::symlink_metadata(self.stage.path())?;
        anyhow::ensure!(
            now.is_dir() && (now.dev(), now.ino()) == (held.dev(), held.ino()),
            "generated namespace changed"
        );
        Ok(())
    }
}

/// Incomplete, actually owned corpus draft. No whole-corpus selection; aggregate Claim stays live.
/// The builder streams a single M1 page/scope; there is no resident corpus table or per-source FD set.
pub struct NativeCorpusDraft {
    job: Arc<NativeRefreshJob>,
    claim: Option<Claim>,
    shared_claim: Option<Arc<Claim>>,
    files: Option<Files>,
    page: Option<Arc<NativeInventoryPage>>,
    provider: Option<FundedTextProvider>,
    vectors: Option<Arc<knowledge_search::PreparedVectors>>,
    vectors_done: bool,
    tuple: Option<tuple::Seal>,
    selection: Option<selection::Selection>,
    #[cfg(test)]
    test_fail_after_switch: bool,
    sealed_files: Option<[[u64; 8]; 4]>,
    coverage: NativeCorpusCoverage,
    source_errors: Arc<Mutex<Vec<anyhow::Error>>>,
    refusals: Arc<Mutex<Vec<wire::authority::ScopeRejoinError>>>,
    sources: u64,
    shards: u64,
    rows: u64,
    source_bytes: u64,
    lexical_done: bool,
    // All actual data/FD/client/page objects above die BEFORE this metadata/write-window loan.
    work: Option<Work>,
}
/// Actual selected-scope M1 metadata coverage; never original bytes, authority or whole-tuple readiness.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct NativeCorpusCoverage {
    /// Actual current-scope M1 observations serialized without inventing digest facts.
    pub observations: u64,
    /// Nondeleted observations actually classified unavailable by M1.
    pub unavailable: u64,
    /// Actual current-scan M1 tombstones, never materialized as fake byte sources.
    pub tombstones: u64,
}
/// A failure owns its actual draft and Claim. No static residue or abandoned operation is created.
pub struct NativeCorpusError {
    cause: anyhow::Error,
    owner: Box<NativeCorpusDraft>,
}
impl std::fmt::Debug for NativeCorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeCorpusError")
    }
}
impl std::fmt::Display for NativeCorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native corpus draft failed; actual foreground ownership retained")
    }
}
impl std::error::Error for NativeCorpusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
impl NativeCorpusError {
    /// Inspect the actual typed cause without losing the real owner.
    pub fn cause(&self) -> &anyhow::Error {
        &self.cause
    }
    /// Retry or explicitly dispose the actual draft, not a reconstructed path/digest capability.
    pub fn into_parts(self) -> (NativeCorpusDraft, anyhow::Error) {
        (*self.owner, self.cause)
    }
}
impl NativeRefreshJob {
    /// Minimum bounded-scope producer envelope; caller includes any greater actual M3 need.
    /// Persistent generated bytes are reconciled from actual completed writes, not source_count*8MiB.
    pub fn corpus_requirement(&self) -> Result<Reservation> {
        self.cause.check()?;
        let kernel = KernelAdapter::publish_requirement(FRAME as u64)?;
        let provider = match &self.cause.model {
            Some(model) => FundedTextProvider::requirement(model.config()?)?.memory_bytes,
            None => 0,
        };
        Ok(Reservation {
            memory_bytes: kernel.memory_bytes
                + METADATA
                + PAGE_MEMORY
                + 128 * 1024
                + 2 * 1024 * 1024
                + provider,
            temporary_bytes: kernel.temporary_bytes + METADATA_DISK,
        })
    }
    /// Begin generated-cache work under actual Refresh intent and the SAME admitted Core volume FD.
    /// `generated_path` is metadata for the library's native path-based writer, never target authority.
    pub fn begin_corpus(
        self,
        claim: Claim,
        generated_path: &Path,
    ) -> std::result::Result<NativeCorpusDraft, NativeCorpusError> {
        let mut owner = NativeCorpusDraft {
            job: Arc::new(self),
            claim: Some(claim),
            shared_claim: None,
            files: None,
            page: None,
            provider: None,
            vectors: None,
            vectors_done: false,
            tuple: None,
            selection: None,
            #[cfg(test)]
            test_fail_after_switch: false,
            sealed_files: None,
            coverage: NativeCorpusCoverage::default(),
            source_errors: Arc::new(Mutex::new(Vec::with_capacity(64))),
            refusals: Arc::new(Mutex::new(Vec::with_capacity(64))),
            sources: 0,
            shards: 0,
            rows: 0,
            source_bytes: 0,
            lexical_done: false,
            work: None,
        };
        match owner.begin(generated_path) {
            Ok(()) => Ok(owner),
            Err(cause) => Err(owner.error(cause)),
        }
    }
}
impl NativeCorpusDraft {
    fn error(self, cause: anyhow::Error) -> NativeCorpusError {
        NativeCorpusError {
            cause,
            owner: Box::new(self),
        }
    }
    fn actual_claim(&self) -> Result<&Claim> {
        self.claim
            .as_ref()
            .or(self.shared_claim.as_deref())
            .context("actual corpus Claim unavailable")
    }
    fn check(&self) -> Result<()> {
        self.job.cause.check()?;
        self.job.permit.check().map_err(refused_error)?;
        let claim = self.actual_claim()?;
        anyhow::ensure!(
            Instant::now() < claim.deadline() && claim.stop_requested(self.job.key())?.is_none(),
            "corpus stopped"
        );
        if let Some(files) = &self.files {
            files.check()?;
            if let Some(expected) = self.sealed_files {
                anyhow::ensure!(
                    [
                        stamp(&files.tape)?,
                        stamp(&files.sources)?,
                        stamp(&files.catalog)?,
                        stamp(&files.inventory)?
                    ] == expected,
                    "prepared actual corpus descriptors changed"
                );
            }
            if let Some(tuple) = &self.tuple {
                tuple.check(files)?;
            }
        }
        if let Some(selection) = &self.selection {
            selection.check(self)?;
        }
        Ok(())
    }
    fn begin(&mut self, path: &Path) -> Result<()> {
        self.check()?;
        anyhow::ensure!(
            matches!(
                self.job.permit.admitted_command().map_err(refused_error)?,
                wire::Command::Refresh { .. }
            ),
            "actual Refresh write intention required"
        );
        let need = self.job.corpus_requirement()?;
        let available = Account::current()?
            .job_status(self.actual_claim()?.job(), self.job.key())?
            .execution;
        anyhow::ensure!(
            available.memory_bytes >= need.memory_bytes
                && available.temporary_bytes >= need.temporary_bytes,
            "complete corpus scope is not prepaid"
        );
        self.work = Some(Work::split_claim(
            self.actual_claim()?,
            self.job.key(),
            Reservation {
                memory_bytes: METADATA,
                temporary_bytes: METADATA_DISK,
            },
        )?);
        if let Some(model) = &self.job.cause.model {
            model.check_permit(&self.job.permit)?;
            let mut funding = knowledge_processing::funding::Funding::ClaimStage {
                claim: self.claim.as_mut().expect("actual claim"),
                expected: self.job.key(),
            };
            // SAME actual model/receiver + aggregate claim, BEFORE any M3 can spend remaining RSS.
            self.provider = Some(FundedTextProvider::open(model.config()?, &mut funding)?);
        }
        let parent = Account::current()?.generated_directory()?;
        let m = parent.metadata()?;
        let now = std::fs::symlink_metadata(path)?;
        anyhow::ensure!(
            now.is_dir() && (now.dev(), now.ino()) == (m.dev(), m.ino()),
            "path is not the actual selected generated directory"
        );
        let volume_parent = parent;
        self.selection = Some(selection::Selection::open(self)?);
        let selected = self
            .selection
            .as_ref()
            .context("actual library tuple namespace")?;
        let parent = selected.directory()?;
        let parent_path = selected.path(path);
        let m = parent.metadata()?;
        let named = std::fs::symlink_metadata(&parent_path)?;
        anyhow::ensure!(
            named.is_dir() && (named.dev(), named.ino()) == (m.dev(), m.ino()),
            "tuple namespace path is not its actual directory"
        );
        let stage = tempfile::Builder::new()
            .prefix("native-corpus-")
            .tempdir_in(&parent_path)?;
        let directory = File::open(stage.path())?;
        let create = |name: &str| -> Result<File> {
            Ok(File::from(rustix::fs::openat(
                &directory,
                name,
                rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::from_bits_truncate(0o600),
            )?))
        };
        let tape = Arc::new(create("representations.data")?);
        let sources = create("sources.rows")?;
        let catalog = create("lexical.rows")?;
        let inventory = create("inventory.rows")?;
        self.files = Some(Files {
            tape,
            sources,
            catalog,
            inventory,
            directory,
            parent,
            volume_parent,
            parent_identity: (m.dev(), m.ino()),
            parent_path,
            stage: Stage::new(stage),
        });
        self.check()
    }
    /// Exact persisted current-scope observations, including honest unavailable/tombstone facts.
    pub fn coverage(&self) -> NativeCorpusCoverage {
        self.coverage
    }
    /// Real consumed source/shard/vector-row counts, not tuple readiness.
    pub fn counts(&self) -> (u64, u64, u64) {
        (self.sources, self.shards, self.rows)
    }
    /// Actual generated namespace for bounded inspection/recovery, not a new disclosure/write grant.
    #[cfg(test)]
    pub(in crate::native_service) fn staging_directory(&self) -> Result<&File> {
        self.check()?;
        Ok(&self.files.as_ref().context("actual namespace")?.directory)
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_vectors(
        &self,
    ) -> Option<&Arc<knowledge_search::PreparedVectors>> {
        self.vectors.as_ref()
    }
    #[cfg(test)]
    pub(in crate::native_service) fn job_key_for_test(&self) -> knowledge_core::jobs::Key<'_> {
        self.job.key()
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_retained_scope(&mut self) -> Result<u64> {
        self.check()?;
        let account = Account::current()?;
        let mut funding = account.split_claim(
            self.actual_claim()?,
            self.job.key(),
            Reservation {
                memory_bytes: PAGE_MEMORY + 128 * 1024,
                temporary_bytes: 0,
            },
        )?;
        let budget = Budget {
            max_files: 64,
            max_bytes: PAGE_MEMORY,
            max_millis: 60000,
            max_memory_bytes: PAGE_MEMORY,
        };
        let mut offset = 0;
        let (page, observed) = loop {
            let page = Arc::new(self.job.page(offset, budget, funding)?);
            let observed = page
                .sources()
                .find(|source| !metadata_only(source.observation()));
            if let Some(observed) = observed {
                break (page, observed);
            }
            let next = page.next_offset();
            let complete = page.complete();
            let page = Arc::try_unwrap(page).map_err(|page| {
                self.page = Some(page);
                anyhow::anyhow!("actual test page still owned")
            })?;
            funding = page.into_funding().map_err(|page| {
                self.page = Some(Arc::new(page));
                anyhow::anyhow!("actual test metadata still owned")
            })?;
            if complete || next <= offset {
                funding.finish()?;
                bail!("actual source row");
            }
            offset = next;
        };
        let before = account
            .job_status(self.actual_claim()?.job(), self.job.key())?
            .execution
            .memory_bytes;
        let processing = self.job.processing(
            self.actual_claim()?,
            observed,
            self.actual_claim()?.budget(),
        )?;
        let (scope, matches) = processing.into_refresh_scope(&self.job.cause);
        anyhow::ensure!(matches, "actual same Refresh cause required");
        let pin = scope.clone();
        let refusal = scope
            .rejoin_processing_scope(self.actual_claim()?, self.job.key())
            .err()
            .context("actual live SDK clone must retain scope")?;
        self.refusals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(refusal);
        let after = account
            .job_status(self.actual_claim()?.job(), self.job.key())?
            .execution
            .memory_bytes;
        drop(pin);
        self.page = Some(page);
        Ok(before - after)
    }
    fn cleanup_scopes(&mut self) -> Result<()> {
        let errors =
            std::mem::take(&mut *self.source_errors.lock().unwrap_or_else(|p| p.into_inner()));
        let mut scopes =
            std::mem::take(&mut *self.refusals.lock().unwrap_or_else(|p| p.into_inner()));
        for error in errors {
            if let Ok(actual) = error.downcast::<NativeRefreshScopeError>() {
                let (scope, cause) = actual.into_parts();
                drop(cause);
                scopes.push(scope);
            }
        }
        if scopes.is_empty() {
            return Ok(());
        }
        if self.actual_claim().is_err() {
            self.refusals
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .extend(scopes);
            bail!("actual scoped cleanup still requires its owning Claim");
        }
        let claim = self.actual_claim().expect("actual Claim retained above");
        let mut retained = Vec::new();
        for actual in scopes {
            match actual {
                wire::authority::ScopeRejoinError::Retained { scope, .. } => {
                    if let Err(actual) = scope.rejoin_processing_scope(claim, self.job.key()) {
                        retained.push(actual);
                    }
                }
                wire::authority::ScopeRejoinError::Released(_) => {} // Actual pins/data already died; its holding was explicitly finished.
            }
        }
        let complete = retained.is_empty();
        self.refusals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .extend(retained);
        anyhow::ensure!(complete, "actual SDK scopes still have live owners");
        Ok(())
    }
    /// End this synchronous draft's buffers/FDs first and return its SAME actual job and Claim
    /// without implicit success/queue settlement. A still-live native reader returns its actual owner.
    pub fn into_job(
        mut self,
    ) -> std::result::Result<(Arc<NativeRefreshJob>, Claim), NativeCorpusError> {
        if let Err(cause) = self.resolve_publication() {
            return Err(self.error(cause));
        }
        if self
            .vectors
            .as_ref()
            .is_some_and(|v| Arc::strong_count(v) > 1)
        {
            return Err(self.error(anyhow::anyhow!(
                "actual vector consumer still owns preparation"
            )));
        }
        if let Some(shared) = self.shared_claim.take() {
            match Arc::try_unwrap(shared) {
                Ok(claim) => self.claim = Some(claim),
                Err(actual) => {
                    self.shared_claim = Some(actual);
                    return Err(self.error(anyhow::anyhow!("actual reader still owns Claim")));
                }
            }
        }
        if let Err(cause) = self.cleanup_scopes() {
            return Err(self.error(cause));
        }
        self.page.take();
        self.provider.take();
        self.vectors.take();
        self.tuple.take();
        self.files.take();
        self.selection.take();
        self.work.take();
        match self.claim.take() {
            Some(claim) => Ok((self.job.clone(), claim)),
            None => Err(self.error(anyhow::anyhow!("actual Claim still in native producer"))),
        }
    }
    /// Stream actual source pages and mature M3 exactly once, producing persistent held lexical inputs.
    /// Waiting reports the actual Core decision while this owner keeps all buffers/FDs/Claim live.
    pub fn build_lexical(
        mut self,
        mut progress: impl FnMut(NativeCorpusProgress),
    ) -> std::result::Result<Self, NativeCorpusError> {
        match self.build(&mut progress) {
            Ok(()) => Ok(self),
            Err(cause) => Err(self.error(cause)),
        }
    }
    /// Prepare existing vectors from saved actual M3 bytes; never selects CURRENT.
    pub fn build_vectors(
        mut self,
        mut progress: impl FnMut(NativeCorpusProgress),
    ) -> std::result::Result<Self, NativeCorpusError> {
        match vectors::prepare(&mut self, &mut progress) {
            Ok(()) => Ok(self),
            Err(cause) => Err(self.error(cause)),
        }
    }
    /// Seal ONE immutable lexical/vector/source/model tuple with bounded descriptor-held IO.
    /// This is preparation only: no whole selection, response completion or implicit settlement.
    pub fn prepare_tuple(
        mut self,
        mut progress: impl FnMut(NativeCorpusProgress),
    ) -> std::result::Result<Self, NativeCorpusError> {
        if self.tuple.is_some() {
            let checked = (|| {
                self.check()?;
                self.tuple
                    .as_ref()
                    .expect("actual tuple")
                    .verify_members(self.files.as_ref().context("actual tuple files")?, || {
                        self.check()
                    })
            })();
            return match checked {
                Ok(()) => Ok(self),
                Err(cause) => Err(self.error(cause)),
            };
        }
        match tuple::prepare(&mut self, &mut progress) {
            Ok(seal) => {
                self.tuple = Some(seal);
                Ok(self)
            }
            Err(cause) => Err(self.error(cause)),
        }
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_tuple_header(&self) -> Result<serde_json::Value> {
        self.check()?;
        Ok(serde_json::to_value(
            &self
                .tuple
                .as_ref()
                .context("actual prepared tuple")?
                .header_data,
        )?)
    }
    /// Select only the complete sealed tuple under the actual same-library writer lease/CAS.
    /// Unknown post-switch durability returns this actual foreground owner; no component selects alone.
    pub fn select_tuple(
        mut self,
        mut progress: impl FnMut(NativeCorpusProgress),
    ) -> std::result::Result<Self, NativeCorpusError> {
        let mut state = match self.selection.take() {
            Some(state) => state,
            None => return Err(self.error(anyhow::anyhow!("actual selector owner unavailable"))),
        };
        let result = state.select(&mut self, &mut progress);
        self.selection = Some(state);
        match result {
            Ok(()) => Ok(self),
            Err(cause) => Err(self.error(cause)),
        }
    }
    fn resolve_publication(&mut self) -> Result<()> {
        let Some(mut state) = self.selection.take() else {
            return Ok(());
        };
        let result = if state.phase == selection::Phase::Switched {
            state.recover(self)
        } else {
            state.discard_candidate()
        };
        self.selection = Some(state);
        result
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_switch_fault(&mut self) {
        self.test_fail_after_switch = true;
    }
    #[cfg(test)]
    pub(in crate::native_service) fn test_selected_pointer(&self) -> Result<serde_json::Value> {
        self.check()?;
        Ok(serde_json::to_value(
            self.selection
                .as_ref()
                .context("actual selection")?
                .selected_pointer()?,
        )?)
    }
    fn write_checkpoint(&self, input: Option<&NativeIndexInput>) -> Result<()> {
        if let Some(input) = input {
            input.source()?;
            self.job.cause.check()?;
            self.files.as_ref().context("actual destination")?.check()?;
            Ok(())
        } else {
            self.check()
        }
    }
    fn wait_growth(
        &mut self,
        bytes: u64,
        input: Option<&NativeIndexInput>,
        progress: &mut impl FnMut(NativeCorpusProgress),
    ) -> Result<()> {
        let bytes = bytes
            .checked_add(METADATA_DISK)
            .context("generated metadata peak overflow")?;
        loop {
            self.write_checkpoint(input)?;
            let work = self.work.as_mut().context("actual corpus metadata owner")?;
            let remaining = work.remaining()?.temporary_bytes;
            if remaining >= bytes {
                progress(NativeCorpusProgress::WriteAdmitted {
                    bytes: bytes - METADATA_DISK,
                    available: remaining,
                });
                self.write_checkpoint(input)?; // An observer can request cancellation or change the selected context.
                return Ok(());
            }
            match work.grow(Reservation {
                memory_bytes: 0,
                temporary_bytes: bytes - remaining,
            })? {
                Decision::Ready(()) => continue,
                Decision::Waiting(reason) => {
                    progress(NativeCorpusProgress::Waiting(reason));
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
    fn phase_storage(
        &mut self,
        bytes: u64,
        progress: &mut impl FnMut(NativeCorpusProgress),
    ) -> Result<()> {
        let available = Account::current()?
            .job_status(self.actual_claim()?.job(), self.job.key())?
            .execution
            .temporary_bytes;
        if available >= bytes {
            return Ok(());
        }
        // Existing affine primitive: no data/FD is loaned by this zero-sized replenishment bridge.
        let mut bridge = Work::split_claim(
            self.actual_claim()?,
            self.job.key(),
            Reservation {
                memory_bytes: 0,
                temporary_bytes: 0,
            },
        )?;
        loop {
            self.check()?;
            match bridge.grow(Reservation {
                memory_bytes: 0,
                temporary_bytes: bytes - available,
            })? {
                Decision::Ready(()) => break,
                Decision::Waiting(reason) => {
                    progress(NativeCorpusProgress::Waiting(reason));
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        bridge.rejoin(self.actual_claim()?, self.job.key())?;
        Ok(())
    }
    fn build(&mut self, progress: &mut impl FnMut(NativeCorpusProgress)) -> Result<()> {
        anyhow::ensure!(!self.lexical_done, "corpus lexical draft already produced");
        let mut offset = 0;
        loop {
            self.check()?;
            let budget = Budget {
                max_files: 64,
                max_bytes: self.actual_claim()?.budget().max_bytes,
                max_millis: self.actual_claim()?.budget().max_millis,
                max_memory_bytes: PAGE_MEMORY,
            };
            let funding = Account::current()?.split_claim(
                self.actual_claim()?,
                self.job.key(),
                Reservation {
                    memory_bytes: PAGE_MEMORY + 128 * 1024,
                    temporary_bytes: 0,
                },
            )?;
            let page = Arc::new(self.job.page(offset, budget, funding)?);
            self.page = Some(page.clone());
            anyhow::ensure!(
                page.issue().is_none(),
                "corpus requires complete actual inventory replay"
            );
            self.persist_inventory_page(&page, progress)?;
            let next = page.next_offset();
            let complete = page.complete();
            let mut parts = Vec::with_capacity(64);
            let mut bytes = 0;
            let mut encoded = 0usize;
            for (slot, observed) in page.sources().enumerate() {
                self.check()?;
                // An actual failed M1 verification is a discoverable metadata outcome, not permission
                // to invent an original digest or retry a denied body through M3/provider contact.
                if metadata_only(observed.observation()) {
                    continue;
                }
                let size = observed.observation().stat.size_bytes;
                self.source_bytes = self
                    .source_bytes
                    .checked_add(size)
                    .context("source work overflow")?;
                anyhow::ensure!(
                    self.sources < self.actual_claim()?.budget().max_files
                        && self.source_bytes <= self.actual_claim()?.budget().max_bytes,
                    "aggregate source budget exhausted"
                );
                self.phase_storage(512 * 1024 * 1024, progress)?;
                let verification = self.actual_claim()?.budget();
                let processing = self
                    .job
                    .processing(self.actual_claim()?, observed, verification)
                    .context("corpus: admit current M1 source")?;
                // Quote the SAME verified source's actual extent, not every source's whole request ceiling.
                let max_bytes = usize::try_from(size.max(1))?; // Mature API requires a positive bound even for actual EOF.
                let start = self
                    .files
                    .as_ref()
                    .expect("files")
                    .sources
                    .metadata()?
                    .len();
                let claim = self.claim.take().expect("actual owned Claim");
                let input = match processing.extract_corpus_input(claim, max_bytes, None) {
                    Ok(input) => input,
                    Err(error) => {
                        let (processing, claim, cause) = error.into_parts();
                        self.claim = Some(claim);
                        self.finish_processing(processing)?;
                        return Err(cause);
                    }
                };
                let written = self.persist_source(offset, slot, &input, progress);
                let finished = input.finish_stage();
                let (processing, claim) = match finished {
                    Ok(value) => value,
                    Err(error) => {
                        let (processing, claim, cause) = error.into_parts();
                        self.claim = Some(claim);
                        self.finish_processing(processing)?;
                        return Err(cause);
                    }
                };
                self.claim = Some(claim);
                self.finish_processing(processing)?;
                written.context("corpus: persist actual M3 representation")?;
                self.sources += 1;
                // Read one bounded actual metadata frame at a time; never retain a corpus inventory.
                let end = self
                    .files
                    .as_ref()
                    .expect("files")
                    .sources
                    .metadata()?
                    .len();
                let mut cursor = start;
                while cursor < end {
                    let (part, next) = read_frame::<SourcePart>(
                        &self.files.as_ref().expect("files").sources,
                        cursor,
                    )?;
                    let duplicate = parts
                        .iter()
                        .any(|p: &SourcePart| p.document.key == part.document.key);
                    let part_bytes = usize::try_from(next - cursor)?;
                    anyhow::ensure!(
                        part_bytes <= FRAME - 4096,
                        "one source metadata frame exceeds native catalog format"
                    );
                    if !parts.is_empty()
                        && (duplicate
                            || parts.len() == 64
                            || bytes + part.tape_length > SHARD_TEXT
                            || encoded + part_bytes > FRAME - 4096)
                    {
                        self.flush(offset, &page, std::mem::take(&mut parts), progress)?;
                        bytes = 0;
                        encoded = 0;
                    }
                    encoded += part_bytes;
                    bytes += part.tape_length;
                    parts.push(part);
                    cursor = next;
                }
                progress(NativeCorpusProgress::Prepared {
                    sources: self.sources,
                    shards: self.shards,
                });
            }
            if !parts.is_empty() {
                self.flush(offset, &page, parts, progress)?;
            }
            drop(page);
            let page = self.page.take().expect("page");
            let page = Arc::try_unwrap(page).map_err(|page| {
                self.page = Some(page);
                anyhow::anyhow!("actual page still has live consumers")
            })?;
            let mut funding = page.into_funding().map_err(|page| {
                self.page = Some(Arc::new(page));
                anyhow::anyhow!("actual page loan still pinned")
            })?;
            Account::current()?.rejoin_claim(&mut funding, self.actual_claim()?, self.job.key())?;
            if complete {
                break;
            }
            anyhow::ensure!(next > offset, "inventory replay made no progress");
            offset = next;
        }
        self.files.as_ref().expect("files").tape.sync_all()?;
        self.files.as_ref().expect("files").sources.sync_all()?;
        self.files.as_ref().expect("files").catalog.sync_all()?;
        self.files.as_ref().expect("files").inventory.sync_all()?;
        let files = self.files.as_ref().expect("files");
        self.sealed_files = Some([
            stamp(&files.tape)?,
            stamp(&files.sources)?,
            stamp(&files.catalog)?,
            stamp(&files.inventory)?,
        ]);
        self.lexical_done = true;
        self.check()
    }
    fn persist_inventory_page(
        &mut self,
        page: &NativeInventoryPage,
        progress: &mut impl FnMut(NativeCorpusProgress),
    ) -> Result<()> {
        for row in &page.page.replay.observations {
            self.check()?;
            let inventory = &page.page.inventory;
            if row.scan != inventory.scan
                || row.root != inventory.root
                || row.stat.backend != inventory.mount.context()
            {
                continue;
            }
            let path = row.stat.path.as_str();
            if inventory.policy.0.read.outside_scope.iter().any(|p| {
                path == p.as_str()
                    || path
                        .strip_prefix(p.as_str())
                        .is_some_and(|tail| tail.starts_with('/'))
            }) {
                continue;
            }
            let wire::Command::Refresh { library_id, .. } =
                self.job.permit.admitted_command().map_err(refused_error)?
            else {
                bail!("actual Refresh metadata intent required")
            };
            self.job
                .permit
                .check_path(library_id, &wire::RelativePath::new(path)?)
                .map_err(refused_error)?;
            anyhow::ensure!(
                self.coverage.observations < self.actual_claim()?.budget().max_files,
                "aggregate metadata observation budget exhausted"
            );
            let bytes = frame_bytes(row)?;
            self.wait_growth(bytes.len() as u64, None, progress)?;
            let file = &mut self.files.as_mut().expect("files").inventory;
            let before = file.metadata()?.len();
            let result = file.write_all(&bytes);
            let after = file.metadata()?.len();
            self.work.as_mut().expect("work").written(
                after
                    .checked_sub(before)
                    .context("inventory size regressed")?,
            )?;
            result?;
            self.coverage.observations += 1;
            if row.change == ChangeKind::Deleted {
                self.coverage.tombstones += 1;
            } else if metadata_only(row) {
                self.coverage.unavailable += 1;
            }
        }
        Ok(())
    }
    fn finish_processing(&mut self, processing: NativeProcessing) -> Result<()> {
        if let Err(error) = self.job.finish_source(processing, self.actual_claim()?) {
            self.refusals
                .lock()
                .map_err(|_| anyhow::anyhow!("scope owner poisoned"))?
                .push(error);
            bail!("actual source cleanup remains owned");
        }
        Ok(())
    }
    fn persist_source(
        &mut self,
        page_offset: u64,
        slot: usize,
        input: &NativeIndexInput,
        progress: &mut impl FnMut(NativeCorpusProgress),
    ) -> Result<()> {
        let extraction = input.extraction()?;
        let text = input.text()?;
        let source = input.source()?;
        let original_version = stamp(source.file()?)?;
        let raw_size = original_version[2];
        let source_id = input.source_id()?;
        let source_digest = input.source_digest()?;
        let representation = input.representation()?;
        let hint = source
            .hint()
            .to_str()
            .context("source display path is not UTF-8")?;
        let name = source
            .hint()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(hint);
        let tape_start = self.files.as_ref().expect("files").tape.metadata()?.len();
        self.wait_growth(text.len() as u64, Some(input), progress)?;
        let before = tape_start;
        let mut target = self.files.as_ref().expect("files").tape.as_ref();
        target.seek(SeekFrom::End(0))?;
        let written = target.write_all(text.as_bytes());
        let after = target.metadata()?.len();
        self.work.as_mut().expect("work").written(
            after
                .checked_sub(before)
                .context("representation size regressed")?,
        )?;
        written?;
        let mut position: usize = 0;
        loop {
            input.source()?;
            let mut end = position.saturating_add(TEXT_WINDOW).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let chunk = &text[position..end];
            let mut metadata = BTreeMap::new();
            metadata.insert(
                "representation_digest".into(),
                representation.digest.clone(),
            );
            metadata.insert("representation_start".into(), position.to_string());
            metadata.insert("processing_status".into(), extraction.status.clone());
            let content = if text.is_empty() {
                Content::Unavailable
            } else {
                Content::Representation {
                    path: format!(
                        "representation-{}-{}",
                        tape_start + position as u64,
                        chunk.len()
                    ),
                    digest: hex::encode(Sha256::digest(chunk.as_bytes())),
                    size: chunk.len() as u64,
                    processor: representation.processor.clone(),
                    fragments: vec![knowledge_retrieval::RepresentationFragment {
                        start: 0,
                        end: chunk.len() as u64,
                        locator: format!("representation:{}@{}", representation.digest, position),
                    }],
                    complete: extraction.status == "extracted",
                }
            };
            let document = Document {
                key: SourceKey {
                    namespace: self.job.cause.library.as_str().to_owned(),
                    id: source_id.to_owned(),
                },
                revision: source_digest.to_owned(),
                digest: source_digest.to_owned(),
                size: raw_size,
                name: name.to_owned(),
                title: name.to_owned(),
                media_type: extraction.format.clone(),
                content,
                text_ranges: None,
                language: Some("unicode".into()),
                metadata,
                locator: Some(hint.to_owned()),
            };
            let part = SourcePart {
                original_version,
                page_offset,
                slot,
                embedding_rows: text_windows(chunk).filter(|s| !s.trim().is_empty()).count() as u64,
                tape_start: tape_start + position as u64,
                tape_length: chunk.len() as u64,
                representation_start: position as u64,
                representation_digest: representation.digest.clone(),
                format: extraction.format.clone(),
                status: extraction.status.clone(),
                document,
            };
            let bytes = frame_bytes(&part)?;
            self.wait_growth(bytes.len() as u64, Some(input), progress)?;
            let file = &mut self.files.as_mut().expect("files").sources;
            let before = file.metadata()?.len();
            let result = file.write_all(&bytes);
            let after = file.metadata()?.len();
            self.work.as_mut().expect("work").written(
                after
                    .checked_sub(before)
                    .context("source metadata size regressed")?,
            )?;
            result?;
            self.rows = self
                .rows
                .checked_add(part.embedding_rows)
                .context("vector row count overflow")?;
            if end == text.len() {
                break;
            }
            position = end;
        }
        Ok(())
    }
    fn flush(
        &mut self,
        page_offset: u64,
        page: &Arc<NativeInventoryPage>,
        parts: Vec<SourcePart>,
        progress: &mut impl FnMut(NativeCorpusProgress),
    ) -> Result<()> {
        self.check()?;
        self.phase_storage(512 * 1024 * 1024, progress)?;
        self.files.as_ref().expect("files").tape.sync_all()?;
        let scope = format!("native-shard-{}", self.shards);
        let view = ScopeView {
            scope: scope.clone(),
            documents: parts.iter().map(|p| p.document.clone()).collect(),
            unavailable_objects: 0,
        };
        let encoded = serde_json::to_vec(&view)?;
        anyhow::ensure!(
            encoded.len() <= FRAME,
            "native bounded scope encoding exceeded its prepaid frame"
        );
        let need = KernelAdapter::publish_requirement(encoded.len() as u64)?;
        drop(encoded);
        let mut work = Work::split_claim(self.actual_claim()?, self.job.key(), need)?;
        let claim = Arc::new(self.claim.take().expect("actual claim"));
        self.shared_claim = Some(claim.clone());
        let files = self.files.as_ref().expect("files");
        let data = Arc::new(DirectoryInputs {
            job: self.job.clone(),
            claim: claim.clone(),
            page: page.clone(),
            tape: files.tape.clone(),
            version: stamp(&files.tape)?,
            parts: parts.clone(),
            source_errors: self.source_errors.clone(),
            refusals: self.refusals.clone(),
            target: TargetLease {
                parent: files.parent.try_clone()?,
                directory: files.directory.try_clone()?,
                parent_path: files.parent_path.clone(),
                stage_path: files.stage.path().to_owned(),
            },
        });
        let guard_data = data.clone();
        let control = Control::guarded(
            claim.deadline(),
            Arc::new(move || guard_data.check().is_ok()),
        );
        let result = (|| -> Result<String> {
            let native = KernelAdapter::open_held(
                files.stage.path(),
                &files.stage.path().join("kernel"),
                &admitted::build_identity(),
                Arc::new(NativeInputs(data.clone())),
                self.work.as_ref().expect("metadata"),
                &control,
            )?;
            Ok(native.publish(view, None, &mut work, &control)?)
        })();
        drop(control);
        drop(data);
        drop(claim);
        let shared = self.shared_claim.take().expect("shared actual claim");
        match Arc::try_unwrap(shared) {
            Ok(claim) => self.claim = Some(claim),
            Err(shared) => {
                self.shared_claim = Some(shared);
                bail!("actual native reader still retains Claim");
            }
        }
        work.rejoin(self.actual_claim()?, self.job.key())?;
        anyhow::ensure!(
            self.refusals
                .lock()
                .map_err(|_| anyhow::anyhow!("scope owner poisoned"))?
                .is_empty(),
            "actual source scopes remain owned"
        );
        let source_error = self
            .source_errors
            .lock()
            .map_err(|_| anyhow::anyhow!("source owner poisoned"))?
            .pop();
        if let Some(error) = source_error {
            return Err(error);
        }
        let generation = result.context("corpus: native lexical publication")?;
        let record = Shard {
            scope,
            generation,
            page_offset,
            parts,
        };
        let bytes = frame_bytes(&record)?;
        self.wait_growth(bytes.len() as u64, None, progress)?;
        let file = &mut self.files.as_mut().expect("files").catalog;
        let before = file.metadata()?.len();
        let result = file.write_all(&bytes);
        let after = file.metadata()?.len();
        self.work.as_mut().expect("work").written(
            after
                .checked_sub(before)
                .context("catalog size regressed")?,
        )?;
        result?;
        self.shards += 1;
        Ok(())
    }
}
impl Drop for NativeCorpusDraft {
    fn drop(&mut self) {
        // A possibly switched pointer is actual foreground work, never a static residue then exit.
        while self.resolve_publication().is_err() {
            std::thread::sleep(Duration::from_millis(250));
        }
        // All producers here are synchronous/collected. Actual still-borrowed scopes stay foreground-owned.
        while self.cleanup_scopes().is_err() {
            std::thread::sleep(Duration::from_millis(250));
        }
        if let Some(mut shared) = self.shared_claim.take() {
            loop {
                match Arc::try_unwrap(shared) {
                    Ok(claim) => {
                        self.claim = Some(claim);
                        break;
                    }
                    Err(actual) => {
                        shared = actual;
                        std::thread::sleep(Duration::from_millis(250));
                    }
                }
            }
        }
        self.page.take();
        self.provider.take();
        self.vectors.take();
        self.tuple.take();
        self.files.take();
        self.selection.take();
        self.work.take();
        if let Some(claim) = self.claim.take() {
            let _ = Account::current().and_then(|a| {
                a.queue()
                    .settle(
                        &claim,
                        self.job.key(),
                        knowledge_core::jobs::Outcome::Failed,
                    )
                    .map_err(Into::into)
            });
        }
    }
}
fn metadata_only(row: &Observation) -> bool {
    row.availability != Availability::Available
        || matches!(row.verification, knowledge_storage::Verification::Failed(_))
}
fn stamp(file: &File) -> Result<[u64; 8]> {
    let m = file.metadata()?;
    Ok([
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime() as u64,
        m.mtime_nsec() as u64,
        m.ctime() as u64,
        m.ctime_nsec() as u64,
        m.nlink(),
    ])
}
fn frame_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let data = serde_json::to_vec(value)?;
    anyhow::ensure!(data.len() <= FRAME, "native metadata frame too large");
    let mut bytes = Vec::with_capacity(4 + data.len());
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    Ok(bytes)
}
fn read_frame<T: serde::de::DeserializeOwned>(file: &File, offset: u64) -> Result<(T, u64)> {
    let mut len = [0; 4];
    file.read_exact_at(&mut len, offset)?;
    let n = u32::from_le_bytes(len) as usize;
    anyhow::ensure!(n <= FRAME, "native metadata frame too large");
    let next = offset
        .checked_add(4)
        .and_then(|v| v.checked_add(n as u64))
        .context("metadata frame overflow")?;
    anyhow::ensure!(next <= file.metadata()?.len(), "incomplete metadata frame");
    let mut data = vec![0; n];
    file.read_exact_at(&mut data, offset + 4)?;
    Ok((serde_json::from_slice(&data)?, next))
}
fn text_windows(text: &str) -> impl Iterator<Item = &str> {
    let mut position = 0;
    std::iter::from_fn(move || {
        if position == text.len() {
            return None;
        }
        let mut end = (position + 48 * 1024).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let value = &text[position..end];
        position = end;
        Some(value)
    })
}

struct TargetLease {
    parent: File,
    directory: File,
    parent_path: PathBuf,
    stage_path: PathBuf,
}
impl TargetLease {
    fn check(&self) -> Result<()> {
        let p = self.parent.metadata()?;
        let n = std::fs::symlink_metadata(&self.parent_path)?;
        anyhow::ensure!(
            n.is_dir() && (p.dev(), p.ino()) == (n.dev(), n.ino()),
            "generated target parent changed"
        );
        let d = self.directory.metadata()?;
        let n = std::fs::symlink_metadata(&self.stage_path)?;
        anyhow::ensure!(
            n.is_dir() && (d.dev(), d.ino()) == (n.dev(), n.ino()),
            "generated target changed"
        );
        Ok(())
    }
}
struct DirectoryInputs {
    job: Arc<NativeRefreshJob>,
    claim: Arc<Claim>,
    page: Arc<NativeInventoryPage>,
    tape: Arc<File>,
    version: [u64; 8],
    parts: Vec<SourcePart>,
    source_errors: Arc<Mutex<Vec<anyhow::Error>>>,
    refusals: Arc<Mutex<Vec<wire::authority::ScopeRejoinError>>>,
    target: TargetLease,
}
impl DirectoryInputs {
    fn check(&self) -> Result<()> {
        self.job.cause.check()?;
        self.target.check()?;
        anyhow::ensure!(
            Instant::now() < self.claim.deadline()
                && self.claim.stop_requested(self.job.key())?.is_none(),
            "native corpus stopped"
        );
        anyhow::ensure!(
            stamp(&self.tape)? == self.version,
            "held representation changed"
        );
        Ok(())
    }
}
struct NativeInputs(Arc<DirectoryInputs>);
impl std::fmt::Debug for NativeInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeCorpusInputs")
    }
}
fn kernel_error() -> KernelError {
    KernelError {
        code: "conflict".into(),
        message: "actual native corpus owner changed".into(),
    }
}
impl ByteDirectory for NativeInputs {
    fn check(&self) -> std::result::Result<(), KernelError> {
        self.0.check().map_err(|_| kernel_error())
    }
    fn exists(&self, name: &str) -> std::result::Result<bool, KernelError> {
        self.check()?;
        Ok(self
            .0
            .parts
            .iter()
            .any(|p| matches!(&p.document.content,Content::Representation{path,..}if path==name)))
    }
    fn open(&self, name: &str) -> std::result::Result<Arc<dyn ByteHandle>, KernelError> {
        self.check()?;
        let part = self
            .0
            .parts
            .iter()
            .find(|p| matches!(&p.document.content,Content::Representation{path,..}if path==name))
            .ok_or_else(kernel_error)?;
        let observed = self
            .0
            .page
            .sources()
            .nth(part.slot)
            .ok_or_else(kernel_error)?;
        let length = usize::try_from(part.tape_length).map_err(|_| kernel_error())?;
        if length > TEXT_WINDOW {
            return Err(kernel_error());
        }
        let processing = match self
            .0
            .job
            .processing(&self.0.claim, observed, self.0.claim.budget())
        {
            Ok(processing) => processing,
            Err(cause) => {
                self.0
                    .source_errors
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(cause);
                return Err(kernel_error());
            }
        };
        let facts = processing.source_facts();
        if facts.0 != part.document.key.id
            || facts.1.as_str() != part.document.digest
            || facts.2.as_str()
                != match &part.document.content {
                    Content::Representation { processor, .. } => processor,
                    _ => return Err(kernel_error()),
                }
        {
            if let Err(scope) = self.0.job.finish_source(processing, &self.0.claim) {
                self.0
                    .refusals
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(scope);
            }
            return Err(kernel_error());
        }
        Ok(Arc::new(NativeSlice {
            owner: self.0.clone(),
            processing: Some(processing),
            start: part.tape_start,
            length,
        }))
    }
}
struct NativeSlice {
    owner: Arc<DirectoryInputs>,
    processing: Option<NativeProcessing>,
    start: u64,
    length: usize,
}
impl std::fmt::Debug for NativeSlice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeCorpusSlice")
    }
}
impl ByteLength for NativeSlice {
    fn len(&self) -> usize {
        self.length
    }
}
impl ByteHandle for NativeSlice {
    fn read_bytes(&self, range: Range<usize>) -> std::io::Result<SourceBytes> {
        let check = || -> Result<()> {
            self.owner.check()?;
            self.processing
                .as_ref()
                .context("source scope unavailable")?
                .check(&self.owner.claim)
        };
        let error = |_| std::io::Error::other("actual native corpus source changed");
        check().map_err(error)?;
        if range.start > range.end || range.end > self.length || range.len() > READ_WINDOW {
            return Err(std::io::Error::other("native range exceeds prepaid window"));
        }
        let offset = self
            .start
            .checked_add(range.start as u64)
            .ok_or_else(|| std::io::Error::other("range overflow"))?;
        let mut bytes = vec![0; range.len()];
        self.owner.tape.read_exact_at(&mut bytes, offset)?;
        check().map_err(error)?;
        Ok(SourceBytes::new(bytes))
    }
}
impl Drop for NativeSlice {
    fn drop(&mut self) {
        if let Some(processing) = self.processing.take()
            && let Err(scope) = self.owner.job.finish_source(processing, &self.owner.claim)
        {
            self.owner
                .refusals
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(scope);
        }
    }
}
