//! Private local derived-object cache, with no eviction, authored storage, or service authorization.
//!
//! Read the cache section of the crate guide and its change record before integration.
//! Only Linux/macOS native effects are offered; an arbitrary malicious same-UID process is not isolated.
use crate::data::ContentDigest;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{FileExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
mod native;

/// Fixed private object framing, not a portable knowledge interchange format.
const MAGIC: &[u8; 8] = b"OMNIDRV1";
/// Magic, object key, payload SHA-256, and big-endian payload length.
const HEADER: u64 = 80;

/// Complete effective processor inputs; values are exact strings, never parsed through floating point.
/// The processor owner must expand defaults/overrides and supply immutable code/model/package identities.
/// Dependency roles must include every used corrected transcript and embedding-relevant annotation digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorIdentity {
    /// Digest of the actual processor code/build, not only a mutable program name.
    pub code: ContentDigest,
    /// Effective processor format/behavior version, nonempty.
    pub version: String,
    /// Effective options in sorted-key order; exact UTF-8 values distinguish numeric spellings.
    pub options: BTreeMap<String, String>,
    /// Actual model artifact digest, or explicit absence when not used.
    pub model: Option<ContentDigest>,
    /// Actual package/tokenizer/runtime recipe digest, or explicit absence.
    pub package: Option<ContentDigest>,
    /// Effective embedding/output space, or explicit absence.
    pub space: Option<String>,
    /// Effective output dimensions, or explicit absence.
    pub dimensions: Option<u64>,
    /// Effective preprocessing, including ordering where meaningful inside exact values.
    pub preprocessing: BTreeMap<String, String>,
    /// Sorted semantic dependency roles and their verified byte digests.
    pub dependencies: BTreeMap<String, ContentDigest>,
}

impl ProcessorIdentity {
    /// Hash a length-framed, typed, version-salted identity within an explicit byte ceiling.
    /// Map insertion order does not matter; string/numeric values are not rounded or normalized.
    /// Returns Budget/Invalid for an oversized or empty required identity.
    pub fn fingerprint(&self, max_bytes: usize) -> Result<ContentDigest, CacheError> {
        if self.version.is_empty() {
            return Err(CacheError::Invalid);
        }
        let mut h = IdentityHash {
            hash: Sha256::new(),
            remaining: max_bytes,
        };
        h.field(b"omni-effective-processor-v1")?;
        h.field(self.code.as_str().as_bytes())?;
        h.field(self.version.as_bytes())?;
        h.map(&self.options)?;
        h.optional(self.model.as_ref().map(|d| d.as_str().as_bytes()))?;
        h.optional(self.package.as_ref().map(|d| d.as_str().as_bytes()))?;
        h.optional(self.space.as_ref().map(|s| s.as_bytes()))?;
        h.optional(
            self.dimensions
                .map(u64::to_be_bytes)
                .as_ref()
                .map(|a| a.as_slice()),
        )?;
        h.map(&self.preprocessing)?;
        h.field(&(self.dependencies.len() as u64).to_be_bytes())?;
        for (role, digest) in &self.dependencies {
            if role.is_empty() {
                return Err(CacheError::Invalid);
            }
            h.field(role.as_bytes())?;
            h.field(digest.as_str().as_bytes())?;
        }
        ContentDigest::new(hex(&h.hash.finalize())).map_err(|_| CacheError::Invalid)
    }
}

/// Streaming canonical identity encoder; no intermediate JSON or ambiguous concatenation.
struct IdentityHash {
    /// SHA-256 state.
    hash: Sha256,
    /// Remaining encoded identity bytes, including framing.
    remaining: usize,
}
impl IdentityHash {
    /// Append one length-prefixed field and account for every byte before hashing.
    fn field(&mut self, bytes: &[u8]) -> Result<(), CacheError> {
        let size = bytes.len().checked_add(8).ok_or(CacheError::Budget)?;
        self.remaining = self.remaining.checked_sub(size).ok_or(CacheError::Budget)?;
        self.hash.update((bytes.len() as u64).to_be_bytes());
        self.hash.update(bytes);
        Ok(())
    }
    /// Distinguish absent from present empty values.
    fn optional(&mut self, value: Option<&[u8]>) -> Result<(), CacheError> {
        self.field(&[u8::from(value.is_some())])?;
        if let Some(value) = value {
            self.field(value)?;
        }
        Ok(())
    }
    /// Encode a sorted map with explicit cardinality and field boundaries.
    fn map(&mut self, map: &BTreeMap<String, String>) -> Result<(), CacheError> {
        self.field(&(map.len() as u64).to_be_bytes())?;
        for (key, value) in map {
            if key.is_empty() {
                return Err(CacheError::Invalid);
            }
            self.field(key.as_bytes())?;
            self.field(value.as_bytes())?;
        }
        Ok(())
    }
}

