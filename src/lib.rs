//! Shared local knowledge-library lifecycle and retrieval for agents and the GPUI interface.
pub mod embedding;
/// Compatibility IO uses the mature M3 bodies under the actual process account.
pub mod epub {
    pub use knowledge_processing::epub::*;
    /// Read through the same native account; the caller retains its admitted result lifetime.
    pub fn read_epub(path: &std::path::Path, max_bytes: usize) -> anyhow::Result<String> {
        super::admitted::processing(|| {
            super::admitted::input(path, |file| {
                super::admitted::retain(knowledge_processing::epub::read_epub_file(
                    file, max_bytes, None,
                )?)
            })
        })
    }
    /// Export through the same native account without changing format or no-clobber rules.
    pub fn export_epub(
        root: &std::path::Path,
        output: &std::path::Path,
        title: &str,
        include_media: bool,
    ) -> anyhow::Result<usize> {
        super::admitted::processing(|| {
            knowledge_processing::epub::export_epub(root, output, title, include_media)
        })
    }
}
/// Mature extraction compatibility with actual shared-account admission, not its legacy fallback.
pub mod extract {
    pub use knowledge_processing::extract::*;
    /// Extract only after the one native process owner admits the actual mature parser costs.
    pub fn extract(path: &std::path::Path, max_bytes: usize) -> anyhow::Result<Extraction> {
        super::admitted::processing(|| {
            super::admitted::input(path, |file| {
                super::admitted::retain(knowledge_processing::extract::extract_file(
                    file, path, max_bytes, None,
                )?)
            })
        })
    }
    /// Render bounded source preview under the same native account and processor hooks.
    pub fn preview_html(path: &std::path::Path, max_bytes: usize) -> anyhow::Result<String> {
        preview_html_at_line(path, max_bytes, None)
    }
    /// Preserve exact source-line validation through the mature admitted preview body.
    pub fn preview_html_at_line(
        path: &std::path::Path,
        max_bytes: usize,
        line: Option<usize>,
    ) -> anyhow::Result<String> {
        super::admitted::processing(|| {
            super::admitted::input(path, |file| {
                super::admitted::retain(knowledge_processing::extract::preview_file_funded(
                    file,
                    path,
                    max_bytes,
                    line,
                    None,
                    &mut knowledge_processing::funding::Funding::BeforeVerification,
                )?)
            })
        })
    }
}
pub mod file_actions;
pub mod file_info;
#[cfg(feature = "gui")]
pub mod gui;
#[cfg(feature = "gui")]
pub(crate) mod gui_assets;
#[path = "search.rs"]
#[allow(
    dead_code,
    reason = "The retained legacy lexical builder is retired; only metadata/vector compatibility adapters remain production callers."
)]
mod legacy_search;
pub mod mcp;
/// Mature media compatibility uses actual shared resources and the actual owned-child RSS watchdog.
pub mod media {
    pub use knowledge_processing::media::*;
    /// Segment real admitted media without replacing the mature codec/coordinate behavior.
    pub fn segment_video(
        path: &std::path::Path,
        output_dir: &std::path::Path,
        seconds: f64,
    ) -> anyhow::Result<Vec<MediaSegment>> {
        super::admitted::processing(|| {
            knowledge_processing::media::segment_video_file(
                &std::fs::File::open(path)?,
                output_dir,
                seconds,
                knowledge_processing::admission::available_memory()? / 2,
                knowledge_processing::admission::available_storage()? / 2,
            )
        })
    }
    /// Read the actual bounded transcript under the process account, preserving source timestamps.
    pub fn parse_transcript(path: &std::path::Path) -> anyhow::Result<Vec<TranscriptSegment>> {
        super::admitted::processing(|| {
            super::admitted::retain(knowledge_processing::media::parse_transcript_path(path)?)
        })
    }
    /// Run explicitly selected local speech tools through actual child admission/RSS observation.
    pub fn transcribe(
        path: &std::path::Path,
        executable: &std::path::Path,
        model: &std::path::Path,
        output_dir: &std::path::Path,
    ) -> anyhow::Result<Vec<TranscriptSegment>> {
        super::admitted::processing(|| {
            super::admitted::retain(knowledge_processing::media::transcribe_file(
                &std::fs::File::open(path)?,
                executable,
                model,
                output_dir,
                knowledge_processing::admission::available_memory()? / 2,
                knowledge_processing::admission::available_storage()? / 2,
            )?)
        })
    }
}
pub mod runtime;
/// Compatibility publication hook records an actual external kernel tuple, never another lexical index.
mod search {
    pub(crate) use super::legacy_search::{search, search_unified};
    /// Keep the old publisher's path shape while removing its competing Tantivy construction.
    pub(crate) fn build_bm25(
        directory: &std::path::Path,
        _records: &[super::FileRecord],
    ) -> anyhow::Result<()> {
        super::admitted::write_reference(directory)
    }
}
pub mod status;
mod storage;

/// Actual native accounting and shared-kernel bridge for retained public entry points.
/// This module is transitional host glue, not another lexical engine or resource account.
pub use admitted::{Stopped as FundedIndexStopped, UnsettledProcessing};
mod admitted {
    use super::*;
    use knowledge_core::admission::{
        Account, Configuration, Decision, Holding, Origin, Scheduling, Waiting,
    };
    use knowledge_core::jobs::Reservation;
    use knowledge_processing::{AdmitError, funding::Funding};
    use knowledge_retrieval::{
        Content, Control, Document, KernelAdapter, Locator, Mode, Query, ScopeView, SourceKey, Work,
    };
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::{Arc, OnceLock};
    use std::time::{Duration, Instant};

