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
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
mod rows;
pub use rows::VectorBuildPlan;
use rows::{Component, JsonBuffer, Output, hash_file, read_row};

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

/// Embedding-input modality; original media/type remains source-owned. This never
/// selects or authorizes a native processor (e.g. PDF-derived text is a text input).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorModality {
    Text,
    Image,
    Audio,
    Video,
}

/// Complete produced-representation data identity, not a source or execution grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepresentationIdentity {
    pub digest: String,
    pub processor: String,
    pub modality: VectorModality,
}
/// Row facts for actual caller eligibility BEFORE distance/candidate limiting.
/// The callback is not authority; native hosts retain their actual opaque owners.
#[derive(Clone, Copy)]
pub struct VectorCandidate<'a> {
    pub source_id: &'a str,
    pub source_digest: &'a str,
    pub representation: &'a RepresentationIdentity,
    pub locator: &'a str,
}
#[derive(Clone, Debug)]
pub struct VectorRow {
    pub source_id: String,
    pub source_digest: String,
    pub representation: RepresentationIdentity,
    pub locator: String,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u32,
    identity: ModelIdentity,
    rows: u64,
    metadata_sha256: String,
    metadata_bytes: u64,
    max_row_bytes: u64,
    segment_sha256: String,
    segment_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowMeta {
    source_id: String,
    source_digest: String,
    representation: RepresentationIdentity,
    locator: String,
    offset: u64,
    /// Hash of this exact row, allowing eligibility before any payload access.
    sha256: String,
}

impl RowMeta {
    fn candidate(&self) -> VectorCandidate<'_> {
        VectorCandidate {
            source_id: &self.source_id,
            source_digest: &self.source_digest,
            representation: &self.representation,
            locator: &self.locator,
        }
    }
}
fn valid_representation(identity: &RepresentationIdentity) -> bool {
    [&identity.digest, &identity.processor].iter().all(|s| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    })
}

/// Native descriptor identity only; the enclosing native caller must additionally
/// retain its actual current source/policy/Claim and generated-cache write intent.
struct NativeReader {
    root: File,
    directory: File,
    segment: File,
    metadata: File,
    metadata_stamp: NativeStamp,
    header: File,
    header_stamp: NativeStamp,
    pin: File,
    segment_bytes: u64,
}
impl NativeReader {
    fn open(
        root: &Path,
        generation: &str,
        created: Option<(File, File, File)>,
    ) -> Result<Self, VectorError> {
        use std::os::unix::fs::OpenOptionsExt;
        let root = fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
            )
            .open(root)
            .map_err(io)?;
        let directory = Self::component(&root, generation, true)?;
        let pin = Self::component(&directory, "READERS", false)?;
        fs2::FileExt::try_lock_shared(&pin).map_err(|_| VectorError::Pinned)?;
        let (segment, metadata, header) = match created {
            Some(files) => files,
            None => (
                Self::component(&directory, "segment.bin", false)?,
                Self::component(&directory, "rows.bin", false)?,
                Self::component(&directory, "manifest.json", false)?,
            ),
        };
        let metadata_stamp = NativeStamp::of(&metadata)?;
        let header_stamp = NativeStamp::of(&header)?;
        if !segment.metadata().map_err(io)?.is_file() {
            return Err(VectorError::Identity("nonregular vector segment".into()));
        }
        let segment_bytes = segment.metadata().map_err(io)?.len();
        let reader = Self {
            root,
            directory,
            segment,
            metadata,
            metadata_stamp,
            header,
            header_stamp,
            pin,
            segment_bytes,
        };
        reader.check(generation)?;
        Ok(reader)
    }
    fn component(parent: &File, name: &str, directory: bool) -> Result<File, VectorError> {
        let mut flags =
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW;
        if directory {
            flags |= rustix::fs::OFlags::DIRECTORY;
        }
        rustix::fs::openat(parent, name, flags, rustix::fs::Mode::empty())
            .map(File::from)
            .map_err(|e| io(e.into()))
    }
    fn open_component(&self, name: &str) -> Result<File, VectorError> {
        Self::component(&self.directory, name, false)
    }
    fn check(&self, generation: &str) -> Result<(), VectorError> {
        Self::entry(&self.root, generation, &self.directory)?;
        Self::entry(&self.directory, "READERS", &self.pin)?;
        Self::entry(&self.directory, "segment.bin", &self.segment)?;
        Self::entry(&self.directory, "rows.bin", &self.metadata)?;
        Self::entry(&self.directory, "manifest.json", &self.header)?;
        if NativeStamp::of(&self.header)? != self.header_stamp {
            return Err(invalid("vector header version drift"));
        }
        if NativeStamp::of(&self.metadata)? != self.metadata_stamp {
            return Err(invalid("vector metadata version drift"));
        }
        if self.segment.metadata().map_err(io)?.len() != self.segment_bytes {
            return Err(VectorError::Identity("vector segment length drift".into()));
        }
        Ok(())
    }
    fn entry(parent: &File, name: &str, held: &File) -> Result<(), VectorError> {
        use std::os::unix::fs::MetadataExt;
        let expected = held.metadata().map_err(io)?;
        let actual = rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|e| io(e.into()))?;
        #[cfg(target_os = "macos")]
        let same_device = expected.dev() as u32 == actual.st_dev as u32;
        #[cfg(not(target_os = "macos"))]
        let same_device = expected.dev() == actual.st_dev;
        if !same_device || expected.ino() != actual.st_ino {
            return Err(VectorError::Identity(
                "admitted vector descriptor namespace drift".into(),
            ));
        }
        Ok(())
    }
}

