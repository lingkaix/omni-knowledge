//! Funded native readers over verified package ranges, without unpacking or a second engine.
mod directory;
mod held;
use crate::{Control, Error, KernelAdapter, Query, SearchResult, Work};
pub use directory::{StoredDirectory, StoredDirectoryPlan};
pub use held::HeldRangeReaders;
use knowledge_package::{Bounds, FrozenArchive, IndexRequirements, Method};
use retrieval_kernel::{ByteDirectory, SourceGeneration};
use std::collections::BTreeMap;
use std::fs::{File, Metadata};
use std::io;
use std::ops::Range;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use tantivy::HasLen;
use tantivy::directory::{FileHandle, OwnedBytes};

/// Native host's actual selected source and current authorization/version owner.
/// Implement this on the host's existing NativeSource/permit carrier, not on a path or digest DTO.
/// Every check must include current policy, namespace/selection and held/current source version.
/// The source must retain its descriptor and its own cost through the last range or delivery owner.
pub trait ArchiveSource: Send + Sync + 'static {
    /// Recheck the actual current native caller lease. Revocation must refuse, not return old bytes.
    fn check(&self) -> Result<(), Error>;
    /// Borrow the exact admitted regular descriptor. Never resolve or reopen its display hint.
    fn file(&self) -> Result<&File, Error>;
    /// Restrict an already paid read BEFORE allocation/native IO. This mints no authority or
    /// Core promise; hosts with an aggregate data ceiling charge its SAME actual caller here.
    fn admit_read(&self, _bytes: u64) -> Result<(), Error> {
        Ok(())
    }
}

/// Actual native consumer format facts, independent of package metadata and source authorization.
pub struct StoredIdentity(retrieval_kernel::GenerationFormat);
impl StoredIdentity {
    /// Bind the real consuming lock/features/target/vendor fingerprint used by native publication.
    pub fn new(build_identity: &str) -> Result<Self, Error> {
        Ok(Self(retrieval_kernel::generation_format(build_identity)?))
    }
    /// Borrow exact M5 requirements for one selected lexical-only generation.
    /// Combined lexical/vector tuples need their actual complete model space, not this lexical helper.
    pub fn requirements<'a>(&'a self, generation: &'a str) -> IndexRequirements<'a> {
        IndexRequirements {
            kernel_build: &self.0.kernel_build,
            lexical_format: self.0.lexical_format,
            segment_format: self.0.segment_format,
            processor: self.0.processor,
            dictionary: self.0.dictionary,
            generation,
            model_space: None,
        }
    }
}

/// Conservative simultaneous native-reader, table, buffer and output plan; never spending credit.
/// A real Work from the ONE Core account is still required before any native object is opened.
#[derive(Clone, Copy, Debug)]
pub struct StoredPlan {
    /// Upper bound on all native index components together, including manifest and sidecars.
    pub index_bytes: u64,
    /// Upper bound on the owned component/range table, including names and allocator overhead.
    pub table_bytes: u64,
}
impl StoredPlan {
    /// Whole concurrent logical envelope, including opaque native execution and manifest decode peaks.
    /// This is conservative admission, not measured physical RSS or native worker qualification.
    pub fn requirement(self) -> Result<knowledge_core::jobs::Reservation, Error> {
        if self.index_bytes > 512 * 1024 * 1024 {
            return Err(Error::Inventory);
        }
        Ok(knowledge_core::jobs::Reservation {
            memory_bytes: self
                .index_bytes
                .checked_mul(4)
                .and_then(|v| v.checked_add(self.table_bytes))
                .and_then(|v| v.checked_add(256 * 1024 * 1024))
                .ok_or(Error::Inventory)?,
            temporary_bytes: 0,
        })
    }
}

