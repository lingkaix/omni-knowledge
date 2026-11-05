//! One owned process group with nonblocking I/O, continuous RSS/deadline observation, and bounded collection.
//! Generated regular-file writes use a prepaid `RLIMIT_FSIZE`, never an address-space/RSS rlimit.

use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use knowledge_core::admission::Holding;

use crate::funding::{Credits, DIAGNOSTIC_CAP, Funding};

/// Typed stop for one owned processor group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerFailure {
    /// Aggregate observed resident memory exceeded the admitted promise.
    RssExceeded {
        limit_bytes: u64,
        observed_bytes: u64,
    },
    /// The operation reached its explicit or claim deadline.
    Deadline,
    /// The current claim owner requested cancellation or supersession.
    Cancelled,
    /// Output exceeded its prepaid byte ceiling.
    OutputExceeded,
    /// Required native ownership/RSS evidence was unavailable.
    Unmeasured,
    /// The owned parser binary is not installed.
    Missing,
    /// Collection did not establish that all owned processes stopped within its finite bound.
    CleanupUnknown,
}
impl fmt::Display for WorkerFailure {
    /// Fixed classifications without source or credential data.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RssExceeded {
                limit_bytes,
                observed_bytes,
            } => write!(
                f,
                "worker resident set {observed_bytes} exceeded admitted limit {limit_bytes}"
            ),
            Self::Deadline => f.write_str("worker deadline"),
            Self::Cancelled => f.write_str("worker cancelled"),
            Self::OutputExceeded => f.write_str("worker output exceeded admitted byte ceiling"),
            Self::Unmeasured => {
                f.write_str("worker resident set or ownership could not be sampled")
            }
            Self::Missing => {
                f.write_str("parser worker is not installed beside the processing crate")
            }
            Self::CleanupUnknown => {
                f.write_str("owned process collection is unresolved; charges retained")
            }
        }
    }
}
impl std::error::Error for WorkerFailure {}

/// Exact owned process identity, captured while the child is still unreaped.
#[derive(Clone, Copy, Debug)]
pub struct ProcessIdentity {
    /// Native process identifier.
    pub pid: u32,
    /// Native start ticks (Linux) or start microseconds (macOS).
    pub start: u64,
}

/// Unresolved cleanup owns the actual child and its costs, not a synthetic completion receipt.
/// Dropping this error does not finish its holdings or claim settlement. The caller retains it for inspection.
pub struct UnsettledChild {
    /// Exact child/group leader identity. Its PID is pinned by keeping the child unreaped.
    pub identity: ProcessIdentity,
    /// Real process handle; no blocking wait is performed on drop.
    child: Child,
    /// Actual child and unwritten-storage capabilities retained until explicit settlement.
    holdings: Vec<Holding>,
    /// Exact adapter-owned descriptor numbers referenced by a still-unknown child.
    descriptors: Vec<File>,
    /// Owned generated fixtures that must not be removed while their writer is unresolved.
    pub artifacts: Vec<std::path::PathBuf>,
}
impl fmt::Debug for UnsettledChild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsettledChild")
            .field("identity", &self.identity)
            .field("retained_holdings", &self.holdings.len())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for UnsettledChild {
    /// Expose only native ownership evidence, not paths or child diagnostics.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsettled owned process {} start {}",
            self.identity.pid, self.identity.start
        )
    }
}
impl std::error::Error for UnsettledChild {
    /// Preserve the typed unknown-cleanup classification.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&WorkerFailure::CleanupUnknown)
    }
}
impl UnsettledChild {
    /// Retain an owned staging path without deleting an unresolved writer's evidence.
    pub(crate) fn retain_artifact(&mut self, path: std::path::PathBuf) {
        self.artifacts.push(path);
    }

    /// Keep the exact descriptor number in a child's FD argument alive; a different dup is not a substitute.
    pub(crate) fn retain_descriptor(&mut self, file: File) {
        self.descriptors.push(file);
    }

    /// Inspect the actual source/directory/generated descriptors, not path-based substitutes.
    pub fn descriptors(&self) -> &[File] {
        &self.descriptors
    }

    /// Retain a live native-record envelope together with the unknown child.
    pub(crate) fn retain_holding(&mut self, holding: Holding) {
        self.holdings.push(holding);
    }

