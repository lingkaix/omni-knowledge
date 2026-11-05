//! The local directory is a single source-owner scope, independent of product admission.
use crate::model::*;
use crate::text::*;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Seek, SeekFrom};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use tantivy::HasLen;

/// Owned, current-policy-checked byte directory supplied by a native host.
/// Implementations retain their actual descriptors, generation pins and prepaid concurrent costs.
/// Names and bytes are facts, not read grants; every open/read must check the live caller lease.
/// Returned handles own that SAME lease, not a borrow into a finished callback.
pub trait ByteDirectory: std::fmt::Debug + Send + Sync + 'static {
    /// Recheck cancellation, current selection/policy and the held source version.
    fn check(&self) -> Result<()>;
    /// Open one confined component without reopening a display path or extracting an archive.
    fn open(&self, name: &str) -> Result<std::sync::Arc<dyn tantivy::directory::FileHandle>>;
    /// Test membership without probing paths outside this admitted directory.
    fn exists(&self, name: &str) -> Result<bool>;
}

/// Immutable native generation over generic byte directories, using the SAME kernel query engine.
/// The host prepays opaque reader/runtime plus buffer/table/output peaks before construction.
#[derive(Clone)]
pub struct SourceGeneration {
    pub(crate) id: String,
    pub(crate) manifest: std::sync::Arc<crate::store::Manifest>,
    pub(crate) components: std::sync::Arc<dyn ByteDirectory>,
    pub(crate) originals: std::sync::Arc<dyn ByteDirectory>,
}
impl SourceGeneration {
    /// Open a bounded, integrity-checked native manifest and exact consuming machinery identity.
    /// Optional package compatibility must be checked by its owner BEFORE this native format check.
    pub fn open(
        scope: &str,
        generation: &str,
        build_identity: &str,
        components: std::sync::Arc<dyn ByteDirectory>,
        originals: std::sync::Arc<dyn ByteDirectory>,
        control: &Control,
    ) -> Result<Self> {
        control.check()?;
        crate::store::validate_scope(scope)?;
        components.check()?;
        originals.check()?;
        let handle = components.open("manifest.json")?;
        if handle.len() > FRAME_BYTES {
            return Err(error(
                "resource-limited",
                "generation manifest exceeds byte budget",
            ));
        }
        let bytes = handle.read_bytes(0..handle.len()).map_err(io_error)?;
        if digest(&bytes) != generation {
            return Err(error(
                "unavailable",
                "committed manifest integrity failed; rebuild scope",
            ));
        }
        let manifest: crate::store::Manifest = serde_json::from_slice(&bytes)
            .map_err(|_| error("unavailable", "committed manifest invalid; rebuild scope"))?;
        if manifest.scope != scope
            || manifest.machinery != crate::store::consumer_machinery(build_identity)?
        {
            return Err(error(
                "unavailable",
                "index identity mismatch; rebuild scope",
            ));
        }
        crate::store::validate_manifest(&manifest)?;
        components.check()?;
        originals.check()?;
        control.check()?;
        Ok(Self {
            id: generation.to_owned(),
            manifest: std::sync::Arc::new(manifest),
            components,
            originals,
        })
    }
    /// Exact immutable native manifest identity. This value is not permission to read a source.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Search a current admitted view with the mature analyzer, ranking, verification and scanners.
    pub fn search(&self, query: Query, control: &Control) -> Result<SearchResult> {
        control.check()?;
        self.components.check()?;
        self.originals.check()?;
        let result = crate::query::search_generation(self, query, &Budget::controlled(control))?;
        self.components.check()?;
        self.originals.check()?;
        control.check()?;
        Ok(result)
    }
}

