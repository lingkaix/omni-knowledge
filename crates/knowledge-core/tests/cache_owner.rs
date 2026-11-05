//! Cross-process owner lifetime. The forking helper is a separate single-threaded
//! public consumer. Readiness is a pipe or a stdout line, never a sleep.
//! A missed helper signal is an observer failure.

#[allow(dead_code, unexpected_cfgs)]
mod support {
    include!("cache/support.rs");
}

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use support::{World, process_state, start_token};

const ADMIT_READY: Duration = Duration::from_secs(2);
const ADMIT_HANG: Duration = Duration::from_millis(500);
const PRODUCT_READY: Duration = Duration::from_secs(8);
const CLEANUP_BOUND: Duration = Duration::from_secs(2);
const ESRCH: i32 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ObserveErr {
    Timeout,
    MissingTerminal,
    Unresolved,
}

#[test]
fn observer_admits_pass_fail_and_timeout() {
    let bin = helper_bin();
    let mut pass = spawn(&bin, &["admit-pass"]);
    let line = pass.next_line(ADMIT_READY);
    assert_eq!(line.expect("observer pass").trim(), "PASS");
    let status = pass.wait_bounded(ADMIT_READY).expect("wait pass");
    assert!(status.success(), "admit-pass exit {status}");

    let mut fail = spawn(&bin, &["admit-fail"]);
    let line = fail.next_line(ADMIT_READY);
    assert_eq!(line.expect("observer fail").trim(), "FAIL");
    let status = fail.wait_bounded(ADMIT_READY).expect("wait fail");
    assert_eq!(status.code(), Some(2), "admit-fail exit {status}");

    let mut early = spawn(&bin, &["admit-eof"]);
    let missing = early.next_line(ADMIT_READY);
    assert_eq!(
        missing,
        Err(ObserveErr::MissingTerminal),
        "early exit was classified as {missing:?}"
    );
    let status = early.wait_bounded(ADMIT_READY).expect("eof exit");
    assert!(status.success(), "admit-eof exit {status}");

    let mut hang = spawn(&bin, &["admit-hang"]);
    let pid = hang.child.id();
    let start = hang.start.clone();
    let timed = hang.next_line(ADMIT_HANG);
    assert_eq!(
        timed,
        Err(ObserveErr::Timeout),
        "hang stand-in classification {timed:?}"
    );
    hang.kill_exact().expect("hang cleanup");
    assert_reaped(pid, &start);

    let mut line_hang = spawn(&bin, &["admit-line-hang"]);
    let pid = line_hang.child.id();
    let start = line_hang.start.clone();
    let line = line_hang.next_line(ADMIT_READY).expect("line then hang");
    assert_eq!(line.trim(), "LINE");
    let waited = line_hang.wait_bounded(ADMIT_HANG);
    assert_eq!(
        waited,
        Err(ObserveErr::Timeout),
        "line-then-hang classification {waited:?}"
    );
    line_hang.kill_exact().expect("line-hang cleanup");
    assert_reaped(pid, &start);

    failure_cleanup(&bin);
    fork_readiness_failure_settles_child(&bin);
}

#[test]
fn inherited_preexec_descriptor_does_not_extend_owner_lifetime() {
    admit_or_fail();
    let world = World::create(b"derived-SENTINEL_cache_6c2e91-payload-v1");
    let derived = fs::read(world.lab.root().join("derived.bin")).expect("derived");
    let mut owner = spawn(
        &helper_bin(),
        &["inherited-owner", &world.lab.root().to_string_lossy()],
    );
    let child_line = owner.next_line(PRODUCT_READY).expect("child line");
    let (child_pid, child_token) = parse_child(&child_line);
    let held = owner.next_line(PRODUCT_READY).expect("held");
    assert_eq!(held.trim(), "HELD");
    assert_eq!(
        owner.command("STATUS_CHILD", PRODUCT_READY).trim(),
        "CHILD_RUNNING"
    );
    let while_held = competitor(&world);
    assert_eq!(
        while_held,
        Competitor::Busy,
        "active owner admitted a competitor"
    );

    assert_eq!(
        owner.command("DROP_CACHE", PRODUCT_READY).trim(),
        "CACHE_DROPPED"
    );
    assert_eq!(
        owner.command("STATUS_CHILD", PRODUCT_READY).trim(),
        "CHILD_RUNNING"
    );
    let while_pinned = competitor(&world);
    assert_eq!(
        while_pinned,
        Competitor::Busy,
        "pin release admitted a competitor"
    );

    assert_eq!(
        owner.command("DROP_PIN", PRODUCT_READY).trim(),
        "PIN_DROPPED"
    );
    assert_eq!(
        owner.command("STATUS_CHILD", PRODUCT_READY).trim(),
        "CHILD_RUNNING",
        "pre-exec child exited before the owner released"
    );
    let released = competitor(&world);
    let primary = released.clone();
    let reaped = owner.command("UNBLOCK", PRODUCT_READY);
    owner.quit();
    let status = owner.wait_bounded(PRODUCT_READY).expect("owner wait");
    assert_eq!(
        primary,
        Competitor::Opened(hex_of(&derived)),
        "competitor after owner release, child reap line {reaped:?}, owner {status}"
    );
    assert!(reaped.trim().starts_with("CHILD_REAPED"), "{reaped}");
    assert!(
        start_token(child_pid).is_none(),
        "child {child_pid} {child_token} still live"
    );
    world.lab.finish();
}