    /// Host-owned persistent runtime charge, matching process-global kernel analyzers/dictionaries.
    struct Native {
        cache: PathBuf,
    }
    static NATIVE: OnceLock<Native> = OnceLock::new();
    static START: std::sync::Mutex<()> = std::sync::Mutex::new(());
    thread_local! {
        /// A decoded snapshot inherits the exact active preallocation lifetime, including cache pins.
        static ALLOCATION: RefCell<Option<Arc<Memory>>> = const { RefCell::new(None) };
        /// Exact completed kernel tuple prepared by this native index operation, not a permission store.
        static PUBLISHED: RefCell<Option<serde_json::Value>> = const { RefCell::new(None) };
        /// One actual native operation's deadline/live revocation check, shared with kernel and M3.
        static CONTROL: RefCell<Option<Control>> = const { RefCell::new(None) };
        /// Canonical/representation read work ceiling, distinct from Core's memory/storage promises.
        static READ_BYTES: Cell<Option<u64>> = const { Cell::new(None) };
        static RESULT: RefCell<Option<(omni_service::authority::Permit, u64)>> = const { RefCell::new(None) };
        static SELECTED_ROOT: RefCell<Option<File>> = const { RefCell::new(None) };
    }
    /// Memory capability attached to actual snapshot/cache/reader lifetimes, never a force-release ID.
    pub(super) struct Memory {
        holding: std::sync::Mutex<Holding>,
        // Envelope inside that ONE prepaid holding, never another independently spendable pool.
        copy_remaining: std::sync::Mutex<u64>,
    }
    impl std::fmt::Debug for Memory {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("NativeMemory")
        }
    }
    impl Drop for Memory {
        /// The last actual snapshot/reader owner releases its charge, not the executing job's slot.
        fn drop(&mut self) {
            if let Ok(holding) = self.holding.get_mut() {
                let _ = holding.finish();
            }
        }
    }
    /// Retain current allocation with a decoded snapshot, including an idle resident cache entry.
    pub(super) fn allocation() -> Option<Arc<Memory>> {
        ALLOCATION.with(|a| a.borrow().clone())
    }
    /// Restore a nested host binding on all normal/error/panic exits.
    struct Binding(Option<Arc<Memory>>, bool);
    impl Drop for Binding {
        fn drop(&mut self) {
            if self.1 {
                PUBLISHED.with(|published| *published.borrow_mut() = None);
            }
            ALLOCATION.with(|a| *a.borrow_mut() = self.0.take());
        }
    }
    /// Start/reuse the ONE process account and a private external derived-cache directory.
    fn native() -> Result<&'static Native> {
        if let Some(native) = NATIVE.get() {
            return Ok(native);
        }
        let _start = START
            .lock()
            .map_err(|_| anyhow::anyhow!("native startup unavailable"))?;
        if let Some(native) = NATIVE.get() {
            return Ok(native);
        }
        let uid = rustix::process::geteuid().as_raw();
        let cache = std::env::var_os("OMNI_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("omni-knowledge-derived-{uid}")));
        match fs::create_dir(&cache) {
            Ok(()) => fs::set_permissions(&cache, fs::Permissions::from_mode(0o700))?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let metadata = fs::symlink_metadata(&cache)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.mode() & 0o7777 != 0o700
        {
            bail!("derived cache is not an owned private native directory");
        }
        let cache = cache.canonicalize()?;
        let account = match Account::current() {
            Ok(account) => account,
            Err(knowledge_core::admission::Error::NotStarted) => Account::start(
                File::open(&cache)?,
                Configuration {
                    memory_bytes: None,
                    storage_bytes: None,
                },
                Scheduling {
                    max_pending: 1024,
                    max_records: 8192,
                    max_descriptor_bytes: 8 * 1024 * 1024,
                    per_mount: 2,
                },
            )?,
            Err(error) => return Err(error.into()),
        };
        if fs::metadata(&cache)?.dev() != account.policy().startup_volume.device {
            bail!("derived cache must use the process account's selected volume");
        }
        // KernelAdapter::open starts its own once-only runtime charge when retrieval actually runs.
        // Extraction/bootstrap does not need a resident tokenizer reservation.
        let _ = NATIVE.set(Native { cache });
        NATIVE.get().context("native runtime did not start")
    }
    /// Typed visible contention, distinct from a single need exceeding the whole budget.
    #[derive(Debug)]
    pub(super) struct WaitingFor(pub Waiting);
    impl std::fmt::Display for WaitingFor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self.0 {
                Waiting::Memory => "waiting for memory",
                Waiting::Storage => "waiting for generated storage",
            })
        }
    }
    impl std::error::Error for WaitingFor {}
    /// Preserve actual waiting versus impossible-whole-budget errors; never create another pool.
    fn ready<T>(decision: Decision<T>) -> Result<T> {
        match decision {
            Decision::Ready(value) => Ok(value),
            Decision::Waiting(reason) => Err(WaitingFor(reason).into()),
        }
    }
    /// Establish native startup without creating a second process account.
    pub(super) fn start() -> Result<()> {
        native().map(|_| ())
    }
    /// Effective policy from the sole process owner; no global hidden-job count is fabricated.
    pub(super) fn budgets() -> Result<omni_service::ResourceBudgets> {
        native()?;
        let policy = Account::current()?.policy();
        Ok(omni_service::ResourceBudgets {
            memory_budget_bytes: omni_service::U64::new(policy.memory_bytes),
            storage_budget_bytes: omni_service::U64::new(policy.storage_bytes),
            memory_override_bytes: policy
                .configuration
                .memory_bytes
                .map(omni_service::U64::new),
            storage_override_bytes: policy
                .configuration
                .storage_bytes
                .map(omni_service::U64::new),
            waiting_for_memory: None,
        })
    }
    fn holding(funding: &mut Funding<'_>, need: Reservation) -> Result<Holding> {
        Ok(match funding {
            Funding::BeforeVerification => {
                ready(Account::current()?.reserve(need, Origin::BeforeVerification)?)?
            }
            Funding::Claim { claim, expected } => {
                Account::current()?.split_claim(claim, *expected, need)?
            }
        })
    }
    // Preflight only: all walk/decode scratch must die before returning this synchronous loan.
    // Never resize/rejoin a holding attached to live or cached values.
    fn resize_preflight(
        held: &mut Holding,
        extra: Reservation,
        funding: &mut Funding<'_>,
    ) -> Result<()> {
        match funding {
            Funding::BeforeVerification => ready(held.grow(extra)?),
            Funding::Claim { claim, expected } => {
                let old = held.remaining()?;
                let total = Reservation {
                    memory_bytes: old
                        .memory_bytes
                        .checked_add(extra.memory_bytes)
                        .context("funded memory overflow")?,
                    temporary_bytes: old
                        .temporary_bytes
                        .checked_add(extra.temporary_bytes)
                        .context("funded storage overflow")?,
                };
                let credit = Account::current()?
                    .job_status(claim.job(), *expected)?
                    .execution;
                if credit.memory_bytes < extra.memory_bytes
                    || credit.temporary_bytes < extra.temporary_bytes
                {
                    return Err(AdmitError::InsufficientCredit.into());
                }
                // Preflight scratch has already died. This is never a return/resize of live output
                // buffers: those use the separate prepaid envelope and keep ONE original holding.
                Account::current()?.rejoin_claim(held, claim, *expected)?;
                *held = Account::current()?.split_claim(claim, *expected, total)?;
                Ok(())
            }
        }
    }
    /// Charge before any snapshot decoding or retained inventory/record construction.
    pub(super) fn memory<T>(
        kb: &KnowledgeBase,
        build: bool,
        body: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        memory_funded(kb, build, &mut Funding::BeforeVerification, |_| body())
    }
    #[derive(Debug)]
    pub struct Stopped(pub knowledge_core::jobs::Cancellation);
    impl std::fmt::Display for Stopped {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "funded execution stopped: {:?}", self.0)
        }
    }
    impl std::error::Error for Stopped {}
    pub(super) fn funded_checkpoint(funding: &Funding<'_>) -> Result<()> {
        checkpoint()?;
        if let Funding::Claim { claim, expected } = funding
            && let Some(stop) = claim.stop_requested(*expected)?
        {
            return Err(Stopped(stop).into());
        }
        Ok(())
    }
    pub(super) fn memory_funded<T>(
        kb: &KnowledgeBase,
        build: bool,
        funding: &mut Funding<'_>,
        body: impl FnOnce(&mut Funding<'_>) -> Result<T>,
    ) -> Result<T> {
        checkpoint()?;
        funded_checkpoint(funding)?;
        let native = native()?;
        if native.cache.starts_with(&kb.root) {
            bail!("derived cache must remain outside the authored library");
        }
        let mut memory = Memory {
            holding: std::sync::Mutex::new(holding(
                funding,
                Reservation {
                    memory_bytes: 1024 * 1024,
                    temporary_bytes: 0,
                },
            )?),
            copy_remaining: std::sync::Mutex::new(0),
        };
        let mut copies = 0u64;
        let mut bytes = 0_u64;
        if let Ok(file) = File::open(kb.root.join(".omni/current.json")) {
            if file.metadata()?.len() > 64 * 1024 {
                bail!("invalid current generation pointer");
            }
            let current: String = serde_json::from_reader(file)?;
            storage::validate_relative(Path::new(&current))?;
            let manifest = kb
                .root
                .join(".omni/indexes")
                .join(current)
                .join("manifest.json");
            storage::check_components(&kb.root, manifest.strip_prefix(&kb.root)?, !build)?;
            match fs::metadata(manifest) {
                Ok(metadata) => {
                    bytes = metadata
                        .len()
                        .checked_mul(24)
                        .context("snapshot memory accounting overflow")?
                }
                Err(error) if build && error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        if build {
            // The first bounded walk estimates before creating the path vector or parser output.
            let mut walker = WalkDir::new(&kb.root).follow_links(false).into_iter();
            while let Some(entry) = walker.next() {
                let entry = entry?;
                if entry.depth() > 0
                    && entry.file_type().is_dir()
                    && (matches!(entry.file_name().to_str(), Some(".omni" | ".git"))
                        || storage::has_marker(entry.path())?)
                {
                    walker.skip_current_dir();
                    continue;
                }
                if entry.file_type().is_file() {
                    if entry.file_name().to_str() == Some(CONFIG_FILE) {
                        continue;
                    }
                    funded_checkpoint(funding)?;
                    let actual_length = entry.metadata()?.len();
                    let length = actual_length.min(MAX_TEXT_BYTES as u64);
                    if matches!(funding, Funding::Claim { .. }) {
                        let relative = entry
                            .path()
                            .strip_prefix(&kb.root)?
                            .to_str()
                            .context("source path is not UTF-8")?;
                        let mut magic = [0u8; 5];
                        use std::os::unix::fs::FileExt;
                        let file = source_file(kb, relative)?;
                        let _ = file.read_at(&mut magic, 0)?;
                        let opaque = actual_length <= MAX_TEXT_BYTES as u64
                            && (&magic == b"%PDF-"
                                || matches!(
                                    entry
                                        .path()
                                        .extension()
                                        .and_then(|e| e.to_str())
                                        .unwrap_or("")
                                        .to_ascii_lowercase()
                                        .as_str(),
                                    "pdf"
                                        | "html"
                                        | "htm"
                                        | "epub"
                                        | "docx"
                                        | "docm"
                                        | "xlsx"
                                        | "xlsm"
                                        | "pptx"
                                        | "pptm"
                                        | "odt"
                                        | "ods"
                                        | "odp"
                                        | "doc"
                                        | "rtf"
                                ));
                        let copy = if opaque {
                            (MAX_TEXT_BYTES as u64 * 4 + 65_536) * 16
                        } else {
                            length
                                .checked_mul(16)
                                .and_then(|n| n.checked_add(128 * 1024))
                                .context("copy plan overflow")?
                        };
                        copies = copies.checked_add(copy).context("copy plan overflow")?;
                        let metadata = kb.metadata_path(relative)?;
                        if let Ok(meta) = fs::symlink_metadata(metadata) {
                            if !meta.is_file() || meta.file_type().is_symlink() {
                                bail!("invalid authored metadata source");
                            }
                            let extra = meta
                                .len()
                                .checked_mul(32)
                                .context("metadata plan overflow")?;
                            bytes = bytes.checked_add(extra).context("metadata plan overflow")?;
                            copies = copies
                                .checked_add(extra)
                                .context("metadata copy plan overflow")?;
                        }
                    }
                    let cost = length
                        .checked_mul(24)
                        .and_then(|v| v.checked_add(32 * 1024))
                        .context("inventory memory accounting overflow")?;
                    bytes = bytes
                        .checked_add(cost)
                        .context("inventory memory accounting overflow")?;
                }
            }
        }
        resize_preflight(
            memory
                .holding
                .get_mut()
                .map_err(|_| anyhow::anyhow!("snapshot memory unavailable"))?,
            Reservation {
                memory_bytes: bytes
                    .checked_add(copies)
                    .context("retained plan overflow")?,
                temporary_bytes: 0,
            },
            funding,
        )?;
        *memory
            .copy_remaining
            .get_mut()
            .map_err(|_| anyhow::anyhow!("copy plan unavailable"))? = copies;
        let memory = Arc::new(memory);
        let previous = ALLOCATION.with(|a| a.replace(Some(memory)));
        let _binding = Binding(previous, build);
        if build {
            PUBLISHED.with(|published| *published.borrow_mut() = None);
        }
        body(funding)
    }
    /// Parse one native sidecar only after its declared memory requirement is admitted.
    /// Successful retained metadata joins the actual snapshot lifetime; errors settle immediately.
    pub(super) fn metadata_funded(file: File, funding: &mut Funding<'_>) -> Result<AssetMetadata> {
        native()?;
        let bytes = file
            .metadata()?
            .len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(4096))
            .context("metadata accounting overflow")?;
        let mut holding = holding(
            funding,
            Reservation {
                memory_bytes: bytes,
                temporary_bytes: 0,
            },
        )?;
        match serde_json::from_reader::<_, AssetMetadata>(file) {
            Ok(value) => retain_funded(
                knowledge_processing::admission::Admitted {
                    value,
                    retained: vec![holding],
                },
                funding,
            ),
            Err(error) => {
                holding.finish()?;
                Err(error.into())
            }
        }
    }
    /// Restore only this synchronous invocation's borrowed native stop context.
    struct ControlBinding(Option<Control>, Option<u64>);
    impl Drop for ControlBinding {
        fn drop(&mut self) {
            CONTROL.with(|control| *control.borrow_mut() = self.0.take());
            READ_BYTES.with(|bytes| bytes.set(self.1));
        }
    }
    /// Run real native work under one live guard/deadline; no background monitor or new account exists.
    pub(super) fn controlled<T>(control: Control, max_bytes: u64, body: impl FnOnce() -> T) -> T {
        let _binding = ControlBinding(
            CONTROL.with(|current| current.replace(Some(control))),
            READ_BYTES.with(|bytes| bytes.replace(Some(max_bytes))),
        );
        body()
    }
    struct ResultBinding(Option<(omni_service::authority::Permit, u64)>, Option<File>);
    impl Drop for ResultBinding {
        fn drop(&mut self) {
            RESULT.with(|r| *r.borrow_mut() = self.0.take());
            SELECTED_ROOT.with(|r| *r.borrow_mut() = self.1.take());
        }
    }
    pub(super) fn result_scope<T>(
        permit: omni_service::authority::Permit,
        root: File,
        bytes: u64,
        body: impl FnOnce() -> T,
    ) -> T {
        let _binding = ResultBinding(
            RESULT.with(|r| r.replace(Some((permit, bytes)))),
            SELECTED_ROOT.with(|r| r.replace(Some(root))),
        );
        body()
    }
    /// Grow the aggregate BEFORE buffer growth; native results retain this exact holding through Delivery.
    pub(super) fn charge_result(bytes: u64) -> Result<()> {
        RESULT.with(|r| {
            if let Some((permit, remaining)) = r.borrow_mut().as_mut() {
                let next = remaining
                    .checked_sub(bytes)
                    .context("aggregate result exceeds request memory budget")?;
                permit
                    .reserve_result(bytes)
                    .map_err(|_| anyhow::anyhow!("aggregate result admission refused"))?;
                *remaining = next;
            } else if let Some(memory) = allocation() {
                ready(
                    memory
                        .holding
                        .lock()
                        .map_err(|_| anyhow::anyhow!("result memory unavailable"))?
                        .grow(Reservation {
                            memory_bytes: bytes,
                            temporary_bytes: 0,
                        })?,
                )?;
            }
            Ok(())
        })
    }
    pub(super) fn charge_hit(
        kb: &KnowledgeBase,
        record: &FileRecord,
        snippet: usize,
    ) -> Result<()> {
        let mut bytes = kb.root.as_os_str().as_encoded_bytes().len()
            + record.path.len()
            + record.title.len()
            + snippet
            + std::mem::size_of::<SearchHit>()
            + 1024;
        for tag in &record.tags {
            bytes = bytes
                .checked_add(tag.len() + std::mem::size_of::<String>())
                .context("result size overflow")?;
        }
        charge_result(
            (bytes as u64)
                .checked_mul(4)
                .context("result size overflow")?,
        )
    }
    /// Admit actual declared reads before opening their data stream, never as a memory-pool substitute.
    pub(super) fn charge_bytes(count: u64) -> Result<()> {
        READ_BYTES.with(|bytes| {
            if let Some(remaining) = bytes.get() {
                let next = remaining
                    .checked_sub(count)
                    .ok_or(knowledge_core::backend::BackendError::BudgetExhausted)?;
                bytes.set(Some(next));
            }
            Ok(())
        })
    }
    /// Check the native operation if one is bound; standalone CLI work keeps the normal local limits.
    pub(super) fn checkpoint() -> Result<()> {
        CONTROL.with(|control| {
            control
                .borrow()
                .as_ref()
                .map_or(Ok(()), |control| control.check().map_err(Into::into))
        })
    }
    /// Kernel invocations share the actual request stop state, instead of extending its deadline per shard.
    fn control() -> Control {
        CONTROL
            .with(|control| control.borrow().clone())
            .unwrap_or_else(|| Control::new(Instant::now() + Duration::from_secs(60)))
    }
    /// Bootstrap/reuse the actual Core owner; M3's file APIs now perform their own exact admission.
    pub(super) fn processing<T>(body: impl FnOnce() -> Result<T>) -> Result<T> {
        native()?;
        checkpoint()?;
        let result = body();
        checkpoint()?;
        result
    }
    /// Compatibility descriptor owner for read-only processors; unresolved work stays foreground.
    /// Keep the exact input FD and returned child capabilities until real group/I/O collection.
    pub(super) fn input<T>(path: &Path, body: impl FnOnce(&File) -> Result<T>) -> Result<T> {
        let file = File::open(path)?;
        let mut result = body(&file);
        if let Some(child) = result
            .as_mut()
            .err()
            .and_then(|error| error.downcast_mut::<knowledge_processing::worker::UnsettledChild>())
        {
            let mut held = loop {
                match child.collect() {
                    Ok(held) => break held,
                    Err(_) => std::thread::sleep(Duration::from_millis(250)),
                }
            };
            while let Some(holding) = held.last_mut() {
                // These read-only adapters generate no files. Do not guess a storage delta.
                if holding.remaining()?.temporary_bytes != 0 || holding.finish().is_err() {
                    std::thread::sleep(Duration::from_millis(250));
                } else {
                    held.pop();
                }
            }
        }
        drop(file);
        result
    }
    /// Actual deep-copy handoff: admit the NEW buffers, copy, DROP the old buffers, then finish
    /// their M3 holdings. This is not an accounting merge, alias, subtraction, or early release.
    /// Only deep-cloning native strings/records call this private helper. One retained snapshot
    /// holding, not one holding per corpus document, owns the copied bytes. Legacy raw callers
    /// without a parent keep the original M3 charge conservatively until process exit.
    pub(super) trait CopyBytes: Clone {
        fn heap_bytes(&self) -> usize;
        fn copy_bytes(&self) -> Result<u64> {
            (self.heap_bytes() as u64)
                .checked_add(std::mem::size_of_val(self) as u64)
                .and_then(|n| n.checked_mul(2))
                .and_then(|n| n.checked_add(1024))
                .context("copy accounting overflow")
        }
    }
    impl CopyBytes for String {
        fn heap_bytes(&self) -> usize {
            self.len()
        }
    }
    impl CopyBytes for AssetMetadata {
        fn heap_bytes(&self) -> usize {
            self.path.len()
                + self.description.len()
                + self
                    .tags
                    .iter()
                    .map(|s| s.len() + std::mem::size_of::<String>())
                    .sum::<usize>()
        }
    }
    impl CopyBytes for knowledge_processing::extract::Extraction {
        fn heap_bytes(&self) -> usize {
            self.text.len()
                + self.format.len()
                + self.status.len()
                + self
                    .warnings
                    .iter()
                    .map(|s| s.len() + std::mem::size_of::<String>())
                    .sum::<usize>()
        }
    }
    impl CopyBytes for Vec<knowledge_processing::media::TranscriptSegment> {
        fn heap_bytes(&self) -> usize {
            self.iter()
                .map(|s| s.text.len() + std::mem::size_of_val(s))
                .sum()
        }
    }
    pub(super) fn retain<T: CopyBytes>(
        value: knowledge_processing::admission::Admitted<T>,
    ) -> Result<T> {
        retain_funded(value, &mut Funding::BeforeVerification)
    }
    pub(super) fn retain_funded<T: CopyBytes>(
        value: knowledge_processing::admission::Admitted<T>,
        funding: &mut Funding<'_>,
    ) -> Result<T> {
        let knowledge_processing::admission::Admitted {
            value,
            mut retained,
        } = value;
        let memory = allocation();
        let mut raw_copy = None;
        let copied = (|| -> Result<T> {
            let bytes = value.copy_bytes()?;
            let Some(memory) = memory else {
                // Legacy T has no last-reader callback. Prepay its actual deep copy, rather than
                // stranding M3's much larger opaque-output envelope for every tiny returned value.
                // Native callers use NativeExtraction and keep the original paid value instead.
                raw_copy = Some(holding(
                    funding,
                    Reservation {
                        memory_bytes: bytes,
                        temporary_bytes: 0,
                    },
                )?);
                return Ok(value.clone());
            };
            match funding {
                Funding::BeforeVerification => ready(
                    memory
                        .holding
                        .lock()
                        .map_err(|_| anyhow::anyhow!("parent memory unavailable"))?
                        .grow(Reservation {
                            memory_bytes: bytes,
                            temporary_bytes: 0,
                        })?,
                )?,
                Funding::Claim { .. } => {
                    funded_checkpoint(funding)?;
                    let mut remaining = memory
                        .copy_remaining
                        .lock()
                        .map_err(|_| anyhow::anyhow!("copy plan unavailable"))?;
                    *remaining = remaining
                        .checked_sub(bytes)
                        .ok_or(AdmitError::InsufficientCredit)?;
                }
            }
            Ok(value.clone())
        })();
        drop(value);
        let mut settlement = Ok(());
        for holding in &mut retained {
            let result = match funding {
                Funding::BeforeVerification => holding.finish(),
                Funding::Claim { claim, expected } => {
                    Account::current()?.rejoin_claim(holding, claim, *expected)
                }
            };
            if let Err(error) = result {
                settlement = Err(anyhow::Error::from(error));
            }
        }
        let copy = copied?;
        settlement?;
        // A successful raw T deliberately retains only its copied-buffer credit until process exit.
        // It is not a native lifetime API and cannot pretend to know when an arbitrary T is freed.
        drop(raw_copy);
        Ok(copy)
    }
    /// Borrow the actual qualified private cache for a caller that already split its own Claim credit.
    pub(super) fn cache_directory() -> Result<&'static Path> {
        Ok(&native()?.cache)
    }
    /// Own scratch outputs on the SAME native selected cache volume as its storage promises.
    pub(super) fn tempdir() -> Result<tempfile::TempDir> {
        let _setup = ready(Work::reserve(
            Reservation {
                memory_bytes: 4096,
                temporary_bytes: 64 * 1024,
            },
            Origin::BeforeVerification,
        )?)?;
        Ok(tempfile::Builder::new()
            .prefix("processor-")
            .tempdir_in(&native()?.cache)?)
    }
    /// An owned immutable parser input plus its actual original digest/stat facts.
    pub(super) struct SourceStage {
        // Delete owned staged bytes before ending their generated-storage promise.
        _directory: tempfile::TempDir,
        pub(super) path: PathBuf,
        pub(super) digest: String,
        pub(super) metadata: fs::Metadata,
        pub(super) file: File,
        _original: File,
        _resources: Work,
    }
    /// Descriptor-relative source open refuses component symlinks instead of trusting a checked pathname.
    fn source_file(kb: &KnowledgeBase, relative: &str) -> Result<File> {
        checkpoint()?;
        storage::validate_relative(Path::new(relative))?;
        if relative
            .split('/')
            .any(|p| matches!(p, ".omni" | ".git" | CONFIG_FILE))
        {
            bail!("source is outside admitted original namespace");
        }
        let mut file = match SELECTED_ROOT
            .with(|r| r.borrow().as_ref().map(File::try_clone))
            .transpose()?
        {
            Some(file) => file,
            None => {
                kb.resolve(relative)?;
                File::open(&kb.root)?
            }
        };
        let mut parts = relative.split('/').peekable();
        while let Some(part) = parts.next() {
            let mut flags = rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK;
            if parts.peek().is_some() {
                flags |= rustix::fs::OFlags::DIRECTORY;
            }
            file = File::from(rustix::fs::openat(
                &file,
                part,
                flags,
                rustix::fs::Mode::empty(),
            )?);
            if parts.peek().is_some()
                && rustix::fs::statat(&file, CONFIG_FILE, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .is_ok()
            {
                bail!("source belongs to a nested selected-root boundary");
            }
        }
        if !file.metadata()?.is_file() {
            bail!("selected source is not a regular file");
        }
        Ok(file)
    }
    /// Relevant native change evidence, never an inode-only stable asset identity.
    fn stat(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    }
    /// Hash exact admitted descriptor bytes; no checked-path-to-open symlink race is used.
    pub(super) fn hash_source(kb: &KnowledgeBase, relative: &str) -> Result<String> {
        let mut input = source_file(kb, relative)?;
        let before = input.metadata()?;
        charge_bytes(before.len())?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 65536];
        loop {
            checkpoint()?;
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if stat(&before) != stat(&input.metadata()?)
            || stat(&before) != stat(&source_file(kb, relative)?.metadata()?)
        {
            bail!("source changed during verification");
        }
        Ok(hex::encode(hash.finalize()))
    }
    /// Stage stable admitted bytes for mature path-only processors, preserving the original extension.
    /// Oversized text/parser input copies only its existing format ceiling plus one refusal byte;
    /// explicitly selected media work copies its full source under the same native storage account.
    pub(super) fn stage(
        kb: &KnowledgeBase,
        relative: &str,
        copy_limit: Option<u64>,
    ) -> Result<SourceStage> {
        let native = native()?;
        let mut input = source_file(kb, relative)?;
        let metadata = input.metadata()?;
        charge_bytes(metadata.len())?;
        let copied = copy_limit.map_or(metadata.len(), |limit| metadata.len().min(limit));
        let mut work = ready(Work::reserve(
            Reservation {
                memory_bytes: 64 * 1024,
                temporary_bytes: copied
                    .checked_add(64 * 1024)
                    .context("staging accounting overflow")?,
            },
            Origin::BeforeVerification,
        )?)?;
        let directory = tempfile::Builder::new()
            .prefix("source-stage-")
            .tempdir_in(&native.cache)?;
        let path = directory.path().join(
            Path::new(relative)
                .file_name()
                .context("source has no filename")?,
        );
        let mut output = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 65536];
        let mut position = 0_u64;
        loop {
            checkpoint()?;
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            let retained = (copied.saturating_sub(position)).min(count as u64) as usize;
            output.write_all(&buffer[..retained])?;
            position = position
                .checked_add(count as u64)
                .context("source size overflow")?;
        }
        output.sync_all()?;
        std::io::Seek::rewind(&mut output)?;
        if position != metadata.len()
            || stat(&metadata) != stat(&input.metadata()?)
            || stat(&metadata) != stat(&source_file(kb, relative)?.metadata()?)
        {
            bail!("source changed while staging; prior complete generation preserved");
        }
        work.written(copied)?;
        Ok(SourceStage {
            _directory: directory,
            path,
            digest: hex::encode(hash.finalize()),
            metadata,
            file: output,
            _original: input,
            _resources: work,
        })
    }
    struct PendingProcessing {
        cause: anyhow::Error,
        _stage: Option<SourceStage>,
        _scratch: Option<tempfile::TempDir>,
        _input: Option<File>,
        _memory: Option<Arc<Memory>>,
        collected: bool,
        retained: Vec<Holding>,
    }
    impl PendingProcessing {
        /// Make one bounded native collection/reconciliation attempt while all original leases remain live.
        fn collect(&mut self) -> Result<()> {
            if !self.collected {
                if let Some(child) = self
                    .cause
                    .downcast_mut::<knowledge_processing::worker::UnsettledChild>()
                {
                    self.retained.extend(child.collect()?);
                } else if let Some(write) = self
                    .cause
                    .downcast_mut::<knowledge_processing::funding::UnsettledWrite>()
                {
                    write.retry()?;
                } else {
                    bail!("native collection capability unavailable");
                }
                self.collected = true;
            }
            while let Some(holding) = self.retained.last_mut() {
                if holding.remaining()?.temporary_bytes != 0 {
                    // Artifact paths can include already-reconciled files (for example ASR's WAV).
                    // M3 has not published a per-inode unknown-write receipt. Never debit them twice
                    // or invent a zero delta; keep this actual owner/lease/credit pending instead.
                    bail!("native generated-write delta remains unresolved");
                }
                holding.finish()?;
                self.retained.pop();
            }
            Ok(())
        }
    }
    impl Drop for PendingProcessing {
        /// Controlled error/unwind shutdown retains the real owner, not a leaked pointer lost at process exit.
        fn drop(&mut self) {
            while self.collect().is_err() {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
    /// Actual unresolved M3 work, with source descriptors, owned scratch namespace and charges retained.
    /// Retain this error while collecting its native M3 error and reconciling actual output extents.
    /// Drop NEVER destroys those leases or releases an unknown worker/write lifetime.
    pub struct UnsettledProcessing {
        pending: Box<PendingProcessing>,
    }
    impl std::fmt::Debug for UnsettledProcessing {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("UnsettledNativeProcessing")
        }
    }
    impl std::fmt::Display for UnsettledProcessing {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("native processing collection/reconciliation is unresolved; descriptors, cache leases and costs retained")
        }
    }
    impl std::error::Error for UnsettledProcessing {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self.pending.cause.as_ref())
        }
    }
    impl UnsettledProcessing {
        /// Inspect the actual typed UnsettledChild/UnsettledWrite; use `collect` to advance this owner.
        /// Completing only the executing job never substitutes for those resource lifetimes.
        pub fn native_error_mut(&mut self) -> &mut anyhow::Error {
            &mut self.pending.cause
        }
        /// Retry actual bounded native collection without dropping the original descriptors/cache leases.
        /// Unknown generated-write deltas stay pending until M3 exposes their exact reconciliation capability.
        pub fn collect(&mut self) -> Result<()> {
            self.pending.collect()
        }
    }
    fn unsettled(error: &anyhow::Error) -> bool {
        error.chain().any(|e| {
            e.is::<knowledge_processing::worker::UnsettledChild>()
                || e.is::<knowledge_processing::funding::UnsettledWrite>()
        })
    }
    pub(super) fn processing_error(
        error: anyhow::Error,
        stage: SourceStage,
        scratch: Option<tempfile::TempDir>,
    ) -> anyhow::Error {
        if unsettled(&error) {
            UnsettledProcessing {
                pending: Box::new(PendingProcessing {
                    cause: error,
                    _stage: Some(stage),
                    _scratch: scratch,
                    _input: None,
                    _memory: allocation(),
                    collected: false,
                    retained: Vec::new(),
                }),
            }
            .into()
        } else {
            error
        }
    }
    pub(super) fn scratch_error(
        error: anyhow::Error,
        scratch: tempfile::TempDir,
        input: File,
    ) -> anyhow::Error {
        if unsettled(&error) {
            UnsettledProcessing {
                pending: Box::new(PendingProcessing {
                    cause: error,
                    _stage: None,
                    _scratch: Some(scratch),
                    _input: Some(input),
                    _memory: allocation(),
                    collected: false,
                    retained: Vec::new(),
                }),
            }
            .into()
        } else {
            error
        }
    }
    /// Admission and owned-worker failures stop publication; they are not extraction coverage gaps.
    pub(super) fn resource_failure(error: &anyhow::Error) -> bool {
        error.chain().any(|cause| {
            cause.is::<WaitingFor>()
                || cause.is::<AdmitError>()
                || cause.is::<UnsettledProcessing>()
                || cause.is::<knowledge_processing::worker::UnsettledChild>()
                || cause.is::<knowledge_processing::funding::UnsettledWrite>()
                || cause.is::<knowledge_processing::WorkerFailure>()
                || cause.is::<knowledge_core::admission::Error>()
                || cause.is::<knowledge_retrieval::Error>()
        })
    }
    /// The real consuming identity, not the retained upstream lock alone.
    fn build_identity() -> String {
        let mut hash = Sha256::new();
        hash.update(include_bytes!("../Cargo.lock"));
        hash.update(include_bytes!("../vendor/retrieval-kernel/PROVENANCE.json"));
        hash.update(include_bytes!("lib.rs"));
        hash.update(include_bytes!("embedding.rs"));
        hash.update(processor_identity().as_bytes());
        hash.update(std::env::consts::OS.as_bytes());
        hash.update(std::env::consts::ARCH.as_bytes());
        hash.update(std::mem::size_of::<usize>().to_le_bytes());
        hash.update([
            cfg!(target_env = "musl") as u8,
            cfg!(target_env = "gnu") as u8,
            cfg!(target_env = "msvc") as u8,
            cfg!(target_endian = "big") as u8,
            cfg!(debug_assertions) as u8,
            cfg!(target_feature = "sse2") as u8,
            cfg!(target_feature = "avx") as u8,
            cfg!(target_feature = "avx2") as u8,
            cfg!(target_feature = "fma") as u8,
            cfg!(target_feature = "neon") as u8,
        ]);
        hash.update(if cfg!(feature = "gui") {
            b"gui".as_slice()
        } else {
            b"headless".as_slice()
        });
        hex::encode(hash.finalize())
    }
    /// Actual output-changing mature processor source identity, not a source-content digest.
    pub(super) fn processor_identity() -> String {
        let mut hash = Sha256::new();
        for source in [
            include_bytes!("../crates/knowledge-processing/src/embedding.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/embedding/funded.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/extract.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/media.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/epub.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/runtime.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/admission.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/funding.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/worker.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/opaque.rs").as_slice(),
            include_bytes!("../crates/knowledge-processing/src/bin/parser-worker.rs").as_slice(),
        ] {
            hash.update(source);
        }
        hash.update(MAX_TEXT_BYTES.to_le_bytes());
        hex::encode(hash.finalize())
    }
    /// Stream snapshot identity without allocating a whole second JSON frame.
    struct HashWriter(Sha256);
    impl Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    /// Bind one complete legacy processing snapshot to one external real-kernel corpus.
    pub(super) fn corpus(snapshot: &Snapshot) -> Result<PathBuf> {
        let mut writer = HashWriter(Sha256::new());
        // This kernel corpus has a MUTABLE writer/current pointer: equal content must not make
        // independently initialized libraries (or separately rebuilt folder copies) share it.
        // Original build-root data stays unchanged when a complete snapshot is moved; this does
        // not turn a path/marker into authority. Fresh native root/policy/source guards still own IO.
        // The outer tuple binds vectors/model; vector-only work reuses its unchanged lexical member.
        writer.0.update(b"omni-native-corpus-v2\0");
        // library_id is intentionally NOT a component: supported legacy upgrades can alter
        // it while the previous complete generation MUST remain readable after a failed build.
        serde_json::to_writer(&mut writer, &snapshot.root)?;
        serde_json::to_writer(&mut writer, &snapshot.records)?;
        writer.0.update(build_identity().as_bytes());
        Ok(native()?.cache.join(hex::encode(writer.0.finalize())))
    }
    /// Explicitly label projection identity; its digest is representation bytes, not original bytes.
    fn documents(snapshot: &Snapshot, directory: &Path, create: bool) -> Result<Vec<ScopeView>> {
        let mut shards = Vec::new();
        let mut current = ScopeView {
            scope: "shard-0".into(),
            documents: Vec::new(),
            unavailable_objects: 0,
        };
        let mut encoded = 0_usize;
        for (record_number, record) in snapshot.records.iter().enumerate() {
            let mut start = 0;
            loop {
                let mut end = (start + 2 * 1024 * 1024).min(record.text.len());
                while !record.text.is_char_boundary(end) {
                    end -= 1;
                }
                let text = &record.text[start..end];
                let projection_digest = hex::encode(Sha256::digest(text.as_bytes()));
                let file = format!("{record_number}-{start}-{projection_digest}.txt");
                if create && !text.is_empty() {
                    let mut output_work = ready(Work::reserve(
                        Reservation {
                            memory_bytes: 64 * 1024,
                            temporary_bytes: text.len() as u64,
                        },
                        Origin::BeforeVerification,
                    )?)?;
                    // Never expose a partial final projection or turn a competing identical
                    // publisher into EEXIST. The actual kernel still verifies canonical bytes
                    // from its opened descriptor; a digest-shaped name is not an authority grant.
                    let mut output = tempfile::NamedTempFile::new_in(directory)?;
                    output.write_all(text.as_bytes())?;
                    output.as_file().sync_all()?;
                    match output.persist_noclobber(directory.join(&file)) {
                        Ok(file) => drop(file),
                        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                            drop(error);
                        }
                        Err(error) => return Err(error.error.into()),
                    }
                    output_work.written(text.len() as u64)?;
                    drop(output_work);
                }
                let mut metadata = BTreeMap::new();
                metadata.insert("source_path".into(), record.path.clone());
                metadata.insert("original_digest".into(), record.sha256.clone());
                metadata.insert("description".into(), record.description.clone());
                metadata.insert("tags".into(), record.tags.join("\n"));
                let document = Document {
                    key: SourceKey {
                        namespace: "omni-representation-v1".into(),
                        id: format!("{record_number}:{start}"),
                    },
                    revision: format!(
                        "{}:{}:{}",
                        record.sha256, record.dependency_hash, projection_digest
                    ),
                    digest: if text.is_empty() {
                        record.sha256.clone()
                    } else {
                        projection_digest
                    },
                    size: if text.is_empty() {
                        record.size
                    } else {
                        text.len() as u64
                    },
                    name: record.path.clone(),
                    title: record.title.clone(),
                    media_type: "text/plain".into(),
                    content: if text.is_empty() {
                        Content::Unreadable
                    } else {
                        Content::Text {
                            path: file,
                            range: None,
                        }
                    },
                    text_ranges: None,
                    language: None,
                    metadata,
                    locator: Some(format!("representation-byte:{start}")),
                };
                let size = serde_json::to_vec(&document)?.len();
                if size > 4 * 1024 * 1024 {
                    bail!("one source projection exceeds the kernel shard metadata format");
                }
                if !current.documents.is_empty()
                    && (encoded + size > 4 * 1024 * 1024 || current.documents.len() >= 8000)
                {
                    shards.push(current);
                    current = ScopeView {
                        scope: format!("shard-{}", shards.len()),
                        documents: Vec::new(),
                        unavailable_objects: 0,
                    };
                    encoded = 0;
                }
                encoded += size;
                current.documents.push(document);
                if end == record.text.len() {
                    break;
                }
                start = end;
            }
        }
        if !current.documents.is_empty() || shards.is_empty() {
            shards.push(current);
        }
        Ok(shards)
    }
    /// Build ALL actual lexical shards before the legacy complete tuple pointer can move.
    pub(super) fn publish(snapshot: &Snapshot, funding: &mut Funding<'_>) -> Result<()> {
        let directory = corpus(snapshot)?;
        let setup = ready(Work::reserve(
            Reservation {
                memory_bytes: 64 * 1024,
                temporary_bytes: 64 * 1024,
            },
            Origin::BeforeVerification,
        )?)?;
        fs::create_dir_all(&directory)?;
        drop(setup);
        let views = documents(snapshot, &directory, true)?;
        let kernel = KernelAdapter::open(&directory, &directory.join("kernel"), &build_identity())?;
        let mut generations = BTreeMap::new();
        for view in views {
            let scope = view.scope.clone();
            let size = serde_json::to_vec(&view)?.len() as u64;
            let need = KernelAdapter::publish_requirement(size)?;
            let mut work = match funding {
                Funding::BeforeVerification => {
                    ready(Work::reserve(need, Origin::BeforeVerification)?)?
                }
                Funding::Claim { claim, expected } => Work::split_claim(claim, *expected, need)?,
            };
            let expected = kernel.generation(&view.scope)?;
            let generation = kernel.publish(view, expected, &mut work, &control())?;
            if let Funding::Claim { claim, expected } = funding {
                work.rejoin(claim, *expected)?;
            }
            generations.insert(scope, generation);
        }
        PUBLISHED.with(|published| *published.borrow_mut() = Some(serde_json::json!({"version":1,"consumer":build_identity(),"corpus":directory.file_name().and_then(|name| name.to_str()),"generations":generations})));
        Ok(())
    }
    /// Write only a small exact tuple reference into the retained compatibility generation.
    /// The real lexical components are complete in the external shared-kernel cache already.
    pub(super) fn write_reference(directory: &Path) -> Result<()> {
        let reference = PUBLISHED
            .with(|published| published.borrow().clone())
            .context("actual shared kernel tuple was not prepared before publication")?;
        let bytes = serde_json::to_vec(&reference)?;
        let _work = ready(Work::reserve(
            Reservation {
                memory_bytes: bytes.len() as u64,
                temporary_bytes: bytes.len() as u64,
            },
            Origin::BeforeVerification,
        )?)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("kernel-reference.json"))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(())
    }
    /// Verify an actual original again immediately before returning derived evidence.
    fn verify(kb: &KnowledgeBase, record: &FileRecord) -> Result<()> {
        if hash_source(kb, &record.path)? != record.sha256 {
            bail!("source changed; refresh the library index before using evidence");
        }
        Ok(())
    }
    /// Query actual kernel shards; mature metadata/vector methods remain separate nonlexical adapters.
    pub(super) fn lexical(
        kb: &KnowledgeBase,
        snapshot: &Snapshot,
        options: &SearchOptions,
    ) -> Result<Vec<SearchHit>> {
        let directory = corpus(snapshot)?;
        if !directory.is_dir() {
            bail!("shared kernel generation is unavailable; rebuild this library's index");
        }
        let kernel = KernelAdapter::open(&directory, &directory.join("kernel"), &build_identity())?;
        let mut views = documents(snapshot, &directory, false)?;
        let mut lists = Vec::new();
        let query = if options.mode == SearchMode::Grep && !options.case_sensitive {
            format!("(?i:{})", options.query)
        } else {
            options.query.clone()
        };
        for view in &mut views {
            let mut output = Vec::new();
            view.documents.retain(|document| {
                let number: usize = document.key.id.split(':').next().unwrap().parse().unwrap();
                options.tags.iter().all(|tag| {
                    snapshot.records[number].tags.iter().any(|actual| {
                        if options.case_sensitive {
                            actual == tag
                        } else {
                            actual.to_lowercase() == tag.to_lowercase()
                        }
                    })
                })
            });
            let read_work = view.documents.iter().try_fold(0_u64, |sum, document| {
                let body = if matches!(document.content, Content::Text { .. }) {
                    document.size
                } else {
                    0
                };
                sum.checked_add(body)
                    .and_then(|n| n.checked_add(64 * 1024))
                    .context("query byte accounting overflow")
            })?;
            charge_bytes(read_work)?;
            let mut work = ready(Work::reserve(
                KernelAdapter::query_requirement(),
                Origin::BeforeVerification,
            )?)?;
            let invocation = if options.mode == SearchMode::Grep {
                control().text_scans()
            } else {
                control()
            };
            let result = kernel.search(
                Query {
                    mode: if options.mode == SearchMode::Grep {
                        Mode::Regex
                    } else {
                        Mode::Lexical
                    },
                    query: query.clone(),
                    scopes: vec![view.clone()],
                    limit: options.limit.clamp(1, 1000),
                },
                &mut work,
                &invocation,
            )?;
            control().check()?;
            if result.coverage.stopped_work > 0 {
                bail!("shared kernel work is incomplete; no absence claim is available");
            }
            for hit in result.hits {
                let (number, part) = hit
                    .key
                    .id
                    .split_once(':')
                    .context("invalid projection identity")?;
                let record = &snapshot.records[number.parse::<usize>()?];
                verify(kb, record)?;
                let line = if let Locator::Text { start, .. } = hit.locator {
                    let offset = part
                        .parse::<usize>()?
                        .checked_add(start as usize)
                        .context("locator overflow")?
                        .min(record.text.len());
                    Some(
                        record.text.as_bytes()[..offset]
                            .iter()
                            .filter(|b| **b == b'\n')
                            .count()
                            + 1,
                    )
                } else {
                    None
                };
                let timed = if record.transcript.is_empty() {
                    None
                } else {
                    let selected = if options.mode == SearchMode::Bm25 {
                        record.transcript.iter().find(|segment| {
                            options.query.split_whitespace().any(|word| {
                                segment.text.to_lowercase().contains(&word.to_lowercase())
                            })
                        })
                    } else {
                        line.and_then(|line| record.transcript.get(line.saturating_sub(1)))
                    };
                    selected.map(|segment| (segment.start_seconds, segment.end_seconds))
                };
                charge_hit(kb, record, hit.snippet.len())?;
                output.push(SearchHit {
                    root: kb.root.clone(),
                    path: record.path.clone(),
                    title: record.title.clone(),
                    snippet: hit.snippet,
                    mode: if options.mode == SearchMode::Grep {
                        "grep".into()
                    } else {
                        "bm25".into()
                    },
                    score: hit.score.unwrap_or(1.0),
                    line: if timed.is_some() { None } else { line },
                    start_seconds: timed.map(|time| time.0),
                    end_seconds: timed.map(|time| time.1),
                    tags: record.tags.clone(),
                });
            }
            lists.push(output);
        }
        if options.mode == SearchMode::Bm25 && lists.len() > 1 {
            let mut fused = fuse(lists, options.limit)?;
            for hit in &mut fused {
                hit.mode = "bm25".into();
            }
            return Ok(fused);
        }
        let mut output: Vec<SearchHit> = lists.into_iter().flatten().collect();
        output.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.line.cmp(&b.line))
        });
        output.dedup_by(|a, b| a.path == b.path && a.line == b.line);
        output.truncate(options.limit);
        Ok(output)
    }
    /// Preserve retained single-mode APIs while replacing every production lexical invocation.
    pub(super) fn search(
        kb: &KnowledgeBase,
        snapshot: &Snapshot,
        directory: &Path,
        options: &SearchOptions,
    ) -> Result<Vec<SearchHit>> {
        search::search(kb, snapshot, directory, options)
    }
    /// One file vote per method/keyword list, preserving distinct evidence and deterministic ties.
    pub(super) fn fuse(lists: Vec<Vec<SearchHit>>, limit: usize) -> Result<Vec<SearchHit>> {
        let bytes = lists.iter().flatten().try_fold(0u64, |sum, h| {
            sum.checked_add(h.path.len() as u64 * 4 + 1024)
                .context("fusion accounting overflow")
        })?;
        charge_result(bytes)?;
        let mut files: HashMap<String, (f32, Vec<SearchHit>)> = HashMap::new();
        for list in lists {
            let mut seen = std::collections::HashSet::new();
            let mut rank = 0;
            for hit in list {
                let first = seen.insert(hit.path.clone());
                let entry = files.entry(hit.path.clone()).or_default();
                if first {
                    rank += 1;
                    entry.0 += 1.0 / (60.0 + rank as f32);
                }
                if !entry.1.iter().any(|old| {
                    old.line == hit.line
                        && old.start_seconds == hit.start_seconds
                        && old.end_seconds == hit.end_seconds
                }) {
                    entry.1.push(hit);
                }
            }
        }
        let mut files: Vec<_> = files.into_iter().collect();
        files.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then_with(|| a.0.cmp(&b.0)));
        Ok(files
            .into_iter()
            .take(limit)
            .flat_map(|(_, (score, mut hits))| {
                for hit in &mut hits {
                    hit.score = score;
                }
                hits
            })
            .collect())
    }
    /// Preserve unified keyword alternatives through the same real kernel and mature vector adapter.
    pub(super) fn unified(
        kb: &KnowledgeBase,
        snapshot: &Snapshot,
        directory: &Path,
        options: &UnifiedSearchOptions,
    ) -> Result<Vec<SearchHit>> {
        search::search_unified(kb, snapshot, directory, options)
    }
}

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Root marker and user configuration filename.
pub const CONFIG_FILE: &str = "omni-knowledge.json";
/// Maximum extracted UTF-8 content per file; oversized sources remain metadata searchable.
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;