/// Tantivy's read-only Directory over actual owned range handles, never a writable fake filesystem.
#[derive(Clone)]
pub(crate) struct IndexDirectory(pub std::sync::Arc<dyn ByteDirectory>);
impl std::fmt::Debug for IndexDirectory {
    /// Do not disclose source names through native diagnostics.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PinnedIndexDirectory")
    }
}
impl IndexDirectory {
    /// Convert Tantivy's single component to its verified generation-relative name.
    fn name(path: &Path) -> std::io::Result<String> {
        if path.components().count() != 1
            || !matches!(
                path.components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "invalid index component",
            ));
        }
        let name = path
            .to_str()
            .ok_or_else(|| std::io::Error::other("non UTF-8 index component"))?;
        Ok(format!("index/{name}"))
    }
}
impl tantivy::directory::Directory for IndexDirectory {
    /// Open only a member of the pinned generation, with no extraction or native-path fallback.
    fn get_file_handle(
        &self,
        path: &Path,
    ) -> std::result::Result<
        std::sync::Arc<dyn tantivy::directory::FileHandle>,
        tantivy::directory::error::OpenReadError,
    > {
        use tantivy::directory::error::OpenReadError;
        let name =
            Self::name(path).map_err(|e| OpenReadError::wrap_io_error(e, path.to_owned()))?;
        self.0
            .check()
            .map_err(|e| OpenReadError::wrap_io_error(std::io::Error::other(e), path.to_owned()))?;
        if !self
            .0
            .exists(&name)
            .map_err(|e| OpenReadError::wrap_io_error(std::io::Error::other(e), path.to_owned()))?
        {
            return Err(OpenReadError::FileDoesNotExist(path.to_owned()));
        }
        self.0
            .open(&name)
            .map_err(|e| OpenReadError::wrap_io_error(std::io::Error::other(e), path.to_owned()))
    }
    /// An immutable generation cannot be deleted through a reader.
    fn delete(
        &self,
        path: &Path,
    ) -> std::result::Result<(), tantivy::directory::error::DeleteError> {
        Err(tantivy::directory::error::DeleteError::IoError {
            io_error: std::sync::Arc::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "read-only generation",
            )),
            filepath: path.to_owned(),
        })
    }
    /// Check admitted membership only, rechecking the actual source lease.
    fn exists(
        &self,
        path: &Path,
    ) -> std::result::Result<bool, tantivy::directory::error::OpenReadError> {
        use tantivy::directory::error::OpenReadError;
        let name =
            Self::name(path).map_err(|e| OpenReadError::wrap_io_error(e, path.to_owned()))?;
        self.0
            .check()
            .and_then(|()| self.0.exists(&name))
            .map_err(|e| OpenReadError::wrap_io_error(std::io::Error::other(e), path.to_owned()))
    }
    /// Writers must use the existing native generation owner, never this read-only directory.
    fn open_write(
        &self,
        path: &Path,
    ) -> std::result::Result<tantivy::directory::WritePtr, tantivy::directory::error::OpenWriteError>
    {
        Err(tantivy::directory::error::OpenWriteError::wrap_io_error(
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read-only generation"),
            path.to_owned(),
        ))
    }
    /// Read bounded native control components under the retained reader envelope.
    fn atomic_read(
        &self,
        path: &Path,
    ) -> std::result::Result<Vec<u8>, tantivy::directory::error::OpenReadError> {
        use tantivy::directory::error::OpenReadError;
        let handle = self.get_file_handle(path)?;
        if handle.len() > FRAME_BYTES {
            return Err(OpenReadError::wrap_io_error(
                std::io::Error::other("index control component exceeds byte budget"),
                path.to_owned(),
            ));
        }
        handle
            .read_bytes(0..handle.len())
            .map(|v| v.to_vec())
            .map_err(|e| OpenReadError::wrap_io_error(e, path.to_owned()))
    }
    /// Refuse mutations, including lockfile creation.
    fn atomic_write(&self, _: &Path, _: &[u8]) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "read-only generation",
        ))
    }
    /// No writes exist to synchronize; current-source validation still applies.
    fn sync_directory(&self) -> std::io::Result<()> {
        self.0.check().map_err(std::io::Error::other)
    }
    /// Immutable native providers already pin every component against collection.
    /// A metadata lock retains that actual lease; writer or unknown locks remain refused.
    fn acquire_lock(
        &self,
        lock: &tantivy::directory::Lock,
    ) -> std::result::Result<tantivy::directory::DirectoryLock, tantivy::directory::error::LockError>
    {
        if lock.filepath != tantivy::directory::META_LOCK.filepath {
            return Err(tantivy::directory::error::LockError::IoError(
                std::sync::Arc::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "read-only generation",
                )),
            ));
        }
        self.0.check().map_err(|e| {
            tantivy::directory::error::LockError::IoError(std::sync::Arc::new(
                std::io::Error::other(e),
            ))
        })?;
        Ok(Box::new(self.0.clone()).into())
    }
    /// Frozen generations use manual reload; no watcher thread or mutable pointer is created.
    fn watch(
        &self,
        _: tantivy::directory::WatchCallback,
    ) -> tantivy::Result<tantivy::directory::WatchHandle> {
        Ok(tantivy::directory::WatchHandle::empty())
    }
}

