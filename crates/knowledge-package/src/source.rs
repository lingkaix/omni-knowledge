//! Source capabilities retain the verified native descriptor through owner delegation.
use crate::{
    ContentDigest, Error, Work, charge_fresh, gate, hex, native, note_read, note_written,
    preallocate,
};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// A verified native file, not a mutable path presented as proof.
/// Owners copy into their own private candidate with [`Self::copy_to`] and publish only on success.
/// No path or writable descriptor is exposed. In-place changes are checked by version and stream digest.
pub struct VerifiedSource {
    file: File,
    digest: ContentDigest,
    version: Version,
}

/// Descriptor identity and observable content version on Unix.
#[derive(PartialEq, Eq)]
struct Version {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Version {
    /// Sample an already held regular descriptor, never a path.
    fn of(meta: &Metadata) -> Self {
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
        self.version.size
    }

    /// Whether the verified file has no bytes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copy the held descriptor into an owner-controlled private writer, checking the exact stream.
    /// Refuses drift, budget exhaustion, cancellation, or deadline before permission to publish.
    /// Failure can leave partial private output; it never authorizes a destination effect.
    pub fn copy_to(&mut self, output: &mut dyn Write, work: &mut Work<'_>) -> Result<(), Error> {
        let expected = self.version.size;
        self.transfer(Some(output), work)?;
        if self.version.size != expected {
            return Err(Error::Drift);
        }
        Ok(())
    }

    /// Hash or copy under one version sample, and rewind the same descriptor afterwards.
    fn transfer(
        &mut self,
        mut output: Option<&mut dyn Write>,
        work: &mut Work<'_>,
    ) -> Result<(), Error> {
        let _scratch = preallocate(work, 8192)?;
        if Version::of(&self.file.metadata().map_err(|_| Error::Io)?) != self.version {
            return Err(Error::Drift);
        }
        if self.version.size > work.bounds.quotas.max_entry_expanded {
            return Err(Error::Quota);
        }
        self.file.seek(SeekFrom::Start(0)).map_err(|_| Error::Io)?;
        let mut sha = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 8192];
        loop {
            gate(work)?;
            let n = self.file.read(&mut buffer).map_err(|_| Error::Io)?;
            if n == 0 {
                break;
            }
            size = size.checked_add(n as u64).ok_or(Error::Quota)?;
            if size > self.version.size {
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
        if size != self.version.size
            || hex(&sha.finalize()) != self.digest.as_str()
            || Version::of(&self.file.metadata().map_err(|_| Error::Io)?) != self.version
        {
            return Err(Error::Checksum);
        }
        self.file.seek(SeekFrom::Start(0)).map_err(|_| Error::Io)?;
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
    let file = native::file(root, path, false)?;
    let version = Version::of(&file.metadata().map_err(|_| Error::Io)?);
    let mut source = VerifiedSource {
        file,
        digest: digest.clone(),
        version,
    };
    source.transfer(None, work)?;
    Ok(source)
}
