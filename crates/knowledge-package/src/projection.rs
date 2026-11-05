//! Real v1 core projection and host-manifest-bound directory/TAR publication.
use crate::{
    ContentDigest, Error, RelativePath, V1File, V1Package, V1PathMap, V1ReferenceMap,
    VerifiedSource, Work, charge_fresh,
    foreign::{self},
    gate, metadata, native, note_file, operation_id, preallocate,
    remap::{self, PathPair},
    source::{self, BoundedBytes, Version},
    tar,
};
use serde_json::Value;
use std::{fs::File, io::Write, os::unix::fs::MetadataExt, path::Path};

/// Original identity and actual projected byte identity, with an explicit non-authorizing path map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V1MappedFile {
    /// Fully verified original bytes; includes raw SHA-256 pins even after relocation.
    pub source: V1File,
    /// Actual destination path, digest, and length after parsed destination rewriting.
    pub projected: V1File,
}

/// Actual output carrier; neither choice activates a library or imports Git/configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V1Carrier {
    /// Genuine retained-v1 directory, published by atomic no-replace rename.
    Directory,
    /// Uncompressed POSIX ustar/PAX file, published by atomic exclusive hard link.
    Tar,
}

/// Publication evidence returned only after native output validation and durable publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V1Artifact {
    /// SHA-256 of the actual host-supplied manifest bytes, not a Notebook revision or grant.
    pub manifest_sha256: ContentDigest,
    /// Number of complete inventory members validated in the output.
    pub files: usize,
    /// Actual selected payload bytes, excluding manifest and carrier framing.
    pub payload_bytes: u64,
}

/// Truthful context refusal is distinct from a completed native filesystem effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum V1ExportOutcome {
    /// Host finalization was not supplied; the same projection remains available for core use.
    CompatibilityRefused,
    /// Actual private output was validated and atomically published without replacement.
    Published(V1Artifact),
}

/// Selected native files or a genuine held foreign package, never an invented filesystem.
enum Input<'w, 'a> {
    Foreign(V1Package<'w, 'a>),
    Native {
        root: File,
        files: Vec<V1File>,
        managed_mount: Option<String>,
        work: &'w mut Work<'a>,
    },
}

impl<'a> Input<'_, 'a> {
    /// Existing shared control; no counter reset or second account is created.
    fn work(&mut self) -> &mut Work<'a> {
        match self {
            Self::Foreign(p) => p.work,
            Self::Native { work, .. } => work,
        }
    }
    /// Existing managed-mount name is a mapping datum, never a root activation or permission.
    fn managed_mount(&self) -> Option<&str> {
        match self {
            Self::Foreign(p) => p.manifest["mounts"]
                .as_array()?
                .iter()
                .find(|m| m["default"] == true)?["name"]
                .as_str(),
            Self::Native { managed_mount, .. } => managed_mount.as_deref(),
        }
    }
    /// Selected original identities, not a scan of an entire home or external mount.
    fn files(&self) -> &[V1File] {
        match self {
            Self::Foreign(p) => &p.files,
            Self::Native { files, .. } => files,
        }
    }
    /// Open and reverify the actual selected descriptor/extent before owner byte production.
    fn open(&mut self, entry: &V1File) -> Result<VerifiedSource, Error> {
        match self {
            Self::Foreign(p) => foreign::open_member(&p.root, &p.carrier, entry, p.work),
            Self::Native { root, work, .. } => {
                let source =
                    source::open(root, Path::new(entry.path.as_str()), &entry.sha256, work)?;
                if source.len() != entry.size {
                    return Err(Error::Checksum);
                }
                Ok(source)
            }
        }
    }
    /// Per-file copies already verify pinned descriptor bytes; TAR pins also bind the whole container.
    /// Full directory namespace/context closure is checked at preview/publication, not N times per file.
    fn revalidate_pin(&mut self) -> Result<(), Error> {
        match self {
            Self::Foreign(p) => match &p.carrier {
                foreign::Carrier::Tar(index) => index.revalidate(&p.root, p.work),
                foreign::Carrier::Directory => gate(p.work),
            },
            Self::Native { work, .. } => gate(work),
        }
    }
    /// Foreign publication provenance can be retained only while its verified input is unchanged.
    fn revalidate(&mut self) -> Result<(), Error> {
        match self {
            Self::Foreign(p) => p.revalidate_origin(),
            Self::Native { work, .. } => gate(work),
        }
    }
}

