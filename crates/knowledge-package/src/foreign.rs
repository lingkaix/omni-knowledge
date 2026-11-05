//! Genuine OpenKit retained-v1 directory/TAR inventory, inert projection, and unchanged re-export.
use crate::{
    ContentDigest, Error, RelativePath, V1File, VerifiedSource, Work, charge_fresh, gate, metadata,
    native, note_file, operation_id, preallocate, read_source, source,
};
use serde_json::Value;
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Verified foreign directory whose provenance remains data, not local publication or grants.
/// The held root and exclusive Work borrow pin all callbacks and their admission lifetime.
/// Paths are projected identically, so prose, code, annotations, links, and digest pins do not change.
/// External mount locations remain inert declarations and are never opened.
pub struct V1Package<'w, 'a> {
    pub(crate) root: File,
    pub(crate) carrier: Carrier,
    pub(crate) manifest: Value,
    pub(crate) manifest_bytes: Vec<u8>,
    pub(crate) manifest_digest: ContentDigest,
    pub(crate) files: Vec<V1File>,
    pub(crate) work: &'w mut Work<'a>,
}

/// Source-compatible name for the original directory entry point.
pub type V1Directory<'w, 'a> = V1Package<'w, 'a>;

/// Real native container choice; a TAR never masquerades as an extracted filesystem.
pub(crate) enum Carrier {
    Directory,
    Tar(crate::tar::Index),
}

impl<'w, 'a> V1Package<'w, 'a> {
    /// Verify the actual retained-v1 schema, inventory closure, mounts, hashes, and native files.
    /// Caller quotas include directories and the manifest, unlike the native ZIP inventory count.
    /// Rejects symlinks/hardlinks/special files, undeclared bytes, reserved mount subtrees, and drift.
    pub fn open(path: &Path, work: &'w mut Work<'a>) -> Result<Self, Error> {
        let root = native::bind(path, work)?;
        Self::from_root(root, work)
    }

    /// Verify an already bound directory without trusting a mutable pathname.
    pub(crate) fn from_root(root: File, work: &'w mut Work<'a>) -> Result<Self, Error> {
        let mut file = native::file(&root, Path::new("manifest.json"), false, work)?;
        note_file(work)?;
        let before = source::Version::of(&file.metadata().map_err(|_| Error::Io)?);
        let meta = file.metadata().map_err(|_| Error::Io)?;
        if meta.nlink() != 1 || meta.len() > 1_048_576 {
            return Err(Error::Invalid);
        }
        let bytes = read_source(&mut file, work)?;
        note_file(work)?;
        if source::Version::of(&file.metadata().map_err(|_| Error::Io)?) != before {
            return Err(Error::Drift);
        }
        let scratch = metadata::parsing(&bytes, work)?;
        let manifest: Value = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
        let files = parse(&manifest, work)?;
        drop(scratch);
        let mut entries = 0;
        let mut total = 0;
        verify_tree(
            &root,
            Path::new(""),
            &manifest,
            &files,
            work,
            &mut entries,
            &mut total,
        )?;
        for entry in &files {
            let _source = source::open(&root, Path::new(entry.path.as_str()), &entry.sha256, work)?;
        }
        let manifest_digest = crate::digest_bytes(&bytes)?;
        Ok(Self {
            root,
            carrier: Carrier::Directory,
            manifest,
            manifest_bytes: bytes,
            manifest_digest,
            files,
            work,
        })
    }

