//! Nonmutating workspace checks for folders, index freshness, and local dependencies.
//! Freshness compares metadata, not source hashes; incomplete checks never report a current index.

use crate::embedding::EmbeddingClient;
use crate::runtime::{RuntimeStatus, detect_runtime};
use crate::{CONFIG_FILE, Config, KnowledgeBase, now_ms, storage};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Maximum metadata scan duration; filesystem permission prompts can still suspend OS calls.
const SCAN_BUDGET: Duration = Duration::from_secs(2);
/// Upper bound on entries inspected during one background check.
const MAX_ENTRIES: usize = 100_000;
/// Maximum manifest input; source contents and vectors are not allocated by this projection.
const MAX_MANIFEST_BYTES: u64 = 512 * 1024 * 1024;
/// Configuration size accepted by the automatic probe, independent of explicit core operations.
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// Indicates incomplete observation rather than a corrupt or absent index.
#[derive(Debug)]
struct ScanIncomplete(&'static str);

impl std::fmt::Display for ScanIncomplete {
    /// Explains the observation budget that prevented a complete result.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for ScanIncomplete {}

/// Classification of the entered folder and its metadata-based index check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderState {
    /// No folder has been entered.
    NotSelected,
    /// The entered location does not exist.
    Missing,
    /// The entered location exists but is not a directory.
    NotDirectory,
    /// No knowledge marker exists in this directory or its ancestors.
    Uninitialized,
    /// A knowledge root exists but its derived index is absent or invalid.
    NeedsIndex,
    /// Names, sizes, and timestamps match the published index; hashes were not checked.
    Current,
    /// Source, metadata, or configuration changes require indexing.
    Stale,
    /// A reader gate or scan budget prevented a complete check.
    Checking,
    /// Folder or configuration inspection failed.
    Unavailable,
}

/// Folder identity, observed changes, and an explanation suitable for a hover hint.
#[derive(Clone, Debug)]
pub struct FolderStatus {
    /// Entered location, retained to reject callbacks for an obsolete selection.
    pub requested: PathBuf,
    /// Canonical nearest knowledge root, when one exists.
    pub root: Option<PathBuf>,
    /// Current folder/index classification.
    pub state: FolderState,
    /// Exact observation or failure; never asserts content-hash verification.
    pub detail: String,
    /// Newly admitted source paths.
    pub added: usize,
    /// Sources with changed size or modification time.
    pub modified: usize,
    /// Indexed source paths no longer owned by this root.
    pub removed: usize,
    /// Editable metadata or root configuration changed after publication.
    pub metadata_changed: bool,
    /// Published index timestamp, when readable.
    pub indexed_at: Option<u64>,
}

/// Availability of an explicitly configured tool or service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    /// Required explicit configuration is absent.
    NotConfigured,
    /// The documented availability observation succeeded.
    Available,
    /// The check failed or a configured file is absent.
    Unavailable,
}

/// Service observation with its scope and diagnostic.
#[derive(Clone, Debug)]
pub struct ServiceStatus {
    /// Availability classification.
    pub state: ServiceState,
    /// Provider identity result or configured-file observation, not inferred inference quality.
    pub detail: String,
}

/// One completed, nonmutating background status observation.
#[derive(Clone, Debug)]
pub struct StatusReport {
    /// Entered folder and index freshness.
    pub folder: FolderStatus,
    /// Executable locations discovered without running installers.
    pub tools: RuntimeStatus,
    /// Validated local embedding health; no document content is sent.
    pub provider: ServiceStatus,
    /// ASR executable/model availability; recognition is not run.
    pub asr: ServiceStatus,
    /// Completion time in Unix milliseconds.
    pub checked_at: u64,
}

/// Reusable probe that caches only the lightweight projection of an unchanged manifest.
#[derive(Clone, Default)]
pub struct StatusProbe {
    cache: Arc<Mutex<Option<CachedIndex>>>,
}

/// Manifest identity and metadata-only source records, excluding text and vector allocations.
#[derive(Clone)]
struct CachedIndex {
    path: PathBuf,
    length: u64,
    modified: std::time::SystemTime,
    summary: IndexSummary,
}