/// A complete byte/path preview pinned to its actual source and the original exclusive Work borrow.
/// Copies stream raw members; Markdown transformations retain only one admitted text unit at a time.
/// No archive content, owner scope, Review label, or digest authorizes a native destination.
pub struct V1Projection<'w, 'a> {
    files: Vec<V1MappedFile>,
    references: Vec<V1ReferenceMap>,
    sources: Vec<VerifiedSource>,
    rewrite_text: bool,
    input: Input<'w, 'a>,
}

impl<'w, 'a> V1Package<'w, 'a> {
    /// Consume this verified package into an explicit path/reference projection.
    /// Unlisted paths remain unchanged; unknown/duplicate/colliding mappings refuse before effects.
    pub fn project(
        self,
        paths: &[V1PathMap],
        references: &[V1ReferenceMap],
    ) -> Result<V1Projection<'w, 'a>, Error> {
        V1Projection::build(Input::Foreign(self), paths, references)
    }

    /// Re-export a genuine unchanged input directly as an uncompressed TAR without extraction.
    pub fn reexport_tar(
        self,
        private: &Path,
        operation: &str,
        destination: &Path,
    ) -> Result<V1Artifact, Error> {
        let mut projection = self.project(&[], &[])?;
        projection.publish_original_tar(private, operation, destination)
    }
}

impl<'w, 'a> V1Projection<'w, 'a> {
    /// Project explicitly selected ordinary native files, without fabricating foreign context.
    /// Source digests/lengths are verified against actual bytes; omitted home content is not scanned.
    /// Host-finalized export below needs a real supplied manifest that binds the resulting inventory.
    pub fn from_native(
        root: &Path,
        files: &[V1File],
        paths: &[V1PathMap],
        references: &[V1ReferenceMap],
        managed_mount: Option<&str>,
        work: &'w mut Work<'a>,
    ) -> Result<Self, Error> {
        gate(work)?;
        if files.len() as u64 >= work.bounds.quotas.max_entries.min(10_000) {
            return Err(Error::Quota);
        }
        let mut bytes = 0u64;
        for file in files {
            gate(work)?;
            tar::confined(file.path.as_str(), work)?;
            if file.size > work.bounds.quotas.max_entry_expanded || file.size > 1_000_000_000 {
                return Err(Error::Quota);
            }
            bytes = bytes.checked_add(file.size).ok_or(Error::Quota)?;
        }
        if bytes > work.bounds.quotas.max_total_expanded.min(8_589_934_592) {
            return Err(Error::Quota);
        }
        if managed_mount.is_some_and(|name| {
            name.is_empty()
                || name.len() > 255
                || name.contains(['/', '\\'])
                || name.chars().any(char::is_control)
        }) {
            return Err(Error::Invalid);
        }
        charge_fresh(
            work,
            files
                .iter()
                .map(|f| f.path.as_str().len() as u64 + 256)
                .sum::<u64>()
                + 512,
            0,
        )?;
        let managed_mount = managed_mount.map(str::to_owned);
        let files = files.to_vec();
        let root = native::bind(root, work)?;
        Self::build(
            Input::Native {
                root,
                files,
                managed_mount,
                work,
            },
            paths,
            references,
        )
    }