#[test]
fn post_fork_child_destruction_does_not_unlock_the_parent() {
    admit_or_fail();
    let world = World::create(b"derived-SENTINEL_cache_6c2e91-payload-v1");
    let derived = fs::read(world.lab.root().join("derived.bin")).expect("derived");
    let mut owner = spawn(
        &helper_bin(),
        &["fork-destroy-owner", &world.lab.root().to_string_lossy()],
    );
    let child_line = owner.next_line(PRODUCT_READY).expect("child line");
    let (child_pid, child_token) = parse_child(&child_line);
    let dropped = owner.next_line(PRODUCT_READY).expect("dropped");
    assert_eq!(dropped.trim(), "CHILD_DROPPED");
    let held = owner.next_line(PRODUCT_READY).expect("held");
    assert_eq!(held.trim(), "HELD");
    let exit_deadline = Instant::now() + Duration::from_secs(1);
    while !exited_or_absent(child_pid) {
        assert!(
            Instant::now() < exit_deadline,
            "child {child_token} still in userspace after destruction: {}",
            process_state(child_pid).unwrap_or('?')
        );
        thread::yield_now();
    }
    let while_held = competitor(&world);
    let primary = while_held.clone();
    let reaped = owner.command("REAP", PRODUCT_READY);
    let released_line = owner.command("RELEASE", PRODUCT_READY);
    let after = competitor(&world);
    owner.quit();
    let status = owner.wait_bounded(PRODUCT_READY).expect("owner wait");
    assert_eq!(
        primary,
        Competitor::Busy,
        "child destruction unlocked the parent; release line {released_line:?}; owner {status}"
    );
    assert!(
        reaped.trim().starts_with("CHILD_REAPED"),
        "cleanup result {reaped}"
    );
    assert_eq!(released_line.trim(), "RELEASED");
    assert_eq!(after, Competitor::Opened(hex_of(&derived)));
    assert!(
        start_token(child_pid).is_none(),
        "forked child {child_pid} was not reaped"
    );
    world.lab.finish();
}

/// Real `fork-destroy-owner` readiness timeout. The forked child stays parked
/// and the helper must reap it. `PR_SET_PDEATHSIG` is not that proof.
fn fork_readiness_failure_settles_child(bin: &Path) {
    let world = World::create(b"derived-SENTINEL_cache_6c2e91-payload-v1");
    let mut owner = spawn(
        bin,
        &[
            "fork-destroy-owner",
            &world.lab.root().to_string_lossy(),
            "hold-ready",
        ],
    );
    let child_line = owner.next_line(PRODUCT_READY).expect("forked child line");
    let (child_pid, child_token) = parse_child(&child_line);
    let failed = owner.next_line(PRODUCT_READY).expect("readiness failure");
    assert_eq!(failed.trim(), "ERROR ChildTimeout");
    let reaped = owner.next_line(PRODUCT_READY).expect("cleanup line");
    assert!(
        reaped
            .trim()
            .starts_with(&format!("CHILD_REAPED {child_pid} ")),
        "helper did not reap the forked child: {reaped}"
    );
    let closed = owner.next_line(ADMIT_READY);
    assert_eq!(
        closed,
        Err(ObserveErr::MissingTerminal),
        "unexpected extra line {closed:?}"
    );
    let status = owner.wait_bounded(ADMIT_READY).expect("helper wait");
    assert_eq!(status.code(), Some(1), "readiness failure exit {status}");
    assert_reaped(child_pid, &child_token);
    world.lab.finish();
}

fn failure_cleanup(bin: &Path) {
    let probe = spawn(bin, &["admit-hang"]);
    let pid = probe.child.id();
    let start = probe.start.clone();
    let caught = panic::catch_unwind(AssertUnwindSafe(|| {
        let _owned = probe;
        panic!("deliberate observer failure");
    }));
    assert!(caught.is_err(), "failure stand-in did not unwind");
    assert_reaped(pid, &start);
}

fn assert_reaped(pid: u32, start: &str) {
    match start_token(pid) {
        None => {}
        Some(token) => assert_ne!(token, start, "child {pid} still live"),
    }
    assert_ne!(process_state(pid), Some('Z'), "child {pid} left unreaped");
}