/// Native directory identity; not a portable declaration, permission, or locality proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectoryIdentity {
    /// Native filesystem device value.
    device: u64,
    /// Native inode value.
    inode: u64,
}
impl DirectoryIdentity {
    /// Observe an existing absolute directory through a no-symlink walk; performs no writes.
    /// This establishes identity only, not locality, ownership, ACL safety, or authorization.
    pub fn observe(path: &Path) -> Result<Self, CacheError> {
        Ok(native::stamp(&native::walk(path)?.0)?.identity)
    }
}

/// Locality supplied by the trusted native owner, not inferred from a manifest or a pathname.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locality {
    /// Independently observed local app cache location.
    Local,
    /// Independently observed explicit local alternative to an unsuitable OS cache location.
    ExplicitLocalAlternative,
    /// Network-hosted, synced, unknown, or not yet independently observed; always refused.
    Untrusted,
}

/// Current complete root facts; callers own native locality/ACL observation and complete root discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootFacts {
    /// Identity to which the trusted native locality observation applies.
    pub cache_identity: DirectoryIdentity,
    /// Actual native observation, never imported portable metadata.
    pub locality: Locality,
    /// Every current home, mount/cloud-sync root, and external history store, absolute and accessible.
    /// Unavailable or symlink-spelled roots cannot be independently excluded in this slice and fail closed.
    pub prohibited_roots: Vec<PathBuf>,
}

/// Separate namespaces for contexts that must never share private derived objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityDomain {
    /// Sensitive local library; never shares with exposed or ordinary unexposed contexts.
    Sensitive,
    /// Ordinary unexposed local library.
    Unexposed,
    /// Currently locally admitted exposed library.
    Exposed,
}

/// Current trusted source mapping, not a transferable authorization token; deliberately not serializable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceAdmission {
    /// Currently selected library home, also present in complete prohibited-root facts.
    pub home: PathBuf,
    /// Current native identity of that selected home; copied library IDs do not imply root identity.
    pub home_identity: DirectoryIdentity,
    /// Verified current original byte digest supplied by the source owner, not a digest lookup request.
    pub content: ContentDigest,
    /// Current local sensitivity/exposure isolation context.
    pub domain: SecurityDomain,
    /// Local admission revision; changes invalidate existing pins even when content remains equal.
    pub revision: u64,
}

/// Minimal trusted boundary for current root and library/source mapping; not a detector/plugin registry.
/// Implement only inside the admitted local owner. Data/IDs and a successful portable parse are not grants.
/// Recheck availability, verified version, exclusions, policy, and every effective recipe dependency.
/// Resolve must refuse obsolete/unauthorized processor inputs; M6 authentication is not implemented here.
pub trait CacheContext: Send + Sync {
    /// Return current complete root facts and fresh trusted native locality/ACL assessment.
    fn roots(&self) -> Result<RootFacts, CacheError>;
    /// Resolve an admitted library/source pair and verify the requested effective processor is current.
    fn resolve(
        &self,
        library: &str,
        source: &str,
        processor: &ProcessorIdentity,
    ) -> Result<SourceAdmission, CacheError>;
}