/// Derived index projection for a status check, not a substitute for search validation.
#[derive(Clone, Deserialize)]
struct IndexSummary {
    version: u32,
    root: PathBuf,
    #[serde(default)]
    library_id: Option<String>,
    indexed_at: u64,
    records: Vec<SourceStamp>,
}

/// Source identity and filesystem stamp retained by an index generation.
#[derive(Clone, Deserialize)]
struct SourceStamp {
    path: String,
    size: u64,
    modified: u64,
}

impl StatusProbe {
    /// Checks a selected folder and dependencies without initialization, indexing, or installation.
    /// Network checks use only validated loopback health endpoints with two-second request deadlines.
    pub fn inspect(&self, path: &Path) -> StatusReport {
        let mut folder = FolderStatus {
            requested: path.into(),
            root: None,
            state: FolderState::NotSelected,
            detail: "Choose a folder in Library".into(),
            added: 0,
            modified: 0,
            removed: 0,
            metadata_changed: false,
            indexed_at: None,
        };
        let configuration = match self.inspect_folder(path, &mut folder) {
            Ok(config) => config,
            Err(error) => {
                folder.state = FolderState::Unavailable;
                folder.detail = format!("{error:#}");
                None
            }
        };
        let (provider, asr) = if configuration.is_none() && folder.state == FolderState::Unavailable
        {
            let unavailable = ServiceStatus { state: ServiceState::Unavailable, detail: "Knowledge configuration could not be inspected; service configuration is unknown".into() };
            (unavailable.clone(), unavailable)
        } else {
            (
                provider_status(configuration.as_ref()),
                asr_status(configuration.as_ref()),
            )
        };
        StatusReport {
            folder,
            tools: detect_runtime(),
            provider,
            asr,
            checked_at: now_ms(),
        }
    }

    /// Finds the nearest marker and checks one published generation under a nonblocking reader gate.
    fn inspect_folder(&self, path: &Path, status: &mut FolderStatus) -> Result<Option<Config>> {
        if path.as_os_str().is_empty() {
            return Ok(None);
        }
        let directory = match fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => path.canonicalize()?,
            Ok(_) => {
                status.state = FolderState::NotDirectory;
                status.detail = "The selected location is not a folder".into();
                return Ok(None);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                status.state = FolderState::Missing;
                status.detail =
                    "The selected folder does not exist; choose another location".into();
                return Ok(None);
            }
            Err(error) => return Err(error).context("Cannot inspect selected folder"),
        };
        let mut ancestor = directory.clone();
        loop {
            match fs::symlink_metadata(ancestor.join(CONFIG_FILE)) {
                Ok(metadata) => {
                    ensure!(
                        metadata.is_file() && !metadata.file_type().is_symlink(),
                        "Knowledge marker must be a regular file"
                    );
                    ensure!(
                        metadata.len() <= MAX_CONFIG_BYTES,
                        "Knowledge marker exceeds the automatic status-check limit"
                    );
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("Cannot inspect knowledge marker"),
            }
            if !ancestor.pop() {
                status.state = FolderState::Uninitialized;
                status.detail =
                    "This folder is not a knowledge base; use Library → Initialize".into();
                return Ok(None);
            }
        }
        let kb = KnowledgeBase::open(&directory)?;
        status.root = Some(kb.root.clone());
        let config = kb.config.clone();
        if let Err(error) = self.inspect_index(&kb, status) {
            status.state = FolderState::Unavailable;
            status.detail = format!("Index inspection unavailable: {error:#}");
        }
        Ok(Some(config))
    }

