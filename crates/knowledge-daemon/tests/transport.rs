//! Transport checks against a synthetic handler.
//!
//! The handler records frames and returns typed data. Those returns are not
//! authority, storage, or exposure proof.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use knowledge_daemon::{
    AccountFacts, Daemon, NativeHandler, TransportFacts, TransportKind, command_op, proxy_mcp,
    serve_mcp,
};
use omni_service::{
    Command, Coverage, Enumeration, Id, Job, JobKind, JobPriority, JobState, Request, Response,
    Result as WireResult, ResultExtent, U64,
};
use serde_json::Value;

struct Recorder {
    calls: Mutex<Vec<String>>,
    facts: Mutex<Vec<TransportFacts>>,
    hold: bool,
    release: Mutex<bool>,
    wake: Condvar,
    running_frames: Mutex<u32>,
}

impl Recorder {
    fn open(hold: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            facts: Mutex::new(Vec::new()),
            hold,
            release: Mutex::new(false),
            wake: Condvar::new(),
            running_frames: Mutex::new(0),
        })
    }

    fn release(self: &Arc<Self>) {
        *self.release.lock().unwrap() = true;
        self.wake.notify_all();
    }
}

impl NativeHandler for Recorder {
    fn handle(&self, request: &Request, facts: &TransportFacts) -> Response {
        self.calls
            .lock()
            .unwrap()
            .push(command_op(&request.command).to_owned());
        self.facts.lock().unwrap().push(facts.clone());
        if self.hold {
            let mut released = self.release.lock().unwrap();
            while !*released {
                released = self.wake.wait(released).unwrap();
            }
        }
        match &request.command {
            Command::JobStatus { .. } => {
                let mut frames = self.running_frames.lock().unwrap();
                *frames += 1;
                let state = if *frames >= 2 {
                    JobState::Completed
                } else {
                    JobState::Running
                };
                job_response(request, state)
            }
            _ => Response {
                wire_version: 1,
                request_id: request.request_id.clone(),
                result: WireResult::Libraries { libraries: vec![] },
            },
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        *self.release.lock().unwrap() = true;
        self.wake.notify_all();
    }
}

fn job_response(request: &Request, state: JobState) -> Response {
    let job_id = match &request.command {
        Command::JobStatus { job_id } => job_id.clone(),
        _ => Id::new("job").unwrap(),
    };
    let zero = U64::new(0);
    Response {
        wire_version: 1,
        request_id: request.request_id.clone(),
        result: WireResult::Job {
            job: Box::new(Job {
                job_id,
                operation_id: Id::new("operation").unwrap(),
                library_id: Id::new("library").unwrap(),
                kind: JobKind::Reconcile,
                state,
                priority: JobPriority::Normal,
                coverage: Coverage {
                    discovered: zero,
                    verified: zero,
                    pending_hash: zero,
                    pending_extraction: zero,
                    pending_embedding: zero,
                    stale: zero,
                    unavailable: zero,
                    unsupported: zero,
                    failed: zero,
                    excluded: zero,
                    enumeration: Enumeration::NotStarted,
                    result_extent: ResultExtent::Enumeration,
                    stopped_reason: None,
                },
                error: None,
            }),
        },
    }
}

fn account() -> AccountFacts {
    AccountFacts {
        memory_budget_bytes: 1000,
        memory_reserved_bytes: 40,
        storage_budget_bytes: 2000,
        storage_reserved_bytes: 50,
        waiting_for_memory: 3,
    }
}

fn post(
    addr: std::net::SocketAddr,
    host: &str,
    path: &str,
    body: &[u8],
    origin: Option<&str>,
) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let origin = origin
        .map(|value| format!("Origin: {value}\r\n"))
        .unwrap_or_default();
    let header = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\n{origin}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut buffer = Vec::new();
    let _ = stream.read_to_end(&mut buffer);
    split_http(&buffer)
}

fn split_http(bytes: &[u8]) -> (u16, Vec<u8>) {
    let split = bytes
        .windows(4)
        .position(|mark| mark == b"\r\n\r\n")
        .unwrap();
    let head = std::str::from_utf8(&bytes[..split]).unwrap();
    let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    let raw = &bytes[split + 4..];
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunks(raw)
    } else {
        raw.to_vec()
    };
    (status, body)
}

