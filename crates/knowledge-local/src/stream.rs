//! Bounded streaming metadata enumeration; output history does not accumulate in working memory.
use super::*;
impl<P: LocalReadPolicy> LocalMount<P> {
    /// Observe the actual current selected native root identity; this data grants no read permission.
    pub fn root_identity(&self) -> Result<(u64, u64), BackendError> {
        let root = self.root()?;
        match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Proceed(_) => {}
            Gate::Cancel => return Err(BackendError::Unavailable(Unavailable::Unknown)),
            Gate::Refuse(e) => return Err(e),
        }
        let (dev, ino, kind) = native::identity_of(&root).map_err(map_native)?;
        if kind != native::NodeKind::Directory {
            return Err(BackendError::Unavailable(Unavailable::Changed));
        }
        Ok((dev, ino))
    }
    /// Stream actual admitted metadata through a synchronous consumer within the SAME prepaid account.
    /// Only an empty-output Complete result proves relevant coverage; Partial/error never establishes absence.
    /// Each owned entry is consumed before the next; consumer-retained copies need their own admitted costs.
    /// Unlike the retained listing compatibility API, any child marker is a dynamically detected boundary.
    pub fn visit(
        &self,
        budget: Budget,
        holding: &knowledge_core::admission::Holding,
        mut consume: impl FnMut(ListedEntry) -> Result<(), BackendError>,
    ) -> Result<Enumeration, BackendError> {
        admitted_budget(holding, budget)?;
        let root = self.root()?;
        let started = Instant::now();
        let a = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)? {
            Gate::Proceed(a) => a,
            Gate::Cancel => {
                return Ok(partial(
                    vec![],
                    EnumerationStop::Cancelled,
                    progress(0, 0, started),
                ));
            }
            Gate::Refuse(e) => return Err(e),
        };
        if !budget_allows_start(budget) || budget.max_memory_bytes < FRAME_RESERVE {
            return Ok(partial(
                vec![],
                EnumerationStop::BudgetExhausted,
                progress(0, 0, started),
            ));
        }
        let mut stack = vec![Frame {
            iter: native::read_dir(&root).map_err(map_native)?,
            prefix: String::new(),
            dir: duplicate_dir(&root)?,
        }];
        let mut examined = 0u64;
        let mut bytes_read = 0u64;
        let stop = |reason, examined, bytes_read| {
            partial(Vec::new(), reason, progress(examined, bytes_read, started))
        };
        loop {
            if expired(started, budget.max_millis) {
                return Ok(stop(EnumerationStop::BudgetExhausted, examined, bytes_read));
            }
            if live(self, &a)? != Live::Current {
                return Ok(stop(EnumerationStop::Cancelled, examined, bytes_read));
            }
            match next_name(&mut stack) {
                FrameStep::Done => {
                    return deliver_list(
                        self,
                        Enumeration::Complete {
                            entries: vec![],
                            progress: progress(examined, bytes_read, started),
                        },
                        &a,
                    );
                }
                FrameStep::Error(e) => {
                    return stop_or_err(vec![], examined, bytes_read, started, e);
                }
                FrameStep::Name { name } => {
                    if examined >= budget.max_files {
                        return Ok(stop(EnumerationStop::BudgetExhausted, examined, bytes_read));
                    }
                    examined += 1;
                    bytes_read = bytes_read
                        .checked_add(name.to_bytes().len() as u64)
                        .ok_or(BackendError::BudgetExhausted)?;
                    if bytes_read > budget.max_bytes {
                        return Ok(stop(EnumerationStop::BudgetExhausted, examined, bytes_read));
                    }
                    let text = match name.to_str() {
                        Ok(t) => t,
                        Err(_) => {
                            return Ok(stop(EnumerationStop::Unsupported, examined, bytes_read));
                        }
                    };
                    let frame = stack.last().ok_or(BackendError::Io)?;
                    let memory = stack
                        .iter()
                        .try_fold(0u64, |n, f| {
                            n.checked_add(FRAME_RESERVE)?
                                .checked_add(f.prefix.len() as u64)
                        })
                        .ok_or(BackendError::BudgetExhausted)?;
                    let len =
                        child_path_len(&frame.prefix, text).ok_or(BackendError::BudgetExhausted)?;
                    if len
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(ENTRY_RESERVE + FRAME_RESERVE))
                        .and_then(|n| n.checked_add(memory))
                        .is_none_or(|n| n > budget.max_memory_bytes)
                    {
                        return Ok(stop(EnumerationStop::BudgetExhausted, examined, bytes_read));
                    }
                    let path = RelativePath::new(child_path(&frame.prefix, text))
                        .map_err(|_| BackendError::Unavailable(Unavailable::Unsupported))?;
                    if outside(path.as_str(), &a.outside) {
                        continue;
                    }
                    let observed = match native::observe(&frame.dir, &name) {
                        Ok(o) => o,
                        Err(e) => return stop_or_err(vec![], examined, bytes_read, started, e),
                    };
                    let kind = match observed.kind {
                        native::NodeKind::File => EntryKind::File,
                        native::NodeKind::Directory => EntryKind::Directory,
                        native::NodeKind::Unsupported => {
                            return Ok(stop(EnumerationStop::Unsupported, examined, bytes_read));
                        }
                    };
                    let charge = list_entry_charge(
                        path.as_str(),
                        &observed,
                        &self.library_id,
                        &self.mount_id,
                    )
                    .ok_or(BackendError::BudgetExhausted)?;
                    if memory
                        .checked_add(charge)
                        .and_then(|n| n.checked_add(len + FRAME_RESERVE))
                        .is_none_or(|n| n > budget.max_memory_bytes)
                    {
                        return Ok(stop(EnumerationStop::BudgetExhausted, examined, bytes_read));
                    }
                    let child = if kind == EntryKind::Directory {
                        let child = native::open_directory(&frame.dir, text).map_err(map_native)?;
                        match native::observe(&child, c"omni-knowledge.json") {
                            Ok(_) => continue,
                            Err(native::NativeError::Os(e)) if e == rustix::io::Errno::NOENT => {}
                            Err(e) => return stop_or_err(vec![], examined, bytes_read, started, e),
                        }
                        let (dev, ino, k) = native::identity_of(&child).map_err(map_native)?;
                        if dev != observed.device
                            || ino != observed.inode
                            || k != native::NodeKind::Directory
                        {
                            return Ok(stop(EnumerationStop::Changed, examined, bytes_read));
                        }
                        let iter = native::read_dir(&child).map_err(map_native)?;
                        Some(Frame {
                            iter,
                            prefix: path.as_str().to_owned(),
                            dir: child,
                        })
                    } else {
                        None
                    };
                    let stat = observation_from(self, &path, &observed);
                    if live(self, &a)? != Live::Current {
                        return Ok(stop(EnumerationStop::Cancelled, examined, bytes_read));
                    }
                    consume(ListedEntry { path, kind, stat })?;
                    if let Some(child) = child {
                        stack.push(child)
                    }
                }
            }
        }
    }
}
