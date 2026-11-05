//! One immutable disk generation of little-endian float32 vectors.
//!
//! The manifest names rows and identity. It does not embed the vectors. A query checks
//! eligibility before reading that row's segment bytes.

use crate::{AccountGrant, GrantError, before_work};
use knowledge_retrieval::Work;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Exact baseline metric. Unknown metrics cannot silently reuse cosine data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorMetric {
    Cosine,
}

/// The provider's vector-output contract; cosine does not make spaces interchangeable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorNormalization {
    Raw,
    L2,
}

/// Complete cache/space identity, never source or native effect authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelIdentity {
    pub provider: String,
    pub model: String,
    pub revision: String,
    pub dimension: usize,
    pub metric: VectorMetric,
    pub normalization: VectorNormalization,
    pub package_fingerprint: String,
    pub embedding_space: String,
    pub preprocessing: String,
}

#[derive(Clone, Debug)]
pub struct VectorRow {
    pub source_id: String,
    pub source_digest: String,
    pub locator: String,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Manifest {
    identity: ModelIdentity,
    rows: Vec<RowMeta>,
    segment_sha256: String,
    segment_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RowMeta {
    source_id: String,
    source_digest: String,
    locator: String,
    offset: u64,
    /// Hash of this exact row, allowing eligibility before any payload access.
    sha256: String,
}

pub struct VectorStore {
    root: PathBuf,
    generation: String,
    manifest: Manifest,
    /// Actual affine envelope; the original grant and all its clones are consumed.
    _work: std::sync::Arc<Work>,
}

impl VectorStore {
    /// Build a private generation and switch the current pointer only after the segment is complete.
    pub fn build(
        root: &Path,
        identity: &ModelIdentity,
        rows: &[VectorRow],
        grant: &AccountGrant,
        deadline: Option<Instant>,
    ) -> Result<Self, VectorError> {
        validate_identity(identity)?;
        let bytes = (rows.len() as u64)
            .checked_mul(identity.dimension as u64)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| VectorError::Identity("segment size overflow".into()))?;
        let identity_bytes = [
            &identity.provider,
            &identity.model,
            &identity.revision,
            &identity.package_fingerprint,
            &identity.embedding_space,
            &identity.preprocessing,
        ]
        .iter()
        .try_fold(4096u64, |sum, field| {
            sum.checked_add((field.len() as u64).checked_mul(6)?)
        })
        .ok_or_else(|| VectorError::Identity("model metadata size overflow".into()))?;
        let metadata_bound = rows
            .iter()
            .try_fold(identity_bytes, |total, row| {
                let strings = (row.source_id.len() as u64)
                    .checked_add(row.source_digest.len() as u64)?
                    .checked_add(row.locator.len() as u64)?;
                total.checked_add(strings.checked_mul(6)?.checked_add(256)?)
            })
            .ok_or_else(|| VectorError::Identity("metadata size overflow".into()))?;
        let memory = bytes
            .checked_add(
                metadata_bound
                    .checked_mul(8)
                    .ok_or_else(|| VectorError::Identity("metadata size overflow".into()))?,
            )
            .and_then(|n| n.checked_add(32768))
            .ok_or_else(|| VectorError::Identity("working size overflow".into()))?;
        let storage = bytes
            .checked_add(metadata_bound)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| VectorError::Identity("storage size overflow".into()))?;
        before_work(grant, memory, deadline).map_err(VectorError::Grant)?;
        grant.covers(memory, storage).map_err(VectorError::Grant)?;
        let mut work = grant.take().map_err(VectorError::Grant)?;
        private_dir(root)?;
        let _writer = scope_lock(root, true)?;
        let stage = root.join(format!(
            ".vector-stage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(invalid)?
                .as_nanos()
        ));
        fs::create_dir(&stage).map_err(io)?;
        let mut segment = File::create(stage.join("segment.bin")).map_err(io)?;
        let mut segment_hash = Sha256::new();
        let mut segment_bytes = 0u64;
        let mut meta = Vec::new();
        let mut seen = BTreeSet::new();
        for row in rows {
            if deadline.is_some_and(|limit| Instant::now() >= limit) {
                return Err(VectorError::Grant(GrantError::Deadline));
            }
            if !seen.insert(row.source_id.clone()) {
                return Err(VectorError::Identity("duplicate source id".into()));
            }
            if !valid_vector(identity, &row.vector) {
                return Err(VectorError::Vector(row.source_id.clone()));
            }
            let offset = segment_bytes;
            let mut row_hash = Sha256::new();
            let mut encoded = [0u8; 8192];
            for values in row.vector.chunks(encoded.len() / 4) {
                for (slot, value) in encoded.as_chunks_mut::<4>().0.iter_mut().zip(values) {
                    slot.copy_from_slice(&value.to_le_bytes());
                }
                let block = &encoded[..values.len() * 4];
                write_funded(&mut segment, block, &mut work, deadline)?;
                row_hash.update(block);
                segment_hash.update(block);
                segment_bytes = segment_bytes
                    .checked_add(block.len() as u64)
                    .ok_or_else(|| VectorError::Identity("segment size overflow".into()))?;
            }
            meta.push(RowMeta {
                source_id: row.source_id.clone(),
                source_digest: row.source_digest.clone(),
                locator: row.locator.clone(),
                offset,
                sha256: format!("{:x}", row_hash.finalize()),
            });
        }
        segment.sync_all().map_err(io)?;
        drop(segment);
        let manifest = Manifest {
            identity: identity.clone(),
            rows: meta,
            segment_sha256: format!("{:x}", segment_hash.finalize()),
            segment_bytes,
        };
        let generation = sha256(serde_json::to_vec(&manifest).map_err(invalid)?.as_slice());
        let directory = root.join(&generation);
        if directory.exists() {
            return Err(VectorError::Identity("generation already exists".into()));
        }
        let metadata = serde_json::to_vec(&manifest).map_err(invalid)?;
        let mut metadata_file = File::create(stage.join("manifest.json")).map_err(io)?;
        write_funded(&mut metadata_file, &metadata, &mut work, deadline)?;
        metadata_file.sync_all().map_err(io)?;
        drop(metadata_file);
        drop(metadata);
        fs::rename(&stage, &directory).map_err(io)?;
        let current = root.join("CURRENT");
        let temporary = root.join("CURRENT.next");
        let mut pointer = File::create(&temporary).map_err(io)?;
        write_funded(&mut pointer, generation.as_bytes(), &mut work, deadline)?;
        pointer.sync_all().map_err(io)?;
        drop(pointer);
        fs::rename(&temporary, &current).map_err(io)?;
        Ok(Self {
            root: root.to_path_buf(),
            generation,
            manifest,
            _work: std::sync::Arc::new(work),
        })
    }

