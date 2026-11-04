//! Public product bounds for MCP frame acquisition and XLSX text assembly.
//!
//! Expectations come from the public CLI, the extraction API, and the processing
//! owner. Controller stand-ins admit the observer and the allocation counter.
//! They are not MCP behavior and they are not a product pass.

mod fixture;

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
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
const WRITE_GRACE: Duration = Duration::from_millis(400);
const INIT_WAIT: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(5);

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;
const SIGKILL: i32 = 9;

unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

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
    StdoutEof,
    Timeout,
    Predicate,
    ExitedDuringSettle(Option<i32>),
}

struct Watch {
    stop: Stop,
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
    Terminal,
    Predicate,
    WriteAccepted,
}

struct Reap {
    child: Child,
    pid: u32,
    done: bool,
}

impl Drop for Reap {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let _ = group_kill(self.pid);
        let _ = self.child.wait();
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
    let flags = unsafe { fcntl(fd, F_GETFL, 0) };
    if flags < 0 {
        return Err(format!("fcntl read flags failed on {fd}"));
    }
    let updated = unsafe { fcntl(fd, F_SETFL, flags | O_NONBLOCK) };
    if updated < 0 {
        return Err(format!("fcntl nonblocking failed on {fd}"));
    }
    Ok(())
}

fn start_ticks(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = text.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
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
    mut command: Command,
    plan: InputPlan,
    goal: Goal,
    settle: Duration,
    wait: Duration,
) -> Result<Watch, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|error| format!("spawn failed: {error}"))?;
    let pid = child.id();
    let start_ticks = start_ticks(pid);
    let stdout_fd = child.stdout.as_ref().ok_or("missing stdout")?.as_raw_fd();
    let stderr_fd = child.stderr.as_ref().ok_or("missing stderr")?.as_raw_fd();
    let stdin_fd = child.stdin.as_ref().ok_or("missing stdin")?.as_raw_fd();
    set_nonblocking(stdout_fd)?;
    set_nonblocking(stderr_fd)?;
    set_nonblocking(stdin_fd)?;
    let mut reap = Reap {
        child,
        pid,
        done: false,
    };
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
    let mut grace_started: Option<Instant> = None;
    let mut wait_error: Option<String> = None;
    while stop.is_none() {
        if !reap.done {
            match reap.child.try_wait() {
                Ok(Some(status)) => {
                    reap.done = true;
                    let during_settle = Instant::now() < settle_deadline && written == 0;
                    stop = Some(if during_settle && !settle.is_zero() {
                        Stop::ExitedDuringSettle(status_code(status))
                    } else {
                        Stop::Exited(status_code(status))
                    });
                }
                Ok(None) => {}
                Err(error) => {
                    reap.done = true;
                    wait_error = Some(error.to_string());
                    stop = Some(Stop::Exited(None));
                }
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
        if stdout_eof && stop.is_none() && !matches!(goal, Goal::WriteAccepted) {
            stop = Some(Stop::StdoutEof);
        }
        if matches!(goal, Goal::Predicate) && predicate_ready(&stdout) && stop.is_none() {
            stop = Some(Stop::Predicate);
        }
        if matches!(goal, Goal::WriteAccepted) && write_finished && stop.is_none() {
            let grace = grace_started.get_or_insert_with(Instant::now);
            if grace.elapsed() >= WRITE_GRACE {
                stop = Some(Stop::Predicate);
            }
        }
        if stop.is_none()
            && settled
            && Instant::now().saturating_duration_since(wait_origin) >= wait
        {
            stop = Some(Stop::Timeout);
        }
        if stop.is_none() {
            std::thread::sleep(POLL);
        }
    }
    let stop = stop.unwrap_or(Stop::Timeout);
    let cleanup_error = match (wait_error, cleanup_reap(&mut reap)) {
        (None, later) => later,
        (Some(error), None) => Some(error),
        (Some(error), Some(later)) => Some(format!("{error}; {later}")),
    };
    let same = same_process(pid, start_ticks);
    Ok(Watch {
        stop,
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

fn status_code(status: ExitStatus) -> Option<i32> {
    status.code()
}

fn predicate_ready(stdout: &[u8]) -> bool {
    stdout.contains(&b'\n')
}

fn cleanup_reap(reap: &mut Reap) -> Option<String> {
    if reap.done {
        return None;
    }
    let kill_error = group_kill(reap.pid).err();
    let wait_error = match reap.child.wait() {
        Ok(_) => None,
        Err(error) => Some(error.to_string()),
    };
    reap.done = true;
    match (kill_error, wait_error) {
        (None, None) => None,
        (Some(kill), None) => Some(kill),
        (None, Some(wait)) => Some(wait),
        (Some(kill), Some(wait)) => Some(format!("{kill}; {wait}")),
    }
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
    let exact = format!("\"id\":{id}").into_bytes();
    let spaced = format!("\"id\": {id}").into_bytes();
    for line in buffer.split(|byte| *byte == b'\n') {
        let has_id = contains_slice(line, &exact) || contains_slice(line, &spaced);
        if has_id && contains_slice(line, b"\"result\"") {
            return true;
        }
    }
    false
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
        "stop={:?} pid={} start_ticks={:?} written={}/{} stdin_closed={} elapsed_ms={} cleanup={} same_pid_after={} stdout={} stderr={}",
        watch.stop,
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
        Goal::Terminal,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    require_cleanup(&watch)?;
    match watch.stop {
        Stop::Exited(Some(2))
            if !watch.stdin_closed_by_us && contains_slice(&watch.stdout, b"refused") =>
        {
            Ok(())
        }
        other => Err(format!(
            "refuse stand-in classified as {other:?}; {}",
            watch_detail(&watch)
        )),
    }
}

fn admit_echo() -> Result<(), String> {
    let watch = watch(
        standin("echo"),
        InputPlan::WriteHold(CANARY.to_vec()),
        Goal::Terminal,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    require_cleanup(&watch)?;
    let saw_canary = contains_slice(&watch.stdout, CANARY);
    let saw_result = has_result_for_id(&watch.stdout, 1);
    let clean = clean_refusal(&watch, CANARY, 1);
    if saw_canary && saw_result && !clean {
        Ok(())
    } else {
        Err(format!(
            "echo detector canary={saw_canary} result={saw_result} clean={clean}; {}",
            watch_detail(&watch)
        ))
    }
}

fn admit_early_eof() -> Result<(), String> {
    let watch = watch(
        standin("early-eof"),
        InputPlan::CloseImmediately,
        Goal::Terminal,
        Duration::ZERO,
        ADMISSION_WAIT,
    )?;
    require_cleanup(&watch)?;
    if watch.stdin_closed_by_us
        && matches!(watch.stop, Stop::Exited(Some(0)))
        && contains_slice(&watch.stdout, b"ended-on-eof")
    {
        Ok(())
    } else {
        Err(format!("early EOF misclassified; {}", watch_detail(&watch)))
    }
}

fn admit_hang() -> Result<(), String> {
    let watch = watch(
        standin("hang"),
        InputPlan::Hold,
        Goal::Terminal,
        Duration::ZERO,
        HANG_WAIT,
    )?;
    require_cleanup(&watch)?;
    if matches!(watch.stop, Stop::Timeout) && !watch.stdin_closed_by_us {
        Ok(())
    } else {
        Err(format!("hang was not a timeout; {}", watch_detail(&watch)))
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
    matches!(watch.stop, Stop::Exited(_) | Stop::StdoutEof)
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
    require_cleanup(watch)?;
    if has_panic(&watch.stderr) || has_panic(&watch.stdout) {
        return Err(format!("panic; {}", watch_detail(watch)));
    }
    if !has_result_for_id(&watch.stdout, id) {
        return Err(format!(
            "missing initialize result; {}",
            watch_detail(watch)
        ));
    }
    if matches!(watch.stop, Stop::Timeout | Stop::ExitedDuringSettle(_)) {
        return Err(format!("no protocol response; {}", watch_detail(watch)));
    }
    Ok(())
}

fn b1_unterminated(root: &Path, current: &Path) -> Result<(), String> {
    let payload = oversized_unterminated();
    let watch = serve(
        root,
        current,
        InputPlan::WriteHold(payload),
        Goal::Terminal,
        REFUSAL_WAIT,
    )?;
    require_cleanup(&watch)?;
    if matches!(watch.stop, Stop::ExitedDuringSettle(_)) {
        return Err(format!(
            "server exited before the frame; {}",
            watch_detail(&watch)
        ));
    }
    if watch.bytes_written <= MCP_FRAME_LIMIT {
        return Err(format!(
            "oversized frame was not delivered; {}",
            watch_detail(&watch)
        ));
    }
    if !clean_refusal(&watch, CANARY, 14) {
        return Err(format!(
            "unterminated oversized frame was not refused; {}",
            watch_detail(&watch)
        ));
    }
    Ok(())
}

fn b1_valid(root: &Path, current: &Path, ending: &[u8], id: i64) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteHold(initialize_bytes(id, ending)),
        Goal::Predicate,
        PROTOCOL_WAIT,
    )?;
    if watch.stdin_closed_by_us {
        return Err(format!(
            "valid frame closed stdin; {}",
            watch_detail(&watch)
        ));
    }
    require_success(&watch, id)
}

fn b1_write_accepted(
    root: &Path,
    current: &Path,
    payload: Vec<u8>,
    echo: Option<(u8, usize)>,
) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteHold(payload),
        Goal::WriteAccepted,
        FRAME_WAIT,
    )?;
    require_cleanup(&watch)?;
    if watch.bytes_written != watch.payload_len {
        return Err(format!(
            "terminated frame was not fully acquired; {}",
            watch_detail(&watch)
        ));
    }
    if has_panic(&watch.stderr)
        || has_panic(&watch.stdout)
        || contains_slice(&watch.stdout, b"\"result\"")
    {
        return Err(format!(
            "terminated frame produced panic or success; {}",
            watch_detail(&watch)
        ));
    }
    if let Some((byte, run)) = echo
        && (contains_run(&watch.stdout, byte, run) || contains_run(&watch.stderr, byte, run))
    {
        return Err(format!(
            "terminated frame echoed input; {}",
            watch_detail(&watch)
        ));
    }
    if matches!(watch.stop, Stop::Timeout | Stop::ExitedDuringSettle(_))
        && watch.bytes_written != watch.payload_len
    {
        return Err(format!(
            "terminated frame timed out; {}",
            watch_detail(&watch)
        ));
    }
    Ok(())
}

fn b1_malformed(root: &Path, current: &Path) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteClose(vec![0xff, 0xfe, b'\n']),
        Goal::Terminal,
        PROTOCOL_WAIT,
    )?;
    require_cleanup(&watch)?;
    if has_panic(&watch.stderr)
        || has_panic(&watch.stdout)
        || contains_slice(&watch.stdout, b"\"result\"")
    {
        return Err(format!(
            "malformed utf-8 failed closed observation; {}",
            watch_detail(&watch)
        ));
    }
    if !matches!(watch.stop, Stop::Exited(_)) {
        return Err(format!(
            "malformed utf-8 did not end on EOF; {}",
            watch_detail(&watch)
        ));
    }
    Ok(())
}

