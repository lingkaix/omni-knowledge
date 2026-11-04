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
    attach, generous_limits, pin_object, publish_bytes, read_exact, start_token, variant_name,
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
        Some("fork-destroy-owner") => fork_destroy_owner(args.next()),
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
    let pid = unsafe { fork() };
    if pid < 0 {
        say("ERROR Fork");
        return 1;
    }
    if pid == 0 {
        arm_parent_death_signal();
        unsafe {
            close(fds[1]);
            let mut byte = [0u8; 1];
            loop {
                let read_bytes = read(fds[0], byte.as_mut_ptr().cast::<c_void>(), 1);
                if read_bytes >= 0 {
                    break;
                }
                if errno() != 4 {
                    break;
                }
            }
            _exit(0);
        }
    }
    unsafe { close(fds[0]) };
    let token = start_token(pid as u32).unwrap_or_else(|| "missing".to_string());
    say(&format!("CHILD {pid} {token}"));
    say("HELD");
    let mut cache = Some(cache);
    let mut pin = Some(pin);
    let mut child = ChildGuard { pid, reaped: false };
    command_loop(&mut cache, &mut pin, &mut child, fds[1])
}

fn fork_destroy_owner(path: Option<String>) -> i32 {
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
    let pid = unsafe { fork() };
    if pid < 0 {
        say("ERROR Fork");
        return 1;
    }
    if pid == 0 {
        arm_parent_death_signal();
        drop(pin);
        drop(cache);
        unsafe {
            close(fds[0]);
            let marker = b"D";
            let _ = write(fds[1], marker.as_ptr().cast::<c_void>(), 1);
            _exit(0);
        }
    }
    let token = start_token(pid as u32).unwrap_or_else(|| "missing".to_string());
    say(&format!("CHILD {pid} {token}"));
    unsafe { close(fds[1]) };
    match read_byte_bounded(fds[0], CHILD_SIGNAL_MS) {
        Ok(_byte) => {}
        Err(error) if error.kind() == io::ErrorKind::TimedOut => {
            say("ERROR ChildTimeout");
            return 1;
        }
        Err(_) => {
            say("ERROR ChildPipe");
            return 1;
        }
    }
    if !observed_zombie(pid) {
        say("ERROR ChildUserspace");
        return 1;
    }
    say("CHILD_DROPPED");
    say("HELD");
    let mut cache = Some(cache);
    let mut pin = Some(pin);
    let mut child = ChildGuard { pid, reaped: false };
    let mut input = String::new();
    loop {
        input.clear();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            break;
        }
        match input.trim() {
            "REAP" => {
                if child.reaped {
                    say("ERROR AlreadyReaped");
                    return 1;
                }
                let (waited, status) = child.reap_now();
                say(&format!("CHILD_REAPED {waited} {status}"));
            }
            "RELEASE" => {
                pin.take();
                cache.take();
                say("RELEASED");
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
            "STATUS_CHILD" => {
                let mut status = 0;
                let waited = unsafe { waitpid(child.pid, &mut status, 1) };
                if waited == 0 {
                    say("CHILD_RUNNING");
                } else {
                    if waited == child.pid {
                        child.reaped = true;
                    }
                    say(&format!("CHILD_DEAD {waited}"));
                }
            }
            "UNBLOCK" => {
                let marker = [1u8];
                unsafe {
                    let _ = write(write_fd, marker.as_ptr().cast::<c_void>(), 1);
                    close(write_fd);
                }
                let (waited, status) = child.reap_now();
                say(&format!("CHILD_REAPED {waited} {status}"));
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

struct ChildGuard {
    pid: i32,
    reaped: bool,
}

impl ChildGuard {
    fn reap_now(&mut self) -> (i32, i32) {
        let mut status = 0;
        let waited = unsafe { waitpid(self.pid, &mut status, 0) };
        if waited == self.pid {
            self.reaped = true;
        }
        (waited, status)
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.reaped || self.pid <= 0 {
            return;
        }
        unsafe {
            kill(self.pid, 9);
            let mut status = 0;
            waitpid(self.pid, &mut status, 0);
        }
        self.reaped = true;
    }
}

fn arm_parent_death_signal() {
    const PR_SET_PDEATHSIG: i32 = 1;
    unsafe {
        prctl(PR_SET_PDEATHSIG, 9, 0, 0, 0);
        if getppid() == 1 {
            _exit(1);
        }
    }
}

fn observed_zombie(pid: i32) -> bool {
    let deadline = Instant::now() + Duration::from_millis(CHILD_SIGNAL_MS as u64);
    loop {
        match proc_state(pid) {
            Some('Z') => return true,
            None => return false,
            Some(_) if Instant::now() >= deadline => return false,
            Some(_) => thread::yield_now(),
        }
    }
}

fn proc_state(pid: i32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let end = stat.rfind(')')?;
    stat[end + 1..].split_whitespace().next()?.chars().next()
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
    unsafe { *__errno_location() }
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
    fn getppid() -> i32;
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    fn __errno_location() -> *mut i32;
}
