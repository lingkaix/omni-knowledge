//! Read-only archive capabilities over one verified, held native container.
use crate::source::Version;
use crate::{
    ContentDigest, Error, Manifest, Method, Opened, RelativePath, Work, gate, note_file, note_read,
    open_verified,
};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::FileExt;

/// A fully verified archive pinned to its native descriptor and canonical snapshot.
///
/// The exclusive `Work` borrow keeps the caller's SAME-account holding live until this
/// reader and every callback end. No path, writable handle, or unbounded slice escapes.
/// Opening scans all inventoried entries; this is not manifest-only verification.
/// Replacement never redirects this reader to another file. Observable in-place drift refuses.
pub struct FrozenArchive<'w, 'a> {
    file: File,
    version: Version,
    opened: Opened,
    work: &'w mut Work<'a>,
}

/// Opaque native container identity/version sampled around complete entry verification.
/// Equality includes device, inode, length, modification time, and change time.
/// This token is a data/version fact, never a filesystem or signer grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveVersion(Version);

/// One checked STORED entry's physical range, not permission to read the whole archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryRange {
    /// Absolute offset in the held container; supports ZIP64 positions above 4 GiB.
    pub offset: u64,
    /// Exact uncompressed length of this STORED member.
    pub length: u64,
}

impl<'w, 'a> FrozenArchive<'w, 'a> {
    /// Consume an admitted ordinary file and verify framing, inventory, CRC, and SHA-256.
    /// Refuses drift across verification, nonregular descriptors, quotas, and cancellation.
    /// No filesystem destination or packaged external reference is accessed.
    pub fn open(mut file: File, work: &'w mut Work<'a>) -> Result<Self, Error> {
        note_file(work)?;
        let metadata = file.metadata().map_err(|_| Error::Io)?;
        if !metadata.is_file() {
            return Err(Error::Hostile);
        }
        let version = Version::of(&metadata);
        let opened = open_verified(&mut file, work)?;
        note_file(work)?;
        if Version::of(&file.metadata().map_err(|_| Error::Io)?) != version {
            return Err(Error::Drift);
        }
        Ok(Self {
            file,
            version,
            opened,
            work,
        })
    }

    /// Canonical manifest digest, independent of ZIP layout and never a grant.
    pub fn snapshot_id(&self) -> &ContentDigest {
        &self.opened.snapshot_id
    }

    /// Pinned native identity/version, distinct from the layout-independent snapshot digest.
    /// `revalidate` and each actual read detect drift from this sampled version.
    pub fn version(&self) -> ArchiveVersion {
        ArchiveVersion(self.version.clone())
    }

    /// Existing shared work control for budgeted metadata-only checks; never a new counter set.
    pub(crate) fn control(&self) -> &Work<'_> {
        self.work
    }

    /// Inert parsed facts, including uncovered references and unsupported required features.
    pub fn manifest(&self) -> &Manifest {
        &self.opened.manifest
    }

    /// Revalidate the current held descriptor under the caller's examined-file ceiling.
    pub fn revalidate(&mut self) -> Result<(), Error> {
        check_version(&self.file, &self.version, self.work)
    }

    /// Run `body` on a digest-verified STORED entry with confined positional access.
    /// The range cannot escape the callback or outlive the container or admission holding.
    /// Deflated text is not a random-access source and returns `Unsupported` here.
    /// Callback success is withheld if the container version changes before return.
    pub fn with_stored<T>(
        &mut self,
        path: &RelativePath,
        body: impl FnOnce(&mut StoredRange<'_, '_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.revalidate()?;
        let entry = self
            .opened
            .manifest
            .entries
            .iter()
            .find(|e| e.path == *path)
            .ok_or(Error::Invalid)?;
        if entry.method != Method::Stored {
            return Err(Error::Unsupported);
        }
        let member = self
            .opened
            .members
            .iter()
            .find(|m| m.name == path.as_str())
            .ok_or(Error::Disagreement)?;
        let range = EntryRange {
            offset: member.data_offset,
            length: member.uncompressed,
        };
        if range.offset.checked_add(range.length).is_none() {
            return Err(Error::Hostile);
        }
        let mut reader = StoredRange {
            file: &self.file,
            version: &self.version,
            range,
            position: 0,
            work: self.work,
        };
        let result = body(&mut reader);
        check_version(&self.file, &self.version, self.work)?;
        result
    }
}

/// Borrowed read-only bounded source for a mature kernel's generic byte adapter.
/// Implements ordinary `Read + Seek` without materialization or a synthetic filesystem.
/// All reads check deadline, cancellation, actual bytes, and held-container version.
pub struct StoredRange<'w, 'a> {
    file: &'w File,
    version: &'w Version,
    range: EntryRange,
    position: u64,
    work: &'w mut Work<'a>,
}

impl StoredRange<'_, '_> {
    /// Physical admitted extent; callers cannot use it to obtain a raw descriptor.
    pub fn extent(&self) -> EntryRange {
        self.range
    }

    /// Read at an entry-relative offset, checking overflow and bounds before native access.
    /// EOF at the exact entry end returns zero. An offset beyond it refuses.
    /// Caller-owned output must already have admission; this method allocates no payload buffer.
    pub fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<usize, Error> {
        if offset > self.range.length {
            return Err(Error::Invalid);
        }
        check_version(self.file, self.version, self.work)?;
        let count = usize::try_from((self.range.length - offset).min(output.len() as u64))
            .map_err(|_| Error::TooLarge)?;
        let absolute = self
            .range
            .offset
            .checked_add(offset)
            .ok_or(Error::Hostile)?;
        let next = self
            .work
            .read_bytes
            .checked_add(count as u64)
            .ok_or(Error::Quota)?;
        if next > self.work.bounds.budget.max_bytes {
            return Err(Error::Quota);
        }
        let n = self
            .file
            .read_at(&mut output[..count], absolute)
            .map_err(|_| Error::Io)?;
        note_read(self.work, n as u64)?;
        if n == 0 && count != 0 {
            return Err(Error::Truncated);
        }
        check_version(self.file, self.version, self.work)?;
        Ok(n)
    }
}

impl Read for StoredRange<'_, '_> {
    /// Read only the admitted entry and advance the entry-relative cursor.
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let n = self
            .read_at(self.position, output)
            .map_err(std::io::Error::other)?;
        self.position += n as u64;
        Ok(n)
    }
}

impl Seek for StoredRange<'_, '_> {
    /// Seek within the entry, never into its ZIP header, neighbour, or central directory.
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        gate(self.work).map_err(std::io::Error::other)?;
        let position = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.range.length) + i128::from(n),
        };
        if position < 0 || position > i128::from(self.range.length) {
            return Err(std::io::Error::other(Error::Invalid));
        }
        self.position = position as u64;
        Ok(self.position)
    }
}

/// Sample only the held descriptor, admitting each fresh examination before metadata access.
fn check_version(file: &File, expected: &Version, work: &mut Work<'_>) -> Result<(), Error> {
    note_file(work)?;
    if Version::of(&file.metadata().map_err(|_| Error::Io)?) != *expected {
        return Err(Error::Drift);
    }
    Ok(())
}