    /// Admit and validate every mapping before hashing projected bytes or exposing a core preview.
    fn build(
        mut input: Input<'w, 'a>,
        paths: &[V1PathMap],
        references: &[V1ReferenceMap],
    ) -> Result<Self, Error> {
        input.revalidate()?;
        let count = input.files().len();
        if paths.len() > count || references.len() as u64 > input.work().bounds.quotas.max_entries {
            return Err(Error::Quota);
        }
        let retained = input
            .files()
            .iter()
            .map(|f| f.path.as_str().len() as u64 * 4 + 1024)
            .sum::<u64>()
            .checked_add(
                paths
                    .iter()
                    .map(|p| p.to.as_str().len() as u64 * 4 + 256)
                    .sum::<u64>(),
            )
            .and_then(|n| {
                n.checked_add(
                    references
                        .iter()
                        .map(|r| (r.from.len() + r.to.len()) as u64 * 4 + 256)
                        .sum(),
                )
            })
            .ok_or(Error::TooLarge)?;
        charge_fresh(input.work(), retained, 0)?;
        let _scratch = preallocate(
            input.work(),
            retained.checked_add(8192).ok_or(Error::TooLarge)?,
        )?;
        let source_files = input.files().to_vec();
        let mut selected = std::collections::BTreeSet::new();
        for f in &source_files {
            gate(input.work())?;
            if !selected.insert(f.path.as_str()) {
                return Err(Error::Hostile);
            }
        }
        let mut map = std::collections::BTreeMap::new();
        for path in paths {
            gate(input.work())?;
            if !selected.contains(path.from.as_str())
                || map.insert(path.from.as_str(), path.to.as_str()).is_some()
            {
                return Err(Error::Invalid);
            }
            tar::confined(path.to.as_str(), input.work())?;
        }
        let mut refs = std::collections::BTreeSet::new();
        for reference in references {
            gate(input.work())?;
            if reference.from.len()
                > input
                    .work()
                    .bounds
                    .quotas
                    .max_path_bytes
                    .saturating_add(512)
                || reference.to.len()
                    > input
                        .work()
                        .bounds
                        .quotas
                        .max_path_bytes
                        .saturating_add(512)
            {
                return Err(Error::Quota);
            }
            remap::validate_reference(reference)?;
            if !refs.insert(reference.from.as_str()) {
                return Err(Error::Invalid);
            }
        }
        let mut files = Vec::new();
        files
            .try_reserve_exact(count)
            .map_err(|_| Error::TooLarge)?;
        let mut names = std::collections::BTreeSet::<String>::new();
        for source in &source_files {
            gate(input.work())?;
            let target = map
                .get(source.path.as_str())
                .copied()
                .unwrap_or(source.path.as_str());
            let folded = target.to_lowercase();
            if folded == "manifest.json" || !names.insert(folded) {
                return Err(Error::Hostile);
            }
            files.push(V1MappedFile {
                source: source.clone(),
                projected: V1File {
                    path: RelativePath::new(target).map_err(|_| Error::Hostile)?,
                    sha256: source.sha256.clone(),
                    size: source.size,
                },
            });
        }
        for name in &names {
            gate(input.work())?;
            for (at, _) in name.match_indices('/') {
                if names.contains(&name[..at]) {
                    return Err(Error::Hostile);
                }
            }
        }
        drop(map);
        drop(selected);
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(count)
            .map_err(|_| Error::TooLarge)?;
        for file in &files {
            sources.push(input.open(&file.source)?);
        }
        let rewrite_text = files.iter().any(|f| f.source.path != f.projected.path)
            || references.iter().any(|r| r.from != r.to);
        let mut projection = Self {
            files,
            references: references.to_vec(),
            sources,
            rewrite_text,
            input,
        };
        for i in 0..projection.files.len() {
            gate(projection.input.work())?;
            if projection.rewrite_text && text_member(&projection.files[i]) {
                let text = projection.render(i)?;
                projection.files[i].projected.sha256 = crate::digest_bytes(&text.bytes)?;
                projection.files[i].projected.size = text.bytes.len() as u64;
            }
        }
        projection.input.revalidate()?;
        Ok(projection)
    }

    /// Explicit equivalence table for source identities, asset locations, and projected core bytes.
    pub fn files(&self) -> &[V1MappedFile] {
        &self.files
    }

    /// Explicit logical reference remints; foreign scopes and labels remain inert, not grants.
    pub fn references(&self) -> &[V1ReferenceMap] {
        &self.references
    }

    /// Recompute one bounded text unit through parser-confirmed destinations on the held source.
    fn render(&mut self, index: usize) -> Result<BoundedBytes, Error> {
        let original = self.sources[index].read_all(self.input.work(), 2 * 1024 * 1024)?;
        let _pin = preallocate(self.input.work(), self.files.len() as u64 * 64 + 1024)?;
        let managed_mount = self.input.managed_mount().map(str::to_owned);
        let paths: Vec<_> = self
            .files
            .iter()
            .map(|f| PathPair {
                from: f.source.path.as_str(),
                to: f.projected.path.as_str(),
                digest: &f.source.sha256,
            })
            .collect();
        let file = &self.files[index];
        remap::rewrite(
            &original.bytes,
            file.source.path.as_str(),
            file.projected.path.as_str(),
            &paths,
            &self.references,
            managed_mount.as_deref(),
            self.input.work(),
        )
    }

