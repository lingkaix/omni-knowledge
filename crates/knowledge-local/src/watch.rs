//! Caller-driven native hints: no worker/thread, replay claim, body probe or event-derived absence.
use super::*;
use knowledge_core::admission::Holding;
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
#[cfg(target_os = "macos")]
#[path = "fsevents.rs"]
mod mac;
#[path = "fsevents_protocol.rs"]
#[allow(dead_code)] // The portable protocol is also compiled/tested without an Apple runtime.
mod protocol;

/// Observed native device cursor; startup still requires full bounded reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeCursor {
    /// Held selected native device and directory identity.
    pub root: (u64, u64),
    /// Native event-store UUID, not a pathname or read grant.
    pub volume: [u8; 16],
    /// Device-relative FSEvent ID observed before the inventory transaction.
    pub event_id: u64,
}

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
    /// The selected backend has no supported native watch (including OS-mounted NAS); reconcile explicitly.
    Unsupported,
    /// Native watch/instance/descriptor capacity is exhausted; show the limit and reconcile instead.
    Exhausted,
    /// Native descriptor/watch creation/read failed; reconcile rather than trusting continuity.
    Native,
    /// The finite native runtime lease ended; rebuild and perform bounded catch-up.
    LeaseExpired,
    /// Exit could not be confirmed within the stop bound; exact handles and costs remain retained.
    QuiescenceUnknown,
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
/// Exact native FD/helper lifetime; no detached callback runtime is presumed stopped.
pub struct HintWatch<'a, P: LocalReadPolicy> {
    /// Root owner remains live and is checked at every poll/acknowledgement.
    mount: &'a LocalMount<P>,
    /// Actual captured current owner revision/exclusions; macOS keeps these in its funded lease.
    #[cfg(target_os = "linux")]
    admission: Admission,
    /// Closed BEFORE releasing the same-account watch allocation.
    #[cfg(target_os = "linux")]
    fd: Option<OwnedFd>,
    /// Complete native watch/metadata/buffer plan, independent of execution mount slots.
    #[cfg(target_os = "linux")]
    holding: Holding,
    /// Exact funded helper lifetime, including its held root descriptor and callback context.
    #[cfg(target_os = "macos")]
    mac: mac::Lease,
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
/// Synchronous constructor prepayment; transfer the holding to the exact owner before native runtime setup.
struct Pending(Option<Holding>);
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
    /// Establish actual native hints; no per-root pool or detached background polling thread.
    /// Partial enumeration/watch-limit exhaustion remains incomplete coverage and forces reconciliation.
    pub fn watch(&self, budget: Budget) -> Result<HintWatch<'_, P>, WatchError> {
        if self.kind == BackendKind::Nas {
            return Err(WatchError::Unsupported);
        }
        #[cfg(target_os = "macos")]
        {
            self.mac_watch(budget, None)
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
impl<'a, P: LocalReadPolicy> HintWatch<'a, P> {
    /// Borrow the exact prepaid admission metadata of the actual native owner.
    fn admission(&self) -> &Admission {
        #[cfg(target_os = "linux")]
        {
            &self.admission
        }
        #[cfg(target_os = "macos")]
        {
            self.mac.admission()
        }
    }
    /// Borrow the exact selected source owner; a cursor cannot substitute a different mount or policy.
    pub fn mount(&self) -> &'a LocalMount<P> {
        self.mount
    }
    /// Return an actually observed native ID only while the current selected root/policy remains live.
    /// This is a candidate for post-inventory persistence, not acknowledgement or continuity authority.
    pub fn native_cursor(&mut self) -> Result<Option<NativeCursor>, WatchError> {
        self.poll()?;
        #[cfg(target_os = "macos")]
        {
            Ok(self.mac.cursor())
        }
        #[cfg(target_os = "linux")]
        {
            Ok(None)
        }
    }
    /// Stop the exact runtime and report unconfirmed quiescence instead of releasing its charge.
    pub fn close(mut self) -> Result<(), WatchError> {
        #[cfg(target_os = "macos")]
        if !self.mac.stop() {
            return Err(WatchError::QuiescenceUnknown);
        }
        #[cfg(target_os = "linux")]
        drop(self.fd.take());
        Ok(())
    }
    /// Read only finite native hint work; overflow/root loss/startup can never establish absence.
    pub fn poll(&mut self) -> Result<HintState, WatchError> {
        let current = live(self.mount, self.admission()).map_err(WatchError::Backend);
        #[cfg(target_os = "macos")]
        if !matches!(current, Ok(Live::Current)) {
            self.mac.stop();
        }
        if current? != Live::Current {
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
        #[cfg(target_os = "macos")]
        {
            let changed = self.mac.poll()?;
            if changed {
                self.dirty = true;
                self.last = Instant::now();
                self.epoch = self.epoch.checked_add(1).ok_or_else(|| {
                    self.complete = false;
                    WatchError::Budget
                })?;
            }
            self.complete &= !self.mac.gap();
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
    /// Close synchronous Linux watches or stop the exact macOS lease; its Drop settles only proven exit.
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        drop(self.fd.take());
        #[cfg(target_os = "macos")]
        self.mac.stop();
        #[cfg(target_os = "linux")]
        {
            self.admission.outside = Vec::new();
        }
        #[cfg(target_os = "linux")]
        let _ = self.holding.finish();
    }
}
