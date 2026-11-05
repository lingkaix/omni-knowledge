//! Caller-driven native hints: no worker/thread, replay claim, body probe or event-derived absence.
use super::*;
use knowledge_core::admission::Holding;
#[cfg(target_os = "linux")]
use knowledge_core::{
    admission::{Account, Decision, Origin},
    jobs::Reservation,
};
#[cfg(target_os = "linux")]
use rustix::{
    fd::{AsRawFd, OwnedFd},
    fs::inotify,
};
#[cfg(target_os = "linux")]
use std::mem::MaybeUninit;

/// Native watch setup/poll refusal; capacity never becomes an empty successful watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchError {
    /// The existing process account refused the complete planned costs.
    Admission(knowledge_core::admission::Error),
    /// Contention, without a retry loop or partial resource promise.
    WaitingMemory,
    /// Caller work/scratch cannot establish the selected watch coverage.
    Budget,
    /// Current selected local admission/root observation refused the call.
    Backend(BackendError),
    /// No implemented native event binding on this target; reconciliation remains available.
    Unsupported,
    /// Native watch/instance/descriptor capacity is exhausted; show the limit and reconcile instead.
    Exhausted,
    /// Native descriptor/watch creation/read failed; reconcile rather than trusting continuity.
    Native,
}
impl std::fmt::Display for WatchError {
    /// Format fixed errors only, never rejected path/content.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "local watch {self:?}")
    }
}
impl std::error::Error for WatchError {}
/// Current hints, never source-version evidence or a durable filesystem replay cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HintState {
    /// In-process monotonic hint generation; restart always invalidates continuity.
    pub epoch: u64,
    /// Startup/changed coverage/overflow/unacknowledged events require a real bounded scan.
    pub reconciliation_required: bool,
    /// Complete initial relevant directory coverage was actually established within budget.
    pub coverage_complete: bool,
    /// Actual current interval since the latest observed hint.
    pub quiet_millis: u64,
}
/// Exact synchronous native FD/kernel-watch lifetime; no detached native worker is presumed stopped.
pub struct HintWatch<'a, P: LocalReadPolicy> {
    /// Root owner remains live and is checked at every poll/acknowledgement.
    mount: &'a LocalMount<P>,
    /// Actual captured current owner revision/exclusions.
    admission: Admission,
    /// Closed BEFORE releasing the same-account watch allocation.
    #[cfg(target_os = "linux")]
    fd: Option<OwnedFd>,
    /// Complete native watch/metadata/buffer plan, independent of execution mount slots.
    holding: Holding,
    /// Original finite polling/work maxima.
    #[cfg(target_os = "linux")]
    budget: Budget,
    /// Actual in-process generation.
    epoch: u64,
    /// A durable inventory acknowledgement is still needed.
    dirty: bool,
    /// Coverage loss is never patched by a false quiet interval.
    complete: bool,
    /// Actual last observed activity.
    last: Instant,
}
/// Synchronous constructor cleanup: no async/native worker can outlive these temporary buffers.
#[cfg(target_os = "linux")]
struct Pending(Option<Holding>);
#[cfg(target_os = "linux")]
impl Drop for Pending {
    /// Release only after later-declared native descriptors/list buffers have already been destroyed.
    fn drop(&mut self) {
        if let Some(h) = &mut self.0 {
            let _ = h.finish();
        }
    }
}
/// Preserve real native capacity refusal instead of collapsing it into an apparently healthy watch.
#[cfg(target_os = "linux")]
fn native_error(e: rustix::io::Errno) -> WatchError {
    if e == rustix::io::Errno::NOSPC
        || e == rustix::io::Errno::MFILE
        || e == rustix::io::Errno::NFILE
    {
        WatchError::Exhausted
    } else {
        WatchError::Native
    }
}
impl<P: LocalReadPolicy> LocalMount<P> {
    /// Establish bounded actual Linux inotify hints; no per-root pool or background polling thread.
    /// Partial enumeration/watch-limit exhaustion remains incomplete coverage and forces reconciliation.
    pub fn watch(&self, budget: Budget) -> Result<HintWatch<'_, P>, WatchError> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = budget;
            return Err(WatchError::Unsupported);
        }
        #[cfg(target_os = "linux")]
        {
            if budget.max_memory_bytes < 65536 || !budget_allows_start(budget) {
                return Err(WatchError::Budget);
            }
            let account = Account::current().map_err(WatchError::Admission)?;
            let holding = match account
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
            let mut listing_budget = budget;
            listing_budget.max_memory_bytes = budget.max_memory_bytes / 2;
            let enumeration = self
                .list_admitted(
                    listing_budget,
                    pending.0.as_ref().ok_or(WatchError::Budget)?,
                )
                .map_err(WatchError::Backend)?;
            let (entries, mut complete) = match enumeration {
                Enumeration::Complete { entries, .. } => (entries, true),
                Enumeration::Partial { entries, .. } => (entries, false),
            };
            let admission = match gate(&self.policy, &self.library_id, &self.mount_id)
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
            let root = self.root().map_err(WatchError::Backend)?;
            let fd = inotify::init(inotify::CreateFlags::NONBLOCK | inotify::CreateFlags::CLOEXEC)
                .map_err(native_error)?;
            let flags = inotify::WatchFlags::CREATE
                | inotify::WatchFlags::DELETE
                | inotify::WatchFlags::MOVED_FROM
                | inotify::WatchFlags::MOVED_TO
                | inotify::WatchFlags::MODIFY
                | inotify::WatchFlags::ATTRIB
                | inotify::WatchFlags::CLOSE_WRITE
                | inotify::WatchFlags::DELETE_SELF
                | inotify::WatchFlags::MOVE_SELF
                | inotify::WatchFlags::ONLYDIR;
            let add = |dir: &File| {
                inotify::add_watch(&fd, format!("/proc/self/fd/{}", dir.as_raw_fd()), flags)
            };
            add(&root).map_err(native_error)?;
            let mut charge = 16384u64;
            for entry in entries.iter().filter(|e| e.kind == EntryKind::Directory) {
                charge = charge.checked_add(8192).ok_or(WatchError::Budget)?;
                if charge > budget.max_memory_bytes / 2 {
                    complete = false;
                    break;
                }
                if live(self, &admission).map_err(WatchError::Backend)? != Live::Current {
                    return Err(WatchError::Backend(BackendError::Unavailable(
                        Unavailable::Unknown,
                    )));
                }
                let (parent, name) =
                    open_parent(&root, &entry.path).map_err(WatchError::Backend)?;
                let child =
                    native::open_directory(&parent, name.to_str().map_err(|_| WatchError::Native)?)
                        .map_err(map_native)
                        .map_err(WatchError::Backend)?;
                if add(&child).is_err() {
                    complete = false;
                    break;
                }
            }
            if live(self, &admission).map_err(WatchError::Backend)? != Live::Current {
                return Err(WatchError::Backend(BackendError::Unavailable(
                    Unavailable::Unknown,
                )));
            }
            Ok(HintWatch {
                mount: self,
                admission,
                fd: Some(fd),
                holding: pending.0.take().ok_or(WatchError::Budget)?,
                budget,
                epoch: 0,
                dirty: true,
                complete,
                last: Instant::now(),
            })
        }
    }
}
impl<P: LocalReadPolicy> HintWatch<'_, P> {
    /// Read only finite native hint work; overflow/root loss/startup can never establish absence.
    pub fn poll(&mut self) -> Result<HintState, WatchError> {
        if live(self.mount, &self.admission).map_err(WatchError::Backend)? != Live::Current {
            return Err(WatchError::Backend(BackendError::Unavailable(
                Unavailable::Unknown,
            )));
        }
        #[cfg(target_os = "linux")]
        {
            let mut bytes = [MaybeUninit::<u8>::uninit(); 8192];
            let fd = self.fd.as_ref().ok_or(WatchError::Native)?;
            let mut reader = inotify::Reader::new(fd, &mut bytes);
            let start = Instant::now();
            let mut n = 0u64;
            loop {
                if n >= self.budget.max_files || expired(start, self.budget.max_millis) {
                    self.dirty = true;
                    self.complete = false;
                    break;
                }
                match reader.next() {
                    Ok(event) => {
                        n += 1;
                        self.epoch = self.epoch.checked_add(1).ok_or(WatchError::Budget)?;
                        self.dirty = true;
                        self.last = Instant::now();
                        if event.events().contains(inotify::ReadFlags::ISDIR)
                            && event.events().intersects(
                                inotify::ReadFlags::CREATE | inotify::ReadFlags::MOVED_TO,
                            )
                        {
                            self.complete = false;
                        }
                        if event.events().intersects(
                            inotify::ReadFlags::QUEUE_OVERFLOW
                                | inotify::ReadFlags::IGNORED
                                | inotify::ReadFlags::DELETE_SELF
                                | inotify::ReadFlags::MOVE_SELF
                                | inotify::ReadFlags::UNMOUNT,
                        ) {
                            self.complete = false;
                        }
                    }
                    Err(e) if e == rustix::io::Errno::AGAIN => break,
                    Err(_) => {
                        self.complete = false;
                        self.dirty = true;
                        return Err(WatchError::Native);
                    }
                }
            }
        }
        Ok(HintState {
            epoch: self.epoch,
            reconciliation_required: self.dirty || !self.complete,
            coverage_complete: self.complete,
            quiet_millis: u64::try_from(self.last.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }
    /// Trusted native owner acknowledges ONLY its durable complete relevant scan at this exact hint epoch.
    /// Events arriving during that commit remain dirty; this never grants body reads or pruning rights.
    pub fn acknowledge_reconciliation(&mut self, epoch: u64) -> Result<HintState, WatchError> {
        let state = self.poll()?;
        if state.epoch == epoch && self.complete {
            self.dirty = false;
        }
        Ok(HintState {
            reconciliation_required: self.dirty || !self.complete,
            ..state
        })
    }
}
impl<P: LocalReadPolicy> Drop for HintWatch<'_, P> {
    /// Exact FD close destroys synchronous kernel watches before their reservation can be released.
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        drop(self.fd.take());
        self.admission.outside = Vec::new();
        let _ = self.holding.finish();
    }
}