/// User-owned library settings; version changes must be explicitly supported.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Durable configuration schema version.
    pub version: u32,
    /// Display name for this library.
    pub name: String,
    /// Stable UUID lineage for portable indexes; absent until an explicit legacy index upgrade.
    #[serde(default)]
    pub library_id: Option<String>,
    /// Optional loopback provider; lexical search does not require it.
    #[serde(default)]
    pub embedding: Option<embedding::EmbeddingConfig>,
    /// Optional local Whisper executable for audio transcription.
    #[serde(default)]
    pub whisper_executable: Option<PathBuf>,
    /// Local Whisper model path; never downloaded implicitly.
    #[serde(default)]
    pub whisper_model: Option<PathBuf>,
}

/// Editable metadata for one root-relative file; retained independently of indexes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Metadata schema version.
    pub version: u32,
    /// Canonical root-relative UTF-8 source path.
    pub path: String,
    /// Exact labels whose original spelling is preserved.
    #[serde(default)]
    pub tags: Vec<String>,
    /// User or agent description, including binary contents that cannot be parsed.
    #[serde(default)]
    pub description: String,
}

/// Indexed source metadata and bounded extracted text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileRecord {
    /// Root-relative source identity.
    pub path: String,
    /// Front matter title or filename.
    pub title: String,
    /// Guessed MIME type; unknown types use application/octet-stream.
    pub media_type: String,
    /// Original source size in bytes.
    pub size: u64,
    /// Source modification timestamp in Unix milliseconds.
    pub modified: u64,
    /// Source SHA-256 for cache invalidation and evidence.
    pub sha256: String,
    /// Hash of transcript and ASR dependencies for safe media cache invalidation.
    #[serde(default)]
    pub dependency_hash: String,
    /// Combined exact front matter and editable metadata tags.
    pub tags: Vec<String>,
    /// Editable asset description.
    pub description: String,
    /// Extracted content; Markdown retains original line coordinates.
    pub text: String,
    /// Parser coverage such as extracted or metadata-only.
    pub status: String,
    /// Extraction, truncation, or provider errors affecting coverage.
    pub warnings: Vec<String>,
    /// Timed speech evidence, when supplied or locally transcribed.
    #[serde(default)]
    pub transcript: Vec<media::TranscriptSegment>,
}

impl FileRecord {
    /// Returns compact source metadata and coverage for CLI and agent discovery.
    pub fn metadata_summary(&self) -> serde_json::Value {
        serde_json::json!({"path":self.path,"title":self.title,"media_type":self.media_type,"size":self.size,"modified":self.modified,"sha256":self.sha256,"tags":self.tags,"description":self.description,"status":self.status,"warnings":self.warnings,"extracted_bytes":self.text.len()})
    }
}

/// A vector tied to a source fragment and one declared model space.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VectorRecord {
    /// Root-relative source.
    pub path: String,
    /// Human-readable source fragment evidence.
    pub text: String,
    /// Original text start line, when applicable.
    pub line: Option<usize>,
    /// Start of a video or audio segment in seconds.
    pub start_seconds: Option<f64>,
    /// End of a video or audio segment in seconds.
    pub end_seconds: Option<f64>,
    /// Finite, normalized provider vector.
    pub vector: Vec<f32>,
}

/// Complete derived generation; the published pointer selects one snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    /// Manifest schema version.
    pub version: u32,
    /// Original build path: legacy validation and derived mutable-cache namespace, never authority.
    pub root: PathBuf,
    /// Stable marker identity; whole-folder copies intentionally retain the same lineage.
    #[serde(default)]
    pub library_id: Option<String>,
    /// Build time in Unix milliseconds.
    pub indexed_at: u64,
    /// Source records sorted by root-relative path.
    pub records: Vec<FileRecord>,
    /// Vectors for sources or fragments.
    pub vectors: Vec<VectorRecord>,
    /// Provider identity used by all vectors in this generation.
    pub embedding: Option<embedding::EmbeddingConfig>,
    /// Trigram candidate postings for literal case-sensitive grep.
    pub trigrams: BTreeMap<String, Vec<usize>>,
    /// End actual record/vector buffers before the last memory/cache/reader capability drops.
    #[serde(skip, default = "admitted::allocation")]
    _allocation: Option<std::sync::Arc<admitted::Memory>>,
}

/// Reconciliation controls shared by CLI and desktop.
#[derive(Clone, Debug, Default)]
pub struct IndexOptions {
    /// Generate text, image, video, and transcribed-audio vectors.
    pub embeddings: bool,
    /// Re-extract unchanged files and regenerate vectors.
    pub force: bool,
}

/// Coverage report returned after a completed atomic publication.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexReport {
    /// Total admitted regular files.
    pub files: usize,
    /// Files with extracted content.
    pub extracted: usize,
    /// Files with metadata only.
    pub metadata_only: usize,
    /// Total vectors in the new snapshot.
    pub vectors: usize,
    /// Reused extracted-content records.
    pub reused: usize,
    /// Warnings including source-specific missing coverage.
    pub warnings: Vec<String>,
    /// Published generation identifier.
    pub generation: String,
}

/// Retrieval strategy; exact tag filters apply to every mode.
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// Indexed word relevance with English and Chinese token support.
    #[default]
    Bm25,
    /// Verified regex matches and line evidence over the extracted snapshot.
    Grep,
    /// Local provider vector similarity over indexed fragments.
    Semantic,
    /// Reciprocal-rank fusion of lexical and semantic candidates.
    Hybrid,
    /// Filename and metadata browsing, including unknown binary formats.
    Files,
}

/// Independently selectable retrieval methods for a shared comma-separated keyword query.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum SearchMethod {
    /// Indexed English and Chinese word relevance.
    Bm25,
    /// Literal case-insensitive text matching with source line evidence.
    Grep,
    /// Local multimodal similarity over compatible indexed vectors.
    Semantic,
    /// Case-insensitive filename, title, and description substrings.
    Files,
    /// Exact case-insensitive tag values as an independent retrieval source.
    Tags,
}

/// Shared keywords searched as alternatives through every enabled retrieval method.
#[derive(Clone, Debug)]
pub struct UnifiedSearchOptions {
    /// Keywords separated by ASCII or Chinese commas; blank and repeated keywords are ignored.
    pub query: String,
    /// Enabled peer methods; nonempty keywords require at least one method.
    pub methods: Vec<SearchMethod>,
    /// Maximum returned evidence records, capped at 10,000.
    pub limit: usize,
}

impl Default for UnifiedSearchOptions {
    /// Creates an indexed word request with a 30-record limit.
    fn default() -> Self {
        Self {
            query: String::new(),
            methods: vec![SearchMethod::Bm25],
            limit: 30,
        }
    }
}

impl UnifiedSearchOptions {
    /// Returns trimmed lowercase keyword alternatives, deduplicated in their first-input order.
    pub fn keywords(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.query
            .split([',', '，'])
            .map(|keyword| keyword.trim().to_lowercase())
            .filter(|keyword| !keyword.is_empty() && seen.insert(keyword.clone()))
            .collect()
    }
}

/// Search request; tags are AND filters and limits apply after filtering.
#[derive(Clone, Debug)]
pub struct SearchOptions {
    /// Natural text, regex, or filename substring according to mode.
    pub query: String,
    /// Retrieval strategy.
    pub mode: SearchMode,
    /// Required exact tags; matching ignores case unless `case_sensitive` is set.
    pub tags: Vec<String>,
    /// Maximum returned hits, capped at 10,000.
    pub limit: usize,
    /// Case sensitivity for grep, filenames, and exact tag filters.
    pub case_sensitive: bool,
    /// Optional image query for semantic or hybrid retrieval.
    pub query_image: Option<PathBuf>,
}

impl Default for SearchOptions {
    /// Creates a lexical request with a 30-result limit and no filters.
    fn default() -> Self {
        Self {
            query: String::new(),
            mode: SearchMode::Bm25,
            tags: vec![],
            limit: 30,
            case_sensitive: false,
            query_image: None,
        }
    }
}

/// Source-linked retrieval evidence; scores are relevance values, not probabilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchHit {
    /// Absolute root identity, including in recursive searches.
    pub root: PathBuf,
    /// Root-relative source.
    pub path: String,
    /// Display title.
    pub title: String,
    /// Bounded matching content or description.
    pub snippet: String,
    /// Retrieval strategy that supplied the hit.
    pub mode: String,
    /// Relevance or similarity score.
    pub score: f32,
    /// Original line for text or extracted line for converted documents.
    pub line: Option<usize>,
    /// Media start time.
    pub start_seconds: Option<f64>,
    /// Media end time.
    pub end_seconds: Option<f64>,
    /// Exact tags for inspection.
    pub tags: Vec<String>,
}

/// Owns the cooperative writer lock for exactly one library operation.
/// Explicit unlock prevents unrelated fork-before-exec children from extending its lifetime.
struct WriterGuard {
    /// Locked open file description; no child process is an intended owner.
    file: File,
}

impl Drop for WriterGuard {
    /// Releases operation ownership before closing a possibly inherited file description.
    fn drop(&mut self) {
        // Closing alone retains flock ownership while an unrelated child still has a copy.
        // A close remains the fallback if the OS rejects explicit unlock.
        let _ = FileExt::unlock(&self.file);
    }
}

/// Open library handle; source reads and writes remain beneath its root.
#[derive(Clone, Debug)]
pub struct KnowledgeBase {
    /// Canonical selected library root.
    pub root: PathBuf,
    /// Current user settings loaded from its marker.
    pub config: Config,
}