    /// Inspects derived state without discarding a readable configuration when index checks fail.
    fn inspect_index(&self, kb: &KnowledgeBase, status: &mut FolderStatus) -> Result<()> {
        storage::check_components(&kb.root, Path::new(".omni/read.lock"), false)?;
        if let Ok(metadata) = fs::symlink_metadata(kb.root.join(".omni/read.lock")) {
            ensure!(
                metadata.is_file(),
                "Index reader gate is not a regular file"
            );
        }
        let gate = match File::open(kb.root.join(".omni/read.lock")) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                status.state = FolderState::NeedsIndex;
                status.detail =
                    "Knowledge base initialized; create its index with Refresh index".into();
                return Ok(());
            }
            Err(error) => return Err(error).context("Cannot inspect index reader gate"),
        };
        if let Err(error) = FileExt::try_lock_shared(&gate) {
            if error.kind() != std::io::ErrorKind::WouldBlock {
                return Err(error).context("Cannot acquire status reader gate");
            }
            status.state = FolderState::Checking;
            status.detail = "Index publication is active; the next check will retry".into();
            return Ok(());
        }
        let start = Instant::now();
        let summary = match self.index_summary(kb, start) {
            Ok(summary) => summary,
            Err(error) => {
                status.state = if incomplete_error(&error) {
                    FolderState::Checking
                } else {
                    FolderState::NeedsIndex
                };
                status.detail = format!("Index check: {error:#}");
                return Ok(());
            }
        };
        status.indexed_at = Some(summary.indexed_at);
        let check = compare_sources(kb, &summary, status, start);
        if let Err(error) = check {
            status.state =
                if status.added + status.modified + status.removed > 0 || status.metadata_changed {
                    FolderState::Stale
                } else {
                    FolderState::Checking
                };
            status.detail = format!("Freshness check incomplete: {error:#}");
        }
        Ok(())
    }

    /// Reads a confined manifest only when its generation or filesystem stamp changes.
    fn index_summary(&self, kb: &KnowledgeBase, start: Instant) -> Result<IndexSummary> {
        storage::check_components(&kb.root, Path::new(".omni/current.json"), true)?;
        let pointer = kb.root.join(".omni/current.json");
        ensure!(
            pointer.metadata()?.is_file() && pointer.metadata()?.len() <= 4096,
            "Invalid index pointer file"
        );
        let generation: String = serde_json::from_reader(File::open(pointer)?)?;
        ensure!(
            generation.starts_with("gen-")
                && generation
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
            "Invalid generation pointer"
        );
        let relative = PathBuf::from(".omni/indexes").join(generation);
        storage::check_components(&kb.root, &relative.join("manifest.json"), true)?;
        storage::check_components(&kb.root, &relative.join("tantivy"), true)?;
        ensure!(
            kb.root.join(relative.join("tantivy")).is_dir(),
            "Missing lexical index directory"
        );
        validate_lexical_metadata(kb, &relative.join("tantivy"), start)?;
        let path = kb.root.join(relative.join("manifest.json"));
        let metadata = path.metadata()?;
        ensure!(metadata.is_file(), "Index manifest is not a regular file");
        if metadata.len() > MAX_MANIFEST_BYTES {
            return Err(
                ScanIncomplete("Index exceeds the automatic status-check size limit").into(),
            );
        }
        let modified = metadata.modified()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| anyhow::anyhow!("Status cache unavailable"))?;
        if let Some(cached) = &*cache
            && cached.path == path
            && cached.length == metadata.len()
            && cached.modified == modified
        {
            ensure!(
                storage::matches_identity(
                    kb,
                    &cached.summary.root,
                    cached.summary.library_id.as_deref()
                )?,
                "Index identity mismatch"
            );
            return Ok(cached.summary.clone());
        }
        let summary: IndexSummary = serde_json::from_reader(BufReader::new(BudgetReader {
            reader: File::open(&path)?,
            start,
        }))?;
        ensure!(
            summary.version == 1
                && storage::matches_identity(kb, &summary.root, summary.library_id.as_deref())?,
            "Index identity mismatch"
        );
        if summary.records.len() > MAX_ENTRIES {
            return Err(
                ScanIncomplete("Index exceeds the automatic source-count check limit").into(),
            );
        }
        for record in &summary.records {
            storage::validate_relative(Path::new(&record.path))?;
            ensure!(
                !record.path.starts_with(".omni/") && !record.path.starts_with(".git/"),
                "Index contains an internal path"
            );
        }
        *cache = Some(CachedIndex {
            path,
            length: metadata.len(),
            modified,
            summary: summary.clone(),
        });
        Ok(summary)
    }
}

