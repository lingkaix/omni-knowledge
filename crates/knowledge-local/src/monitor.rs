//! Explicit native-event or budgeted-poll reconciliation hints, with visible reduced capability.
use super::*;
use knowledge_core::{
    admission::{Account, Decision, Holding, Origin},
    jobs::Reservation,
};
use std::time::Duration;
#[cfg(target_os = "macos")]
use {
    notify::Watcher,
    std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
/// Actual selected tracking mechanism, never a silent unsupported/native-success alias.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackingMode {
    /// Actual Linux inotify descriptors.
    Inotify,
    /// Actual macOS FSEvents stream; no fabricated persistent event ID/replay is claimed.
    Fsevents,
    /// Bounded native metadata polling; reduced event capability is explicit.
    Polling,
}
/// Native owner choices, not a portable manifest grant or hidden scheduler.
pub struct MonitorPolicy {
    /// Explicitly permit polling if native watch setup is unavailable/exhausted.
    pub allow_poll_fallback: bool,
    /// Explicit polling-only selection, including default NAS nightly/manual drivers.
    pub polling_only: bool,
    /// Actual interval between caller-driven metadata polls; zero is rejected.
    pub poll_interval: Duration,
}
/// Current mechanism/status with native failure and partial coverage retained.
pub struct TrackingStatus {
    /// Actual active mechanism.
    pub mode: TrackingMode,
    /// Why native setup could not be used; None for intentional polling-only mode.
    pub native_failure: Option<WatchError>,
    /// Native/metadata hints, never byte verification or inferred deletion.
    pub hints: HintState,
    /// This adapter always scans on restart; no durable event-ID replay is advertised.
    pub durable_event_replay: bool,
}
/// Caller-driven monitor; no timer/background polling worker and no per-library account.
pub struct Monitor<'a, P: LocalReadPolicy> {
    /// Already selected real local/OS-mounted root.
    mount: &'a LocalMount<P>,
    /// Concrete owned native mechanism.
    driver: Driver<'a, P>,
    /// Visible native failure for the fallback's lifetime.
    failure: Option<WatchError>,
}
/// Concrete mechanism state, not a plugin/provider registry.
enum Driver<'a, P: LocalReadPolicy> {
    /// Existing exact synchronous Linux FD owner.
    Native(HintWatch<'a, P>),
    /// Real native joined FSEvents owner.
    #[cfg(target_os = "macos")]
    Mac(MacWatch),
    /// Complete prepaid metadata polling state.
    Poll(PollWatch),
}
/// No worker exists; metadata buffers are destroyed before each call returns.
struct PollWatch {
    /// Actual SAME-account working-memory reservation.
    holding: Holding,
    /// Finite per-poll work maxima.
    budget: Budget,
    /// Caller-selected interval.
    interval: Duration,
    /// Actual next due instant; None means startup requires a scan.
    next: Option<Instant>,
    /// In-process scan/hint generation.
    epoch: u64,
    /// Most recent actual metadata completeness, never an absence inference.
    complete: bool,
    /// Pending durable reconciliation cannot disappear merely because the next poll is not due.
    dirty: bool,
}
impl Drop for PollWatch {
    /// No native worker or outstanding I/O remains; release only actual synchronous scratch.
    fn drop(&mut self) {
        let _ = self.holding.finish();
    }
}
/// Actual FSEvents lifetime; notify's stop implementation stops CFRunLoop and joins its exact thread.
#[cfg(target_os = "macos")]
struct MacWatch {
    /// Drop joins the native runloop BEFORE its resource promise can be released.
    watcher: Option<notify::FsEventWatcher>,
    /// Bounded event counter only, never queued paths/bodies/raw callbacks.
    counter: Option<Arc<AtomicU64>>,
    /// Native must-scan/error/overflow indication.
    gap: Option<Arc<AtomicBool>>,
    /// Actual native owner revision/exclusions captured at setup.
    admission: Admission,
    /// Prepaid runtime/thread/metadata plan, not a physical RSS enforcement claim.
    holding: Holding,
    /// Last observed native generation.
    epoch: u64,
    /// Actual latest observed activity time.
    last: Instant,
    /// Startup/native loss always requires reconciliation.
    dirty: bool,
}
#[cfg(target_os = "macos")]
impl Drop for MacWatch {
    /// Real native stop/join, then callback/owner metadata destruction, then settlement; panic strands charge.
    fn drop(&mut self) {
        drop(self.watcher.take());
        drop(self.counter.take());
        drop(self.gap.take());
        self.admission.outside = Vec::new();
        let _ = self.holding.finish();
    }
}
impl<P: LocalReadPolicy> LocalMount<P> {
    /// Select actual native hints, or ONLY explicitly permitted budgeted polling with reduced status.
    pub fn monitor(
        &self,
        budget: Budget,
        policy: MonitorPolicy,
    ) -> Result<Monitor<'_, P>, WatchError> {
        if policy.poll_interval.is_zero() {
            return Err(WatchError::Budget);
        }
        if !policy.polling_only {
            #[cfg(target_os = "linux")]
            match self.watch(budget) {
                Ok(w) => {
                    return Ok(Monitor {
                        mount: self,
                        driver: Driver::Native(w),
                        failure: None,
                    });
                }
                Err(e) => {
                    if !policy.allow_poll_fallback {
                        return Err(e);
                    }
                    return self.poll_monitor(budget, policy.poll_interval, Some(e));
                }
            }
            #[cfg(target_os = "macos")]
            match self.mac_watch(budget) {
                Ok(w) => {
                    return Ok(Monitor {
                        mount: self,
                        driver: Driver::Mac(w),
                        failure: None,
                    });
                }
                Err(e) => {
                    if !policy.allow_poll_fallback {
                        return Err(e);
                    }
                    return self.poll_monitor(budget, policy.poll_interval, Some(e));
                }
            }
        }
        self.poll_monitor(budget, policy.poll_interval, None)
    }
    /// Bind actual prepaid polling without probing any source body or inventing watch continuity.
    fn poll_monitor(
        &self,
        budget: Budget,
        interval: Duration,
        failure: Option<WatchError>,
    ) -> Result<Monitor<'_, P>, WatchError> {
        if !budget_allows_start(budget) || budget.max_memory_bytes < 32768 {
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
        Ok(Monitor {
            mount: self,
            driver: Driver::Poll(PollWatch {
                holding,
                budget,
                interval,
                next: None,
                epoch: 0,
                complete: false,
                dirty: true,
            }),
            failure,
        })
    }
    /// Establish the actual locked native FSEvents binding after prepaid runtime costs.
    #[cfg(target_os = "macos")]
    fn mac_watch(&self, budget: Budget) -> Result<MacWatch, WatchError> {
        if self.kind == BackendKind::Nas {
            return Err(WatchError::Unsupported);
        }
        if budget.max_memory_bytes < 8 * 1024 * 1024 || !budget_allows_start(budget) {
            return Err(WatchError::Budget);
        }
        let mut holding = match Account::current()
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
        let result = (|| {
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
            self.root().map_err(WatchError::Backend)?;
            let counter = Arc::new(AtomicU64::new(0));
            let gap = Arc::new(AtomicBool::new(false));
            let c = counter.clone();
            let g = gap.clone();
            let metadata = admission
                .outside
                .iter()
                .try_fold(self.path.as_os_str().len() as u64, |n, p| {
                    n.checked_add(p.as_str().len() as u64)
                })
                .ok_or(WatchError::Budget)?;
            if metadata
                .checked_mul(4)
                .is_none_or(|n| n > budget.max_memory_bytes / 4)
            {
                return Err(WatchError::Budget);
            }
            let root = self.path.clone();
            let exclusions = admission.outside.clone();
            let mut watcher = notify::FsEventWatcher::new(
                move |event: notify::Result<notify::Event>| {
                    if event.as_ref().is_ok_and(|e| {
                        !e.need_rescan()
                            && !e.paths.is_empty()
                            && e.paths.iter().all(|p| {
                                p.strip_prefix(&root)
                                    .ok()
                                    .and_then(|p| p.to_str())
                                    .is_some_and(|p| outside(p, &exclusions))
                            })
                    }) {
                        return;
                    }
                    if match &event {
                        Ok(e) => e.need_rescan(),
                        Err(_) => true,
                    } {
                        g.store(true, Ordering::Release)
                    }
                    if c.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
                        .is_err()
                    {
                        g.store(true, Ordering::Release)
                    }
                },
                notify::Config::default(),
            )
            .map_err(|_| WatchError::Native)?;
            watcher
                .watch(&self.path, notify::RecursiveMode::Recursive)
                .map_err(|_| WatchError::Native)?;
            if live(self, &admission).map_err(WatchError::Backend)? != Live::Current {
                return Err(WatchError::Backend(BackendError::Unavailable(
                    Unavailable::Changed,
                )));
            }
            Ok((watcher, counter, gap, admission))
        })();
        match result {
            Ok((watcher, counter, gap, admission)) => Ok(MacWatch {
                watcher: Some(watcher),
                counter: Some(counter),
                gap: Some(gap),
                admission,
                holding,
                epoch: 0,
                last: Instant::now(),
                dirty: true,
            }),
            Err(e) => {
                holding.finish().map_err(WatchError::Admission)?;
                Err(e)
            }
        }
    }
}
impl<P: LocalReadPolicy> Monitor<'_, P> {
    /// Run only due bounded metadata work; all modes require current native root/policy checks.
    pub fn poll(&mut self) -> Result<TrackingStatus, WatchError> {
        let (mode, hints) = match &mut self.driver {
            Driver::Native(w) => (TrackingMode::Inotify, w.poll()?),
            Driver::Poll(w) => {
                let now = Instant::now();
                let due = w.next.is_none_or(|n| now >= n);
                if due {
                    let e = self
                        .mount
                        .visit(w.budget, &w.holding, |_| Ok(()))
                        .map_err(WatchError::Backend)?;
                    w.complete = matches!(e, Enumeration::Complete { .. });
                    w.dirty = true;
                    w.epoch = w.epoch.checked_add(1).ok_or(WatchError::Budget)?;
                    w.next = now.checked_add(w.interval);
                    if w.next.is_none() {
                        return Err(WatchError::Budget);
                    }
                }
                (
                    TrackingMode::Polling,
                    HintState {
                        epoch: w.epoch,
                        reconciliation_required: w.dirty || !w.complete,
                        coverage_complete: w.complete,
                        quiet_millis: 0,
                    },
                )
            }
            #[cfg(target_os = "macos")]
            Driver::Mac(w) => {
                if live(self.mount, &w.admission).map_err(WatchError::Backend)? != Live::Current {
                    return Err(WatchError::Backend(BackendError::Unavailable(
                        Unavailable::Changed,
                    )));
                }
                let epoch = w
                    .counter
                    .as_ref()
                    .ok_or(WatchError::Native)?
                    .load(Ordering::Acquire);
                if epoch != w.epoch {
                    w.epoch = epoch;
                    w.dirty = true;
                    w.last = Instant::now();
                }
                let gap = w
                    .gap
                    .as_ref()
                    .ok_or(WatchError::Native)?
                    .load(Ordering::Acquire);
                (
                    TrackingMode::Fsevents,
                    HintState {
                        epoch,
                        reconciliation_required: w.dirty || gap,
                        coverage_complete: !gap,
                        quiet_millis: u64::try_from(w.last.elapsed().as_millis())
                            .unwrap_or(u64::MAX),
                    },
                )
            }
        };
        Ok(TrackingStatus {
            mode,
            native_failure: self.failure,
            hints,
            durable_event_replay: false,
        })
    }
    /// Native owner acknowledges ONLY a durable complete scan at an unchanged actual hint generation.
    pub fn acknowledge_reconciliation(&mut self, epoch: u64) -> Result<TrackingStatus, WatchError> {
        if let Driver::Native(w) = &mut self.driver {
            let h = w.acknowledge_reconciliation(epoch)?;
            return Ok(TrackingStatus {
                mode: TrackingMode::Inotify,
                native_failure: self.failure,
                hints: h,
                durable_event_replay: false,
            });
        }
        let mut status = self.poll()?;
        if let Driver::Poll(w) = &mut self.driver
            && status.hints.epoch == epoch
            && w.complete
        {
            w.dirty = false;
            status.hints.reconciliation_required = false;
        }
        #[cfg(target_os = "macos")]
        if let Driver::Mac(w) = &mut self.driver
            && status.hints.epoch == epoch
            && status.hints.coverage_complete
        {
            w.dirty = false;
            status.hints.reconciliation_required = false;
        }
        #[cfg(not(target_os = "macos"))]
        let _ = epoch;
        Ok(status)
    }
}
