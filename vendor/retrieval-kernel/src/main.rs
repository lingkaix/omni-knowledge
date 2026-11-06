//! Directory CLI, startup-bound MCP, and trusted local stdio service.
use retrieval_kernel::{directory::Directory, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::BufRead;
use std::path::PathBuf;
use std::time::{Duration, Instant};
mod owned;

/// Dispatch a face through the actual native Core/RSS worker boundary; the engine has no subprocesses.
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // The foreground native owner has completed controlled shutdown before run returns.
            owned::diagnostic(serde_json::to_string(&error).unwrap().as_bytes());
            std::process::ExitCode::FAILURE
        }
    }
}
/// Local bounded error constructor.
fn error(code: &str, message: &str) -> Error {
    Error {
        code: code.into(),
        message: message.into(),
    }
}
/// Parse flags without an alternate command framework or a second search path.
fn run() -> Result<()> {
    process_limits()?;
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--version") {
        owned::output(format!("retrieval-kernel {}", env!("CARGO_PKG_VERSION")).as_bytes())?;
        return Ok(());
    }
    if args.is_empty() {
        return Err(error(
            "invalid-request",
            "usage: retrieval-kernel index|search|mcp|service [arguments]",
        ));
    }
    let command = args.remove(0);
    let store_flag = flag(&mut args, "--store")?;
    let source_flag = flag(&mut args, "--sources")?;
    let limit = flag(&mut args, "--limit")?
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| error("invalid-request", "limit must be a positive integer"))?
        .unwrap_or(20);
    if command == "service" {
        if !args.is_empty() {
            return Err(error("invalid-request", "unexpected service arguments"));
        }
        let root = source_flag
            .ok_or_else(|| error("invalid-request", "service requires a startup source root"))?;
        let store = store_flag.ok_or_else(|| {
            error(
                "invalid-request",
                "service requires an owner-assigned derived store",
            )
        })?;
        let store = PathBuf::from(store);
        let kernel = Kernel::new(&PathBuf::from(root), &store)?;
        if let Some(status) = owned::supervise(&store)? {
            if !status.success() {
                return Err(error("worker-failed", "collected kernel worker failed"));
            }
            return Ok(());
        }
        return stdio(|request| service(&kernel, request), true);
    }
    if source_flag.is_some() {
        return Err(error("invalid-request", "sources flag is service-only"));
    }
    let query_args = if command == "search" {
        if args.len() < 2 {
            return Err(error("invalid-request", "search requires mode and query"));
        }
        let mode = serde_json::from_value(json!(args.remove(0)))
            .map_err(|_| error("invalid-request", "unknown query mode"))?;
        Some((mode, args.remove(0)))
    } else {
        None
    };
    if !["index", "search", "mcp"].contains(&command.as_str()) || args.len() > 1 {
        return Err(error(
            "invalid-request",
            "unknown command or extra arguments",
        ));
    }
    let root = args.pop().map(PathBuf::from).unwrap_or(
        std::env::current_dir().map_err(|_| error("io", "current directory unavailable"))?,
    );
    let store = store_flag
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".retrieval-kernel"));
    let kernel = Kernel::new(&root, &store)?;
    if let Some(status) = owned::supervise(&store)? {
        if !status.success() {
            return Err(error("worker-failed", "collected kernel worker failed"));
        }
        return Ok(());
    }
    let directory = Directory::new(&root, &store)?;
    match command.as_str() {
        "index" => {
            let view = directory.inventory()?;
            let expected = kernel.generation(&view.scope)?;
            let generation = kernel.publish(view, expected)?;
            owned::output(
                json!({"generation":generation,"scope":directory.scope()})
                    .to_string()
                    .as_bytes(),
            )?;
            Ok(())
        }
        "search" => {
            let (mode, query) = query_args.unwrap();
            let result = directory.search(&kernel, mode, query, limit)?;
            owned::output(
                &serde_json::to_vec(&result).map_err(|_| error("io", "serialization failed"))?,
            )?;
            Ok(())
        }
        "mcp" => stdio(|request| mcp(&kernel, &directory, request), false),
        _ => unreachable!(),
    }
}
/// Remove one exact flag/value; duplicate flags and missing values fail closed.
fn flag(args: &mut Vec<String>, name: &str) -> Result<Option<String>> {
    let Some(index) = args.iter().position(|s| s == name) else {
        return Ok(None);
    };
    args.remove(index);
    if index >= args.len() || args[index].starts_with("--") {
        return Err(error("invalid-request", "flag requires a value"));
    }
    let value = args.remove(index);
    if args.iter().any(|s| s == name) {
        return Err(error("invalid-request", "duplicate flag"));
    }
    Ok(Some(value))
}
/// Consume newline-delimited JSON with bounded frame storage and stdout reserved for replies.
fn stdio(mut handle: impl FnMut(Value) -> Option<Value>, local_service: bool) -> Result<()> {
    let mut input = std::io::BufReader::new(
        owned::Channel::new(0, Instant::now() + Duration::from_secs(30))
            .map_err(|_| error("io", "owned input unavailable"))?,
    );
    loop {
        input
            .get_mut()
            .deadline(Instant::now() + Duration::from_secs(30));
        let Some(frame) = frame(&mut input)? else {
            break;
        };
        cpu_limit()?;
        let reply = match frame.and_then(|b| {
            serde_json::from_slice::<Value>(&b)
                .map_err(|_| error("invalid-request", "invalid JSON frame"))
        }) {
            Ok(value) => handle(value),
            Err(e) => Some(if local_service {
                json!({"version":1,"id":null,"error":e})
            } else {
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":e.message}})
            }),
        };
        if let Some(reply) = reply {
            let bytes = serde_json::to_vec(&reply)
                .map_err(|_| error("io", "response serialization failed"))?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err(error("resource-limited", "response byte budget exhausted"));
            }
            owned::output(&bytes)?;
        }
    }
    Ok(())
}
/// An oversized frame is discarded through its LF without an unbounded line allocation.
fn frame(input: &mut impl BufRead) -> Result<Option<Result<Vec<u8>>>> {
    const MAX: usize = 8 * 1024 * 1024;
    let mut bytes = Vec::new();
    let mut oversized = false;
    loop {
        let available = input
            .fill_buf()
            .map_err(|_| error("io", "stdio input failed"))?;
        if available.is_empty() {
            if bytes.is_empty() && !oversized {
                return Ok(None);
            }
            break;
        }
        let end = available
            .iter()
            .position(|&b| b == b'\n')
            .map(|n| n + 1)
            .unwrap_or(available.len());
        let finished = available[end - 1] == b'\n';
        if bytes.len() + end > MAX {
            oversized = true;
            bytes.clear();
        }
        if !oversized {
            bytes.extend_from_slice(&available[..end]);
        }
        input.consume(end);
        if finished {
            break;
        }
    }
    Ok(Some(if oversized {
        Err(error("resource-limited", "stdio frame exceeds byte budget"))
    } else {
        Ok(bytes)
    }))
}

