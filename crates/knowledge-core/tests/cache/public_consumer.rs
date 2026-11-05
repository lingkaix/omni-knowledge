//! Single-threaded public consumer. The integration test spawns this binary so
//! fork never happens inside the multithreaded test harness.
//! Stand-in modes admit the observer. Cache modes use only the public cache API.

#[allow(dead_code, unexpected_cfgs)]
mod support {
    include!("support.rs");
}

use knowledge_core::cache::{CacheError, Publication};
use std::env;
use std::ffi::c_void;
use std::io::{self, Read, Write};
use std::process::exit;
use std::thread;
use std::time::{Duration, Instant};
use support::{
    attach, generous_limits, pin_object, process_state, publish_bytes, read_exact, start_token,
    variant_name,
};

fn main() {
    let mut args = env::args().skip(1);
    let code = match args.next().as_deref() {
        Some("admit-pass") => {
            say("PASS");
            0
        }
        Some("admit-fail") => {
            say("FAIL");
            2
        }
        Some("admit-hang") => {
            let mut byte = [0u8; 1];
            let _ = io::stdin().read(&mut byte);
            0
        }
        Some("admit-eof") => 0,
        Some("admit-line-hang") => {
            say("LINE");
            let mut byte = [0u8; 1];
            let _ = io::stdin().read(&mut byte);
            0
        }
        Some("sha256") => {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes).expect("sha256 stdin");
            say(&support::sha256_hex(&bytes));
            0
        }
        Some("competitor") => competitor(args.next()),
        Some("inherited-owner") => inherited_owner(args.next()),
        Some("fork-destroy-owner") => {
            let path = args.next();
            let hold_ready = args.next().as_deref() == Some("hold-ready");
            fork_destroy_owner(path, hold_ready)
        }
        _ => 2,
    };
    exit(code);
}

fn say(line: &str) {
    let mut out = io::stdout().lock();
    writeln!(out, "{line}").expect("stdout");
    out.flush().expect("flush");
}

fn fixture_path(path: Option<String>) -> String {
    let Some(path) = path else {
        say("ERROR Argument");
        exit(2);
    };
    path
}

fn competitor(path: Option<String>) -> i32 {
    let world = attach(std::path::Path::new(&fixture_path(path)));
    let cache = match world.open(generous_limits()) {
        Ok(cache) => cache,
        Err(error) => {
            say(&format!("RESULT {}", outcome(&error)));
            return 0;
        }
    };
    match read_exact(&cache, &world.premise, world.derived.len()) {
        Ok(bytes) => say(&format!("RESULT Opened {}", hex(&bytes))),
        Err(error) => say(&format!("RESULT Err {}", variant_name(&error))),
    }
    0
}

fn outcome(error: &CacheError) -> String {
    if matches!(error, CacheError::Busy) {
        "Busy".to_string()
    } else {
        format!("Err {}", variant_name(error))
    }
}

fn inherited_owner(path: Option<String>) -> i32 {
    let world = attach(std::path::Path::new(&fixture_path(path)));
    let cache = match world.open(generous_limits()) {
        Ok(cache) => cache,
        Err(error) => {
            say(&format!("ERROR {}", variant_name(&error)));
            return 1;
        }
    };
    match publish_bytes(&cache, &world.premise, &world.derived) {
        Ok(Publication::Installed) => {}
        Ok(other) => {
            say(&format!("ERROR {other:?}"));
            return 1;
        }
        Err(error) => {
            say(&format!("ERROR {}", variant_name(&error)));
            return 1;
        }
    }
    let pin = match pin_object(&cache, &world.premise) {
        Ok(pin) => pin,
        Err(error) => {
            say(&format!("ERROR {}", variant_name(&error)));
            return 1;
        }
    };
    let mut fds = [0i32; 2];
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        say("ERROR Pipe");
        return 1;
    }
    let expected_parent = std::process::id() as i32;
    let pid = unsafe { fork() };
    if pid < 0 {
        unsafe {
            close(fds[0]);
            close(fds[1]);
        }
        say("ERROR Fork");
        return 1;
    }
    if pid == 0 {
        arm_parent_death_signal(expected_parent);
        unsafe {
            close(fds[1]);
            let mut byte = [0u8; 1];
            loop {
                let read_bytes = read(fds[0], byte.as_mut_ptr().cast::<c_void>(), 1);
                if read_bytes >= 0 {
                    break;
                }
                if errno() != EINTR {
                    break;
                }
            }
            _exit(0);
        }
    }
    // The child is owned before any readiness line. A panic or early return
    // drops this guard. The inherited child stays blocked until UNBLOCK.
    let mut child = ChildGuard::arm(pid);
    unsafe { close(fds[0]) };
    let Some(token) = start_token(pid as u32) else {
        return report_failure(&mut child, "ERROR Observer");
    };
    say(&format!("CHILD {pid} {token}"));
    say("HELD");
    let mut cache = Some(cache);
    let mut pin = Some(pin);
    let code = command_loop(&mut cache, &mut pin, &mut child, fds[1]);
    abandon(&mut child, code)
}