    /// Produce the exact planned bytes into an owner's actual private writer, never a destination path.
    /// The owner must not publish on error; cancellation or drift can leave partial private evidence.
    pub fn copy_file(&mut self, path: &RelativePath, output: &mut dyn Write) -> Result<(), Error> {
        gate(self.input.work())?;
        let index = self
            .files
            .iter()
            .position(|f| f.projected.path == *path)
            .ok_or(Error::Invalid)?;
        if self.rewrite_text && text_member(&self.files[index]) {
            let bytes = self.render(index)?;
            if bytes.bytes.len() as u64 != self.files[index].projected.size
                || crate::digest_bytes(&bytes.bytes)? != self.files[index].projected.sha256
            {
                return Err(Error::Drift);
            }
            tar::write(output, &bytes.bytes, self.input.work())?;
            self.sources[index].revalidate(self.input.work())?;
        } else {
            self.sources[index].copy_to(output, self.input.work())?;
        }
        self.input.revalidate_pin()
    }

    /// Export only with an actual host-supplied finalized manifest that exactly binds this preview.
    /// None returns CompatibilityRefused with no private/destination access and keeps the core preview.
    /// No owner/revision/profile token or native Git commit is promoted to Notebook publication.
    pub fn export(
        &mut self,
        finalized_manifest: Option<File>,
        carrier: V1Carrier,
        private: &Path,
        operation: &str,
        destination: &Path,
    ) -> Result<V1ExportOutcome, Error> {
        gate(self.input.work())?;
        let Some(file) = finalized_manifest else {
            return Ok(V1ExportOutcome::CompatibilityRefused);
        };
        let context = Finalization::open(file, self.input.work())?;
        self.check_context(&context)?;
        let artifact = self.publish(context, carrier, private, operation, destination)?;
        Ok(V1ExportOutcome::Published(artifact))
    }

    /// Compare actual projected byte identities to actual finalized host inventory, in either order.
    fn check_context(&mut self, context: &Finalization) -> Result<(), Error> {
        if context.files.len() != self.files.len() {
            return Err(Error::Checksum);
        }
        if matches!(self.input, Input::Native { .. })
            && !context
                .files
                .iter()
                .any(|f| f.path.as_str() == "schema/scope-schema.yaml" && f.size != 0)
        {
            // Current v1 host import/publication owns this exact profile; a new native home cannot invent it.
            return Err(Error::CompatibilityRefused);
        }
        if let Input::Native {
            managed_mount: Some(mount),
            ..
        } = &self.input
            && !context.value["mounts"].as_array().is_some_and(|ms| {
                ms.iter()
                    .any(|m| m["default"] == true && m["name"] == *mount)
            })
        {
            return Err(Error::CompatibilityRefused);
        }
        let _pin = preallocate(self.input.work(), self.files.len() as u64 * 128 + 1024)?;
        let mut inventory = std::collections::BTreeMap::new();
        for file in &self.files {
            inventory.insert(file.projected.path.as_str(), &file.projected);
        }
        for file in &context.files {
            gate(self.input.work())?;
            if inventory.remove(file.path.as_str()) != Some(file) {
                return Err(Error::Checksum);
            }
        }
        if let Input::Foreign(original) = &self.input {
            let changed = self.files.iter().any(|f| f.source != f.projected)
                || self.references.iter().any(|r| r.from != r.to);
            if changed && context.value["notebookRevision"] == original.manifest["notebookRevision"]
            {
                return Err(Error::CompatibilityRefused);
            }
        }
        Ok(())
    }

    /// Preserve genuinely supplied unchanged provenance through the same validated TAR publisher.
    fn publish_original_tar(
        &mut self,
        private: &Path,
        operation: &str,
        destination: &Path,
    ) -> Result<V1Artifact, Error> {
        let Input::Foreign(package) = &mut self.input else {
            return Err(Error::Invalid);
        };
        package.revalidate_origin()?;
        let manifest_entry = V1File {
            path: RelativePath::new("manifest.json").map_err(|_| Error::Invalid)?,
            sha256: package.manifest_digest.clone(),
            size: package.manifest_bytes.len() as u64,
        };
        let mut manifest = foreign::open_member(
            &package.root,
            &package.carrier,
            &manifest_entry,
            package.work,
        )?;
        // Use an actual owner-private file for the identical supplied context, not a fabricated context DTO.
        operation_id(operation)?;
        let private_root = native::bind(private, package.work)?;
        let stage = create_stage(&private_root, operation, package.work)?;
        let mut copy = native::file(&stage, Path::new("HOST-MANIFEST"), true, package.work)?;
        manifest.copy_to(&mut copy, package.work)?;
        copy.sync_all().map_err(|_| Error::Io)?;
        let context = Finalization::open(copy, package.work)?;
        self.check_context(&context)?;
        self.publish_in_stage(
            context,
            V1Carrier::Tar,
            &private_root,
            &stage,
            operation,
            destination,
        )
    }