fn admit_or_fail() {
    let bin = helper_bin();
    let mut pass = spawn(&bin, &["admit-pass"]);
    let line = pass
        .next_line(ADMIT_READY)
        .expect("observer was not admitted");
    assert_eq!(line.trim(), "PASS");
    assert!(pass.wait_bounded(ADMIT_READY).expect("wait").success());
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Competitor {
    Busy,
    Opened(String),
    Other(String),
}

fn competitor(world: &World) -> Competitor {
    let mut child = spawn(
        &helper_bin(),
        &["competitor", &world.lab.root().to_string_lossy()],
    );
    let line = child
        .next_line(PRODUCT_READY)
        .expect("competitor produced no line");
    let status = child.wait_bounded(PRODUCT_READY).unwrap_or_else(|error| {
        let cleanup = child.kill_exact();
        panic!("competitor wait {error:?} cleanup {cleanup:?} line {line}");
    });
    assert!(status.success(), "competitor exit {status} line {line}");
    let line = line.trim();
    if line == "RESULT Busy" {
        Competitor::Busy
    } else if let Some(hex) = line.strip_prefix("RESULT Opened ") {
        Competitor::Opened(hex.to_string())
    } else {
        Competitor::Other(line.to_string())
    }
}

fn hex_of(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn parse_child(line: &str) -> (u32, String) {
    let mut parts = line.split_whitespace();
    assert_eq!(parts.next(), Some("CHILD"), "{line}");
    let pid: u32 = parts.next().expect("pid").parse().expect("pid number");
    let token = parts.next().expect("start token").to_string();
    assert_eq!(
        token,
        start_token(pid).expect("child start token"),
        "{line}"
    );
    (pid, token)
}

struct Owned {
    child: Child,
    lines: mpsc::Receiver<String>,
    start: String,
    stdin: std::process::ChildStdin,
    settled: bool,
}

impl Owned {
    fn next_line(&mut self, timeout: Duration) -> Result<String, ObserveErr> {
        match self.lines.recv_timeout(timeout) {
            Ok(line) => Ok(line),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ObserveErr::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ObserveErr::MissingTerminal),
        }
    }

    fn wait_bounded(&mut self, timeout: Duration) -> Result<ExitStatus, ObserveErr> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.settled = true;
                    return Ok(status);
                }
                Ok(None) if Instant::now() >= deadline => return Err(ObserveErr::Timeout),
                Ok(None) => thread::yield_now(),
                Err(_) => return Err(ObserveErr::MissingTerminal),
            }
        }
    }

    fn command(&mut self, command: &str, timeout: Duration) -> String {
        writeln!(self.stdin, "{command}").expect("stdin");
        self.stdin.flush().expect("flush");
        self.next_line(timeout)
            .unwrap_or_else(|error| panic!("{error:?} waiting for {command}"))
    }

    fn quit(&mut self) {
        writeln!(self.stdin, "QUIT").expect("stdin");
        self.stdin.flush().expect("flush");
    }

    fn kill_exact(&mut self) -> Result<(), ObserveErr> {
        if self.settled {
            return Ok(());
        }
        let current = start_token(self.child.id());
        if current.as_deref() != Some(self.start.as_str()) {
            return Err(ObserveErr::Unresolved);
        }
        let kill_result = self.child.kill();
        match self.collect_bounded() {
            Ok(()) => Ok(()),
            Err(error) => {
                if let Err(kill_error) = kill_result
                    && kill_error.raw_os_error() != Some(ESRCH)
                {
                    eprintln!("kill failed before unresolved cleanup: {kill_error}");
                }
                Err(error)
            }
        }
    }

    fn collect_bounded(&mut self) -> Result<(), ObserveErr> {
        let deadline = Instant::now() + CLEANUP_BOUND;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.settled = true;
                    return Ok(());
                }
                Ok(None) if Instant::now() >= deadline => return Err(ObserveErr::Unresolved),
                Ok(None) => thread::yield_now(),
                Err(_) => return Err(ObserveErr::Unresolved),
            }
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        if let Err(error) = self.kill_exact() {
            eprintln!("ERROR Unresolved {error:?}");
        }
    }
}

/// An unreaped child stays observable as `Z` after `_exit`. That is not userspace.
/// `None` means the pid is absent. A live owned pid has a real state and is not absent.
fn exited_or_absent(pid: u32) -> bool {
    match process_state(pid) {
        None | Some('Z') => true,
        Some(_) => false,
    }
}

fn spawn(bin: &Path, args: &[&str]) -> Owned {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn helper");
    let start = start_token(child.id()).expect("helper start token");
    let stdout = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let stdin = child.stdin.take().expect("stdin");
    Owned {
        child,
        lines: rx,
        start,
        stdin,
        settled: false,
    }
}

fn helper_bin() -> PathBuf {
    support::consumer_executable()
}