/// Versioned trusted-local request envelope. Unknown additive members are ignored.
#[derive(Deserialize)]
struct Request {
    version: u64,
    id: Value,
    #[serde(flatten)]
    operation: Operation,
}
/// Closed local operations; complete replacement inventories also express updates and removals.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
enum Operation {
    DigestWindows {
        request: digest_window::DigestWindowRequest,
    },
    Publish {
        view: ScopeView,
        #[serde(deserialize_with = "required_expected")]
        expected_generation: Option<String>,
    },
    Search {
        request: Query,
    },
    Inspect {
        scope: String,
    },
    Analyze {
        query: String,
        title: String,
        body: String,
        #[serde(default)]
        language: Option<String>,
    },
}
/// Project local requests into the same kernel used by CLI and MCP.
fn service(kernel: &Kernel, value: Value) -> Option<Value> {
    let id = value
        .get("id")
        .filter(|id| valid_id(id))
        .cloned()
        .unwrap_or(Value::Null);
    let result = (|| -> Result<Value> {
        let request: Request = serde_json::from_value(value)
            .map_err(|_| error("invalid-request", "invalid service envelope or operation"))?;
        if request.version != 1
            || !(request.id.is_number() || request.id.as_str().is_some_and(|s| s.len() <= 256))
        {
            return Err(error(
                "invalid-request",
                "unsupported service version or request id",
            ));
        }
        match request.operation {
            Operation::DigestWindows { request } => {
                serde_json::to_value(kernel.digest_windows(request)?)
                    .map_err(|_| error("io", "response serialization failed"))
            }
            Operation::Publish {
                view,
                expected_generation,
            } => Ok(json!({"generation":kernel.publish(view, expected_generation)?})),
            Operation::Search { request } => serde_json::to_value(kernel.search(request)?)
                .map_err(|_| error("io", "response serialization failed")),
            Operation::Analyze {
                query,
                title,
                body,
                language,
            } => serde_json::to_value(analyze(&query, &title, &body, language.as_deref())?)
                .map_err(|_| error("io", "response serialization failed")),
            Operation::Inspect { scope } => Ok(json!({"generation":kernel.generation(&scope)?})),
        }
    })();
    Some(match result {
        Ok(result) => json!({"version":1,"id":id,"result":result}),
        Err(error) => json!({"version":1,"id":id,"error":error}),
    })
}
/// MCP search arguments expose no path, scope, write, or index-preparation operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    mode: Mode,
    query: String,
    limit: usize,
}
/// The sole MCP tool projects the bound directory's admitted scope into the shared search path.
fn mcp(kernel: &Kernel, directory: &Directory, value: Value) -> Option<Value> {
    let id = value.get("id").filter(|id| valid_id(id)).cloned();
    let method = value.get("method").and_then(Value::as_str).unwrap_or("");
    if id.is_none() && method.starts_with("notifications/") {
        return None;
    }
    let id = id.unwrap_or(Value::Null);
    let result = (|| -> Result<Value> {
        if value.get("jsonrpc") != Some(&json!("2.0")) {
            return Err(error("invalid-request", "MCP requires JSON-RPC 2.0"));
        }
        match method {
            "initialize" => {
                let requested = value
                    .pointer("/params/protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = if ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&requested) {
                    requested
                } else {
                    "2025-06-18"
                };
                Ok(
                    json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"retrieval-kernel","version":env!("CARGO_PKG_VERSION")}}),
                )
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(
                json!({"tools":[{"name":"retrieval_search","description":"Search the startup-bound directory's prepared index and current text or identity sidecars.","inputSchema":{"type":"object","properties":{"mode":{"type":"string","enum":["lexical","exact","regex"]},"query":{"type":"string","minLength":1,"maxLength":4096},"limit":{"type":"integer","minimum":1,"maximum":1000}},"required":["mode","query","limit"],"additionalProperties":false}}]}),
            ),
            "tools/call" => {
                if value.pointer("/params/name").and_then(Value::as_str) != Some("retrieval_search")
                {
                    return Err(error("invalid-request", "unknown MCP tool"));
                }
                let args: SearchArguments = serde_json::from_value(
                    value
                        .pointer("/params/arguments")
                        .cloned()
                        .unwrap_or(Value::Null),
                )
                .map_err(|_| {
                    error(
                        "invalid-request",
                        "invalid search arguments; root is fixed at startup",
                    )
                })?;
                let result = directory.search(kernel, args.mode, args.query, args.limit);
                match result {
                    Ok(result) => Ok(
                        json!({"content":[{"type":"text","text":serde_json::to_string(&result).unwrap()}],"isError":false}),
                    ),
                    Err(error) => Ok(
                        json!({"content":[{"type":"text","text":serde_json::to_string(&error).unwrap()}],"isError":true}),
                    ),
                }
            }
            _ => Err(error("method-not-found", "unknown MCP method")),
        }
    })();
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(error) => {
            json!({"jsonrpc":"2.0","id":id,"error":{"code":if error.code == "method-not-found" {-32601} else {-32602},"message":error.message}})
        }
    })
}