    /// Create private evidence only after context and mapping validation have succeeded.
    fn publish(
        &mut self,
        context: Finalization,
        carrier: V1Carrier,
        private: &Path,
        operation: &str,
        destination: &Path,
    ) -> Result<V1Artifact, Error> {
        operation_id(operation)?;
        let root = native::bind(private, self.input.work())?;
        let stage = create_stage(&root, operation, self.input.work())?;
        self.publish_in_stage(context, carrier, &root, &stage, operation, destination)
    }

    /// Write, reopen, validate, and atomically publish the real selected carrier; never replay/overwrite.
    fn publish_in_stage(
        &mut self,
        context: Finalization,
        carrier: V1Carrier,
        root: &File,
        stage: &File,
        operation: &str,
        destination: &Path,
    ) -> Result<V1Artifact, Error> {
        self.input.revalidate()?;
        let mut tar_file = if carrier == V1Carrier::Tar {
            Some(native::file(
                stage,
                Path::new("package.tar"),
                true,
                self.input.work(),
            )?)
        } else {
            None
        };
        let directory = if carrier == V1Carrier::Directory {
            Some(native::directory(
                stage,
                Path::new("package"),
                true,
                self.input.work(),
            )?)
        } else {
            None
        };
        if let Some(output) = tar_file.as_mut() {
            tar::header(
                output,
                "manifest.json",
                context.bytes.bytes.len() as u64,
                self.input.work(),
            )?;
            tar::write(output, &context.bytes.bytes, self.input.work())?;
            tar::padding(output, context.bytes.bytes.len() as u64, self.input.work())?;
        }
        for i in 0..self.files.len() {
            gate(self.input.work())?;
            let path = self.files[i].projected.path.clone();
            let size = self.files[i].projected.size;
            if let Some(output) = tar_file.as_mut() {
                tar::header(output, path.as_str(), size, self.input.work())?;
                self.copy_file(&path, output)?;
                tar::padding(output, size, self.input.work())?;
            } else {
                let mut output = native::file(
                    directory.as_ref().ok_or(Error::Invalid)?,
                    Path::new(path.as_str()),
                    true,
                    self.input.work(),
                )?;
                self.copy_file(&path, &mut output)?;
                output.sync_all().map_err(|_| Error::Io)?;
            }
        }
        if let Some(output) = tar_file.as_mut() {
            tar::write(output, &[0u8; 1024], self.input.work())?;
            output.sync_all().map_err(|_| Error::Io)?;
        } else {
            let mut output = native::file(
                directory.as_ref().ok_or(Error::Invalid)?,
                Path::new("manifest.json"),
                true,
                self.input.work(),
            )?;
            tar::write(&mut output, &context.bytes.bytes, self.input.work())?;
            output.sync_all().map_err(|_| Error::Io)?;
        }
        context.revalidate(self.input.work())?;
        self.input.revalidate()?;
        if let Some(file) = tar_file {
            let verified = V1Package::open_tar(file, self.input.work())?;
            let file = verified.root;
            drop(verified.manifest);
            context.revalidate(self.input.work())?;
            self.input.revalidate()?;
            note_file(self.input.work())?;
            let held = rustix::fs::fstat(&file).map_err(|_| Error::Io)?;
            let named =
                rustix::fs::statat(stage, "package.tar", rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|_| Error::Drift)?;
            if held.st_dev != named.st_dev || held.st_ino != named.st_ino {
                return Err(Error::Drift);
            }
            gate(self.input.work())?;
            if !native::link(stage, "package.tar", destination, self.input.work())? {
                return Err(Error::Conflict);
            }
            let parent = native::bind(
                destination.parent().ok_or(Error::Hostile)?,
                self.input.work(),
            )?;
            parent.sync_all().map_err(|_| Error::Indeterminate)?;
        } else {
            let directory = directory.ok_or(Error::Invalid)?;
            let verified = V1Package::from_root(directory, self.input.work())?;
            let directory = verified.root;
            drop(verified.manifest);
            foreign::sync_directories(&directory, &context.files, self.input.work())?;
            context.revalidate(self.input.work())?;
            self.input.revalidate()?;
            let parent = native::bind(
                destination.parent().ok_or(Error::Hostile)?,
                self.input.work(),
            )?;
            note_file(self.input.work())?;
            let held = rustix::fs::fstat(&directory).map_err(|_| Error::Io)?;
            let named = rustix::fs::statat(stage, "package", rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| Error::Drift)?;
            if held.st_dev != named.st_dev || held.st_ino != named.st_ino {
                return Err(Error::Drift);
            }
            rustix::fs::renameat_with(
                stage,
                "package",
                &parent,
                destination.file_name().ok_or(Error::Hostile)?,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|e| {
                if matches!(e, rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY) {
                    Error::Conflict
                } else {
                    Error::Io
                }
            })?;
            parent.sync_all().map_err(|_| Error::Indeterminate)?;
        }
        // Keep operation evidence; successful publication does not authorize cleanup of unknown files.
        let _ = (root, operation);
        Ok(V1Artifact {
            manifest_sha256: context.digest,
            files: self.files.len(),
            payload_bytes: self.files.iter().map(|f| f.projected.size).sum(),
        })
    }
}