fn b1_empty_eof(root: &Path, current: &Path) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::CloseImmediately,
        Goal::Terminal,
        PROTOCOL_WAIT,
    )?;
    require_cleanup(&watch)?;
    if has_panic(&watch.stderr) || contains_slice(&watch.stdout, b"\"result\"") {
        return Err(format!(
            "empty EOF produced a result or panic; {}",
            watch_detail(&watch)
        ));
    }
    if !matches!(watch.stop, Stop::Exited(_)) {
        return Err(format!("empty EOF timed out; {}", watch_detail(&watch)));
    }
    Ok(())
}

fn b1_short_eof(root: &Path, current: &Path) -> Result<(), String> {
    let watch = serve(
        root,
        current,
        InputPlan::WriteClose(initialize_bytes(13, b"")),
        Goal::Terminal,
        PROTOCOL_WAIT,
    )?;
    require_success(&watch, 13)?;
    if !matches!(watch.stop, Stop::Exited(_)) {
        return Err(format!("short EOF did not exit; {}", watch_detail(&watch)));
    }
    Ok(())
}

fn prepare_library(directory: &Path) -> Result<PathBuf, String> {
    let root = directory.join("library");
    std::fs::create_dir_all(&root).map_err(|error| format!("create library: {error}"))?;
    let mut command = omni(&root, directory);
    command.arg("init");
    let watch = watch(
        command,
        InputPlan::CloseImmediately,
        Goal::Terminal,
        Duration::ZERO,
        INIT_WAIT,
    )?;
    require_cleanup(&watch)?;
    match watch.stop {
        Stop::Exited(Some(0)) => Ok(root),
        _ => Err(format!("init did not succeed; {}", watch_detail(&watch))),
    }
}

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
    record(
        cases,
        "product",
        "b2_working_allocation",
        b2_amplification(&directory),
    );
    match prepare_library(&directory) {
        Ok(root) => {
            record(
                cases,
                "product",
                "b1_unterminated_open",
                b1_unterminated(&root, &directory),
            );
            record(
                cases,
                "product",
                "b1_valid_lf",
                b1_valid(&root, &directory, b"\n", 11),
            );
            record(
                cases,
                "product",
                "b1_valid_crlf",
                b1_valid(&root, &directory, b"\r\n", 12),
            );
            record(
                cases,
                "product",
                "b1_at_bound",
                b1_write_accepted(
                    &root,
                    &directory,
                    repeated(b'A', MCP_FRAME_LIMIT),
                    Some((b'A', 64)),
                ),
            );
            record(
                cases,
                "product",
                "b1_oversized_terminated",
                b1_write_accepted(
                    &root,
                    &directory,
                    repeated(b'B', MCP_FRAME_LIMIT + FRAME_OVERSHOOT),
                    Some((b'B', 64)),
                ),
            );
            record(
                cases,
                "product",
                "b1_empty_eof",
                b1_empty_eof(&root, &directory),
            );
            record(
                cases,
                "product",
                "b1_short_eof",
                b1_short_eof(&root, &directory),
            );
            record(
                cases,
                "product",
                "b1_malformed_utf8",
                b1_malformed(&root, &directory),
            );
        }
        Err(error) => {
            for name in [
                "b1_unterminated_open",
                "b1_valid_lf",
                "b1_valid_crlf",
                "b1_at_bound",
                "b1_oversized_terminated",
                "b1_empty_eof",
                "b1_short_eof",
                "b1_malformed_utf8",
            ] {
                record_not_run(cases, name, format!("setup init: {error}"));
            }
        }
    }
    if let Err(error) = std::fs::remove_dir_all(&directory) {
        println!("CLEANUP temp_dir_remove_failed {error}");
    } else {
        println!("CLEANUP temp_dir_removed");
    }
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
    if !failed.is_empty() {
        panic!("product boundary cases failed: {failed}");
    }
    if !blocked.is_empty() {
        panic!("setup blocked product cases: {blocked}");
    }
}