/// A nullable expected generation is required, rather than silently defaulting to initial publication.
fn required_expected<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
/// Bound reflected request identifiers independently of input frames.
fn valid_id(value: &Value) -> bool {
    value.is_number() || value.as_str().is_some_and(|s| s.len() <= 256)
}
/// Enforce the executable's memory and individual derived-file ceilings before untrusted input.
///
/// One self virtual-size and resident-size summary is written to stderr after
/// those limits are installed. A failed install puts that one summary in the
/// error instead. The summary is not a ceiling and does not change enforcement.
fn process_limits() -> Result<()> {
    // Convert the file-growth ceiling to EFBIG so publication can refuse it without a core dump.
    // SAFETY: SIG_IGN installs no handler and changes only this executable's signal disposition.
    if unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_IGN) } == libc::SIG_ERR {
        return Err(error(
            "unavailable",
            "file-growth signal enforcement unavailable",
        ));
    }
    // Darwin EINVAL remains historical evidence. No blanket address-space ceiling overrides
    // the selected native budget on either OS; owned full-RSS observation is the active bound.
    set_limit(libc::RLIMIT_FSIZE, 512 * 1024 * 1024)?;
    cpu_limit()?;
    // Darwin has no public cross-process getrlimit. Report ACTUAL readback from this same
    // installed controller, not desired constants or a platform/cfg acceptance assertion.
    eprintln!("{}", native_limits_summary()?);
    eprintln!("{}", self_vm_summary("installed"));
    Ok(())
}
/// Bounded native startup witness, kept off the request/response protocol.
/// Linux acceptance cross-checks it against /proc; Darwin uses it for limits and libproc for
/// the actual worker graph/resident sample. It grants no source/execution authority.
fn native_limits_summary() -> Result<String> {
    fn read(resource: Resource) -> Result<libc::rlimit> {
        let mut limits: libc::rlimit = unsafe { std::mem::zeroed() };
        // SAFETY: this exact process and a correctly sized caller-owned native output buffer.
        if unsafe { libc::getrlimit(resource, &mut limits) } != 0 {
            return Err(error("unavailable", "native resource readback unavailable"));
        }
        Ok(limits)
    }
    let address = read(libc::RLIMIT_AS)?;
    let file = read(libc::RLIMIT_FSIZE)?;
    let cpu = read(libc::RLIMIT_CPU)?;
    Ok(format!(
        "native_limits pid={} source=getrlimit as_soft={} as_hard={} fsize_soft={} fsize_hard={} cpu_soft={} cpu_hard={}",
        std::process::id(),
        address.rlim_cur,
        address.rlim_max,
        file.rlim_cur,
        file.rlim_max,
        cpu.rlim_cur,
        cpu.rlim_max,
    ))
}
/// Give each foreground request 65 CPU seconds; the cooperative wall ceiling is 60 seconds.
fn cpu_limit() -> Result<()> {
    // SAFETY: getrusage initializes a correctly sized caller-owned structure.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return Err(error(
            "unavailable",
            &format!(
                "CPU resource enforcement unavailable {}",
                self_vm_summary("getrusage")
            ),
        ));
    }
    set_limit(
        libc::RLIMIT_CPU,
        (usage.ru_utime.tv_sec + usage.ru_stime.tv_sec).max(0) as u64 + 65,
    )
}
/// Preserve stricter inherited limits. Failure to install a limit is an explicit startup failure.
fn set_limit(resource: Resource, ceiling: u64) -> Result<()> {
    let name = match resource {
        libc::RLIMIT_AS => "RLIMIT_AS",
        libc::RLIMIT_FSIZE => "RLIMIT_FSIZE",
        libc::RLIMIT_CPU => "RLIMIT_CPU",
        _ => "unknown",
    };
    // SAFETY: getrlimit/setrlimit receive valid initialized resource structures.
    let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrlimit(resource, &mut limit) } != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error();
        return Err(error(
            "unavailable",
            &format!(
                "process resource limits unavailable: getrlimit({name}/{resource}) errno={errno:?} ceiling={ceiling} {}",
                self_vm_summary("getrlimit"),
            ),
        ));
    }
    let inherited_soft = limit.rlim_cur;
    let inherited_hard = limit.rlim_max;
    limit.rlim_cur = if resource == libc::RLIMIT_CPU {
        ceiling.min(limit.rlim_max)
    } else {
        ceiling.min(limit.rlim_cur).min(limit.rlim_max)
    };
    if unsafe { libc::setrlimit(resource, &limit) } != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error();
        return Err(error(
            "unavailable",
            &format!(
                "process resource enforcement failed: setrlimit({name}/{resource}) errno={errno:?} inherited_soft={inherited_soft} inherited_hard={inherited_hard} requested_soft={} requested_hard={} ceiling={ceiling} {}",
                limit.rlim_cur,
                limit.rlim_max,
                self_vm_summary("setrlimit"),
            ),
        ));
    }
    Ok(())
}

