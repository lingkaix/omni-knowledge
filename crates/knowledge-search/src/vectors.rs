//! One immutable disk generation of little-endian float32 vectors.
//!
//! The manifest names rows and identity. It does not embed the vectors. A query checks
//! eligibility before reading that row's segment bytes.

use crate::{AccountGrant, GrantError, before_work};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIdentity {
    pub model: String,
    pub dimension: usize,
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
}

pub struct VectorStore {
    root: PathBuf,
    generation: String,
    manifest: Manifest,
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
        let bytes = (rows.len() as u64)
            .saturating_mul(identity.dimension as u64)
            .saturating_mul(4);
        before_work(grant, bytes.max(1), deadline).map_err(VectorError::Grant)?;
        private_dir(root)?;
        let mut segment = Vec::with_capacity(bytes as usize);
        let mut meta = Vec::new();
        let mut seen = BTreeSet::new();
        for row in rows {
            if !seen.insert(row.source_id.clone()) {
                return Err(VectorError::Identity("duplicate source id".into()));
            }
            if row.vector.len() != identity.dimension || !finite_nonzero(&row.vector) {
                return Err(VectorError::Vector(row.source_id.clone()));
            }
            let offset = segment.len() as u64;
            for value in &row.vector {
                segment.extend(value.to_le_bytes());
            }
            meta.push(RowMeta {
                source_id: row.source_id.clone(),
                source_digest: row.source_digest.clone(),
                locator: row.locator.clone(),
                offset,
            });
        }
        let manifest = Manifest {
            identity: identity.clone(),
            rows: meta,
            segment_sha256: sha256(&segment),
            segment_bytes: segment.len() as u64,
        };
        let generation = sha256(serde_json::to_vec(&manifest).map_err(invalid)?.as_slice());
        let directory = root.join(&generation);
        if directory.exists() {
            return Err(VectorError::Identity("generation already exists".into()));
        }
        fs::create_dir(&directory).map_err(io)?;
        fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).map_err(invalid)?,
        )
        .map_err(io)?;
        fs::write(directory.join("segment.bin"), &segment).map_err(io)?;
        let current = root.join("CURRENT");
        let temporary = root.join("CURRENT.next");
        fs::write(&temporary, &generation).map_err(io)?;
        fs::rename(&temporary, &current).map_err(io)?;
        Ok(Self {
            root: root.to_path_buf(),
            generation,
            manifest,
        })
    }

    /// Open the current generation only when its model identity matches.
    pub fn open(root: &Path, identity: &ModelIdentity) -> Result<Self, VectorError> {
        let generation = fs::read_to_string(root.join("CURRENT")).map_err(io)?;
        let generation = generation.trim().to_owned();
        let directory = root.join(&generation);
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(directory.join("manifest.json")).map_err(io)?)
                .map_err(invalid)?;
        let segment = fs::read(directory.join("segment.bin")).map_err(io)?;
        if sha256(&segment) != manifest.segment_sha256
            || segment.len() as u64 != manifest.segment_bytes
        {
            return Err(VectorError::Identity("segment hash mismatch".into()));
        }
        if &manifest.identity != identity {
            return Err(VectorError::Identity("model identity mismatch".into()));
        }
        Ok(Self {
            root: root.to_path_buf(),
            generation,
            manifest,
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
        &self,
        eligible: &BTreeSet<String>,
        query: &[f32],
    ) -> Result<Vec<(String, f32)>, VectorError> {
        if query.len() != self.manifest.identity.dimension || !finite_nonzero(query) {
            return Err(VectorError::Vector("query".into()));
        }
        let mut file =
            File::open(self.root.join(&self.generation).join("segment.bin")).map_err(io)?;
        let mut ranked = Vec::new();
        for row in &self.manifest.rows {
            if !eligible.contains(&row.source_id) {
                continue;
            }
            file.seek(SeekFrom::Start(row.offset)).map_err(io)?;
            let mut bytes = vec![0; self.manifest.identity.dimension * 4];
            file.read_exact(&mut bytes).map_err(io)?;
            let vector: Vec<f32> = bytes
                .chunks(4)
                .map(|chunk| {
                    let mut le = [0; 4];
                    le.copy_from_slice(chunk);
                    f32::from_le_bytes(le)
                })
                .collect();
            if !finite_nonzero(&vector) {
                return Err(VectorError::Vector(row.source_id.clone()));
            }
            ranked.push((row.source_id.clone(), cosine(query, &vector)));
        }
        ranked.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
        Ok(ranked)
    }

    pub fn pin(&self) -> Result<Pin, VectorError> {
        let directory = self.root.join("pins");
        private_dir(&directory)?;
        let path = directory.join(&self.generation);
        fs::write(&path, b"pinned\n").map_err(io)?;
        Ok(Pin { path })
    }

    /// Refuse retirement while a pin exists. Previous generations stay until an unpinned retire.
    pub fn retire(root: &Path, generation: &str) -> Result<(), VectorError> {
        let pin = root.join("pins").join(generation);
        if pin.exists() {
            return Err(VectorError::Pinned);
        }
        let current = fs::read_to_string(root.join("CURRENT")).unwrap_or_default();
        if current.trim() == generation {
            return Err(VectorError::Pinned);
        }
        fs::remove_dir_all(root.join(generation)).map_err(io)?;
        Ok(())
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }
}

pub struct Pin {
    path: PathBuf,
}

impl Drop for Pin {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
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

fn expected_segment(identity: &ModelIdentity, rows: &[VectorRow]) -> Result<String, VectorError> {
    let mut segment = Vec::new();
    for row in rows {
        if row.vector.len() != identity.dimension {
            return Err(VectorError::Vector(row.source_id.clone()));
        }
        for value in &row.vector {
            segment.extend(value.to_le_bytes());
        }
    }
    Ok(sha256(&segment))
}

fn finite_nonzero(vector: &[f32]) -> bool {
    let norm: f64 = vector
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum();
    vector.iter().all(|value| value.is_finite()) && norm.is_finite() && norm > 0.0
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
    fs::create_dir_all(path).map_err(io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(io)?;
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn io(error: std::io::Error) -> VectorError {
    VectorError::Io(error.to_string())
}

fn invalid(error: serde_json::Error) -> VectorError {
    VectorError::Identity(error.to_string())
}

#[derive(Debug)]
pub enum VectorError {
    Grant(GrantError),
    Vector(String),
    Identity(String),
    Pinned,
    Io(String),
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

impl std::error::Error for VectorError {}