#[derive(PartialEq, Eq)]
struct NativeStamp(u64, u64, u64, i64, i64, i64, i64);
impl NativeStamp {
    fn of(file: &File) -> Result<Self, VectorError> {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata().map_err(io)?;
        if !m.is_file() {
            return Err(invalid("nonregular row metadata"));
        }
        Ok(Self(
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        ))
    }
}

enum Source {
    Native(NativeReader),
    Directory(std::sync::Arc<dyn knowledge_retrieval::ByteDirectory>),
}
impl Source {
    fn check(&self, generation: &str) -> Result<(), VectorError> {
        match self {
            Self::Native(reader) => reader.check(generation),
            Self::Directory(directory) => directory
                .check()
                .map_err(|_| invalid("admitted vector source refused")),
        }
    }
}
pub struct VectorStore {
    _root: PathBuf,
    generation: String,
    manifest: Manifest,
    /// Exact used native descriptors and real reader pin, never a hash-then-reopen pathname.
    metadata: Component,
    segment: Component,
    native: Source,
    /// Actual affine envelope; the original grant and all its clones are consumed.
    _work: std::sync::Arc<Work>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuildMode {
    CompatibilityCurrent,
    Prepared,
}

impl VectorStore {
    fn from_native(
        root: &Path,
        generation: String,
        manifest: Manifest,
        native: NativeReader,
        work: Work,
    ) -> Result<Self, VectorError> {
        let metadata = Component::Native(native.metadata.try_clone().map_err(io)?);
        let segment = Component::Native(native.segment.try_clone().map_err(io)?);
        Ok(Self {
            _root: root.to_path_buf(),
            generation,
            manifest,
            metadata,
            segment,
            native: Source::Native(native),
            _work: std::sync::Arc::new(work),
        })
    }
    /// Open actual admitted generic components. The directory must retain its real source,
    /// source-policy/Claim checks, descriptors and own cost; it is not a path/digest authority.
    /// The exact selected vector subgeneration and complete current model space are mandatory.
    pub fn open_directory(
        directory: std::sync::Arc<dyn knowledge_retrieval::ByteDirectory>,
        generation: &str,
        identity: &ModelIdentity,
        grant: &AccountGrant,
    ) -> Result<Self, VectorError> {
        validate_identity(identity)?;
        grant.covers(64 * 1024, 0).map_err(VectorError::Grant)?;
        let work = std::sync::Arc::new(grant.take().map_err(VectorError::Grant)?);
        Self::open_directory_held(&directory, generation, identity, &work, None)
    }
    /// Use an actual already-paid enclosing lifetime, without taking its resource owner away.
    /// The caller retains BOTH actual directory and Work on failure; live stores/pins retain the
    /// SAME Work until their last consumer. This reserves nothing and grants no source permission.
    /// The enclosing host must prepay its complete simultaneous peak and serialize consumers or
    /// include their combined peak; sharing this owner does not multiply its spendable memory.
    /// An enclosing immutable tuple may bind its exact row count before row-table access; this
    /// is DATA consistency/admission, not a source permission or a new corpus ceiling.
    pub fn open_directory_held(
        directory: &std::sync::Arc<dyn knowledge_retrieval::ByteDirectory>,
        generation: &str,
        identity: &ModelIdentity,
        work: &std::sync::Arc<Work>,
        expected_rows: Option<u64>,
    ) -> Result<Self, VectorError> {
        validate_identity(identity)?;
        if work.remaining().map_err(invalid)?.memory_bytes < 64 * 1024 {
            return Err(VectorError::Grant(GrantError::TooSmall));
        }
        if generation.len() != 64 || !generation.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("invalid selected generation"));
        }
        let source = Source::Directory(directory.clone());
        source.check(generation)?;
        let header = Component::Directory(
            directory
                .open("manifest.json")
                .map_err(|_| invalid("missing admitted vector manifest"))?,
        );
        let length = header.len().map_err(io)?;
        if length > work.remaining().map_err(invalid)?.memory_bytes / 8 {
            return Err(VectorError::Grant(GrantError::TooSmall));
        }
        let mut bytes = vec![0; usize::try_from(length).map_err(invalid)?];
        header.read_exact_at(&mut bytes, 0).map_err(io)?;
        source.check(generation)?;
        if sha256(&bytes) != generation {
            return Err(invalid("selected vector generation hash mismatch"));
        }
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(invalid)?;
        check_manifest(&manifest, identity, work)?;
        if expected_rows.is_some_and(|rows| manifest.rows != rows) {
            return Err(invalid(
                "vector row count differs from actual enclosing tuple",
            ));
        }
        let metadata = Component::Directory(
            directory
                .open("rows.bin")
                .map_err(|_| invalid("missing admitted row table"))?,
        );
        let segment = Component::Directory(
            directory
                .open("segment.bin")
                .map_err(|_| invalid("missing admitted float segment"))?,
        );
        if hash_file(&metadata, manifest.metadata_bytes, || {
            source.check(generation)
        })? != manifest.metadata_sha256
            || hash_file(&segment, manifest.segment_bytes, || {
                source.check(generation)
            })? != manifest.segment_sha256
        {
            return Err(invalid("admitted vector component hash mismatch"));
        }
        source.check(generation)?;
        Ok(Self {
            _root: PathBuf::new(),
            generation: generation.to_owned(),
            manifest,
            metadata,
            segment,
            native: source,
            _work: work.clone(),
        })
    }
    /// Compatibility input slice; native producers use the owned, bounded stream below.
    /// The caller retains separate input-buffer funding if it keeps the borrowed slice.
    pub fn build(
        root: &Path,
        identity: &ModelIdentity,
        rows: &[VectorRow],
        grant: &AccountGrant,
        deadline: Option<Instant>,
    ) -> Result<Self, VectorError> {
        let plan = row_plan(rows)?;
        Self::build_stream(
            root,
            identity,
            &plan,
            rows.iter().map(|row| {
                if !valid_vector(identity, &row.vector) {
                    return Err(VectorError::Vector(row.source_id.clone()));
                }
                Ok(row.clone())
            }),
            grant,
            deadline,
        )
    }

    /// Prepare immutable data ONLY; do not select a vector CURRENT independently of the
    /// caller's complete lexical/vector/source tuple. The same actual paid owner follows exports.
    pub fn prepare(
        root: &Path,
        identity: &ModelIdentity,
        rows: &[VectorRow],
        grant: &AccountGrant,
        deadline: Option<Instant>,
    ) -> Result<PreparedVectors, VectorError> {
        let plan = row_plan(rows)?;
        Self::prepare_stream(
            root,
            identity,
            &plan,
            rows.iter().map(|row| {
                if !valid_vector(identity, &row.vector) {
                    return Err(VectorError::Vector(row.source_id.clone()));
                }
                Ok(row.clone())
            }),
            grant,
            deadline,
        )
    }
    /// Streaming preparation under the complete existing envelope, before producer advancement.
    pub fn prepare_stream<I>(
        root: &Path,
        identity: &ModelIdentity,
        plan: &VectorBuildPlan,
        rows: I,
        grant: &AccountGrant,
        deadline: Option<Instant>,
    ) -> Result<PreparedVectors, VectorError>
    where
        I: IntoIterator<Item = Result<VectorRow, VectorError>>,
    {
        Ok(PreparedVectors {
            store: Self::build_stream_mode(
                root,
                identity,
                plan,
                rows,
                grant,
                deadline,
                BuildMode::Prepared,
            )?,
        })
    }

    /// Stream one admitted row at a time; neither float payload nor corpus metadata is assembled.
    /// Plan facts grant nothing: the actual complete same-account envelope is consumed BEFORE IO.
    /// This standalone CURRENT is a compatibility seam, not a native whole-tuple publisher.
    pub fn build_stream<I>(
        root: &Path,
        identity: &ModelIdentity,
        plan: &VectorBuildPlan,
        rows: I,
        grant: &AccountGrant,
        deadline: Option<Instant>,
    ) -> Result<Self, VectorError>
    where
        I: IntoIterator<Item = Result<VectorRow, VectorError>>,
    {
        Self::build_stream_mode(
            root,
            identity,
            plan,
            rows,
            grant,
            deadline,
            BuildMode::CompatibilityCurrent,
        )
    }
    fn build_stream_mode<I>(
        root: &Path,
        identity: &ModelIdentity,
        plan: &VectorBuildPlan,
        rows: I,
        grant: &AccountGrant,
        deadline: Option<Instant>,
        mode: BuildMode,
    ) -> Result<Self, VectorError>
    where
        I: IntoIterator<Item = Result<VectorRow, VectorError>>,
    {
        validate_identity(identity)?;
        let row_bytes = (identity.dimension as u64)
            .checked_mul(4)
            .ok_or_else(|| invalid("row length overflow"))?;
        let bytes = row_bytes
            .checked_mul(plan.rows)
            .ok_or_else(|| invalid("segment size overflow"))?;
        if plan.max_row_bytes == 0 || plan.max_row_bytes > u32::MAX as u64 {
            return Err(invalid("invalid row metadata frame plan"));
        }
        let header = identity_bound(identity)?;
        let knowledge_core::jobs::Reservation {
            memory_bytes: memory,
            temporary_bytes: storage,
        } = plan.requirement(identity)?;
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
        let mut payload = Output::new(create_component(&stage.join("segment.bin"))?, bytes);
        let mut table = Output::new(
            create_component(&stage.join("rows.bin"))?,
            plan.metadata_bytes,
        );
        let mut encoder = JsonBuffer::new(plan.max_row_bytes)?;
        let mut count = 0u64;
        let mut maximum = 0u64;
        for row in rows {
            if deadline.is_some_and(|limit| Instant::now() >= limit) {
                return Err(VectorError::Grant(GrantError::Deadline));
            }
            if count >= plan.rows {
                return Err(invalid("producer exceeded complete row count"));
            }
            let row = row?;
            if !valid_vector(identity, &row.vector)
                || row.source_id.is_empty()
                || row.source_digest.len() != 64
                || !row.source_digest.bytes().all(|b| b.is_ascii_hexdigit())
                || !valid_representation(&row.representation)
                || row.locator.is_empty()
            {
                return Err(VectorError::Vector(row.source_id));
            }
            let mut row_hash = Sha256::new();
            let mut encoded = [0u8; 8192];
            for values in row.vector.chunks(encoded.len() / 4) {
                for (slot, value) in encoded.as_chunks_mut::<4>().0.iter_mut().zip(values) {
                    slot.copy_from_slice(&value.to_le_bytes());
                }
                let block = &encoded[..values.len() * 4];
                row_hash.update(block);
                payload.push(block, &mut work, deadline)?;
            }
            let meta = RowMeta {
                source_id: row.source_id,
                source_digest: row.source_digest,
                representation: row.representation,
                locator: row.locator,
                offset: count
                    .checked_mul(row_bytes)
                    .ok_or_else(|| invalid("row offset overflow"))?,
                sha256: format!("{:x}", row_hash.finalize()),
            };
            let frame = encoder.encode(&meta)?;
            maximum = maximum.max(frame.len() as u64);
            table.push(
                &(u32::try_from(frame.len()).map_err(invalid)?).to_le_bytes(),
                &mut work,
                deadline,
            )?;
            table.push(frame, &mut work, deadline)?;
            count += 1;
        }
        if count != plan.rows {
            return Err(invalid("producer ended before complete row count"));
        }
        let (segment, segment_sha256, segment_bytes) = payload.finish(&mut work, deadline)?;
        let (row_file, metadata_sha256, metadata_bytes) = table.finish(&mut work, deadline)?;
        let manifest = Manifest {
            format_version: 3,
            identity: identity.clone(),
            rows: count,
            metadata_sha256,
            metadata_bytes,
            max_row_bytes: maximum,
            segment_sha256,
            segment_bytes,
        };
        let metadata = serde_json::to_vec(&manifest).map_err(invalid)?;
        if metadata.len() as u64 > header {
            return Err(invalid("manifest exceeds complete plan"));
        }
        let generation = sha256(&metadata);
        let directory = root.join(&generation);
        if directory.exists() {
            return Err(invalid("generation already exists"));
        }
        let mut metadata_file = create_component(&stage.join("manifest.json"))?;
        write_funded(&mut metadata_file, &metadata, &mut work, deadline)?;
        metadata_file.sync_all().map_err(io)?;
        drop(metadata);
        let marker = create_component(&stage.join("READERS"))?;
        marker.sync_all().map_err(io)?;
        drop(marker);
        fs::rename(&stage, &directory).map_err(io)?;
        let native =
            NativeReader::open(root, &generation, Some((segment, row_file, metadata_file)))?;
        if deadline.is_some_and(|limit| Instant::now() >= limit) {
            return Err(VectorError::Grant(GrantError::Deadline));
        }
        if mode == BuildMode::Prepared {
            return Self::from_native(root, generation, manifest, native, work);
        }
        let temporary = root.join("CURRENT.next");
        let mut pointer = create_component(&temporary)?;
        write_funded(&mut pointer, generation.as_bytes(), &mut work, deadline)?;
        pointer.sync_all().map_err(io)?;
        drop(pointer);
        native.check(&generation)?;
        fs::rename(&temporary, root.join("CURRENT")).map_err(io)?;
        Self::from_native(root, generation, manifest, native, work)
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
        let native = NativeReader::open(root, &generation, None)?;
        drop(scope);
        let file = native.header.try_clone().map_err(io)?;
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
        check_manifest(&manifest, identity, &work)?;
        if hash_file(
            &Component::Native(native.segment.try_clone().map_err(io)?),
            manifest.segment_bytes,
            || native.check(&generation),
        )? != manifest.segment_sha256
            || hash_file(
                &Component::Native(native.metadata.try_clone().map_err(io)?),
                manifest.metadata_bytes,
                || native.check(&generation),
            )? != manifest.metadata_sha256
        {
            return Err(invalid("vector component hash mismatch"));
        }
        native.check(&generation)?;
        Self::from_native(root, generation, manifest, native, work)
    }

    /// Compatibility byte-slice seam, NOT an archive reader or activation grant.
    /// Decode just one paid row at a time; generic held archive IO is a separate adapter.
    pub fn import_stored(
        root: &Path,
        identity: &ModelIdentity,
        rows: &[VectorRow],
        segment: &[u8],
        grant: &AccountGrant,
    ) -> Result<Self, VectorError> {
        validate_identity(identity)?;
        let row_bytes = identity
            .dimension
            .checked_mul(4)
            .ok_or_else(|| invalid("row size overflow"))?;
        if rows.len().checked_mul(row_bytes) != Some(segment.len()) {
            return Err(invalid("stored segment length mismatch"));
        }
        let plan = row_plan(rows)?;
        let mut checked = false;
        let producer = rows.iter().enumerate().map(|(number, row)| {
            if !checked {
                if sha256(segment) != expected_segment(identity, rows)? {
                    return Err(invalid("stored segment does not match the rows"));
                }
                checked = true;
            }
            let start = number
                .checked_mul(row_bytes)
                .ok_or_else(|| invalid("row offset overflow"))?;
            let vector = segment[start..start + row_bytes]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            Ok(VectorRow {
                source_id: row.source_id.clone(),
                source_digest: row.source_digest.clone(),
                representation: row.representation.clone(),
                locator: row.locator.clone(),
                vector,
            })
        });
        Self::build_stream(root, identity, &plan, producer, grant, None)
    }

    /// Rank DISTINCT eligible files by their best exact row, never limit fragments first.
    pub fn query(
        self,
        eligible: &BTreeSet<String>,
        query: &[f32],
    ) -> Result<VectorResult, VectorError> {
        self.query_filtered(query, eligible.len().max(1), |source| {
            Ok(eligible.contains(source))
        })
    }

    /// Exact whole-corpus scan with paged metadata and bounded distinct-file output.
    /// The predicate runs before any vector payload read/distance. A native caller must
    /// supply actual current source/tag eligibility, not treat this data predicate as a grant.
    pub fn query_filtered<F>(
        self,
        query: &[f32],
        limit: usize,
        mut eligible: F,
    ) -> Result<VectorResult, VectorError>
    where
        F: FnMut(&str) -> Result<bool, VectorError>,
    {
        self.query_rows(query, limit, |row| eligible(row.source_id))
    }
    /// Full dependency metadata is available BEFORE distance. Native callers bind it
    /// to their current real source/representation/tag scope, never a bare ID grant.
    pub fn query_rows<F>(
        self,
        query: &[f32],
        limit: usize,
        mut eligible: F,
    ) -> Result<VectorResult, VectorError>
    where
        F: FnMut(VectorCandidate<'_>) -> Result<bool, VectorError>,
    {
        if !valid_vector(&self.manifest.identity, query) {
            return Err(VectorError::Vector("query".into()));
        }
        if limit == 0 {
            return Err(invalid("zero distinct-file limit"));
        }
        let maximum = limit.min(usize::try_from(self.manifest.rows).map_err(invalid)?);
        if reader_need(&self.manifest, maximum)?
            > self._work.remaining().map_err(invalid)?.memory_bytes
        {
            return Err(VectorError::Grant(GrantError::TooSmall));
        }
        self.native.check(&self.generation)?;
        let file = self.segment.try_clone().map_err(io)?;
        let row_bytes = self
            .manifest
            .identity
            .dimension
            .checked_mul(4)
            .ok_or_else(|| invalid("row size overflow"))?;
        let mut winners = std::collections::BTreeMap::<String, (f32, RowMeta)>::new();
        let mut order = BTreeSet::<RankKey>::new();
        let mut offset = 0u64;
        let mut bytes = vec![0; row_bytes];
        for number in 0..self.manifest.rows {
            self.native.check(&self.generation)?;
            let row = read_row(
                &self.metadata,
                &mut offset,
                self.manifest.metadata_bytes,
                self.manifest.max_row_bytes,
            )?;
            self.native.check(&self.generation)?;
            if row.offset
                != number
                    .checked_mul(row_bytes as u64)
                    .ok_or_else(|| invalid("row offset overflow"))?
                || row.sha256.len() != 64
                || row.source_digest.len() != 64
                || row.source_id.is_empty()
                || row.locator.is_empty()
                || !valid_representation(&row.representation)
            {
                return Err(invalid("invalid paged row identity or order"));
            }
            if !eligible(row.candidate())? {
                continue;
            }
            self.native.check(&self.generation)?;
            file.read_exact_at(&mut bytes, row.offset).map_err(io)?;
            self.native.check(&self.generation)?;
            if sha256(&bytes) != row.sha256 {
                return Err(invalid("eligible row hash mismatch"));
            }
            let vector: Vec<f32> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            if !valid_vector(&self.manifest.identity, &vector) {
                return Err(VectorError::Vector(row.source_id));
            }
            let score = cosine(query, &vector);
            let rank = RankKey {
                score,
                source: row.source_id.clone(),
            };
            if let Some((old, _)) = winners.get(&row.source_id) {
                if score.total_cmp(old) != std::cmp::Ordering::Greater {
                    continue;
                }
                order.remove(&RankKey {
                    score: *old,
                    source: row.source_id.clone(),
                });
            } else if winners.len() == maximum {
                let worst = order
                    .first()
                    .ok_or_else(|| invalid("rank table contradiction"))?;
                if rank <= *worst {
                    continue;
                }
                let removed = order
                    .pop_first()
                    .ok_or_else(|| invalid("rank table contradiction"))?;
                winners.remove(&removed.source);
            }
            order.insert(rank);
            winners.insert(row.source_id.clone(), (score, row));
        }
        if offset != self.manifest.metadata_bytes {
            return Err(invalid("row count does not cover complete metadata"));
        }
        self.native.check(&self.generation)?;
        let mut evidence = Vec::with_capacity(winners.len());
        for (score, row) in winners.into_values() {
            evidence.push(VectorEvidence {
                source_id: row.source_id,
                source_digest: row.source_digest,
                representation: row.representation,
                locator: row.locator,
                score,
            });
        }
        evidence.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(a.source_id.cmp(&b.source_id))
        });
        let ranked = evidence
            .iter()
            .map(|hit| (hit.source_id.clone(), hit.score))
            .collect();
        Ok(VectorResult {
            ranked,
            evidence,
            query: query.to_vec(),
            file,
            store: self,
        })
    }

    pub fn pin(&self) -> Result<Pin, VectorError> {
        let work = AccountGrant::reserve(
            knowledge_core::jobs::Reservation {
                memory_bytes: 64 * 1024,
                temporary_bytes: 0,
            },
            knowledge_core::admission::Origin::BeforeVerification,
        )
        .map_err(VectorError::Grant)?
        .take()
        .map_err(VectorError::Grant)?;
        self.native.check(&self.generation)?;
        let resource = match &self.native {
            Source::Native(reader) => {
                let file = reader.open_component("READERS")?;
                fs2::FileExt::try_lock_shared(&file).map_err(|_| VectorError::Pinned)?;
                PinResource::Native { _file: file }
            }
            Source::Directory(directory) => PinResource::Directory {
                _directory: directory.clone(),
            },
        };
        self.native.check(&self.generation)?;
        Ok(Pin {
            _resource: resource,
            _work: work,
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
                || !["manifest.json", "segment.bin", "rows.bin", "READERS"]
                    .iter()
                    .any(|name| entry.file_name() == *name)
            {
                return Err(VectorError::Identity(
                    "unrecognized generation files retained".into(),
                ));
            }
        }
        for name in ["manifest.json", "segment.bin", "rows.bin", "READERS"] {
            fs::remove_file(directory.join(name)).map_err(io)?;
        }
        fs::remove_dir(&directory).map_err(io)?;
        Ok(())
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }
}