    /// Transfer descriptor leases only after actual collection returned the cost capabilities.
    pub fn take_descriptors(&mut self) -> Result<Vec<File>, WorkerFailure> {
        if !self.holdings.is_empty() {
            return Err(WorkerFailure::CleanupUnknown);
        }
        Ok(std::mem::take(&mut self.descriptors))
    }

    /// Retry ONLY native collection for this exact owned group, within two seconds.
    /// Return the still-owned cost capabilities; the host must reconcile unknown file writes before finishing them.
    pub fn collect(&mut self) -> Result<Vec<Holding>, WorkerFailure> {
        if self.holdings.is_empty() || self.identity.pid != self.child.id() {
            return Err(WorkerFailure::CleanupUnknown);
        }
        if self.identity.start == 0 {
            self.identity =
                owned_identity(&self.child).map_err(|_| WorkerFailure::CleanupUnknown)?;
        }
        collect(&mut self.child, self.identity, Duration::from_secs(2))?;
        Ok(std::mem::take(&mut self.holdings))
    }
}

impl Drop for UnsettledChild {
    /// Lost unknown owners strand actual native FD pins as well as Core credit; descriptor reuse cannot retarget a live child.
    fn drop(&mut self) {
        if !self.holdings.is_empty() {
            for file in self.descriptors.drain(..) {
                std::mem::forget(file);
            }
            if let Some(pipe) = self.child.stdout.take() {
                std::mem::forget(pipe);
            }
            if let Some(pipe) = self.child.stderr.take() {
                std::mem::forget(pipe);
            }
        }
    }
}

/// Compatibility path wrappers own their input FD and must retain its exact number on unknown child return.
pub(crate) fn retain_source<T>(mut result: Result<T>, file: File) -> Result<T> {
    if let Some(pending) = result
        .as_mut()
        .err()
        .and_then(|error| error.downcast_mut::<UnsettledChild>())
    {
        pending.retain_descriptor(file);
    }
    result
}

/// Child output bytes; the caller's transport holding must outlive this buffer.
pub(crate) struct Capture {
    /// Bounded stdout, allocated only after complete call admission.
    pub bytes: Vec<u8>,
}