/// Owned range cursor for canonical source/representation verification and bounded scans.
pub(crate) struct RangeReader {
    slice: tantivy::directory::FileSlice,
    position: u64,
}
impl RangeReader {
    /// Slice a held range before any read. Checked extents cannot reach neighbouring entries.
    pub(crate) fn new(
        handle: std::sync::Arc<dyn tantivy::directory::FileHandle>,
        range: Option<&ByteRange>,
    ) -> Result<Self> {
        let slice = tantivy::directory::FileSlice::new(handle);
        let slice = if let Some(range) = range {
            if range.end < range.start || range.end > slice.len() as u64 {
                return Err(error("stale", "source range changed"));
            }
            slice.slice(range.start as usize..range.end as usize)
        } else {
            slice
        };
        Ok(Self { slice, position: 0 })
    }
}
impl std::io::Read for RangeReader {
    /// Read only the requested bounded window; do not materialize the whole source.
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = output.len().min(self.slice.len() - self.position as usize);
        if count == 0 {
            return Ok(0);
        }
        let bytes = self
            .slice
            .read_bytes_slice(self.position as usize..self.position as usize + count)?;
        output[..count].copy_from_slice(&bytes);
        self.position += count as u64;
        Ok(count)
    }
}
impl Seek for RangeReader {
    /// Refuse seeks outside the current verified entry.
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => self.slice.len() as i128 + i128::from(n),
        };
        if position < 0 || position > self.slice.len() as i128 {
            return Err(std::io::Error::other("source seek outside range"));
        }
        self.position = position as u64;
        Ok(self.position)
    }
}