    /// Open the current generation only when its model identity matches.
    pub fn open(root: &Path, identity: &ModelIdentity) -> Result<Self, VectorError> {
        let grant = AccountGrant::reserve(
            knowledge_core::jobs::Reservation {
                memory_bytes: 64 * 1024 * 1024,
                temporary_bytes: 0,
            },
            knowledge_core::admission::Origin::BeforeVerification,
        )
        .map_err(VectorError::Grant)?;
        Self::open_funded(root, identity, &grant)
    }

    /// Open under a complete existing envelope, before native metadata or payload access.
    pub fn open_funded(
        root: &Path,
        identity: &ModelIdentity,
        grant: &AccountGrant,
    ) -> Result<Self, VectorError> {
        validate_identity(identity)?;
        grant.covers(64 * 1024, 0).map_err(VectorError::Grant)?;
        let work = grant.take().map_err(VectorError::Grant)?;
        let cap = work.remaining().map_err(invalid)?.memory_bytes / 8;
        let scope = scope_lock(root, false)?;
        let mut generation = String::new();
        File::open(root.join("CURRENT"))
            .map_err(io)?
            .take(65)
            .read_to_string(&mut generation)
            .map_err(io)?;
        if generation.len() != 64 || !generation.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(VectorError::Identity("invalid generation reference".into()));
        }
        let directory = root.join(&generation);
        let reader = open_lock(&directory.join("READERS"))?;
        fs2::FileExt::try_lock_shared(&reader).map_err(|_| VectorError::Pinned)?;
        drop(scope);
        let file = File::open(directory.join("manifest.json")).map_err(io)?;
        if file.metadata().map_err(io)?.len() > cap {
            return Err(VectorError::Grant(GrantError::TooSmall));
        }
        let mut metadata = Vec::new();
        file.take(cap.saturating_add(1))
            .read_to_end(&mut metadata)
            .map_err(io)?;
        if metadata.len() as u64 > cap {
            return Err(VectorError::Grant(GrantError::TooSmall));
        }
        if sha256(&metadata) != generation {
            return Err(VectorError::Identity(
                "manifest generation hash mismatch".into(),
            ));
        }
        let manifest: Manifest = serde_json::from_slice(&metadata).map_err(invalid)?;
        if &manifest.identity != identity {
            return Err(VectorError::Identity("model identity mismatch".into()));
        }
        let mut file = File::open(directory.join("segment.bin")).map_err(io)?;
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 8192];
        let mut total = 0u64;
        loop {
            let n = file.read(&mut bytes).map_err(io)?;
            if n == 0 {
                break;
            }
            total = total
                .checked_add(n as u64)
                .ok_or_else(|| VectorError::Identity("segment size overflow".into()))?;
            if total > manifest.segment_bytes {
                return Err(VectorError::Identity("segment length mismatch".into()));
            }
            hash.update(&bytes[..n]);
        }
        if total != manifest.segment_bytes
            || format!("{:x}", hash.finalize()) != manifest.segment_sha256
        {
            return Err(VectorError::Identity("segment hash mismatch".into()));
        }
        Ok(Self {
            root: root.to_path_buf(),
            generation,
            manifest,
            _work: std::sync::Arc::new(work),
        })
    }

    /// Accept an already validated STORED payload. This does not open or extract a ZIP.
    pub fn import_stored(
        root: &Path,
        identity: &ModelIdentity,
        rows: &[VectorRow],
        segment: &[u8],
        grant: &AccountGrant,
    ) -> Result<Self, VectorError> {
        if sha256(segment) != expected_segment(identity, rows)? {
            return Err(VectorError::Identity(
                "stored segment does not match the rows".into(),
            ));
        }
        let mut owned = Vec::new();
        for (row, chunk) in rows.iter().zip(segment.chunks(identity.dimension * 4)) {
            let mut vector = Vec::new();
            for bytes in chunk.chunks(4) {
                let mut le = [0; 4];
                le.copy_from_slice(bytes);
                vector.push(f32::from_le_bytes(le));
            }
            owned.push(VectorRow {
                source_id: row.source_id.clone(),
                source_digest: row.source_digest.clone(),
                locator: row.locator.clone(),
                vector,
            });
        }
        Self::build(root, identity, &owned, grant, None)
    }

    /// Rank eligible rows. Ineligible offsets are not read.
    pub fn query(
        self,
        eligible: &BTreeSet<String>,
        query: &[f32],
    ) -> Result<VectorResult, VectorError> {
        if !valid_vector(&self.manifest.identity, query) {
            return Err(VectorError::Vector("query".into()));
        }
        let row_bytes = self
            .manifest
            .identity
            .dimension
            .checked_mul(4)
            .ok_or_else(|| VectorError::Identity("row size overflow".into()))?;
        let result_bytes = self
            .manifest
            .rows
            .iter()
            .try_fold(0u64, |sum, row| {
                sum.checked_add(row.source_id.len() as u64 + 96)
            })
            .ok_or_else(|| VectorError::Identity("result size overflow".into()))?;
        let resident = (self.manifest.rows.capacity() as u64)
            .checked_mul(std::mem::size_of::<RowMeta>() as u64)
            .and_then(|base| {
                self.manifest.rows.iter().try_fold(base, |sum, row| {
                    sum.checked_add(row.source_id.capacity() as u64)?
                        .checked_add(row.source_digest.capacity() as u64)?
                        .checked_add(row.locator.capacity() as u64)?
                        .checked_add(row.sha256.capacity() as u64)
                })
            })
            .ok_or_else(|| VectorError::Identity("resident metadata size overflow".into()))?;
        let need = result_bytes
            .checked_add(resident)
            .ok_or_else(|| VectorError::Identity("query size overflow".into()))?
            .checked_add(
                (row_bytes as u64)
                    .checked_mul(3)
                    .ok_or_else(|| VectorError::Identity("row size overflow".into()))?,
            )
            .and_then(|n| n.checked_add(32768))
            .ok_or_else(|| VectorError::Identity("query size overflow".into()))?;
        if self._work.remaining().map_err(invalid)?.memory_bytes < need {
            return Err(VectorError::Grant(GrantError::TooSmall));
        }
        let pin = self.pin()?;
        let mut file =
            File::open(self.root.join(&self.generation).join("segment.bin")).map_err(io)?;
        let mut ranked = Vec::new();
        for row in &self.manifest.rows {
            if !eligible.contains(&row.source_id) {
                continue;
            }
            file.seek(SeekFrom::Start(row.offset)).map_err(io)?;
            let mut bytes = vec![0; row_bytes];
            file.read_exact(&mut bytes).map_err(io)?;
            if sha256(&bytes) != row.sha256 {
                return Err(VectorError::Identity("eligible row hash mismatch".into()));
            }
            let vector: Vec<f32> = bytes
                .chunks(4)
                .map(|chunk| {
                    let mut le = [0; 4];
                    le.copy_from_slice(chunk);
                    f32::from_le_bytes(le)
                })
                .collect();
            if !valid_vector(&self.manifest.identity, &vector) {
                return Err(VectorError::Vector(row.source_id.clone()));
            }
            ranked.push((row.source_id.clone(), cosine(query, &vector)));
        }
        ranked.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
        Ok(VectorResult {
            ranked,
            file,
            pin,
            store: self,
        })
    }

    pub fn pin(&self) -> Result<Pin, VectorError> {
        let _scope = scope_lock(&self.root, false)?;
        let file = open_lock(&self.root.join(&self.generation).join("READERS"))?;
        fs2::FileExt::try_lock_shared(&file).map_err(|_| VectorError::Pinned)?;
        Ok(Pin {
            _file: file,
            _work: self._work.clone(),
        })
    }

    /// Refuse retirement while a pin exists. Previous generations stay until an unpinned retire.
    pub fn retire(root: &Path, generation: &str) -> Result<(), VectorError> {
        let _work = AccountGrant::reserve(
            knowledge_core::jobs::Reservation {
                memory_bytes: 64 * 1024,
                temporary_bytes: 0,
            },
            knowledge_core::admission::Origin::BeforeVerification,
        )
        .map_err(VectorError::Grant)?
        .take()
        .map_err(VectorError::Grant)?;
        if generation.len() != 64 || !generation.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(VectorError::Identity("invalid generation reference".into()));
        }
        let _scope = scope_lock(root, true)?;
        let directory = root.join(generation);
        let reader = open_lock(&directory.join("READERS"))?;
        fs2::FileExt::try_lock_exclusive(&reader).map_err(|_| VectorError::Pinned)?;
        let mut current = String::new();
        File::open(root.join("CURRENT"))
            .map_err(io)?
            .take(65)
            .read_to_string(&mut current)
            .map_err(io)?;
        if current.len() != 64 || !current.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(VectorError::Identity(
                "unrecognized current generation retained".into(),
            ));
        }
        if current == generation {
            return Err(VectorError::Pinned);
        }
        for entry in fs::read_dir(&directory).map_err(io)? {
            let entry = entry.map_err(io)?;
            if !entry.file_type().map_err(io)?.is_file()
                || !["manifest.json", "segment.bin", "READERS"]
                    .iter()
                    .any(|name| entry.file_name() == *name)
            {
                return Err(VectorError::Identity(
                    "unrecognized generation files retained".into(),
                ));
            }
        }
        for name in ["manifest.json", "segment.bin", "READERS"] {
            fs::remove_file(directory.join(name)).map_err(io)?;
        }
        fs::remove_dir(&directory).map_err(io)?;
        Ok(())
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }
}

