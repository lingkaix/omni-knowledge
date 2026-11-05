//! Public product bounds for MCP frame acquisition and XLSX text assembly.
//!
//! Expectations come from the public CLI, the extraction API, and the processing
//! owner. Controller stand-ins admit the observer and the allocation counter.
//! They are not MCP behavior and they are not a product pass.
//! Unresolved child ownership keeps that fixture and does not start later product work.

mod fixture;

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Existing serve-mcp frame ceiling. Content longer than this is oversized.
const MCP_FRAME_LIMIT: usize = 4 * 1024 * 1024;
/// Modest bytes past the frame ceiling. This is not a stress size.
const FRAME_OVERSHOOT: usize = 64;
/// Caller-selected extraction budget, below the public 8 MiB default.
const CALLER_LIMIT: usize = 1024 * 1024;
/// One shared string, retained a single time in the admitted workbook.
const SHARED_LEN: usize = 64 * 1024;
/// Repeated references whose cloned text is the amplification under test.
const REFERENCE_COUNT: usize = 256;
/// Referenced text size before truncation: `SHARED_LEN * REFERENCE_COUNT`.
const REFERENCED_EXPANSION: usize = SHARED_LEN * REFERENCE_COUNT;
/// Prerecorded working-set factor. This is not a product memory cap.
const WORKING_FACTOR: usize = 4;
/// Three simultaneous copies of the referenced text must stay under this stop.
const HOST_COPY_STOP: usize = 96 * 1024 * 1024;
/// Instrument sizes. Positive is above the product observation ceiling.
const POSITIVE_ALLOC: usize = 8 * 1024 * 1024;
const NEGATIVE_ALLOC: usize = 4096;
const NEGATIVE_CEILING: usize = 1024 * 1024;
const DEALLOC_SLOP: usize = 64 * 1024;
const CAPTURE_CAP: usize = 1024 * 1024;
const CANARY: &[u8] = b"OMNI-CANARY-9f3c2a7b";
const MARKER: &str = "OMNIBOUND:";

const _: () = assert!(REFERENCED_EXPANSION > WORKING_FACTOR * CALLER_LIMIT);
const _: () = assert!(REFERENCED_EXPANSION * 3 < 128 * 1024 * 1024);
const _: () = assert!(REFERENCED_EXPANSION * 3 <= HOST_COPY_STOP);
const _: () = assert!(POSITIVE_ALLOC > WORKING_FACTOR * CALLER_LIMIT);
const _: () = assert!(MCP_FRAME_LIMIT + FRAME_OVERSHOOT > MCP_FRAME_LIMIT);

const SETTLE: Duration = Duration::from_millis(400);
const REFUSAL_WAIT: Duration = Duration::from_secs(5);
const FRAME_WAIT: Duration = Duration::from_secs(8);
const PROTOCOL_WAIT: Duration = Duration::from_secs(5);
const HANG_WAIT: Duration = Duration::from_millis(500);
const ADMISSION_WAIT: Duration = Duration::from_secs(2);
const INIT_WAIT: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(5);
const COLLECT_WAIT: Duration = Duration::from_secs(1);

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
#[cfg(target_os = "macos")]
const O_NONBLOCK: i32 = libc::O_NONBLOCK;
#[cfg(not(target_os = "macos"))]
const O_NONBLOCK: i32 = 0o4000;
const SIGKILL: i32 = 9;

unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
    fn close(fd: i32) -> i32;
}

static UNRESOLVED: Mutex<Vec<String>> = Mutex::new(Vec::new());

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct ObservedAllocator;

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let previous = CURRENT.fetch_add(size, Ordering::SeqCst);
        raise_peak(previous.saturating_add(size));
        let pointer = unsafe { System.alloc(layout) };
        if pointer.is_null() {
            CURRENT.fetch_sub(size, Ordering::SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static OBSERVED: ObservedAllocator = ObservedAllocator;

fn raise_peak(now: usize) {
    let mut peak = PEAK.load(Ordering::SeqCst);
    while now > peak {
        match PEAK.compare_exchange_weak(peak, now, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(seen) => peak = seen,
        }
    }
}

fn allocation_delta<T>(body: impl FnOnce() -> T) -> (T, usize) {
    let start = CURRENT.load(Ordering::SeqCst);
    PEAK.store(start, Ordering::SeqCst);
    let value = body();
    let peak = PEAK.load(Ordering::SeqCst);
    (value, peak.saturating_sub(start))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Exited(Option<i32>),
    Signaled(i32),
    StdoutClosedLive,
    Timeout,
    Response,
    ExitedDuringSettle(Option<i32>),
}

struct Watch {
    stop: Stop,
    natural: String,
    cleanup: String,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    bytes_written: usize,
    payload_len: usize,
    stdin_closed_by_us: bool,
    pid: u32,
    start_ticks: Option<u64>,
    cleanup_error: Option<String>,
    elapsed_ms: u128,
    same_process_after_cleanup: bool,
}

enum InputPlan {
    Hold,
    CloseImmediately,
    WriteHold(Vec<u8>),
    WriteClose(Vec<u8>),
}

enum Goal {
    Exit,
    ResponseOrExit,
}

struct Reap {
    child: Child,
    pid: u32,
    start_ticks: Option<u64>,
    done: bool,
    /// Withholds signalling and `try_wait`. Finish then reports unresolved ownership.
    /// This is an admission seam for a killable child, not an observed kernel wait failure.
    collector_status_withheld: bool,
}

impl Drop for Reap {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Err(error) = self.finish(true) {
            remember_unresolved(error);
        }
    }
}

impl Reap {
    fn arm(mut command: Command) -> Result<Self, String> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command
            .spawn()
            .map_err(|error| format!("spawn failed: {error}"))?;
        let pid = child.id();
        let start_ticks = start_ticks(pid);
        Ok(Self {
            child,
            pid,
            start_ticks,
            done: false,
            collector_status_withheld: false,
        })
    }

    /// Collects the owned child, or returns the pid and start token still owned.
    ///
    /// When collector status is withheld, this does not signal and does not wait.
    /// `done` stays false. That seam is not a kernel `try_wait` failure.
    fn finish(&mut self, kill: bool) -> Result<(), String> {
        if self.done {
            return Ok(());
        }
        if self.collector_status_withheld {
            return Err(self.unresolved("collector-status-withheld", None));
        }
        let mut kill_error = None;
        if kill {
            kill_error = group_kill(self.pid).err();
        }
        let deadline = Instant::now() + COLLECT_WAIT;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.done = true;
                    return Ok(());
                }
                Ok(None) if Instant::now() >= deadline => {
                    return Err(self.unresolved("still live", kill_error));
                }
                Ok(None) => {}
                Err(error) if Instant::now() >= deadline => {
                    return Err(self.unresolved(&error.to_string(), kill_error));
                }
                Err(_) => {}
            }
            std::thread::sleep(POLL);
        }
    }

    fn unresolved(&self, wait: &str, kill_error: Option<String>) -> String {
        format!(
            "unresolved pid {} start_ticks={:?} wait={wait} kill={}",
            self.pid,
            self.start_ticks,
            kill_error.unwrap_or_else(|| "none".to_string())
        )
    }
}

fn group_kill(pid: u32) -> Result<(), String> {
    let pid = i32::try_from(pid).map_err(|_| format!("pid {pid} is outside the kill range"))?;
    if pid <= 1 {
        return Err(format!("refusing to signal pid {pid}"));
    }
    let result = unsafe { kill(-pid, SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("group kill of {pid} returned {result}"))
    }
}