fn decode_chunks(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(line_end) = rest.windows(2).position(|mark| mark == b"\r\n") {
        let size =
            usize::from_str_radix(std::str::from_utf8(&rest[..line_end]).unwrap().trim(), 16)
                .unwrap();
        rest = &rest[line_end + 2..];
        if size == 0 {
            break;
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
    out
}

#[test]
fn stdio_mcp_rejects_unknown_framing_before_the_handler() {
    let recorder = Recorder::open(false);
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"exposure_set","arguments":{}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"library_list","arguments":{}}}"#,
        "\n",
        "{\"not json\"\n",
    );
    let mut output = Vec::new();
    serve_mcp(
        &mut std::io::Cursor::new(input.as_bytes()),
        &mut output,
        recorder.as_ref(),
        Some(account()),
    )
    .unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(lines[0]["error"]["code"].as_i64().is_some());
    assert_eq!(lines[1]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(lines[2]["result"]["tools"].as_array().unwrap().len(), 23);
    assert!(lines[3]["error"]["code"].as_i64().is_some());
    assert_eq!(lines[4]["result"]["isError"], false);
    assert_eq!(
        lines[4]["result"]["structuredContent"]["result"]["type"],
        "libraries"
    );
    assert_eq!(recorder.calls.lock().unwrap().as_slice(), ["library_list"]);
    let facts = recorder.facts.lock().unwrap();
    assert_eq!(facts[0].kind, TransportKind::McpStdio);
    assert_eq!(facts[0].account.unwrap().waiting_for_memory, 3);
    assert!(facts[0].host.is_none());
}