/// Finite caller settings, not storage-format quotas or a measured RSS/physical-volume reservation.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum accounted logical bytes, including retained candidates and unknown regular/link entries.
    /// Hard-linked candidate/object names are conservatively charged twice; filesystem overhead is extra.
    pub max_disk_bytes: u64,
    /// Maximum complete payload bytes per object.
    pub max_object_bytes: u64,
    /// Maximum bytes returned by one pinned range read.
    pub max_read_bytes: usize,
    /// Fixed streaming buffer size for actual publication and verification I/O.
    pub buffer_bytes: usize,
    /// Maximum entries visited per accounting scan, including directories.
    pub max_entries: usize,
    /// Maximum canonical identity bytes and retained request string bytes.
    pub max_identity_bytes: usize,
    /// Maximum simultaneous intended reader pins, bounding retained file handles and recipe copies.
    pub max_pins: usize,
}
impl Limits {
    /// Reject zero, overflowing, or inconsistent settings before opening a cache.
    fn validate(self) -> Result<(), CacheError> {
        if self.max_disk_bytes < HEADER
            || self.max_object_bytes == 0
            || self.max_read_bytes == 0
            || self.buffer_bytes == 0
            || self.max_entries < 8
            || self.max_identity_bytes == 0
            || self.max_pins == 0
            || self.max_object_bytes.checked_add(HEADER).is_none()
            || self.max_read_bytes.checked_add(self.buffer_bytes).is_none()
        {
            return Err(CacheError::Invalid);
        }
        Ok(())
    }
}

/// Current bounded accounting; no cleanup or eviction is implied by inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usage {
    /// Conservative logical bytes, not filesystem allocation blocks or a reservation against other programs.
    pub charged_bytes: u64,
    /// Visited directory entries.
    pub entries: usize,
    /// Unknown regular files, symlinks, or special nodes retained without following/deleting them.
    pub unknown_entries: usize,
    /// Retained complete/partial private candidate names; reclamation requires the next owned GC slice.
    pub retained_candidates: usize,
}

/// Whether this call created the complete name or verified an identical existing complete payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
    /// New complete object atomically linked without replacement.
    Installed,
    /// Existing complete object independently verified identical.
    AlreadyPresent,
}

/// Fixed input-free failures; no raw paths, bodies, or provider/OS error source chains escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheError {
    /// Invalid request or finite settings.
    Invalid,
    /// Unsafe/symlinked/special root or node, permissions, untrusted locality, or prohibited overlap.
    UnsafeRoot,
    /// Another cooperative service or a concurrent/reentrant cache I/O operation owns the lane.
    Busy,
    /// Root, mapping, file identity, or admission changed during the operation.
    Changed,
    /// Byte/memory-buffer/entry accounting ceiling exhausted.
    Budget,
    /// Caller cancellation observed between bounded I/O chunks; candidate remains private and charged.
    Cancelled,
    /// Required directory or current complete object is absent; never proof that an original was deleted.
    Missing,
    /// Object failed complete framing, size, or payload-integrity verification.
    Corrupt,
    /// An existing candidate/name conflicts or an identical key has different verified derived bytes.
    Conflict,
    /// Unknown directory/subtree or state needs explicit inspection; nothing is deleted.
    InspectionRequired,
    /// Effects may exist but complete publication/binding/durability was not established; inspect before retry.
    Indeterminate,
    /// Classified I/O failure; private candidates remain inspectable and charged.
    Io,
}
impl fmt::Display for CacheError {
    /// Emit only the fixed classification, never the rejected input or underlying OS diagnostic.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cache {self:?}")
    }
}
impl std::error::Error for CacheError {}

/// One exclusive service owner; reader pins retain this guard and explicit unlock ends semantic lifetime.
struct Owner {
    /// Locked inode; never passed to callers.
    file: File,
    /// Creator PID prevents forked copies from issuing effects or unlocking the parent's shared description.
    pid: u32,
}
impl Drop for Owner {
    /// Explicitly unlock before close, even if an unrelated pre-exec child inherited the description.
    fn drop(&mut self) {
        if self.pid == std::process::id() {
            let _ = self.file.unlock();
        }
    }
}

/// Captured cache root and trusted owner state, shared only with pins in this process.
struct Inner<C> {
    /// Native root handle never replaced by an ancestor fallback.
    root: File,
    /// Selected absolute root spelling for current-identity revalidation.
    path: PathBuf,
    /// Captured inode identity.
    identity: DirectoryIdentity,
    /// Exclusive service lifetime, including intended reader pins.
    owner: Owner,
    /// Trusted live mapping/root observations, not a portable approval object.
    context: C,
    /// Explicit operation/accounting ceilings.
    limits: Limits,
    /// Serialize cache-owned I/O buffers/accounting without a hidden waiting queue.
    sequence: Mutex<u64>,
    /// Bounded intended-reader handles; release never waits on a producer callback.
    pins: AtomicUsize,
}

