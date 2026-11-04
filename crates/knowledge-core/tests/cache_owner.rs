//! Cross-process owner lifetime. The forking helper is a separate single-threaded
//! public consumer. Readiness is a pipe or a stdout line, never a sleep.
//! A missed helper signal is an observer failure.

#[allow(dead_code)]
mod support {
    include!("cache/support.rs");
}

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::thread;
use std::time::Duration;
use support::{World, start_token};

const ADMIT_READY: Duration = Duration::from_secs(2);
const ADMIT_HANG: Duration = Duration::from_millis(500);
const PRODUCT_READY: Duration = Duration::from_secs(8);

#[test]
fn observer_admits_pass_fail_and_timeout() {
    let bin = helper_bin();
    let mut pass = spawn(&bin, &["admit-pass"]);
    let line = pass.next_line(ADMIT_READY);
    assert_eq!(line.expect("observer pass").trim(), "PASS");
    let status = pass.child.wait().expect("wait pass");
    assert!(status.success(), "admit-pass exit {status}");

    let mut fail = spawn(&bin, &["admit-fail"]);
    let line = fail.next_line(ADMIT_READY);
    assert_eq!(line.expect("observer fail").trim(), "FAIL");
    let status = fail.child.wait().expect("wait fail");
    assert_eq!(status.code(), Some(2), "admit-fail exit {status}");

    let mut hang = spawn(&bin, &["admit-hang"]);
    let timed = hang.next_line(ADMIT_HANG);
    assert!(timed.is_err(), "hang stand-in wrote {timed:?}");
    let observation = "timeout";
    hang.kill_exact();
    assert_eq!(observation, "timeout");
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
    let status = owner.child.wait().expect("owner wait");
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
    let exit_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !exited_or_absent(child_pid) {
        assert!(
            std::time::Instant::now() < exit_deadline,
            "child {child_token} still in userspace after destruction: {}",
            process_state(child_pid).unwrap_or('?')
        );
        thread::yield_now();
    }
    let while_held = competitor(&world);
    let primary = while_held.clone();
    let released_line = owner.command("RELEASE", PRODUCT_READY);
    let after = competitor(&world);
    owner.quit();
    let status = owner.child.wait().expect("owner wait");
    assert_eq!(
        primary,
        Competitor::Busy,
        "child destruction unlocked the parent; release line {released_line:?}; owner {status}"
    );
    assert_eq!(released_line.trim(), "RELEASED");
    assert_eq!(after, Competitor::Opened(hex_of(&derived)));
    world.lab.finish();
}

fn admit_or_fail() {
    let bin = helper_bin();
    let mut pass = spawn(&bin, &["admit-pass"]);
    let line = pass
        .next_line(ADMIT_READY)
        .expect("observer was not admitted");
    assert_eq!(line.trim(), "PASS");
    assert!(pass.child.wait().expect("wait").success());
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
    let status = child.child.wait().expect("competitor wait");
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
}

impl Owned {
    fn next_line(&mut self, timeout: Duration) -> Result<String, String> {
        match self.lines.recv_timeout(timeout) {
            Ok(line) => Ok(line),
            Err(mpsc::RecvTimeoutError::Timeout) => Err("observer timeout".to_string()),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err("observer closed".to_string()),
        }
    }

    fn command(&mut self, command: &str, timeout: Duration) -> String {
        writeln!(self.stdin, "{command}").expect("stdin");
        self.stdin.flush().expect("flush");
        self.next_line(timeout)
            .unwrap_or_else(|error| panic!("{error} waiting for {command}"))
    }

    fn quit(&mut self) {
        writeln!(self.stdin, "QUIT").expect("stdin");
        self.stdin.flush().expect("flush");
    }

    fn kill_exact(&mut self) {
        let current = start_token(self.child.id());
        if let Some(current) = current {
            assert_eq!(current, self.start, "refusing to signal a reused pid");
            self.child.kill().expect("kill");
        }
        let _ = self.child.wait();
    }
}

fn process_state(pid: u32) -> Option<char> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let end = stat.rfind(')')?;
    stat[end + 1..].split_whitespace().next()?.chars().next()
}

/// An unreaped child remains in `/proc` as `Z` after `_exit`. That is not userspace.
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
    }
}

fn helper_bin() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(compile_helper).clone()
}

fn compile_helper() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = manifest.join("tests/cache/public_consumer.rs");
    let support = manifest.join("tests/cache/support.rs");
    let exe = std::env::current_exe().expect("test exe");
    let deps = exe.parent().expect("deps").to_path_buf();
    let out = deps.parent().expect("target").join("cache-public-consumer");
    let newest = fs::metadata(&source)
        .and_then(|meta| meta.modified())
        .ok()
        .max(fs::metadata(&support).and_then(|meta| meta.modified()).ok());
    if let (Some(built), Some(source_time)) =
        (out.metadata().and_then(|m| m.modified()).ok(), newest)
        && built >= source_time
    {
        return out;
    }
    let rlib = newest_rlib(&deps);
    let output = Command::new("rustc")
        .arg("--edition=2024")
        .arg("--crate-type=bin")
        .arg("--crate-name=cache_public_consumer")
        .arg("-Dwarnings")
        .arg("-o")
        .arg(&out)
        .arg(&source)
        .arg("--extern")
        .arg(format!("knowledge_core={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .output()
        .expect("rustc");
    assert!(
        output.status.success(),
        "helper failed to compile\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    out
}

fn newest_rlib(deps: &Path) -> PathBuf {
    let mut found = fs::read_dir(deps)
        .expect("deps")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("libknowledge_core-") && name.ends_with(".rlib")
        })
        .collect::<Vec<_>>();
    found.sort_by_key(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok());
    found.pop().expect("knowledge_core rlib")
}
