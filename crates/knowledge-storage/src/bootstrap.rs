//! Explicit marker-last initialization and source-preserving native import before a verified job exists.
use crate::{Error, Store, Version, WriteContext, WritePolicy, component, hash, journal, native};
use knowledge_core::{
    admission::{Account, Holding},
    backend::Budget,
    manifest::{ManifestDocument, ManifestLimits, ManifestState, TextRoot},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Write,
    os::unix::fs::{FileExt, MetadataExt},
    time::{Duration, Instant},
};

/// Actual pre-verification costs, not an invented verified-source job key or a second queue.
pub struct Preflight<'a> {
    /// SAME process account holding retained by the caller throughout real input/result lifetimes.
    pub holding: &'a Holding,
    /// Explicit native per-call work maxima; no whole-corpus quota or unlimited zero.
    pub budget: Budget,
}
/// Borrowed exact initialization bytes; caller-owned template allocation must already be charged.
pub struct Seed<'a> {
    /// Normalized destination; only the root marker can use the marker basename.
    pub path: &'a str,
    /// Exact bytes to install, including opaque authored extensions/provenance.
    pub bytes: &'a [u8],
}
/// Honest initializer outcome, including actual marker uncertainty after a partial native failure.
pub enum InitOutcome {
    /// All files and the LAST manifest were synchronized; Git initialization/commit remains explicit.
    Initialized {
        /// Actual installed marker digest/stat, available for a real subsequent verified job key.
        manifest: Version,
    },
    /// Payload may be partial; preserve all evidence and never delete unknown/preexisting data.
    Partial {
        /// Only paths whose install/sync returned success, not an inference about the other paths.
        confirmed_installed: Vec<String>,
        /// Actual current marker observation, or None if it could not be established.
        marker_present: Option<bool>,
        /// Fixed reason; no rollback or worker termination is implied.
        cause: Error,
    },
}
/// Actual exclusive imported copy; equal digest does not associate different source identities.
pub struct ImportResult {
    /// Effective destination, including the reserved-marker basename correction.
    pub path: String,
    /// Verified complete destination bytes and native stat.
    pub version: Version,
}
/// Explicit versioned payload recipe; original bodies remain separate retained files, never JSONL data.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    /// Native encoding version.
    format_version: u32,
    /// init or import, not a processor registry.
    kind: String,
    /// Native operation identity.
    operation_id: String,
    /// Selected library identity, never a grant.
    library_id: String,
    /// Native captured root.
    root: (u64, u64),
    /// Original writer evidence, not authority to append another writer's stream.
    writer_id: String,
    /// Exact selected destinations and candidate byte facts.
    files: Vec<Item>,
    /// Unverified source stat tuple: dev, inode, length, mtime-sec/ns, ctime-sec/ns; no digest invented.
    source_stamp: Option<(u64, u64, u64, i64, i64, i64, i64)>,
}
/// One exact initialization/import destination; digest stays null until bytes were actually hashed.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    /// Normalized selected destination.
    path: String,
    /// Known actual bytes, or null for the initial unverified import recipe.
    digest: Option<String>,
    /// Native length or exact template byte count.
    length: u64,
}
/// Synchronized selected-copy outcome bound to actual file version and visible final record.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Imported {
    /// Exact corrected destination.
    path: String,
    /// Actual installed inode/stat/bytes, not equal-content asset association.
    version: Version,
    /// Actual synchronized visible observation.
    proof: journal::Proof,
}
/// Actual pre-verification work bound; does not release the caller's resource capability.
pub(super) struct Bounds {
    /// Reserved scratch available for this call.
    pub(super) memory: u64,
    /// Finite native input/output work ceiling.
    pub(super) bytes: u64,
    /// Earliest representable caller deadline.
    pub(super) deadline: Instant,
}
impl Bounds {
    /// Establish actual process ownership and prepaid resources before operation allocations.
    pub(super) fn new(p: &Preflight<'_>) -> Result<Self, Error> {
        Account::current()?;
        let remaining = p.holding.remaining()?;
        let memory = remaining.memory_bytes.min(p.budget.max_memory_bytes);
        if memory < 32768
            || p.budget.max_files == 0
            || p.budget.max_bytes == 0
            || p.budget.max_millis == 0
        {
            return Err(Error::Budget);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(p.budget.max_millis))
            .ok_or(Error::Budget)?;
        Ok(Self {
            memory,
            bytes: p.budget.max_bytes,
            deadline,
        })
    }
    /// Check between native steps/chunks, never pretend this preempts a blocked kernel operation.
    pub(super) fn check(&self) -> Result<(), Error> {
        if Instant::now() >= self.deadline {
            Err(Error::Budget)
        } else {
            Ok(())
        }
    }
    /// Preflight every operation-owned identity copy/escaping before metadata allocation.
    pub(super) fn identities(
        &self,
        operation: &str,
        library: &str,
        writer: &str,
    ) -> Result<(), Error> {
        if operation.is_empty() {
            return Err(Error::Invalid);
        }
        let bytes = [operation.len(), library.len(), writer.len()]
            .into_iter()
            .try_fold(4096u64, |n, len| {
                n.checked_add((len as u64).checked_mul(6)?)
            })
            .ok_or(Error::Budget)?;
        if bytes > self.control() {
            return Err(Error::Budget);
        }
        Ok(())
    }
    /// Bound private metadata parsing/encoding within prepaid scratch, not a persistent-format quota.
    pub(super) fn control(&self) -> u64 {
        self.bytes.min(self.memory.saturating_sub(16384) / 64)
    }
}
impl<P: WritePolicy> Store<P> {
    /// Preflight every destination and retain exact exclusive stages without installing home payloads.
    /// The initializer/resume installs the marker LAST; no Git commit or cross-domain save is fabricated.
    pub fn stage_initialization(
        &self,
        operation: &str,
        seeds: &[Seed<'_>],
        preflight: &Preflight<'_>,
    ) -> Result<(), Error> {
        let bounds = Bounds::new(preflight)?;
        bounds.identities(operation, &self.library, &self.writer)?;
        let c = self.gate()?;
        bounds.check()?;
        if seeds.len() as u64 > preflight.budget.max_files {
            return Err(Error::Budget);
        }
        let marker = seeds
            .iter()
            .find(|s| s.path == "omni-knowledge.json")
            .ok_or(Error::Invalid)?;
        if marker.bytes.len() as u64 > bounds.memory / 8 {
            return Err(Error::Budget);
        }
        self.initial_marker(marker.bytes, &c, &bounds)?;
        let mut items = Vec::new();
        let mut total = 0u64;
        let mut meta = 4096u64;
        for seed in seeds {
            bounds.check()?;
            self.payload_selected(seed.path, &c, &bounds)?;
            if items.iter().any(|i: &Item| {
                i.path == seed.path
                    || i.path
                        .strip_prefix(seed.path)
                        .is_some_and(|s| s.starts_with('/'))
                    || seed
                        .path
                        .strip_prefix(&i.path)
                        .is_some_and(|s| s.starts_with('/'))
            }) {
                return Err(Error::Invalid);
            }
            total = total
                .checked_add(seed.bytes.len() as u64)
                .ok_or(Error::Budget)?;
            meta = meta
                .checked_add(seed.path.len() as u64 * 6 + 512)
                .ok_or(Error::Budget)?;
            if total > bounds.bytes || meta > bounds.control() {
                return Err(Error::Budget);
            }
            if native::observe(&self.root, seed.path, bounds.bytes, bounds.deadline)?.is_some() {
                return Err(Error::Conflict);
            }
            items.push(Item {
                path: seed.path.to_owned(),
                digest: Some(hash(seed.bytes)),
                length: seed.bytes.len() as u64,
            });
        }
        let intent = Payload {
            format_version: 1,
            kind: "init".into(),
            operation_id: operation.to_owned(),
            library_id: self.library.clone(),
            root: native::identity(&self.root)?,
            writer_id: self.writer.clone(),
            files: items,
            source_stamp: None,
        };
        let lane = self.lane()?;
        let dir = native::new_child(&lane.recovery, &hash(operation.as_bytes()))?
            .ok_or(Error::Conflict)?;
        let fingerprint = payload_hash(&intent)?;
        let mut journal = journal::Journal::open(&lane.journal, &self.writer, bounds.control())?;
        let begin = journal.append(operation, &fingerprint, None, "init_intent")?;
        native::control_new(&dir, "init.json", &intent)?;
        native::control_new(&dir, "begin.json", &begin)?;
        for (n, seed) in seeds.iter().enumerate() {
            bounds.check()?;
            native::create(&dir, &format!("new-{n}"), seed.bytes)?;
        }
        self.live(&c)?;
        Ok(())
    }
    /// Initialize only a freshly staged operation; repeats require explicit inspection, never blind replay.
    pub fn initialize(
        &self,
        operation: &str,
        seeds: &[Seed<'_>],
        preflight: &Preflight<'_>,
    ) -> Result<InitOutcome, Error> {
        self.stage_initialization(operation, seeds, preflight)?;
        self.resume_initialization(operation, preflight)
    }
    /// Inspect actual retained initialization without creating, replaying or deleting any home bytes.
    pub fn inspect_initialization(
        &self,
        operation: &str,
        preflight: &Preflight<'_>,
    ) -> Result<InitOutcome, Error> {
        let b = Bounds::new(preflight)?;
        b.identities(operation, &self.library, &self.writer)?;
        let c = self.gate()?;
        let lane = self.existing_lane()?;
        let dir = native::directory(&lane.recovery, &hash(operation.as_bytes()))?;
        let intent: Payload = native::control(&dir, "init.json", b.control())?;
        self.payload_intent(&intent, operation, "init")?;
        let fp = payload_hash(&intent)?;
        let begin: journal::Proof = native::control(&dir, "begin.json", b.control())?;
        if begin.writer != intent.writer_id {
            return Err(Error::Recovery);
        }
        journal::Journal::read(&lane.journal, &begin.writer, b.control())?.verify(
            &begin,
            operation,
            &fp,
            "init_intent",
        )?;
        let mut confirmed = Vec::new();
        for (n, item) in intent.files.iter().enumerate() {
            b.check()?;
            self.payload_selected(&item.path, &c, &b)?;
            let staged = native::version(
                native::regular(&dir, &format!("new-{n}"))?.ok_or(Error::Recovery)?,
                b.bytes,
                b.deadline,
            )?;
            if Some(&staged.digest) != item.digest.as_ref() {
                return Err(Error::Recovery);
            }
            if native::observe(&self.root, &item.path, b.bytes, b.deadline)?
                .is_some_and(|v| crate::same_displaced(&v, &staged))
            {
                confirmed.push(item.path.clone());
            }
        }
        self.live(&c)?;
        if confirmed.len() == intent.files.len()
            && native::regular(&dir, "initialized.json")?.is_some()
        {
            let proof: journal::Proof = native::control(&dir, "initialized.json", b.control())?;
            journal::Journal::read(&lane.journal, &proof.writer, b.control())?.verify(
                &proof,
                operation,
                &fp,
                "initialized",
            )?;
            return Ok(InitOutcome::Initialized {
                manifest: native::observe(&self.root, "omni-knowledge.json", b.bytes, b.deadline)?
                    .ok_or(Error::Recovery)?,
            });
        }
        Ok(InitOutcome::Partial {
            confirmed_installed: confirmed,
            marker_present: native::regular(&self.root, "omni-knowledge.json")
                .map(|f| f.is_some())
                .ok(),
            cause: Error::Indeterminate,
        })
    }
    /// Inspect a synchronized selected import without copying again or claiming its original source identity.
    pub fn inspect_import(
        &self,
        operation: &str,
        preflight: &Preflight<'_>,
    ) -> Result<ImportResult, Error> {
        let b = Bounds::new(preflight)?;
        b.identities(operation, &self.library, &self.writer)?;
        let c = self.gate()?;
        let lane = self.existing_lane()?;
        let dir = native::directory(&lane.recovery, &hash(operation.as_bytes()))?;
        let intent: Payload = native::control(&dir, "import.json", b.control())?;
        self.payload_intent(&intent, operation, "import")?;
        let fp = payload_hash(&intent)?;
        let begin: journal::Proof = native::control(&dir, "begin.json", b.control())?;
        if begin.writer != intent.writer_id {
            return Err(Error::Recovery);
        }
        journal::Journal::read(&lane.journal, &begin.writer, b.control())?.verify(
            &begin,
            operation,
            &fp,
            "import_intent",
        )?;
        let imported: Imported = native::control(&dir, "imported.json", b.control())?;
        if intent.files.len() != 1 || intent.files[0].path != imported.path {
            return Err(Error::Recovery);
        }
        self.payload_selected(&imported.path, &c, &b)?;
        journal::Journal::read(&lane.journal, &imported.proof.writer, b.control())?
            .verify_observed(
                &imported.proof,
                operation,
                &fp,
                &imported.path,
                &imported.version.digest,
                "imported",
            )?;
        if native::observe(&self.root, &imported.path, b.bytes, b.deadline)?
            != Some(imported.version.clone())
        {
            return Err(Error::Conflict);
        }
        self.live(&c)?;
        Ok(ImportResult {
            path: imported.path,
            version: imported.version,
        })
    }
    /// Resume ONLY explicitly selected proven staged initialization, never replay unknown recovery bytes.
    pub fn resume_initialization(
        &self,
        operation: &str,
        preflight: &Preflight<'_>,
    ) -> Result<InitOutcome, Error> {
        let bounds = Bounds::new(preflight)?;
        let c = self.gate()?;
        let lane = self.lane()?;
        let dir = native::directory(&lane.recovery, &hash(operation.as_bytes()))?;
        let intent: Payload = native::control(&dir, "init.json", bounds.control())?;
        self.payload_intent(&intent, operation, "init")?;
        let fingerprint = payload_hash(&intent)?;
        let proof: journal::Proof = native::control(&dir, "begin.json", bounds.control())?;
        if proof.writer != intent.writer_id {
            return Err(Error::Recovery);
        }
        journal::Journal::read(&lane.journal, &proof.writer, bounds.control())?.verify(
            &proof,
            operation,
            &fingerprint,
            "init_intent",
        )?;
        let marker = intent
            .files
            .iter()
            .position(|i| i.path == "omni-knowledge.json")
            .ok_or(Error::Recovery)?;
        let text = native::text(
            &dir,
            &format!("new-{marker}"),
            bounds.bytes.min(bounds.memory / 8),
        )?;
        self.initial_marker(text.as_bytes(), &c, &bounds)?;
        let mut journal = journal::Journal::open(&lane.journal, &self.writer, bounds.control())?;
        self.finish_init(&intent, &dir, &mut journal, &c, &bounds)
    }
    /// Validate supported raw manifest bytes without ever serializing the lossy metadata projection.
    fn initial_marker(&self, bytes: &[u8], c: &WriteContext, b: &Bounds) -> Result<(), Error> {
        let doc = ManifestDocument::decode(
            bytes,
            ManifestLimits {
                max_bytes: usize::try_from(b.bytes.min(b.memory / 8)).map_err(|_| Error::Budget)?,
            },
        )
        .map_err(|_| Error::Invalid)?;
        if doc.state() != ManifestState::EditableV2 {
            return Err(Error::Unsupported);
        }
        let meta = doc.metadata().ok_or(Error::Invalid)?;
        let text = match meta.text_root {
            TextRoot::Pages => "pages",
            TextRoot::Wiki => "wiki",
        };
        if meta.library_id != self.library || text != c.text_root {
            return Err(Error::Context);
        }
        Ok(())
    }
    /// Only explicit new template/copy destinations bypass raw replacement prohibition; never runtime internals.
    fn payload_selected(&self, path: &str, c: &WriteContext, b: &Bounds) -> Result<(), Error> {
        if path.len() as u64 > b.control() / 8 {
            return Err(Error::Budget);
        }
        knowledge_core::data::RelativePath::new(path.to_owned()).map_err(|_| Error::Invalid)?;
        if [".git", ".omni", "journal"]
            .iter()
            .any(|p| path == *p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
        {
            return Err(Error::Context);
        }
        if path.rsplit('/').next() == Some("omni-knowledge.json") && path != "omni-knowledge.json" {
            return Err(Error::Context);
        }
        if c.outside_scope
            .iter()
            .any(|p| path == p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
        {
            return Err(Error::Context);
        }
        Ok(())
    }
    /// Install proven stages idempotently by native inode AND digest; the marker is always last.
    fn finish_init(
        &self,
        intent: &Payload,
        dir: &File,
        journal: &mut journal::Journal,
        c: &WriteContext,
        b: &Bounds,
    ) -> Result<InitOutcome, Error> {
        let mut installed = Vec::new();
        let fp = payload_hash(intent)?;
        let run = (|| -> Result<Version, Error> {
            let mut order = (0..intent.files.len()).collect::<Vec<_>>();
            order.sort_by_key(|n| intent.files[*n].path == "omni-knowledge.json");
            for n in order {
                b.check()?;
                self.live(c)?;
                let item = &intent.files[n];
                self.payload_selected(&item.path, c, b)?;
                let staged = native::version(
                    native::regular(dir, &format!("new-{n}"))?.ok_or(Error::Recovery)?,
                    b.bytes,
                    b.deadline,
                )?;
                if Some(&staged.digest) != item.digest.as_ref() || staged.length != item.length {
                    return Err(Error::Recovery);
                }
                let current = native::observe(&self.root, &item.path, b.bytes, b.deadline)?;
                if let Some(v) = current {
                    if !crate::same_displaced(&v, &staged) {
                        return Err(Error::Conflict);
                    }
                } else {
                    let (p, name) = native::parent(&self.root, &item.path, true)?;
                    self.live(c)?;
                    native::install(dir, &format!("new-{n}"), &p, &name)?;
                }
                installed.push(item.path.clone());
            }
            self.live(c)?;
            let proof = journal.append(&intent.operation_id, &fp, None, "initialized")?;
            if native::regular(dir, "initialized.json")?.is_none() {
                native::control_new(dir, "initialized.json", &proof)?;
            }
            native::observe(&self.root, "omni-knowledge.json", b.bytes, b.deadline)?
                .ok_or(Error::Indeterminate)
        })();
        match run {
            Ok(manifest) => Ok(InitOutcome::Initialized { manifest }),
            Err(cause) => Ok(InitOutcome::Partial {
                confirmed_installed: installed,
                marker_present: native::regular(&self.root, "omni-knowledge.json")
                    .map(|f| f.is_some())
                    .ok(),
                cause,
            }),
        }
    }
    /// Copy a currently selected regular source to an exclusive destination; never rename/delete the source.
    /// This pre-verification path needs no fake digest; complete copied bytes establish the returned version.
    pub fn import_copy(
        &self,
        operation: &str,
        source: &File,
        destination: &str,
        preflight: &Preflight<'_>,
    ) -> Result<ImportResult, Error> {
        let b = Bounds::new(preflight)?;
        b.identities(operation, &self.library, &self.writer)?;
        b.check()?;
        let c = self.gate()?;
        if destination.len() as u64 > b.control() / 16 {
            return Err(Error::Budget);
        }
        let path = if destination.rsplit('/').next() == Some("omni-knowledge.json") {
            let prefix = destination
                .strip_suffix("omni-knowledge.json")
                .ok_or(Error::Invalid)?;
            format!("{prefix}omni-knowledge.asset.json")
        } else {
            destination.to_owned()
        };
        self.payload_selected(&path, &c, &b)?;
        if native::observe(&self.root, &path, b.bytes, b.deadline)?.is_some() {
            return Err(Error::Conflict);
        }
        let before = source.metadata().map_err(|_| Error::Io)?;
        if !before.is_file() {
            return Err(Error::Unsupported);
        }
        if before.len() > b.bytes {
            return Err(Error::Budget);
        }
        let stamp = (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec(),
        );
        let intent = Payload {
            format_version: 1,
            kind: "import".into(),
            operation_id: operation.to_owned(),
            library_id: self.library.clone(),
            root: native::identity(&self.root)?,
            writer_id: self.writer.clone(),
            files: vec![Item {
                path: path.clone(),
                digest: None,
                length: before.len(),
            }],
            source_stamp: Some(stamp),
        };
        let lane = self.lane()?;
        let dir = native::new_child(&lane.recovery, &hash(operation.as_bytes()))?
            .ok_or(Error::Conflict)?;
        let fp = payload_hash(&intent)?;
        let mut journal = journal::Journal::open(&lane.journal, &self.writer, b.control())?;
        let proof = journal.append(operation, &fp, None, "import_intent")?;
        native::control_new(&dir, "import.json", &intent)?;
        native::control_new(&dir, "begin.json", &proof)?;
        let mut target = native::create_file(&dir, "new-0")?;
        let mut offset = 0u64;
        let mut buffer = [0u8; 8192];
        let mut h = Sha256::new();
        loop {
            b.check()?;
            self.live(&c)?;
            let n = source.read_at(&mut buffer, offset).map_err(|_| Error::Io)?;
            if n == 0 {
                break;
            }
            offset = offset.checked_add(n as u64).ok_or(Error::Budget)?;
            if offset > b.bytes {
                return Err(Error::Budget);
            }
            target.write_all(&buffer[..n]).map_err(|_| Error::Io)?;
            h.update(&buffer[..n]);
        }
        let after = source.metadata().map_err(|_| Error::Io)?;
        if stamp
            != (
                after.dev(),
                after.ino(),
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec(),
            )
            || offset != before.len()
        {
            return Err(Error::Conflict);
        }
        target.sync_all().map_err(|_| Error::Io)?;
        drop(target);
        dir.sync_all().map_err(|_| Error::Io)?;
        let digest = format!("{:x}", h.finalize());
        native::control_new(&dir, "verified.json", &digest)?;
        self.live(&c)?;
        let (parent, name) = native::parent(&self.root, &path, true)?;
        native::install(&dir, "new-0", &parent, &name)?;
        let version =
            native::observe(&self.root, &path, b.bytes, b.deadline)?.ok_or(Error::Indeterminate)?;
        if version.digest != digest {
            return Err(Error::Indeterminate);
        }
        self.live(&c)?;
        let proof = journal.append_observed(operation, &fp, &path, &digest, "imported")?;
        native::control_new(
            &dir,
            "imported.json",
            &Imported {
                path: path.clone(),
                version: version.clone(),
                proof,
            },
        )?;
        Ok(ImportResult { path, version })
    }
    /// Validate native recovery binding without turning its metadata into current authority.
    fn payload_intent(&self, i: &Payload, operation: &str, kind: &str) -> Result<(), Error> {
        if i.format_version != 1
            || i.operation_id != operation
            || i.kind != kind
            || i.library_id != self.library
            || i.root != native::identity(&self.root)?
            || !component(&i.writer_id)
        {
            return Err(Error::Recovery);
        }
        Ok(())
    }
}
/// Hash only versioned metadata; original/template bodies remain outside the journal.
fn payload_hash(i: &Payload) -> Result<String, Error> {
    let mut sink = crate::HashWriter(Sha256::new());
    serde_json::to_writer(&mut sink, i).map_err(|_| Error::Invalid)?;
    Ok(format!("{:x}", sink.0.finalize()))
}
