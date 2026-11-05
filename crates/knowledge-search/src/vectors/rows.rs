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
    file: &File,
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
    file: &File,
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
    if file.metadata().map_err(io)?.len() != expected {
        return Err(invalid("component length drift"));
    }
    Ok(format!("{:x}", hash.finalize()))
}