/// Immutable unpublished vector artifacts, not permission to publish a native tuple.
/// No independent vector CURRENT is switched. Actual native hosts retain source/model
/// guards and separately fund their complete consumer/publication/delivery envelope.
pub struct PreparedVectors {
    store: VectorStore,
}
impl PreparedVectors {
    pub fn generation(&self) -> &str {
        self.store.generation()
    }
    pub fn check(&self) -> Result<(), VectorError> {
        self.store.native.check(&self.store.generation)
    }
    /// Clone an exact created descriptor, retaining its actual paid/pinned preparation owner.
    /// Only the three known immutable members are available; the caller pays its read buffers.
    pub fn component(
        self: &std::sync::Arc<Self>,
        name: &str,
    ) -> Result<PreparedComponent, VectorError> {
        self.check()?;
        let Source::Native(native) = &self.store.native else {
            return Err(invalid("not a native prepared generation"));
        };
        let file = match name {
            "manifest.json" => &native.header,
            "rows.bin" => &native.metadata,
            "segment.bin" => &native.segment,
            _ => return Err(invalid("unknown prepared member")),
        }
        .try_clone()
        .map_err(io)?;
        self.check()?;
        Ok(PreparedComponent {
            file,
            position: 0,
            owner: self.clone(),
        })
    }
}
/// Streaming exact member; FD dies before its source/reader/pin/credit owner.
pub struct PreparedComponent {
    file: File,
    position: u64,
    owner: std::sync::Arc<PreparedVectors>,
}
impl Read for PreparedComponent {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        use std::os::unix::fs::FileExt;
        self.owner.check().map_err(std::io::Error::other)?;
        let count = self.file.read_at(output, self.position)?;
        self.owner.check().map_err(std::io::Error::other)?;
        self.position = self
            .position
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("prepared member offset overflow"))?;
        Ok(count)
    }
}

