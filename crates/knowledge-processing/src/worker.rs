//! Owned child settlement and resident-set watchdog.
//!
//! The watchdog samples the child's resident set and kills that process group
//! when it passes the host-admitted ceiling. It does not call `setrlimit` or
//! `RLIMIT_AS`. A missing sample while a ceiling is set is a failure.
//! macOS uses `proc_pidinfo` `PROC_PIDTASKINFO` `pti_resident_size`. Linux reads
//! `VmRSS`. Neither path treats a one-shot peak measurement as enforcement.

use std::fmt;
use std::io;
use std::process::Child;

/// Typed stop for one owned processor child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerFailure {
    /// Resident set passed the admitted ceiling.
    RssExceeded {
        /// Admitted ceiling in bytes.
        limit_bytes: u64,
        /// Sampled resident set in bytes.
        observed_bytes: u64,
    },
    /// The worker passed its deadline and was waited.
    Deadline,
    /// Captured output passed the existing 1 MiB worker cap.
    OutputExceeded,
    /// A required resident-set sample was unavailable.
    Unmeasured,
}

impl fmt::Display for WorkerFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RssExceeded {
                limit_bytes,
                observed_bytes,
            } => write!(
                formatter,
                "worker resident set {observed_bytes} exceeded admitted limit {limit_bytes}"
            ),
            Self::Deadline => formatter.write_str("worker deadline"),
            Self::OutputExceeded => formatter.write_str("worker output exceeded 1 MiB"),
            Self::Unmeasured => formatter.write_str("worker resident set could not be sampled"),
        }
    }
}

impl std::error::Error for WorkerFailure {}

/// Kill the owned process group and wait until this child is reaped.
pub fn settle(child: &mut Child) {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Current resident set of `pid` in bytes.
pub fn resident_bytes(pid: u32) -> io::Result<u64> {
    #[cfg(target_os = "linux")]
    {
        linux_resident(pid)
    }
    #[cfg(target_os = "macos")]
    {
        macos_resident(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "resident set is unavailable",
        ))
    }
}

#[cfg(target_os = "linux")]
fn linux_resident(pid: u32) -> io::Result<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        let kb: u64 = rest
            .split_whitespace()
            .next()
            .ok_or_else(|| io::Error::other("VmRSS is empty"))?
            .parse()
            .map_err(|_| io::Error::other("VmRSS is not a number"))?;
        return Ok(kb.saturating_mul(1024));
    }
    Err(io::Error::other("VmRSS is missing"))
}

#[cfg(target_os = "macos")]
fn macos_resident(pid: u32) -> io::Result<u64> {
    // SAFETY: `info` is a zeroed libc `proc_taskinfo` and the buffer length is its size.
    // `proc_pidinfo` writes at most that many bytes. The call does not use `RLIMIT_AS`.
    unsafe {
        let mut info = std::mem::zeroed::<libc::proc_taskinfo>();
        let wrote = libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            (&mut info as *mut libc::proc_taskinfo).cast::<libc::c_void>(),
            std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int,
        );
        if wrote <= 0 {
            return Err(io::Error::other("proc_pidinfo returned no task info"));
        }
        Ok(info.pti_resident_size)
    }
}

/// Sample once. A set ceiling that cannot be sampled is [`WorkerFailure::Unmeasured`].
pub fn enforce_rss(child: &mut Child, limit: Option<u64>) -> Result<(), WorkerFailure> {
    let Some(limit) = limit else {
        return Ok(());
    };
    match resident_bytes(child.id()) {
        Ok(observed) if observed > limit => {
            settle(child);
            Err(WorkerFailure::RssExceeded {
                limit_bytes: limit,
                observed_bytes: observed,
            })
        }
        Ok(_) => Ok(()),
        Err(_) => {
            settle(child);
            Err(WorkerFailure::Unmeasured)
        }
    }
}