/// Run an actual tool with borrowed input (no detached writer or payload copy).
/// Pipes are nonblocking and read/write progress shares the same RSS/cancellation/deadline loop.
pub(crate) fn run(
    mut command: Command,
    input: Option<&[u8]>,
    output_cap: u64,
    timeout: Duration,
    credits: &mut Credits,
    funding: &Funding<'_>,
    file_limit: Option<u64>,
) -> Result<Capture> {
    if timeout.is_zero() {
        bail!(WorkerFailure::Deadline);
    }
    if credits.rss == 0 {
        bail!(crate::admission::AdmitError::WaitingMemory);
    }
    funding.checkpoint()?;
    let capture_need = crate::opaque::capture_memory(output_cap);
    let paid = credits
        .transport
        .0
        .as_ref()
        .context("Missing transport credit")?
        .remaining()?
        .memory_bytes;
    if paid < capture_need {
        bail!(crate::admission::AdmitError::InsufficientCredit);
    }
    let cap = usize::try_from(output_cap).context("Worker output cap is not addressable")?;
    // The whole cap is promised above, not eagerly materialized. Grow only for
    // observed bytes under drain's prepaid-capacity boundary.
    let mut output = Vec::new();
    let mut diagnostic = Vec::new();
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&mut command, file_limit)?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("Worker deadline overflow")?;
    let mut child = command.spawn().context("Cannot start owned processor")?;
    let identity = match owned_identity(&child) {
        Ok(identity) => identity,
        Err(_) => {
            // Still unreaped, so the PID cannot be reused. Unknown identity is not successful cleanup.
            return Err(unsettled(
                child,
                ProcessIdentity { pid: 0, start: 0 },
                credits,
            ));
        }
    };
    let Some(mut stdout) = child.stdout.take() else {
        return Err(unsettled(child, identity, credits));
    };
    let Some(mut stderr) = child.stderr.take() else {
        return Err(unsettled(child, identity, credits));
    };
    let mut stdin = child.stdin.take();
    let setup = nonblocking(&stdout)
        .and_then(|_| nonblocking(&stderr))
        .and_then(|_| match &stdin {
            Some(pipe) => nonblocking(pipe),
            None => Ok(()),
        });
    let mut written: usize = 0;
    let mut stdout_done = false;
    let mut stderr_done = false;
    let mut unseen_samples = 0u8;
    let result: Result<()> = (|| {
        setup?;
        loop {
            funding.checkpoint()?;
            if Instant::now() >= deadline {
                bail!(WorkerFailure::Deadline);
            }
            // Sample even when a tool does not produce output or does not consume stdin.
            match group_resident(identity) {
                Ok(observed) => {
                    unseen_samples = 0;
                    if observed.resident > credits.rss {
                        bail!(WorkerFailure::RssExceeded {
                            limit_bytes: credits.rss,
                            observed_bytes: observed.resident
                        });
                    }
                }
                // Native exec/exit can briefly remove VmRSS before the zombie state is visible.
                // Still poll I/O, cancellation and the deadline; never turn a missing sample into zero.
                Err(_) if unseen_samples < 5 => unseen_samples += 1,
                Err(error) => {
                    if std::env::var_os("OMNI_NATIVE_TRACE").is_some() {
                        eprintln!(
                            "omni-native processor observation: pid={} start={} error={error}",
                            identity.pid, identity.start
                        );
                    }
                    bail!(WorkerFailure::Unmeasured)
                }
            }
            stdout_done |= drain(&mut stdout, &mut output, cap)?;
            stderr_done |= drain(&mut stderr, &mut diagnostic, DIAGNOSTIC_CAP as usize)?;
            if let Some(pipe) = &mut stdin {
                let payload = input.unwrap_or_default();
                match pipe.write(&payload[written..payload.len().min(written.saturating_add(8192))])
                {
                    Ok(n) => written += n,
                    Err(error)
                        if error.kind() == io::ErrorKind::WouldBlock
                            || error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error)
                        if error.kind() == io::ErrorKind::BrokenPipe && exited(identity.pid)? =>
                    {
                        stdin.take();
                    }
                    Err(error) => return Err(error.into()),
                }
                if written == payload.len() {
                    stdin.take();
                }
            }
            if exited(identity.pid)? {
                // A direct exit is not group/I/O settlement. Kill surviving group members before collection.
                if group_resident(identity)?.live != 0 || !stdout_done || !stderr_done {
                    signal_owned(identity)?;
                }
                if stdout_done && stderr_done && group_resident(identity)?.live == 0 {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })();
    // Keep actual read ends in the child state until EOF is proved by bounded collection.
    // A surviving writer outside the group is unknown I/O, not permission to release its charges.
    drop(stdin);
    child.stdout = Some(stdout);
    child.stderr = Some(stderr);
    if collect(&mut child, identity, Duration::from_secs(2)).is_err() {
        return Err(unsettled(child, identity, credits));
    }
    result?;
    let status = child
        .try_wait()?
        .context("Owned child status is unavailable")?;
    if !status.success() {
        let detail = String::from_utf8_lossy(&diagnostic[..diagnostic.len().min(4096)]);
        bail!("Processor failed with {status}: {}", detail.trim());
    }
    Ok(Capture { bytes: output })
}

/// Move unknown resource costs into the error instead of releasing an RAII child/storage guard.
fn unsettled(child: Child, mut identity: ProcessIdentity, credits: &mut Credits) -> anyhow::Error {
    if identity.pid == 0 {
        identity.pid = child.id();
    }
    let holdings = [
        credits.child.0.take(),
        credits.storage.0.take(),
        credits.transport.0.take(),
    ]
    .into_iter()
    .flatten()
    .collect();
    anyhow::Error::new(UnsettledChild {
        identity,
        child,
        holdings,
        descriptors: Vec::new(),
        artifacts: Vec::new(),
    })
}

/// Drain a bounded amount per watchdog tick; a continuously writing tool cannot starve observation.
fn drain(reader: &mut impl Read, output: &mut Vec<u8>, cap: usize) -> Result<bool> {
    let mut buffer = [0u8; 8192];
    for _ in 0..8 {
        let remaining = cap.saturating_sub(output.len());
        let bound = buffer.len().min(remaining.saturating_add(1));
        match reader.read(&mut buffer[..bound]) {
            Ok(0) => return Ok(true),
            Ok(n) if n > remaining => bail!(WorkerFailure::OutputExceeded),
            Ok(n) => {
                // Keep actual capacity inside the prepaid capture extent. Vec's default
                // doubling can exceed it even when the observed length still fits.
                let needed = output
                    .len()
                    .checked_add(n)
                    .context("capture size overflow")?;
                if needed > output.capacity() {
                    let grown = output.capacity().saturating_add(output.capacity() / 2);
                    let capacity = grown.max(needed).min(cap);
                    output
                        .try_reserve_exact(capacity - output.len())
                        .context("capture allocation refused")?;
                }
                output.extend_from_slice(&buffer[..n]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(false)
}

/// Native nonblocking pipes remove every detached I/O lifetime.
#[cfg(unix)]
fn nonblocking(pipe: &impl std::os::fd::AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    // SAFETY: the caller owns a live pipe and changes only its file-status flags.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Set a new owned group and a kernel write ceiling before exec, not after file growth.
#[cfg(unix)]
fn configure(command: &mut Command, file_limit: Option<u64>) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    if let Some(bytes) = file_limit {
        // SAFETY: only async-signal-safe setrlimit is called between fork and exec; no allocator or locks.
        unsafe {
            command.pre_exec(move || {
                let limit = libc::rlimit {
                    rlim_cur: bytes as libc::rlim_t,
                    rlim_max: bytes as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    Ok(())
}

/// Observe exit without reaping: keep the leader PID pinned until its entire group is settled.
#[cfg(unix)]
fn exited(pid: u32) -> io::Result<bool> {
    // SAFETY: initialized siginfo storage and a nonblocking, non-reaping wait on our owned child only.
    unsafe {
        let mut info: libc::siginfo_t = std::mem::zeroed();
        if libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        ) != 0
        {
            return Err(io::Error::last_os_error());
        }
        let observed = info.si_pid();
        if observed == 0 {
            return Ok(false);
        }
        if observed != pid as libc::pid_t
            || info.si_signo != libc::SIGCHLD
            || !matches!(
                info.si_code,
                libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
            )
        {
            return Err(io::Error::other(
                "Owned wait identity or exit status changed",
            ));
        }
        Ok(true)
    }
}

/// Exact WNOWAIT exit pins this owned PID even when Darwin hides a zombie's BSD data.
/// Missing information alone never authorizes signals, a zero-RSS sample or settlement.
fn checked_leader_exit(identity: ProcessIdentity) -> io::Result<bool> {
    let done = exited(identity.pid)?;
    if !done && (identity.start == 0 || process_identity(identity.pid)?.start != identity.start) {
        return Err(io::Error::other("Owned process identity changed"));
    }
    Ok(done)
}

/// Signal only the exact owned group while its original unreaped leader still pins the PID.
fn signal_owned(identity: ProcessIdentity) -> io::Result<()> {
    checked_leader_exit(identity)?;
    // SAFETY: process_group(0) created this group; its leader remains unreaped and identity was checked.
    if unsafe { libc::kill(-(identity.pid as i32), libc::SIGKILL) } != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}

/// Bounded exact group collection. No blocking wait and no ignored kill/wait failures.
fn collect(
    child: &mut Child,
    identity: ProcessIdentity,
    timeout: Duration,
) -> Result<(), WorkerFailure> {
    if identity.pid != child.id() {
        return Err(WorkerFailure::CleanupUnknown);
    }
    drop(child.stdin.take());
    if child
        .stdout
        .as_ref()
        .is_some_and(|p| nonblocking(p).is_err())
        || child
            .stderr
            .as_ref()
            .is_some_and(|p| nonblocking(p).is_err())
    {
        return Err(WorkerFailure::CleanupUnknown);
    }
    let end = Instant::now() + timeout;
    loop {
        let observed = (|| -> io::Result<bool> {
            let done = exited(identity.pid)?;
            let live = group_resident(identity)?;
            if done && live.live == 0 {
                let out = child
                    .stdout
                    .as_mut()
                    .map(discard_to_eof)
                    .transpose()?
                    .unwrap_or(true);
                let err = child
                    .stderr
                    .as_mut()
                    .map(discard_to_eof)
                    .transpose()?
                    .unwrap_or(true);
                if out && err {
                    // Reap only after every owned native process and pipe writer is proved settled.
                    if child.try_wait()?.is_some() {
                        drop(child.stdout.take());
                        drop(child.stderr.take());
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        if matches!(observed, Ok(true)) {
            return Ok(());
        }
        // Exit races can briefly make proc observations unavailable. Re-observe within the SAME bound;
        // an observation/kill/wait error is never accepted as completion or used to release a holding.
        let signalled = signal_owned(identity);
        if Instant::now() >= end
            || signalled.is_err()
                && matches!(observed, Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied)
        {
            if std::env::var_os("OMNI_NATIVE_TRACE").is_some() {
                eprintln!(
                    "omni-native processor collection: pid={} start={} observed={:?} signal_errno={:?}",
                    identity.pid,
                    identity.start,
                    observed.as_ref().err(),
                    signalled.as_ref().err().and_then(io::Error::raw_os_error)
                );
            }
            return Err(WorkerFailure::CleanupUnknown);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Discard only a fixed batch during failed-child collection, without retaining or growing any output.
fn discard_to_eof(reader: &mut impl Read) -> io::Result<bool> {
    let mut buffer = [0u8; 8192];
    for _ in 0..8 {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

/// Explicit bounded collection for a child already created in its own process group.
/// Unknown cleanup is an error; this function never asserts that an uncollected child stopped.
pub fn settle(child: &mut Child) -> Result<(), WorkerFailure> {
    let identity = owned_identity(child).map_err(|_| WorkerFailure::CleanupUnknown)?;
    collect(child, identity, Duration::from_secs(2))
}

/// A zero-RSS live process is still live and must be collected.
struct GroupSample {
    resident: u64,
    live: usize,
}

/// One native group sample includes every non-zombie descendant still in that owned group.
#[cfg(target_os = "linux")]
fn group_resident(identity: ProcessIdentity) -> io::Result<GroupSample> {
    if process_identity(identity.pid)?.start != identity.start {
        return Err(io::Error::other("Owned process identity changed"));
    }
    let mut total = 0u64;
    let mut live = 0;
    let mut examined = 0;
    for entry in std::fs::read_dir("/proc")? {
        examined += 1;
        if examined > 131_072 {
            return Err(io::Error::other("Process observation bound exceeded"));
        }
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        // Inspect only this native owned group; inability to inspect a live member is not a zero-RSS sample.
        // SAFETY: getpgid is a read-only native process observation.
        let group = unsafe { libc::getpgid(pid as i32) };
        if group != identity.pid as i32 {
            continue;
        }
        let stat = match linux_stat(pid) {
            Ok(stat) => stat,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if stat.group == identity.pid && stat.state != 'Z' && stat.state != 'X' {
            let resident = match resident_bytes(pid) {
                Ok(bytes) => bytes,
                Err(_) if linux_stat(pid).is_ok_and(|now| matches!(now.state, 'Z' | 'X')) => {
                    continue;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            total = total
                .checked_add(resident)
                .ok_or_else(|| io::Error::other("RSS overflow"))?;
            live += 1;
        }
    }
    Ok(GroupSample {
        resident: total,
        live,
    })
}

/// Bounded native Linux process facts; no shell and no unbounded status read.
#[cfg(target_os = "linux")]
struct LinuxStat {
    group: u32,
    state: char,
    start: u64,
}
#[cfg(target_os = "linux")]
fn linux_stat(pid: u32) -> io::Result<LinuxStat> {
    let mut stat = String::new();
    File::open(format!("/proc/{pid}/stat"))?
        .take(4096)
        .read_to_string(&mut stat)?;
    let (_, fields) = stat
        .rsplit_once(')')
        .ok_or_else(|| io::Error::other("Process stat is invalid"))?;
    let mut parts = fields.split_whitespace();
    let state = parts
        .next()
        .and_then(|s| s.chars().next())
        .ok_or_else(|| io::Error::other("Process state missing"))?;
    let group = parts
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::other("Process group missing"))?;
    let start = parts
        .nth(16)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::other("Process start missing"))?;
    Ok(LinuxStat {
        group,
        state,
        start,
    })
}

/// Native start identity; no ownership is inferred from the PID alone.
fn process_identity(pid: u32) -> io::Result<ProcessIdentity> {
    #[cfg(target_os = "linux")]
    {
        Ok(ProcessIdentity {
            pid,
            start: linux_stat(pid)?.start,
        })
    }
    #[cfg(target_os = "macos")]
    {
        let info = macos_bsd(pid)?;
        Ok(ProcessIdentity {
            pid,
            start: info
                .pbi_start_tvsec
                .saturating_mul(1_000_000)
                .saturating_add(info.pbi_start_tvusec),
        })
    }
}

/// Capture only a real owned Child. An exact unreaped exit is the sole fallback
/// when Darwin no longer supplies a short-lived child's BSD start identity.
fn owned_identity(child: &Child) -> io::Result<ProcessIdentity> {
    match process_identity(child.id()) {
        Ok(identity) => Ok(identity),
        #[cfg(target_os = "macos")]
        Err(_) if exited(child.id())? => Ok(ProcessIdentity {
            pid: child.id(),
            start: 0,
        }),
        Err(error) => Err(error),
    }
}

/// Current native resident set for public observers.
pub fn resident_bytes(pid: u32) -> io::Result<u64> {
    #[cfg(target_os = "linux")]
    {
        let mut text = String::new();
        File::open(format!("/proc/{pid}/status"))?
            .take(32 * 1024)
            .read_to_string(&mut text)?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let kb: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| io::Error::other("VmRSS invalid"))?;
                return kb
                    .checked_mul(1024)
                    .ok_or_else(|| io::Error::other("VmRSS overflow"));
            }
        }
        Err(io::Error::other("VmRSS missing"))
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: native fixed-size proc_taskinfo buffer; this is an observation, not RLIMIT_AS.
        unsafe {
            let mut info: libc::proc_taskinfo = std::mem::zeroed();
            let n = libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTASKINFO,
                0,
                (&mut info as *mut libc::proc_taskinfo).cast(),
                std::mem::size_of_val(&info) as i32,
            );
            if n != std::mem::size_of_val(&info) as i32 {
                return Err(io::Error::last_os_error());
            }
            Ok(info.pti_resident_size)
        }
    }
}

/// Native BSD identity and zombie state for the bounded macOS group walk.
#[cfg(target_os = "macos")]
fn macos_bsd(pid: u32) -> io::Result<libc::proc_bsdinfo> {
    // SAFETY: fixed-size native buffer, checked native result size.
    unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let n = libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            std::mem::size_of_val(&info) as i32,
        );
        if n != std::mem::size_of_val(&info) as i32 {
            return Err(io::Error::last_os_error());
        }
        Ok(info)
    }
}

/// macOS group observation stays bounded; native runner qualification is still required.
#[cfg(target_os = "macos")]
fn group_resident(identity: ProcessIdentity) -> io::Result<GroupSample> {
    let done = checked_leader_exit(identity)?;
    let mut pids = [0i32; 4096];
    // SAFETY: fixed array passed to libproc for the one owned process group (PROC_PGRP_ONLY=2).
    let bytes = unsafe {
        libc::proc_listpids(
            2,
            identity.pid,
            pids.as_mut_ptr().cast(),
            std::mem::size_of_val(&pids) as i32,
        )
    };
    if bytes < 0 || bytes as usize >= std::mem::size_of_val(&pids) {
        return Err(io::Error::other("Group observation unavailable"));
    }
    let mut rss = 0u64;
    let mut live = 0;
    for &pid in &pids[..bytes as usize / std::mem::size_of::<i32>()] {
        if pid <= 0 {
            continue;
        }
        if done && pid as u32 == identity.pid {
            continue;
        }
        let info = match macos_bsd(pid as u32) {
            Ok(info) => info,
            Err(_) if pid as u32 == identity.pid && exited(identity.pid)? => continue,
            Err(error) => return Err(error),
        };
        if info.pbi_pgid != identity.pid {
            return Err(io::Error::other("Owned group observation changed"));
        }
        if info.pbi_status != 5 {
            let resident = match resident_bytes(pid as u32) {
                Ok(bytes) => bytes,
                Err(_) if pid as u32 == identity.pid && exited(identity.pid)? => continue,
                Err(error) => return Err(error),
            };
            rss = rss
                .checked_add(resident)
                .ok_or_else(|| io::Error::other("RSS overflow"))?;
            live += 1;
        }
    }
    Ok(GroupSample {
        resident: rss,
        live,
    })
}

/// Sample one already owned child. The caller still has to collect it explicitly after a refusal.
pub fn enforce_rss(child: &mut Child, limit: u64) -> Result<(), WorkerFailure> {
    match resident_bytes(child.id()) {
        Ok(observed) if observed > limit => Err(WorkerFailure::RssExceeded {
            limit_bytes: limit,
            observed_bytes: observed,
        }),
        Ok(_) => Ok(()),
        Err(_) if exited(child.id()).unwrap_or(false) => Ok(()),
        Err(_) => Err(WorkerFailure::Unmeasured),
    }
}

/// Reference only the native descriptor already selected by the caller, never its former source path.
pub(crate) fn fd_path(file: &File) -> String {
    use std::os::fd::AsRawFd;
    #[cfg(target_os = "linux")]
    {
        format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd())
    }
    #[cfg(target_os = "macos")]
    {
        format!("/dev/fd/{}", file.as_raw_fd())
    }
}

/// Keep an already admitted source descriptor visible across macOS exec.
pub(crate) struct InheritedFd {
    #[cfg(target_os = "macos")]
    fd: std::os::fd::RawFd,
    #[cfg(target_os = "macos")]
    flags: i32,
}
impl InheritedFd {
    /// Linux children resolve the parent's descriptor. macOS inherits only this already owned descriptor.
    pub(crate) fn new(file: &File) -> Self {
        #[cfg(target_os = "macos")]
        {
            use std::os::fd::AsRawFd;
            let fd = file.as_raw_fd();
            // SAFETY: live caller-owned descriptor; restore exactly these flags when the guard dies.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if flags >= 0 {
                unsafe {
                    libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
                }
            }
            Self { fd, flags }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = file;
            Self {}
        }
    }
}
impl Drop for InheritedFd {
    /// Restore only the descriptor flags changed by this guard.
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if self.flags >= 0 {
            unsafe {
                libc::fcntl(self.fd, libc::F_SETFD, self.flags);
            }
        }
    }
}

/// Resolve the installed owned parser; the existing runtime selection order is unchanged.
pub(crate) fn parser_worker_path() -> Result<std::path::PathBuf, WorkerFailure> {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_parser_worker") {
        return Ok(path.into());
    }
    if let Some(path) = std::env::var_os("KNOWLEDGE_PROCESSING_PARSER_WORKER") {
        return Ok(path.into());
    }
    let exe = std::env::current_exe().map_err(|_| WorkerFailure::Missing)?;
    let mut candidates = Vec::new();
    if let Some(dir) = exe.parent() {
        candidates.push(dir.join("parser-worker"));
        if let Some(parent) = dir.parent() {
            candidates.push(parent.join("parser-worker"));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or(WorkerFailure::Missing)
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn capture_capacity_does_not_exceed_its_prepaid_extent() {
        let cap = 131_073;
        let input = vec![b'x'; cap];
        let mut reader = Cursor::new(&input);
        let mut output = Vec::new();
        while !drain(&mut reader, &mut output, cap).unwrap() {
            assert!(output.capacity() <= cap);
        }
        assert_eq!(output, input);
        assert!(output.capacity() <= cap);
    }

    #[test]
    fn excess_capture_byte_is_refused_before_buffer_growth() {
        let cap = 131_073;
        let mut reader = Cursor::new(vec![b'x'; cap + 1]);
        let mut output = Vec::new();
        loop {
            match drain(&mut reader, &mut output, cap) {
                Ok(false) => {}
                Ok(true) => panic!("extra byte accepted"),
                Err(error) => {
                    assert!(error.is::<WorkerFailure>());
                    break;
                }
            }
        }
        assert!(output.len() <= cap);
        assert!(output.capacity() <= cap);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::admission;

    /// A silent child that never consumes its input cannot strand an I/O thread or block deadline collection.
    #[test]
    fn blocked_stdin_has_bounded_real_collection() {
        let _gate = admission::TEST_GATE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        admission::establish_process_account().unwrap();
        let mut funding = Funding::BeforeVerification;
        let mut credits = Credits::new(
            &mut funding,
            8 * 1024 * 1024,
            0,
            crate::opaque::capture_memory(1024),
            Some(64 * 1024 * 1024),
            0,
        )
        .unwrap();
        let input = vec![b'x'; 8 * 1024 * 1024];
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let started = Instant::now();
        let error = run(
            command,
            Some(&input),
            1024,
            Duration::from_millis(200),
            &mut credits,
            &funding,
            None,
        )
        .err()
        .unwrap();
        assert_eq!(
            error.downcast_ref::<WorkerFailure>(),
            Some(&WorkerFailure::Deadline)
        );
        assert!(started.elapsed() < Duration::from_secs(4));
        drop(input);
        credits.input.release_now().unwrap();
    }

    /// RSS remains observed after startup even when no stdout/read completion wakes the parent.
    #[test]
    fn silent_late_allocation_is_observed_continuously() {
        let _gate = admission::TEST_GATE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        admission::establish_process_account().unwrap();
        let mut funding = Funding::BeforeVerification;
        let mut credits = Credits::new(
            &mut funding,
            0,
            0,
            crate::opaque::capture_memory(1024),
            Some(64 * 1024 * 1024),
            0,
        )
        .unwrap();
        let mut command = Command::new("python3");
        command.args([
            "-c",
            "import time; time.sleep(.25); data=bytearray(96*1024*1024); time.sleep(30)",
        ]);
        let started = Instant::now();
        let error = run(
            command,
            None,
            1024,
            Duration::from_secs(4),
            &mut credits,
            &funding,
            None,
        )
        .err()
        .unwrap();
        assert!(
            matches!(
                error.downcast_ref::<WorkerFailure>(),
                Some(WorkerFailure::RssExceeded {
                    limit_bytes: 67_108_864,
                    ..
                })
            ),
            "{error:#}"
        );
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    /// Actual stdout growth is stopped in a bounded prepaid memory buffer, not in an unreserved diagnostic tempfile.
    #[test]
    fn real_pipe_output_is_bounded_before_vector_growth() {
        let _gate = admission::TEST_GATE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        admission::establish_process_account().unwrap();
        let mut funding = Funding::BeforeVerification;
        let mut credits = Credits::new(
            &mut funding,
            0,
            0,
            crate::opaque::capture_memory(1024),
            Some(64 * 1024 * 1024),
            0,
        )
        .unwrap();
        let mut command = Command::new("python3");
        command.args(["-c", "import os; os.write(1,b'x'*1048576)"]);
        let error = run(
            command,
            None,
            1024,
            Duration::from_secs(4),
            &mut credits,
            &funding,
            None,
        )
        .err()
        .unwrap();
        assert_eq!(
            error.downcast_ref::<WorkerFailure>(),
            Some(&WorkerFailure::OutputExceeded)
        );
    }

    /// Native escaped I/O cannot be asserted collected: retain the actual child, pipes, costs, and fixture until proven stopped.
    #[test]
    fn unknown_io_retains_real_costs_and_recovers_after_exact_fixture_stop() {
        let _gate = admission::TEST_GATE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        admission::establish_process_account().unwrap();
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("owned-escaped-pid");
        let mut funding = Funding::BeforeVerification;
        let mut credits = Credits::new(
            &mut funding,
            0,
            0,
            crate::opaque::capture_memory(4096),
            Some(64 * 1024 * 1024),
            4096,
        )
        .unwrap();
        let charged = admission::current().unwrap().status().unwrap().reserved;
        let mut command = Command::new("python3");
        command.args(["-c", "import subprocess,sys; p=subprocess.Popen(['/bin/sleep','30'],start_new_session=True,stdout=sys.stdout,stderr=sys.stderr); open(sys.argv[1],'w').write(str(p.pid))"]).arg(&marker);
        let started = Instant::now();
        let result = run(
            command,
            None,
            4096,
            Duration::from_millis(250),
            &mut credits,
            &funding,
            Some(4096),
        );
        let during = admission::current().unwrap().status().unwrap().reserved;
        // Cleanup this test's explicitly created escaped process even when a deciding assertion fails.
        let escaped: u32 = std::fs::read_to_string(&marker).unwrap().parse().unwrap();
        let token = process_identity(escaped).unwrap();
        assert_eq!(process_identity(escaped).unwrap().start, token.start);
        // SAFETY: exact live PID/start just read from this owned fixture's native PID record, never a name/broad kill.
        assert_eq!(unsafe { libc::kill(escaped as i32, libc::SIGKILL) }, 0);
        let end = Instant::now() + Duration::from_secs(2);
        while linux_stat(escaped).is_ok_and(|s| !matches!(s.state, 'Z' | 'X'))
            && Instant::now() < end
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut error = result
            .err()
            .expect("escaped open writer must remain unknown");
        let pending = error
            .downcast_mut::<UnsettledChild>()
            .expect("actual unresolved child owner");
        let mut holdings = pending
            .collect()
            .expect("exact child/group/pipe collection after owned escaped writer stopped");
        assert_eq!(
            during, charged,
            "unknown cleanup cannot release child, transport, or storage costs"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(holdings.len(), 3);
        for holding in &mut holdings {
            if holding.remaining().unwrap().temporary_bytes > 0 {
                holding
                    .written(std::fs::metadata(&marker).unwrap().len())
                    .unwrap();
            }
            holding.finish().unwrap();
        }
        assert!(
            pending.collect().is_err(),
            "a collected owner does not replay native collection or finish costs again"
        );
    }
}
