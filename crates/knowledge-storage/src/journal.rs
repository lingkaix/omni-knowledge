//! Visible bounded JSONL appends and verifiable final-record evidence; malformed bytes are retained.
use crate::{Error, native};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};

/// Exact native byte extent of a synchronized journal record, not a permission or logical clock.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Proof {
    /// Actual stream identity, retained across recovery by a different current writer.
    pub(crate) writer: String,
    /// Byte offset in this writer's stream.
    offset: u64,
    /// Full encoded length including newline.
    length: u64,
    /// SHA-256 of exactly those bytes.
    digest: String,
}
/// Versioned metadata-only writer observation; bodies and raw request arguments are never stored here.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    /// Explicit native journal encoding version.
    format_version: u32,
    /// Stable single-component stream writer identity.
    writer_id: String,
    /// Canonical writer-local decimal sequence, without JavaScript number loss.
    sequence: String,
    /// Stable writer/sequence observation identity, not authentication.
    observation_id: String,
    /// Operation data identity, null for observations without one.
    operation_id: Option<String>,
    /// Asset identity when known; page/aggregate operation records use null.
    source_id: Option<String>,
    /// Verified old digest when this record selects one source, otherwise null.
    old_digest: Option<String>,
    /// Verified new digest when this record selects one source, otherwise null.
    new_digest: Option<String>,
    /// Old selected relative path when relevant.
    old_path: Option<String>,
    /// New selected relative path when relevant.
    new_path: Option<String>,
    /// Actual observation classification, not a filesystem existence inference.
    availability: String,
    /// Kind of operation evidence.
    evidence_kind: String,
    /// UTC RFC3339 observation time; never ordering authority.
    observed_at: String,
    /// Normalized metadata intent digest.
    intent_digest: String,
    /// Actual prepared Git commit when present, not cross-filesystem saved proof.
    candidate: Option<String>,
    /// Current typed local phase.
    outcome: String,
}
/// Held writer stream; the caller already owns the native single-writer lock.
pub(crate) struct Journal {
    /// File opened append-only for writes without following a link.
    file: File,
    /// Selected writer identity.
    writer: String,
    /// Finite caller memory-derived record ceiling, not a durable format quota.
    max: u64,
}
impl Journal {
    /// Open one visible writer stream; refuse malformed/truncated tails without modifying them.
    pub(crate) fn open(dir: &File, writer: &str, max: u64) -> Result<Self, Error> {
        let mut j = Self {
            file: native::append_file(dir, &format!("{writer}.jsonl"))?,
            writer: writer.to_owned(),
            max,
        };
        j.last_sequence()?;
        Ok(j)
    }
    /// Open retained evidence read-only, never creating a stream or modifying a damaged tail.
    pub(crate) fn read(dir: &File, writer: &str, max: u64) -> Result<Self, Error> {
        if !crate::component(writer) {
            return Err(Error::Recovery);
        }
        Ok(Self {
            file: native::regular(dir, &format!("{writer}.jsonl"))?.ok_or(Error::Recovery)?,
            writer: writer.to_owned(),
            max,
        })
    }
    /// Report a tail problem without discarding earlier verifiable operation evidence.
    pub(crate) fn issue(&mut self) -> Option<Error> {
        self.last_sequence().err()
    }
    /// Read only a bounded final line to derive the next writer-local sequence.
    /// The complete prefix is retained; no repair, rewrite or silent truncation occurs.
    fn last_sequence(&mut self) -> Result<u64, Error> {
        let len = self.file.metadata().map_err(|_| Error::Io)?.len();
        if len == 0 {
            return Ok(0);
        }
        let size = len.min(self.max.checked_add(1).ok_or(Error::Budget)?);
        let start = len - size;
        self.file
            .seek(SeekFrom::Start(start))
            .map_err(|_| Error::Io)?;
        let mut bytes = vec![0u8; usize::try_from(size).map_err(|_| Error::Budget)?];
        self.file
            .read_exact(&mut bytes)
            .map_err(|_| Error::Recovery)?;
        if bytes.last() != Some(&b'\n') {
            return Err(Error::Recovery);
        }
        let end = bytes.len() - 1;
        let from = bytes[..end]
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |n| n + 1);
        if from == 0 && start != 0 {
            return Err(Error::Budget);
        }
        let record: Record =
            serde_json::from_slice(&bytes[from..end]).map_err(|_| Error::Recovery)?;
        if record.format_version != 1 || record.writer_id != self.writer {
            return Err(Error::Recovery);
        }
        let n = record
            .sequence
            .parse::<u64>()
            .map_err(|_| Error::Recovery)?;
        if n == 0 || n.to_string() != record.sequence {
            return Err(Error::Recovery);
        }
        Ok(n)
    }
    /// Append aggregate operation metadata; no bodies, raw arguments or invented source digests.
    pub(crate) fn append(
        &mut self,
        operation: &str,
        intent: &str,
        candidate: Option<&str>,
        phase: &str,
    ) -> Result<Proof, Error> {
        self.append_with(operation, intent, candidate, phase, None)
    }
    /// Record an actually verified new file observation without assigning a hash-derived source identity.
    pub(crate) fn append_observed(
        &mut self,
        operation: &str,
        intent: &str,
        path: &str,
        digest: &str,
        phase: &str,
    ) -> Result<Proof, Error> {
        self.append_with(operation, intent, None, phase, Some((path, digest)))
    }
    /// Append synchronized metadata before/after real effects and return the exact durable record extent.
    fn append_with(
        &mut self,
        operation: &str,
        intent: &str,
        candidate: Option<&str>,
        phase: &str,
        asset: Option<(&str, &str)>,
    ) -> Result<Proof, Error> {
        let needed = [
            self.writer.len(),
            operation.len(),
            intent.len(),
            candidate.map_or(0, str::len),
            phase.len(),
            asset.map_or(0, |(p, _)| p.len()),
            asset.map_or(0, |(_, d)| d.len()),
        ]
        .into_iter()
        .try_fold(2048u64, |n, len| {
            n.checked_add((len as u64).checked_mul(6)?)
        })
        .ok_or(Error::Budget)?;
        if needed > self.max {
            return Err(Error::Budget);
        }
        let seq = self.last_sequence()?.checked_add(1).ok_or(Error::Budget)?;
        let observed_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| Error::Io)?;
        let record = Record {
            format_version: 1,
            writer_id: self.writer.clone(),
            sequence: seq.to_string(),
            observation_id: format!(
                "{:x}",
                Sha256::digest(format!("{}:{}:{seq}", self.writer.len(), self.writer).as_bytes())
            ),
            operation_id: Some(operation.to_owned()),
            source_id: None,
            old_digest: None,
            new_digest: asset.map(|(_, d)| d.to_owned()),
            old_path: None,
            new_path: asset.map(|(p, _)| p.to_owned()),
            availability: "available".into(),
            evidence_kind: "local_operation".into(),
            observed_at,
            intent_digest: intent.to_owned(),
            candidate: candidate.map(str::to_owned),
            outcome: phase.to_owned(),
        };
        let mut bytes = serde_json::to_vec(&record).map_err(|_| Error::Io)?;
        if bytes.len() as u64 >= self.max {
            return Err(Error::Budget);
        }
        bytes.push(b'\n');
        let offset = self.file.metadata().map_err(|_| Error::Io)?.len();
        self.file
            .write_all(&bytes)
            .map_err(|_| Error::Indeterminate)?;
        self.file.sync_all().map_err(|_| Error::Indeterminate)?;
        Ok(Proof {
            writer: self.writer.clone(),
            offset,
            length: bytes.len() as u64,
            digest: format!("{:x}", Sha256::digest(&bytes)),
        })
    }
    /// Verify synchronized aggregate evidence and report a damaged current tail without rewriting it.
    pub(crate) fn verify(
        &mut self,
        proof: &Proof,
        operation: &str,
        intent: &str,
        phase: &str,
    ) -> Result<(), Error> {
        self.verify_with(proof, operation, intent, phase, None)
    }
    /// Bind a verified file result to its ACTUAL visible path/digest observation, not a mutable sidecar alone.
    pub(crate) fn verify_observed(
        &mut self,
        proof: &Proof,
        operation: &str,
        intent: &str,
        path: &str,
        digest: &str,
        phase: &str,
    ) -> Result<(), Error> {
        self.verify_with(proof, operation, intent, phase, Some((path, digest)))
    }
    /// Verify a saved marker against actual retained stream bytes; marker declarations alone prove nothing.
    fn verify_with(
        &mut self,
        proof: &Proof,
        operation: &str,
        intent: &str,
        phase: &str,
        asset: Option<(&str, &str)>,
    ) -> Result<(), Error> {
        if proof.writer != self.writer {
            return Err(Error::Recovery);
        }
        if proof.length > self.max {
            return Err(Error::Budget);
        }
        let end = proof
            .offset
            .checked_add(proof.length)
            .ok_or(Error::Recovery)?;
        if end > self.file.metadata().map_err(|_| Error::Io)?.len() {
            return Err(Error::Recovery);
        }
        self.file
            .seek(SeekFrom::Start(proof.offset))
            .map_err(|_| Error::Io)?;
        let mut bytes = vec![0u8; usize::try_from(proof.length).map_err(|_| Error::Budget)?];
        self.file
            .read_exact(&mut bytes)
            .map_err(|_| Error::Recovery)?;
        if format!("{:x}", Sha256::digest(&bytes)) != proof.digest {
            return Err(Error::Recovery);
        }
        let r: Record = serde_json::from_slice(&bytes).map_err(|_| Error::Recovery)?;
        if r.format_version != 1
            || r.writer_id != self.writer
            || r.operation_id.as_deref() != Some(operation)
            || r.intent_digest != intent
            || r.outcome != phase
            || asset.is_some_and(|(p, d)| {
                r.new_path.as_deref() != Some(p) || r.new_digest.as_deref() != Some(d)
            })
        {
            return Err(Error::Recovery);
        }
        self.last_sequence()?;
        Ok(())
    }
}