fn fork_destroy_owner(path: Option<String>, hold_ready: bool) -> i32 {
    let world = attach(std::path::Path::new(&fixture_path(path)));
    let cache = match world.open(generous_limits()) {
        Ok(cache) => cache,
        Err(error) => {
            say(&format!("ERROR {}", variant_name(&error)));
            return 1;
        }
    };
    match publish_bytes(&cache, &world.premise, &world.derived) {
        Ok(Publication::Installed) => {}
        Ok(other) => {
            say(&format!("ERROR {other:?}"));
            return 1;
        }
        Err(error) => {
            say(&format!("ERROR {}", variant_name(&error)));
            return 1;
        }
    }
    let pin = match pin_object(&cache, &world.premise) {
        Ok(pin) => pin,
        Err(error) => {
            say(&format!("ERROR {}", variant_name(&error)));
            return 1;
        }
    };
    let mut fds = [0i32; 2];
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        say("ERROR Pipe");
        return 1;
    }
    let expected_parent = std::process::id() as i32;
    let pid = unsafe { fork() };
    if pid < 0 {
        unsafe {
            close(fds[0]);
            close(fds[1]);
        }
        say("ERROR Fork");
        return 1;
    }
    if pid == 0 {
        arm_parent_death_signal(expected_parent);
        if hold_ready {
            unsafe {
                // Keep the write end open so the parent blocks until its deadline.
                close(fds[0]);
                loop {
                    if pause() < 0 && errno() != EINTR {
                        break;
                    }
                }
                _exit(0);
            }
        }
        drop(pin);
        drop(cache);
        unsafe {
            close(fds[0]);
            let marker = b"D";
            let wrote = write(fds[1], marker.as_ptr().cast::<c_void>(), 1);
            if wrote != 1 {
                _exit(1);
            }
            _exit(0);
        }
    }
    let mut child = ChildGuard::arm(pid);
    let Some(token) = start_token(pid as u32) else {
        return report_failure(&mut child, "ERROR Observer");
    };
    say(&format!("CHILD {pid} {token}"));
    unsafe { close(fds[1]) };
    let ready = read_byte_bounded(fds[0], CHILD_SIGNAL_MS);
    unsafe { close(fds[0]) };
    match ready {
        Ok(_byte) => {}
        Err(error) if error.kind() == io::ErrorKind::TimedOut => {
            return report_failure(&mut child, "ERROR ChildTimeout");
        }
        Err(_) => return report_failure(&mut child, "ERROR ChildPipe"),
    }
    match observed_zombie(pid) {
        Ok(true) => {}
        Ok(false) => return report_failure(&mut child, "ERROR ChildUserspace"),
        Err(()) => return report_failure(&mut child, "ERROR Observer"),
    }
    say("CHILD_DROPPED");
    say("HELD");
    let mut cache = Some(cache);
    let mut pin = Some(pin);
    let mut input = String::new();
    let mut code = 0;
    loop {
        input.clear();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            break;
        }
        match input.trim() {
            "REAP" => {
                if child.reaped {
                    say("ERROR AlreadyReaped");
                    code = 1;
                    break;
                }
                code = announce_collect(&mut child);
                if code != 0 {
                    break;
                }
            }
            "RELEASE" => {
                pin.take();
                cache.take();
                say("RELEASED");
            }
            "QUIT" => break,
            _ => {
                say("ERROR Command");
                code = 1;
                break;
            }
        }
    }
    abandon(&mut child, code)
}

