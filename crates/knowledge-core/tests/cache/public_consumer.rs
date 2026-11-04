//! Single-threaded public consumer. The integration test spawns this binary so
//! fork never happens inside the multithreaded test harness.
//! Stand-in modes admit the observer. Cache modes use only the public cache API.

#[allow(dead_code)]
mod support {
    include!("support.rs");
}

use knowledge_core::cache::{CacheError, Publication};
use std::env;
use std::ffi::c_void;
use std::io::{self, Read, Write};
use std::process::exit;
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
    command_loop(&mut cache, &mut pin, pid, fds[1])
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
    let mut byte = [0u8; 1];
    let read_bytes = unsafe { read(fds[0], byte.as_mut_ptr().cast::<c_void>(), 1) };
    if read_bytes != 1 {
        say("ERROR ChildPipe");
        return 1;
    }
    say("CHILD_DROPPED");
    say("HELD");
    let mut cache = Some(cache);
    let mut pin = Some(pin);
    let mut input = String::new();
    loop {
        input.clear();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            break;
        }
        match input.trim() {
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
    child: i32,
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
                let waited = unsafe { waitpid(child, &mut status, 1) };
                if waited == 0 {
                    say("CHILD_RUNNING");
                } else {
                    say(&format!("CHILD_DEAD {waited}"));
                }
            }
            "UNBLOCK" => {
                let marker = [1u8];
                unsafe {
                    let _ = write(write_fd, marker.as_ptr().cast::<c_void>(), 1);
                    close(write_fd);
                    let mut status = 0;
                    let waited = waitpid(child, &mut status, 0);
                    say(&format!("CHILD_REAPED {waited} {status}"));
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
    fn __errno_location() -> *mut i32;
}