/// Report this process's virtual and resident sizes at one named install point.
///
/// Linux copies `VmSize` and `VmRSS` from one bounded `/proc/self/status` read.
/// proc(5) defines those `kB` tokens as 1024-byte units; the token itself is copied.
/// macOS reads `mach_task_basic_info.virtual_size` and `resident_size`, which
/// `task_info.h` defines in bytes. A missing read or return code stays unavailable.
fn self_vm_summary(point: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        linux_self_vm(point)
    }
    #[cfg(target_os = "macos")]
    {
        macos_self_vm(point)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        format!("self_vm point={point} source=none unavailable reason=unsupported")
    }
}

/// Read `VmSize` and `VmRSS` for this process. The read is one 4096-byte buffer.
#[cfg(target_os = "linux")]
fn linux_self_vm(point: &str) -> String {
    let mut file = match std::fs::File::open("/proc/self/status") {
        Ok(file) => file,
        Err(error) => {
            return format!(
                "self_vm point={point} source=/proc/self/status unavailable errno={:?}",
                error.raw_os_error()
            );
        }
    };
    let mut buffer = [0u8; 4096];
    let size = match std::io::Read::read(&mut file, &mut buffer) {
        Ok(size) => size,
        Err(error) => {
            return format!(
                "self_vm point={point} source=/proc/self/status unavailable errno={:?}",
                error.raw_os_error()
            );
        }
    };
    let text = String::from_utf8_lossy(&buffer[..size]);
    let vm_size = match proc_status_field(&text, "VmSize") {
        Ok(field) => field,
        Err(reason) => {
            return format!(
                "self_vm point={point} source=/proc/self/status unavailable field=VmSize reason={reason}"
            );
        }
    };
    let vm_rss = match proc_status_field(&text, "VmRSS") {
        Ok(field) => field,
        Err(reason) => {
            return format!(
                "self_vm point={point} source=/proc/self/status unavailable field=VmRSS reason={reason}"
            );
        }
    };
    format!(
        "self_vm point={point} source=/proc/self/status VmSize={} VmSize_unit={} VmRSS={} VmRSS_unit={}",
        vm_size.0, vm_size.1, vm_rss.0, vm_rss.1
    )
}