fn command_loop(
    cache: &mut Option<CacheHolder>,
    pin: &mut Option<PinHolder>,
    child: &mut ChildGuard,
    write_fd: i32,
) -> i32 {
    let mut input = String::new();
    loop {
        input.clear();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            break;
        }
        match input.trim() {
            "DROP_CACHE" => {
                cache.take();
                say("CACHE_DROPPED");
            }
            "DROP_PIN" => {
                pin.take();
                say("PIN_DROPPED");
            }
            "STATUS_CHILD" => match poll_child(child.pid) {
                PollChild::Running => say("CHILD_RUNNING"),
                PollChild::Collected { waited, status } => {
                    child.note_collected(waited, status);
                    say(&format!("CHILD_DEAD {waited}"));
                }
                PollChild::Unresolved => say("ERROR Unresolved"),
            },
            "UNBLOCK" => {
                let marker = [1u8];
                let wrote = unsafe { write(write_fd, marker.as_ptr().cast::<c_void>(), 1) };
                unsafe { close(write_fd) };
                if wrote != 1 {
                    say("ERROR Unblock");
                    return 1;
                }
                let code = announce_collect(child);
                if code != 0 {
                    return code;
                }
            }
            "QUIT" => break,
            _ => {
                say("ERROR Command");
                return 1;
            }
        }
    }
    0
}

type CacheHolder = knowledge_core::cache::Cache<support::Premise>;
type PinHolder = knowledge_core::cache::PinnedObject<support::Premise>;

const CHILD_SIGNAL_MS: i32 = 2_000;
const POLLIN: i16 = 1;
const WNOHANG: i32 = 1;
const SIGKILL: i32 = 9;
const ESRCH: i32 = 3;
const EINTR: i32 = 4;
const CLEANUP_BOUND: Duration = Duration::from_millis(2_000);

enum PollChild {
    Running,
    Collected { waited: i32, status: i32 },
    Unresolved,
}

fn poll_child(pid: i32) -> PollChild {
    for _ in 0..4 {
        let mut status = 0;
        let waited = unsafe { waitpid(pid, &mut status, WNOHANG) };
        if waited == 0 {
            return PollChild::Running;
        }
        if waited == pid {
            return PollChild::Collected { waited, status };
        }
        if errno() != EINTR {
            return PollChild::Unresolved;
        }
    }
    PollChild::Unresolved
}

struct ChildGuard {
    pid: i32,
    reaped: bool,
    waited: i32,
    status: i32,
}

impl ChildGuard {
    fn arm(pid: i32) -> Self {
        Self {
            pid,
            reaped: false,
            waited: 0,
            status: 0,
        }
    }

    fn note_collected(&mut self, waited: i32, status: i32) {
        self.reaped = true;
        self.waited = waited;
        self.status = status;
    }

    fn collect_bounded(&mut self) -> Result<(), ()> {
        if self.reaped {
            return Ok(());
        }
        if self.pid <= 0 {
            return Err(());
        }
        let deadline = Instant::now() + CLEANUP_BOUND;
        loop {
            match poll_child(self.pid) {
                PollChild::Collected { waited, status } => {
                    self.note_collected(waited, status);
                    return Ok(());
                }
                PollChild::Running if Instant::now() >= deadline => return Err(()),
                PollChild::Running => thread::yield_now(),
                PollChild::Unresolved => return Err(()),
            }
        }
    }