/// One affine query/delivery lifetime. Buffers and the exact used FD die before
/// the generation and actual funding; `into_store` destroys them before reuse.
///
/// ```compile_fail,E0382
/// use knowledge_search::VectorStore;
/// use std::collections::BTreeSet;
/// fn cannot_spend_twice(store: VectorStore, eligible: &BTreeSet<String>) {
///     let first = store.query(eligible, &[1.0, 0.0]).unwrap();
///     let second = store.query(eligible, &[1.0, 0.0]).unwrap();
/// }
/// ```
pub struct VectorResult {
    ranked: Vec<(String, f32)>,
    file: File,
    pin: Pin,
    store: VectorStore,
}
impl std::ops::Deref for VectorResult {
    type Target = Vec<(String, f32)>;
    fn deref(&self) -> &Self::Target {
        &self.ranked
    }
}
impl VectorResult {
    pub fn into_store(self) -> VectorStore {
        let Self {
            ranked,
            file,
            pin,
            store,
        } = self;
        drop(ranked);
        drop(file);
        drop(pin);
        store
    }
}

pub struct Pin {
    _file: File,
    _work: std::sync::Arc<Work>,
}

fn open_lock(path: &Path) -> Result<File, VectorError> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(io)
}
fn scope_lock(root: &Path, exclusive: bool) -> Result<File, VectorError> {
    let file = open_lock(&root.join("LOCK"))?;
    let locked = if exclusive {
        fs2::FileExt::try_lock_exclusive(&file)
    } else {
        fs2::FileExt::try_lock_shared(&file)
    };
    locked.map_err(|_| VectorError::Pinned)?;
    Ok(file)
}

