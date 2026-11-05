//! Cross-writer inventory reconciliation without a global writer clock or digest identity.
use super::*;

/// Bounded independent projections rebuilt from all visible streams under the native writer lane.
pub(super) struct Inventory {
    /// Writer-qualified append/offset namespaces; no cross-writer position comparison is valid.
    pub(super) projections: Vec<Projection>,
    /// Only this invocation's selected writer may receive new observations.
    current: usize,
    /// Actual visible evidence directory, not a projection-supplied pathname.
    journal_dir: File,
}
impl Inventory {
    /// Rebuild visible writer evidence with a complete metadata and replay work bound.
    /// Conflicting or malformed streams remain untouched and fail reconciliation honestly.
    pub(super) fn rebuild(
        root: (u64, u64),
        backend: &BackendIdentity,
        writer: &str,
        dir: &File,
        b: &Bounds,
    ) -> Result<Self, Error> {
        let fd = rustix::fs::openat(
            dir,
            ".",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| Error::Io)?;
        let mut projections = Vec::new();
        let mut bytes = 0u64;
        let mut metadata = 0u64;
        let mut current = None;
        for entry in rustix::fs::Dir::read_from(fd).map_err(|_| Error::Io)? {
            b.check()?;
            let entry = entry.map_err(|_| Error::Io)?;
            let name = entry.file_name().to_str().map_err(|_| Error::Recovery)?;
            let Some(peer) = name.strip_suffix(".jsonl") else {
                continue;
            };
            if !component(peer) {
                return Err(Error::Recovery);
            }
            // Size namespace serialization/copies and geometric Vec capacity before allocating them.
            // Record decode storage is sequential and must not impose a fixed writer-count ceiling.
            let mut size = JsonSize {
                bytes: 0,
                limit: b.control() / 16,
            };
            serde_json::to_writer(&mut size, &(root, backend, peer)).map_err(|_| Error::Budget)?;
            metadata = size
                .bytes
                .checked_mul(16)
                .and_then(|n| n.checked_add(4096 + 2 * std::mem::size_of::<Projection>() as u64))
                .and_then(|n| metadata.checked_add(n))
                .ok_or(Error::Budget)?;
            if metadata > b.memory / 8 {
                return Err(Error::Budget);
            }
            let projection = Projection::open(root, backend, peer)?;
            let mut stream = journal::Journal::read(dir, peer, b.control())?;
            let mut cursor = 0;
            loop {
                b.check()?;
                let Some((next, item)) = stream.next_tracking(cursor, b.control())? else {
                    break;
                };
                bytes = bytes.checked_add(next - cursor).ok_or(Error::Budget)?;
                if bytes > b.bytes {
                    return Err(Error::Budget);
                }
                if let Some(journal::ReplayItem::Source(observation, proof)) = item
                    && observation.root == root
                    && observation.stat.backend == *backend
                {
                    projection.save(
                        &Cached {
                            observation: *observation,
                            proof,
                        },
                        b,
                    )?;
                }
                cursor = next;
            }
            if peer == writer {
                current = Some(projections.len());
            }
            projections.push(projection);
        }
        Ok(Self {
            projections,
            current: current.ok_or(Error::Recovery)?,
            journal_dir: dir.try_clone().map_err(|_| Error::Io)?,
        })
    }
    /// Combine path evidence using actual qualified identity, never a cross-writer timestamp.
    pub(super) fn path(&self, stat: &StatSnapshot, b: &Bounds) -> Result<Option<Cached>, Error> {
        self.lookup(stat, b, false)
    }
    /// Combine reuse-protected native identity evidence; equal content is irrelevant.
    pub(super) fn identity(
        &self,
        stat: &StatSnapshot,
        b: &Bounds,
    ) -> Result<Option<Cached>, Error> {
        self.lookup(stat, b, true)
    }
    /// Refuse conflicting mappings and prevent stale writer evidence from resurrecting a tombstone.
    fn lookup(
        &self,
        stat: &StatSnapshot,
        b: &Bounds,
        identity: bool,
    ) -> Result<Option<Cached>, Error> {
        let mut found: Option<Cached> = None;
        for projection in &self.projections {
            b.check()?;
            let candidate = if identity {
                projection.identity(stat, b, &self.journal_dir)?
            } else {
                projection.path(stat.path.as_str(), b, &self.journal_dir)?
            };
            let Some(candidate) = candidate else {
                continue;
            };
            if let Some(prior) = &mut found {
                let prior_matches = identity_key(&prior.observation.stat)? == identity_key(stat)?;
                let candidate_matches =
                    identity_key(&candidate.observation.stat)? == identity_key(stat)?;
                if !identity && prior_matches != candidate_matches {
                    if candidate_matches {
                        *prior = candidate;
                    }
                    continue;
                }
                if prior.observation.source_id != candidate.observation.source_id {
                    // Different historical native objects at one path do not merge their asset IDs.
                    if !identity && !candidate_matches {
                        continue;
                    }
                    return Err(Error::Recovery);
                }
                let ambiguous = prior.observation.identity_ambiguous
                    || candidate.observation.identity_ambiguous;
                if candidate.observation.change == ChangeKind::Deleted
                    || (prior.observation.change != ChangeKind::Deleted
                        && candidate.observation.stat.path == stat.path)
                {
                    *prior = candidate;
                }
                prior.observation.identity_ambiguous = ambiguous;
            } else {
                found = Some(candidate);
            }
        }
        Ok(found)
    }
    /// Write only the current writer's disposable frames after its visible append is synchronized.
    pub(super) fn save(&self, value: &Cached, b: &Bounds) -> Result<(), Error> {
        self.projections[self.current].save(value, b)
    }
}