    /// Kill, then collect with `WNOHANG`. A failed signal or wait leaves the child unsettled.
    fn settle(&mut self) -> Result<(), ()> {
        if self.reaped {
            return Ok(());
        }
        if self.pid <= 0 {
            return Err(());
        }
        let killed = unsafe { kill(self.pid, SIGKILL) };
        let kill_errno = if killed == 0 { 0 } else { errno() };
        match self.collect_bounded() {
            Ok(()) => Ok(()),
            Err(()) => {
                if killed != 0 && kill_errno != ESRCH {
                    eprintln!("ERROR Unresolved kill {kill_errno}");
                }
                Err(())
            }
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.reaped || self.pid <= 0 {
            return;
        }
        if self.settle().is_err() {
            eprintln!("ERROR Unresolved");
        }
    }
}

fn announce_collect(child: &mut ChildGuard) -> i32 {
    match child.collect_bounded() {
        Ok(()) => {
            say(&format!("CHILD_REAPED {} {}", child.waited, child.status));
            0
        }
        Err(()) => {
            say("ERROR Unresolved");
            1
        }
    }
}

fn report_failure(child: &mut ChildGuard, line: &str) -> i32 {
    say(line);
    abandon(child, 1)
}

fn abandon(child: &mut ChildGuard, code: i32) -> i32 {
    if child.reaped {
        return code;
    }
    match child.settle() {
        Ok(()) => {
            say(&format!("CHILD_REAPED {} {}", child.waited, child.status));
            code
        }
        Err(()) => {
            say("ERROR Unresolved");
            1
        }
    }
}

/// Ask the kernel to kill this child if its parent dies.
/// That signal does not reap a zombie and is not the cleanup result.
/// The parent `ChildGuard` still collects this pid.
fn arm_parent_death_signal(expected: i32) {
    #[cfg(target_os = "linux")]
    unsafe {
        const PR_SET_PDEATHSIG: i32 = 1;
        prctl(PR_SET_PDEATHSIG, SIGKILL as u64, 0, 0, 0);
        let parent = getppid();
        if parent == 1 || parent != expected {
            _exit(1);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if expected == 1 {
            unsafe { _exit(1) };
        }
        arm_macos_parent_death(expected);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = expected;
        compile_error!("cache consumer parent-death guard is unimplemented for this target");
    }
}

/// Record `expected` before the race, then watch it from a C thread created
/// only after fork. This process does not start a Rust thread before fork.
#[cfg(target_os = "macos")]
fn arm_macos_parent_death(expected: i32) {
    unsafe {
        let mut thread: libc::pthread_t = 0;
        let started = libc::pthread_create(
            &mut thread,
            std::ptr::null(),
            macos_parent_watch,
            expected as isize as *mut c_void,
        );
        if started != 0 {
            _exit(1);
        }
        let _ = libc::pthread_detach(thread);
        if getppid() != expected {
            _exit(1);
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn macos_parent_watch(arg: *mut c_void) -> *mut c_void {
    unsafe { macos_parent_watch_native(arg as isize as i32) }
}

/// Self-kill on parent death. `SIGKILL` is not `waitpid` and is not readiness.
#[cfg(target_os = "macos")]
unsafe fn macos_parent_watch_native(expected: i32) -> ! {
    unsafe {
        if getppid() != expected {
            die_uncollected();
        }
        let queue = libc::kqueue();
        if queue >= 0 {
            let change = libc::kevent {
                ident: expected as libc::uintptr_t,
                filter: libc::EVFILT_PROC,
                flags: libc::EV_ADD | libc::EV_ENABLE,
                fflags: libc::NOTE_EXIT,
                data: 0 as libc::intptr_t,
                udata: std::ptr::null_mut(),
            };
            let armed = libc::kevent(queue, &change, 1, std::ptr::null_mut(), 0, std::ptr::null());
            if armed == 0 && getppid() == expected {
                let mut event = libc::kevent {
                    ident: 0,
                    filter: 0,
                    flags: 0,
                    fflags: 0,
                    data: 0,
                    udata: std::ptr::null_mut(),
                };
                loop {
                    let ready =
                        libc::kevent(queue, std::ptr::null(), 0, &mut event, 1, std::ptr::null());
                    if getppid() != expected {
                        die_uncollected();
                    }
                    if ready < 0 {
                        break;
                    }
                }
            }
            close(queue);
        }
        let delay = libc::timespec {
            tv_sec: 0,
            tv_nsec: 50_000_000,
        };
        loop {
            if getppid() != expected {
                die_uncollected();
            }
            libc::nanosleep(&delay, std::ptr::null_mut());
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn die_uncollected() -> ! {
    unsafe {
        kill(libc::getpid(), SIGKILL);
        _exit(1);
    }
}

/// `Ok(true)` is an unreaped zombie. `Ok(false)` is userspace, absence, or the deadline.
/// `Err` means the native observer could not answer. That is a setup failure, not a pass.
fn observed_zombie(pid: i32) -> Result<bool, ()> {
    #[cfg(target_os = "linux")]
    {
        let deadline = Instant::now() + Duration::from_millis(CHILD_SIGNAL_MS as u64);
        loop {
            match process_state(pid as u32) {
                Some('Z') => return Ok(true),
                None => return Ok(false),
                Some(_) if Instant::now() >= deadline => return Ok(false),
                Some(_) => thread::yield_now(),
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        observe_darwin_zombie(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        compile_error!("cache consumer zombie observation is unimplemented for this target");
    }
}

/// Confirm `SZOMB` for this pid and an unreaped exit from `waitid` `WNOWAIT`.
/// `WNOWAIT` leaves the child waitable. `ChildGuard` still collects it.
#[cfg(target_os = "macos")]
fn observe_darwin_zombie(pid: i32) -> Result<bool, ()> {
    let deadline = Instant::now() + Duration::from_millis(CHILD_SIGNAL_MS as u64);
    loop {
        let state = process_state(pid as u32);
        let exited = unreaped_owned_exit(pid)?;
        if state == Some('Z') && exited {
            return Ok(true);
        }
        if state.is_none() {
            return Ok(false);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::yield_now();
    }
}

/// `Ok(true)` means this pid has exited and remains waitable.
/// `Ok(false)` means it is still running or is not this parent's unreaped child.
/// `Err` is a `waitid` failure other than `EINTR` or `ECHILD`.
#[cfg(target_os = "macos")]
fn unreaped_owned_exit(pid: i32) -> Result<bool, ()> {
    if pid <= 0 {
        return Err(());
    }
    // SAFETY: `info` is zeroed `siginfo_t` storage for one `waitid` call.
    // `P_PID` selects this pid only. `WNOWAIT` does not collect it.
    unsafe {
        let mut info = std::mem::zeroed::<libc::siginfo_t>();
        *libc::__error() = 0;
        let options = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
        let waited = libc::waitid(libc::P_PID, pid as libc::id_t, &mut info, options);
        if waited != 0 {
            let err = *libc::__error();
            if err == EINTR || err == libc::ECHILD {
                return Ok(false);
            }
            return Err(());
        }
        if info.si_pid == 0 || info.si_pid != pid {
            return if info.si_pid == 0 { Ok(false) } else { Err(()) };
        }
        match info.si_code {
            libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED => Ok(true),
            _ => Ok(false),
        }
    }
}

fn read_byte_bounded(fd: i32, timeout_ms: i32) -> io::Result<u8> {
    let mut pollfd = PollFd {
        fd,
        events: POLLIN,
        revents: 0,
    };
    let ready = unsafe { poll(&mut pollfd, 1, timeout_ms) };
    if ready == 0 {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "child signal"));
    }
    if ready < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut byte = [0u8; 1];
    let read_bytes = unsafe { read(fd, byte.as_mut_ptr().cast::<c_void>(), 1) };
    if read_bytes == 1 {
        Ok(byte[0])
    } else if read_bytes == 0 {
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "child closed the signal pipe",
        ))
    } else {
        Err(io::Error::last_os_error())
    }
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

unsafe extern "C" {
    fn fork() -> i32;
    fn _exit(code: i32) -> !;
    fn pipe(pipefd: *mut i32) -> i32;
    fn read(fd: i32, buf: *mut c_void, count: usize) -> isize;
    fn write(fd: i32, buf: *const c_void, count: usize) -> isize;
    fn close(fd: i32) -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
    fn pause() -> i32;
    fn getppid() -> i32;
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
}