#[test]
fn ipc_half_close_and_malformed_frame_do_not_invent_success() {
    let recorder = Recorder::open(false);
    let daemon = Daemon::start(recorder.clone(), None).unwrap();
    let mut stream = TcpStream::connect(daemon.ipc_addr()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .write_all(br#"{"wire_version":1,"request_id":"ipc-1","command":{"op":"library_list"}}"#)
        .unwrap();
    stream.write_all(b"\n").unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).unwrap();
    let response: Value = serde_json::from_slice(buffer.trim_ascii()).unwrap();
    assert_eq!(response["result"]["type"], "libraries");
    assert_eq!(recorder.calls.lock().unwrap().as_slice(), ["library_list"]);

    let before = recorder.calls.lock().unwrap().len();
    let mut bad = TcpStream::connect(daemon.ipc_addr()).unwrap();
    bad.set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    bad.write_all(
        br#"{"wire_version":1,"request_id":"bad","command":{"op":"library_list","extra":true}}"#,
    )
    .unwrap();
    bad.write_all(b"\n").unwrap();
    let mut ignored = Vec::new();
    let _ = bad.read_to_end(&mut ignored);
    assert_eq!(recorder.calls.lock().unwrap().len(), before);
    drop(daemon);
}

#[test]
fn http_forwards_host_and_refuses_bad_origin_without_a_handler_call() {
    let recorder = Recorder::open(false);
    let daemon = Daemon::start(recorder.clone(), Some(account())).unwrap();
    let body = br#"{"wire_version":1,"request_id":"http-1","command":{"op":"library_list"}}"#;
    let (status, bytes) = post(
        daemon.http_addr(),
        &daemon.allowed_host(),
        "/v1/command",
        body,
        None,
    );
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    let response: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(response["result"]["type"], "libraries");
    let facts = recorder.facts.lock().unwrap();
    assert_eq!(facts[0].kind, TransportKind::Http);
    assert_eq!(
        facts[0].host.as_deref(),
        Some(daemon.allowed_host().as_str())
    );
    assert!(facts[0].origin.is_none());
    assert_eq!(facts[0].account.unwrap().memory_budget_bytes, 1000);
    drop(facts);

    let calls = recorder.calls.lock().unwrap().len();
    let (status, _) = post(
        daemon.http_addr(),
        "evil.example",
        "/v1/command",
        body,
        None,
    );
    assert_eq!(status, 403);
    let (status, _) = post(
        daemon.http_addr(),
        &daemon.allowed_host(),
        "/v1/command",
        body,
        Some("http://evil.example"),
    );
    assert_eq!(status, 403);
    let (status, _) = post(
        daemon.http_addr(),
        &daemon.allowed_host(),
        "/v1/command",
        br#"{"wire_version":2,"request_id":"http-2","command":{"op":"library_list"}}"#,
        None,
    );
    assert_eq!(status, 400);
    let (status, _) = post(
        daemon.http_addr(),
        &daemon.allowed_host(),
        "/v1/command",
        br#"{"wire_version":1,"request_id":"http-3","command":{"op":"nope"}}"#,
        None,
    );
    assert_eq!(status, 400);
    assert_eq!(recorder.calls.lock().unwrap().len(), calls);
    drop(daemon);
}

#[test]
fn status_stream_paces_frames_and_refuses_the_seventeenth_connection() {
    let recorder = Recorder::open(false);
    let daemon = Daemon::start(recorder.clone(), None).unwrap();
    let started = Instant::now();
    let body = br#"{"wire_version":1,"request_id":"stream-1","command":{"op":"job_status","job_id":"job-1"}}"#;
    let (status, bytes) = post(
        daemon.http_addr(),
        &daemon.allowed_host(),
        "/v1/status-stream",
        body,
        None,
    );
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    let lines: Vec<Value> = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["result"]["job"]["state"], "running");
    assert_eq!(lines[1]["result"]["job"]["state"], "completed");
    assert!(started.elapsed() >= Duration::from_millis(900));
    assert!(
        recorder
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|op| op == "job_status")
    );

    let (status, bytes) = post(
        daemon.http_addr(),
        &daemon.allowed_host(),
        "/v1/status-stream",
        br#"{"wire_version":1,"request_id":"stream-2","command":{"op":"library_list"}}"#,
        None,
    );
    assert_eq!(status, 400);
    let refused: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(refused["result"]["error"]["code"], "invalid_request");
    drop(daemon);

    let held = Recorder::open(true);
    let daemon = Daemon::start(held.clone(), None).unwrap();
    let host = daemon.allowed_host();
    let addr = daemon.http_addr();
    let mut clients = Vec::new();
    for index in 0..16 {
        let host = host.clone();
        let body = format!(
            r#"{{"wire_version":1,"request_id":"hold-{index}","command":{{"op":"job_status","job_id":"job-{index}"}}}}"#
        );
        clients.push(std::thread::spawn(move || {
            let _ = post(addr, &host, "/v1/status-stream", body.as_bytes(), None);
        }));
    }
    let wait_started = Instant::now();
    while held.calls.lock().unwrap().len() < 16 {
        assert!(
            wait_started.elapsed() < Duration::from_secs(5),
            "handlers did not fill"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let (status, bytes) = post(addr, &host, "/v1/status-stream", body, None);
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    let refused: Value = serde_json::from_slice(bytes.trim_ascii()).unwrap();
    let code = refused["result"]["error"]["code"].as_str().unwrap();
    assert_eq!(code, "budget_exhausted");
    assert!(refused["result"]["error"]["job_id"].is_null());
    assert_eq!(held.calls.lock().unwrap().len(), 16);
    held.release();
    for client in clients {
        client.join().unwrap();
    }
    drop(daemon);
}

#[test]
fn disconnect_does_not_send_cancel() {
    let recorder = Recorder::open(false);
    let daemon = Daemon::start(recorder.clone(), None).unwrap();
    let mut stream = TcpStream::connect(daemon.http_addr()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let body = br#"{"wire_version":1,"request_id":"drop-1","command":{"op":"job_status","job_id":"job-1"}}"#;
    let header = format!(
        "POST /v1/status-stream HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        daemon.allowed_host(),
        body.len()
    );
    stream.write_all(header.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut buffer = [0; 2048];
    let _ = stream.read(&mut buffer);
    drop(stream);
    std::thread::sleep(Duration::from_millis(1500));
    let calls = recorder.calls.lock().unwrap().clone();
    assert!(calls.iter().all(|op| op == "job_status"));
    assert!(!calls.is_empty());
    drop(daemon);
}

#[test]
fn proxy_round_trip_uses_the_daemon_handler() {
    let recorder = Recorder::open(false);
    let daemon = Daemon::start(recorder.clone(), None).unwrap();
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":"proxy-1","method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":"proxy-2","method":"tools/call","params":{"name":"library_list","arguments":{}}}"#,
        "\n",
    );
    let mut output = Vec::new();
    proxy_mcp(
        &mut std::io::Cursor::new(input.as_bytes()),
        &mut output,
        daemon.ipc_addr(),
    )
    .unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(lines[1]["result"]["isError"], false);
    assert_eq!(recorder.calls.lock().unwrap().as_slice(), ["library_list"]);
    assert_eq!(recorder.facts.lock().unwrap()[0].kind, TransportKind::Ipc);
    drop(daemon);
}
