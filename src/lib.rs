//! Shared local knowledge-library lifecycle and retrieval for agents and the GPUI interface.
pub mod embedding;
pub mod epub;
pub mod extract;
pub mod file_actions;
pub mod file_info;
#[cfg(feature = "gui")]
pub mod gui;
#[cfg(feature = "gui")]
pub(crate) mod gui_assets;
pub mod mcp;
pub mod media;
pub mod runtime;
mod search;
pub mod status;
mod storage;

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
    /// Original build path, used only to validate manifests without a stable library identity.
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
        let _lock = self.writer_lock()?;
        let library_id = storage::identity_for_index(self)?;
        let old = self.snapshot().ok();
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
            let path = self.resolve(&relative)?;
            let info = fs::metadata(&path)?;
            let hash = hash_file(&path)?;
            let metadata = self.read_metadata(&relative)?;
            let kind = media::media_kind(&path);
            let transcript_path = PathBuf::from(format!("{}.transcript.json", path.display()));
            let dependency_hash = if matches!(kind, Some("audio" | "video")) {
                if transcript_path.exists() {
                    let transcript_relative = transcript_path
                        .strip_prefix(&self.root)?
                        .to_str()
                        .context("transcript path is not UTF-8")?;
                    self.resolve(transcript_relative)?;
                    hash_file(&transcript_path)?
                } else {
                    format!("missing-transcript:{speech_identity}")
                }
            } else {
                String::new()
            };
            let mut record = if !options.force
                && previous
                    .get(relative.as_str())
                    .is_some_and(|r| r.sha256 == hash && r.dependency_hash == dependency_hash)
            {
                report.reused += 1;
                (*previous[relative.as_str()]).clone()
            } else {
                let extracted = extract::extract(&path, MAX_TEXT_BYTES).unwrap_or_else(|error| {
                    extract::Extraction {
                        text: String::new(),
                        format: "unknown".into(),
                        status: "metadata-only".into(),
                        warnings: vec![format!("extraction failed: {error:#}")],
                    }
                });
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
                let directory = tempfile::tempdir()?;
                let speech = if transcript_path.exists() {
                    Some(media::parse_transcript(&transcript_path))
                } else if options.embeddings {
                    if let (Some(executable), Some(model)) =
                        (&self.config.whisper_executable, &self.config.whisper_model)
                    {
                        Some(media::transcribe(
                            &path,
                            executable,
                            model,
                            directory.path(),
                        ))
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
                        Err(error) => record
                            .warnings
                            .push(format!("transcription unavailable: {error:#}")),
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
                match self.vectorize(&record, &path, client) {
                    Ok(items) => vectors.extend(items),
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
            records.push(record);
        }
        let trigrams = search::build_trigrams(&records);
        let snapshot = Snapshot {
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
        let (generation, cleanup_warnings) = storage::publish(self, &snapshot)?;
        report.generation = generation;
        report.warnings.extend(cleanup_warnings);
        Ok(report)
    }

    /// Searches one published snapshot and refuses unindexed or incompatible semantic requests.
    pub fn search(&self, options: &SearchOptions) -> Result<Vec<SearchHit>> {
        if options.limit == 0 {
            return Ok(vec![]);
        }
        if options.limit > 10_000 {
            bail!("result limit exceeds 10000");
        }
        let loaded = storage::load_search(self)?;
        let current = KnowledgeBase {
            root: self.root.clone(),
            config: loaded.config.clone(),
        };
        search::search(&current, &loaded.snapshot, &loaded.directory, options)
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
        let loaded = storage::load_search(self)?;
        let current = KnowledgeBase {
            root: self.root.clone(),
            config: loaded.config.clone(),
        };
        search::search_unified(&current, &loaded.snapshot, &loaded.directory, options)
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
        Ok(storage::load(self)?.0)
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
        let path = self.metadata_path(relative)?;
        if !path.exists() {
            return Ok(AssetMetadata {
                version: 1,
                path: relative.into(),
                tags: vec![],
                description: String::new(),
            });
        }
        let metadata: AssetMetadata = serde_json::from_reader(File::open(path)?)?;
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
        path: &Path,
        client: &embedding::EmbeddingClient,
    ) -> Result<Vec<VectorRecord>> {
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
                let directory = tempfile::tempdir()?;
                for segment in media::segment_video(path, directory.path(), 10.0)? {
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
