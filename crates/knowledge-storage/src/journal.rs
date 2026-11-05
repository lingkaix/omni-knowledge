//! Visible bounded JSONL appends and verifiable final-record evidence; malformed bytes are retained.
use crate::{Error, native};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
};

/// Exact native byte extent of a synchronized journal record, not a permission or logical clock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
impl Proof {
    /// Exact writer-local byte position, not a global clock or authority.
    pub(crate) fn position(&self) -> u64 {
        self.offset
    }
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
    /// Optional metadata-only qualified source observation; old operation records remain readable.
    #[serde(default)]
    observation: Option<crate::tracking::Observation>,
    /// Optional actual qualified scan boundary; old records remain readable.
    #[serde(default)]
    checkpoint: Option<crate::tracking::Checkpoint>,
    /// Native cursor published after the corresponding complete inventory checkpoint; never a grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    event_cursor: Option<crate::tracking::EventCursor>,
}
/// Validated metadata-only replay variants; source frames carry their exact visible anchor.
pub(crate) enum ReplayItem {
    /// One actual source observation.
    Source(Box<crate::tracking::Observation>, Proof),
    /// One actual complete/partial scope boundary.
    Checkpoint(crate::tracking::Checkpoint),
    /// Actual native cursor following its durable relevant inventory transaction.
    EventCursor(crate::tracking::EventCursor),
}
/// Borrow actual typed source/scope metadata without introducing another controller.
#[derive(Clone, Copy)]
enum TrackingEvidence<'a> {
    /// Source evidence.
    Source(&'a crate::tracking::Observation),
    /// Scope completion evidence.
    Checkpoint(&'a crate::tracking::Checkpoint),
    /// Post-inventory native event cursor.
    EventCursor(&'a crate::tracking::EventCursor),
}
/// Next byte cursor plus an optional source record; operation records also advance the cursor.
type ReplayRecord = (u64, Option<ReplayItem>);
/// Encode the actual observed availability, not a successful empty-source substitute.
fn availability_name(a: knowledge_core::backend::Availability) -> &'static str {
    use knowledge_core::backend::{Availability, Unavailable};
    match a {
        Availability::Available => "available",
        Availability::Unavailable(u) => match u {
            Unavailable::Offline => "offline",
            Unavailable::Denied => "denied",
            Unavailable::Placeholder => "placeholder",
            Unavailable::Unknown => "unknown",
            Unavailable::Changed => "changed",
            Unavailable::Unsupported => "unsupported",
            Unavailable::Deleted => "deleted",
        },
    }
}
/// Held writer stream; the caller already owns the native single-writer lock.
pub(crate) struct Journal {
    /// File opened append-only for writes without following a link.
    file: File,
    /// Selected writer identity.
    writer: String,
    /// Finite caller memory-derived record ceiling, not a durable format quota.
    max: u64,
    /// Reuse bounded sequential buffering; random proof/tail I/O explicitly invalidates it.
    replay: Option<BufReader<File>>,
    /// Exact next logical replay offset, independent of buffered read-ahead.
    replay_next: u64,
    /// Actual sequence continuity through this exact sequential replay.
    replay_sequence: Option<u64>,
}
impl Journal {
    /// Open one visible writer stream; refuse malformed/truncated tails without modifying them.
    pub(crate) fn open(dir: &File, writer: &str, max: u64) -> Result<Self, Error> {
        let mut j = Self {
            file: native::append_file(dir, &format!("{writer}.jsonl"))?,
            writer: writer.to_owned(),
            max,
            replay: None,
            replay_next: 0,
            replay_sequence: None,
        };
        j.last_sequence()?;
        Ok(j)
    }
    /// Identify this actual visible stream incarnation by its bounded, validated first record.
    /// An empty stream has no reusable projection generation; cache state cannot create its history.
    pub(crate) fn projection_generation(&mut self) -> Result<Option<String>, Error> {
        let Some((end, _)) = self.next_tracking(0, self.max)? else {
            return Ok(None);
        };
        self.replay = None;
        self.replay_next = 0;
        self.replay_sequence = None;
        self.file.seek(SeekFrom::Start(0)).map_err(|_| Error::Io)?;
        let mut bytes = vec![0; usize::try_from(end).map_err(|_| Error::Budget)?];
        self.file
            .read_exact(&mut bytes)
            .map_err(|_| Error::Recovery)?;
        Ok(Some(format!("{:x}", Sha256::digest(&bytes))))
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
            replay: None,
            replay_next: 0,
            replay_sequence: None,
        })
    }
    /// Report a tail problem without discarding earlier verifiable operation evidence.
    pub(crate) fn issue(&mut self) -> Option<Error> {
        self.last_sequence().err()
    }
    /// Read only a bounded final line to derive the next writer-local sequence.
    /// The complete prefix is retained; no repair, rewrite or silent truncation occurs.
    fn last_sequence(&mut self) -> Result<u64, Error> {
        self.replay = None;
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
        self.append_with(operation, intent, candidate, phase, (None, None), || Ok(()))
    }
    /// Append an operation record with exact live executor checks before controlled reads and writes.
    pub(crate) fn append_checked(
        &mut self,
        operation: &str,
        intent: &str,
        candidate: Option<&str>,
        phase: &str,
        check: impl Fn() -> Result<(), Error>,
    ) -> Result<Proof, Error> {
        self.append_with(operation, intent, candidate, phase, (None, None), check)
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
        self.append_with(
            operation,
            intent,
            None,
            phase,
            (Some((path, digest)), None),
            || Ok(()),
        )
    }
    /// Append synchronized metadata before/after real effects and return the exact durable record extent.
    fn append_with(
        &mut self,
        operation: &str,
        intent: &str,
        candidate: Option<&str>,
        phase: &str,
        evidence: (Option<(&str, &str)>, Option<TrackingEvidence<'_>>),
        check: impl Fn() -> Result<(), Error>,
    ) -> Result<Proof, Error> {
        check()?;
        let (asset, tracking) = evidence;
        let observation = match tracking {
            Some(TrackingEvidence::Source(o)) => Some(o),
            _ => None,
        };
        let checkpoint = match tracking {
            Some(TrackingEvidence::Checkpoint(c)) => Some(c),
            _ => None,
        };
        let event_cursor = match tracking {
            Some(TrackingEvidence::EventCursor(c)) => Some(c),
            _ => None,
        };
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
        check()?;
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
            source_id: observation.map(|o| o.source_id.clone()),
            old_digest: None,
            new_digest: observation
                .and_then(|o| o.version.as_ref().map(|v| v.digest.as_str().to_owned()))
                .or_else(|| asset.map(|(_, d)| d.to_owned())),
            old_path: None,
            new_path: observation
                .map(|o| o.stat.path.as_str().to_owned())
                .or_else(|| asset.map(|(p, _)| p.to_owned())),
            availability: observation
                .map(|o| availability_name(o.availability))
                .unwrap_or("available")
                .into(),
            evidence_kind: "local_operation".into(),
            observed_at,
            intent_digest: intent.to_owned(),
            candidate: candidate.map(str::to_owned),
            outcome: phase.to_owned(),
            observation: observation.cloned(),
            checkpoint: checkpoint.cloned(),
            event_cursor: event_cursor.cloned(),
        };
        let mut bytes = serde_json::to_vec(&record).map_err(|_| Error::Io)?;
        if bytes.len() as u64 >= self.max {
            return Err(Error::Budget);
        }
        bytes.push(b'\n');
        let offset = self.file.metadata().map_err(|_| Error::Io)?.len();
        native::write_checked(&mut self.file, &bytes, &check)?;
        check()?;
        self.file.sync_all().map_err(|_| Error::Indeterminate)?;
        Ok(Proof {
            writer: self.writer.clone(),
            offset,
            length: bytes.len() as u64,
            digest: format!("{:x}", Sha256::digest(&bytes)),
        })
    }
    /// Append actual qualified source metadata without a body/annotation authority or inferred digest.
    pub(crate) fn append_tracking(
        &mut self,
        operation: &str,
        observation: &crate::tracking::Observation,
    ) -> Result<Proof, Error> {
        let mut size = crate::JsonSize {
            bytes: 0,
            limit: self.max / 4,
        };
        serde_json::to_writer(&mut size, observation).map_err(|_| Error::Budget)?;
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(observation).map_err(|_| Error::Budget)?)
        );
        self.append_with(
            operation,
            &digest,
            None,
            "observed",
            (None, Some(TrackingEvidence::Source(observation))),
            || Ok(()),
        )
    }
    /// Append a bounded native cursor only after its complete inventory transaction is synchronized.
    pub(crate) fn append_event_cursor(
        &mut self,
        c: &crate::tracking::EventCursor,
    ) -> Result<Proof, Error> {
        let mut size = crate::JsonSize {
            bytes: 0,
            limit: self.max / 4,
        };
        serde_json::to_writer(&mut size, c).map_err(|_| Error::Budget)?;
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(c).map_err(|_| Error::Budget)?)
        );
        self.append_with(
            &c.scan,
            &digest,
            None,
            "native_cursor",
            (None, Some(TrackingEvidence::EventCursor(c))),
            || Ok(()),
        )
    }
    /// Persist an actual qualified scope boundary only after corresponding native inventory effects are durable.
    pub(crate) fn append_checkpoint(
        &mut self,
        c: &crate::tracking::Checkpoint,
    ) -> Result<Proof, Error> {
        let mut size = crate::JsonSize {
            bytes: 0,
            limit: self.max / 4,
        };
        serde_json::to_writer(&mut size, c).map_err(|_| Error::Budget)?;
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(c).map_err(|_| Error::Budget)?)
        );
        self.append_with(
            &c.scan,
            &digest,
            None,
            if c.complete {
                "reconcile_complete"
            } else {
                "reconcile_partial"
            },
            (None, Some(TrackingEvidence::Checkpoint(c))),
            || Ok(()),
        )
    }
    /// Validate projected metadata against actual exact synchronized visible bytes.
    pub(crate) fn verify_tracking(
        &mut self,
        proof: &Proof,
        observation: &crate::tracking::Observation,
    ) -> Result<(), Error> {
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(observation).map_err(|_| Error::Budget)?)
        );
        self.verify(proof, &observation.scan, &digest, "observed")
    }
    /// Validate the bounded immediate predecessor of a resumed byte cursor, including EOF.
    /// Interior offsets, truncated predecessors and noncanonical stream sequences fail without repair.
    fn predecessor(&mut self, offset: u64, max: u64) -> Result<Option<u64>, Error> {
        if offset == 0 {
            return Ok(None);
        }
        let length = offset.min(max.checked_add(1).ok_or(Error::Budget)?);
        self.file
            .seek(SeekFrom::Start(offset - length))
            .map_err(|_| Error::Io)?;
        let mut bytes = vec![0; usize::try_from(length).map_err(|_| Error::Budget)?];
        self.file
            .read_exact(&mut bytes)
            .map_err(|_| Error::Recovery)?;
        if bytes.last() != Some(&b'\n') {
            return Err(Error::Recovery);
        }
        let end = bytes.len() - 1;
        let start = bytes[..end]
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |n| n + 1);
        if start == 0 && length != offset {
            return Err(Error::Budget);
        }
        let record: Record =
            serde_json::from_slice(&bytes[start..end]).map_err(|_| Error::Recovery)?;
        let sequence = record
            .sequence
            .parse::<u64>()
            .map_err(|_| Error::Recovery)?;
        if record.format_version != 1
            || record.writer_id != self.writer
            || sequence == 0
            || sequence.to_string() != record.sequence
            || (offset - length + start as u64 == 0 && sequence != 1)
        {
            return Err(Error::Recovery);
        }
        Ok(Some(sequence))
    }
    /// Read one exact bounded retained record; malformed bytes stop at their original offset.
    pub(crate) fn next_tracking(
        &mut self,
        offset: u64,
        max: u64,
    ) -> Result<Option<ReplayRecord>, Error> {
        let end = self.file.metadata().map_err(|_| Error::Io)?.len();
        if offset > end {
            return Err(Error::Recovery);
        }
        let restarting = self.replay.is_none() || self.replay_next != offset;
        if restarting {
            self.replay = None;
            self.replay_sequence = self.predecessor(offset, max)?;
        }
        if offset == end {
            return Ok(None);
        }
        let length = (end - offset).min(max);
        if restarting {
            self.file
                .seek(SeekFrom::Start(offset))
                .map_err(|_| Error::Io)?;
            self.replay = Some(BufReader::with_capacity(
                8192,
                self.file.try_clone().map_err(|_| Error::Io)?,
            ));
        }
        let mut bytes = Vec::new();
        self.replay
            .as_mut()
            .ok_or(Error::Recovery)?
            .take(length)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| Error::Io)?;
        if bytes.last() != Some(&b'\n') {
            self.replay = None;
            return Err(if length == end - offset {
                Error::Recovery
            } else {
                Error::Budget
            });
        }
        self.replay_next = offset
            .checked_add(bytes.len() as u64)
            .ok_or(Error::Recovery)?;
        let record: Record = serde_json::from_slice(&bytes).map_err(|_| Error::Recovery)?;
        if record.format_version != 1
            || record.writer_id != self.writer
            || record
                .sequence
                .parse::<u64>()
                .ok()
                .is_none_or(|n| n == 0 || n.to_string() != record.sequence)
        {
            return Err(Error::Recovery);
        }
        let sequence = record
            .sequence
            .parse::<u64>()
            .map_err(|_| Error::Recovery)?;
        if (offset == 0 && sequence != 1)
            || self
                .replay_sequence
                .is_some_and(|p| p.checked_add(1) != Some(sequence))
        {
            return Err(Error::Recovery);
        }
        self.replay_sequence = Some(sequence);
        let next = offset
            .checked_add(bytes.len() as u64)
            .ok_or(Error::Recovery)?;
        if usize::from(record.observation.is_some())
            + usize::from(record.checkpoint.is_some())
            + usize::from(record.event_cursor.is_some())
            > 1
        {
            return Err(Error::Recovery);
        }
        let item = if let Some(o) = record.observation {
            if o.format_version != 1
                || record.source_id.as_deref() != Some(&o.source_id)
                || record.new_path.as_deref() != Some(o.stat.path.as_str())
                || record.operation_id.as_deref() != Some(&o.scan)
                || record.outcome != "observed"
            {
                return Err(Error::Recovery);
            }
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&o).map_err(|_| Error::Budget)?)
            );
            if record.intent_digest != digest {
                return Err(Error::Recovery);
            }
            Some(ReplayItem::Source(
                Box::new(o),
                Proof {
                    writer: self.writer.clone(),
                    offset,
                    length: bytes.len() as u64,
                    digest: format!("{:x}", Sha256::digest(&bytes)),
                },
            ))
        } else if let Some(c) = record.checkpoint {
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&c).map_err(|_| Error::Budget)?)
            );
            if c.format_version != 1
                || record.intent_digest != digest
                || record.operation_id.as_deref() != Some(&c.scan)
                || record.outcome
                    != if c.complete {
                        "reconcile_complete"
                    } else {
                        "reconcile_partial"
                    }
            {
                return Err(Error::Recovery);
            }
            Some(ReplayItem::Checkpoint(c))
        } else if let Some(c) = record.event_cursor {
            let digest = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&c).map_err(|_| Error::Budget)?)
            );
            if record.intent_digest != digest
                || record.operation_id.as_deref() != Some(&c.scan)
                || record.outcome != "native_cursor"
                || c.event_id == 0
                || c.volume == [0; 16]
            {
                return Err(Error::Recovery);
            }
            Some(ReplayItem::EventCursor(c))
        } else {
            None
        };
        Ok(Some((next, item)))
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
        self.replay = None;
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