/// Recognizes explicit scan limits and deadlines, including I/O errors wrapped by the JSON parser.
fn incomplete_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.is::<ScanIncomplete>()
            || cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::TimedOut)
            || cause
                .downcast_ref::<serde_json::Error>()
                .is_some_and(|error| error.io_error_kind() == Some(std::io::ErrorKind::TimedOut))
    })
}

/// Parses lexical metadata in memory and checks referenced core segment files without starting a reader or writer.
fn validate_lexical_metadata(kb: &KnowledgeBase, relative: &Path, start: Instant) -> Result<()> {
    use tantivy::directory::{Directory, RamDirectory};
    storage::check_components(&kb.root, &relative.join("meta.json"), true)?;
    let path = kb.root.join(relative.join("meta.json"));
    let metadata = fs::metadata(&path)?;
    ensure!(metadata.is_file(), "Lexical metadata is not a regular file");
    if metadata.len() > 2 * 1024 * 1024 {
        return Err(ScanIncomplete("Lexical metadata exceeds the automatic check limit").into());
    }
    let mut bytes = vec![];
    BudgetReader {
        reader: File::open(path)?.take(2 * 1024 * 1024 + 1),
        start,
    }
    .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 2 * 1024 * 1024,
        "Lexical metadata changed during inspection"
    );
    let memory = RamDirectory::create();
    memory.atomic_write(Path::new("meta.json"), &bytes)?;
    let index = tantivy::Index::open(memory)?;
    for name in ["path", "title", "body", "tags"] {
        index
            .schema()
            .get_field(name)
            .with_context(|| format!("Lexical index lacks {name}"))?;
    }
    for segment in index.searchable_segment_metas()? {
        for component in [
            tantivy::index::SegmentComponent::Postings,
            tantivy::index::SegmentComponent::Terms,
            tantivy::index::SegmentComponent::Store,
            tantivy::index::SegmentComponent::Positions,
            tantivy::index::SegmentComponent::FastFields,
            tantivy::index::SegmentComponent::FieldNorms,
        ] {
            if start.elapsed() > SCAN_BUDGET {
                return Err(ScanIncomplete("Lexical metadata check deadline exceeded").into());
            }
            let component = relative.join(segment.relative_path(component));
            storage::check_components(&kb.root, &component, true)?;
            ensure!(
                fs::metadata(kb.root.join(component))?.is_file(),
                "Lexical segment component is not a regular file"
            );
        }
    }
    Ok(())
}

/// Stops manifest parsing at the check deadline instead of consuming an unlimited derived file.
struct BudgetReader<R> {
    reader: R,
    start: Instant,
}

impl<R: Read> Read for BudgetReader<R> {
    /// Reads one chunk while the metadata-check budget remains available.
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.start.elapsed() > SCAN_BUDGET {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "status scan budget exceeded",
            ));
        }
        self.reader.read(buffer)
    }
}

