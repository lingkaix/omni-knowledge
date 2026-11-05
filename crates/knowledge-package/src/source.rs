//! Source capabilities retain the verified native descriptor through owner delegation.
use crate::{
    ContentDigest, Error, Work, charge_fresh, gate, hex, native, note_file, note_read,
    note_written, preallocate,
};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::Path;

/// A verified native file, not a mutable path presented as proof.
/// Owners copy into their own private candidate with [`Self::copy_to`] and publish only on success.
/// No path or writable descriptor is exposed. In-place changes are checked by version and stream digest.
pub struct VerifiedSource {
    file: File,
    digest: ContentDigest,
    version: Version,
    extent: Option<(u64, u64)>,
}

/// Descriptor identity and observable content version on Unix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Version {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Version {
    /// Sample an already held regular descriptor, never a path.
    pub(crate) fn of(meta: &Metadata) -> Self {
        Self {
            device: meta.dev(),
            inode: meta.ino(),
            size: meta.len(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
        }
    }
}

impl VerifiedSource {
    /// Expected SHA-256 of the exact stream that must reach the owner's private candidate.
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Length of the verified native version.
    pub fn len(&self) -> u64 {
        self.extent.map_or(self.version.size, |(_, length)| length)
    }

    /// Whether the verified file has no bytes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Hash this held regular descriptor and retain that same descriptor.
    /// No path is opened. The digest is the bytes just read, not a caller-supplied grant.
    pub fn retain_held(file: File, work: &mut Work<'_>) -> Result<Self, Error> {
        note_file(work)?;
        let meta = file.metadata().map_err(|_| Error::Io)?;
        if !meta.is_file() {
            return Err(Error::Invalid);
        }
        let version = Version::of(&meta);
        let mut sha = Sha256::new();
        let mut offset = 0u64;
        let mut buffer = [0u8; 8192];
        let _scratch = preallocate(work, 8192)?;
        loop {
            let read = file.read_at(&mut buffer, offset).map_err(|_| Error::Io)?;
            if read == 0 {
                break;
            }
            note_read(work, read as u64)?;
            sha.update(&buffer[..read]);
            offset = offset.checked_add(read as u64).ok_or(Error::TooLarge)?;
        }
        if offset != meta.len() {
            return Err(Error::Checksum);
        }
        let digest = ContentDigest::new(hex(&sha.finalize())).map_err(|_| Error::Invalid)?;
        let mut source = Self {
            file,
            digest,
            version,
            extent: None,
        };
        source.transfer(None, work)?;
        Ok(source)
    }

    /// Copy the held descriptor into an owner-controlled private writer, checking the exact stream.
    /// Refuses drift, budget exhaustion, cancellation, or deadline before permission to publish.
    /// Failure can leave partial private output; it never authorizes a destination effect.
    pub fn copy_to(&mut self, output: &mut dyn Write, work: &mut Work<'_>) -> Result<(), Error> {
        let expected = self.len();
        self.transfer(Some(output), work)?;
        if self.len() != expected {
            return Err(Error::Drift);
        }
        Ok(())
    }

    /// Sample the held descriptor again without opening a name or promoting a digest to authority.
    pub(crate) fn revalidate(&self, work: &mut Work<'_>) -> Result<(), Error> {
        note_file(work)?;
        if Version::of(&self.file.metadata().map_err(|_| Error::Io)?) != self.version {
            return Err(Error::Drift);
        }
        Ok(())
    }

    /// Read one bounded text/metadata unit with a same-account scratch pin, never a whole corpus.
    pub(crate) fn read_all(
        &self,
        work: &mut Work<'_>,
        ceiling: u64,
    ) -> Result<BoundedBytes, Error> {
        self.revalidate(work)?;
        let length = self.len();
        if length > ceiling || length > work.bounds.quotas.max_entry_expanded {
            return Err(Error::Quota);
        }
        let mut output = BoundedBytes::reserve(length, work)?;
        output
            .bytes
            .resize(usize::try_from(length).map_err(|_| Error::TooLarge)?, 0);
        read_exact_at(
            &self.file,
            self.extent.map_or(0, |(offset, _)| offset),
            &mut output.bytes,
            work,
        )?;
        self.revalidate(work)?;
        if crate::digest_bytes(&output.bytes)? != self.digest {
            return Err(Error::Checksum);
        }
        Ok(output)
    }