    /// Verify a genuine uncompressed POSIX ustar/PAX carrier on its original held descriptor.
    /// Complete inventory hashing precedes access; no file is extracted or external root contacted.
    pub fn open_tar(file: File, work: &'w mut Work<'a>) -> Result<Self, Error> {
        let (index, bytes) = crate::tar::open(&file, work)?;
        let scratch = metadata::parsing(&bytes.bytes, work)?;
        let manifest: Value = serde_json::from_slice(&bytes.bytes).map_err(|_| Error::Invalid)?;
        let files = parse(&manifest, work)?;
        let mut actual = 0;
        for member in index.members.values() {
            gate(work)?;
            if member.path.as_str() == "manifest.json" && !member.directory {
                continue;
            }
            valid_path(member.path.as_str(), member.directory, &manifest, work)?;
            if !member.directory {
                actual += 1;
                let expected = &files[files
                    .binary_search_by(|f| f.path.as_str().cmp(member.path.as_str()))
                    .map_err(|_| Error::Hostile)?];
                if expected.size != member.length {
                    return Err(Error::Checksum);
                }
            }
        }
        if actual != files.len() {
            return Err(Error::Invalid);
        }
        drop(scratch);
        let digest = crate::digest_bytes(&bytes.bytes)?;
        charge_fresh(work, bytes.bytes.len() as u64, 0)?;
        let manifest_bytes = bytes.bytes.clone();
        let package = Self {
            root: file,
            carrier: Carrier::Tar(index),
            manifest,
            manifest_bytes,
            manifest_digest: digest,
            files,
            work,
        };
        for entry in &package.files {
            let _verified = open_member(&package.root, &package.carrier, entry, package.work)?;
            package.work.expanded = package
                .work
                .expanded
                .checked_add(entry.size)
                .ok_or(Error::Quota)?;
            if package.work.expanded > package.work.bounds.quotas.max_total_expanded {
                return Err(Error::Quota);
            }
        }
        if let Carrier::Tar(index) = &package.carrier {
            index.revalidate(&package.root, package.work)?;
        }
        Ok(package)
    }

    /// Recheck original context and closure before any output publication, never reopening a TAR name.
    pub(crate) fn revalidate_origin(&mut self) -> Result<(), Error> {
        match &self.carrier {
            Carrier::Tar(index) => index.revalidate(&self.root, self.work),
            Carrier::Directory => {
                let _manifest = source::open(
                    &self.root,
                    Path::new("manifest.json"),
                    &self.manifest_digest,
                    self.work,
                )?;
                verify_tree(
                    &self.root,
                    Path::new(""),
                    &self.manifest,
                    &self.files,
                    self.work,
                    &mut 0,
                    &mut 0,
                )
            }
        }
    }

    /// Complete exact-byte core mapping; foreign and native relative names are identical.
    /// Foreign Review/owner/schema labels remain inert and do not grant local permissions.
    pub fn files(&self) -> &[V1File] {
        &self.files
    }

    /// Original foreign context, including required owner/base/revision and optional extensions.
    /// A new native library cannot manufacture this context with DTO booleans or a Git commit.
    pub fn context(&self) -> &Value {
        &self.manifest
    }