/// Best exact row for a ranked file; more locations stream through `visit_evidence`.
#[derive(Clone, Debug)]
pub struct VectorEvidence {
    pub source_id: String,
    pub source_digest: String,
    pub representation: RepresentationIdentity,
    pub locator: String,
    pub score: f32,
}
struct RankKey {
    score: f32,
    source: String,
}
impl PartialEq for RankKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}
impl Eq for RankKey {}
impl PartialOrd for RankKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RankKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .total_cmp(&other.score)
            .then_with(|| other.source.cmp(&self.source))
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
    evidence: Vec<VectorEvidence>,
    query: Vec<f32>,
    file: Component,
    store: VectorStore,
}
impl std::ops::Deref for VectorResult {
    type Target = Vec<(String, f32)>;
    fn deref(&self) -> &Self::Target {
        &self.ranked
    }
}
impl VectorResult {
    /// Recheck the retained actual source/Claim owner before every controllable delivery chunk.
    pub fn check(&self) -> Result<(), VectorError> {
        self.store.native.check(&self.store.generation)
    }
    pub fn evidence(&self) -> &[VectorEvidence] {
        &self.evidence
    }
    /// Stream all row locations of winning files under the same pin; the native
    /// caller rechecks actual source scope before delivery and pays for collection.
    pub fn visit_evidence(
        &self,
        visit: impl FnMut(&VectorEvidence) -> Result<(), VectorError>,
    ) -> Result<(), VectorError> {
        self.visit_rows(|_| Ok(true), visit)
    }
    /// Re-admit each representation under current source/tag/dependency scope BEFORE
    /// reading its payload or computing distance; collect only under separate actual credit.
    pub fn visit_rows(
        &self,
        mut eligible: impl FnMut(VectorCandidate<'_>) -> Result<bool, VectorError>,
        mut visit: impl FnMut(&VectorEvidence) -> Result<(), VectorError>,
    ) -> Result<(), VectorError> {
        let selected: std::collections::BTreeMap<_, _> = self
            .evidence
            .iter()
            .map(|hit| (hit.source_id.as_str(), hit))
            .collect();
        let mut offset = 0;
        let mut bytes = vec![
            0;
            self.store
                .manifest
                .identity
                .dimension
                .checked_mul(4)
                .ok_or_else(|| invalid("row size overflow"))?
        ];
        for _ in 0..self.store.manifest.rows {
            self.store.native.check(&self.store.generation)?;
            let row = read_row(
                &self.store.metadata,
                &mut offset,
                self.store.manifest.metadata_bytes,
                self.store.manifest.max_row_bytes,
            )?;
            self.store.native.check(&self.store.generation)?;
            if selected
                .get(row.source_id.as_str())
                .is_none_or(|winner| winner.source_digest != row.source_digest)
                || !eligible(row.candidate())?
            {
                continue;
            }
            self.file
                .read_exact_at(&mut bytes, row.offset)
                .map_err(io)?;
            self.store.native.check(&self.store.generation)?;
            if sha256(&bytes) != row.sha256 {
                return Err(invalid("eligible evidence row hash mismatch"));
            }
            let vector: Vec<f32> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            if !valid_vector(&self.store.manifest.identity, &vector) {
                return Err(VectorError::Vector(row.source_id));
            }
            visit(&VectorEvidence {
                source_id: row.source_id,
                source_digest: row.source_digest,
                representation: row.representation,
                locator: row.locator,
                score: cosine(&self.query, &vector),
            })?;
            self.store.native.check(&self.store.generation)?;
        }
        if offset != self.store.manifest.metadata_bytes {
            return Err(invalid("incomplete evidence metadata"));
        }
        Ok(())
    }
    pub fn into_store(self) -> VectorStore {
        let Self {
            ranked,
            evidence,
            query,
            file,
            store,
        } = self;
        drop(ranked);
        drop(evidence);
        drop(query);
        drop(file);
        store
    }
}

enum PinResource {
    Native {
        _file: File,
    },
    Directory {
        _directory: std::sync::Arc<dyn knowledge_retrieval::ByteDirectory>,
    },
}
pub struct Pin {
    _resource: PinResource,
    _work: Work,
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

fn check_manifest(
    manifest: &Manifest,
    identity: &ModelIdentity,
    work: &Work,
) -> Result<(), VectorError> {
    if &manifest.identity != identity {
        return Err(VectorError::Identity("model identity mismatch".into()));
    }
    if manifest.format_version != 3
        || manifest.segment_bytes
            != manifest
                .rows
                .checked_mul(identity.dimension as u64)
                .and_then(|n| n.checked_mul(4))
                .ok_or_else(|| invalid("manifest segment length overflow"))?
        || (manifest.rows == 0 && (manifest.metadata_bytes != 0 || manifest.max_row_bytes != 0))
        || (manifest.rows != 0 && manifest.max_row_bytes == 0)
    {
        return Err(invalid("incompatible paged vector manifest"));
    }
    if reader_need(manifest, 0)? > work.remaining().map_err(invalid)?.memory_bytes {
        return Err(VectorError::Grant(GrantError::TooSmall));
    }
    Ok(())
}

fn row_plan(rows: &[VectorRow]) -> Result<VectorBuildPlan, VectorError> {
    let mut plan = VectorBuildPlan {
        rows: 0,
        metadata_bytes: 0,
        max_row_bytes: 1,
    };
    for row in rows {
        plan.include(
            VectorCandidate {
                source_id: &row.source_id,
                source_digest: &row.source_digest,
                representation: &row.representation,
                locator: &row.locator,
            },
            1,
        )?;
    }
    Ok(plan)
}

fn create_component(path: &Path) -> Result<File, VectorError> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(io)
}
fn identity_bound(identity: &ModelIdentity) -> Result<u64, VectorError> {
    [
        &identity.provider,
        &identity.model,
        &identity.revision,
        &identity.package_fingerprint,
        &identity.embedding_space,
        &identity.preprocessing,
    ]
    .iter()
    .try_fold(4096u64, |sum, s| {
        sum.checked_add((s.len() as u64).checked_mul(6)?)
    })
    .ok_or_else(|| invalid("identity serialization bound overflow"))
}
fn reader_need(manifest: &Manifest, files: usize) -> Result<u64, VectorError> {
    reader_need_parts(&manifest.identity, manifest.max_row_bytes, files)
}
fn reader_need_parts(
    identity: &ModelIdentity,
    max_row_bytes: u64,
    files: usize,
) -> Result<u64, VectorError> {
    max_row_bytes
        .checked_mul(10)
        .and_then(|n| n.checked_add((identity.dimension as u64).checked_mul(16)?))
        .and_then(|n| n.checked_add(identity_bound(identity).ok()?.checked_mul(8)?))
        .and_then(|n| {
            n.checked_add(
                (files as u64).checked_mul(max_row_bytes.checked_mul(8)?.checked_add(256)?)?,
            )
        })
        .and_then(|n| n.checked_add(65536))
        .ok_or_else(|| invalid("complete reader plan overflow"))
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

impl ModelIdentity {
    /// Apply the SAME dimension/finite/nonzero/declared-normalization oracle used by disk rows.
    /// Native producers call this before exposing a provider vector; identity facts grant nothing.
    pub fn check_vector(&self, vector: &[f32]) -> Result<(), VectorError> {
        if valid_vector(self, vector) {
            Ok(())
        } else {
            Err(VectorError::Identity(
                "provider vector/model normalization".into(),
            ))
        }
    }
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