/// Real private local derived-object store; has no naked-hash read, deletion, or source-copy API.
pub struct Cache<C: CacheContext> {
    /// Shared captured root and semantic owner guard.
    inner: Arc<Inner<C>>,
}

impl<C: CacheContext> Cache<C> {
    /// Admit an existing app-owned 0700 root using current trusted facts, then acquire one service lock.
    /// No directory or byte is written before placement/ownership/locality/budget admission.
    /// Refuse unknown/inaccessible prohibited roots instead of silently assuming they cannot overlap.
    pub fn open(path: &Path, context: C, limits: Limits) -> Result<Self, CacheError> {
        limits.validate()?;
        let (root, _) = native::walk(path)?;
        native::private(&root, true, 0o700)?;
        let identity = native::stamp(&root)?.identity;
        validate_roots(path, identity, &context.roots()?, limits)?;
        native::usage(&root, limits.max_entries, limits.max_disk_bytes)?;
        let owner_file = native::owner(&root)?;
        owner_file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => CacheError::Busy,
            _ => CacheError::Io,
        })?;
        let owner = Owner {
            file: owner_file,
            pid: std::process::id(),
        };
        let inner = Arc::new(Inner {
            root,
            path: path.to_path_buf(),
            identity,
            owner,
            context,
            limits,
            sequence: Mutex::new(0),
            pins: AtomicUsize::new(0),
        });
        inner.validate()?;
        Ok(Self { inner })
    }

    /// Inspect conservative bounded usage and warnings without deleting any file or following a symlink.
    pub fn usage(&self) -> Result<Usage, CacheError> {
        let _guard = self.inner.operation()?;
        self.inner.validate()?;
        let out = native::usage(
            &self.inner.root,
            self.inner.limits.max_entries,
            self.inner.limits.max_disk_bytes,
        )?;
        self.inner.validate()?;
        Ok(out)
    }

    /// Stream actual derived bytes into a private candidate, then atomically link a complete no-clobber name.
    /// The producer owns derivation correctness and must never submit original copies as cache payloads.
    /// EOF must mean validated complete processor success, not merely a failed child's closed output pipe.
    /// Cancellation/budget/I/O failure retains a private charged candidate, never replaces complete data.
    /// Cancellation is cooperative between reads; the supplied Read must itself have bounded/interruptible waits.
    pub fn publish<R: Read>(
        &self,
        library: &str,
        source: &str,
        processor: &ProcessorIdentity,
        input: &mut R,
        cancelled: impl Fn() -> bool,
    ) -> Result<Publication, CacheError> {
        let mut sequence = self.inner.operation()?;
        let admission = self.inner.admit(library, source, processor)?;
        let used = native::usage(
            &self.inner.root,
            self.inner.limits.max_entries,
            self.inner.limits.max_disk_bytes,
        )?;
        if used.entries.checked_add(4).ok_or(CacheError::Budget)? > self.inner.limits.max_entries {
            return Err(CacheError::Budget);
        }
        // A new final hard link is conservatively charged a second time, even though it shares data blocks.
        let capacity = self
            .inner
            .limits
            .max_disk_bytes
            .checked_sub(used.charged_bytes)
            .ok_or(CacheError::Budget)?
            / 2;
        let payload_limit = capacity
            .checked_sub(HEADER)
            .ok_or(CacheError::Budget)?
            .min(self.inner.limits.max_object_bytes);
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let key = object_key(
            &admission.content,
            processor,
            self.inner.limits.max_identity_bytes,
        )?;
        let location = self.inner.location(library, &admission, &key, true)?;
        self.inner.same(library, source, processor, &admission)?;
        location.validate(&self.inner.root)?;
        *sequence = sequence.checked_add(1).ok_or(CacheError::Budget)?;
        let name = format!(".candidate-{}-{}", self.inner.owner.pid, *sequence);
        let mut candidate = native::create(&location.fanout, &name)?;
        let candidate_identity = native::stamp(&candidate)?.identity;
        candidate
            .write_all(&[0; HEADER as usize])
            .map_err(|_| CacheError::Io)?;
        let mut buffer = vec![0; self.inner.limits.buffer_bytes];
        let mut hash = Sha256::new();
        let mut length = 0u64;
        loop {
            if cancelled() {
                return Err(CacheError::Cancelled);
            }
            let available = payload_limit - length;
            // Read at most one extra byte to establish overbudget; never write it.
            let wanted = buffer
                .len()
                .min(usize::try_from(available.saturating_add(1)).unwrap_or(usize::MAX));
            let size = input
                .read(&mut buffer[..wanted])
                .map_err(|_| CacheError::Io)?;
            if size == 0 {
                break;
            }
            if size > wanted {
                return Err(CacheError::Invalid);
            }
            if size as u64 > available {
                return Err(CacheError::Budget);
            }
            candidate
                .write_all(&buffer[..size])
                .map_err(|_| CacheError::Io)?;
            hash.update(&buffer[..size]);
            length += size as u64;
        }
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let digest: [u8; 32] = hash.finalize().into();
        let mut header = [0u8; HEADER as usize];
        header[..8].copy_from_slice(MAGIC);
        header[8..40].copy_from_slice(&key);
        header[40..72].copy_from_slice(&digest);
        header[72..].copy_from_slice(&length.to_be_bytes());
        candidate
            .seek(SeekFrom::Start(0))
            .map_err(|_| CacheError::Io)?;
        candidate.write_all(&header).map_err(|_| CacheError::Io)?;
        candidate
            .set_permissions(std::fs::Permissions::from_mode(0o400))
            .map_err(|_| CacheError::Io)?;
        candidate
            .sync_all()
            .map_err(|_| CacheError::Indeterminate)?;
        self.inner.same(library, source, processor, &admission)?;
        location.validate(&self.inner.root)?;
        let named = native::regular(&location.fanout, &name, 0o400)?;
        if native::stamp(&named)?.identity != candidate_identity {
            return Err(CacheError::Indeterminate);
        }
        verify(&named, &key, self.inner.limits, None)?;
        if cancelled() {
            return Err(CacheError::Cancelled);
        }
        let object_name = hex(&key);
        let outcome = match rustix::fs::linkat(
            &location.fanout,
            name.as_str(),
            &location.fanout,
            object_name.as_str(),
            rustix::fs::AtFlags::empty(),
        ) {
            Ok(()) => Publication::Installed,
            Err(rustix::io::Errno::EXIST) => {
                let existing = native::regular(&location.fanout, &object_name, 0o400)?;
                let observed = verify(&existing, &key, self.inner.limits, None)?;
                if observed.digest != digest || observed.length != length {
                    return Err(CacheError::Conflict);
                }
                Publication::AlreadyPresent
            }
            Err(_) => return Err(CacheError::Indeterminate),
        };
        if outcome == Publication::Installed {
            let installed = native::regular(&location.fanout, &object_name, 0o400)
                .map_err(|_| CacheError::Indeterminate)?;
            if native::stamp(&installed)?.identity != candidate_identity {
                return Err(CacheError::Indeterminate);
            }
            let installed_bytes = verify(&installed, &key, self.inner.limits, None)
                .map_err(|_| CacheError::Indeterminate)?;
            if installed_bytes.digest != digest || installed_bytes.length != length {
                return Err(CacheError::Indeterminate);
            }
        }
        location
            .fanout
            .sync_all()
            .map_err(|_| CacheError::Indeterminate)?;
        location
            .namespace
            .sync_all()
            .map_err(|_| CacheError::Indeterminate)?;
        self.inner
            .root
            .sync_all()
            .map_err(|_| CacheError::Indeterminate)?;
        location
            .validate(&self.inner.root)
            .map_err(|_| CacheError::Indeterminate)?;
        self.inner
            .same(library, source, processor, &admission)
            .map_err(|_| CacheError::Indeterminate)?;
        Ok(outcome)
    }

    /// Open and verify a complete object only through current admitted library/source/processor mapping.
    /// The returned pin retains the single-service guard; no cross-process reader API is offered.
    pub fn pin(
        &self,
        library: &str,
        source: &str,
        processor: &ProcessorIdentity,
    ) -> Result<PinnedObject<C>, CacheError> {
        let _guard = self.inner.operation()?;
        if self.inner.pins.load(Ordering::Relaxed) >= self.inner.limits.max_pins {
            return Err(CacheError::Budget);
        }
        let admission = self.inner.admit(library, source, processor)?;
        let key = object_key(
            &admission.content,
            processor,
            self.inner.limits.max_identity_bytes,
        )?;
        let location = self.inner.location(library, &admission, &key, false)?;
        let file = native::regular(&location.fanout, &hex(&key), 0o400)?;
        let verified = verify(&file, &key, self.inner.limits, None)?;
        location.validate(&self.inner.root)?;
        self.inner.same(library, source, processor, &admission)?;
        let stamp = verified.stamp;
        let current = native::regular(&location.fanout, &hex(&key), 0o400)?;
        if native::stamp(&current)? != stamp {
            return Err(CacheError::Changed);
        }
        self.inner.pins.fetch_add(1, Ordering::Relaxed);
        Ok(PinnedObject {
            inner: self.inner.clone(),
            file,
            location,
            key,
            stamp,
            admission,
            library: library.into(),
            source: source.into(),
            processor: processor.clone(),
            length: verified.length,
        })
    }
}

