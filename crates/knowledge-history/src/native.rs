//! Bounded owned Git/SSH tool collection; the caller's existing claim pays the entire native call.
//! No scheduler, quota pool, PID rediscovery authority, or automatic retry of Git effects is created here.

use super::*;
use rustix::process::{Pid, WaitId, WaitIdOptions};
use std::io;
use std::process::{Child, ExitStatus};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Native child identity captured before any reap, not a persistent authority token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    /// Exact owned PID/group leader.
    pub pid: u32,
    /// Native process start ticks or macOS start microseconds.
    pub start: Option<u64>,
}

/// Real pending child and its owned I/O, writer locks, and scratch evidence.
struct Pending {
    /// Unreaped child pins the native leader identity.
    child: Child,
    /// Fork inheritance is not native collection/unlock authority for this original owner.
    owner_pid: u32,
    /// Captured native start identity.
    identity: ProcessIdentity,
    /// Cooperating writer locks cannot be released while this child remains unknown.
    locks: Vec<File>,
    /// Recognized private scratch directories retained for inspection.
    artifacts: Vec<PathBuf>,
    /// Only set after actual group absence, pipe EOF and native reap.
    collected: bool,
}

/// Actual machine-only retained native owner; never serialize/reconstruct it from PID data.
#[derive(Clone)]
pub struct UnsettledChild {
    inner: Arc<Mutex<Pending>>,
    /// Completion belongs only to this exact native owner, never a reusable runtime-wide flag.
    settled: Arc<AtomicBool>,
}
/// One public call's actual unresolved owner; guards bind here before they can unwind.
pub(crate) type Retention = Arc<std::sync::OnceLock<UnsettledChild>>;
impl std::fmt::Debug for UnsettledChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UnsettledChild")
    }
}
impl std::fmt::Display for UnsettledChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("history native owner unsettled")
    }
}
impl std::error::Error for UnsettledChild {}
impl PartialEq for UnsettledChild {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}
impl Eq for UnsettledChild {}
impl UnsettledChild {
    /// Inspect actual retained identity/evidence; None explicitly means the native start sample is still unavailable.
    pub fn inspect(&self) -> Result<(ProcessIdentity, Vec<PathBuf>), Error> {
        let pending = self.inner.try_lock().map_err(|_| Error::Busy)?;
        Ok((pending.identity, pending.artifacts.clone()))
    }
    /// Finite retry of actual collection, without replaying Git or declaring the causing claim complete.
    pub fn collect(&self) -> Result<(), Error> {
        let mut pending = self.inner.try_lock().map_err(|_| Error::Busy)?;
        if pending.owner_pid != std::process::id() {
            return Err(Error::ContextMismatch);
        }
        if !pending.collected {
            let identity = pending.identity;
            if collect(&mut pending.child, identity, Duration::from_secs(2)).is_err() {
                return Err(Error::Unsettled(self.clone()));
            }
            pending.collected = true;
        }
        while let Some(lock) = pending.locks.pop() {
            if FileExt::unlock(&lock).is_err() {
                pending.locks.push(lock);
                return Err(Error::Unsettled(self.clone()));
            }
        }
        self.settled.store(true, Ordering::Release);
        Ok(())
    }

    /// The runtime may retire only this exact owner after whole native/lock collection.
    fn is_settled(&self) -> bool {
        self.settled.load(Ordering::Acquire)
    }

    /// Attach only this call's actual descriptor, serialized against this owner's whole collection.
    pub(crate) fn retain_lock(&self, file: File) {
        match self.inner.lock() {
            Ok(mut pending) if pending.owner_pid == std::process::id() => {
                if self.is_settled() {
                    let _ = FileExt::unlock(&file);
                } else {
                    pending.locks.push(file);
                }
            }
            _ => std::mem::forget(file),
        }
    }

    /// Keep this call's recognized directory until its actual owner is wholly collected; no later runtime owner is consulted.
    fn retain_directory(&self, directory: tempfile::TempDir) {
        match self.inner.lock() {
            Ok(mut pending) if pending.owner_pid == std::process::id() => {
                if !self.is_settled() {
                    pending.artifacts.push(directory.keep());
                }
            }
            _ => {
                let _ = directory.keep();
            }
        }
    }
}
impl Drop for Pending {
    /// A lost unknown owner strands actual native handles, not merely a counter or a false completion bit.
    fn drop(&mut self) {
        for lock in self.locks.drain(..) {
            std::mem::forget(lock);
        }
        if let Some(pipe) = self.child.stdout.take() {
            std::mem::forget(pipe);
        }
        if let Some(pipe) = self.child.stderr.take() {
            std::mem::forget(pipe);
        }
    }
}

