//! Length-delimited row metadata and fixed-window paid generation output.
use super::{RowMeta, VectorError, invalid, io, write_funded};
use knowledge_retrieval::Work;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Write, os::unix::fs::FileExt, time::Instant};

/// Caller inventory facts, not credit or publication authority. Counts must be exact;
/// byte bounds must cover the complete immutable component before any build IO.
#[derive(Clone, Debug)]
pub struct VectorBuildPlan {
    pub rows: u64,
    pub metadata_bytes: u64,
    pub max_row_bytes: u64,
}

impl VectorBuildPlan {
    /// Accumulate the existing escaped metadata bound before a producer is advanced.
    /// Candidate/count facts grant nothing. A streaming host supplies the longest actual locator.
    pub fn include(
        &mut self,
        row: super::VectorCandidate<'_>,
        count: u64,
    ) -> Result<(), VectorError> {
        if count == 0 {
            return Ok(());
        }
        let strings = (row.source_id.len() as u64)
            .checked_add(row.source_digest.len() as u64)
            .and_then(|n| n.checked_add(row.locator.len() as u64))
            .and_then(|n| n.checked_add(row.representation.digest.len() as u64))
            .and_then(|n| n.checked_add(row.representation.processor.len() as u64))
            .ok_or_else(|| invalid("row size overflow"))?;
        let bound = strings
            .checked_mul(6)
            .and_then(|n| n.checked_add(384))
            .ok_or_else(|| invalid("row size overflow"))?;
        let rows = self
            .rows
            .checked_add(count)
            .ok_or_else(|| invalid("row count overflow"))?;
        let metadata_bytes = bound
            .checked_add(4)
            .and_then(|n| n.checked_mul(count))
            .and_then(|n| self.metadata_bytes.checked_add(n))
            .ok_or_else(|| invalid("metadata size overflow"))?;
        self.rows = rows;
        self.metadata_bytes = metadata_bytes;
        self.max_row_bytes = self.max_row_bytes.max(bound);
        Ok(())
    }
    /// The SAME mature reader formula, bounded by actual row metadata facts and distinct-file K.
    /// This is quote DATA, not admission or model/source authority; opening still validates the
    /// selected real manifest and Work before payload IO. It allocates no corpus/vector buffers.
    pub fn query_requirement(
        &self,
        identity: &super::ModelIdentity,
        files: usize,
    ) -> Result<knowledge_core::jobs::Reservation, VectorError> {
        super::validate_identity(identity)?;
        if files == 0 || self.max_row_bytes == 0 || self.max_row_bytes > u32::MAX as u64 {
            return Err(invalid("invalid reader metadata plan"));
        }
        let files = files.min(usize::try_from(self.rows).map_err(invalid)?);
        Ok(knowledge_core::jobs::Reservation {
            memory_bytes: super::reader_need_parts(identity, self.max_row_bytes, files)?,
            temporary_bytes: 0,
        })
    }
    /// The SAME complete quote used by the existing vector writer, before buffers, IO or next().
    pub fn requirement(
        &self,
        identity: &super::ModelIdentity,
    ) -> Result<knowledge_core::jobs::Reservation, VectorError> {
        super::validate_identity(identity)?;
        if self.max_row_bytes == 0 || self.max_row_bytes > u32::MAX as u64 {
            return Err(invalid("invalid row metadata frame plan"));
        }
        let row_bytes = (identity.dimension as u64)
            .checked_mul(4)
            .ok_or_else(|| invalid("row length overflow"))?;
        let bytes = row_bytes
            .checked_mul(self.rows)
            .ok_or_else(|| invalid("segment size overflow"))?;
        let header = super::identity_bound(identity)?;
        let memory_bytes = self
            .max_row_bytes
            .checked_mul(10)
            .and_then(|n| n.checked_add(row_bytes.checked_mul(4)?))
            .and_then(|n| n.checked_add(header.checked_mul(8)?))
            .and_then(|n| n.checked_add(65536))
            .ok_or_else(|| invalid("complete working plan overflow"))?;
        let temporary_bytes = bytes
            .checked_add(self.metadata_bytes)
            .and_then(|n| n.checked_add(header))
            .and_then(|n| n.checked_add(65536))
            .ok_or_else(|| invalid("complete storage plan overflow"))?;
        Ok(knowledge_core::jobs::Reservation {
            memory_bytes,
            temporary_bytes,
        })
    }
}

/// Exact retained native FD or the ONE kernel's already admitted component handle.
pub(super) enum Component {
    Native(File),
    Directory(std::sync::Arc<dyn knowledge_retrieval::ByteHandle>),
}
impl Component {
    pub(super) fn read_exact_at(&self, output: &mut [u8], offset: u64) -> std::io::Result<()> {
        match self {
            Self::Native(file) => file.read_exact_at(output, offset),
            Self::Directory(handle) => {
                let start = usize::try_from(offset).map_err(std::io::Error::other)?;
                let end = start
                    .checked_add(output.len())
                    .ok_or_else(|| std::io::Error::other("component offset overflow"))?;
                let bytes = handle.read_bytes(start..end)?;
                if bytes.len() != output.len() {
                    return Err(std::io::Error::other("short admitted component"));
                }
                output.copy_from_slice(&bytes);
                Ok(())
            }
        }
    }
    pub(super) fn len(&self) -> std::io::Result<u64> {
        match self {
            Self::Native(file) => Ok(file.metadata()?.len()),
            Self::Directory(handle) => Ok(handle.len() as u64),
        }
    }
    pub(super) fn try_clone(&self) -> std::io::Result<Self> {
        match self {
            Self::Native(file) => Ok(Self::Native(file.try_clone()?)),
            Self::Directory(handle) => Ok(Self::Directory(handle.clone())),
        }
    }
}