/// Parse one `Key:\tvalue unit` line. The unit token is returned unchanged.
#[cfg(target_os = "linux")]
fn proc_status_field(text: &str, key: &str) -> std::result::Result<(u64, String), &'static str> {
    let prefix = format!("{key}:");
    let Some(line) = text.lines().find(|line| line.starts_with(&prefix)) else {
        return Err("missing");
    };
    let mut parts = line[prefix.len()..].split_whitespace();
    let Some(raw) = parts.next() else {
        return Err("missing-value");
    };
    let Ok(value) = raw.parse::<u64>() else {
        return Err("unparsed-value");
    };
    let Some(unit) = parts.next() else {
        return Err("missing-unit");
    };
    if parts.next().is_some() {
        return Err("extra-token");
    }
    Ok((value, unit.to_string()))
}

/// Read this task's `MACH_TASK_BASIC_INFO` sizes. Units are bytes.
///
/// `task_info` returns `kern_return_t`. `KERN_SUCCESS` is 0. The count is in
/// `natural_t` words. Both 8-byte fields are read only when that count covers them.
#[cfg(target_os = "macos")]
fn macos_self_vm(point: &str) -> String {
    let mut info: libc::mach_task_basic_info = unsafe { std::mem::zeroed() };
    let mut count: libc::mach_msg_type_number_t = libc::MACH_TASK_BASIC_INFO_COUNT;
    // SAFETY: mach_task_self is this task. task_info writes the caller-owned info
    // and count. libc 0.2.190 exposes this Mach call; mach2 is not a dependency.
    #[allow(deprecated)]
    let task = unsafe { libc::mach_task_self() };
    let result = unsafe {
        libc::task_info(
            task,
            libc::MACH_TASK_BASIC_INFO,
            (&raw mut info).cast::<libc::integer_t>(),
            &raw mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return format!(
            "self_vm point={point} source=mach_task_basic_info unavailable kern_return={result}"
        );
    }
    let needed = (std::mem::size_of::<u64>() * 2 / std::mem::size_of::<libc::natural_t>()) as u32;
    if count < needed {
        return format!(
            "self_vm point={point} source=mach_task_basic_info unavailable kern_return={result} count={count}"
        );
    }
    let virtual_size = unsafe { std::ptr::addr_of!(info.virtual_size).read_unaligned() };
    let resident_size = unsafe { std::ptr::addr_of!(info.resident_size).read_unaligned() };
    format!(
        "self_vm point={point} source=mach_task_basic_info virtual_size={virtual_size} resident_size={resident_size} unit=bytes"
    )
}

/// libc uses different resource identifier types on Linux and other Unix targets.
#[cfg(target_os = "linux")]
type Resource = libc::__rlimit_resource_t;
#[cfg(not(target_os = "linux"))]
type Resource = libc::c_int;