/// Private scratch retained whenever an actual child can still access its paths.
pub(crate) struct Scratch {
    directory: Option<tempfile::TempDir>,
    retention: Retention,
}
impl Scratch {
    /// Create only below the selected external state directory, bound to this exact call's native owner.
    pub fn new(parent: &Path, call: &Call) -> Result<Self, Error> {
        Ok(Self {
            directory: Some(tempfile::tempdir_in(parent)?),
            retention: call.retention.clone(),
        })
    }
    /// Borrow the live private path; the directory exists until this owning guard is dropped.
    pub fn path(&self) -> &Path {
        self.directory.as_ref().expect("live owned scratch").path()
    }
}
impl Drop for Scratch {
    /// A stale/different runtime generation cannot decide this call's directory lifetime.
    fn drop(&mut self) {
        if let Some(owner) = self.retention.get()
            && let Some(directory) = self.directory.take()
        {
            owner.retain_directory(directory);
        }
    }
}

/// At most one actual child per selected History handle; try-lock refuses rather than creating another queue.
#[derive(Default)]
pub(crate) struct Runtime {
    /// Actual unresolved process owner, not a transaction registry.
    pending: Mutex<Option<UnsettledChild>>,
}

/// Bounded actual native result, including nonzero status for verification/refusal classification.
pub(crate) struct Output {
    /// Collected native exit status.
    pub status: ExitStatus,
    /// Complete capped stdout.
    pub stdout: Vec<u8>,
    /// Complete capped stderr; never automatically reflected to service errors.
    pub stderr: Vec<u8>,
}

impl Runtime {
    /// Inspect actual retained native identity/evidence without signalling it.
    pub fn pending(&self) -> Result<Option<(ProcessIdentity, Vec<PathBuf>)>, Error> {
        let slot = self.pending.try_lock().map_err(|_| Error::Busy)?;
        slot.as_ref().map(UnsettledChild::inspect).transpose()
    }

    /// Retry only exact native collection, never the Git command or a remote effect.
    pub fn collect(&self) -> Result<(), Error> {
        let mut slot = self.pending.try_lock().map_err(|_| Error::Busy)?;
        let Some(pending) = slot.as_mut() else {
            return Ok(());
        };
        pending.collect()?;
        slot.take();
        Ok(())
    }