/// Reconciles owned paths and stamps, then checks sidecar/configuration modification times.
fn compare_sources(
    kb: &KnowledgeBase,
    summary: &IndexSummary,
    status: &mut FolderStatus,
    start: Instant,
) -> Result<()> {
    let indexed: BTreeMap<_, _> = summary
        .records
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect();
    let mut remaining = indexed.clone();
    let mut entries = 0;
    let mut walker = walkdir::WalkDir::new(&kb.root)
        .follow_links(false)
        .into_iter();
    while let Some(entry) = walker.next() {
        entries += 1;
        ensure!(
            entries <= MAX_ENTRIES && start.elapsed() <= SCAN_BUDGET,
            "source scan budget exceeded; use a manual index refresh for complete reconciliation"
        );
        let entry = entry?;
        if entry.depth() > 0
            && entry.file_type().is_dir()
            && (matches!(entry.file_name().to_str(), Some(".omni" | ".git"))
                || storage::has_marker(entry.path())?)
        {
            walker.skip_current_dir();
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&kb.root)?
            .to_str()
            .context("Source path is not UTF-8")?;
        if relative == CONFIG_FILE || relative == ".DS_Store" {
            continue;
        }
        let metadata = entry.metadata()?;
        if let Some(record) = remaining.remove(relative) {
            if metadata.len() != record.size || modified_ms(&metadata)? != record.modified {
                status.modified += 1;
            }
        } else {
            status.added += 1;
        }
    }
    status.removed = remaining.len();
    status.metadata_changed =
        modified_ms(&fs::metadata(kb.root.join(CONFIG_FILE))?)? > summary.indexed_at;
    storage::check_components(&kb.root, Path::new(".omni/metadata"), false)?;
    if kb.root.join(".omni/metadata").exists() {
        let metadata_root = kb.root.join(".omni/metadata");
        ensure!(
            metadata_root.is_dir(),
            "Editable metadata path is not a directory"
        );
        status.metadata_changed |=
            modified_ms(&fs::metadata(&metadata_root)?)? > summary.indexed_at;
        for entry in fs::read_dir(metadata_root)? {
            entries += 1;
            ensure!(
                entries <= MAX_ENTRIES && start.elapsed() <= SCAN_BUDGET,
                "metadata scan budget exceeded"
            );
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "Editable metadata contains a symlink"
            );
            if metadata.is_file() {
                status.metadata_changed |= modified_ms(&metadata)? > summary.indexed_at;
            }
        }
    }
    if status.added + status.modified + status.removed > 0 || status.metadata_changed {
        status.state = FolderState::Stale;
        status.detail = format!(
            "Reindex needed: {} added, {} modified, {} removed; metadata/configuration changed: {}",
            status.added, status.modified, status.removed, status.metadata_changed
        );
    } else {
        status.state = FolderState::Current;
        status.detail = "Quick check: names, sizes, and modification times match the index; source hashes were not checked".into();
    }
    Ok(())
}

/// Converts a filesystem modification time to the same millisecond precision used by indexing.
fn modified_ms(metadata: &fs::Metadata) -> Result<u64> {
    Ok(metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64)
}

/// Validates local readiness and model identity with a short deadline and no source request.
fn provider_status(config: Option<&Config>) -> ServiceStatus {
    let Some(embedding) = config.and_then(|config| config.embedding.as_ref()) else {
        return ServiceStatus {
            state: ServiceState::NotConfigured,
            detail: "No local embedding provider configured; lexical search remains available"
                .into(),
        };
    };
    let mut bounded = embedding.clone();
    bounded.timeout_seconds = 2;
    match EmbeddingClient::new(&bounded).and_then(|client| client.health()) {
        Ok(health) => ServiceStatus {
            state: ServiceState::Available,
            detail: format!(
                "Local provider ready; validated {} / {} dimensions / {}",
                health.model, health.dimension, health.backend
            ),
        },
        Err(error) => ServiceStatus {
            state: ServiceState::Unavailable,
            detail: format!("{error:#}"),
        },
    }
}

/// Checks explicitly configured speech paths without executing recognition or loading weights.
fn asr_status(config: Option<&Config>) -> ServiceStatus {
    let Some(config) = config else {
        return ServiceStatus {
            state: ServiceState::NotConfigured,
            detail: "No knowledge-base ASR configuration".into(),
        };
    };
    match (&config.whisper_executable, &config.whisper_model) {
        (None, None) => ServiceStatus {
            state: ServiceState::NotConfigured,
            detail: "Optional ASR is not configured; imported transcript sidecars still work"
                .into(),
        },
        (Some(executable), Some(model)) => {
            let valid = executable.is_absolute()
                && model.is_absolute()
                && fs::metadata(executable).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
                && fs::metadata(model)
                    .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0);
            ServiceStatus {
                state: if valid {
                    ServiceState::Available
                } else {
                    ServiceState::Unavailable
                },
                detail: if valid {
                    "Configured ASR executable and model files are available; recognition was not tested".into()
                } else {
                    "Configured ASR executable or model is missing or unusable; repair the paths"
                        .into()
                },
            }
        }
        _ => ServiceStatus {
            state: ServiceState::Unavailable,
            detail: "ASR requires both an executable and a local model path".into(),
        },
    }
}
