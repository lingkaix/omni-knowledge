//! Owned finite-lease Apple runtime; no framework callback ever borrows parent Rust memory.
use super::*;
use protocol::{FRAME_BYTES, Frame, GAP, READY};
use std::{
    io::Read,
    process::{Child, ChildStdout, Command, Stdio},
    time::Duration,
};

/// One actual helper, its bounded transport and its independent SAME-account resource holding.
pub(super) struct Lease {
    /// Exact child retained until wait confirms exit, never just EOF or a stop request.
    child: Option<Child>,
    /// Exact bounded nonblocking transport; no unbounded reader thread.
    output: Option<ChildStdout>,
    /// Retained through confirmed exit, transport close, and metadata destruction.
    holding: Option<Holding>,
    /// Current owner metadata remains prepaid until it is actually destroyed.
    admission: Admission,
    /// Packaged helper metadata remains prepaid through constructor failure and teardown.
    helper: PathBuf,
    /// Original selected held root facts.
    root: (u64, u64),
    /// Complete observed startup identity and latest native ID.
    frame: Option<Frame>,
    /// Partial fixed-width transport input, never path-sized callback data.
    bytes: [u8; FRAME_BYTES],
    /// Actual partial frame length.
    used: usize,
    /// Persistent continuity loss cannot be acknowledged away.
    lost: bool,
    /// Finite per-call work and native-runtime memory ceiling.
    budget: Budget,
    /// Finite runtime expiry, including when the caller stops polling.
    deadline: Instant,
    /// An unknown stop must not be retried as an apparent settlement.
    unknown: bool,
}
impl<P: LocalReadPolicy> LocalMount<P> {
    /// Start the selected packaged helper after reserving its complete declared runtime plan.
    /// `helper` must be the host's trusted absolute executable, not a source-controlled PATH lookup.
    /// Missing helpers, current admission changes and native setup faults are explicit failures.
    pub fn watch_with_helper(
        &self,
        budget: Budget,
        helper: &Path,
    ) -> Result<HintWatch<'_, P>, WatchError> {
        self.mac_watch(budget, Some(helper))
    }
    /// Resolve and construct the native runtime only after complete SAME-account prepayment.
    pub(super) fn mac_watch(
        &self,
        budget: Budget,
        helper: Option<&Path>,
    ) -> Result<HintWatch<'_, P>, WatchError> {
        if self.kind == BackendKind::Nas {
            return Err(WatchError::Unsupported);
        }
        if helper.is_some_and(|p| !p.is_absolute())
            || budget.max_memory_bytes < 8 * 1024 * 1024
            || !budget_allows_start(budget)
        {
            return Err(WatchError::Budget);
        }
        let holding = match Account::current()
            .map_err(WatchError::Admission)?
            .reserve(
                Reservation {
                    memory_bytes: budget.max_memory_bytes,
                    temporary_bytes: 0,
                },
                Origin::Startup,
            )
            .map_err(WatchError::Admission)?
        {
            Decision::Ready(h) => h,
            Decision::Waiting(_) => return Err(WatchError::WaitingMemory),
        };
        let mut pending = Pending(Some(holding));
        let admission = match gate(&self.policy, &self.library_id, &self.mount_id, self.kind)
            .map_err(WatchError::Backend)?
        {
            Gate::Proceed(a) => a,
            Gate::Cancel => {
                return Err(WatchError::Backend(BackendError::Unavailable(
                    Unavailable::Unknown,
                )));
            }
            Gate::Refuse(e) => return Err(WatchError::Backend(e)),
        };
        let fixed_metadata = (admission.outside.capacity() as u64)
            .checked_mul(std::mem::size_of::<RelativePath>() as u64)
            .and_then(|n| n.checked_add(self.path.as_os_str().len() as u64))
            .ok_or(WatchError::Budget)?;
        let metadata = admission
            .outside
            .iter()
            .try_fold(fixed_metadata, |n, p| {
                n.checked_add(p.as_str().len() as u64)
            })
            .ok_or(WatchError::Budget)?;
        if metadata
            .checked_mul(4)
            .is_none_or(|n| n > budget.max_memory_bytes / 8)
        {
            return Err(WatchError::Budget);
        }
        let root = self.root().map_err(WatchError::Backend)?;
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(budget.max_millis))
            .ok_or(WatchError::Budget)?;
        // Parent policy, argv, pipe/kernel and fixed transport costs are not charged again to child RSS.
        let parent_bytes = metadata
            .checked_mul(4)
            .and_then(|n| n.checked_add(512 * 1024))
            .ok_or(WatchError::Budget)?;
        let runtime_budget = Budget {
            max_memory_bytes: budget
                .max_memory_bytes
                .checked_sub(parent_bytes)
                .ok_or(WatchError::Budget)?,
            ..budget
        };
        let mut lease = Lease {
            child: None,
            output: None,
            helper: resolve_helper(helper)?,
            admission,
            holding: pending.0.take(),
            root: (self.device, self.inode),
            frame: None,
            bytes: [0; FRAME_BYTES],
            used: 0,
            lost: false,
            budget: runtime_budget,
            deadline,
            unknown: false,
        };
        let mut child = Command::new(&lease.helper)
            .arg(runtime_budget.max_memory_bytes.to_string())
            .arg(budget.max_millis.to_string())
            .arg(budget.max_files.to_string())
            .args(lease.admission.outside.iter().map(|p| p.as_str()))
            .stdin(Stdio::from(root))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir("/")
            .spawn()
            .map_err(|_| WatchError::Native)?;
        lease.output = child.stdout.take();
        lease.child = Some(child);
        let output = lease.output.as_ref().ok_or(WatchError::Native)?;
        let flags = rustix::fs::fcntl_getfl(output).map_err(|_| WatchError::Native)?;
        rustix::fs::fcntl_setfl(output, flags | rustix::fs::OFlags::NONBLOCK)
            .map_err(|_| WatchError::Native)?;
        while lease.frame.is_none() {
            lease.poll()?;
            if live(self, &lease.admission).map_err(WatchError::Backend)? != Live::Current {
                return Err(WatchError::Backend(BackendError::Unavailable(
                    Unavailable::Changed,
                )));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(HintWatch {
            mount: self,
            mac: lease,
            epoch: 0,
            dirty: true,
            complete: true,
            last: Instant::now(),
        })
    }
}
/// Locate only a sibling packaged executable (or Cargo's sibling debug/release binary), never PATH.
fn resolve_helper(selected: Option<&Path>) -> Result<PathBuf, WatchError> {
    if let Some(path) = selected {
        return Ok(path.to_owned());
    }
    let executable = std::env::current_exe().map_err(|_| WatchError::Native)?;
    let mut directory = executable.parent().ok_or(WatchError::Native)?;
    if directory.file_name().is_some_and(|n| n == "deps") {
        directory = directory.parent().ok_or(WatchError::Native)?;
    }
    Ok(directory.join("omni-fsevents"))
}
impl Lease {
    /// Borrow exact current captured owner metadata, not a cursor-derived grant.
    pub(super) fn admission(&self) -> &Admission {
        &self.admission
    }
    /// Read bounded frames and observe the exact live process; malformed or missing state is not quiet.
    pub(super) fn poll(&mut self) -> Result<bool, WatchError> {
        let result = self.poll_inner();
        if result.is_err() {
            self.lost = true;
        }
        result
    }
    /// Internal transport attempt; every refusal is latched by the public native-owner poll.
    fn poll_inner(&mut self) -> Result<bool, WatchError> {
        if self.unknown {
            return Err(WatchError::QuiescenceUnknown);
        }
        let child = self.child.as_mut().ok_or(WatchError::Native)?;
        if let Some(status) = child.try_wait().map_err(|_| WatchError::Native)? {
            return Err(match status.code() {
                Some(0) => WatchError::LeaseExpired,
                Some(65 | 69) => WatchError::Budget,
                _ => WatchError::Native,
            });
        }
        if Instant::now() >= self.deadline {
            return Err(WatchError::LeaseExpired);
        }
        match knowledge_processing::worker::resident_bytes(child.id()) {
            Ok(n) if n <= self.budget.max_memory_bytes => (),
            _ => {
                self.lost = true;
                self.stop();
                return Err(WatchError::Budget);
            }
        }
        let start = Instant::now();
        let mut changed = false;
        for _ in 0..self.budget.max_files {
            if expired(start, self.budget.max_millis) {
                self.lost = true;
                break;
            }
            let n = match self
                .output
                .as_mut()
                .ok_or(WatchError::Native)?
                .read(&mut self.bytes[self.used..])
            {
                Ok(0) => {
                    self.lost = true;
                    return Err(WatchError::Native);
                }
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(changed),
                Err(_) => {
                    self.lost = true;
                    return Err(WatchError::Native);
                }
            };
            self.used += n;
            if self.used == FRAME_BYTES {
                let f = Frame::decode(&self.bytes).ok_or(WatchError::Native)?;
                self.used = 0;
                if (f.device, f.root) != self.root || f.volume == [0; 16] {
                    self.lost = true;
                    return Err(WatchError::Native);
                }
                if let Some(old) = self.frame {
                    if old.volume != f.volume || f.event_id <= old.event_id || f.flags & READY != 0
                    {
                        self.lost = true;
                    }
                    changed = true;
                } else if f.flags & READY == 0 {
                    self.lost = true;
                    return Err(WatchError::Native);
                }
                self.lost |= f.flags & GAP != 0;
                self.frame = Some(f);
            }
        }
        self.lost = true;
        Ok(changed)
    }
    /// Return only observed nonzero native IDs; a gap cannot become a usable checkpoint candidate.
    pub(super) fn cursor(&self) -> Option<NativeCursor> {
        self.frame
            .filter(|f| !self.lost && f.event_id != 0)
            .map(|f| NativeCursor {
                root: self.root,
                volume: f.volume,
                event_id: f.event_id,
            })
    }
    /// Persistent loss requires a new stream and real bounded reconciliation, never an acknowledgement.
    pub(super) fn gap(&self) -> bool {
        self.lost
    }
    /// Request stop, then wait a finite bound for actual exit; retain exact resources if unknown.
    pub(super) fn stop(&mut self) -> bool {
        if self.unknown {
            return false;
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let start = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    _ if start.elapsed() >= Duration::from_millis(250) => {
                        self.unknown = true;
                        std::mem::forget(self.child.take());
                        std::mem::forget(self.output.take());
                        std::mem::forget(self.holding.take());
                        return false;
                    }
                    _ => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        }
        drop(self.child.take());
        drop(self.output.take());
        self.frame = None;
        self.bytes.fill(0);
        true
    }
}
impl Drop for Lease {
    /// No Rust callback context crosses this boundary; only confirmed process exit releases costs.
    fn drop(&mut self) {
        if self.stop() {
            self.admission.outside = Vec::new();
            self.helper = PathBuf::new();
            if let Some(mut h) = self.holding.take() {
                let _ = h.finish();
            }
        }
    }
}