    /// Execute a real tool using bounded nonblocking pipes, continuous resident observation, and finite collection.
    pub fn execute(
        &self,
        mut command: Command,
        input: &[u8],
        call: &Call,
    ) -> Result<Output, Error> {
        call.check()?;
        // An uncertain public call aborts; retry is a new explicit call, never a second owner in its guard scope.
        if call.retention.get().is_some() {
            return Err(Error::Recovery);
        }
        let mut slot = self.pending.try_lock().map_err(|_| Error::Busy)?;
        if let Some(pending) = slot.as_ref() {
            if !pending.is_settled() {
                return Err(Error::Unsettled(pending.clone()));
            }
            slot.take();
        }
        if input.len() > call.limit {
            return Err(Error::Capacity);
        }
        call.account_bytes(input.len())?;
        let cap = call
            .limit
            .min(usize::try_from(call.bytes_left.get()).unwrap_or(usize::MAX));
        // The existing claim covers capture, kernel pipes/native scratch and the explicit child RSS remainder.
        let mut stdout_bytes = Vec::with_capacity(cap);
        let mut stderr_bytes = Vec::with_capacity(cap);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure(&mut command, call.file_limit)?;
        let mut child = command.spawn()?;
        let identity = match process_identity(child.id()) {
            Ok(identity) => identity,
            Err(_) => {
                let identity = ProcessIdentity {
                    pid: child.id(),
                    start: None,
                };
                return self.abandon(&mut slot, child, identity, call);
            }
        };
        let Some(mut stdout) = child.stdout.take() else {
            return self.abandon(&mut slot, child, identity, call);
        };
        let Some(mut stderr) = child.stderr.take() else {
            return self.abandon(&mut slot, child, identity, call);
        };
        let mut stdin = child.stdin.take();
        let mut out_done = false;
        let mut err_done = false;
        let mut sent = 0usize;
        let mut unseen = 0u8;
        let setup = nonblocking(&stdout)
            .and_then(|_| nonblocking(&stderr))
            .and_then(|_| stdin.as_ref().map(nonblocking).transpose().map(|_| ()));
        let result = (|| -> Result<(), Error> {
            setup?;
            loop {
                call.check()?;
                match group_sample(identity) {
                    Ok(sample) => {
                        unseen = 0;
                        if sample.rss > call.rss_limit {
                            return Err(Error::Capacity);
                        }
                    }
                    Err(_) if unseen < 5 => unseen += 1,
                    Err(_) => return Err(Error::Recovery),
                }
                out_done |= drain(&mut stdout, &mut stdout_bytes, cap)?;
                err_done |= drain(&mut stderr, &mut stderr_bytes, cap)?;
                if let Some(pipe) = &mut stdin {
                    let end = input.len().min(sent.saturating_add(8192));
                    match pipe.write(&input[sent..end]) {
                        Ok(n) => sent += n,
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                            ) => {}
                        Err(error)
                            if error.kind() == io::ErrorKind::BrokenPipe
                                && exited(identity.pid)? =>
                        {
                            stdin.take();
                        }
                        Err(error) => return Err(error.into()),
                    }
                    if sent == input.len() {
                        stdin.take();
                    }
                }
                if exited(identity.pid)? {
                    let sample = group_sample(identity)?;
                    if sample.live > 0 {
                        signal(identity)?;
                        return Err(Error::Recovery);
                    }
                    if !out_done || !err_done {
                        signal(identity)?;
                    }
                    if sample.live == 0 && out_done && err_done {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(())
        })();
        drop(stdin);
        child.stdout = Some(stdout);
        child.stderr = Some(stderr);
        if collect(&mut child, identity, Duration::from_secs(2)).is_err() {
            return self.abandon(&mut slot, child, identity, call);
        }
        result?;
        let status = child.try_wait()?.ok_or(Error::Recovery)?;
        call.account_bytes(stdout_bytes.len())?;
        call.account_bytes(stderr_bytes.len())?;
        Ok(Output {
            status,
            stdout: stdout_bytes,
            stderr: stderr_bytes,
        })
    }

    /// Transfer the actual child into retained state before an error can unwind its caller's scratch/lock guards.
    fn abandon<T>(
        &self,
        slot: &mut Option<UnsettledChild>,
        child: Child,
        identity: ProcessIdentity,
        call: &Call,
    ) -> Result<T, Error> {
        let pending = UnsettledChild {
            inner: Arc::new(Mutex::new(Pending {
                child,
                identity,
                locks: Vec::new(),
                artifacts: Vec::new(),
                owner_pid: std::process::id(),
                collected: false,
            })),
            settled: Arc::new(AtomicBool::new(false)),
        };
        // execute checks an empty call scope before spawning, and aborts on its first unresolved child.
        assert!(
            call.retention.set(pending.clone()).is_ok(),
            "one native owner per public call"
        );
        *slot = Some(pending.clone());
        Err(Error::Unsettled(pending))
    }
}

/// Set nonblocking only on a live owned pipe, through the safe rustix descriptor API.
fn nonblocking(pipe: &impl std::os::fd::AsFd) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(pipe)?;
    rustix::fs::fcntl_setfl(pipe, flags | rustix::fs::OFlags::NONBLOCK)?;
    Ok(())
}

/// Drain a bounded batch per observation tick; output cannot grow the preallocated vector past its promise.
fn drain(reader: &mut impl Read, output: &mut Vec<u8>, cap: usize) -> Result<bool, Error> {
    let mut bytes = [0u8; 8192];
    for _ in 0..8 {
        let remaining = cap.saturating_sub(output.len());
        let bound = bytes.len().min(remaining.saturating_add(1));
        match reader.read(&mut bytes[..bound]) {
            Ok(0) => return Ok(true),
            Ok(n) if n > remaining => return Err(Error::Capacity),
            Ok(n) => output.extend_from_slice(&bytes[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(false)
}

/// Bounded discard after a failed child; no output allocation or detached I/O survives collection.
fn discard(reader: &mut impl Read) -> io::Result<bool> {
    let mut buffer = [0u8; 8192];
    for _ in 0..8 {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

/// Observe exit without reaping, pinning the exact original PID until group and I/O settlement.
fn exited(pid: u32) -> io::Result<bool> {
    let pid = Pid::from_raw(pid as i32).ok_or_else(|| io::Error::other("invalid native PID"))?;
    Ok(rustix::process::waitid(
        WaitId::Pid(pid),
        WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG,
    )?
    .is_some())
}

/// Signal only a group whose unreaped original leader still has the captured native identity.
fn signal(identity: ProcessIdentity) -> io::Result<()> {
    if identity.start.is_none() || process_identity(identity.pid)?.start != identity.start {
        return Err(io::Error::other("owned identity changed"));
    }
    let pid =
        Pid::from_raw(identity.pid as i32).ok_or_else(|| io::Error::other("invalid native PID"))?;
    match rustix::process::kill_process_group(pid, rustix::process::Signal::KILL) {
        Ok(()) => Ok(()),
        Err(error) => {
            let error = io::Error::from(error);
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

/// Finite collection retains the owner on every unresolved kill/wait/native-I/O condition.
fn collect(
    child: &mut Child,
    mut identity: ProcessIdentity,
    timeout: Duration,
) -> Result<(), Error> {
    if identity.pid != child.id() {
        return Err(Error::Recovery);
    }
    if identity.start.is_none() {
        identity = process_identity(child.id())?;
    }
    drop(child.stdin.take());
    if child
        .stdout
        .as_ref()
        .is_some_and(|pipe| nonblocking(pipe).is_err())
        || child
            .stderr
            .as_ref()
            .is_some_and(|pipe| nonblocking(pipe).is_err())
    {
        return Err(Error::Recovery);
    }
    let end = Instant::now() + timeout;
    loop {
        let complete = (|| -> io::Result<bool> {
            if exited(identity.pid)? && group_sample(identity)?.live == 0 {
                let out = child
                    .stdout
                    .as_mut()
                    .map(discard)
                    .transpose()?
                    .unwrap_or(true);
                let err = child
                    .stderr
                    .as_mut()
                    .map(discard)
                    .transpose()?
                    .unwrap_or(true);
                if out && err && child.try_wait()?.is_some() {
                    drop(child.stdout.take());
                    drop(child.stderr.take());
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        if matches!(complete, Ok(true)) {
            return Ok(());
        }
        let _signal_result = signal(identity); // Never accepted as settlement; the next iteration must prove every fact.
        if Instant::now() >= end {
            return Err(Error::Recovery);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Kernel regular-file extent ceiling before exec; no address-space rlimit is substituted for RSS observation.
fn configure(command: &mut Command, limit: u64) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    // SAFETY: only async-signal-safe setrlimit is called between fork/exec, with a checked native constant.
    unsafe {
        command.pre_exec(move || {
            let value = libc::rlimit {
                rlim_cur: limit as libc::rlim_t,
                rlim_max: limit as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &value) != 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    Ok(())
}

/// Bounded native process/group observation, not a one-shot peak measurement.
struct Sample {
    live: usize,
    rss: u64,
}

#[cfg(target_os = "linux")]
/// Minimal bounded Linux stat fields; no command lines, environment or user credentials are read.
struct Stat {
    group: u32,
    state: char,
    start: u64,
}
#[cfg(target_os = "linux")]
fn stat(pid: u32) -> io::Result<Stat> {
    let mut value = String::new();
    File::open(format!("/proc/{pid}/stat"))?
        .take(4096)
        .read_to_string(&mut value)?;
    let mut parts = value
        .rsplit_once(')')
        .ok_or_else(|| io::Error::other("invalid stat"))?
        .1
        .split_whitespace();
    let state = parts
        .next()
        .and_then(|v| v.chars().next())
        .ok_or_else(|| io::Error::other("missing state"))?;
    let group = parts
        .nth(1)
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| io::Error::other("missing group"))?;
    let start = parts
        .nth(16)
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| io::Error::other("missing start"))?;
    Ok(Stat {
        group,
        state,
        start,
    })
}

fn process_identity(pid: u32) -> io::Result<ProcessIdentity> {
    #[cfg(target_os = "linux")]
    {
        Ok(ProcessIdentity {
            pid,
            start: Some(stat(pid)?.start),
        })
    }
    #[cfg(target_os = "macos")]
    {
        let info = bsd(pid)?;
        Ok(ProcessIdentity {
            pid,
            start: Some(
                info.pbi_start_tvsec
                    .saturating_mul(1_000_000)
                    .saturating_add(info.pbi_start_tvusec),
            ),
        })
    }
}

#[cfg(target_os = "linux")]
fn resident(pid: u32) -> io::Result<u64> {
    let mut text = String::new();
    File::open(format!("/proc/{pid}/status"))?
        .take(32 * 1024)
        .read_to_string(&mut text)?;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            return value
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .and_then(|n| n.checked_mul(1024))
                .ok_or_else(|| io::Error::other("invalid RSS"));
        }
    }
    Err(io::Error::other("missing RSS"))
}

#[cfg(target_os = "linux")]
fn group_sample(identity: ProcessIdentity) -> io::Result<Sample> {
    if identity.start.is_none() || process_identity(identity.pid)?.start != identity.start {
        return Err(io::Error::other("owned identity changed"));
    }
    let mut sample = Sample { live: 0, rss: 0 };
    for (index, entry) in fs::read_dir("/proc")?.enumerate() {
        if index > 131_072 {
            return Err(io::Error::other("native process observation bound"));
        }
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|v| v.parse::<u32>().ok())
        else {
            continue;
        };
        // Kernel threads may have PGID zero; rustix getpgid assumes a nonzero group, so use the checked native query here.
        // SAFETY: getpgid takes only a bounded numeric PID, writes no memory and returns a native scalar.
        let group = unsafe {
            libc::getpgid(i32::try_from(pid).map_err(|_| io::Error::other("invalid PID"))?)
        };
        if group < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                continue;
            }
            return Err(error);
        }
        if group as u32 != identity.pid {
            continue;
        }
        let current = match stat(pid) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if current.group != identity.pid || matches!(current.state, 'Z' | 'X') {
            continue;
        }
        let rss = match resident(pid) {
            Ok(rss) => rss,
            Err(_) if stat(pid).is_ok_and(|s| matches!(s.state, 'Z' | 'X')) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        sample.live += 1;
        sample.rss = sample
            .rss
            .checked_add(rss)
            .ok_or_else(|| io::Error::other("RSS overflow"))?;
    }
    Ok(sample)
}

#[cfg(target_os = "macos")]
fn bsd(pid: u32) -> io::Result<libc::proc_bsdinfo> {
    // SAFETY: fixed-size native buffer; success requires an exact native result length.
    unsafe {
        let mut value: libc::proc_bsdinfo = std::mem::zeroed();
        let n = libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut value as *mut libc::proc_bsdinfo).cast(),
            std::mem::size_of_val(&value) as i32,
        );
        if n != std::mem::size_of_val(&value) as i32 {
            return Err(io::Error::last_os_error());
        }
        Ok(value)
    }
}
#[cfg(target_os = "macos")]
fn group_sample(identity: ProcessIdentity) -> io::Result<Sample> {
    if identity.start.is_none() || process_identity(identity.pid)?.start != identity.start {
        return Err(io::Error::other("owned identity changed"));
    }
    let mut pids = [0i32; 4096];
    // SAFETY: fixed PID array for the one owned group, libproc PROC_PGRP_ONLY=2.
    let bytes = unsafe {
        libc::proc_listpids(
            2,
            identity.pid,
            pids.as_mut_ptr().cast(),
            std::mem::size_of_val(&pids) as i32,
        )
    };
    if bytes < 0 || bytes as usize >= std::mem::size_of_val(&pids) {
        return Err(io::Error::other("native group observation bound"));
    }
    let mut sample = Sample { live: 0, rss: 0 };
    for &pid in &pids[..bytes as usize / std::mem::size_of::<i32>()] {
        if pid <= 0 || bsd(pid as u32)?.pbi_status == 5 {
            continue;
        }
        // SAFETY: fixed-size native task buffer; exact result length is required.
        let rss = unsafe {
            let mut task: libc::proc_taskinfo = std::mem::zeroed();
            let n = libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTASKINFO,
                0,
                (&mut task as *mut libc::proc_taskinfo).cast(),
                std::mem::size_of_val(&task) as i32,
            );
            if n != std::mem::size_of_val(&task) as i32 {
                return Err(io::Error::last_os_error());
            }
            task.pti_resident_size
        };
        sample.live += 1;
        sample.rss = sample
            .rss
            .checked_add(rss)
            .ok_or_else(|| io::Error::other("RSS overflow"))?;
    }
    Ok(sample)
}