/// Held directories keep effects attached to the selected namespace/fanout, never ancestor fallback.
struct Location {
    /// Native library isolation namespace.
    namespace: File,
    /// Namespace component hash.
    name: String,
    /// Native key fanout directory.
    fanout: File,
    /// Fanout component.
    prefix: String,
}
impl Location {
    /// Recheck both path-to-inode bindings before/after descriptor effects.
    fn validate(&self, root: &File) -> Result<(), CacheError> {
        native::bound_directory(root, &self.name, &self.namespace)?;
        native::bound_directory(&self.namespace, &self.prefix, &self.fanout)
    }
}

impl<C: CacheContext> Inner<C> {
    /// Admit one cache-owned I/O buffer lane; contention/reentrancy returns Busy, never deadlocks a producer.
    fn operation(&self) -> Result<MutexGuard<'_, u64>, CacheError> {
        self.process()?;
        self.sequence.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => CacheError::Busy,
            TryLockError::Poisoned(_) => CacheError::Indeterminate,
        })
    }
    /// Refuse post-fork use before touching a potentially inherited locked Rust mutex.
    fn process(&self) -> Result<(), CacheError> {
        if self.owner.pid == std::process::id() {
            Ok(())
        } else {
            Err(CacheError::Changed)
        }
    }
    /// Verify captured root, owner inode, process, permissions, and all current prohibited roots.
    fn validate(&self) -> Result<(), CacheError> {
        if self.owner.pid != std::process::id() {
            return Err(CacheError::Changed);
        }
        native::private(&self.root, true, 0o700)?;
        let owner = native::regular(&self.root, ".owner", 0o600)?;
        let stamp = native::stamp(&owner)?;
        if stamp.identity != native::stamp(&self.owner.file)?.identity
            || stamp.links != 1
            || stamp.length != 0
        {
            return Err(CacheError::Changed);
        }
        validate_roots(
            &self.path,
            self.identity,
            &self.context.roots()?,
            self.limits,
        )
    }
    /// Resolve a current trusted source mapping and verify its selected home against complete root facts.
    fn admit(
        &self,
        library: &str,
        source: &str,
        processor: &ProcessorIdentity,
    ) -> Result<SourceAdmission, CacheError> {
        self.validate()?;
        if library.is_empty()
            || source.is_empty()
            || library
                .len()
                .checked_add(source.len())
                .ok_or(CacheError::Budget)?
                > self.limits.max_identity_bytes
        {
            return Err(CacheError::Invalid);
        }
        processor.fingerprint(self.limits.max_identity_bytes)?;
        let admitted = self.context.resolve(library, source, processor)?;
        let facts = self.context.roots()?;
        validate_roots(&self.path, self.identity, &facts, self.limits)?;
        if !facts.prohibited_roots.contains(&admitted.home)
            || DirectoryIdentity::observe(&admitted.home)? != admitted.home_identity
        {
            return Err(CacheError::Changed);
        }
        Ok(admitted)
    }
    /// Refuse detected mapping/policy/version drift rather than return stale bytes or acknowledge publication.
    fn same(
        &self,
        library: &str,
        source: &str,
        processor: &ProcessorIdentity,
        expected: &SourceAdmission,
    ) -> Result<(), CacheError> {
        if self.admit(library, source, processor)? != *expected {
            return Err(CacheError::Changed);
        }
        Ok(())
    }
    /// Resolve generated components inside held root; creates only after admission and budget accounting.
    fn location(
        &self,
        library: &str,
        admission: &SourceAdmission,
        key: &[u8; 32],
        create: bool,
    ) -> Result<Location, CacheError> {
        let mut h = IdentityHash {
            hash: Sha256::new(),
            remaining: usize::MAX,
        };
        h.field(b"omni-library-cache-namespace-v1")?;
        h.field(library.as_bytes())?;
        h.field(&admission.home_identity.device.to_be_bytes())?;
        h.field(&admission.home_identity.inode.to_be_bytes())?;
        h.field(&[match admission.domain {
            SecurityDomain::Sensitive => 0,
            SecurityDomain::Unexposed => 1,
            SecurityDomain::Exposed => 2,
        }])?;
        let name = hex(&h.hash.finalize());
        let prefix = hex(key)[..2].to_owned();
        let open = |parent: &File, child: &str| {
            if create {
                native::child(parent, child)
            } else {
                native::directory(parent, child)
            }
        };
        let namespace = open(&self.root, &name)?;
        native::private(&namespace, true, 0o700)?;
        let fanout = open(&namespace, &prefix)?;
        native::private(&fanout, true, 0o700)?;
        Ok(Location {
            namespace,
            name,
            fanout,
            prefix,
        })
    }
}