pub fn import_stored_segment(
    root: &Path,
    identity: &ModelIdentity,
    rows: &[VectorRow],
    segment: &[u8],
    grant: &AccountGrant,
) -> Result<VectorStore, VectorError> {
    VectorStore::import_stored(root, identity, rows, segment, grant)
}

/// Native writes are counted on each successful syscall, including a partial write
/// before a later native error. No unobserved write_all prefix becomes a refund.
fn write_funded(
    file: &mut File,
    mut bytes: &[u8],
    work: &mut Work,
    deadline: Option<Instant>,
) -> Result<(), VectorError> {
    while !bytes.is_empty() {
        if deadline.is_some_and(|limit| Instant::now() >= limit) {
            return Err(VectorError::Grant(GrantError::Deadline));
        }
        let n = match file.write(&bytes[..bytes.len().min(8192)]) {
            Ok(0) => {
                return Err(io(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "vector write made no progress",
                )));
            }
            Ok(n) => n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(io(error)),
        };
        work.written(n as u64).map_err(invalid)?;
        bytes = &bytes[n..];
    }
    Ok(())
}

fn expected_segment(identity: &ModelIdentity, rows: &[VectorRow]) -> Result<String, VectorError> {
    let mut hash = Sha256::new();
    for row in rows {
        if row.vector.len() != identity.dimension {
            return Err(VectorError::Vector(row.source_id.clone()));
        }
        for value in &row.vector {
            hash.update(value.to_le_bytes());
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn valid_vector(identity: &ModelIdentity, vector: &[f32]) -> bool {
    if vector.len() != identity.dimension || vector.iter().any(|v| !v.is_finite()) {
        return false;
    }
    let squared_norm: f64 = vector.iter().map(|v| f64::from(*v).powi(2)).sum();
    squared_norm.is_finite()
        && squared_norm > 0.0
        && (identity.normalization == VectorNormalization::Raw
            || (squared_norm - 1.0).abs() <= 1e-4)
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let dot: f64 = left
        .iter()
        .zip(right)
        .map(|(a, b)| f64::from(*a) * f64::from(*b))
        .sum();
    let left_norm: f64 = left
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum();
    let right_norm: f64 = right
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum();
    (dot / (left_norm.sqrt() * right_norm.sqrt())) as f32
}

fn private_dir(path: &Path) -> Result<(), VectorError> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && metadata.permissions().mode() & 0o077 == 0 => Ok(()),
        Ok(_) => Err(VectorError::Identity(
            "existing vector root is not a private native directory".into(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(io),
        Err(error) => Err(io(error)),
    }
}

fn validate_identity(identity: &ModelIdentity) -> Result<(), VectorError> {
    if identity.dimension == 0
        || identity.dimension.checked_mul(4).is_none()
        || identity.provider.trim().is_empty()
        || identity.model.trim().is_empty()
        || identity.revision.trim().is_empty()
        || identity.embedding_space.trim().is_empty()
        || identity.preprocessing.trim().is_empty()
        || identity.package_fingerprint.len() != 64
        || !identity
            .package_fingerprint
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(VectorError::Identity(
            "incomplete vector model identity".into(),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn io(error: std::io::Error) -> VectorError {
    VectorError::Io(error)
}

fn invalid(error: impl std::fmt::Display) -> VectorError {
    VectorError::Identity(error.to_string())
}

#[derive(Debug)]
pub enum VectorError {
    Grant(GrantError),
    Vector(String),
    Identity(String),
    Pinned,
    Io(std::io::Error),
}

impl std::fmt::Display for VectorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grant(error) => write!(formatter, "{error}"),
            Self::Vector(source) => write!(formatter, "rejected vector for {source}"),
            Self::Identity(error) => write!(formatter, "{error}"),
            Self::Pinned => formatter.write_str("generation is pinned or current"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for VectorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Grant(error) => Some(error),
            _ => None,
        }
    }
}