/// Owned native generation whose actual descriptor, native lease and SAME-account Work follow readers.
/// No borrowed package callback, extracted component, or filesystem-only kernel escapes construction.
/// This affine envelope permits one active query/result, never cloned spending against the same Work.
/// ```compile_fail
/// fn duplicate(reader: knowledge_retrieval::StoredGeneration) { let _ = reader.clone(); }
/// ```
pub struct StoredGeneration {
    generation: SourceGeneration,
    backing: Arc<Backing>,
}
impl StoredGeneration {
    /// Verify the real archive through M5, check exact optional-index identities, then open its ranges.
    /// `prefix` names the native generation's archived directory, not a host filesystem destination.
    /// `native` must come from this consuming build/generation, never from manifest trust flags.
    /// `work` is an independent actual lifetime, normally split from the existing real Claim.
    /// Errors preserve package omission, cancellation, drift, waiting and whole-account refusal.
    ///
    /// Initial M5 framing/payload verification and later range refills recheck this same actual
    /// source owner and Control, including its real Claim when native work is job-funded.
    /// A held File or callback supplied from DTO facts cannot create native authority.
    /// Native hosts must still bind the existing source/permit/Claim and retain them through delivery.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        source: Arc<dyn ArchiveSource>,
        native: &IndexRequirements<'_>,
        scope: &str,
        build_identity: &str,
        prefix: &str,
        plan: StoredPlan,
        mut work: Work,
        bounds: &Bounds,
        cancel: &AtomicBool,
        control: &Control,
    ) -> Result<Self, Error> {
        KernelAdapter::initialize_runtime()?;
        let need = plan.requirement()?;
        work.require(need.memory_bytes, 0)?;
        let actual = StoredIdentity::new(build_identity)?;
        let actual = actual.requirements(native.generation);
        if native.kernel_build != actual.kernel_build
            || native.lexical_format != actual.lexical_format
            || native.segment_format != actual.segment_format
            || native.processor != actual.processor
            || native.dictionary != actual.dictionary
            || native.model_space.is_some()
        {
            return Err(Error::Inventory);
        }
        source.check()?;
        control.check()?;
        if prefix.is_empty()
            || prefix.len() > bounds.quotas.max_path_bytes
            || prefix.starts_with('/')
            || prefix
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
        {
            return Err(Error::Inventory);
        }
        let file = source.file()?.try_clone().map_err(|_| Error::Inventory)?;
        let version = Version::of(&file.metadata().map_err(|_| Error::Inventory)?);
        let current = || {
            control
                .check()
                .map_err(|_| knowledge_package::Error::Cancelled)?;
            source.check().map_err(|_| knowledge_package::Error::Drift)
        };
        let mut package_work =
            knowledge_package::Work::guarded_reader(bounds, &mut work.0, cancel, &current)?;
        let mut frozen = FrozenArchive::open(file, &mut package_work)?;
        // Both Result-returning M5 admission operations run before table allocation or native open.
        frozen.index_paths(native)?;
        let mut index_bytes = 0u64;
        let mut table_bytes = 0u64;
        for entry in &frozen.manifest().entries {
            control.check()?;
            source.check()?;
            if entry.role == knowledge_package::Role::Index {
                index_bytes = index_bytes
                    .checked_add(entry.size_bytes)
                    .ok_or(Error::Inventory)?;
            }
            if entry.method == Method::Stored {
                table_bytes = table_bytes
                    .checked_add(
                        (entry.path.as_str().len() as u64)
                            .saturating_mul(4)
                            .saturating_add(512),
                    )
                    .ok_or(Error::Inventory)?;
            }
        }
        if index_bytes > plan.index_bytes || table_bytes > plan.table_bytes {
            return Err(Error::Reservation);
        }
        let mut ranges = BTreeMap::new();
        // All entries have been CRC/digest verified by FrozenArchive; extents come from StoredRange.
        let paths: Vec<_> = frozen
            .manifest()
            .entries
            .iter()
            .filter(|e| e.method == Method::Stored)
            .map(|e| (e.path.clone(), e.role == knowledge_package::Role::Index))
            .collect();
        for (path, index) in &paths {
            control.check()?;
            source.check()?;
            let extent = frozen.with_stored(path, |range| Ok(range.extent()))?;
            usize::try_from(extent.length).map_err(|_| Error::Inventory)?;
            ranges.insert(
                path.as_str().to_owned(),
                AdmittedRange {
                    extent,
                    index: *index,
                },
            );
        }
        frozen.with_indexes(native, |directory| {
            for path in directory.paths() {
                if !path.as_str().starts_with(&format!("{prefix}/")) {
                    return Err(knowledge_package::Error::Invalid);
                }
            }
            Ok(())
        })?;
        frozen.revalidate()?;
        drop(frozen);
        drop(package_work);
        drop(paths);
        source.check()?;
        if Version::of(&source.file()?.metadata().map_err(|_| Error::Inventory)?) != version {
            return Err(Error::Inventory);
        }
        let backing = Arc::new(Backing {
            source,
            version,
            ranges,
            control: control.clone(),
            read_limit: None,
            reads: ReadLiveness::new(plan.index_bytes.checked_mul(4).ok_or(Error::Reservation)?),
            _work: Arc::new(Mutex::new(work)),
        });
        let components = Arc::new(ArchiveDirectory {
            backing: backing.clone(),
            prefix: format!("{prefix}/"),
            indexes_only: true,
        });
        let originals = Arc::new(ArchiveDirectory {
            backing: backing.clone(),
            prefix: String::new(),
            indexes_only: false,
        });
        let generation = SourceGeneration::open(
            scope,
            native.generation,
            build_identity,
            components,
            originals,
            control,
        )?;
        Ok(Self {
            generation,
            backing,
        })
    }
    /// Exact selected native manifest identity, distinct from the archive snapshot digest.
    pub fn id(&self) -> &str {
        self.generation.id()
    }
    /// Recheck the real source lease through the final controllable delivery boundary.
    pub fn check(&self) -> Result<(), Error> {
        self.backing.check().map_err(|_| Error::Inventory)
    }
    /// Query the ONE mature kernel and retain the real archive/Work with every result-byte owner.
    /// Keep the returned value and its borrowed result through actual write/flush settlement.
    pub fn search(self, query: Query, control: &Control) -> Result<StoredResult, Error> {
        self.check()?;
        let result = self.generation.search(query, control)?;
        self.check()?;
        Ok(StoredResult {
            result,
            generation: self,
        })
    }
}

