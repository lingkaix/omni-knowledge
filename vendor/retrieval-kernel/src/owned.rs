//! Prototype host boundary: one actual Core account and one exact owned pure-kernel worker.
//!
//! The kernel worker launches no subprocesses. Its complete thread/runtime/model RSS belongs to
//! this one process reservation. This is not the product's opaque processing/descendant executor.
use knowledge_core::admission::{Account, Configuration, Decision, Holding, Origin, Scheduling};
use knowledge_core::jobs::Reservation;
use retrieval_kernel::{Error, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

/// Internal native parent identity, never read from protocol input or accepted as authority.
const PARENT: &str = "OMNI_KERNEL_OWNED_PARENT";
static SIGNAL: AtomicI32 = AtomicI32::new(0);
/// Emit fixed classifications and numeric observations, never source paths/arguments.
fn error(code: &str, message: &str) -> Error {
    Error {
        code: code.into(),
        message: message.into(),
    }
}
/// A source-free budget refusal; the actual Core distinction is retained in its Debug classification.
fn admission(error: impl std::fmt::Debug) -> Error {
    self::error(
        "resource-limited",
        &format!("native process admission: {error:?}"),
    )
}
/// Settled parent/bootstrap or actual child lifetime; declare before the owned child guard.
struct Resource(Holding);
impl Drop for Resource {
    /// A live child guard always settles before these scope resources end.
    fn drop(&mut self) {
        let _ = self.0.finish();
    }
}
/// Reserve actual simultaneous parent/child resources, with no second pool/controller.
fn reserve(account: &Account, reservation: Reservation) -> Result<Resource> {
    match account
        .reserve(reservation, Origin::BeforeVerification)
        .map_err(admission)?
    {
        Decision::Ready(holding) => Ok(Resource(holding)),
        Decision::Waiting(reason) => Err(admission(reason)),
    }
}
/// Native signal handler does only an atomic store; cleanup remains on the actual owning thread.
extern "C" fn cancelled(signal: libc::c_int) {
    SIGNAL.store(signal, Ordering::Release);
}
/// Restore exact previous handlers; never change unrelated process configuration.
struct Signals(Vec<(libc::c_int, libc::sighandler_t)>);
impl Signals {
    /// Forward cancellation by settling the one owned group, not by exiting while it runs.
    fn install() -> Result<Self> {
        SIGNAL.store(0, Ordering::Release);
        let mut guard = Self(Vec::new());
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            // SAFETY: handler has the native signal ABI and only stores an atomic integer.
            let previous =
                unsafe { libc::signal(signal, cancelled as *const () as libc::sighandler_t) };
            if previous == libc::SIG_ERR {
                return Err(error(
                    "resource-limited",
                    "native signal installation failed",
                ));
            }
            guard.0.push((signal, previous));
        }
        Ok(guard)
    }
}
impl Drop for Signals {
    /// Restore only handlers replaced by this actual scope.
    fn drop(&mut self) {
        for (signal, previous) in self.0.drain(..).rev() {
            unsafe {
                libc::signal(signal, previous);
            }
        }
    }
}
/// Exact worker capability. An unreaped owned PID cannot be reused during observation/signalling.
struct Worker {
    child: Child,
    reaped: bool,
    start: Option<(u64, u64)>,
    stdout_eof: bool,
    stderr_eof: bool,
    // Last field: child descriptors die before their credit can finish.
    resources: Resource,
}
impl Worker {
    /// Observe exit without consuming the PID that pins the owned group against reuse.
    fn exited(&self) -> Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(error("unsettled", "owned worker wait evidence unavailable"));
        }
        Ok(unsafe { info.si_pid() } != 0)
    }
    /// Relay finite nonblocking batches. A stopped consumer cannot hide cancellation or RSS work.
    fn relay(&mut self) -> Result<()> {
        self.stdout_eof = relay(self.child.stdout.as_mut(), libc::STDOUT_FILENO)?;
        self.stderr_eof = relay(self.child.stderr.as_mut(), libc::STDERR_FILENO)?;
        Ok(())
    }
    /// Collect the actual group and pipe EOF before accepting terminal status or native peak RSS.
    fn poll(&mut self) -> Result<Option<(ExitStatus, u64)>> {
        self.relay()?;
        if !self.exited()? || !self.stdout_eof || !self.stderr_eof {
            return Ok(None);
        }
        self.stop()?;
        let status = self
            .child
            .try_wait()
            .map_err(|_| error("unsettled", "collected worker status unavailable"))?
            .ok_or_else(|| error("unsettled", "collected worker status missing"))?;
        // This executable owns exactly one child. RUSAGE_CHILDREN is its complete native
        // collected high-water RSS, including short work missed by live sampling, not a delta.
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) } != 0 {
            return Err(error(
                "resource-limited",
                "collected native RSS unavailable",
            ));
        }
        let peak = u64::try_from(usage.ru_maxrss)
            .map_err(|_| error("resource-limited", "native RSS observation invalid"))?;
        #[cfg(target_os = "linux")]
        let peak = peak
            .checked_mul(1024)
            .ok_or_else(|| error("resource-limited", "native RSS unit overflow"))?;
        Ok(Some((status, peak)))
    }
    /// One bounded collection attempt through M3's existing native group/I/O collector.
    /// No private collector copy, direct-child-only receipt, or successful kill implies settlement.
    fn stop(&mut self) -> Result<()> {
        if self.reaped {
            return Ok(());
        }
        let exited = self.exited()?;
        match observation(self.child.id()) {
            Ok(observed) => {
                if observed.group != self.child.id()
                    || self.start.is_some_and(|start| start != observed.start)
                {
                    return Err(error("unsettled", "owned worker identity changed"));
                }
                self.start = Some(observed.start);
            }
            Err(_) if exited => {} // Native exit evidence still pins this exact unreaped PID.
            Err(failure) => return Err(failure),
        }
        knowledge_processing::worker::settle(&mut self.child)
            .map_err(|_| error("unsettled", "owned group or IO collection unconfirmed"))?;
        self.reaped = true;
        Ok(())
    }
}
impl Drop for Worker {
    /// Foreground controlled shutdown retains this reachable owner, descriptors and ONE account.
    /// EOF/error/signal/unwind cannot return to main while native collection remains unknown.
    fn drop(&mut self) {
        while let Err(failure) = self.stop() {
            diagnostic(
                format!(
                    "unsettled owned-kernel pid={} start={:?} pending=true charged_bytes={:?}: {}",
                    self.child.id(),
                    self.start,
                    self.resources.0.remaining().ok(),
                    failure.message
                )
                .as_bytes(),
            );
            // Each attempt is bounded; failed native evidence is visible, never a CPU busy loop
            // or an invented completion. The parent/bootstrap/handler/cache scopes stay live too.
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}
/// Forward at most 64 KiB from an owned nonblocking pipe without an unbounded buffer or writer thread.
/// Return true only for actual EOF; a stopped consumer returns an error before the next native check.
fn relay(pipe: Option<&mut impl Read>, fd: i32) -> Result<bool> {
    let Some(pipe) = pipe else { return Ok(true) };
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut bytes = [0u8; 8192];
    for _ in 0..8 {
        match pipe.read(&mut bytes) {
            Ok(0) => return Ok(true),
            Ok(n) => Channel::new(fd, deadline)
                .and_then(|mut channel| channel.write_all(&bytes[..n]))
                .map_err(|_| error("io", "owned relay consumer unavailable"))?,
            Err(failure) if failure.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(failure) if failure.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(error("io", "owned relay read unavailable")),
        }
    }
    Ok(false)
}
/// Owned face IO: nonblocking syscalls and a finite monotonic deadline, including partial writes.
/// The face owns these descriptors. Normal completion restores the exact inherited flags.
pub struct Channel {
    fd: i32,
    flags: i32,
    deadline: Instant,
}
impl Channel {
    pub fn new(fd: i32, deadline: Instant) -> std::io::Result<Self> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self {
            fd,
            flags,
            deadline,
        })
    }
    pub fn deadline(&mut self, deadline: Instant) {
        self.deadline = deadline;
    }
    fn ready(&self, events: i16) -> std::io::Result<()> {
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "owned IO deadline",
                ));
            }
            let mut poll = libc::pollfd {
                fd: self.fd,
                events,
                revents: 0,
            };
            let n = unsafe { libc::poll(&mut poll, 1, remaining.as_millis().clamp(1, 100) as i32) };
            if n > 0 {
                return Ok(());
            }
            if n < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
}
impl Read for Channel {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        loop {
            self.ready(libc::POLLIN)?;
            let n = unsafe { libc::read(self.fd, bytes.as_mut_ptr().cast(), bytes.len()) };
            if n >= 0 {
                return Ok(n as usize);
            }
            let error = std::io::Error::last_os_error();
            if !matches!(
                error.kind(),
                std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
            ) {
                return Err(error);
            }
        }
    }
}
impl Write for Channel {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        loop {
            self.ready(libc::POLLOUT)?;
            let n = unsafe { libc::write(self.fd, bytes.as_ptr().cast(), bytes.len()) };
            if n >= 0 {
                return Ok(n as usize);
            }
            let error = std::io::Error::last_os_error();
            if !matches!(
                error.kind(),
                std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
            ) {
                return Err(error);
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Drop for Channel {
    fn drop(&mut self) {
        unsafe {
            libc::fcntl(self.fd, libc::F_SETFL, self.flags);
        }
    }
}
/// Best-effort diagnostics cannot block collection behind a stopped stderr consumer.
pub fn diagnostic(bytes: &[u8]) {
    if let Ok(mut stderr) = Channel::new(
        libc::STDERR_FILENO,
        Instant::now() + Duration::from_millis(100),
    ) {
        let _ = stderr.write_all(bytes);
        let _ = stderr.write_all(b"\n");
    }
}
struct InheritedFlags(Vec<(i32, i32)>);
impl InheritedFlags {
    fn capture() -> Self {
        Self(
            [0, 1, 2]
                .into_iter()
                .map(|fd| (fd, unsafe { libc::fcntl(fd, libc::F_GETFL) }))
                .collect(),
        )
    }
}
impl Drop for InheritedFlags {
    fn drop(&mut self) {
        for (fd, flags) in &self.0 {
            if *flags >= 0 {
                unsafe {
                    libc::fcntl(*fd, libc::F_SETFL, *flags);
                }
            }
        }
    }
}
pub fn output(bytes: &[u8]) -> Result<()> {
    let mut output = Channel::new(
        libc::STDOUT_FILENO,
        Instant::now() + Duration::from_secs(30),
    )
    .map_err(|_| error("io", "owned output unavailable"))?;
    output
        .write_all(bytes)
        .and_then(|_| output.write_all(b"\n"))
        .map_err(|_| error("resource-limited", "owned output deadline or IO failure"))
}
/// Full native process identity and resident bytes, not subtraction from a preexisting baseline.
struct Observation {
    start: (u64, u64),
    group: u32,
    resident: u64,
}
#[cfg(target_os = "linux")]
fn observation(pid: u32) -> Result<Observation> {
    let mut stat = String::new();
    File::open(format!("/proc/{pid}/stat"))
        .and_then(|file| file.take(16 * 1024).read_to_string(&mut stat))
        .map_err(|_| error("resource-limited", "owned worker identity unavailable"))?;
    let fields: Vec<_> = stat
        .rsplit_once(')')
        .ok_or_else(|| error("resource-limited", "owned worker identity invalid"))?
        .1
        .split_whitespace()
        .collect();
    let group = fields
        .get(2)
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| error("resource-limited", "owned worker group invalid"))?;
    let start = fields
        .get(19)
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| error("resource-limited", "owned worker start invalid"))?;
    let mut status = String::new();
    File::open(format!("/proc/{pid}/status"))
        .and_then(|file| file.take(16 * 1024).read_to_string(&mut status))
        .map_err(|_| error("resource-limited", "owned worker RSS unavailable"))?;
    let resident = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.parse::<u64>().ok())
        .and_then(|v| v.checked_mul(1024))
        .ok_or_else(|| error("resource-limited", "owned worker RSS invalid"))?;
    Ok(Observation {
        start: (start, 0),
        group,
        resident,
    })
}
#[cfg(target_os = "macos")]
fn observation(pid: u32) -> Result<Observation> {
    // SAFETY: native calls receive exact owned PID and correctly sized zeroed native result buffers.
    unsafe {
        let mut bsd = std::mem::zeroed::<libc::proc_bsdinfo>();
        let mut task = std::mem::zeroed::<libc::proc_taskinfo>();
        if libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut bsd as *mut libc::proc_bsdinfo).cast(),
            std::mem::size_of_val(&bsd) as i32,
        ) != std::mem::size_of_val(&bsd) as i32
            || libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTASKINFO,
                0,
                (&mut task as *mut libc::proc_taskinfo).cast(),
                std::mem::size_of_val(&task) as i32,
            ) != std::mem::size_of_val(&task) as i32
        {
            return Err(error(
                "resource-limited",
                "owned native worker observation unavailable",
            ));
        }
        Ok(Observation {
            start: (bsd.pbi_start_tvsec, bsd.pbi_start_tvusec),
            group: bsd.pbi_pgid,
            resident: task.pti_resident_size,
        })
    }
}
/// Verify the native internal parent is this exact executable, not an environment boolean grant.
fn parent_matches(parent: u32) -> Result<bool> {
    let current = fs::metadata(
        std::env::current_exe()
            .map_err(|_| error("resource-limited", "worker executable unavailable"))?,
    )
    .map_err(|_| error("resource-limited", "worker executable unavailable"))?;
    #[cfg(target_os = "linux")]
    let other = fs::metadata(format!("/proc/{parent}/exe"))
        .map_err(|_| error("resource-limited", "native worker parent unavailable"))?;
    #[cfg(target_os = "macos")]
    let other = {
        let mut bytes = [0_u8; 4096];
        let size = unsafe {
            libc::proc_pidpath(parent as i32, bytes.as_mut_ptr().cast(), bytes.len() as u32)
        };
        if size <= 0 {
            return Err(error(
                "resource-limited",
                "native worker parent unavailable",
            ));
        }
        use std::os::unix::ffi::OsStrExt;
        let length = bytes.iter().position(|v| *v == 0).unwrap_or(bytes.len());
        fs::metadata(std::ffi::OsStr::from_bytes(&bytes[..length]))
            .map_err(|_| error("resource-limited", "native worker parent unavailable"))?
    };
    Ok(current.dev() == other.dev() && current.ino() == other.ino())
}
/// Bind worker termination to its actual native parent before any input is processed.
fn parent_watch(parent: u32) -> Result<()> {
    if unsafe { libc::getppid() } as u32 != parent || !parent_matches(parent)? {
        return Err(error(
            "invalid-request",
            "unowned internal worker invocation",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let mut signal = 0;
        if unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut signal) } != 0
            || signal != libc::SIGKILL
        {
            return Err(error(
                "invalid-request",
                "native parent-death binding missing",
            ));
        }
    }
    #[cfg(target_os = "macos")]
    {
        // Register the exact live parent in the native kernel event queue; no process scan or timer.
        let queue = unsafe { libc::kqueue() };
        if queue < 0 {
            return Err(error("resource-limited", "native parent watch unavailable"));
        }
        let event = libc::kevent {
            ident: parent as libc::uintptr_t,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        if unsafe { libc::kevent(queue, &event, 1, std::ptr::null_mut(), 0, std::ptr::null()) } != 0
            || unsafe { libc::getppid() } as u32 != parent
        {
            unsafe {
                libc::close(queue);
            }
            return Err(error("resource-limited", "native parent watch failed"));
        }
        std::thread::spawn(move || {
            let mut event = std::mem::MaybeUninit::<libc::kevent>::zeroed();
            unsafe {
                libc::kevent(
                    queue,
                    std::ptr::null(),
                    0,
                    event.as_mut_ptr(),
                    1,
                    std::ptr::null(),
                );
                libc::close(queue);
                libc::_exit(1);
            }
        });
    }
    Ok(())
}
/// Run one actual owned kernel worker under the process-wide account, or admit its native child role.
/// The child inherits protocol IO unchanged; this module adds no command/authority registry.
pub(super) fn supervise(store: &Path) -> Result<Option<ExitStatus>> {
    if let Some(parent) = std::env::var_os(PARENT) {
        let parent = parent
            .to_str()
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or_else(|| error("invalid-request", "invalid internal worker parent"))?;
        parent_watch(parent)?;
        return Ok(None);
    }
    let memory = std::env::var("OMNI_KERNEL_MEMORY_BYTES")
        .ok()
        .map(|value| {
            if value.is_empty()
                || !value.bytes().all(|v| v.is_ascii_digit())
                || value.len() > 1 && value.starts_with('0')
            {
                return Err(error("invalid-request", "invalid native memory override"));
            }
            value
                .parse::<u64>()
                .map_err(|_| error("invalid-request", "invalid native memory override"))
        })
        .transpose()?;
    // The selected derived store may have historical public directory permissions. Never chmod
    // that existing user-selected directory: create an owned private native account directory on
    // its actual volume, then let Core validate the opened descriptor's owner/mode/device.
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let native = store.join(".native-account");
    match fs::DirBuilder::new().mode(0o700).create(&native) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => {
            return Err(error(
                "resource-limited",
                "native account directory unavailable",
            ));
        }
    }
    let directory = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(native)
        .map_err(|_| error("resource-limited", "selected native store unavailable"))?;
    let account = Account::start(
        directory,
        Configuration {
            memory_bytes: memory,
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 0,
            max_records: 32,
            max_descriptor_bytes: 4096,
            per_mount: 1,
        },
    )
    .map_err(admission)?;
    let _parent = reserve(
        account,
        Reservation {
            memory_bytes: 16 * 1024 * 1024,
            temporary_bytes: 0,
        },
    )?;
    let allowance = account.status().map_err(admission)?.memory_available;
    let _work = reserve(
        account,
        Reservation {
            memory_bytes: allowance,
            temporary_bytes: 512 * 1024 * 1024,
        },
    )?;
    let _signals = Signals::install()?;
    let mut command = Command::new(
        std::env::current_exe()
            .map_err(|_| error("resource-limited", "worker executable unavailable"))?,
    );
    command
        .args(std::env::args_os().skip(1))
        .env(PARENT, std::process::id().to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    #[cfg(target_os = "linux")]
    {
        let parent = std::process::id() as i32;
        // SAFETY: fork-before-exec hook calls only native prctl/getppid and creates no Rust allocation.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let _inherited_flags = InheritedFlags::capture();
    let mut worker = Worker {
        child: command
            .spawn()
            .map_err(|_| error("resource-limited", "owned kernel worker could not start"))?,
        resources: _work,
        reaped: false,
        start: None,
        stdout_eof: false,
        stderr_eof: false,
    };
    for fd in [
        worker
            .child
            .stdout
            .as_ref()
            .map(std::os::fd::AsRawFd::as_raw_fd),
        worker
            .child
            .stderr
            .as_ref()
            .map(std::os::fd::AsRawFd::as_raw_fd),
    ]
    .into_iter()
    .flatten()
    {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(error("io", "owned relay unavailable"));
        }
    }
    let mut unseen = 0u8;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if Instant::now() >= deadline {
            worker.stop()?;
            return Err(error(
                "resource-limited",
                "owned worker IO/work deadline exhausted; collection confirmed",
            ));
        }
        if SIGNAL.load(Ordering::Acquire) != 0 {
            worker.stop()?;
            return Err(error(
                "cancelled",
                "owned kernel worker cancelled and collected",
            ));
        }
        if let Some((status, peak)) = worker.poll()? {
            if peak > allowance {
                return Err(error(
                    "resource-limited",
                    &format!("owned worker peak RSS {peak} exceeded admitted bytes {allowance}"),
                ));
            }
            return Ok(Some(status));
        }
        match observation(worker.child.id()) {
            Ok(observed) => {
                unseen = 0;
                if observed.group != worker.child.id()
                    || worker.start.is_some_and(|start| start != observed.start)
                {
                    return Err(error("resource-limited", "owned worker identity changed"));
                }
                if worker.start.is_none() {
                    worker.start = Some(observed.start);
                    diagnostic(
                        format!(
                            "owned-kernel pid={} start={}:{} group={} rss_budget_bytes={allowance}",
                            worker.child.id(),
                            observed.start.0,
                            observed.start.1,
                            observed.group
                        )
                        .as_bytes(),
                    );
                }
                if observed.resident > allowance {
                    worker.stop()?;
                    return Err(error(
                        "resource-limited",
                        &format!(
                            "owned worker RSS {} exceeded admitted bytes {allowance}",
                            observed.resident
                        ),
                    ));
                }
            }
            Err(failure) => {
                if let Some((status, peak)) = worker.poll()? {
                    if peak > allowance {
                        return Err(error(
                            "resource-limited",
                            "owned short worker exceeded admitted RSS",
                        ));
                    }
                    return Ok(Some(status));
                }
                // Exec/exit teardown can temporarily omit VmRSS before wait4 is ready. Do not
                // invent a zero measurement: keep the FULL admitted charge, retry for <=100 ms,
                // and collect real peak RSS if this exact unreaped child exits in that window.
                if unseen < 10 {
                    unseen += 1;
                } else {
                    return Err(failure);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A full native pipe refuses at its finite deadline instead of trapping controlled shutdown.
    #[test]
    fn full_owned_pipe_refuses_at_io_deadline() {
        use std::os::fd::FromRawFd;
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let _reader = unsafe { File::from_raw_fd(fds[0]) };
        let _writer = unsafe { File::from_raw_fd(fds[1]) };
        let mut channel = Channel::new(fds[1], Instant::now() + Duration::from_millis(25)).unwrap();
        let start = Instant::now();
        let error = channel.write_all(&vec![0u8; 1024 * 1024]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    /// ECHILD leaves the actual owner foreground and charged; test-only SIGKILL is containment, not settlement.
    #[test]
    fn unconfirmed_collection_never_releases_the_worker_promise() {
        if std::env::var_os("OMNI_TEST_UNKNOWN_OWNER").is_none() {
            let containment = tempfile::tempdir().unwrap();
            let mut subject = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "owned::tests::unconfirmed_collection_never_releases_the_worker_promise",
                    "--nocapture",
                ])
                .env("OMNI_TEST_UNKNOWN_OWNER", containment.path())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let stderr = subject.stderr.take().unwrap();
            use std::io::BufRead;
            let mut lines = std::io::BufReader::new(stderr).lines();
            let evidence = lines.next().unwrap().unwrap();
            assert!(evidence.contains("pending=true"), "{evidence}");
            assert!(evidence.contains("67108864"), "{evidence}");
            std::thread::sleep(Duration::from_millis(300));
            assert!(
                subject.try_wait().unwrap().is_none(),
                "unknown owner exited"
            );
            // Test-only containment of an intentionally irrecoverable ECHILD fixture, NOT settlement.
            subject.kill().unwrap();
            subject.wait().unwrap();
            return;
        }
        static CACHE: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        let cache = CACHE.get_or_init(|| {
            let dir =
                tempfile::tempdir_in(std::env::var_os("OMNI_TEST_UNKNOWN_OWNER").unwrap()).unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
            dir
        });
        let account = Account::start(
            File::open(cache.path()).unwrap(),
            Configuration {
                memory_bytes: None,
                storage_bytes: None,
            },
            Scheduling {
                max_pending: 0,
                max_records: 32,
                max_descriptor_bytes: 4096,
                per_mount: 1,
            },
        )
        .unwrap();
        let before = account.status().unwrap().memory_available;
        let resources = reserve(
            account,
            Reservation {
                memory_bytes: 64 * 1024 * 1024,
                temporary_bytes: 0,
            },
        )
        .unwrap();
        let child = Command::new("/usr/bin/true")
            .process_group(0)
            .spawn()
            .unwrap();
        let mut worker = Worker {
            child,
            resources,
            reaped: false,
            start: None,
            stdout_eof: false,
            stderr_eof: false,
        };
        // Simulate another reaper consuming this exact child: the owner has no wait4 receipt.
        worker.child.wait().unwrap();
        assert_eq!(worker.stop().unwrap_err().code, "unsettled");
        assert!(!worker.reaped);
        assert_eq!(
            account.status().unwrap().memory_available,
            before - 64 * 1024 * 1024
        );
        // This must stay in controlled foreground shutdown; the outer native test observes it.
        drop(worker);
        panic!("unknown native owner returned to main");
    }

    /// An escaped pipe writer prevents settlement after leader exit, then permits real foreground recovery at EOF.
    #[cfg(target_os = "linux")]
    #[test]
    fn foreground_shutdown_waits_for_actual_pipe_collection_and_returns_credit() {
        use std::io::BufRead;
        use std::os::unix::fs::PermissionsExt;
        let cache = tempfile::tempdir().unwrap();
        fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let account = Account::start(
            File::open(cache.path()).unwrap(),
            Configuration {
                memory_bytes: None,
                storage_bytes: None,
            },
            Scheduling {
                max_pending: 0,
                max_records: 32,
                max_descriptor_bytes: 4096,
                per_mount: 1,
            },
        )
        .unwrap();
        let before = account.status().unwrap().memory_available;
        let resources = reserve(
            account,
            Reservation {
                memory_bytes: 4096,
                temporary_bytes: 0,
            },
        )
        .unwrap();
        let child = Command::new("/bin/sh")
            .args(["-c", "setsid sh -c 'echo ready; sleep 5' & wait"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let mut worker = Worker {
            child,
            resources,
            reaped: false,
            start: None,
            stdout_eof: false,
            stderr_eof: false,
        };
        let mut ready = String::new();
        std::io::BufReader::new(worker.child.stdout.as_mut().unwrap())
            .read_line(&mut ready)
            .unwrap();
        assert_eq!(ready.trim(), "ready");
        // The escaped writer already owns its pipe; killing the original group cannot close it.
        assert_eq!(worker.stop().unwrap_err().code, "unsettled");
        assert!(!worker.reaped);
        assert_eq!(account.status().unwrap().memory_available, before - 4096);
        let pending = Instant::now();
        drop(worker);
        assert!(pending.elapsed() >= Duration::from_millis(500));
        assert_eq!(account.status().unwrap().memory_available, before);
    }
}