    /// Hash or copy under one version sample, and rewind the same descriptor afterwards.
    fn transfer(
        &mut self,
        mut output: Option<&mut dyn Write>,
        work: &mut Work<'_>,
    ) -> Result<(), Error> {
        note_file(work)?;
        let _scratch = preallocate(work, 8192)?;
        if Version::of(&self.file.metadata().map_err(|_| Error::Io)?) != self.version {
            return Err(Error::Drift);
        }
        if self.len() > work.bounds.quotas.max_entry_expanded {
            return Err(Error::Quota);
        }
        self.file
            .seek(SeekFrom::Start(self.extent.map_or(0, |(offset, _)| offset)))
            .map_err(|_| Error::Io)?;
        let mut sha = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 8192];
        loop {
            gate(work)?;
            if self.extent.is_some() && size == self.len() {
                break;
            }
            let want = self.len().saturating_sub(size).min(buffer.len() as u64) as usize;
            if want as u64 > work.bounds.budget.max_bytes.saturating_sub(work.read_bytes) {
                return Err(Error::Quota);
            }
            let n = self.file.read(&mut buffer[..want]).map_err(|_| Error::Io)?;
            if n == 0 {
                break;
            }
            size = size.checked_add(n as u64).ok_or(Error::Quota)?;
            if size > self.len() {
                return Err(Error::Drift);
            }
            note_read(work, n as u64)?;
            sha.update(&buffer[..n]);
            if let Some(writer) = output.as_mut() {
                charge_fresh(work, 0, n as u64)?;
                let mut at = 0;
                while at < n {
                    gate(work)?;
                    let written = writer.write(&buffer[at..n]).map_err(|_| Error::Io)?;
                    if written == 0 {
                        return Err(Error::Io);
                    }
                    note_written(work, written as u64)?;
                    at += written;
                }
            }
        }
        if size != self.len()
            || hex(&sha.finalize()) != self.digest.as_str()
            || Version::of(&self.file.metadata().map_err(|_| Error::Io)?) != self.version
        {
            return Err(Error::Checksum);
        }
        self.file
            .seek(SeekFrom::Start(self.extent.map_or(0, |(offset, _)| offset)))
            .map_err(|_| Error::Io)?;
        Ok(())
    }
}

/// Bind every ancestor, verify the native source, and retain that same descriptor for delegation.
pub(crate) fn open(
    root: &File,
    path: &Path,
    digest: &ContentDigest,
    work: &mut Work<'_>,
) -> Result<VerifiedSource, Error> {
    let file = native::file(root, path, false, work)?;
    let version = Version::of(&file.metadata().map_err(|_| Error::Io)?);
    let mut source = VerifiedSource {
        file,
        digest: digest.clone(),
        version,
        extent: None,
    };
    source.transfer(None, work)?;
    Ok(source)
}

/// Bind an inventoried region of an already held TAR, with no extraction or pathname reopen.
pub(crate) fn range(
    file: &File,
    offset: u64,
    length: u64,
    digest: &ContentDigest,
    work: &mut Work<'_>,
) -> Result<VerifiedSource, Error> {
    note_file(work)?;
    let meta = file.metadata().map_err(|_| Error::Io)?;
    if !meta.is_file() || offset.checked_add(length).ok_or(Error::Invalid)? > meta.len() {
        return Err(Error::Invalid);
    }
    charge_fresh(work, 128, 0)?;
    let mut source = VerifiedSource {
        file: file.try_clone().map_err(|_| Error::Io)?,
        digest: digest.clone(),
        version: Version::of(&meta),
        extent: Some((offset, length)),
    };
    source.transfer(None, work)?;
    Ok(source)
}

/// Bytes drop before their scratch pin, including early returns and unwinding.
pub(crate) struct BoundedBytes {
    pub(crate) bytes: Vec<u8>,
    _pin: crate::Scratch,
}

impl BoundedBytes {
    /// Reserve actual per-unit peak space before allocation on the existing account.
    pub(crate) fn reserve(length: u64, work: &Work<'_>) -> Result<Self, Error> {
        let pin = preallocate(work, length.checked_add(64).ok_or(Error::TooLarge)?)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(usize::try_from(length).map_err(|_| Error::TooLarge)?)
            .map_err(|_| Error::TooLarge)?;
        Ok(Self { bytes, _pin: pin })
    }
}

/// Pre-admit bounded positional IO and debit only bytes actually returned by the native file.
pub(crate) fn read_exact_at(
    file: &File,
    mut offset: u64,
    mut output: &mut [u8],
    work: &mut Work<'_>,
) -> Result<(), Error> {
    while !output.is_empty() {
        gate(work)?;
        let want = output.len().min(8192);
        if want as u64 > work.bounds.budget.max_bytes.saturating_sub(work.read_bytes) {
            return Err(Error::Quota);
        }
        let n = file
            .read_at(&mut output[..want], offset)
            .map_err(|_| Error::Io)?;
        if n == 0 {
            return Err(Error::Invalid);
        }
        note_read(work, n as u64)?;
        offset = offset.checked_add(n as u64).ok_or(Error::Invalid)?;
        output = &mut output[n..];
    }
    Ok(())
}