    /// Copy or inspect one inventory member through the same verified-source owner capability.
    /// This does not install bytes. Storage must revalidate its exact copied stream before publication.
    pub fn with_file<T>(
        &mut self,
        path: &RelativePath,
        body: impl FnOnce(&mut VerifiedSource, &mut Work<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let entry = self
            .files
            .iter()
            .find(|f| f.path == *path)
            .ok_or(Error::Invalid)?;
        let mut source = open_member(&self.root, &self.carrier, entry, self.work)?;
        body(&mut source, self.work)
    }

    /// Re-export only this genuinely supplied context and unchanged inventoried bytes.
    /// Writes a private directory, validates it, then atomically renames without replacement.
    /// Changed content requires host finalization and refuses here; no revision is invented.
    /// Partial output remains private evidence on failure; unrelated destinations are never deleted.
    pub fn reexport(
        &mut self,
        private: &Path,
        operation: &str,
        destination: &Path,
    ) -> Result<(), Error> {
        operation_id(operation)?;
        let private = native::bind(private, self.work)?;
        note_file(self.work)?;
        rustix::fs::mkdirat(&private, operation, rustix::fs::Mode::from_raw_mode(0o700)).map_err(
            |e| {
                if e == rustix::io::Errno::EXIST {
                    Error::Interrupted
                } else {
                    Error::Io
                }
            },
        )?;
        let target = native::directory(&private, Path::new(operation), false, self.work)?;
        // Revalidate the original context and entire source closure before any publication.
        self.revalidate_origin()?;
        for entry in &self.files {
            let mut source = open_member(&self.root, &self.carrier, entry, self.work)?;
            let mut output =
                native::file(&target, Path::new(entry.path.as_str()), true, self.work)?;
            source.copy_to(&mut output, self.work)?;
            output.sync_all().map_err(|_| Error::Io)?;
        }
        let mut manifest = native::file(&target, Path::new("manifest.json"), true, self.work)?;
        charge_fresh(self.work, 0, self.manifest_bytes.len() as u64)?;
        // Reconcile short writes, not only the final write_all result.
        let mut at = 0;
        while at < self.manifest_bytes.len() {
            gate(self.work)?;
            let n = manifest
                .write(&self.manifest_bytes[at..])
                .map_err(|_| Error::Io)?;
            if n == 0 {
                return Err(Error::Io);
            }
            crate::note_written(self.work, n as u64)?;
            at += n;
        }
        manifest.sync_all().map_err(|_| Error::Io)?;
        drop(manifest);
        let validated = V1Directory::from_root(target, self.work)?;
        let target = validated.root;
        drop(validated.manifest);
        sync_directories(&target, &self.files, self.work)?;
        note_file(self.work)?;
        let held = rustix::fs::fstat(&target).map_err(|_| Error::Io)?;
        let named = rustix::fs::statat(&private, operation, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Error::Drift)?;
        if held.st_dev != named.st_dev || held.st_ino != named.st_ino {
            return Err(Error::Drift);
        }
        let parent = native::bind(destination.parent().ok_or(Error::Hostile)?, self.work)?;
        note_file(self.work)?;
        rustix::fs::renameat_with(
            &private,
            operation,
            &parent,
            destination.file_name().ok_or(Error::Hostile)?,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|e| {
            if e == rustix::io::Errno::EXIST || e == rustix::io::Errno::NOTEMPTY {
                Error::Conflict
            } else {
                Error::Io
            }
        })?;
        // Publication succeeded, but a durability failure is not permission to replay.
        parent.sync_all().map_err(|_| Error::Indeterminate)
    }
}

/// Synchronize known output parents child-first before publishing a directory artifact.
/// Names borrow the already verified inventory; scratch survives the complete temporary set.
pub(crate) fn sync_directories(
    root: &File,
    files: &[V1File],
    work: &mut Work<'_>,
) -> Result<(), Error> {
    gate(work)?;
    let count = files.iter().try_fold(0u64, |n, file| {
        n.checked_add(file.path.as_str().bytes().filter(|b| *b == b'/').count() as u64)
            .ok_or(Error::TooLarge)
    })?;
    let _pin = preallocate(
        work,
        count
            .checked_mul(128)
            .and_then(|n| n.checked_add(512))
            .ok_or(Error::TooLarge)?,
    )?;
    let mut parents = std::collections::BTreeSet::new();
    for file in files {
        gate(work)?;
        let path = file.path.as_str();
        for (at, _) in path.match_indices('/') {
            parents.insert(&path[..at]);
        }
    }
    for path in parents.into_iter().rev() {
        let directory = native::directory(root, Path::new(path), false, work)?;
        gate(work)?;
        directory.sync_all().map_err(|_| Error::Io)?;
    }
    gate(work)?;
    root.sync_all().map_err(|_| Error::Io)
}

/// Reuse the exact owner capability for a native file or bounded held-TAR member.
pub(crate) fn open_member(
    root: &File,
    carrier: &Carrier,
    entry: &V1File,
    work: &mut Work<'_>,
) -> Result<VerifiedSource, Error> {
    let source = match carrier {
        Carrier::Directory => {
            source::open(root, Path::new(entry.path.as_str()), &entry.sha256, work)?
        }
        Carrier::Tar(index) => {
            index.revalidate(root, work)?;
            let member = index.member(entry.path.as_str(), work)?;
            let source = source::range(root, member.offset, member.length, &entry.sha256, work)?;
            index.revalidate(root, work)?;
            source
        }
    };
    if source.len() != entry.size {
        return Err(Error::Checksum);
    }
    Ok(source)
}

/// Validate the foreign schema before inventory reads; extensions remain in the retained Value.
pub(crate) fn parse(value: &Value, work: &mut Work<'_>) -> Result<Vec<V1File>, Error> {
    if value["schemaVersion"].as_u64() != Some(1)
        || value["format"].as_str() != Some("openkit.knowledge-package")
    {
        return Err(Error::Unsupported);
    }
    let scope = value["ownerScope"].as_object().ok_or(Error::Invalid)?;
    let scope_ok = match scope.get("kind").and_then(Value::as_str) {
        Some("server") => scope.len() == 1,
        Some("user") => {
            scope.len() == 2
                && scope
                    .get("userId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| entity(id, "usr", false))
        }
        Some("workspace") => {
            scope.len() == 2
                && scope
                    .get("workspaceId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| entity(id, "ws", true))
        }
        _ => false,
    };
    if !scope_ok
        || !value["knowledgeBaseId"]
            .as_str()
            .is_some_and(|id| entity(id, "kb", false))
    {
        return Err(Error::Invalid);
    }
    let revision = value["notebookRevision"].as_str().ok_or(Error::Invalid)?;
    if revision.is_empty() || revision.len() > 256 {
        return Err(Error::Invalid);
    }
    let features = value["requiredFeatures"].as_array().ok_or(Error::Invalid)?;
    for (i, feature) in features.iter().enumerate() {
        if !matches!(
            feature.as_str(),
            Some("openkit.knowledge-git-notebook.v1" | "openkit.knowledge-owner-scope.v1")
        ) {
            return Err(Error::Unsupported);
        }
        if features[..i].contains(feature) {
            return Err(Error::Invalid);
        }
    }
    let mounts = value["mounts"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or(Error::Invalid)?;
    let mut defaults = 0;
    for (i, mount) in mounts.iter().enumerate() {
        let name = bounded(&mount["name"], 255)?;
        if name == "."
            || name == ".."
            || name.contains(['/', '\\'])
            || mounts[..i].iter().any(|m| m["name"] == mount["name"])
        {
            return Err(Error::Invalid);
        }
        let backend = bounded(&mount["backend"], 128)?;
        let root = bounded(&mount["root"], 4096)?;
        if root.contains("://") && (root.contains('@') || root.contains('?') || root.contains('#'))
        {
            return Err(Error::Forbidden);
        }
        if mount["default"].as_bool().ok_or(Error::Invalid)? {
            defaults += 1;
            if backend != "local" || root != "raw/" {
                return Err(Error::Invalid);
            }
        }
    }
    if defaults != 1 {
        return Err(Error::Invalid);
    }
    let inventory = value["contentInventory"].as_array().ok_or(Error::Invalid)?;
    if inventory.len() as u64 >= work.bounds.quotas.max_entries {
        return Err(Error::Quota);
    }
    charge_fresh(work, inventory.len() as u64 * 256, 0)?;
    let mut files = Vec::new();
    files
        .try_reserve_exact(inventory.len())
        .map_err(|_| Error::TooLarge)?;
    for item in inventory {
        gate(work)?;
        let path = item["path"].as_str().ok_or(Error::Invalid)?;
        valid_path(path, false, value, work)?;
        let sha256 = ContentDigest::new(
            item["contentDigest"]
                .as_str()
                .and_then(|s| s.strip_prefix("sha256:"))
                .ok_or(Error::Invalid)?,
        )
        .map_err(|_| Error::Invalid)?;
        let size = item["byteLength"]
            .as_u64()
            .filter(|n| *n <= 9_007_199_254_740_991)
            .ok_or(Error::Invalid)?;
        if size > work.bounds.quotas.max_entry_expanded {
            return Err(Error::Quota);
        }
        charge_fresh(work, path.len() as u64 + 128, 0)?;
        files.push(V1File {
            path: RelativePath::new(path).map_err(|_| Error::Hostile)?,
            sha256,
            size,
        });
    }
    files.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    for (i, file) in files.iter().enumerate() {
        if files[..i].iter().any(|prior| {
            let a = prior.path.as_str();
            let b = file.path.as_str();
            a == b
                || b.strip_prefix(a)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }) {
            return Err(Error::Hostile);
        }
    }
    Ok(files)
}

/// Apply the genuine family/path and reserved-mount rules, including empty directories.
pub(crate) fn valid_path(
    path: &str,
    directory: bool,
    manifest: &Value,
    work: &Work<'_>,
) -> Result<(), Error> {
    if path.len() > work.bounds.quotas.max_path_bytes
        || path.split('/').count() > work.bounds.quotas.max_depth
    {
        return Err(Error::Quota);
    }
    crate::manifest::path(path)?;
    if path.chars().any(|c| c.is_control()) {
        return Err(Error::Hostile);
    }
    let mut parts = path.split('/');
    let family = parts.next().ok_or(Error::Hostile)?;
    if !matches!(family, "pages" | "sources" | "schema" | "raw" | "indexes")
        || (!directory && parts.clone().next().is_none())
        || (!directory && matches!(family, "pages" | "sources") && !path.ends_with(".md"))
    {
        return Err(Error::Hostile);
    }
    if family == "raw"
        && let Some(second) = parts.next()
        && manifest["mounts"]
            .as_array()
            .ok_or(Error::Invalid)?
            .iter()
            .any(|m| m["default"] == false && m["name"] == second)
    {
        return Err(Error::Hostile);
    }
    Ok(())
}

/// Enumerate actual held-directory objects; reject undeclared files before reading their bytes.
#[allow(clippy::too_many_arguments)]
fn verify_tree(
    root: &File,
    prefix: &Path,
    manifest: &Value,
    files: &[V1File],
    work: &mut Work<'_>,
    count: &mut u64,
    total: &mut u64,
) -> Result<(), Error> {
    let _scratch = preallocate(work, 32_768)?;
    note_file(work)?;
    let mut dir = rustix::fs::Dir::read_from(root).map_err(|_| Error::Io)?;
    while let Some(entry) = dir.read() {
        gate(work)?;
        let entry = entry.map_err(|_| Error::Io)?;
        let name = entry.file_name().to_str().map_err(|_| Error::Hostile)?;
        if matches!(name, "." | "..") {
            continue;
        }
        *count = count.checked_add(1).ok_or(Error::Quota)?;
        if *count > work.bounds.quotas.max_entries {
            return Err(Error::Quota);
        }
        note_file(work)?;
        let stat = rustix::fs::statat(root, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Error::Io)?;
        let _path_scratch = preallocate(work, (prefix.as_os_str().len() + name.len() + 1) as u64)?;
        let path = prefix.join(name);
        let path_text = path.to_str().ok_or(Error::Hostile)?;
        match rustix::fs::FileType::from_raw_mode(stat.st_mode) {
            rustix::fs::FileType::Directory => {
                valid_path(path_text, true, manifest, work)?;
                let sub = native::directory(root, Path::new(name), false, work)?;
                verify_tree(&sub, &path, manifest, files, work, count, total)?;
            }
            rustix::fs::FileType::RegularFile => {
                if stat.st_nlink != 1 {
                    return Err(Error::Hostile);
                }
                let length = u64::try_from(stat.st_size).map_err(|_| Error::Hostile)?;
                *total = total.checked_add(length).ok_or(Error::Quota)?;
                if *total > work.bounds.budget.max_bytes
                    || *total > work.bounds.quotas.max_total_expanded
                {
                    return Err(Error::Quota);
                }
                if path_text == "manifest.json" {
                    continue;
                }
                valid_path(path_text, false, manifest, work)?;
                let expected = files
                    .iter()
                    .find(|f| f.path.as_str() == path_text)
                    .ok_or(Error::Hostile)?;
                if expected.size != length {
                    return Err(Error::Checksum);
                }
            }
            _ => return Err(Error::Hostile),
        }
    }
    Ok(())
}

/// Validate the foreign owner's canonical kind-prefixed UUID identity without allocating a registry.
pub(crate) fn entity(id: &str, kind: &str, derived: bool) -> bool {
    let Some(uuid) = id.strip_prefix(kind).and_then(|s| s.strip_prefix('_')) else {
        return false;
    };
    let b = uuid.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                *c == b'-'
            } else {
                c.is_ascii_digit() || (b'a'..=b'f').contains(c)
            }
        })
        && (b[14] == b'7' || derived && b[14] == b'5')
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// Validate bounded foreign descriptive text without resolving or activating it.
fn bounded(value: &Value, max: usize) -> Result<&str, Error> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= max && !s.chars().any(|c| c.is_control()))
        .ok_or(Error::Invalid)
}
