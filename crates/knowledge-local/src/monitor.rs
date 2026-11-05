//! Explicit native-event or budgeted-poll reconciliation hints, with visible reduced capability.
use super::*;
use knowledge_core::{
    admission::{Account, Decision, Holding, Origin},
    jobs::Reservation,
};
use std::time::Duration;
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
    /// Exact native watch owner shared by the direct-watch and monitor entry points.
    Native(HintWatch<'a, P>),
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
}
/// Report the actual selected native mechanism, without claiming durable replay.
fn native_mode() -> TrackingMode {
    #[cfg(target_os = "macos")]
    {
        TrackingMode::Fsevents
    }
    #[cfg(target_os = "linux")]
    {
        TrackingMode::Inotify
    }
}
impl<P: LocalReadPolicy> Monitor<'_, P> {
    /// Run only due bounded metadata work; all modes require current native root/policy checks.
    pub fn poll(&mut self) -> Result<TrackingStatus, WatchError> {
        let (mode, hints) = match &mut self.driver {
            Driver::Native(w) => (native_mode(), w.poll()?),
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
                mode: native_mode(),
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
        Ok(status)
    }
}