/// Search bytes and the native source/reader/account lifetime that pays for them.
pub struct StoredResult {
    result: SearchResult,
    generation: StoredGeneration,
}
impl StoredResult {
    /// Borrow results without detaching them from their native permit, descriptor and Work lifetime.
    pub fn result(&self) -> &SearchResult {
        &self.result
    }
    /// Check current native source authorization/version before each controllable output chunk.
    pub fn check(&self) -> Result<(), Error> {
        self.generation.check()
    }
    /// Destroy ALL prior result buffers before reusing the SAME prepaid query envelope.
    /// This does not clone credit, reopen a source, complete a Claim or release its retained pin.
    pub fn into_generation(self) -> StoredGeneration {
        let Self { result, generation } = self;
        drop(result);
        generation
    }
}

/// Full observable native descriptor version; equal bytes or inode alone is insufficient.
#[derive(PartialEq, Eq)]
struct Version(u64, u64, u64, i64, i64, i64, i64);
impl Version {
    /// Sample the held native descriptor; the host separately checks its current selected name.
    fn of(m: &Metadata) -> Self {
        Self(
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    }
}
/// Verified physical entry extent and its complete optional-index membership.
struct AdmittedRange {
    extent: knowledge_package::EntryRange,
    index: bool,
}

/// Declaration order keeps the actual source and table alive until before the Work is finished.
struct Backing {
    source: Arc<dyn ArchiveSource>,
    version: Version,
    ranges: BTreeMap<String, AdmittedRange>,
    control: Control,
    read_limit: Option<u64>,
    reads: Arc<ReadLiveness>,
    _work: Arc<Mutex<Work>>,
}
impl Backing {
    /// Reject current-source/policy/stop changes before and after all native range access.
    fn check(&self) -> io::Result<()> {
        self.control.check().map_err(io::Error::other)?;
        self.source.check().map_err(io::Error::other)?;
        let file = self.source.file().map_err(io::Error::other)?;
        if !file.metadata()?.is_file() || Version::of(&file.metadata()?) != self.version {
            return Err(io::Error::other("archive source drift"));
        }
        Ok(())
    }
}
/// Confined native generation-relative names over actual verified STORED extents.
struct ArchiveDirectory {
    backing: Arc<Backing>,
    prefix: String,
    indexes_only: bool,
}
impl std::fmt::Debug for ArchiveDirectory {
    /// Avoid leaking source names or archive paths.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedStoredDirectory")
    }
}
impl ByteDirectory for ArchiveDirectory {
    /// Carry actual native source checks into the mature kernel's generic directory.
    fn check(&self) -> retrieval_kernel::Result<()> {
        self.backing.check().map_err(kernel_io)
    }
    /// Open only a checked table member, retaining descriptor, permit and cost in the returned handle.
    fn open(&self, name: &str) -> retrieval_kernel::Result<Arc<dyn FileHandle>> {
        self.check()?;
        let range = self
            .backing
            .ranges
            .get(&format!("{}{name}", self.prefix))
            .filter(|range| !self.indexes_only || range.index)
            .ok_or_else(|| kernel_io("missing admitted component"))?;
        Ok(Arc::new(RangeHandle {
            backing: self.backing.clone(),
            extent: range.extent,
        }))
    }
    /// Never fall back to a native path or probe another generation.
    fn exists(&self, name: &str) -> retrieval_kernel::Result<bool> {
        self.check()?;
        Ok(self
            .backing
            .ranges
            .get(&format!("{}{name}", self.prefix))
            .is_some_and(|range| !self.indexes_only || range.index))
    }
}
/// A real held native file range; no mmap alignment assumption or archive extraction is required.
struct RangeHandle {
    backing: Arc<Backing>,
    extent: knowledge_package::EntryRange,
}
impl std::fmt::Debug for RangeHandle {
    /// Keep diagnostics bounded and source-free.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedStoredRange")
    }
}
impl HasLen for RangeHandle {
    /// The conversion was checked before publishing the owned range table.
    fn len(&self) -> usize {
        self.extent.length as usize
    }
}
impl FileHandle for RangeHandle {
    /// Allocate only after the whole concurrent reader envelope is prepaid, checking every refill.
    fn read_bytes(&self, range: Range<usize>) -> io::Result<OwnedBytes> {
        self.backing.check()?;
        if range.end < range.start
            || range.end > self.len()
            || self
                .backing
                .read_limit
                .is_some_and(|limit| range.len() as u64 > limit)
        {
            return Err(io::Error::other("read outside admitted range"));
        }
        let absolute = self
            .extent
            .offset
            .checked_add(range.start as u64)
            .ok_or_else(|| io::Error::other("range overflow"))?;
        let loan = self.backing.reads.acquire(range.len() as u64)?;
        self.backing
            .source
            .admit_read(range.len() as u64)
            .map_err(io::Error::other)?;
        let mut bytes = vec![0; range.len()];
        let mut read = 0;
        while read < bytes.len() {
            self.backing.check()?;
            let end = bytes.len().min(read + 64 * 1024);
            let n = self
                .backing
                .source
                .file()
                .map_err(io::Error::other)?
                .read_at(&mut bytes[read..end], absolute + read as u64)?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "archive truncated",
                ));
            }
            read += n;
            self.backing.check()?;
        }
        self.backing.check()?;
        Ok(OwnedBytes::new(RetainedBytes {
            bytes,
            _loan: loan,
            _backing: self.backing.clone(),
        }))
    }
}
// The immutable Vec allocation never moves or reallocates after construction. Dropping bytes
// precedes descriptor/currentness/one-account Work destruction, including OwnedBytes::slice clones.
struct RetainedBytes {
    bytes: Vec<u8>,
    _loan: ReadLoan,
    _backing: Arc<Backing>,
}
impl std::ops::Deref for RetainedBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}
// This only bounds simultaneous bytes within the existing PREPAID holding. It reserves,
// finishes and refunds nothing on Core; dropping a loan returns local liveness, not credit.
struct ReadLiveness {
    limit: u64,
    live: std::sync::atomic::AtomicU64,
}
impl ReadLiveness {
    fn new(limit: u64) -> Arc<Self> {
        Arc::new(Self {
            limit,
            live: std::sync::atomic::AtomicU64::new(0),
        })
    }
    fn acquire(self: &Arc<Self>, bytes: u64) -> io::Result<ReadLoan> {
        self.live
            .try_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |old| old.checked_add(bytes).filter(|n| *n <= self.limit),
            )
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "paid native range bytes are still held",
                )
            })?;
        Ok(ReadLoan {
            owner: self.clone(),
            bytes,
        })
    }
}
struct ReadLoan {
    owner: Arc<ReadLiveness>,
    bytes: u64,
}
impl Drop for ReadLoan {
    fn drop(&mut self) {
        self.owner
            .live
            .fetch_sub(self.bytes, std::sync::atomic::Ordering::AcqRel);
    }
}
// SAFETY: moves/clones of the holder never move its Vec allocation; no mutable accessor exists.
unsafe impl ownedbytes::StableDeref for RetainedBytes {}
/// Classify a bounded native source refusal without treating it as an empty successful result.
fn kernel_io(_: impl std::fmt::Display) -> retrieval_kernel::Error {
    retrieval_kernel::Error {
        code: "unavailable".into(),
        message: "held archive source refused".into(),
    }
}