/// A real open complete-object pin; retains root ownership until the last intended reader drops.
/// Reads re-resolve current source/dependency/policy admission and verify the entire bounded payload.
pub struct PinnedObject<C: CacheContext> {
    /// Current service context and exclusive-owner lifetime.
    inner: Arc<Inner<C>>,
    /// Actual captured file; never exposed as a naked pathname/descriptor to consumers.
    file: File,
    /// Captured namespace path bindings.
    location: Location,
    /// Exact content/processor-bound object key.
    key: [u8; 32],
    /// File identity and version evidence captured after complete verification.
    stamp: native::Stamp,
    /// Current admitted mapping expected by this pin.
    admission: SourceAdmission,
    /// Admitted requested library identity.
    library: String,
    /// Admitted requested source identity, never derived from equal content.
    source: String,
    /// Effective pipeline identity; cloned only after bounded fingerprint admission.
    processor: ProcessorIdentity,
    /// Verified payload bytes, excluding framing.
    length: u64,
}
impl<C: CacheContext> PinnedObject<C> {
    /// Return a bounded range (possibly short at EOF) from a single fully SHA-verified stream.
    /// No bytes escape on detected file/root/mapping drift, corruption, or exhausted read bounds.
    pub fn read(&self, offset: u64, length: usize) -> Result<Vec<u8>, CacheError> {
        let _guard = self.inner.operation()?;
        if length == 0
            || length > self.inner.limits.max_read_bytes
            || offset.checked_add(length as u64).is_none()
        {
            return Err(CacheError::Budget);
        }
        self.inner.same(
            &self.library,
            &self.source,
            &self.processor,
            &self.admission,
        )?;
        self.location.validate(&self.inner.root)?;
        let named = native::regular(&self.location.fanout, &hex(&self.key), 0o400)?;
        if native::stamp(&named)? != self.stamp || native::stamp(&self.file)? != self.stamp {
            return Err(CacheError::Changed);
        }
        let verified = verify(
            &self.file,
            &self.key,
            self.inner.limits,
            Some((offset, length)),
        )?;
        if verified.length != self.length || native::stamp(&self.file)? != self.stamp {
            return Err(CacheError::Changed);
        }
        self.location.validate(&self.inner.root)?;
        self.inner.same(
            &self.library,
            &self.source,
            &self.processor,
            &self.admission,
        )?;
        Ok(verified.bytes)
    }
}