/// Fixed local view; MCP cannot change its root through a tool argument.
pub struct Directory {
    root: PathBuf,
    store: PathBuf,
    scope: String,
}
impl Directory {
    /// Bind one canonical directory and one derived cache directory.
    pub fn new(root: &Path, store: &Path) -> Result<Self> {
        let root = fs::canonicalize(root).map_err(io_error)?;
        let store = fs::canonicalize(store).map_err(io_error)?;
        if root == store {
            return Err(error(
                "invalid-request",
                "derived store must be separate from source root",
            ));
        }
        let scope = format!("directory:{}", digest(root.as_os_str().as_encoded_bytes()));
        Ok(Self { root, store, scope })
    }
    /// Enumerate dotfiles and regular files without Git-ignore rules or symlink traversal.
    pub fn inventory(&self) -> Result<ScopeView> {
        let mut documents = Vec::new();
        let mut unavailable_objects = 0;
        let root = File::open(&self.root).map_err(io_error)?;
        self.visit(
            &root,
            Path::new(""),
            &mut documents,
            &mut unavailable_objects,
            0,
            &Budget::new(),
        )?;
        documents.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(ScopeView {
            scope: self.scope.clone(),
            documents,
            unavailable_objects,
        })
    }
    /// Query one startup-bound scope and withhold files that changed during the operation.
    pub fn search(
        &self,
        kernel: &crate::Kernel,
        mode: Mode,
        query: String,
        limit: usize,
    ) -> Result<SearchResult> {
        self.search_inner(kernel, mode, query, limit, || {})
    }
    /// Observe a final canonical reconciliation; tests can change a source at this exact boundary.
    pub(crate) fn search_inner(
        &self,
        kernel: &crate::Kernel,
        mode: Mode,
        query: String,
        limit: usize,
        after_search: impl FnOnce(),
    ) -> Result<SearchResult> {
        crate::query::validate_query(&Query {
            mode,
            query: query.clone(),
            scopes: vec![],
            limit,
        })?;
        let before = self.inventory()?;
        let mut result = kernel.search(Query {
            mode,
            query,
            limit,
            scopes: vec![before.clone()],
        })?;
        after_search();
        let after = self.inventory()?;
        if before != after {
            let live: BTreeMap<_, _> = after.documents.iter().map(|d| (&d.key, d)).collect();
            let changed: std::collections::BTreeSet<_> = before
                .documents
                .iter()
                .filter(|d| live.get(&d.key).is_none_or(|now| **now != **d))
                .map(|d| &d.key)
                .collect();
            result.hits.retain(|h| !changed.contains(&h.key));
            result.coverage.inventory_complete = false;
            result.coverage.enumeration_complete = false;
            *result
                .coverage
                .reasons
                .entry("directory-changed".into())
                .or_default() += 1;
            result.complete = false;
        }
        Ok(result)
    }
    /// Report the startup-bound scope identity.
    pub fn scope(&self) -> &str {
        &self.scope
    }
    /// Walk through held descriptors. A renamed directory cannot redirect this walk outside the root.
    fn visit(
        &self,
        directory: &File,
        relative_dir: &Path,
        documents: &mut Vec<Document>,
        unavailable_objects: &mut usize,
        depth: usize,
        budget: &Budget,
    ) -> Result<()> {
        budget.check()?;
        if depth > 64 {
            return Err(error(
                "resource-limited",
                "directory depth budget exhausted",
            ));
        }
        let entries = match names(directory) {
            Ok(entries) => entries,
            Err(e) if e.code == "unavailable" || e.code == "io" => {
                *unavailable_objects += 1;
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        for name in entries {
            budget.check()?;
            let relative = relative_dir.join(&name);
            if self.root.join(&relative) == self.store {
                continue;
            }
            if documents.len() + *unavailable_objects >= DOCUMENT_LIMIT {
                return Err(error(
                    "resource-limited",
                    "directory object budget exhausted",
                ));
            }
            #[cfg(test)]
            crate::tests::directory_boundary("stat", &self.root.join(&relative));
            let name_c = std::ffi::CString::new(name.as_bytes()).map_err(io_error)?;
            // SAFETY: fstatat receives a live parent descriptor, initialized storage, and a C name.
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::fstatat(
                    directory.as_raw_fd(),
                    name_c.as_ptr(),
                    &mut stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } < 0
            {
                *unavailable_objects += 1;
                continue;
            }
            let kind = stat.st_mode & libc::S_IFMT;
            if kind != libc::S_IFREG && kind != libc::S_IFDIR {
                continue;
            }
            #[cfg(test)]
            crate::tests::directory_boundary("open", &self.root.join(&relative));
            // SAFETY: openat receives a held parent and a single component; symlinks are refused.
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name_c.as_ptr(),
                    libc::O_RDONLY
                        | libc::O_CLOEXEC
                        | libc::O_NOFOLLOW
                        | libc::O_NONBLOCK
                        | if kind == libc::S_IFDIR {
                            libc::O_DIRECTORY
                        } else {
                            0
                        },
                )
            };
            #[cfg(test)]
            if kind == libc::S_IFREG {
                crate::tests::access("directory-open", &relative.to_string_lossy());
            }
            if fd < 0 {
                *unavailable_objects += 1;
                continue;
            }
            // SAFETY: this successful open transfers ownership of the descriptor to File.
            let mut file = unsafe { File::from_raw_fd(fd) };
            if kind == libc::S_IFDIR {
                self.visit(
                    &file,
                    &relative,
                    documents,
                    unavailable_objects,
                    depth + 1,
                    budget,
                )?;
                continue;
            }
            let relative = relative
                .to_str()
                .ok_or_else(|| error("invalid-request", "directory path is not UTF-8"))?
                .to_string();
            let name = name
                .to_str()
                .ok_or_else(|| error("invalid-request", "directory filename is not UTF-8"))?
                .to_string();
            // No Document exists until canonical bytes and metadata have been verified.
            let identity = (|| -> Result<_> {
                let before = file.metadata().map_err(io_error)?;
                if !before.is_file() {
                    return Err(error("unavailable", "source type changed"));
                }
                let (hash, size) = digest_reader(&mut file, budget)?;
                file.seek(SeekFrom::Start(0)).map_err(io_error)?;
                let mut binary = false;
                let mut chunks = Chunks::new(&mut file);
                loop {
                    match chunks.next(budget) {
                        Ok(Some((_, text))) => binary |= text.as_bytes().contains(&0),
                        Ok(None) => break,
                        Err(e) if e.code == "failed" => {
                            binary = true;
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                }
                let after = file.metadata().map_err(io_error)?;
                if before.ino() != after.ino()
                    || before.dev() != after.dev()
                    || before.len() != after.len()
                    || before.mtime() != after.mtime()
                    || before.mtime_nsec() != after.mtime_nsec()
                    || before.ctime() != after.ctime()
                    || before.ctime_nsec() != after.ctime_nsec()
                {
                    return Err(error("unavailable", "source changed during verification"));
                }
                let revision_metadata = format!(
                    "{}:{}:{}:{}:{}:{}:{}",
                    before.dev(),
                    before.ino(),
                    before.len(),
                    before.mtime(),
                    before.mtime_nsec(),
                    before.ctime(),
                    before.ctime_nsec()
                );
                let content = if binary {
                    Content::Unreadable
                } else {
                    Content::Text {
                        path: relative.clone(),
                        range: None,
                    }
                };
                Ok((hash, size, revision_metadata, content))
            })();
            let (hash, size, revision_metadata, content) = match identity {
                Ok(identity) => identity,
                Err(e) if e.code == "io" || e.code == "unavailable" => {
                    *unavailable_objects += 1;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let media_type = if matches!(content, Content::Text { .. }) {
                "text/plain; charset=utf-8"
            } else {
                "application/octet-stream"
            }
            .into();
            let metadata = BTreeMap::from([("path".into(), relative.clone())]);
            let revision = digest(format!("{hash}:{revision_metadata}:{name}").as_bytes());
            documents.push(Document {
                key: SourceKey {
                    namespace: "directory".into(),
                    id: relative,
                },
                revision,
                digest: hash,
                size,
                name: name.clone(),
                title: name,
                media_type,
                content,
                text_ranges: None,
                language: None,
                metadata,
                locator: None,
            });
        }
        Ok(())
    }
}
/// Own a native directory stream until all bounded names have been copied.
struct Names(*mut libc::DIR);
impl Drop for Names {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}
/// Collect bounded names from a held descriptor, including dotfiles but excluding dot entries.
fn names(directory: &File) -> Result<Vec<std::ffi::OsString>> {
    use std::os::fd::IntoRawFd;
    let fd = directory.try_clone().map_err(io_error)?.into_raw_fd();
    // SAFETY: fdopendir takes ownership of the duplicated descriptor on success.
    let raw = unsafe { libc::fdopendir(fd) };
    if raw.is_null() {
        unsafe {
            libc::close(fd);
        }
        return Err(error("unavailable", "directory enumeration unavailable"));
    }
    let stream = Names(raw);
    let mut names = Vec::new();
    loop {
        // SAFETY: readdir's pointer remains valid until the next call on this live stream.
        #[cfg(target_os = "linux")]
        unsafe {
            *libc::__errno_location() = 0;
        }
        #[cfg(target_os = "macos")]
        unsafe {
            *libc::__error() = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            if std::io::Error::last_os_error().raw_os_error() != Some(0) {
                return Err(error("unavailable", "directory enumeration stopped"));
            }
            break;
        }
        let bytes = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        if names.len() == DOCUMENT_LIMIT {
            return Err(error(
                "resource-limited",
                "directory entry budget exhausted",
            ));
        }
        names.push(std::ffi::OsString::from_vec(bytes.to_vec()));
    }
    names.sort();
    Ok(names)
}