/// Real held host manifest plus parsed inventory; never accepted merely from package strings.
struct Finalization {
    bytes: BoundedBytes,
    value: Value,
    files: Vec<V1File>,
    digest: ContentDigest,
    file: File,
    version: Version,
}
impl Finalization {
    /// Read and validate the actual supplied host context with bounded memory and descriptor version.
    fn open(file: File, work: &mut Work<'_>) -> Result<Self, Error> {
        note_file(work)?;
        let meta = file.metadata().map_err(|_| Error::Io)?;
        if !meta.is_file()
            || meta.nlink() != 1
            || meta.len() > 1_048_576
            || meta.len() > work.bounds.quotas.max_entry_expanded
        {
            return Err(Error::Invalid);
        }
        let version = Version::of(&meta);
        let mut bytes = BoundedBytes::reserve(meta.len(), work)?;
        bytes.bytes.resize(meta.len() as usize, 0);
        source::read_exact_at(&file, 0, &mut bytes.bytes, work)?;
        let _scratch = metadata::parsing(&bytes.bytes, work)?;
        let value: Value = serde_json::from_slice(&bytes.bytes).map_err(|_| Error::Invalid)?;
        let files = foreign::parse(&value, work)?;
        let digest = crate::digest_bytes(&bytes.bytes)?;
        let result = Self {
            bytes,
            value,
            files,
            digest,
            file,
            version,
        };
        result.revalidate(work)?;
        Ok(result)
    }
    /// Keep host context live through validation and the final publication gate.
    fn revalidate(&self, work: &mut Work<'_>) -> Result<(), Error> {
        note_file(work)?;
        if Version::of(&self.file.metadata().map_err(|_| Error::Io)?) != self.version {
            return Err(Error::Drift);
        }
        Ok(())
    }
}

/// Create exclusive inspectable evidence, preserving an interrupted/competing operation directory.
fn create_stage(root: &File, operation: &str, work: &mut Work<'_>) -> Result<File, Error> {
    note_file(work)?;
    rustix::fs::mkdirat(root, operation, rustix::fs::Mode::from_raw_mode(0o700)).map_err(|e| {
        if e == rustix::io::Errno::EXIST {
            Error::Interrupted
        } else {
            Error::Io
        }
    })?;
    native::directory(root, Path::new(operation), false, work)
}

/// Raw originals and schema/index bytes are never parsed or rewritten, even with a .md suffix.
fn text_member(file: &V1MappedFile) -> bool {
    file.source.path.as_str().ends_with(".md")
        && !["raw/", "schema/", "indexes/"].iter().any(|prefix| {
            file.source.path.as_str().starts_with(prefix)
                || file.projected.path.as_str().starts_with(prefix)
        })
}