/// Two bounded windows share the one actual Work, never an independent resource pool.
pub(super) struct Output {
    file: File,
    pending: [u8; 8192],
    used: usize,
    hash: Sha256,
    bytes: u64,
    maximum: u64,
}
impl Output {
    pub(super) fn new(file: File, maximum: u64) -> Self {
        Self {
            file,
            pending: [0; 8192],
            used: 0,
            hash: Sha256::new(),
            bytes: 0,
            maximum,
        }
    }
    pub(super) fn push(
        &mut self,
        mut bytes: &[u8],
        work: &mut Work,
        deadline: Option<Instant>,
    ) -> Result<(), VectorError> {
        let next = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("component length overflow"))?;
        if next > self.maximum {
            return Err(invalid("component exceeds complete prepaid plan"));
        }
        self.hash.update(bytes);
        self.bytes = next;
        while !bytes.is_empty() {
            let n = bytes.len().min(self.pending.len() - self.used);
            self.pending[self.used..self.used + n].copy_from_slice(&bytes[..n]);
            self.used += n;
            bytes = &bytes[n..];
            if self.used == self.pending.len() {
                self.flush(work, deadline)?;
            }
        }
        Ok(())
    }
    fn flush(&mut self, work: &mut Work, deadline: Option<Instant>) -> Result<(), VectorError> {
        write_funded(&mut self.file, &self.pending[..self.used], work, deadline)?;
        self.used = 0;
        Ok(())
    }
    pub(super) fn finish(
        mut self,
        work: &mut Work,
        deadline: Option<Instant>,
    ) -> Result<(File, String, u64), VectorError> {
        self.flush(work, deadline)?;
        self.file.sync_all().map_err(io)?;
        Ok((self.file, format!("{:x}", self.hash.finalize()), self.bytes))
    }
}

/// Check the bound BEFORE serializer buffer growth, including its replacement peak.
pub(super) struct JsonBuffer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl JsonBuffer {
    pub(super) fn new(maximum: u64) -> Result<Self, VectorError> {
        Ok(Self {
            bytes: Vec::new(),
            maximum: usize::try_from(maximum).map_err(invalid)?,
        })
    }
    pub(super) fn encode(&mut self, row: &RowMeta) -> Result<&[u8], VectorError> {
        self.bytes.clear();
        serde_json::to_writer(&mut *self, row).map_err(invalid)?;
        Ok(&self.bytes)
    }
}
impl Write for JsonBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("row metadata overflow"))?;
        if next > self.maximum {
            return Err(std::io::Error::other("row metadata exceeds prepaid window"));
        }
        if next > self.bytes.capacity() {
            self.bytes
                .try_reserve_exact(next - self.bytes.len())
                .map_err(std::io::Error::other)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Read one framed row through the SAME retained metadata FD. The caller checks
/// currentness before/after; a frame length cannot exceed the already paid window.
pub(super) fn read_row(
    file: &Component,
    offset: &mut u64,
    total: u64,
    maximum: u64,
) -> Result<RowMeta, VectorError> {
    let prefix_end = offset
        .checked_add(4)
        .ok_or_else(|| invalid("row offset overflow"))?;
    if prefix_end > total {
        return Err(invalid("truncated row prefix"));
    }
    let mut prefix = [0u8; 4];
    file.read_exact_at(&mut prefix, *offset).map_err(io)?;
    let len = u32::from_le_bytes(prefix) as u64;
    let end = prefix_end
        .checked_add(len)
        .ok_or_else(|| invalid("row offset overflow"))?;
    if len == 0 || len > maximum || end > total {
        return Err(invalid("row frame exceeds paid metadata bounds"));
    }
    let mut bytes = vec![0u8; usize::try_from(len).map_err(invalid)?];
    file.read_exact_at(&mut bytes, prefix_end).map_err(io)?;
    let row = serde_json::from_slice(&bytes).map_err(invalid)?;
    *offset = end;
    Ok(row)
}

pub(super) fn hash_file(
    file: &Component,
    expected: u64,
    check: impl Fn() -> Result<(), VectorError>,
) -> Result<String, VectorError> {
    let mut hash = Sha256::new();
    let mut position = 0u64;
    let mut bytes = [0u8; 8192];
    while position < expected {
        let take = (expected - position).min(bytes.len() as u64) as usize;
        check()?;
        file.read_exact_at(&mut bytes[..take], position)
            .map_err(io)?;
        check()?;
        hash.update(&bytes[..take]);
        position += take as u64;
    }
    if file.len().map_err(io)? != expected {
        return Err(invalid("component length drift"));
    }
    Ok(format!("{:x}", hash.finalize()))
}