fn set_nonblocking(fd: i32) -> Result<(), String> {
    // fcntl IS variadic. In particular, Apple arm64 passes variadic arguments
    // differently from three fixed C parameters; a fixed-signature redeclaration
    // can appear successful while leaving a pipe blocking.
    let flags = unsafe { fcntl(fd, F_GETFL) };
    if flags < 0 {
        return Err(format!("fcntl read flags failed on {fd}"));
    }
    let updated = unsafe { fcntl(fd, F_SETFL, flags | O_NONBLOCK) };
    if updated < 0 {
        return Err(format!("fcntl nonblocking failed on {fd}"));
    }
    let actual = unsafe { fcntl(fd, F_GETFL) };
    if actual < 0 || actual & O_NONBLOCK == 0 {
        return Err(format!("native nonblocking flags did not stick on {fd}"));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn start_ticks(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = text.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// Independent native birth observation for the controller, not the product's RSS/collector.
/// The field retains its legacy name: Linux ticks, Darwin checked start microseconds.
#[cfg(target_os = "macos")]
fn start_ticks(pid: u32) -> Option<u64> {
    let native_pid = i32::try_from(pid).ok().filter(|p| *p > 1)?;
    // SAFETY: exact public libproc ABI and fixed owned buffer; no pointer escapes this call.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let n = unsafe {
        libc::proc_pidinfo(
            native_pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            std::mem::size_of_val(&info) as i32,
        )
    };
    if n as usize != std::mem::size_of_val(&info)
        || info.pbi_pid != pid
        || info.pbi_start_tvsec == 0
        || info.pbi_start_tvusec >= 1_000_000
    {
        return None;
    }
    info.pbi_start_tvsec
        .checked_mul(1_000_000)?
        .checked_add(info.pbi_start_tvusec)
}

fn same_process(pid: u32, ticks: Option<u64>) -> bool {
    match (ticks, start_ticks(pid)) {
        (Some(original), Some(now)) => original == now,
        (None, Some(_)) => true,
        (_, None) => false,
    }
}

fn read_available(source: &mut Option<impl Read>, sink: &mut Vec<u8>, eof: &mut bool) {
    let Some(file) = source.as_mut() else {
        return;
    };
    if *eof || sink.len() >= CAPTURE_CAP {
        return;
    }
    let mut buffer = [0u8; 8192];
    loop {
        if sink.len() >= CAPTURE_CAP {
            break;
        }
        let room = (CAPTURE_CAP - sink.len()).min(buffer.len());
        match file.read(&mut buffer[..room]) {
            Ok(0) => {
                *eof = true;
                break;
            }
            Ok(size) => sink.extend_from_slice(&buffer[..size]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
}

fn write_available(
    destination: &mut Option<impl Write>,
    payload: &[u8],
    written: &mut usize,
) -> bool {
    let Some(file) = destination.as_mut() else {
        return false;
    };
    if *written >= payload.len() {
        return false;
    }
    match file.write(&payload[*written..]) {
        Ok(0) => true,
        Ok(size) => {
            *written += size;
            false
        }
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => false,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(_) => true,
    }
}

fn watch(
    command: Command,
    plan: InputPlan,
    goal: Goal,
    settle: Duration,
    wait: Duration,
) -> Result<Watch, String> {
    let mut reap = Reap::arm(command)?;
    if let Err(error) = prepare_stdio(&mut reap) {
        return setup_failure(reap, error);
    }
    let pid = reap.pid;
    let start_ticks = reap.start_ticks;
    let started = Instant::now();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdout_eof = false;
    let mut written = 0usize;
    let mut stdin_closed_by_us = false;
    let mut write_finished = false;
    let payload = match &plan {
        InputPlan::WriteHold(bytes) | InputPlan::WriteClose(bytes) => bytes.clone(),
        InputPlan::Hold | InputPlan::CloseImmediately => Vec::new(),
    };
    let payload_len = payload.len();
    let mut stop = None;
    let settle_deadline = started + settle;
    let wait_origin = settle_deadline;
    let mut stdout_closed_live = false;
    while stop.is_none() {
        if !reap.done {
            match reap.child.try_wait() {
                Ok(Some(status)) => {
                    reap.done = true;
                    let during_settle =
                        !settle.is_zero() && Instant::now() < settle_deadline && written == 0;
                    stop = Some(stop_from_status(status, during_settle));
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
        read_available(&mut reap.child.stdout, &mut stdout, &mut stdout_eof);
        let mut stderr_eof = false;
        read_available(&mut reap.child.stderr, &mut stderr, &mut stderr_eof);
        let settled = Instant::now() >= settle_deadline;
        if settled && !stdin_closed_by_us && !write_finished {
            match &plan {
                InputPlan::CloseImmediately => {
                    drop(reap.child.stdin.take());
                    stdin_closed_by_us = true;
                    write_finished = true;
                }
                InputPlan::Hold => write_finished = true,
                InputPlan::WriteHold(_) | InputPlan::WriteClose(_) => {
                    let failed = write_available(&mut reap.child.stdin, &payload, &mut written);
                    if failed || written >= payload.len() {
                        write_finished = true;
                        if matches!(plan, InputPlan::WriteClose(_)) {
                            drop(reap.child.stdin.take());
                            stdin_closed_by_us = true;
                        }
                    }
                }
            }
        }
        if stdout_eof && !reap.done {
            stdout_closed_live = true;
        }
        if matches!(goal, Goal::ResponseOrExit) && stop.is_none() && response_ready(&stdout) {
            stop = Some(Stop::Response);
        }
        if stop.is_none()
            && settled
            && Instant::now().saturating_duration_since(wait_origin) >= wait
        {
            stop = Some(if stdout_closed_live && !reap.done {
                Stop::StdoutClosedLive
            } else {
                Stop::Timeout
            });
        }
        if stop.is_none() {
            std::thread::sleep(POLL);
        }
    }
    let stop = stop.unwrap_or(Stop::Timeout);
    let natural = natural_label(stop);
    let kill = !reap.done;
    let cleanup = if reap.done {
        "already-collected"
    } else if matches!(stop, Stop::Response) {
        "killed-after-response"
    } else {
        "killed-for-containment"
    };
    let cleanup_error = reap.finish(kill).err();
    let cleanup = if cleanup_error.is_some() {
        "unresolved"
    } else {
        cleanup
    };
    let same = same_process(pid, start_ticks);
    Ok(Watch {
        stop,
        natural,
        cleanup: cleanup.to_string(),
        stdout,
        stderr,
        bytes_written: written,
        payload_len,
        stdin_closed_by_us,
        pid,
        start_ticks,
        cleanup_error,
        elapsed_ms: started.elapsed().as_millis(),
        same_process_after_cleanup: same,
    })
}

fn prepare_stdio(reap: &mut Reap) -> Result<(), String> {
    let stdout_fd = reap
        .child
        .stdout
        .as_ref()
        .ok_or("missing stdout")?
        .as_raw_fd();
    let stderr_fd = reap
        .child
        .stderr
        .as_ref()
        .ok_or("missing stderr")?
        .as_raw_fd();
    let stdin_fd = reap
        .child
        .stdin
        .as_ref()
        .ok_or("missing stdin")?
        .as_raw_fd();
    set_nonblocking(stdout_fd)?;
    set_nonblocking(stderr_fd)?;
    set_nonblocking(stdin_fd)?;
    Ok(())
}

fn setup_failure(mut reap: Reap, error: String) -> Result<Watch, String> {
    let pid = reap.pid;
    let ticks = reap.start_ticks;
    let cleanup = reap.finish(true);
    let action = if cleanup.is_ok() {
        "killed-for-containment"
    } else {
        "unresolved"
    };
    println!(
        "OBSERVE setup_failure stop=setup natural=none cleanup={action} pid={pid} start_ticks={ticks:?}"
    );
    let primary = format!("setup failed: {error} pid={pid} start_ticks={ticks:?} cleanup={action}");
    match cleanup {
        Ok(()) => Err(primary),
        Err(cleanup) => Err(format!("{primary}; cleanup unresolved: {cleanup}")),
    }
}

fn stop_from_status(status: ExitStatus, during_settle: bool) -> Stop {
    if let Some(signal) = status.signal() {
        return Stop::Signaled(signal);
    }
    let code = status.code();
    if during_settle {
        Stop::ExitedDuringSettle(code)
    } else {
        Stop::Exited(code)
    }
}

fn natural_label(stop: Stop) -> String {
    match stop {
        Stop::Exited(Some(code)) => format!("exit({code})"),
        Stop::Exited(None) => "exit(unknown)".to_string(),
        Stop::Signaled(signal) => format!("signaled({signal})"),
        Stop::StdoutClosedLive => "stdout-closed-live".to_string(),
        Stop::Timeout => "timeout".to_string(),
        Stop::Response => "response".to_string(),
        Stop::ExitedDuringSettle(Some(code)) => format!("settle-exit({code})"),
        Stop::ExitedDuringSettle(None) => "settle-exit(unknown)".to_string(),
    }
}

fn response_ready(stdout: &[u8]) -> bool {
    stdout.split(|byte| *byte == b'\n').any(|line| {
        contains_slice(line, b"\"jsonrpc\"")
            && (contains_slice(line, b"\"result\"") || contains_slice(line, b"\"error\""))
    })
}

struct Outcome {
    kind: &'static str,
    name: &'static str,
    state: &'static str,
    detail: String,
}

fn record(
    cases: &mut Vec<Outcome>,
    kind: &'static str,
    name: &'static str,
    result: Result<(), String>,
) {
    let (state, detail) = match result {
        Ok(()) => ("PASS", String::new()),
        Err(detail) => ("FAIL", detail),
    };
    println!("CASE {kind} {name} {state} {detail}");
    cases.push(Outcome {
        kind,
        name,
        state,
        detail,
    });
}

fn record_not_run(cases: &mut Vec<Outcome>, name: &'static str, detail: String) {
    println!("CASE product {name} NOT_RUN {detail}");
    cases.push(Outcome {
        kind: "product",
        name,
        state: "NOT_RUN",
        detail,
    });
}

fn contains_slice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn contains_run(haystack: &[u8], byte: u8, run: usize) -> bool {
    haystack
        .windows(run)
        .any(|window| window.iter().all(|item| *item == byte))
}

fn has_result_for_id(buffer: &[u8], id: i64) -> bool {
    protocol_line(buffer, id, b"\"result\"")
}

fn has_error_for_id(buffer: &[u8], id: i64) -> bool {
    protocol_line(buffer, id, b"\"error\"")
}

fn has_jsonrpc_error(buffer: &[u8]) -> bool {
    buffer
        .split(|byte| *byte == b'\n')
        .any(|line| contains_slice(line, b"\"jsonrpc\"") && contains_slice(line, b"\"error\""))
}

fn protocol_line(buffer: &[u8], id: i64, kind: &[u8]) -> bool {
    let exact = format!("\"id\":{id}").into_bytes();
    let spaced = format!("\"id\": {id}").into_bytes();
    buffer.split(|byte| *byte == b'\n').any(|line| {
        let has_id = contains_slice(line, &exact) || contains_slice(line, &spaced);
        has_id && contains_slice(line, kind) && contains_slice(line, b"\"jsonrpc\"")
    })
}

fn has_panic(buffer: &[u8]) -> bool {
    contains_slice(buffer, b"panicked at")
}

fn one_line(bytes: &[u8]) -> String {
    let end = bytes.len().min(180);
    String::from_utf8_lossy(&bytes[..end])
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn watch_detail(watch: &Watch) -> String {
    format!(
        "stop={:?} natural={} cleanup={} pid={} start_ticks={:?} written={}/{} stdin_closed={} elapsed_ms={} cleanup_error={} same_pid_after={} stdout={} stderr={}",
        watch.stop,
        watch.natural,
        watch.cleanup,
        watch.pid,
        watch.start_ticks,
        watch.bytes_written,
        watch.payload_len,
        watch.stdin_closed_by_us,
        watch.elapsed_ms,
        watch.cleanup_error.as_deref().unwrap_or("none"),
        watch.same_process_after_cleanup,
        one_line(&watch.stdout),
        one_line(&watch.stderr),
    )
}

fn observe(name: &str, watch: &Watch) {
    println!(
        "OBSERVE {name} stop={:?} natural={} cleanup={} pid={} start_ticks={:?} written={}/{} stdin_closed={} elapsed_ms={} same_pid_after={}",
        watch.stop,
        watch.natural,
        watch.cleanup,
        watch.pid,
        watch.start_ticks,
        watch.bytes_written,
        watch.payload_len,
        watch.stdin_closed_by_us,
        watch.elapsed_ms,
        watch.same_process_after_cleanup,
    );
}

fn finish_case(primary: Result<(), String>, cleanup: Result<(), String>) -> Result<(), String> {
    match (primary, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(cleanup)) => Err(format!("cleanup separate from primary pass: {cleanup}")),
        (Err(primary), Err(cleanup)) => Err(format!("primary {primary}; cleanup {cleanup}")),
    }
}

fn require_cleanup(watch: &Watch) -> Result<(), String> {
    if let Some(error) = &watch.cleanup_error {
        return Err(format!("cleanup failed: {error}; {}", watch_detail(watch)));
    }
    if watch.same_process_after_cleanup {
        return Err(format!(
            "child still present after cleanup; {}",
            watch_detail(watch)
        ));
    }
    Ok(())
}

fn admit_crc() -> Result<(), String> {
    let actual = fixture::crc32(b"123456789");
    if actual == fixture::CRC32_VECTOR {
        Ok(())
    } else {
        Err(format!(
            "crc32 vector {actual:#x} != {:#x}",
            fixture::CRC32_VECTOR
        ))
    }
}

fn admit_refuse() -> Result<(), String> {
    let watch = watch(
        standin("refuse"),
        InputPlan::Hold,
        Goal::Exit,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    observe("observer_refuse", &watch);
    let primary = match watch.stop {
        Stop::Exited(Some(2))
            if !watch.stdin_closed_by_us
                && watch.cleanup == "already-collected"
                && contains_slice(&watch.stdout, b"refused")
                && clean_refusal(&watch, b"unused-canary", 1) =>
        {
            Ok(())
        }
        _ => Err(format!(
            "natural refusal misclassified; {}",
            watch_detail(&watch)
        )),
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_echo() -> Result<(), String> {
    let watch = watch(
        standin("echo"),
        InputPlan::WriteHold(CANARY.to_vec()),
        Goal::Exit,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    observe("observer_echo", &watch);
    let saw_canary = contains_slice(&watch.stdout, CANARY);
    let saw_result = has_result_for_id(&watch.stdout, 1);
    let clean = clean_refusal(&watch, CANARY, 1);
    let primary = if saw_canary && saw_result && !clean {
        Ok(())
    } else {
        Err(format!(
            "echo detector canary={saw_canary} result={saw_result} clean={clean}; {}",
            watch_detail(&watch)
        ))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_early_eof() -> Result<(), String> {
    let watch = watch(
        standin("early-eof"),
        InputPlan::CloseImmediately,
        Goal::Exit,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    observe("observer_early_eof", &watch);
    let primary = if watch.stdin_closed_by_us
        && matches!(watch.stop, Stop::Exited(Some(0)))
        && watch.cleanup == "already-collected"
        && contains_slice(&watch.stdout, b"ended-on-eof")
    {
        Ok(())
    } else {
        Err(format!("early EOF misclassified; {}", watch_detail(&watch)))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_hang() -> Result<(), String> {
    let watch = watch(
        standin("hang"),
        InputPlan::Hold,
        Goal::Exit,
        Duration::ZERO,
        HANG_WAIT,
    )?;
    observe("observer_hang", &watch);
    let primary = if matches!(watch.stop, Stop::Timeout)
        && watch.cleanup == "killed-for-containment"
        && !watch.stdin_closed_by_us
        && !clean_refusal(&watch, b"unused-canary", 1)
    {
        Ok(())
    } else {
        Err(format!("hang was not a timeout; {}", watch_detail(&watch)))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_stdout_live() -> Result<(), String> {
    let watch = watch(
        standin("stdout-live"),
        InputPlan::Hold,
        Goal::Exit,
        Duration::ZERO,
        HANG_WAIT,
    )?;
    observe("observer_stdout_live", &watch);
    let primary = if matches!(watch.stop, Stop::StdoutClosedLive)
        && watch.cleanup == "killed-for-containment"
        && !clean_refusal(&watch, b"unused-canary", 1)
    {
        Ok(())
    } else {
        Err(format!(
            "stdout eof while live counted as refusal; {}",
            watch_detail(&watch)
        ))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_drain_hang() -> Result<(), String> {
    let watch = watch(
        standin("drain-hang"),
        InputPlan::WriteHold(b"drain-check\n".to_vec()),
        Goal::ResponseOrExit,
        Duration::ZERO,
        HANG_WAIT,
    )?;
    observe("observer_drain_hang", &watch);
    let rejected = terminated_outcome(&watch, Terminated::Refuse, None).is_err();
    let primary = if rejected {
        Ok(())
    } else {
        Err(format!(
            "drain without a response counted as a terminated outcome; {}",
            watch_detail(&watch)
        ))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_signal() -> Result<(), String> {
    let watch = watch(
        standin("signal"),
        InputPlan::Hold,
        Goal::Exit,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    observe("observer_signal", &watch);
    let primary = if matches!(watch.stop, Stop::Signaled(_))
        && watch.cleanup == "already-collected"
        && !clean_refusal(&watch, b"unused-canary", 1)
    {
        Ok(())
    } else {
        Err(format!(
            "signal death counted as ordinary refusal; {}",
            watch_detail(&watch)
        ))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_success() -> Result<(), String> {
    let watch = watch(
        standin("success"),
        InputPlan::Hold,
        Goal::ResponseOrExit,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    observe("observer_success", &watch);
    let primary = if matches!(watch.stop, Stop::Response)
        && has_result_for_id(&watch.stdout, 21)
        && watch.cleanup == "killed-after-response"
        && !clean_refusal(&watch, b"unused-canary", 21)
    {
        Ok(())
    } else {
        Err(format!(
            "persistent success was not separated from cleanup; {}",
            watch_detail(&watch)
        ))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn admit_ownership() -> Result<(), String> {
    let mut reap = Reap::arm(standin("hang"))?;
    let pid = reap.pid;
    let ticks = reap.start_ticks;
    let cleanup = reap.finish(true);
    let action = if cleanup.is_ok() {
        "killed-for-containment"
    } else {
        "unresolved"
    };
    let alive = same_process(pid, ticks);
    println!(
        "OBSERVE observer_guard_settled stop=armed natural=none cleanup={action} pid={pid} start_ticks={ticks:?} same_pid_after={alive}"
    );
    if alive {
        return Err(format!("armed child remained after settlement pid={pid}"));
    }
    if let Err(error) = cleanup {
        return Err(format!("armed child cleanup unresolved: {error}"));
    }
    let pid_slot = AtomicU32::new(0);
    let tick_slot = AtomicU64::new(0);
    let saw_ticks = AtomicBool::new(false);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = catch_unwind(AssertUnwindSafe(|| {
        let reap = Reap::arm(standin("hang")).expect("spawn unwind stand-in");
        pid_slot.store(reap.pid, Ordering::SeqCst);
        if let Some(ticks) = reap.start_ticks {
            tick_slot.store(ticks, Ordering::SeqCst);
            saw_ticks.store(true, Ordering::SeqCst);
        }
        panic!("representative post-spawn unwind");
    }));
    std::panic::set_hook(previous);
    let pid = pid_slot.load(Ordering::SeqCst);
    let ticks = saw_ticks
        .load(Ordering::SeqCst)
        .then_some(tick_slot.load(Ordering::SeqCst));
    let alive = pid > 1 && same_process(pid, ticks);
    let unresolved = unresolved_note();
    let action = if unresolved.is_empty() {
        "dropped-containment"
    } else {
        "unresolved"
    };
    println!(
        "OBSERVE observer_unwind stop=unwind natural=none cleanup={action} pid={pid} start_ticks={ticks:?} same_pid_after={alive}"
    );
    if caught.is_ok() {
        return Err("unwind admission did not panic".to_string());
    }
    if pid <= 1 {
        return Err("unwind admission did not own a child".to_string());
    }
    if alive {
        return Err(format!("unwind left child pid={pid}"));
    }
    if !unresolved.is_empty() {
        return Err(format!("unwind cleanup unresolved: {unresolved}"));
    }
    Ok(())
}

/// Admits the shared spawn and guard when collector status stays unresolved.
///
/// The child mode is the hang stand-in. Status is withheld on purpose, so this
/// does not observe a kernel wait failure. Later product names are `NOT_RUN`,
/// the fixture stays, and the same child is collected before its directory is removed.
fn admit_unresolved_fixture() -> Result<(), String> {
    let directory = std::env::temp_dir().join(format!(
        "omni-product-boundaries-unresolved-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("unresolved fixture directory: {error}"))?;
    let marker = directory.join("owned-marker");
    if let Err(error) = std::fs::write(&marker, b"owned") {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(format!("unresolved fixture marker: {error}"));
    }
    let mut reap = match Reap::arm(standin("hang")) {
        Ok(reap) => reap,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&directory);
            return Err(error);
        }
    };
    let pid = reap.pid;
    let ticks = reap.start_ticks;
    reap.collector_status_withheld = true;
    let mut pushed = None;
    let disposition = observe_unresolved_disposition(&mut reap, &directory, &marker, &mut pushed);
    reap.collector_status_withheld = false;
    let _ = reap.finish(true);
    drop(reap);
    let alive = same_process(pid, ticks);
    if !alive && let Some(obligation) = &pushed {
        forget_unresolved(obligation);
    }
    if alive || !unresolved_note().is_empty() {
        println!(
            "CLEANUP fixture_retained path={} pid={pid} start_ticks={ticks:?} obligation={}",
            directory.display(),
            unresolved_note()
        );
        return Err(format!(
            "admission child pid={pid} start_ticks={ticks:?} stayed unresolved alive={alive}; {disposition:?}"
        ));
    }
    if let Err(error) = disposition {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(error);
    }
    std::fs::remove_dir_all(&directory).map_err(|error| {
        format!(
            "remove collected unresolved fixture {}: {error}",
            directory.display()
        )
    })?;
    println!(
        "OBSERVE unresolved_fixture_collected seam=collector-status-withheld kernel_wait=not-observed mode=hang pid={pid} start_ticks={ticks:?} same_pid_after=false path={} fixture_removed=true",
        directory.display()
    );
    Ok(())
}

/// Runs one withheld-status pass and the fixture stop used by product cases.
fn observe_unresolved_disposition(
    reap: &mut Reap,
    directory: &Path,
    marker: &Path,
    pushed: &mut Option<String>,
) -> Result<(), String> {
    let pid = reap.pid;
    let ticks = reap
        .start_ticks
        .ok_or_else(|| format!("pid {pid} has no start token"))?;
    let obligation = reap
        .finish(true)
        .err()
        .ok_or_else(|| "withheld collector status was treated as collected".to_string())?;
    if !obligation.contains("collector-status-withheld") {
        return Err(format!("collector seam lost its label: {obligation}"));
    }
    if !same_process(pid, Some(ticks)) {
        return Err(format!(
            "withheld status did not keep owned pid={pid} start_ticks={ticks}"
        ));
    }
    remember_unresolved(obligation.clone());
    *pushed = Some(obligation);
    let mut later = Vec::new();
    if !stop_unresolved_fixture(&mut later, directory) {
        return Err("unresolved ownership did not stop later fixture work".to_string());
    }
    let stopped = later.len() == B2_OBSERVATIONS.len() + B1_OBSERVATIONS.len()
        && later.iter().all(|case| case.state == "NOT_RUN");
    if !stopped {
        return Err(format!(
            "later fixture work was not all NOT_RUN ({})",
            later.len()
        ));
    }
    if !marker.is_file() || !directory.is_dir() {
        return Err(format!(
            "fixture {} was removed while pid={pid} remained unresolved",
            directory.display()
        ));
    }
    println!(
        "OBSERVE unresolved_fixture seam=collector-status-withheld kernel_wait=not-observed mode=hang stop=unresolved natural=none cleanup=unresolved pid={pid} start_ticks={ticks} same_pid_after=true path={} later_not_run={} fixture_retained=true",
        directory.display(),
        later.len()
    );
    Ok(())
}

fn unresolved_note() -> String {
    UNRESOLVED
        .lock()
        .map(|note| note.join(";"))
        .unwrap_or_else(|poisoned| poisoned.into_inner().join(";"))
}

/// Records one owned pid and start token whose collection is still unconfirmed.
fn remember_unresolved(error: String) {
    if let Ok(mut unresolved) = UNRESOLVED.lock() {
        unresolved.push(error);
    }
}

/// Drops one obligation after that owned child has actually been collected.
fn forget_unresolved(obligation: &str) {
    if let Ok(mut unresolved) = UNRESOLVED.lock() {
        unresolved.retain(|note| note != obligation);
    }
}

fn clean_refusal(watch: &Watch, canary: &[u8], id: i64) -> bool {
    if watch.stdin_closed_by_us || has_panic(&watch.stderr) || has_panic(&watch.stdout) {
        return false;
    }
    if contains_slice(&watch.stdout, canary) || contains_slice(&watch.stderr, canary) {
        return false;
    }
    if has_result_for_id(&watch.stdout, id) {
        return false;
    }
    match watch.stop {
        Stop::Exited(Some(_)) => true,
        Stop::Response => has_jsonrpc_error(&watch.stdout),
        _ => false,
    }
}

fn admit_allocator() -> Result<(), String> {
    let negative_start = CURRENT.load(Ordering::SeqCst);
    PEAK.store(negative_start, Ordering::SeqCst);
    let negative_held = vec![0u8; NEGATIVE_ALLOC];
    std::hint::black_box(negative_held.as_slice());
    let negative_delta = PEAK.load(Ordering::SeqCst).saturating_sub(negative_start);
    drop(negative_held);
    let negative_after = CURRENT.load(Ordering::SeqCst);
    if !(NEGATIVE_ALLOC..NEGATIVE_CEILING).contains(&negative_delta) {
        return Err(format!("negative allocation delta {negative_delta}"));
    }
    if negative_after > negative_start.saturating_add(DEALLOC_SLOP) {
        return Err(format!(
            "negative dealloc left {negative_after} from {negative_start}"
        ));
    }
    let positive_start = CURRENT.load(Ordering::SeqCst);
    PEAK.store(positive_start, Ordering::SeqCst);
    let positive_held = vec![0u8; POSITIVE_ALLOC];
    std::hint::black_box(positive_held.as_slice());
    let positive_delta = PEAK.load(Ordering::SeqCst).saturating_sub(positive_start);
    drop(positive_held);
    let positive_after = CURRENT.load(Ordering::SeqCst);
    if positive_delta < POSITIVE_ALLOC || positive_delta <= negative_delta {
        return Err(format!(
            "positive allocation delta {positive_delta} negative {negative_delta}"
        ));
    }
    if positive_after > positive_start.saturating_add(DEALLOC_SLOP) {
        return Err(format!(
            "positive dealloc left {positive_after} from {positive_start}"
        ));
    }
    Ok(())
}

fn standin(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("current test executable"));
    command
        .arg("--exact")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .arg("product_boundaries")
        .env("OMNI_PRODUCT_BOUND_STANDIN", mode)
        .process_group(0);
    command
}

fn omni(root: &Path, current: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omni"));
    command
        .current_dir(current)
        .arg("--root")
        .arg(root)
        .process_group(0);
    command
}

fn at_bound_frame() -> Result<Vec<u8>, String> {
    let prefix = "{\"jsonrpc\":\"2.0\",\"id\":15,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"b1\",\"version\":\"0\",\"note\":\"";
    let suffix = "\"}}}";
    let pad = MCP_FRAME_LIMIT
        .checked_sub(prefix.len() + suffix.len())
        .ok_or("initialize prefix exceeds the 4 MiB ceiling")?;
    let mut bytes = prefix.as_bytes().to_vec();
    bytes.extend(std::iter::repeat_n(b'P', pad));
    bytes.extend_from_slice(suffix.as_bytes());
    if bytes.len() != MCP_FRAME_LIMIT || bytes.contains(&b'\n') || !json_text_balanced(&bytes) {
        return Err(format!(
            "at-bound initialize is not a {} byte JSON value",
            MCP_FRAME_LIMIT
        ));
    }
    bytes.push(b'\n');
    Ok(bytes)
}

fn json_text_balanced(bytes: &[u8]) -> bool {
    let mut braces = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for &byte in bytes {
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => braces += 1,
            b'}' => {
                braces -= 1;
                if braces < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    braces == 0 && !in_string && !escape
}

fn admit_at_bound_fixture() -> Result<(), String> {
    let bytes = at_bound_frame()?;
    let content = &bytes[..bytes.len() - 1];
    if content.len() == MCP_FRAME_LIMIT
        && bytes.ends_with(b"\n")
        && json_text_balanced(content)
        && content.starts_with(b"{\"jsonrpc\":\"2.0\",\"id\":15,")
    {
        Ok(())
    } else {
        Err(format!("at-bound fixture length {}", content.len()))
    }
}

fn initialize_bytes(id: i64, ending: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{}},\"clientInfo\":{{\"name\":\"b1\",\"version\":\"0\"}}}}}}"
    )
    .into_bytes();
    bytes.extend_from_slice(ending);
    bytes
}

fn oversized_unterminated() -> Vec<u8> {
    let mut bytes = initialize_bytes(14, b"");
    bytes.extend(std::iter::repeat_n(b'Z', 8192));
    bytes.extend_from_slice(CANARY);
    let target = MCP_FRAME_LIMIT + FRAME_OVERSHOOT;
    if bytes.len() < target {
        bytes.extend(std::iter::repeat_n(b'Z', target - bytes.len()));
    }
    assert!(!bytes.contains(&b'\n'));
    assert!(bytes.len() == target);
    bytes
}

fn repeated(byte: u8, len: usize) -> Vec<u8> {
    let mut bytes = vec![byte; len];
    bytes.push(b'\n');
    bytes
}

fn run_admission() -> Result<(), String> {
    let mut cases = Vec::new();
    record(&mut cases, "controller", "crc32_vector", admit_crc());
    record(&mut cases, "controller", "observer_refuse", admit_refuse());
    record(&mut cases, "controller", "observer_echo", admit_echo());
    record(
        &mut cases,
        "controller",
        "observer_early_eof",
        admit_early_eof(),
    );
    record(&mut cases, "controller", "observer_hang", admit_hang());
    record(
        &mut cases,
        "controller",
        "observer_stdout_live",
        admit_stdout_live(),
    );
    record(
        &mut cases,
        "controller",
        "observer_drain_hang",
        admit_drain_hang(),
    );
    record(&mut cases, "controller", "observer_signal", admit_signal());
    record(
        &mut cases,
        "controller",
        "observer_success",
        admit_success(),
    );
    record(
        &mut cases,
        "controller",
        "observer_ownership",
        admit_ownership(),
    );
    record(
        &mut cases,
        "controller",
        "observer_unresolved_fixture",
        admit_unresolved_fixture(),
    );
    record(
        &mut cases,
        "controller",
        "at_bound_fixture",
        admit_at_bound_fixture(),
    );
    record(
        &mut cases,
        "controller",
        "allocator_windows",
        admit_allocator(),
    );
    let failed: Vec<_> = cases
        .iter()
        .filter(|case| case.state != "PASS")
        .map(|case| case.name)
        .collect();
    println!(
        "SUMMARY controller_pass={} controller_fail={}",
        cases.len() - failed.len(),
        failed.len()
    );
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join(","))
    }
}

fn ordinary_workbook() -> (Vec<u8>, String, String) {
    let first = "Alpha".to_string();
    let second = "Betaβ".to_string();
    let bytes =
        fixture::shared_string_workbook(&[first.clone(), second.clone()], &[(0, 0), (1, 1)]);
    (bytes, first, second)
}

fn amplifying_workbook() -> (Vec<u8>, String) {
    let mut shared = String::with_capacity(SHARED_LEN);
    shared.push_str(MARKER);
    while shared.len() < SHARED_LEN {
        shared.push('Q');
    }
    assert_eq!(shared.len(), SHARED_LEN);
    let rows: Vec<(u32, usize)> = (0..u32::try_from(REFERENCE_COUNT).expect("refs"))
        .map(|row| (row, 0))
        .collect();
    let bytes = fixture::shared_string_workbook(std::slice::from_ref(&shared), &rows);
    (bytes, shared)
}

fn working_ceiling(input_len: usize) -> usize {
    CALLER_LIMIT
        .saturating_mul(WORKING_FACTOR)
        .saturating_add(input_len)
}

fn write_fixture(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

fn b2_ordinary(directory: &Path) -> Result<(), String> {
    let (bytes, first, second) = ordinary_workbook();
    if !bytes.starts_with(b"PK\x03\x04") || !contains_slice(&bytes, first.as_bytes()) {
        return Err("ordinary fixture is not a stored workbook".to_string());
    }
    if bytes.len() > CALLER_LIMIT {
        return Err(format!(
            "ordinary input {} exceeds caller limit {CALLER_LIMIT}",
            bytes.len()
        ));
    }
    let path = write_fixture(directory, "ordinary.xlsx", &bytes);
    let extraction = omni_knowledge::extract::extract(&path, CALLER_LIMIT).map_err(|error| {
        format!(
            "ordinary extract error {}",
            one_line(format!("{error}").as_bytes())
        )
    })?;
    if extraction.status != "extracted" {
        return Err(format!(
            "ordinary status {} warnings {}",
            extraction.status,
            extraction.warnings.len()
        ));
    }
    if extraction.text.len() > CALLER_LIMIT {
        return Err(format!(
            "ordinary text {} exceeds {CALLER_LIMIT}",
            extraction.text.len()
        ));
    }
    match (extraction.text.find(&first), extraction.text.find(&second)) {
        (Some(left), Some(right)) if left < right => Ok(()),
        _ => Err(format!(
            "ordinary cell text missing in order {}",
            one_line(extraction.text.as_bytes())
        )),
    }
}

fn b2_amplification(directory: &Path) -> Result<(), String> {
    let (bytes, shared) = amplifying_workbook();
    let input_len = bytes.len();
    let ceiling = working_ceiling(input_len);
    println!(
        "PRERECORD b2 input_len={input_len} referenced_expansion={REFERENCED_EXPANSION} ceiling={ceiling} caller_limit={CALLER_LIMIT} factor={WORKING_FACTOR}"
    );
    if !bytes.starts_with(b"PK\x03\x04") || !contains_slice(&bytes, MARKER.as_bytes()) {
        return Err("amplifying fixture is not a stored workbook".to_string());
    }
    if input_len > CALLER_LIMIT {
        return Err(format!(
            "amplifying input {input_len} is not admitted by {CALLER_LIMIT}"
        ));
    }
    if REFERENCED_EXPANSION <= ceiling || REFERENCED_EXPANSION > HOST_COPY_STOP {
        return Err(format!(
            "fixture expansion {REFERENCED_EXPANSION} is outside the prerecorded window"
        ));
    }
    let path = write_fixture(directory, "amplify.xlsx", &bytes);
    let (extraction, peak) =
        allocation_delta(|| omni_knowledge::extract::extract(&path, CALLER_LIMIT));
    let extraction = extraction.map_err(|error| {
        format!(
            "amplifying extract error peak={peak} ceiling={ceiling} {}",
            one_line(format!("{error}").as_bytes())
        )
    })?;
    if extraction.status == "metadata-only" || extraction.status == "failed" {
        return Err(format!(
            "amplifying status {} is not an admitted parse peak={peak}",
            extraction.status
        ));
    }
    let mut problems = Vec::new();
    if extraction.text.len() > CALLER_LIMIT {
        problems.push(format!(
            "returned text {} exceeds caller limit",
            extraction.text.len()
        ));
    }
    let cell_prefix = &shared[..MARKER.len() + 16];
    if !extraction.text.contains(cell_prefix) {
        problems.push(format!(
            "computed cell prefix absent {}",
            one_line(extraction.text.as_bytes())
        ));
    }
    if peak > ceiling {
        problems.push(format!(
            "working allocation {peak} exceeds prerecorded ceiling {ceiling}"
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} returned_len={}",
            problems.join("; "),
            extraction.text.len()
        ))
    }
}

fn serve(
    root: &Path,
    current: &Path,
    plan: InputPlan,
    goal: Goal,
    wait: Duration,
) -> Result<Watch, String> {
    let mut command = omni(root, current);
    command.arg("serve-mcp");
    watch(command, plan, goal, SETTLE, wait)
}

fn require_success(watch: &Watch, id: i64) -> Result<(), String> {
    if has_panic(&watch.stderr) || has_panic(&watch.stdout) {
        return Err(format!("panic; {}", watch_detail(watch)));
    }
    if !has_result_for_id(&watch.stdout, id) {
        return Err(format!(
            "missing initialize result; {}",
            watch_detail(watch)
        ));
    }
    if !matches!(watch.stop, Stop::Response | Stop::Exited(Some(_))) {
        return Err(format!(
            "no natural protocol response; {}",
            watch_detail(watch)
        ));
    }
    Ok(())
}

enum Terminated {
    Accept,
    Refuse,
}

fn terminated_outcome(
    watch: &Watch,
    mode: Terminated,
    echo: Option<(u8, usize)>,
) -> Result<(), String> {
    if watch.bytes_written != watch.payload_len {
        return Err(format!(
            "terminated frame was not fully acquired; {}",
            watch_detail(watch)
        ));
    }
    if has_panic(&watch.stderr) || has_panic(&watch.stdout) {
        return Err(format!(
            "terminated frame panicked; {}",
            watch_detail(watch)
        ));
    }
    if let Some((byte, run)) = echo
        && (contains_run(&watch.stdout, byte, run) || contains_run(&watch.stderr, byte, run))
    {
        return Err(format!(
            "terminated frame echoed input; {}",
            watch_detail(watch)
        ));
    }
    match mode {
        Terminated::Accept => {
            let accepted =
                has_result_for_id(&watch.stdout, 15) || has_error_for_id(&watch.stdout, 15);
            let natural = matches!(watch.stop, Stop::Response | Stop::Exited(Some(_)));
            if accepted && natural {
                Ok(())
            } else {
                Err(format!(
                    "exact ceiling request had no public response; {}",
                    watch_detail(watch)
                ))
            }
        }
        Terminated::Refuse => {
            if contains_slice(&watch.stdout, b"\"result\"") {
                return Err(format!(
                    "terminated oversized frame produced success; {}",
                    watch_detail(watch)
                ));
            }
            if clean_refusal(watch, b"unused-canary", 0) {
                Ok(())
            } else {
                Err(format!(
                    "terminated oversized frame was not a natural refusal; {}",
                    watch_detail(watch)
                ))
            }
        }
    }
}

fn b1_unterminated(root: &Path, current: &Path) -> Result<(), String> {
    let payload = oversized_unterminated();
    let watch = serve(
        root,
        current,
        InputPlan::WriteHold(payload),
        Goal::ResponseOrExit,
        REFUSAL_WAIT,
    )?;
    observe("b1_unterminated_open", &watch);
    let primary = if matches!(watch.stop, Stop::ExitedDuringSettle(_)) {
        Err(format!(
            "server exited before the frame; {}",
            watch_detail(&watch)
        ))
    } else if watch.bytes_written <= MCP_FRAME_LIMIT {
        Err(format!(
            "oversized frame was not delivered; {}",
            watch_detail(&watch)
        ))
    } else if clean_refusal(&watch, CANARY, 14) {
        Ok(())
    } else {
        Err(format!(
            "unterminated oversized frame was not refused; {}",
            watch_detail(&watch)
        ))
    };
    finish_case(primary, require_cleanup(&watch))
}

fn b1_valid(root: &Path, current: &Path, ending: &[u8], id: i64, name: &str) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteHold(initialize_bytes(id, ending)),
        Goal::ResponseOrExit,
        PROTOCOL_WAIT,
    )?;
    observe(name, &watch);
    let primary = if watch.stdin_closed_by_us {
        Err(format!(
            "valid frame closed stdin; {}",
            watch_detail(&watch)
        ))
    } else {
        require_success(&watch, id)
    };
    finish_case(primary, require_cleanup(&watch))
}

fn b1_at_bound(root: &Path, current: &Path) -> Result<(), String> {
    let payload = at_bound_frame()?;
    let watch = serve(
        root,
        current,
        InputPlan::WriteHold(payload),
        Goal::ResponseOrExit,
        FRAME_WAIT,
    )?;
    observe("b1_at_bound", &watch);
    let primary = terminated_outcome(&watch, Terminated::Accept, None);
    finish_case(primary, require_cleanup(&watch))
}

fn b1_oversized(root: &Path, current: &Path) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteHold(repeated(b'B', MCP_FRAME_LIMIT + FRAME_OVERSHOOT)),
        Goal::ResponseOrExit,
        FRAME_WAIT,
    )?;
    observe("b1_oversized_terminated", &watch);
    let primary = terminated_outcome(&watch, Terminated::Refuse, Some((b'B', 64)));
    finish_case(primary, require_cleanup(&watch))
}

fn b1_malformed(root: &Path, current: &Path) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteClose(vec![0xff, 0xfe, b'\n']),
        Goal::Exit,
        PROTOCOL_WAIT,
    )?;
    observe("b1_malformed_utf8", &watch);
    let primary = if has_panic(&watch.stderr)
        || has_panic(&watch.stdout)
        || contains_slice(&watch.stdout, b"\"result\"")
    {
        Err(format!(
            "malformed utf-8 failed closed observation; {}",
            watch_detail(&watch)
        ))
    } else if !matches!(watch.stop, Stop::Exited(Some(_))) {
        Err(format!(
            "malformed utf-8 did not end naturally; {}",
            watch_detail(&watch)
        ))
    } else {
        Ok(())
    };
    finish_case(primary, require_cleanup(&watch))
}

fn b1_empty_eof(root: &Path, current: &Path) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::CloseImmediately,
        Goal::Exit,
        PROTOCOL_WAIT,
    )?;
    observe("b1_empty_eof", &watch);
    let primary = if has_panic(&watch.stderr) || contains_slice(&watch.stdout, b"\"result\"") {
        Err(format!(
            "empty EOF produced a result or panic; {}",
            watch_detail(&watch)
        ))
    } else if !matches!(watch.stop, Stop::Exited(Some(_))) {
        Err(format!(
            "empty EOF had no natural exit; {}",
            watch_detail(&watch)
        ))
    } else {
        Ok(())
    };
    finish_case(primary, require_cleanup(&watch))
}

fn b1_short_eof(root: &Path, current: &Path) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteClose(initialize_bytes(13, b"")),
        Goal::Exit,
        PROTOCOL_WAIT,
    )?;
    observe("b1_short_eof", &watch);
    let primary = if !matches!(watch.stop, Stop::Exited(Some(_))) {
        Err(format!(
            "short EOF did not exit naturally; {}",
            watch_detail(&watch)
        ))
    } else {
        require_success(&watch, 13)
    };
    finish_case(primary, require_cleanup(&watch))
}

fn prepare_library(directory: &Path) -> Result<PathBuf, String> {
    let root = directory.join("library");
    std::fs::create_dir_all(&root).map_err(|error| format!("create library: {error}"))?;
    let mut command = omni(&root, directory);
    command.arg("init");
    let watch = watch(
        command,
        InputPlan::CloseImmediately,
        Goal::Exit,
        Duration::ZERO,
        INIT_WAIT,
    )?;
    observe("init", &watch);
    let primary = match watch.stop {
        Stop::Exited(Some(0)) => Ok(root),
        _ => Err(format!("init did not succeed; {}", watch_detail(&watch))),
    };
    match (primary, require_cleanup(&watch)) {
        (Ok(root), Ok(())) => Ok(root),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(cleanup)) => Err(format!("cleanup separate from primary pass: {cleanup}")),
        (Err(error), Err(cleanup)) => Err(format!("primary {error}; cleanup {cleanup}")),
    }
}

const B2_OBSERVATIONS: [&str; 2] = ["b2_ordinary_prefix", "b2_working_allocation"];

const B1_OBSERVATIONS: [&str; 8] = [
    "b1_unterminated_open",
    "b1_valid_lf",
    "b1_valid_crlf",
    "b1_at_bound",
    "b1_oversized_terminated",
    "b1_empty_eof",
    "b1_short_eof",
    "b1_malformed_utf8",
];

/// Marks product observations that have not started as `NOT_RUN`.
///
/// Returns true when an unresolved pid and start token still own the fixture.
fn unresolved_blocks(cases: &mut Vec<Outcome>, directory: &Path) -> bool {
    let obligation = unresolved_note();
    if obligation.is_empty() {
        return false;
    }
    for name in B2_OBSERVATIONS
        .iter()
        .chain(B1_OBSERVATIONS.iter())
        .copied()
    {
        if cases.iter().any(|case| case.name == name) {
            continue;
        }
        record_not_run(
            cases,
            name,
            format!(
                "unresolved ownership retains fixture {}; {obligation}",
                directory.display()
            ),
        );
    }
    true
}

/// Removes a fixture only after collection. An unresolved child keeps its path.
fn settle_fixture(directory: &Path) {
    let obligation = unresolved_note();
    if !obligation.is_empty() {
        println!(
            "CLEANUP fixture_retained path={} obligation={obligation}",
            directory.display()
        );
        return;
    }
    if let Err(error) = std::fs::remove_dir_all(directory) {
        println!("CLEANUP temp_dir_remove_failed {error}");
    } else {
        println!("CLEANUP temp_dir_removed");
    }
}

/// Stops the remaining product work and retains the fixture when ownership is unresolved.
fn stop_unresolved_fixture(cases: &mut Vec<Outcome>, directory: &Path) -> bool {
    if !unresolved_blocks(cases, directory) {
        return false;
    }
    settle_fixture(directory);
    true
}

/// Runs the ten product observations in one fixture directory.
///
/// A failed product predicate still continues when its child was collected.
/// Unresolved ownership records later observations as `NOT_RUN` and keeps the directory.
fn run_product(cases: &mut Vec<Outcome>) {
    let directory =
        std::env::temp_dir().join(format!("omni-product-boundaries-{}", std::process::id()));
    if directory.exists() {
        let _ = std::fs::remove_dir_all(&directory);
    }
    if let Err(error) = std::fs::create_dir_all(&directory) {
        record(
            cases,
            "product",
            "workspace",
            Err(format!("temp dir: {error}")),
        );
        return;
    }
    record(
        cases,
        "product",
        "b2_ordinary_prefix",
        b2_ordinary(&directory),
    );
    if stop_unresolved_fixture(cases, &directory) {
        return;
    }
    record(
        cases,
        "product",
        "b2_working_allocation",
        b2_amplification(&directory),
    );
    if stop_unresolved_fixture(cases, &directory) {
        return;
    }
    match prepare_library(&directory) {
        Ok(root) => {
            record(
                cases,
                "product",
                "b1_unterminated_open",
                b1_unterminated(&root, &directory),
            );
            if stop_unresolved_fixture(cases, &directory) {
                return;
            }
            record(
                cases,
                "product",
                "b1_valid_lf",
                b1_valid(&root, &directory, b"\n", 11, "b1_valid_lf"),
            );
            if stop_unresolved_fixture(cases, &directory) {
                return;
            }
            record(
                cases,
                "product",
                "b1_valid_crlf",
                b1_valid(&root, &directory, b"\r\n", 12, "b1_valid_crlf"),
            );
            if stop_unresolved_fixture(cases, &directory) {
                return;
            }
            record(
                cases,
                "product",
                "b1_at_bound",
                b1_at_bound(&root, &directory),
            );
            if stop_unresolved_fixture(cases, &directory) {
                return;
            }
            record(
                cases,
                "product",
                "b1_oversized_terminated",
                b1_oversized(&root, &directory),
            );
            if stop_unresolved_fixture(cases, &directory) {
                return;
            }
            record(
                cases,
                "product",
                "b1_empty_eof",
                b1_empty_eof(&root, &directory),
            );
            if stop_unresolved_fixture(cases, &directory) {
                return;
            }
            record(
                cases,
                "product",
                "b1_short_eof",
                b1_short_eof(&root, &directory),
            );
            if stop_unresolved_fixture(cases, &directory) {
                return;
            }
            record(
                cases,
                "product",
                "b1_malformed_utf8",
                b1_malformed(&root, &directory),
            );
        }
        Err(error) => {
            for name in B1_OBSERVATIONS {
                record_not_run(cases, name, format!("setup init: {error}"));
            }
        }
    }
    settle_fixture(&directory);
}

fn run_standin_mode(mode: &str) -> ! {
    match mode {
        "refuse" => {
            let _ = std::io::stdout().write_all(b"refused\n");
            let _ = std::io::stdout().flush();
            std::process::exit(2);
        }
        "echo" => {
            let mut buffer = [0u8; 128];
            let size = std::io::stdin().read(&mut buffer).unwrap_or(0);
            let mut out = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n".to_vec();
            out.extend_from_slice(&buffer[..size]);
            let _ = std::io::stdout().write_all(&out);
            let _ = std::io::stdout().flush();
            std::process::exit(0);
        }
        "early-eof" => {
            let mut buffer = [0u8; 4096];
            let mut total = 0usize;
            let mut input = std::io::stdin();
            loop {
                if total >= 65_536 {
                    std::process::exit(3);
                }
                match input.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => total += size,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => std::process::exit(4),
                }
            }
            let _ = std::io::stdout().write_all(b"ended-on-eof\n");
            let _ = std::io::stdout().flush();
            std::process::exit(0);
        }
        "hang" => {
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(0);
        }
        "stdout-live" => {
            let _ = std::io::stdout().flush();
            unsafe { close(1) };
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(0);
        }
        "drain-hang" => {
            let _ = set_nonblocking(std::io::stdin().as_raw_fd());
            let mut buffer = [0u8; 4096];
            let mut total = 0usize;
            let started = Instant::now();
            while started.elapsed() < Duration::from_millis(200) && total < 8192 {
                match std::io::stdin().read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => total += size,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(POLL);
                    }
                    Err(_) => break,
                }
            }
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(0);
        }
        "signal" => {
            match i32::try_from(std::process::id()) {
                Ok(pid) if pid > 1 => unsafe {
                    kill(pid, SIGKILL);
                },
                _ => std::process::exit(78),
            }
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(78);
        }
        "success" => {
            let _ = std::io::stdout().write_all(b"{\"jsonrpc\":\"2.0\",\"id\":21,\"result\":{}}\n");
            let _ = std::io::stdout().flush();
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(0);
        }
        _ => std::process::exit(78),
    }
}

#[test]
fn product_boundaries() {
    if let Ok(mode) = std::env::var("OMNI_PRODUCT_BOUND_STANDIN") {
        run_standin_mode(&mode);
    }
    println!(
        "PRERECORD frame_limit={MCP_FRAME_LIMIT} overshoot={FRAME_OVERSHOOT} caller_limit={CALLER_LIMIT} shared_len={SHARED_LEN} references={REFERENCE_COUNT} expansion={REFERENCED_EXPANSION} factor={WORKING_FACTOR} host_copy_stop={HOST_COPY_STOP} max_text_bytes={}",
        omni_knowledge::MAX_TEXT_BYTES
    );
    println!("BINARY {}", env!("CARGO_BIN_EXE_omni"));
    if let Err(error) = run_admission() {
        panic!("instrument admission failed: {error}");
    }
    let mut cases = Vec::new();
    run_product(&mut cases);
    let pass = cases.iter().filter(|case| case.state == "PASS").count();
    let fail = cases.iter().filter(|case| case.state == "FAIL").count();
    let not_run = cases.iter().filter(|case| case.state == "NOT_RUN").count();
    println!("SUMMARY product_pass={pass} product_fail={fail} product_not_run={not_run}");
    let failed = cases
        .iter()
        .filter(|case| case.state == "FAIL")
        .map(|case| format!("{}:{}:{}", case.kind, case.name, case.detail))
        .collect::<Vec<_>>()
        .join(",");
    let blocked = cases
        .iter()
        .filter(|case| case.state == "NOT_RUN")
        .map(|case| format!("{}:{}:{}", case.kind, case.name, case.detail))
        .collect::<Vec<_>>()
        .join(",");
    let unresolved = unresolved_note();
    if !failed.is_empty() || !unresolved.is_empty() {
        panic!("product boundary cases failed: {failed}; unresolved: {unresolved}");
    }
    if !blocked.is_empty() {
        panic!("setup blocked product cases: {blocked}");
    }
}