impl KnowledgeBase {
    /// Initializes a folder without overwriting content and adds two source reference concepts.
    /// Returns an error on collisions, symlinked managed paths, or filesystem failures.
    pub fn init(path: &Path, name: &str) -> Result<Self> {
        fs::create_dir_all(path).context("create library folder")?;
        let root = path.canonicalize()?;
        let files = storage::seed_files(name);
        for (relative, _) in &files {
            storage::check_components(&root, Path::new(relative), false)?;
            if root.join(relative).exists() {
                bail!("initialization collision: {relative}; existing content is preserved");
            }
        }
        storage::check_components(&root, Path::new(".omni/metadata"), false)?;
        for directory in [
            "raw",
            "wiki/sources",
            "wiki/concepts",
            ".omni/metadata",
            ".omni/indexes",
        ] {
            storage::check_components(&root, Path::new(directory), false)?;
            fs::create_dir_all(root.join(directory))?;
        }
        let config = Config {
            version: 1,
            name: name.to_owned(),
            library_id: Some(uuid::Uuid::new_v4().to_string()),
            embedding: Some(embedding::EmbeddingConfig::default()),
            whisper_executable: None,
            whisper_model: None,
        };
        // Create the root marker last; a interrupted initialization never appears complete.
        for (relative, text) in files {
            if relative != CONFIG_FILE {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root.join(relative))?;
                file.write_all(text.as_bytes())?;
                file.sync_all()?;
            }
        }
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(CONFIG_FILE))?;
        serde_json::to_writer_pretty(&mut marker, &config)?;
        marker.write_all(b"\n")?;
        marker.sync_all()?;
        Ok(Self { root, config })
    }

    /// Opens the nearest ancestor library of an existing path and validates its schema.
    pub fn open(path: &Path) -> Result<Self> {
        let canonical = path.canonicalize().context("library path does not exist")?;
        let mut directory = if canonical.is_file() {
            canonical.parent().unwrap().to_path_buf()
        } else {
            canonical
        };
        loop {
            if storage::has_marker(&directory)? {
                let config = storage::read_config(&directory)?;
                storage::check_components(&directory, Path::new(".omni"), false)?;
                return Ok(Self {
                    root: directory,
                    config,
                });
            }
            if !directory.pop() {
                bail!("no {CONFIG_FILE} found in path or ancestors");
            }
        }
    }

    /// Resolves an existing source path, rejecting traversal, symlinks, internal state, and child ownership.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf> {
        self.owned_path(relative, true)
    }

    /// Resolves a source or future import path under its nearest owner without following symlinks.
    fn owned_path(&self, relative: &str, required: bool) -> Result<PathBuf> {
        let path = Path::new(relative);
        storage::validate_relative(path)?;
        if path.components().next() == Some(Component::Normal(".omni".as_ref())) {
            bail!("internal state is not a source");
        }
        storage::check_components(&self.root, path, required)?;
        let absolute = self.root.join(path);
        let mut ancestor = absolute.parent();
        while let Some(directory) = ancestor {
            if directory == self.root {
                break;
            }
            if storage::has_marker(directory)? {
                bail!("source belongs to nested library {}", directory.display());
            }
            ancestor = directory.parent();
        }
        Ok(absolute)
    }

    /// Copies an external regular source to raw/ and records tags without overwriting a destination.
    pub fn import(&self, source: &Path, tags: &[String]) -> Result<String> {
        if !source.is_file() {
            bail!("import source must be a regular file");
        }
        let name = source
            .file_name()
            .context("source has no filename")?
            .to_str()
            .context("source filename is not UTF-8")?;
        let destination_name = if name == CONFIG_FILE {
            "omni-knowledge.asset.json"
        } else {
            name
        };
        let relative = format!("raw/{destination_name}");
        self.owned_path(&relative, false)?;
        let _lock = self.writer_lock()?;
        let mut input = File::open(source)?;
        let mut output = tempfile::NamedTempFile::new_in(self.root.join("raw"))?;
        std::io::copy(&mut input, &mut output)?;
        output.as_file().sync_all()?;
        output
            .persist_noclobber(self.root.join(&relative))
            .map_err(|error| error.error)
            .context("destination already exists or cannot be created")?;
        let metadata = AssetMetadata {
            version: 1,
            path: relative.clone(),
            tags: normalized_tags(tags),
            description: String::new(),
        };
        self.write_metadata(&metadata)?;
        Ok(relative)
    }

    /// Replaces editable tags and optionally the description of an existing owned source.
    pub fn set_metadata(
        &self,
        relative: &str,
        tags: &[String],
        description: Option<&str>,
    ) -> Result<()> {
        let path = self.resolve(relative)?;
        if !path.is_file() {
            bail!("metadata target must be a regular file");
        }
        let _lock = self.writer_lock()?;
        let mut metadata = self.read_metadata(relative)?;
        metadata.tags = normalized_tags(tags);
        if let Some(text) = description {
            metadata.description = text.to_owned();
        }
        self.write_metadata(&metadata)
    }

    /// Returns one source's user metadata; absent sidecars return an empty editable record.
    pub fn metadata(&self, relative: &str) -> Result<AssetMetadata> {
        self.resolve(relative)?;
        self.read_metadata(relative)
    }

    /// Reconciles all owned files and atomically publishes a complete searchable generation.
    /// An unavailable requested provider fails before replacing the previous generation.
    pub fn index(&self, options: &IndexOptions) -> Result<IndexReport> {
        self.index_funded(
            options,
            &mut knowledge_processing::funding::Funding::BeforeVerification,
            None,
        )
    }
    /// Execute M3 through the exact native caller's funded claim, never another reservation.
    /// Funding is NOT source/write authority. The owner retains that context and its storage
    /// publication responsibility. None gives M3 remaining credit after simultaneous buffers.
    pub fn index_funded(
        &self,
        options: &IndexOptions,
        funding: &mut knowledge_processing::funding::Funding<'_>,
        child_rss_bytes: Option<u64>,
    ) -> Result<IndexReport> {
        admitted::memory_funded(self, true, funding, |funding| {
            let _lock = self.writer_lock()?;
            let library_id = storage::identity_for_index(self)?;
            let old = storage::load(self).ok().map(|loaded| loaded.0);
            let previous: HashMap<&str, &FileRecord> = old
                .as_ref()
                .map(|s| s.records.iter().map(|r| (r.path.as_str(), r)).collect())
                .unwrap_or_default();
            let provider_config = if options.embeddings {
                Some(
                    self.config
                        .embedding
                        .clone()
                        .context("embedding is not configured")?,
                )
            } else {
                old.as_ref().and_then(|s| s.embedding.clone())
            };
            let provider = if options.embeddings {
                let client = embedding::EmbeddingClient::new(provider_config.as_ref().unwrap())?;
                client.health()?;
                Some(client)
            } else {
                None
            };
            let mut report = IndexReport {
                files: 0,
                extracted: 0,
                metadata_only: 0,
                vectors: 0,
                reused: 0,
                warnings: vec![],
                generation: String::new(),
            };
            let mut records = vec![];
            let mut vectors = vec![];
            let speech_identity = self.speech_identity();
            for relative in self.source_paths()? {
                admitted::funded_checkpoint(funding)?;
                let original = self.resolve(&relative)?;
                let selected_media = options.embeddings
                    && matches!(
                        media::media_kind(&original),
                        Some("audio" | "video" | "image")
                    );
                let stage = admitted::stage(
                    self,
                    &relative,
                    if selected_media {
                        None
                    } else {
                        Some(MAX_TEXT_BYTES as u64 + 1)
                    },
                )?;
                let path = stage.path.clone();
                let info = &stage.metadata;
                let hash = stage.digest.clone();
                let metadata = self.read_metadata_funded(&relative, funding)?;
                let kind = media::media_kind(&path);
                let transcript_path =
                    PathBuf::from(format!("{}.transcript.json", original.display()));
                let dependency_hash = if matches!(kind, Some("audio" | "video")) {
                    if transcript_path.exists() {
                        let transcript_relative = transcript_path
                            .strip_prefix(&self.root)?
                            .to_str()
                            .context("transcript path is not UTF-8")?;
                        self.resolve(transcript_relative)?;
                        admitted::hash_source(self, transcript_relative)?
                    } else {
                        format!("missing-transcript:{speech_identity}")
                    }
                } else {
                    String::new()
                };
                let dependency_hash =
                    format!("{}:{dependency_hash}", admitted::processor_identity());
                let mut record = if !options.force
                    && previous
                        .get(relative.as_str())
                        .is_some_and(|r| r.sha256 == hash && r.dependency_hash == dependency_hash)
                {
                    report.reused += 1;
                    (*previous[relative.as_str()]).clone()
                } else {
                    let extraction = knowledge_processing::extract::extract_file_funded(
                        &stage.file,
                        &path,
                        MAX_TEXT_BYTES,
                        child_rss_bytes,
                        funding,
                    );
                    let extracted = match extraction {
                        Ok(value) => admitted::retain_funded(value, funding)?,
                        Err(error) => return Err(admitted::processing_error(error, stage, None)),
                    };
                    FileRecord {
                        path: relative.clone(),
                        title: path.file_name().unwrap().to_string_lossy().into_owned(),
                        media_type: mime_guess::from_path(&path)
                            .first_or_octet_stream()
                            .to_string(),
                        size: info.len(),
                        modified: 0,
                        sha256: hash.clone(),
                        dependency_hash: dependency_hash.clone(),
                        tags: vec![],
                        description: String::new(),
                        text: extracted.text,
                        status: extracted.status,
                        warnings: extracted.warnings,
                        transcript: vec![],
                    }
                };
                record.warnings.retain(|warning| {
                    !warning.starts_with("embedding unavailable:")
                        && !warning.starts_with("transcription unavailable:")
                });
                if matches!(kind, Some("audio" | "video"))
                    && (options.force
                        || record.transcript.is_empty()
                        || record.dependency_hash != dependency_hash)
                {
                    let directory = admitted::tempdir()?;
                    let speech = if transcript_path.exists() {
                        let relative = transcript_path
                            .strip_prefix(&self.root)?
                            .to_str()
                            .context("transcript path is not UTF-8")?;
                        let transcript_stage =
                            admitted::stage(self, relative, Some(MAX_TEXT_BYTES as u64 + 1))?;
                        Some(
                            match knowledge_processing::media::parse_transcript_file_funded(
                                &transcript_stage.file,
                                Some(MAX_TEXT_BYTES as u64),
                                funding,
                            ) {
                                Ok(value) => admitted::retain_funded(value, funding),
                                Err(error) => {
                                    return Err(admitted::processing_error(
                                        error,
                                        transcript_stage,
                                        Some(directory),
                                    ));
                                }
                            },
                        )
                    } else if options.embeddings {
                        if let (Some(executable), Some(model)) =
                            (&self.config.whisper_executable, &self.config.whisper_model)
                        {
                            let storage_bytes = funding.available()?.temporary_bytes / 2;
                            Some(
                                match knowledge_processing::media::transcribe_file_funded(
                                    &stage.file,
                                    executable,
                                    model,
                                    directory.path(),
                                    knowledge_processing::media::AsrPlan {
                                        child_rss_bytes,
                                        storage_bytes,
                                        transcript_bytes: MAX_TEXT_BYTES,
                                    },
                                    funding,
                                ) {
                                    Ok(value) => admitted::retain_funded(value, funding),
                                    Err(error) => {
                                        return Err(admitted::processing_error(
                                            error,
                                            stage,
                                            Some(directory),
                                        ));
                                    }
                                },
                            )
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(result) = speech {
                        match result {
                            Ok(items) => {
                                record.text = items
                                    .iter()
                                    .map(|item| item.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                record.transcript = items;
                                record.status = "transcript".into();
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    record.dependency_hash = dependency_hash.clone();
                }
                record.size = info.len();
                record.modified = info
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|t| t.as_millis() as u64)
                    .unwrap_or(0);
                record.description = metadata.description.clone();
                let (frontmatter, _) = extract::markdown_frontmatter(&record.text);
                if let Some(title) = frontmatter.get("title").and_then(|v| v.as_str()) {
                    record.title = title.to_owned();
                }
                record.tags = normalized_tags(&metadata.tags);
                if let Some(tags) = frontmatter.get("tags").and_then(|v| v.as_array()) {
                    record
                        .tags
                        .extend(tags.iter().filter_map(|t| t.as_str()).map(str::to_owned));
                    record.tags = normalized_tags(&record.tags);
                }
                if let Some(tags) = frontmatter.get("tags").and_then(|v| v.as_str()) {
                    record.tags.extend(tags.split(',').map(str::to_owned));
                    record.tags = normalized_tags(&record.tags);
                }
                // Create a general metadata record for every regular file without replacing edits.
                if !self.metadata_path(&relative)?.exists() {
                    self.write_metadata(&metadata)?;
                }
                let content_unchanged = previous.get(relative.as_str()).is_some_and(|r| {
                    r.sha256 == hash
                        && r.dependency_hash == dependency_hash
                        && r.description == record.description
                        && r.text == record.text
                });
                let old_vectors: Vec<VectorRecord> = old
                    .as_ref()
                    .filter(|s| s.embedding == provider_config)
                    .map(|s| {
                        s.vectors
                            .iter()
                            .filter(|v| v.path == relative)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                if content_unchanged && !options.force && !old_vectors.is_empty() {
                    vectors.extend(old_vectors);
                } else if let Some(client) = &provider {
                    match self.vectorize(&record, &stage, client, funding, child_rss_bytes) {
                        Ok(items) => vectors.extend(items),
                        Err(error) if admitted::resource_failure(&error) => {
                            return Err(admitted::processing_error(error, stage, None));
                        }
                        Err(error)
                            if error
                                .downcast_ref::<embedding::ProviderRequestError>()
                                .is_some() =>
                        {
                            return Err(error).context(format!(
                                "provider failed for {relative}; previous index preserved"
                            ));
                        }
                        Err(error) => record
                            .warnings
                            .push(format!("embedding unavailable: {error:#}")),
                    }
                }
                report.files += 1;
                if record.text.is_empty() {
                    report.metadata_only += 1;
                } else {
                    report.extracted += 1;
                }
                report.warnings.extend(
                    record
                        .warnings
                        .iter()
                        .map(|warning| format!("{relative}: {warning}")),
                );
                if admitted::hash_source(self, &relative)? != hash {
                    bail!("source changed while indexing; previous complete generation preserved");
                }
                records.push(record);
            }
            // Preserve the old wire field, not another lexical/trigram construction. Actual grep
            // and lexical evidence both come from the admitted vendored kernel component.
            let trigrams = BTreeMap::new();
            let snapshot = Snapshot {
                _allocation: admitted::allocation(),
                version: 1,
                root: self.root.clone(),
                library_id: Some(library_id),
                indexed_at: now_ms(),
                records,
                vectors,
                embedding: provider_config,
                trigrams,
            };
            report.vectors = snapshot.vectors.len();
            admitted::publish(&snapshot, funding)?;
            let (generation, cleanup_warnings) = storage::publish(self, &snapshot)?;
            report.generation = generation;
            report.warnings.extend(cleanup_warnings);
            Ok(report)
        })
    }

    /// Searches one published snapshot and refuses unindexed or incompatible semantic requests.
    pub fn search(&self, options: &SearchOptions) -> Result<Vec<SearchHit>> {
        if options.limit == 0 {
            return Ok(vec![]);
        }
        if options.limit > 10_000 {
            bail!("result limit exceeds 10000");
        }
        admitted::memory(self, false, || {
            let loaded = storage::load_search(self)?;
            let current = KnowledgeBase {
                root: self.root.clone(),
                config: loaded.config.clone(),
            };
            admitted::search(&current, &loaded.snapshot, &loaded.directory, options)
        })
    }

    /// Searches peer methods and keyword alternatives under one validated snapshot reader lease.
    ///
    /// Empty keywords browse indexed files; invalid methods, result bounds, and unavailable selected
    /// semantic retrieval return errors instead of silently returning partial local results.
    pub fn search_unified(&self, options: &UnifiedSearchOptions) -> Result<Vec<SearchHit>> {
        if options.methods.is_empty() && !options.keywords().is_empty() {
            bail!("select at least one search method");
        }
        if options.limit > 10_000 {
            bail!("result limit exceeds 10000");
        }
        if options.limit == 0 {
            return Ok(vec![]);
        }
        admitted::memory(self, false, || {
            let loaded = storage::load_search(self)?;
            let current = KnowledgeBase {
                root: self.root.clone(),
                config: loaded.config.clone(),
            };
            admitted::unified(&current, &loaded.snapshot, &loaded.directory, options)
        })
    }

    /// Lists indexed regular files; call index to reconcile recent filesystem edits.
    pub fn list(&self) -> Result<Vec<FileRecord>> {
        Ok(self.snapshot()?.records)
    }

    /// Returns status from the published snapshot without contacting any model.
    pub fn status(&self) -> Result<serde_json::Value> {
        let snapshot = self.snapshot()?;
        Ok(
            serde_json::json!({"root":self.root,"name":self.config.name,"indexed_at":snapshot.indexed_at,"files":snapshot.records.len(),"vectors":snapshot.vectors.len(),"embedding":snapshot.embedding,"warnings":snapshot.records.iter().flat_map(|r| r.warnings.iter().map(move |w| format!("{}: {}",r.path,w))).collect::<Vec<_>>() }),
        )
    }

    /// Finds child roots without following symlinks; each is independently opened and indexed.
    pub fn children(&self) -> Result<Vec<Self>> {
        let mut result = vec![];
        for entry in WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !matches!(e.file_name().to_str(), Some(".omni" | ".git")))
        {
            let entry = entry?;
            if entry.file_type().is_file()
                && entry.file_name() == CONFIG_FILE
                && entry.path().parent() != Some(self.root.as_path())
            {
                result.push(Self::open(entry.path())?);
            }
        }
        result.sort_by(|a, b| a.root.cmp(&b.root));
        Ok(result)
    }

    /// Reads the selected immutable snapshot and rejects foreign or malformed state.
    fn snapshot(&self) -> Result<Snapshot> {
        admitted::memory(self, false, || Ok(storage::load(self)?.0))
    }

    /// Enumerates owned regular files, stopping at child libraries and internal state.
    fn source_paths(&self) -> Result<Vec<String>> {
        let mut paths = vec![];
        let mut walker = WalkDir::new(&self.root).follow_links(false).into_iter();
        while let Some(entry) = walker.next() {
            let entry = entry?;
            if entry.depth() > 0 && entry.file_type().is_dir() {
                let name = entry.file_name().to_str().unwrap_or("");
                if matches!(name, ".omni" | ".git") || storage::has_marker(entry.path())? {
                    walker.skip_current_dir();
                    continue;
                }
            }
            if entry.file_type().is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(&self.root)?
                    .to_str()
                    .context("source path is not UTF-8")?
                    .to_owned();
                if relative != CONFIG_FILE && relative != ".DS_Store" {
                    paths.push(relative);
                }
            }
        }
        paths.sort();
        Ok(paths)
    }

    /// Acquires the sole mutation lock and recreates missing managed directories after restoration.
    fn writer_lock(&self) -> Result<WriterGuard> {
        for path in [".omni", ".omni/metadata", ".omni/indexes"] {
            storage::check_components(&self.root, Path::new(path), false)?;
            fs::create_dir_all(self.root.join(path))?;
        }
        storage::check_components(&self.root, Path::new(".omni/write.lock"), false)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(".omni/write.lock"))?;
        file.try_lock_exclusive()
            .context("another library writer is active; retry after it finishes")?;
        let writer = WriterGuard { file };
        storage::check_components(&self.root, Path::new(".omni/read.lock"), false)?;
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(".omni/read.lock"))?;
        Ok(writer)
    }

    /// Returns the authored sidecar location for an already validated canonical root-relative source identity.
    /// Rejects symlink components and managed-path inspection failures; does not require or create a sidecar.
    pub(crate) fn metadata_path(&self, relative: &str) -> Result<PathBuf> {
        let filename = format!(
            ".omni/metadata/{}.json",
            hex::encode(Sha256::digest(relative.as_bytes()))
        );
        storage::check_components(&self.root, Path::new(&filename), false)?;
        Ok(self.root.join(filename))
    }

    /// Reads and validates user metadata; absent records receive safe defaults.
    fn read_metadata(&self, relative: &str) -> Result<AssetMetadata> {
        self.read_metadata_funded(
            relative,
            &mut knowledge_processing::funding::Funding::BeforeVerification,
        )
    }
    fn read_metadata_funded(
        &self,
        relative: &str,
        funding: &mut knowledge_processing::funding::Funding<'_>,
    ) -> Result<AssetMetadata> {
        let path = self.metadata_path(relative)?;
        if !path.exists() {
            return Ok(AssetMetadata {
                version: 1,
                path: relative.into(),
                tags: vec![],
                description: String::new(),
            });
        }
        let metadata: AssetMetadata = admitted::metadata_funded(File::open(path)?, funding)?;
        if metadata.version != 1 || metadata.path != relative {
            bail!("metadata identity mismatch for {relative}");
        }
        Ok(metadata)
    }

    /// Atomically replaces one metadata sidecar; caller must hold the writer lock.
    fn write_metadata(&self, metadata: &AssetMetadata) -> Result<()> {
        storage::write_json_atomic(&self.metadata_path(&metadata.path)?, metadata)
    }

    /// Computes the identity of explicitly configured speech tools; model contents control cache reuse.
    fn speech_identity(&self) -> String {
        match (&self.config.whisper_executable, &self.config.whisper_model) {
            (Some(executable), Some(model)) => format!(
                "{}:{}:{}:{}",
                executable.display(),
                hash_file(executable).unwrap_or_else(|_| "unavailable".into()),
                model.display(),
                hash_file(model).unwrap_or_else(|_| "unavailable".into())
            ),
            (None, None) => "no-asr".into(),
            _ => "incomplete-asr-config".into(),
        }
    }

    /// Creates local vectors with source line or timestamp evidence and separate metadata evidence.
    fn vectorize(
        &self,
        record: &FileRecord,
        stage: &admitted::SourceStage,
        client: &embedding::EmbeddingClient,
        funding: &mut knowledge_processing::funding::Funding<'_>,
        child_rss_bytes: Option<u64>,
    ) -> Result<Vec<VectorRecord>> {
        let path = &stage.path;
        let mut output = vec![];
        let kind = media::media_kind(path);
        let metadata_text = format!("{}\n{}", record.title, record.description);
        for (_, chunk) in text_chunks(&metadata_text, 1600) {
            output.push(VectorRecord {
                path: record.path.clone(),
                text: chunk.clone(),
                line: None,
                start_seconds: None,
                end_seconds: None,
                vector: client.embed_text(&chunk)?,
            });
        }
        match kind {
            Some("image") => output.push(VectorRecord {
                path: record.path.clone(),
                text: record.title.clone(),
                line: None,
                start_seconds: None,
                end_seconds: None,
                vector: client.embed_image(path)?,
            }),
            Some("video") => {
                let directory = admitted::tempdir()?;
                let storage_bytes = funding.available()?.temporary_bytes / 2;
                let segments = knowledge_processing::media::segment_video_file_funded(
                    &stage.file,
                    directory.path(),
                    10.0,
                    knowledge_processing::media::ClipPlan {
                        child_rss_bytes,
                        storage_bytes,
                        max_segments: 100_000,
                    },
                    funding,
                );
                let segments = match segments {
                    Ok(value) => value,
                    Err(error) => {
                        return Err(admitted::scratch_error(
                            error,
                            directory,
                            stage.file.try_clone()?,
                        ));
                    }
                };
                let result = (|| -> Result<()> {
                    for segment in &segments.value {
                        output.push(VectorRecord {
                            path: record.path.clone(),
                            text: format!(
                                "{} [{:.1}–{:.1}s]",
                                record.title, segment.start_seconds, segment.end_seconds
                            ),
                            line: None,
                            start_seconds: Some(segment.start_seconds),
                            end_seconds: Some(segment.end_seconds),
                            vector: client.embed_video(&segment.path)?,
                        });
                    }
                    Ok(())
                })();
                let knowledge_processing::admission::Admitted {
                    value,
                    mut retained,
                } = segments;
                drop(value);
                drop(directory);
                for holding in &mut retained {
                    holding.finish()?;
                }
                result?;
            }
            Some("audio") => {
                if record.transcript.is_empty() {
                    bail!(
                        "audio needs <filename>.transcript.json or configured whisper_executable and whisper_model"
                    );
                }
            }
            _ => {
                for (line, chunk) in text_chunks(&record.text, 1600) {
                    output.push(VectorRecord {
                        path: record.path.clone(),
                        text: chunk.clone(),
                        line: Some(line),
                        start_seconds: None,
                        end_seconds: None,
                        vector: client.embed_text(&chunk)?,
                    });
                }
            }
        }
        for item in &record.transcript {
            for (_, chunk) in text_chunks(&item.text, 1600) {
                output.push(VectorRecord {
                    path: record.path.clone(),
                    text: chunk.clone(),
                    line: None,
                    start_seconds: Some(item.start_seconds),
                    end_seconds: Some(item.end_seconds),
                    vector: client.embed_text(&chunk)?,
                });
            }
        }
        Ok(output)
    }
}

/// Native selected-library composition for the existing service runtime.
/// Transport adapters must retain `Delivery` until their bounded write has actually settled.
pub mod native_service {
    #[cfg(test)]
    mod embedding_tests;
    mod extraction;
    mod held;
    #[cfg(test)]
    mod held_tests;
    mod inventory;
    mod models;
    mod source;
    use super::*;
    use base64::Engine;
    pub use extraction::{
        NativeAdmitted, NativeAdmittedError, NativeEmbeddedText, NativeEmbeddingError,
        NativeEmbeddingProducer, NativeEmbeddingState, NativeExtraction, NativeExtractionError,
        NativeIndexInput,
    };
    pub use held::{NativeEpubMember, NativeEpubPlan, NativeVideoPlan};
    pub use inventory::{NativeInventory, NativeInventoryPage, NativeObservedSource};
    use knowledge_core::admission::{Decision, Origin};
    use knowledge_core::jobs::Reservation;
    use knowledge_retrieval::Work;
    pub use models::NativeEmbeddingSelection;
    use omni_service as wire;
    pub use source::{NativeJobPlan, NativeProcessing, NativeSource};
    use std::os::unix::fs::MetadataExt;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use wire::authority::{
        HumanApproval, LocalPolicy, NativeAuthority, Owner, Permit, Selection, Session,
        TransportFacts,
    };
    use wire::runtime::{Delivery, Domain, DomainHost, ResolvedScope, Runtime, RuntimeError};

    /// Native marker bytes from the selected descriptor, not a pathname or decoded authority claim.
    fn current_marker(root: &File) -> std::result::Result<[u8; 32], wire::ErrorCode> {
        // The actual selection's CurrentContext retains its prepaid 64KiB scratch/FD
        // lifetime. Checks are serialized by that SAME selected-root lock, not late
        // global reservations that could falsely revoke a valid fully funded source.
        let mut file = File::from(
            rustix::fs::openat(
                root,
                CONFIG_FILE,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .map_err(|_| wire::ErrorCode::NotExposed)?,
        );
        let before = file.metadata().map_err(refused)?;
        if !before.is_file() || before.len() > wire::MAX_CONTROL_FRAME_BYTES as u64 {
            return Err(wire::ErrorCode::NotExposed);
        }
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 8192];
        let mut count = 0u64;
        loop {
            let n = file.read(&mut buffer).map_err(refused)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > before.len() {
                return Err(wire::ErrorCode::NotExposed);
            }
            hash.update(&buffer[..n]);
        }
        if count != before.len()
            || LibraryHost::stat(&before) != LibraryHost::stat(&file.metadata().map_err(refused)?)
        {
            return Err(wire::ErrorCode::NotExposed);
        }
        Ok(hash.finalize().into())
    }
    /// Actual native runtime plus real selected-library hostcalls, never an authority inferred from JSON.
    pub struct NativeLibraryService {
        authority: NativeAuthority,
        selection: Selection,
        library: wire::Id,
        runtime: Runtime,
        host: Mutex<LibraryHost>,
    }
    impl NativeLibraryService {
        /// Bind a known ordinary-local library and explicitly owner-selected current transport policy.
        /// Returns the separate native owner/selection capabilities; do not give these to transports.
        /// Nonlocal/cloud homes require their qualified backend adapter instead of this constructor.
        pub fn ordinary_local(
            kb: KnowledgeBase,
            policy: LocalPolicy,
        ) -> Result<(Self, Owner, Selection)> {
            admitted::start()?;
            let marker_memory = match Work::reserve(
                Reservation {
                    memory_bytes: 64 * 1024,
                    temporary_bytes: 0,
                },
                Origin::BeforeVerification,
            )? {
                Decision::Ready(work) => work,
                Decision::Waiting(reason) => return Err(admitted::WaitingFor(reason).into()),
            };
            let current = KnowledgeBase::open(&kb.root)?;
            if current.root != kb.root || current.config.library_id != kb.config.library_id {
                bail!("selected library context changed");
            }
            let library = wire::Id::new(kb.config.library_id.clone().unwrap_or_else(|| {
                format!(
                    "legacy-{}",
                    hex::encode(Sha256::digest(kb.root.as_os_str().as_encoded_bytes()))
                )
            }))?;
            let (authority, owner) = NativeAuthority::new();
            let selected_root = File::open(&kb.root)?;
            let selected_path = kb.root.clone();
            let marker = current_marker(&selected_root)
                .map_err(|_| anyhow::anyhow!("native marker unavailable"))?;
            let selection = owner
                .select_current(
                    library.clone(),
                    selected_root,
                    policy,
                    std::sync::Arc::new(move |root| {
                        let _actual_context_credit = &marker_memory;
                        let Ok(held) = root.metadata() else {
                            return false;
                        };
                        let Ok(current) = fs::symlink_metadata(&selected_path) else {
                            return false;
                        };
                        current.is_dir()
                            && !current.file_type().is_symlink()
                            && (held.dev(), held.ino()) == (current.dev(), current.ino())
                            && current_marker(root).is_ok_and(|digest| digest == marker)
                    }),
                )
                .map_err(|_| anyhow::anyhow!("native library selection refused"))?;
            let memory = match Work::reserve(
                Reservation {
                    memory_bytes: 4096,
                    temporary_bytes: 0,
                },
                Origin::BeforeVerification,
            )? {
                Decision::Ready(work) => work,
                Decision::Waiting(_) => bail!("waiting for native host memory"),
            };
            let service = Self {
                runtime: Runtime::new(authority.clone()),
                authority,
                selection: selection.clone(),
                library: library.clone(),
                host: Mutex::new(LibraryHost {
                    kb,
                    library,
                    sources: HashMap::new(),
                    extensions: None,
                    extension_selected: false,
                    memory,
                }),
            };
            Ok((service, owner, selection))
        }
        /// Label of the actual native selection; it is not a permission or a root path.
        pub fn library_id(&self) -> &wire::Id {
            &self.library
        }
        /// Connect only this native selection using actual unmodified transport observations.
        pub fn connect(
            &self,
            facts: TransportFacts,
        ) -> std::result::Result<Session, wire::ErrorCode> {
            self.authority
                .connect(std::slice::from_ref(&self.selection), facts)
        }
        /// Install real typed owner adapters while preserving this runtime's native checks.
        /// Missing owner implementations stay explicitly unsupported before effects, not fake successes.
        pub fn bind_domains(&self, domains: Box<dyn DomainHost + Send>) -> Result<()> {
            self.host
                .lock()
                .map_err(|_| anyhow::anyhow!("native domain host unavailable"))?
                .extensions = Some(domains);
            Ok(())
        }
        /// Begin a native CLI/domain scope over the actual existing command catalog/session.
        /// This performs no IO/effect and is not a DTO-created grant. Source opens below recheck paths.
        pub fn native_scope(
            &self,
            session: &Session,
            command: &wire::Command,
            approval: Option<HumanApproval>,
        ) -> std::result::Result<Permit, wire::ErrorCode> {
            self.runtime.native_scope(session, command, approval)
        }
        /// Open/hash exactly the currently selected descriptor-relative source, retaining native
        /// root/read-policy/source-version checks and real delivery resources. Never reopen its hint.
        pub fn source(
            &self,
            permit: &Permit,
            path: &wire::RelativePath,
            verification: knowledge_core::backend::Budget,
        ) -> Result<NativeSource> {
            NativeSource::open(&self.library, path, permit, verification)
        }
        /// Invoke the actual shared dispatcher; retain the returned Delivery until bytes settle.
        pub fn dispatch(
            &self,
            frame: &[u8],
            session: &Session,
        ) -> std::result::Result<Delivery, RuntimeError> {
            let mut host = self
                .host
                .lock()
                .map_err(|_| RuntimeError::Admission(wire::ErrorCode::Internal))?;
            self.runtime.dispatch(frame, session, &mut *host)
        }
        /// Consume one separately confirmed exact native intention, never a DTO confirmation field.
        pub fn dispatch_approved(
            &self,
            frame: &[u8],
            session: &Session,
            approval: HumanApproval,
        ) -> std::result::Result<Delivery, RuntimeError> {
            let mut host = self
                .host
                .lock()
                .map_err(|_| RuntimeError::Admission(wire::ErrorCode::Internal))?;
            self.runtime
                .dispatch_approved(frame, session, &mut *host, Some(approval))
        }
    }
    /// Real native source references are retained by this host, with a same-account lifetime.
    struct LibraryHost {
        kb: KnowledgeBase,
        library: wire::Id,
        sources: HashMap<String, String>,
        extensions: Option<Box<dyn DomainHost + Send>>,
        /// An owner adapter claims support during resolution; execution is never retried after effects.
        extension_selected: bool,
        // Drop maps/owner adapters before ending their native metadata resource lifetime.
        memory: Work,
    }
    /// Convert local errors without reflecting paths, query strings or private source bytes.
    fn refused(_: impl std::fmt::Display) -> wire::ErrorCode {
        wire::ErrorCode::Changed
    }
    /// Preserve contention/whole-budget/provider failures without leaking raw native diagnostics.
    fn work_error(error: anyhow::Error) -> wire::ErrorCode {
        if error
            .chain()
            .any(|cause| cause.is::<admitted::WaitingFor>())
        {
            return wire::ErrorCode::Busy;
        }
        if error.chain().any(|cause| {
            cause.is::<knowledge_core::admission::Error>()
                || cause.is::<knowledge_core::backend::BackendError>()
                || cause.is::<knowledge_retrieval::Error>()
                || cause.is::<knowledge_processing::AdmitError>()
                || cause.is::<knowledge_processing::WorkerFailure>()
        }) {
            return wire::ErrorCode::BudgetExhausted;
        }
        if error
            .chain()
            .any(|cause| cause.is::<embedding::ProviderRequestError>())
        {
            return wire::ErrorCode::ProviderUnavailable;
        }
        wire::ErrorCode::Changed
    }
    impl LibraryHost {
        /// Bind a source label only after native selected-root/exclusion admission, before retaining it.
        fn remember(
            &mut self,
            path: &str,
            permit: &Permit,
        ) -> std::result::Result<wire::Id, wire::ErrorCode> {
            let relative =
                wire::RelativePath::new(path).map_err(|_| wire::ErrorCode::Unsupported)?;
            permit.check_path(&self.library, &relative)?;
            let id = wire::Id::new(format!(
                "source-{}",
                hex::encode(Sha256::digest(path.as_bytes()))
            ))
            .map_err(|_| wire::ErrorCode::Internal)?;
            if !self.sources.contains_key(id.as_str()) {
                let bytes = (path.len() as u64)
                    .checked_mul(4)
                    .and_then(|v| v.checked_add(512))
                    .ok_or(wire::ErrorCode::BudgetExhausted)?;
                match self
                    .memory
                    .grow(Reservation {
                        memory_bytes: bytes,
                        temporary_bytes: 0,
                    })
                    .map_err(|_| wire::ErrorCode::BudgetExhausted)?
                {
                    Decision::Ready(()) => {}
                    Decision::Waiting(_) => return Err(wire::ErrorCode::Busy),
                }
                self.sources.insert(id.as_str().to_owned(), path.into());
            }
            Ok(id)
        }
        /// Resolve an opaque source only from this native host's selected-root references.
        fn path(
            &self,
            source: &wire::SourceRef,
            permit: &Permit,
        ) -> std::result::Result<String, wire::ErrorCode> {
            if source.library_id != self.library {
                return Err(wire::ErrorCode::NotExposed);
            }
            let path = self
                .sources
                .get(source.source_id.as_str())
                .ok_or(wire::ErrorCode::NotExposed)?;
            permit.check_path(
                &self.library,
                &wire::RelativePath::new(path).map_err(|_| wire::ErrorCode::NotExposed)?,
            )?;
            Ok(path.clone())
        }
        /// Open through the actual held selected descriptor, refusing symlink components/races.
        fn open(&self, path: &str, permit: &Permit) -> std::result::Result<File, wire::ErrorCode> {
            permit.with_root(&self.library, |root| Self::open_root(root, path))
        }
        fn open_root(root: &File, path: &str) -> std::result::Result<File, wire::ErrorCode> {
            if path.is_empty() {
                return root.try_clone().map_err(|_| wire::ErrorCode::Denied);
            }
            let mut directory = root.try_clone().map_err(|_| wire::ErrorCode::Denied)?;
            let mut parts = path.split('/').peekable();
            while let Some(part) = parts.next() {
                if part.is_empty() || part == "." || part == ".." {
                    return Err(wire::ErrorCode::InvalidRequest);
                }
                let mut flags = rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK;
                if parts.peek().is_some() {
                    flags |= rustix::fs::OFlags::DIRECTORY;
                }
                directory = File::from(
                    rustix::fs::openat(&directory, part, flags, rustix::fs::Mode::empty())
                        .map_err(|error| {
                            if error == rustix::io::Errno::NOENT {
                                wire::ErrorCode::NotFound
                            } else {
                                wire::ErrorCode::Denied
                            }
                        })?,
                );
                if parts.peek().is_some()
                    && rustix::fs::statat(
                        &directory,
                        CONFIG_FILE,
                        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                    )
                    .is_ok()
                {
                    return Err(wire::ErrorCode::NotExposed);
                }
            }
            Ok(directory)
        }
        fn track_version(
            library: &wire::Id,
            path: &str,
            input: &File,
            before: &fs::Metadata,
            permit: &Permit,
        ) -> std::result::Result<(), wire::ErrorCode> {
            let expected = Self::stat(before);
            if input.metadata().map_err(refused).map(|m| Self::stat(&m))? != expected {
                return Err(wire::ErrorCode::Changed);
            }
            permit.reserve_result(2048 + path.len() as u64 * 4)?;
            let root = permit.with_root(library, |root| {
                root.try_clone().map_err(|_| wire::ErrorCode::Denied)
            })?;
            let observed = input.try_clone().map_err(|_| wire::ErrorCode::Denied)?;
            let path = path.to_owned();
            permit.retain_check(std::sync::Arc::new(move || {
                observed
                    .metadata()
                    .is_ok_and(|m| Self::stat(&m) == expected)
                    && Self::open_root(&root, &path)
                        .is_ok_and(|file| file.metadata().is_ok_and(|m| Self::stat(&m) == expected))
            }))
        }
        /// Read only the bounded native sidecar; source details never hashes or parses originals.
        fn metadata(
            &self,
            path: &str,
            permit: &Permit,
        ) -> std::result::Result<AssetMetadata, wire::ErrorCode> {
            let relative = format!(
                ".omni/metadata/{}.json",
                hex::encode(Sha256::digest(path.as_bytes()))
            );
            let input = match self.open(&relative, permit) {
                Err(wire::ErrorCode::NotFound) => {
                    return Ok(AssetMetadata {
                        version: 1,
                        path: path.into(),
                        tags: vec![],
                        description: String::new(),
                    });
                }
                Err(error) => return Err(error),
                Ok(input) => input,
            };
            let before = input.metadata().map_err(refused)?;
            let bytes = before.len();
            if bytes > wire::MAX_CONTROL_FRAME_BYTES as u64 {
                return Err(wire::ErrorCode::BudgetExhausted);
            }
            let memory = bytes
                .checked_mul(16)
                .and_then(|n| n.checked_add(64 * 1024))
                .ok_or(wire::ErrorCode::BudgetExhausted)?;
            permit.reserve_result(memory)?;
            let _work = match Work::reserve(
                Reservation {
                    memory_bytes: memory,
                    temporary_bytes: 0,
                },
                Origin::BeforeVerification,
            )
            .map_err(|_| wire::ErrorCode::BudgetExhausted)?
            {
                Decision::Ready(work) => work,
                Decision::Waiting(_) => return Err(wire::ErrorCode::Busy),
            };
            let metadata: AssetMetadata = serde_json::from_reader((&input).take(bytes))
                .map_err(|_| wire::ErrorCode::InvalidRequest)?;
            Self::track_version(&self.library, &relative, &input, &before, permit)?;
            if metadata.version != 1 || metadata.path != path {
                return Err(wire::ErrorCode::Changed);
            }
            Ok(metadata)
        }
        /// Report actual native metadata without hashing the source or asserting verified content.
        fn source(
            &mut self,
            path: &str,
            permit: &Permit,
        ) -> std::result::Result<wire::Source, wire::ErrorCode> {
            permit.reserve_result(4096 + path.len() as u64 * 8)?;
            let source_id = self.remember(path, permit)?;
            let native = Path::new(path);
            let input = self.open(path, permit)?;
            let info = input.metadata().map_err(refused)?;
            Self::track_version(&self.library, path, &input, &info, permit)?;
            if !info.is_file() {
                return Err(wire::ErrorCode::Unsupported);
            }
            let metadata = self.metadata(path, permit)?;
            let relative =
                wire::RelativePath::new(path).map_err(|_| wire::ErrorCode::Unsupported)?;
            Ok(wire::Source {
                library_id: self.library.clone(),
                source_id,
                mount_id: None,
                path: relative.clone(),
                logical_path: relative,
                title: native
                    .file_name()
                    .and_then(|p| p.to_str())
                    .ok_or(wire::ErrorCode::Unsupported)?
                    .into(),
                description: metadata.description,
                tags: metadata.tags,
                annotation_path: None,
                digest: None,
                digest_state: wire::DigestState::Unverified,
                version: None,
                size_bytes: Some(wire::U64::new(info.len())),
                media_type: Some(
                    mime_guess::from_path(native)
                        .first_or_octet_stream()
                        .to_string(),
                ),
                created_at: None,
                modified_at: None,
                availability: wire::Availability::Available,
                residency: wire::Residency::Unknown,
                activity: wire::Activity::Stable,
                conflict_source_id: None,
            })
        }
        /// Actual descriptor/stat evidence complements a full digest; inode alone is not stable identity.
        fn stat(info: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
            (
                info.dev(),
                info.ino(),
                info.len(),
                info.mtime(),
                info.mtime_nsec(),
                info.ctime(),
                info.ctime_nsec(),
            )
        }
        /// Fully verify selected ordinary-local bytes under explicit work limits and fresh policy.
        fn read(
            &self,
            path: &str,
            expected: Option<&wire::ExpectedVersion>,
            offset: u64,
            length: u64,
            budget: &wire::Budget,
            permit: &Permit,
        ) -> std::result::Result<(wire::ExpectedVersion, Vec<u8>), wire::ErrorCode> {
            if budget.max_files.get() == 0
                || budget.max_millis.get() == 0
                || budget.max_memory_bytes.get() < 64 * 1024 + length
            {
                return Err(wire::ErrorCode::BudgetExhausted);
            }
            permit.reserve_result(
                length
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(64 * 1024))
                    .ok_or(wire::ErrorCode::BudgetExhausted)?,
            )?;
            let mut input = self.open(path, permit)?;
            let before = input.metadata().map_err(refused)?;
            if expected.is_some_and(|version| version.size_bytes.get() != before.len()) {
                return Err(wire::ErrorCode::Changed);
            }
            if !before.is_file() || before.len() > budget.max_bytes.get() {
                return Err(wire::ErrorCode::BudgetExhausted);
            }
            let deadline = Instant::now()
                .checked_add(Duration::from_millis(budget.max_millis.get()))
                .ok_or(wire::ErrorCode::BudgetExhausted)?;
            let _work = match Work::reserve(
                Reservation {
                    memory_bytes: 64 * 1024 + length,
                    temporary_bytes: 0,
                },
                Origin::BeforeVerification,
            )
            .map_err(|_| wire::ErrorCode::BudgetExhausted)?
            {
                Decision::Ready(work) => work,
                Decision::Waiting(_) => return Err(wire::ErrorCode::Busy),
            };
            let mut bytes = Vec::with_capacity(length as usize);
            let mut buffer = [0_u8; 65536];
            let mut position = 0_u64;
            let mut hash = Sha256::new();
            loop {
                permit.check_path(
                    &self.library,
                    &wire::RelativePath::new(path).map_err(|_| wire::ErrorCode::Unsupported)?,
                )?;
                if Instant::now() >= deadline {
                    return Err(wire::ErrorCode::BudgetExhausted);
                }
                let count = input
                    .read(&mut buffer)
                    .map_err(|_| wire::ErrorCode::Denied)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
                let end = position
                    .checked_add(count as u64)
                    .ok_or(wire::ErrorCode::BudgetExhausted)?;
                let start = position.max(offset);
                let stop = end.min(offset.saturating_add(length));
                if stop > start {
                    bytes.extend_from_slice(
                        &buffer[(start - position) as usize..(stop - position) as usize],
                    );
                }
                position = end;
            }
            let after = input.metadata().map_err(refused)?;
            let current = self.open(path, permit)?.metadata().map_err(refused)?;
            if Self::stat(&before) != Self::stat(&after)
                || Self::stat(&before) != Self::stat(&current)
            {
                return Err(wire::ErrorCode::Changed);
            }
            Self::track_version(&self.library, path, &input, &before, permit)?;
            let version = wire::ExpectedVersion {
                digest: wire::Digest::new(hex::encode(hash.finalize()))
                    .map_err(|_| wire::ErrorCode::Internal)?,
                size_bytes: wire::U64::new(position),
                modified_ns: Some(
                    wire::SignedDecimal::new(
                        (i128::from(before.mtime()) * 1_000_000_000
                            + i128::from(before.mtime_nsec()))
                        .to_string(),
                    )
                    .map_err(|_| wire::ErrorCode::Internal)?,
                ),
                changed_ns: Some(
                    wire::SignedDecimal::new(
                        (i128::from(before.ctime()) * 1_000_000_000
                            + i128::from(before.ctime_nsec()))
                        .to_string(),
                    )
                    .map_err(|_| wire::ErrorCode::Internal)?,
                ),
                file_id: Some(format!("{}:{}", before.dev(), before.ino())),
                backend_version: None,
            };
            if expected.is_some_and(|expected| *expected != version) {
                return Err(wire::ErrorCode::Changed);
            }
            if offset > position {
                return Err(wire::ErrorCode::InvalidRequest);
            }
            permit.check()?;
            Ok((version, bytes))
        }
        /// Public projection of the actual selected retained library, not a new history/mount claim.
        fn library(&self, permit: &Permit) -> std::result::Result<wire::Library, wire::ErrorCode> {
            Ok(wire::Library {
                library_id: self.library.clone(),
                name: self.kb.config.name.clone(),
                home_path: if permit.is_http() {
                    String::new()
                } else {
                    self.kb.root.display().to_string()
                },
                text_root: wire::TextRoot::Wiki,
                manifest_version: Some(
                    wire::SafeInteger::new(self.kb.config.version.into())
                        .map_err(|_| wire::ErrorCode::Internal)?,
                ),
                state: wire::LibraryState::MigrationRequired,
                history_mode: wire::HistoryMode::NotInitialized,
                exposure: permit.visible_exposure(&self.library)?,
                mounts: vec![],
                sync_schedule: wire::SyncSchedule {
                    enabled: false,
                    interval_minutes: wire::UnsignedInteger::new(0)
                        .map_err(|_| wire::ErrorCode::Internal)?,
                    allow_push: false,
                    next_run_at: None,
                },
                remote_url: None,
                remote_access: wire::RemoteAccess::None,
            })
        }
    }
    impl DomainHost for LibraryHost {
        /// Recheck actual selected native identity/current marker before resolving any source or owner ID.
        fn resolve(
            &mut self,
            command: &wire::Command,
            permit: &Permit,
        ) -> std::result::Result<ResolvedScope, wire::ErrorCode> {
            permit.with_root(&self.library, |root| {
                let held = root.metadata().map_err(|_| wire::ErrorCode::NotExposed)?;
                let current =
                    fs::metadata(&self.kb.root).map_err(|_| wire::ErrorCode::NotExposed)?;
                if held.dev() != current.dev() || held.ino() != current.ino() {
                    return Err(wire::ErrorCode::NotExposed);
                }
                Ok(())
            })?;
            // The owner-bound marker and namespace are checked on EVERY permit use. Never refresh
            // this selected context by reopening its pathname after resolution.
            permit.check()?;
            self.extension_selected = false;
            if let Some(host) = self.extensions.as_mut() {
                match host.resolve(command, permit) {
                    Ok(scope) => {
                        self.extension_selected = true;
                        return Ok(scope);
                    }
                    Err(wire::ErrorCode::Unsupported) => {}
                    Err(error) => return Err(error),
                }
            }
            let path = match command {
                wire::Command::SourceDetails { source }
                | wire::Command::VerifySource { source, .. }
                | wire::Command::ReadRange { source, .. } => Some(self.path(source, permit)?),
                _ => None,
            };
            if let Some(path) = path {
                return Ok(ResolvedScope {
                    libraries: vec![self.library.clone()],
                    paths: vec![(
                        self.library.clone(),
                        wire::RelativePath::new(path).map_err(|_| wire::ErrorCode::Unsupported)?,
                    )],
                });
            }
            match command {
                wire::Command::LibraryList {}
                | wire::Command::LibraryStatus { .. }
                | wire::Command::MountList { .. }
                | wire::Command::Browse { .. }
                | wire::Command::ExposureGet { .. }
                | wire::Command::Query { .. } => Ok(ResolvedScope {
                    libraries: vec![self.library.clone()],
                    paths: vec![],
                }),
                _ => match self.extensions.as_mut() {
                    Some(host) => host.resolve(command, permit),
                    None => Err(wire::ErrorCode::Unsupported),
                },
            }
        }
        /// Real native selected-library reads and actual root-kernel invocation; other owners remain typed hostcalls.
        fn execute(
            &mut self,
            domain: Domain,
            command: &wire::Command,
            permit: &Permit,
        ) -> std::result::Result<wire::Result, wire::ErrorCode> {
            use wire::Command;
            if self.extension_selected {
                return self
                    .extensions
                    .as_mut()
                    .ok_or(wire::ErrorCode::Internal)?
                    .execute(domain, command, permit);
            }
            match command {
                Command::LibraryList {} => Ok(wire::Result::Libraries {
                    libraries: vec![self.library(permit)?],
                }),
                Command::MountList { .. } => Err(wire::ErrorCode::Unsupported),
                Command::LibraryStatus { .. } => {
                    if permit.has_exclusions(&self.library)? {
                        return Err(wire::ErrorCode::Unsupported);
                    }
                    let discovered = self.count_sources(permit)?;
                    let generation_id = match self.open(".omni/current.json", permit) {
                        Ok(file) => Some(
                            wire::Id::new(
                                serde_json::from_reader::<_, String>(file.take(64 * 1024))
                                    .map_err(refused)?,
                            )
                            .map_err(|_| wire::ErrorCode::Changed)?,
                        ),
                        Err(wire::ErrorCode::NotFound) => None,
                        Err(error) => return Err(error),
                    };
                    let mut cache_used = 0_u64;
                    if generation_id.is_some() {
                        let pinned = self.selected_snapshot(permit, u64::MAX)?;
                        let directory = admitted::corpus(&pinned.snapshot).map_err(refused)?;
                        if directory.is_dir() {
                            for entry in WalkDir::new(&directory).follow_links(false) {
                                let entry = entry.map_err(|_| wire::ErrorCode::Denied)?;
                                if entry.file_type().is_file() {
                                    cache_used = cache_used
                                        .checked_add(
                                            entry
                                                .metadata()
                                                .map_err(|_| wire::ErrorCode::Denied)?
                                                .len(),
                                        )
                                        .ok_or(wire::ErrorCode::BudgetExhausted)?;
                                }
                            }
                        }
                    }
                    let budgets = admitted::budgets().map_err(|_| wire::ErrorCode::Busy)?;
                    let coverage = wire::Coverage {
                        discovered: wire::U64::new(discovered),
                        verified: wire::U64::new(0),
                        pending_hash: wire::U64::new(discovered),
                        pending_extraction: wire::U64::new(0),
                        pending_embedding: wire::U64::new(0),
                        stale: wire::U64::new(0),
                        unavailable: wire::U64::new(0),
                        unsupported: wire::U64::new(0),
                        failed: wire::U64::new(0),
                        excluded: wire::U64::new(0),
                        enumeration: wire::Enumeration::Complete,
                        result_extent: wire::ResultExtent::Enumeration,
                        stopped_reason: None,
                    };
                    Ok(wire::Result::Status { status: Box::new(wire::Status { resource_budgets: Some(budgets.clone()), library: self.library(permit)?, coverage, jobs: vec![], generation_id, cache_path: String::new(), history_path: None, cache_used_bytes: wire::U64::new(cache_used), cache_budget_bytes: budgets.storage_budget_bytes, provider: if self.kb.config.embedding.is_some() { wire::ProviderState::Unavailable } else { wire::ProviderState::NotConfigured }, signer: wire::SignerState::Missing, recovery_operation_ids: vec![], warnings: vec!["retained library requires full-version migration; provider readiness is not established; owner domain adapters remain explicit".into()] }) })
                }
                Command::ExposureGet { library_id } => Ok(wire::Result::Exposure {
                    exposure: Box::new(permit.exposure(library_id)?),
                }),
                Command::Browse {
                    library_id,
                    mount_id,
                    path,
                    cursor,
                    limit,
                } => {
                    if mount_id.is_some() || cursor.is_some() {
                        return Err(wire::ErrorCode::Unsupported);
                    }
                    if path
                        .as_str()
                        .split('/')
                        .any(|part| matches!(part, ".omni" | ".git"))
                    {
                        return Err(wire::ErrorCode::NotExposed);
                    }
                    let directory = if path.as_str().is_empty() {
                        permit.with_root(library_id, |root| {
                            root.try_clone().map_err(|_| wire::ErrorCode::Denied)
                        })?
                    } else {
                        permit.check_path(
                            library_id,
                            &wire::RelativePath::new(path.as_str())
                                .map_err(|_| wire::ErrorCode::InvalidRequest)?,
                        )?;
                        self.open(path.as_str(), permit)?
                    };
                    let directory_version = directory.metadata().map_err(refused)?;
                    Self::track_version(
                        &self.library,
                        path.as_str(),
                        &directory,
                        &directory_version,
                        permit,
                    )?;
                    let mut entries = Vec::new();
                    let mut complete = true;
                    for entry in rustix::fs::Dir::read_from(&directory)
                        .map_err(|_| wire::ErrorCode::Denied)?
                    {
                        permit.check()?;
                        let entry = entry.map_err(|_| wire::ErrorCode::Denied)?;
                        let name = entry
                            .file_name()
                            .to_str()
                            .map_err(|_| wire::ErrorCode::Unsupported)?;
                        if matches!(name, "." | ".." | ".omni" | ".git" | CONFIG_FILE) {
                            continue;
                        }
                        let relative = wire::RelativePath::new(if path.as_str().is_empty() {
                            name.to_owned()
                        } else {
                            format!("{}/{name}", path.as_str())
                        })
                        .map_err(|_| wire::ErrorCode::Unsupported)?;
                        if permit.check_path(library_id, &relative).is_err() {
                            continue;
                        }
                        let stat = rustix::fs::statat(
                            &directory,
                            entry.file_name(),
                            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                        )
                        .map_err(|_| wire::ErrorCode::Changed)?;
                        let kind = rustix::fs::FileType::from_raw_mode(stat.st_mode);
                        if !matches!(
                            kind,
                            rustix::fs::FileType::RegularFile | rustix::fs::FileType::Directory
                        ) {
                            continue;
                        }
                        if kind == rustix::fs::FileType::Directory {
                            let child = self.open(relative.as_str(), permit)?;
                            if rustix::fs::statat(
                                &child,
                                CONFIG_FILE,
                                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                            )
                            .is_ok()
                            {
                                continue;
                            }
                        }
                        if entries.len() == limit.get() as usize {
                            complete = false;
                            break;
                        }
                        permit.reserve_result(
                            (relative.as_str().len() as u64 + name.len() as u64 + 1024) * 4,
                        )?;
                        let source = if kind == rustix::fs::FileType::RegularFile {
                            Some(self.source(relative.as_str(), permit)?)
                        } else {
                            None
                        };
                        entries.push(wire::ListingEntry {
                            name: name.to_owned(),
                            path: relative,
                            kind: if kind == rustix::fs::FileType::Directory {
                                wire::EntryKind::Directory
                            } else {
                                wire::EntryKind::File
                            },
                            source,
                        });
                    }
                    if !complete {
                        return Err(wire::ErrorCode::BudgetExhausted);
                    }
                    entries.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
                    Ok(wire::Result::Listing {
                        entries,
                        next_cursor: None,
                        complete,
                    })
                }
                Command::SourceDetails { source } => {
                    let path = self.path(source, permit)?;
                    Ok(wire::Result::Source {
                        source: Box::new(self.source(&path, permit)?),
                    })
                }
                Command::VerifySource { source, budget } => {
                    let path = self.path(source, permit)?;
                    let (version, _) = self.read(&path, None, 0, 0, budget, permit)?;
                    let mut result = self.source(&path, permit)?;
                    result.digest = Some(version.digest.clone());
                    result.version = Some(version);
                    result.digest_state = wire::DigestState::Verified;
                    Ok(wire::Result::Source {
                        source: Box::new(result),
                    })
                }
                Command::ReadRange {
                    source,
                    expected,
                    offset,
                    length,
                } => {
                    let path = self.path(source, permit)?;
                    let budget = wire::Budget {
                        max_files: wire::UnsignedInteger::new(1)
                            .map_err(|_| wire::ErrorCode::Internal)?,
                        max_bytes: expected.size_bytes,
                        max_millis: wire::UnsignedInteger::new(30_000)
                            .map_err(|_| wire::ErrorCode::Internal)?,
                        max_memory_bytes: wire::U64::new(64 * 1024 + length.get()),
                    };
                    let (version, bytes) = self.read(
                        &path,
                        Some(expected),
                        offset.get(),
                        length.get(),
                        &budget,
                        permit,
                    )?;
                    let next = offset
                        .get()
                        .checked_add(bytes.len() as u64)
                        .ok_or(wire::ErrorCode::BudgetExhausted)?;
                    Ok(wire::Result::Content {
                        source: source.clone(),
                        source_digest: version.digest,
                        representation_digest: None,
                        locator: wire::Locator::Unlocated,
                        bytes_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                        truncated: next < version.size_bytes.get(),
                        next_offset: (next < version.size_bytes.get())
                            .then_some(wire::U64::new(next)),
                        next_line: None,
                    })
                }
                Command::Query { query } => self.query(query, permit),
                _ => match self.extensions.as_mut() {
                    Some(host) => host.execute(domain, command, permit),
                    None => Err(wire::ErrorCode::Unsupported),
                },
            }
        }
    }
    struct SelectedSnapshot {
        snapshot: Snapshot,
        directory: PathBuf,
        _pin: File,
    }
    impl LibraryHost {
        fn count_sources(&self, permit: &Permit) -> std::result::Result<u64, wire::ErrorCode> {
            let root = permit.with_root(&self.library, |root| {
                root.try_clone().map_err(|_| wire::ErrorCode::Denied)
            })?;
            let mut stack = vec![(root, String::new())];
            let mut count = 0u64;
            let deadline = Instant::now() + Duration::from_secs(30);
            while let Some((directory, prefix)) = stack.pop() {
                for entry in
                    rustix::fs::Dir::read_from(&directory).map_err(|_| wire::ErrorCode::Denied)?
                {
                    permit.check()?;
                    if Instant::now() >= deadline {
                        return Err(wire::ErrorCode::BudgetExhausted);
                    }
                    let entry = entry.map_err(|_| wire::ErrorCode::Denied)?;
                    let name = entry
                        .file_name()
                        .to_str()
                        .map_err(|_| wire::ErrorCode::Unsupported)?;
                    if matches!(name, "." | ".." | ".omni" | ".git" | CONFIG_FILE) {
                        continue;
                    }
                    permit.reserve_result((prefix.len() + name.len() + 1024) as u64 * 4)?;
                    let path = if prefix.is_empty() {
                        name.to_owned()
                    } else {
                        format!("{prefix}/{name}")
                    };
                    let relative =
                        wire::RelativePath::new(&path).map_err(|_| wire::ErrorCode::Unsupported)?;
                    if permit.check_path(&self.library, &relative).is_err() {
                        continue;
                    }
                    let stat = rustix::fs::statat(
                        &directory,
                        entry.file_name(),
                        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                    )
                    .map_err(|_| wire::ErrorCode::Changed)?;
                    match rustix::fs::FileType::from_raw_mode(stat.st_mode) {
                        rustix::fs::FileType::RegularFile => {
                            count = count
                                .checked_add(1)
                                .ok_or(wire::ErrorCode::BudgetExhausted)?
                        }
                        rustix::fs::FileType::Directory => {
                            let child = self.open(&path, permit)?;
                            if rustix::fs::statat(
                                &child,
                                CONFIG_FILE,
                                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                            )
                            .is_err()
                            {
                                stack.push((child, path));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(count)
        }
        /// Decode and pin the actual selected descriptor's complete tuple, never a replacement pathname.
        fn selected_snapshot(
            &self,
            permit: &Permit,
            max_memory: u64,
        ) -> std::result::Result<SelectedSnapshot, wire::ErrorCode> {
            let pin = self.open(".omni/read.lock", permit)?;
            FileExt::try_lock_shared(&pin).map_err(|_| wire::ErrorCode::Busy)?;
            let pointer = self.open(".omni/current.json", permit)?;
            if pointer.metadata().map_err(refused)?.len() > 64 * 1024 {
                return Err(wire::ErrorCode::Changed);
            }
            let generation: String =
                serde_json::from_reader(pointer.take(64 * 1024)).map_err(refused)?;
            if !generation.starts_with("gen-")
                || !generation
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                return Err(wire::ErrorCode::Changed);
            }
            let directory = self.kb.root.join(".omni/indexes").join(&generation);
            let input = self.open(&format!(".omni/indexes/{generation}/manifest.json"), permit)?;
            let before = input.metadata().map_err(refused)?;
            let bytes = before
                .len()
                .checked_mul(24)
                .ok_or(wire::ErrorCode::BudgetExhausted)?;
            if bytes > max_memory {
                return Err(wire::ErrorCode::BudgetExhausted);
            }
            permit.reserve_result(bytes)?;
            let snapshot: Snapshot =
                serde_json::from_reader((&input).take(before.len() + 1)).map_err(refused)?;
            if LibraryHost::stat(&before) != LibraryHost::stat(&input.metadata().map_err(refused)?)
                || snapshot.version != 1
                || snapshot.library_id != self.kb.config.library_id
                || (snapshot.library_id.is_none() && snapshot.root != self.kb.root)
            {
                return Err(wire::ErrorCode::Changed);
            }
            for record in &snapshot.records {
                storage::validate_relative(Path::new(&record.path)).map_err(refused)?;
                if record
                    .path
                    .split('/')
                    .any(|p| matches!(p, ".omni" | ".git" | CONFIG_FILE))
                {
                    return Err(wire::ErrorCode::NotExposed);
                }
            }
            for vector in &snapshot.vectors {
                if snapshot
                    .embedding
                    .as_ref()
                    .is_none_or(|c| vector.vector.len() != c.dimension)
                    || embedding::validate_vector(&vector.vector).is_err()
                {
                    return Err(wire::ErrorCode::InvalidVector);
                }
            }
            permit.check()?;
            Ok(SelectedSnapshot {
                snapshot,
                directory,
                _pin: pin,
            })
        }
        /// Invoke actual root/kernel retrieval, returning truthful bounded/current-source evidence.
        fn query(
            &mut self,
            query: &wire::SearchRequest,
            permit: &Permit,
        ) -> std::result::Result<wire::Result, wire::ErrorCode> {
            let deadline = Instant::now()
                .checked_add(Duration::from_millis(query.budget.max_millis.get()))
                .ok_or(wire::ErrorCode::BudgetExhausted)?;
            let native = permit.clone();
            let control = knowledge_retrieval::Control::guarded(
                deadline,
                std::sync::Arc::new(move || native.check().is_ok()),
            );
            let root = permit.with_root(&self.library, |root| {
                root.try_clone().map_err(|_| wire::ErrorCode::Denied)
            })?;
            admitted::result_scope(
                permit.clone(),
                root,
                query.budget.max_memory_bytes.get(),
                || {
                    admitted::controlled(control, query.budget.max_bytes.get(), || {
                        self.query_controlled(query, permit)
                    })
                },
            )
        }
        /// Pin one actual complete tuple for all methods/keywords in this native invocation.
        fn query_controlled(
            &mut self,
            query: &wire::SearchRequest,
            permit: &Permit,
        ) -> std::result::Result<wire::Result, wire::ErrorCode> {
            if permit.has_exclusions(&self.library)? || query.image_query.is_some() {
                return Err(wire::ErrorCode::Unsupported);
            }
            let pinned = self.selected_snapshot(permit, query.budget.max_memory_bytes.get())?;
            let snapshot = &pinned.snapshot;
            let text_bytes = snapshot
                .records
                .iter()
                .try_fold(0_u64, |sum, record| {
                    sum.checked_add(record.text.len() as u64)
                })
                .ok_or(wire::ErrorCode::BudgetExhausted)?;
            if snapshot.records.len() as u64 > query.budget.max_files.get()
                || text_bytes > query.budget.max_bytes.get()
                || query.budget.max_memory_bytes.get() < 128 * 1024 * 1024
                || query.budget.max_millis.get() == 0
            {
                return Err(wire::ErrorCode::BudgetExhausted);
            }
            let deadline = Instant::now()
                .checked_add(Duration::from_millis(query.budget.max_millis.get()))
                .ok_or(wire::ErrorCode::BudgetExhausted)?;
            let mut lists = Vec::new();
            for keyword in &query.keywords {
                for method in &query.methods {
                    permit.check()?;
                    if Instant::now() >= deadline {
                        return Err(wire::ErrorCode::BudgetExhausted);
                    }
                    let mode = match method {
                        wire::SearchMethod::Bm25 => SearchMode::Bm25,
                        wire::SearchMethod::Literal | wire::SearchMethod::Regex => SearchMode::Grep,
                        wire::SearchMethod::Semantic => SearchMode::Semantic,
                        wire::SearchMethod::Files | wire::SearchMethod::Tags => SearchMode::Files,
                    };
                    if *method == wire::SearchMethod::Tags {
                        return Err(wire::ErrorCode::Unsupported);
                    }
                    let text = if *method == wire::SearchMethod::Literal {
                        regex::escape(keyword)
                    } else {
                        keyword.clone()
                    };
                    admitted::charge_result(1024).map_err(work_error)?;
                    lists.push(
                        admitted::search(
                            &self.kb,
                            snapshot,
                            &pinned.directory,
                            &SearchOptions {
                                query: text,
                                mode,
                                tags: query.required_tags.clone(),
                                limit: query.limit.get() as usize,
                                case_sensitive: query.case_sensitive,
                                query_image: None,
                            },
                        )
                        .map_err(work_error)?,
                    );
                }
            }
            let hits = admitted::fuse(lists, query.limit.get() as usize).map_err(work_error)?;
            let mut files: Vec<wire::FileHit> = Vec::new();
            for hit in hits {
                permit.check()?;
                if Instant::now() >= deadline {
                    return Err(wire::ErrorCode::BudgetExhausted);
                }
                permit.reserve_result(8192 + hit.snippet.len() as u64 * 8)?;
                let source = self.source(&hit.path, permit)?;
                let native_ref = wire::SourceRef {
                    library_id: self.library.clone(),
                    source_id: source.source_id.clone(),
                };
                let (version, _) = self.read(&hit.path, None, 0, 0, &query.budget, permit)?;
                if snapshot
                    .records
                    .iter()
                    .find(|r| r.path == hit.path)
                    .is_none_or(|r| {
                        r.sha256 != version.digest.as_str() || r.description != source.description
                    })
                {
                    return Err(wire::ErrorCode::Changed);
                }
                let mut source = source;
                source.digest = Some(version.digest.clone());
                source.version = Some(version.clone());
                source.digest_state = wire::DigestState::Verified;
                let method = match hit.mode.as_str() {
                    "grep" => {
                        if query.methods.contains(&wire::SearchMethod::Regex) {
                            wire::SearchMethod::Regex
                        } else {
                            wire::SearchMethod::Literal
                        }
                    }
                    "semantic" => wire::SearchMethod::Semantic,
                    "files" => wire::SearchMethod::Files,
                    _ => wire::SearchMethod::Bm25,
                };
                let locator = if let (Some(start), Some(end)) = (hit.start_seconds, hit.end_seconds)
                {
                    wire::Locator::Time {
                        start_ms: wire::U64::new((start * 1000.0) as u64),
                        end_ms: wire::U64::new((end * 1000.0) as u64),
                        representation_digest: None,
                    }
                } else {
                    wire::Locator::Unlocated
                };
                let evidence = wire::Evidence {
                    method,
                    keyword: None,
                    locator,
                    snippet: hit.snippet.chars().take(500).collect(),
                    kind: if method == wire::SearchMethod::Semantic {
                        wire::EvidenceKind::SemanticCandidate
                    } else if method == wire::SearchMethod::Files {
                        wire::EvidenceKind::MetadataObservation
                    } else {
                        wire::EvidenceKind::VerifiedMatch
                    },
                    source_digest: Some(version.digest),
                    representation_digest: None,
                    processor_id: None,
                    model_space: if method == wire::SearchMethod::Semantic {
                        snapshot
                            .embedding
                            .as_ref()
                            .map(|model| model.embedding_space.clone())
                    } else {
                        None
                    },
                    score: Some(
                        wire::FiniteNumber::new(hit.score.into())
                            .map_err(|_| wire::ErrorCode::InvalidVector)?,
                    ),
                };
                if let Some(file) = files
                    .iter_mut()
                    .find(|file| file.source.source_id == native_ref.source_id)
                {
                    file.evidence.push(evidence);
                    file.retained_evidence_count = wire::U64::new(file.evidence.len() as u64);
                } else {
                    files.push(wire::FileHit {
                        source,
                        rank: wire::PositiveInteger::new(files.len() as u64 + 1)
                            .map_err(|_| wire::ErrorCode::Internal)?,
                        score: evidence.score,
                        evidence: vec![evidence],
                        retained_evidence_count: wire::U64::new(1),
                        useful_location_count: wire::U64::new(0),
                    });
                }
            }
            let count = files.len() as u64;
            let coverage = wire::Coverage {
                discovered: wire::U64::new(snapshot.records.len() as u64),
                verified: wire::U64::new(count),
                pending_hash: wire::U64::new(snapshot.records.len() as u64 - count),
                pending_extraction: wire::U64::new(0),
                pending_embedding: wire::U64::new(0),
                stale: wire::U64::new(0),
                unavailable: wire::U64::new(0),
                unsupported: wire::U64::new(0),
                failed: wire::U64::new(0),
                excluded: wire::U64::new(0),
                enumeration: wire::Enumeration::Partial,
                result_extent: wire::ResultExtent::TopK,
                stopped_reason: Some(
                    "bounded retained generation; current full source inventory is not certified"
                        .into(),
                ),
            };
            Ok(wire::Result::Search {
                search: Box::new(wire::SearchResult {
                    applied: query.clone(),
                    groups: vec![wire::SearchGroup {
                        library_id: self.library.clone(),
                        generation_id: pinned
                            .directory
                            .file_name()
                            .and_then(|name| name.to_str())
                            .map(wire::Id::new)
                            .transpose()
                            .map_err(|_| wire::ErrorCode::Changed)?,
                        partitioning_id: Some(
                            wire::Id::new("native-kernel-shards-v1")
                                .map_err(|_| wire::ErrorCode::Internal)?,
                        ),
                        files,
                        coverage,
                        returned_files: wire::UnsignedInteger::new(count)
                            .map_err(|_| wire::ErrorCode::Internal)?,
                        limit_reached: count >= query.limit.get(),
                    }],
                }),
            })
        }
    }
}

/// Returns a stable SHA-256 by streaming the source without allocating its entire contents.
fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hex::encode(hash.finalize()))
}

/// Removes blank and duplicate exact tags without changing case or interior whitespace.
fn normalized_tags(tags: &[String]) -> Vec<String> {
    let mut output: Vec<String> = tags
        .iter()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .collect();
    output.sort();
    output.dedup();
    output
}

/// Returns current Unix milliseconds for snapshot evidence and unique generation prefixes.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Splits text into bounded Unicode fragments with a short overlap and original line evidence.
fn text_chunks(text: &str, maximum: usize) -> Vec<(usize, String)> {
    let characters: Vec<char> = text.chars().collect();
    let mut output = vec![];
    let mut offset = 0;
    let mut line = 1;
    while offset < characters.len() {
        let end = (offset + maximum).min(characters.len());
        output.push((line, characters[offset..end].iter().collect()));
        if end == characters.len() {
            break;
        }
        let next = end.saturating_sub(160).max(offset + 1);
        line += characters[offset..next]
            .iter()
            .filter(|&&c| c == '\n')
            .count();
        offset = next;
    }
    output
}

#[cfg(test)]
mod native_kernel_builder_tests {
    use super::*;
    use omni_service as wire;
    use std::time::Duration;
    use wire::authority::{HumanIntent, LocalPolicy, NativeIntent, TransportFacts};

    /// Unique owned native fixture; no personal source, model or configuration is read.
    fn library() -> (tempfile::TempDir, KnowledgeBase, String) {
        let directory = tempfile::tempdir().unwrap();
        let kb = KnowledgeBase::init(directory.path(), "Native kernel authority builder").unwrap();
        let word = format!("nativeproof{}", uuid::Uuid::new_v4().simple());
        fs::write(
            kb.root.join("raw/proof.txt"),
            format!("{word}\nactual native source\n"),
        )
        .unwrap();
        (directory, kb, word)
    }
    /// Real Rootlib production queries must consume the actual kernel, not a legacy lexical fallback.
    #[test]
    fn root_query_depends_on_actual_kernel_and_preserves_native_source() {
        let (_directory, kb, word) = library();
        kb.index(&IndexOptions::default()).unwrap();
        assert_eq!(
            kb.search(&SearchOptions {
                query: word.clone(),
                ..Default::default()
            })
            .unwrap()[0]
                .path,
            "raw/proof.txt"
        );
        let snapshot = kb.snapshot().unwrap();
        let corpus = admitted::corpus(&snapshot).unwrap();
        assert!(!corpus.starts_with(&kb.root));
        let scope = fs::read_dir(corpus.join("kernel"))
            .unwrap()
            .find_map(|entry| {
                let current = entry.unwrap().path().join("CURRENT");
                current.is_file().then_some(current)
            })
            .unwrap();
        let previous = fs::read(&scope).unwrap();
        fs::write(&scope, b"not a committed generation").unwrap();
        let refused = kb.search(&SearchOptions {
            query: word.clone(),
            ..Default::default()
        });
        fs::write(&scope, previous).unwrap();
        assert!(
            refused.is_err(),
            "a broken real kernel cannot fall back to the old lexical engine"
        );
        assert!(
            fs::read_to_string(kb.root.join("raw/proof.txt"))
                .unwrap()
                .contains(&word)
        );
    }
    /// Cache and detached snapshot pins retain actual memory without retaining a writer execution slot.
    #[test]
    fn root_snapshot_memory_follows_actual_reader_and_cache_lifetimes() {
        let (_directory, kb, word) = library();
        kb.index(&IndexOptions::default()).unwrap();
        kb.search(&SearchOptions {
            query: word,
            ..Default::default()
        })
        .unwrap();
        let loaded = storage::load_search(&kb).unwrap();
        let memory = loaded.snapshot._allocation.as_ref().unwrap().clone();
        assert!(std::sync::Arc::strong_count(&memory) >= 2);
        let (send, receive) = std::sync::mpsc::channel();
        let writer = kb.clone();
        let thread = std::thread::spawn(move || {
            let outcome = writer.index(&IndexOptions::default());
            send.send(outcome).unwrap();
        });
        assert!(receive.recv_timeout(Duration::from_millis(50)).is_err());
        let retained = loaded.snapshot.clone();
        drop(loaded);
        receive
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
            .unwrap();
        thread.join().unwrap();
        assert!(retained._allocation.is_some());
        assert!(
            knowledge_core::admission::Account::current()
                .unwrap()
                .status()
                .unwrap()
                .reserved
                .memory_bytes
                > 0
        );
        drop(retained);
        drop(memory);
    }
    /// Explicit fixture UI confirms the exact native exposure-read intention only.
    struct Confirm;
    impl HumanIntent for Confirm {
        fn confirm(&mut self, intent: NativeIntent<'_>) -> std::io::Result<()> {
            assert!(matches!(intent.command, wire::Command::ExposureGet { .. }));
            Ok(())
        }
    }
    /// Consume only complete versioned bytes from a still-live admitted Delivery.
    fn result(delivery: &wire::runtime::Delivery) -> serde_json::Value {
        serde_json::from_slice::<serde_json::Value>(&delivery.encode().unwrap()).unwrap()["result"]
            .clone()
    }
    /// Existing Request envelope; helpers add no alternate operation or actor/confirmation schema.
    fn frame(command: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(
            &serde_json::json!({"wire_version":1,"request_id":"native-builder","command":command}),
        )
        .unwrap()
    }
    /// The real Root caller uses M3's claim credit and complete native work facts, not pending grants.
    #[test]
    fn root_index_composes_actual_funded_claim() {
        use knowledge_core::{
            backend::Budget,
            data::ContentDigest,
            jobs::{Key, Outcome, Priority, Reservation, Submission},
        };
        let (_directory, kb, word) = library();
        fs::write(
            kb.root.join("raw/funded.html"),
            "<p>Actual funded HTML caller</p>",
        )
        .unwrap();
        fs::write(kb.root.join("raw/funded.wav"), b"RIFF-owned-fixture").unwrap();
        fs::write(
            kb.root.join("raw/funded.wav.transcript.json"),
            r#"{"segments":[{"start":0.0,"end":1.0,"text":"Actual funded transcript"}]}"#,
        )
        .unwrap();
        admitted::start().unwrap();
        let mut hash = Sha256::new();
        for path in kb.source_paths().unwrap() {
            hash.update(path.as_bytes());
            hash.update(admitted::hash_source(&kb, &path).unwrap().as_bytes());
        }
        let version = ContentDigest::new(hex::encode(hash.finalize())).unwrap();
        let recipe = ContentDigest::new(hex::encode(Sha256::digest(
            format!("{:?}", IndexOptions::default()).as_bytes(),
        )))
        .unwrap();
        let processor = ContentDigest::new(admitted::processor_identity()).unwrap();
        let key = Key {
            context: kb.root.to_str().unwrap(),
            library: kb.config.library_id.as_deref().unwrap(),
            mount: None,
            source: "complete-owned-inventory",
            version: &version,
            recipe: &recipe,
            processor: &processor,
        };
        let account = knowledge_core::admission::Account::current().unwrap();
        account
            .submit(Submission {
                operation_id: "root-funded",
                job_id: "root-funded",
                key,
                budget: Budget {
                    max_files: 1000,
                    max_bytes: 128 * 1024 * 1024,
                    max_memory_bytes: 1024 * 1024 * 1024,
                    max_millis: 60_000,
                },
                reservation: Reservation {
                    memory_bytes: 1024 * 1024 * 1024,
                    temporary_bytes: 1024 * 1024 * 1024,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let mut claim = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        let report = kb
            .index_funded(
                &IndexOptions::default(),
                &mut knowledge_processing::funding::Funding::Claim {
                    claim: &mut claim,
                    expected: key,
                },
                Some(128 * 1024 * 1024),
            )
            .unwrap();
        assert!(report.extracted > 0);
        let built = kb.snapshot().unwrap();
        assert!(
            built.records.iter().any(
                |r| r.path == "raw/funded.html" && r.text.contains("Actual funded HTML caller")
            )
        );
        assert!(built.records.iter().any(|r| r.path == "raw/funded.wav"
            && r.status == "transcript"
            && r.text.contains("Actual funded transcript")));
        drop(built);
        account
            .queue()
            .settle(&claim, key, Outcome::Completed)
            .unwrap();
        assert!(
            kb.search(&SearchOptions {
                query: word,
                ..Default::default()
            })
            .unwrap()
            .iter()
            .any(|h| h.path == "raw/proof.txt")
        );
    }
    #[test]
    fn native_source_real_claim_key_and_delivery_bridge() {
        use knowledge_core::{
            backend::Budget,
            data::ContentDigest,
            jobs::{Outcome, Priority, State},
        };
        use native_service::NativeJobPlan;
        let (_directory, kb, word) = library();
        let policy = LocalPolicy {
            mode: wire::ExposureMode::ReadOnly,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        };
        let (service, _owner, _selection) =
            native_service::NativeLibraryService::ordinary_local(kb.clone(), policy).unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        let frame = frame(
            serde_json::json!({"op":"library_status","library_id":service.library_id().as_str()}),
        );
        let command = wire::decode_request(&frame).unwrap().command;
        let permit = service.native_scope(&session, &command, None).unwrap();
        let path = wire::RelativePath::new("raw/proof.txt").unwrap();
        let budget = Budget {
            max_files: 1,
            max_bytes: 1024 * 1024,
            max_memory_bytes: 64 * 1024 * 1024,
            max_millis: 60_000,
        };
        let source = service.source(&permit, &path, budget).unwrap();
        let digest = |bytes: &[u8]| ContentDigest::new(hex::encode(Sha256::digest(bytes))).unwrap();
        assert_eq!(
            source.version(),
            &digest(&fs::read(kb.root.join("raw/proof.txt")).unwrap())
        );
        assert_eq!(source.library_id(), service.library_id());
        let processor = digest(b"actual-root-native-source-bridge-test");
        let bridge = source
            .processing(digest(b"extract:max_bytes=1048576"), processor.clone())
            .unwrap();
        assert_eq!(
            bridge.key().library,
            kb.config.library_id.as_deref().unwrap()
        );
        assert_eq!(bridge.key().context, bridge.source().context());
        let plan = |id| NativeJobPlan {
            operation_id: id,
            job_id: id,
            budget,
            reservation: knowledge_core::jobs::Reservation {
                memory_bytes: 64 * 1024 * 1024,
                temporary_bytes: 16 * 1024 * 1024,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        };
        let first = bridge.submit(plan("native-source-first")).unwrap();
        let account = knowledge_core::admission::Account::current().unwrap();
        let mut claim = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        assert_eq!(claim.job(), first.job);
        let second = service
            .source(&permit, &path, budget)
            .unwrap()
            .processing(digest(b"extract:max_bytes=524288"), processor)
            .unwrap();
        let queued = second.submit(plan("native-source-second")).unwrap();
        assert!(queued.created);
        assert_eq!(
            account
                .job_status(queued.job, second.key())
                .unwrap()
                .queue
                .state,
            State::Queued
        );
        let output = knowledge_processing::extract::extract_file_funded(
            bridge.source().file().unwrap(),
            bridge.source().hint(),
            1024 * 1024,
            Some(16 * 1024 * 1024),
            &mut bridge.funding(&mut claim).unwrap(),
        )
        .unwrap();
        assert!(output.value.text.contains(&word));
        let knowledge_processing::admission::Admitted {
            value,
            mut retained,
        } = output;
        drop(value); // text AND format/status/warnings precede every finish/rejoin.
        for held in &mut retained {
            account.rejoin_claim(held, &claim, bridge.key()).unwrap();
        }
        bridge.check(&claim).unwrap();
        account
            .queue()
            .settle(&claim, bridge.key(), Outcome::Completed)
            .unwrap();
        assert!(bridge.funding(&mut claim).is_err());
        let mut cancelled = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        assert_eq!(cancelled.job(), queued.job);
        account
            .queue()
            .cancel(cancelled.job(), second.key())
            .unwrap();
        assert!(
            second
                .funding(&mut cancelled)
                .err()
                .unwrap()
                .downcast_ref::<FundedIndexStopped>()
                .is_some()
        );
        account
            .queue()
            .settle(&cancelled, second.key(), Outcome::Cancelled)
            .unwrap();
        drop(second);
        let response_owner = service.dispatch(&frame, &session).unwrap();
        let encoded = response_owner.encode().unwrap();
        let source = bridge.into_source();
        source
            .reserve_result(encoded.len() as u64 * 24 + 4096)
            .unwrap();
        let response = serde_json::from_slice(&encoded).unwrap();
        drop(response_owner);
        let delivery = source.into_delivery(response).unwrap();
        delivery.encode().unwrap();
        fs::write(
            kb.root.join("raw/proof.txt"),
            "changed after native FD/queue work",
        )
        .unwrap();
        assert!(delivery.check().is_err());
        assert_eq!(result(&delivery)["error"]["code"], "not_exposed");
    }
    /// Real M3 HTML -> source/processor-bound M4 rows -> immutable unpublished components.
    /// Precomputed numeric values test the IO contract, not a fake provider or model quality.
    #[test]
    fn actual_m3_text_binds_index_representation_before_prepared_vector_consumption() {
        // Keep the shared process queue's selected-Claim assertions independent of
        // unrelated native builder tests; preserve all within-case real queue controls.
        if std::env::var_os("OMNI_NATIVE_INDEX_INPUT_CHILD").is_none() {
            let output=std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact","native_kernel_builder_tests::actual_m3_text_binds_index_representation_before_prepared_vector_consumption","--nocapture"])
                .env("OMNI_NATIVE_INDEX_INPUT_CHILD","1").output().unwrap();
            assert!(
                output.status.success(),
                "isolated actual native pipeline: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        use knowledge_core::{
            backend::Budget,
            data::ContentDigest,
            jobs::{Priority, Reservation},
        };
        use knowledge_search::{
            AccountGrant, ModelIdentity, VectorMetric, VectorNormalization, VectorRow, VectorStore,
        };
        use std::os::unix::fs::PermissionsExt;
        let (_directory, kb, word) = library();
        let html = format!("<h1>{word}</h1><p>Native 日本語 provenance</p>");
        fs::write(kb.root.join("raw/indexed.html"), &html).unwrap();
        let policy = LocalPolicy {
            mode: wire::ExposureMode::ReadOnly,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        };
        let (service, owner, selection) =
            native_service::NativeLibraryService::ordinary_local(kb, policy.clone()).unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        let browse = frame(
            serde_json::json!({"op":"browse","library_id":service.library_id().as_str(),"mount_id":null,"path":"raw","cursor":null,"limit":100}),
        );
        let listing = service.dispatch(&browse, &session).unwrap();
        let listed = result(&listing);
        let source_id = listed["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == "indexed.html")
            .unwrap()["source"]["source_id"]
            .as_str()
            .unwrap()
            .to_owned();
        drop(listed);
        drop(listing);
        let process = frame(
            serde_json::json!({"op":"process","library_id":service.library_id().as_str(),"source_ids":[source_id],"recipe":"extract",
            "budget":{"max_files":1,"max_bytes":"65536","max_millis":60000,"max_memory_bytes":"134217728"},"operation_id":"native-index-input"}),
        );
        let command = wire::decode_request(&process).unwrap().command;
        assert!(
            matches!(
                service.native_scope(&session, &command, None),
                Err(wire::ErrorCode::ReadOnly)
            ),
            "actual Process must not borrow a fabricated read-only status permit"
        );
        // Explicit controlled native OWNER policy selection, never a request flag/transport grant.
        owner
            .set_policy(
                &selection,
                LocalPolicy {
                    mode: wire::ExposureMode::ReadWrite,
                    ..policy
                },
            )
            .unwrap();
        let permit = service.native_scope(&session, &command, None).unwrap();
        let clone = permit.clone();
        assert!(
            std::ptr::eq(
                permit.admitted_command().unwrap(),
                clone.admitted_command().unwrap()
            ),
            "permit clones share the one paid admitted command"
        );
        drop(clone);
        let input_memory = AccountGrant::reserve(
            Reservation {
                memory_bytes: 16 * 1024 * 1024,
                temporary_bytes: 0,
            },
            knowledge_core::admission::Origin::BeforeVerification,
        )
        .unwrap();
        let mut oversized = command.clone();
        if let wire::Command::Process { source_ids, .. } = &mut oversized {
            let id = source_ids[0].clone();
            *source_ids = vec![id; 100_000];
        }
        assert!(
            matches!(
                service.native_scope(&session, &oversized, None),
                Err(wire::ErrorCode::InvalidRequest)
            ),
            "native command size must be bounded before storing its clone"
        );
        drop(oversized);
        drop(input_memory);
        let budget = Budget {
            max_files: 1,
            max_bytes: 65536,
            max_memory_bytes: 128 * 1024 * 1024,
            max_millis: 60000,
        };
        let digest = |bytes: &[u8]| ContentDigest::new(hex::encode(Sha256::digest(bytes))).unwrap();
        {
            let status=wire::decode_request(&frame(serde_json::json!({"op":"library_status","library_id":service.library_id().as_str()}))).unwrap().command;
            let wrong_permit = service.native_scope(&session, &status, None).unwrap();
            let wrong = service
                .source(
                    &wrong_permit,
                    &wire::RelativePath::new("raw/indexed.html").unwrap(),
                    budget,
                )
                .unwrap()
                .processing(
                    digest(b"wrong-index-status-scope"),
                    digest(b"native-m3-m4-consumer"),
                )
                .unwrap();
            let account = knowledge_core::admission::Account::current().unwrap();
            let submitted = wrong
                .submit(native_service::NativeJobPlan {
                    operation_id: "wrong-index-scope",
                    job_id: "wrong-index-scope",
                    budget,
                    reservation: Reservation {
                        memory_bytes: 128 * 1024 * 1024,
                        temporary_bytes: 0,
                    },
                    priority: Priority::Normal,
                    max_attempts: 1,
                })
                .unwrap();
            let claim = account
                .queue()
                .claim(std::time::Instant::now())
                .unwrap()
                .unwrap();
            assert_eq!(claim.job(), submitted.job);
            assert!(
                wrong.extract_indexed(claim, 65536, None).is_err(),
                "a status Permit is not the actual Process command/selected source intention"
            );
        }
        let processing = service
            .source(
                &permit,
                &wire::RelativePath::new("raw/indexed.html").unwrap(),
                budget,
            )
            .unwrap()
            .processing(
                digest(b"actual-index-html-65536"),
                digest(b"native-m3-m4-consumer"),
            )
            .unwrap();
        let expected_processor = processing.key().processor.clone();
        let expected_source = processing.key().source.to_owned();
        let account = knowledge_core::admission::Account::current().unwrap();
        let submitted = processing
            .submit(native_service::NativeJobPlan {
                operation_id: "native-index-input",
                job_id: "native-index-input",
                budget,
                reservation: Reservation {
                    memory_bytes: 128 * 1024 * 1024,
                    temporary_bytes: 2 * 1024 * 1024,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        assert_eq!(claim.job(), submitted.job);
        // Split the consumer sibling from the SAME complete Claim BEFORE M3 can spend remaining RSS.
        let work = knowledge_retrieval::Work::split_claim(
            &claim,
            processing.key(),
            Reservation {
                memory_bytes: 2 * 1024 * 1024,
                temporary_bytes: 2 * 1024 * 1024,
            },
        )
        .unwrap();
        let input = processing.extract_indexed(claim, 65536, None).unwrap();
        let mut marker_pressure = match account
            .reserve(
                Reservation {
                    memory_bytes: account.status().unwrap().memory_available,
                    temporary_bytes: 0,
                },
                knowledge_core::admission::Origin::BeforeVerification,
            )
            .unwrap()
        {
            knowledge_core::admission::Decision::Ready(holding) => holding,
            _ => panic!("marker pressure admission"),
        };
        assert!(
            input.text().is_ok(),
            "actual currentness must use its prepaid context, not late global credit or false revocation"
        );
        marker_pressure.finish().unwrap();
        assert!(input.text().unwrap().contains(&word));
        assert_eq!(input.source_id().unwrap(), expected_source);
        assert_eq!(
            input.source_digest().unwrap(),
            digest(html.as_bytes()).as_str()
        );
        assert_eq!(
            input.representation().unwrap().digest,
            hex::encode(Sha256::digest(input.text().unwrap().as_bytes()))
        );
        assert_eq!(
            input.representation().unwrap().processor,
            expected_processor.as_str()
        );
        let model = ModelIdentity {
            provider: "precomputed-control".into(),
            model: "numeric-f32".into(),
            revision: "fixture-v1".into(),
            dimension: 2,
            metric: VectorMetric::Cosine,
            normalization: VectorNormalization::Raw,
            package_fingerprint: "cc".repeat(32),
            embedding_space: "numeric-only".into(),
            preprocessing: "fixture-input".into(),
        };
        let rows = [VectorRow {
            source_id: input.source_id().unwrap().into(),
            source_digest: input.source_digest().unwrap().into(),
            representation: input.representation().unwrap().clone(),
            locator: "extracted-line:1".into(),
            vector: vec![1.0, 0.0],
        }];
        let output = tempfile::tempdir().unwrap();
        fs::set_permissions(output.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let prepared = std::sync::Arc::new(
            VectorStore::prepare(
                output.path(),
                &model,
                &rows,
                &AccountGrant::from_work(work),
                None,
            )
            .unwrap(),
        );
        assert!(
            !output.path().join("CURRENT").exists(),
            "no partial/independent tuple selection"
        );
        let mut member = prepared.component("rows.bin").unwrap();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut member, &mut bytes).unwrap();
        let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let row: serde_json::Value = serde_json::from_slice(&bytes[4..4 + length]).unwrap();
        assert_eq!(row["source_digest"], input.source_digest().unwrap());
        assert_eq!(
            row["representation"]["processor"],
            expected_processor.as_str()
        );
        assert_eq!(
            row["representation"]["digest"],
            input.representation().unwrap().digest
        );
        drop(row);
        drop(bytes);
        drop(member);
        drop(prepared);
        drop(rows);
        drop(model);
        let source = input.complete().unwrap();
        source.check().unwrap();
        assert_eq!(account.status().unwrap().queue.unsettled, 0);
    }

    /// Actual funded opaque extraction retains its source/Claim/paid value and returns credit only after the last reader.
    #[test]
    fn native_extraction_owns_complete_claim_and_paid_result_lifetime() {
        use knowledge_core::{
            backend::Budget,
            data::ContentDigest,
            jobs::{Outcome, Priority, State},
        };
        use native_service::NativeJobPlan;
        let (_directory, kb, word) = library();
        fs::write(kb.root.join("raw/owned.html"), format!("<h1>{word}</h1>")).unwrap();
        fs::write(kb.root.join("raw/owned2.html"), format!("<h1>{word}</h1>")).unwrap();
        let policy = LocalPolicy {
            mode: wire::ExposureMode::ReadOnly,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        };
        let (service, _owner, _selection) =
            native_service::NativeLibraryService::ordinary_local(kb, policy).unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        let frame = frame(
            serde_json::json!({"op":"library_status","library_id":service.library_id().as_str()}),
        );
        let command = wire::decode_request(&frame).unwrap().command;
        let permit = service.native_scope(&session, &command, None).unwrap();
        let budget = Budget {
            max_files: 1,
            max_bytes: 65536,
            max_memory_bytes: 64 * 1024 * 1024,
            max_millis: 60000,
        };
        let path = wire::RelativePath::new("raw/owned.html").unwrap();
        let digest = |bytes: &[u8]| ContentDigest::new(hex::encode(Sha256::digest(bytes))).unwrap();
        let processing = service
            .source(&permit, &path, budget)
            .unwrap()
            .processing(
                digest(b"html:65536"),
                digest(b"native-owned-extraction-test"),
            )
            .unwrap();
        let account = knowledge_core::admission::Account::current().unwrap();
        let before = account.status().unwrap().memory_available;
        let submitted = processing
            .submit(NativeJobPlan {
                operation_id: "native-owned-extract",
                job_id: "native-owned-extract",
                budget,
                reservation: knowledge_core::jobs::Reservation {
                    memory_bytes: 64 * 1024 * 1024,
                    temporary_bytes: 0,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        assert_eq!(claim.job(), submitted.job);
        // None is bounded by THIS claim's complete credit, not the account's unrelated free memory.
        let extracted = processing.extract(claim, 65536, None).unwrap();
        assert!(extracted.value().unwrap().text.contains(&word));
        assert!(account.status().unwrap().memory_available < before);
        let source = extracted.complete().unwrap();
        assert_eq!(account.status().unwrap().memory_available, before);
        let key_owner = source
            .processing(
                digest(b"html:65536"),
                digest(b"native-owned-extraction-test"),
            )
            .unwrap();
        assert_eq!(
            account
                .job_status(submitted.job, key_owner.key())
                .unwrap()
                .queue
                .state,
            State::Finished(Outcome::Completed)
        );
        let path2 = wire::RelativePath::new("raw/owned2.html").unwrap();
        let second = service
            .source(&permit, &path2, budget)
            .unwrap()
            .processing(
                digest(b"html:65536"),
                digest(b"native-owned-extraction-test"),
            )
            .unwrap();
        let queued = second
            .submit(NativeJobPlan {
                operation_id: "native-owned-extract2",
                job_id: "native-owned-extract2",
                budget,
                reservation: knowledge_core::jobs::Reservation {
                    memory_bytes: 64 * 1024 * 1024,
                    temporary_bytes: 0,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = account
            .queue()
            .claim(std::time::Instant::now())
            .unwrap()
            .unwrap();
        assert_eq!(claim.job(), queued.job);
        // A scheduler-selected Claim for another source is returned intact, never discarded or force-settled.
        let error = key_owner.extract(claim, 65536, None).err().unwrap();
        let (_wrong_source, claim, _cause) = error.into_parts();
        assert_eq!(claim.job(), queued.job);
        let extracted = second.extract(claim, 65536, None).unwrap();
        assert!(extracted.value().unwrap().text.contains(&word));
        let expected = service
            .source(&permit, &path2, budget)
            .unwrap()
            .processing(
                digest(b"html:65536"),
                digest(b"native-owned-extraction-test"),
            )
            .unwrap();
        account.queue().cancel(queued.job, expected.key()).unwrap();
        assert!(extracted.value().is_err());
        drop(extracted);
        assert_eq!(
            account
                .job_status(queued.job, expected.key())
                .unwrap()
                .queue
                .state,
            State::Finished(Outcome::Cancelled)
        );
    }
    /// Namespace drift after resolution cannot expose a replacement root, even if the old FD lives.
    #[test]
    fn native_selected_root_drift_refuses_before_names_and_at_delivery() {
        use wire::runtime::{Domain, DomainHost, ResolvedScope};
        struct Drift {
            root: PathBuf,
            moved: PathBuf,
        }
        impl DomainHost for Drift {
            fn resolve(
                &mut self,
                _: &wire::Command,
                _: &wire::authority::Permit,
            ) -> std::result::Result<ResolvedScope, wire::ErrorCode> {
                fs::rename(&self.root, &self.moved).unwrap();
                fs::create_dir(&self.root).unwrap();
                fs::write(self.root.join("replacement-only.txt"), "not selected").unwrap();
                Err(wire::ErrorCode::Unsupported)
            }
            fn execute(
                &mut self,
                _: Domain,
                _: &wire::Command,
                _: &wire::authority::Permit,
            ) -> std::result::Result<wire::Result, wire::ErrorCode> {
                panic!("drift is refused before execution")
            }
        }
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("selected");
        let kb = KnowledgeBase::init(&root, "Selected descriptor").unwrap();
        let policy = LocalPolicy {
            mode: wire::ExposureMode::ReadOnly,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        };
        let (service, _, _) =
            native_service::NativeLibraryService::ordinary_local(kb, policy).unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        let browse = frame(
            serde_json::json!({"op":"browse","library_id":service.library_id(),"mount_id":null,"path":"","cursor":null,"limit":100}),
        );
        let pending = service.dispatch(&browse, &session).unwrap();
        assert_eq!(result(&pending)["type"], "listing");
        service
            .bind_domains(Box::new(Drift {
                root: root.clone(),
                moved: parent.path().join("moved"),
            }))
            .unwrap();
        let refused = service.dispatch(&browse, &session).unwrap();
        assert_eq!(result(&refused)["error"]["code"], "not_exposed");
        assert_eq!(result(&pending)["error"]["code"], "not_exposed");
        assert!(
            !String::from_utf8(pending.encode().unwrap())
                .unwrap()
                .contains("replacement-only")
        );
    }
    /// Actual multi-keyword/method buffers survive all shard/merge work and remain charged to Delivery.
    #[test]
    fn native_aggregate_result_charge_follows_delivery() {
        // These are process-account deltas, not per-owner readings. Execute the
        // same assertions in an isolated process, without racing unrelated tests'
        // real releases in the shared account (the normal gate uses two threads).
        const CHILD: &str = "OMNI_AGGREGATE_CHARGE_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "native_kernel_builder_tests::native_aggregate_result_charge_follows_delivery",
                    "--test-threads=1",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "actual charge/Delivery child failed: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let (_directory, kb, word) = library();
        let original = kb.root.join("raw/proof.txt");
        kb.index(&IndexOptions::default()).unwrap();
        let policy = LocalPolicy {
            mode: wire::ExposureMode::ReadOnly,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        };
        let (service, _, _) =
            native_service::NativeLibraryService::ordinary_local(kb, policy).unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        let query = frame(
            serde_json::json!({"op":"query","query":{"library_ids":[service.library_id()],"keywords":[word,word.to_uppercase()],"methods":["bm25","literal"],"required_tags":[],"case_sensitive":false,"limit":10,"budget":{"max_files":1000,"max_bytes":"134217728","max_millis":30000,"max_memory_bytes":"536870912"},"image_query":null}}),
        );
        let account = knowledge_core::admission::Account::current().unwrap();
        let before = account.status().unwrap().memory_available;
        let delivery = service.dispatch(&query, &session).unwrap();
        assert_eq!(result(&delivery)["type"], "search");
        let during = account.status().unwrap().memory_available;
        assert!(
            before - during > 64 * 1024 * 1024,
            "more than a fixed frame: aggregates and snapshot are retained"
        );
        delivery.encode().unwrap();
        assert_eq!(
            account.status().unwrap().memory_available,
            during,
            "encoding does not settle output"
        );
        fs::write(original, "a newly changed native source version\n").unwrap();
        assert!(
            delivery.check().is_err(),
            "encoded evidence is not a current-version grant"
        );
        assert_eq!(result(&delivery)["error"]["code"], "not_exposed");
        drop(delivery);
        assert!(account.status().unwrap().memory_available > during + 64 * 1024 * 1024);
    }
    /// Exercise real native browse/verify/range/current-version, agent denial, approval and revocation.
    #[test]
    fn actual_native_domain_roundtrip_and_revocation() {
        let (_directory, kb, _word) = library();
        let policy = LocalPolicy {
            mode: wire::ExposureMode::ReadWrite,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        };
        let (service, owner, selection) =
            native_service::NativeLibraryService::ordinary_local(kb.clone(), policy.clone())
                .unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        let library = service.library_id().as_str();
        let browse = frame(
            serde_json::json!({"op":"browse","library_id":library,"mount_id":null,"path":"raw","cursor":null,"limit":1000}),
        );
        let listing = service.dispatch(&browse, &session).unwrap();
        let observation = result(&listing);
        let source = observation["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["path"] == "raw/proof.txt")
            .unwrap()["source"]
            .clone();
        assert_eq!(source["digest_state"], "unverified");
        let source_ref = serde_json::json!({"library_id":library,"source_id":source["source_id"]});
        let verify = frame(
            serde_json::json!({"op":"verify_source","source":source_ref,"budget":{"max_files":1,"max_bytes":"1048576","max_millis":30000,"max_memory_bytes":"1048576"}}),
        );
        let verified = service.dispatch(&verify, &session).unwrap();
        let verified = result(&verified);
        assert_eq!(verified["source"]["digest_state"], "verified");
        let read = frame(
            serde_json::json!({"op":"read_range","source":source_ref,"expected":verified["source"]["version"],"offset":"0","length":512}),
        );
        let bytes = service.dispatch(&read, &session).unwrap();
        assert_eq!(result(&bytes)["type"], "content");
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(result(&bytes)["bytes_base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, fs::read(kb.root.join("raw/proof.txt")).unwrap());
        fs::write(kb.root.join("raw/proof.txt"), b"changed native bytes").unwrap();
        assert_eq!(
            result(&service.dispatch(&read, &session).unwrap())["error"]["code"],
            "changed"
        );
        let exposure = frame(serde_json::json!({"op":"exposure_get","library_id":library}));
        assert_eq!(
            result(&service.dispatch(&exposure, &session).unwrap())["error"]["code"],
            "owner_required"
        );
        let approval = owner
            .approve(
                &session,
                &wire::decode_request(&exposure).unwrap().command,
                &mut Confirm,
            )
            .unwrap();
        assert_eq!(
            result(
                &service
                    .dispatch_approved(&exposure, &session, approval)
                    .unwrap()
            )["type"],
            "exposure"
        );
        let mut reduced = policy;
        reduced
            .excluded_paths
            .push(wire::RelativePath::new("raw/proof.txt").unwrap());
        owner.set_policy(&selection, reduced).unwrap();
        assert_eq!(result(&listing)["error"]["code"], "not_exposed");
        assert_eq!(
            result(&service.dispatch(&read, &session).unwrap())["error"]["code"],
            "not_exposed"
        );
        owner.revoke(&selection).unwrap();
        assert_eq!(
            result(&service.dispatch(&browse, &session).unwrap())["error"]["code"],
            "not_exposed"
        );
    }
    /// Native query/status use actual Rootlib/kernel outputs and the SAME installed process budgets.
    #[test]
    fn native_query_and_policy_budget_status_are_actual() {
        let (_directory, kb, word) = library();
        kb.index(&IndexOptions::default()).unwrap();
        let policy = LocalPolicy {
            mode: wire::ExposureMode::ReadOnly,
            sensitive: false,
            excluded_paths: vec![],
            transports: vec![wire::Transport::OwnerStdio],
            http_hosts: vec![],
            http_origins: vec![],
        };
        let (service, _owner, _selection) =
            native_service::NativeLibraryService::ordinary_local(kb, policy).unwrap();
        let session = service.connect(TransportFacts::OwnerStdio).unwrap();
        let library = service.library_id().as_str();
        let query = frame(
            serde_json::json!({"op":"query","query":{"library_ids":[library],"keywords":[word],"methods":["bm25"],"required_tags":[],"case_sensitive":false,"limit":10,"budget":{"max_files":1000,"max_bytes":"33554432","max_millis":30000,"max_memory_bytes":"536870912"},"image_query":null}}),
        );
        let searched = service.dispatch(&query, &session).unwrap();
        let searched = result(&searched);
        assert_eq!(searched["type"], "search", "{searched}");
        assert_eq!(
            searched["search"]["groups"][0]["files"][0]["source"]["path"],
            "raw/proof.txt"
        );
        assert_eq!(
            searched["search"]["groups"][0]["coverage"]["enumeration"],
            "partial"
        );
        let status = frame(serde_json::json!({"op":"library_status","library_id":library}));
        let status = service.dispatch(&status, &session).unwrap();
        let status = result(&status);
        assert_eq!(status["type"], "status", "{status}");
        assert_eq!(
            status["status"]["resource_budgets"]["memory_budget_bytes"],
            knowledge_core::admission::Account::current()
                .unwrap()
                .policy()
                .memory_bytes
                .to_string()
        );
        assert!(status["status"]["resource_budgets"]["waiting_for_memory"].is_null());
    }
}