impl<C: CacheContext> Drop for PinnedObject<C> {
    /// Release only this process's intended pin; no file is deleted and no unrelated descriptor is closed.
    fn drop(&mut self) {
        if self.inner.owner.pid == std::process::id() {
            self.inner.pins.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// Apply current overlap and identity checks without creating or writing any node.
fn validate_roots(
    path: &Path,
    identity: DirectoryIdentity,
    facts: &RootFacts,
    limits: Limits,
) -> Result<(), CacheError> {
    if facts.cache_identity != identity {
        return Err(CacheError::Changed);
    }
    if !cfg!(any(target_os = "linux", target_os = "macos"))
        || facts.locality == Locality::Untrusted
        || facts.prohibited_roots.is_empty()
        || facts.prohibited_roots.len() > limits.max_entries
    {
        return Err(CacheError::UnsafeRoot);
    }
    let (root, chain) = native::walk(path)?;
    native::private(&root, true, 0o700)?;
    if native::stamp(&root)?.identity != identity {
        return Err(CacheError::Changed);
    }
    for prohibited in &facts.prohibited_roots {
        let (other, ancestors) = native::walk(prohibited)?;
        let other_identity = native::stamp(&other)?.identity;
        if path.starts_with(prohibited)
            || prohibited.starts_with(path)
            || ancestors.contains(&identity)
            || chain.contains(&other_identity)
        {
            return Err(CacheError::UnsafeRoot);
        }
    }
    Ok(())
}

/// Bind verified original content plus the complete effective recipe with explicit domain/version salt.
fn object_key(
    content: &ContentDigest,
    processor: &ProcessorIdentity,
    maximum: usize,
) -> Result<[u8; 32], CacheError> {
    let fingerprint = processor.fingerprint(maximum)?;
    let mut hash = Sha256::new();
    hash.update(b"omni-derived-object-key-v1\0");
    hash.update(content.as_str().as_bytes());
    hash.update(fingerprint.as_str().as_bytes());
    Ok(hash.finalize().into())
}

/// Full bounded verification result; selected bytes were collected from the same verified stream.
struct Verified {
    /// Native version actually covered by the complete verification pass.
    stamp: native::Stamp,
    /// Actual complete payload digest.
    digest: [u8; 32],
    /// Exact payload byte count.
    length: u64,
    /// Requested range only; no full-payload RAM materialization.
    bytes: Vec<u8>,
}

/// Read exactly one bounded range through positional I/O; never share a mutable descriptor cursor.
fn read_at(file: &File, buffer: &mut [u8], mut offset: u64) -> Result<(), CacheError> {
    let mut done = 0;
    while done < buffer.len() {
        let n = file
            .read_at(&mut buffer[done..], offset)
            .map_err(|_| CacheError::Io)?;
        if n == 0 {
            return Err(CacheError::Corrupt);
        }
        offset += n as u64;
        done += n;
    }
    Ok(())
}

/// Verify framing, key, exact length, before/after native stamps, and full streamed payload SHA-256.
fn verify(
    file: &File,
    key: &[u8; 32],
    limits: Limits,
    range: Option<(u64, usize)>,
) -> Result<Verified, CacheError> {
    native::private(file, false, 0o400)?;
    let before = native::stamp(file)?;
    let mut header = [0; HEADER as usize];
    read_at(file, &mut header, 0)?;
    if &header[..8] != MAGIC || &header[8..40] != key {
        return Err(CacheError::Corrupt);
    }
    let length = u64::from_be_bytes(header[72..].try_into().map_err(|_| CacheError::Corrupt)?);
    if length > limits.max_object_bytes || length.checked_add(HEADER) != Some(before.length) {
        return Err(CacheError::Corrupt);
    }
    let mut buffer = vec![0; limits.buffer_bytes];
    let mut hash = Sha256::new();
    let mut offset = 0;
    let mut output = Vec::new();
    while offset < length {
        let size = buffer
            .len()
            .min(usize::try_from(length - offset).unwrap_or(usize::MAX));
        read_at(file, &mut buffer[..size], HEADER + offset)?;
        hash.update(&buffer[..size]);
        if let Some((start, count)) = range {
            let begin = start.max(offset);
            let end = (start + count as u64).min(offset + size as u64);
            if begin < end {
                output
                    .extend_from_slice(&buffer[(begin - offset) as usize..(end - offset) as usize]);
            }
        }
        offset += size as u64;
    }
    let digest: [u8; 32] = hash.finalize().into();
    if digest.as_slice() != &header[40..72] {
        return Err(CacheError::Corrupt);
    }
    if native::stamp(file)? != before {
        return Err(CacheError::Changed);
    }
    Ok(Verified {
        stamp: before,
        digest,
        length,
        bytes: output,
    })
}

/// Encode fixed digest bytes as lowercase hex; no path text comes from source identifiers.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| {
            [
                DIGITS[(b >> 4) as usize] as char,
                DIGITS[(b & 15) as usize] as char,
            ]
        })
        .collect()
}
