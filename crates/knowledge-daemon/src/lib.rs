//! Local daemon transport for the frozen version-1 service codec.
//!
//! Startup binds loopback listeners and stops them on drop. A native handler is
//! required. This crate does not decide exposure, confirmation, or storage, and
//! it does not invent a process-wide memory pool.
//!
//! The synthetic handlers used by this crate's tests prove framing and delivery
//! only. They are not authority and they are not a storage result.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod catalog;
mod facts;
mod frame;
mod http;
mod ipc;
mod limits;
mod mcp;
mod reply;

use std::io::{self, BufRead, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

pub use catalog::{command_op, mcp_tool_names};
pub use facts::{AccountFacts, TransportFacts, TransportKind};
pub use limits::StreamLimits;
use omni_service::{Request, Response};

/// The product binary has no native authority handler bound.
#[derive(Debug)]
pub struct UnboundHandler;

impl std::fmt::Display for UnboundHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("native authority handler is not bound")
    }
}

impl std::error::Error for UnboundHandler {}

/// Trusted native seam. The transport calls it only after a frame decodes.
pub trait NativeHandler: Send + Sync {
    /// Observe one decoded request and the transport facts that carried it.
    fn handle(&self, request: &Request, facts: &TransportFacts) -> Response;
}

/// Loopback HTTP and IPC listeners owned by the starting thread.
pub struct Daemon {
    http: SocketAddr,
    ipc: SocketAddr,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl Daemon {
    /// Bind both listeners and serve `handler` until drop.
    pub fn start(
        handler: Arc<dyn NativeHandler>,
        account: Option<AccountFacts>,
    ) -> io::Result<Self> {
        let http_listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let ipc_listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        http_listener.set_nonblocking(false)?;
        ipc_listener.set_nonblocking(false)?;
        let http = http_listener.local_addr()?;
        let ipc = ipc_listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let shared = http::shared(Arc::clone(&handler), account, http, Arc::clone(&stop));
        let stop_http = Arc::clone(&stop);
        let stop_ipc = Arc::clone(&stop);
        let accept = std::thread::Builder::new()
            .name("knowledge-daemon".to_owned())
            .spawn(move || {
                let workers = Arc::new(std::sync::Mutex::new(Vec::<JoinHandle<()>>::new()));
                let http_thread = accept_http(
                    http_listener,
                    Arc::clone(&shared),
                    Arc::clone(&stop_http),
                    Arc::clone(&workers),
                );
                let ipc_thread = accept_ipc(
                    ipc_listener,
                    handler,
                    account,
                    ipc,
                    stop_ipc,
                    Arc::clone(&workers),
                );
                let _ = http_thread.join();
                let _ = ipc_thread.join();
                let mut guard = workers.lock().unwrap_or_else(|poison| poison.into_inner());
                for worker in guard.drain(..) {
                    let _ = worker.join();
                }
            })?;
        Ok(Self {
            http,
            ipc,
            stop,
            accept: Some(accept),
        })
    }

    /// Bound HTTP address. The Host allowlist is exactly `127.0.0.1` and this port.
    pub fn http_addr(&self) -> SocketAddr {
        self.http
    }

    /// Bound IPC address.
    pub fn ipc_addr(&self) -> SocketAddr {
        self.ipc
    }

    /// Literal Host header for this HTTP listener.
    pub fn allowed_host(&self) -> String {
        format!("127.0.0.1:{}", self.http.port())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.http, Duration::from_millis(200));
        let _ = TcpStream::connect_timeout(&self.ipc, Duration::from_millis(200));
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

fn accept_http(
    listener: TcpListener,
    shared: Arc<http::Shared>,
    stop: Arc<AtomicBool>,
    workers: Arc<std::sync::Mutex<Vec<JoinHandle<()>>>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let shared = Arc::clone(&shared);
                    let worker = std::thread::spawn(move || {
                        if stream.set_nodelay(true).is_err()
                            || stream.set_nonblocking(true).is_err()
                        {
                            return;
                        }
                        let runtime = match tokio::runtime::Builder::new_current_thread()
                            .enable_time()
                            .enable_io()
                            .build()
                        {
                            Ok(runtime) => runtime,
                            Err(_) => return,
                        };
                        runtime.block_on(async move {
                            let stream = match tokio::net::TcpStream::from_std(stream) {
                                Ok(stream) => stream,
                                Err(_) => return,
                            };
                            http::connection(stream, peer, shared).await;
                        });
                    });
                    push_worker(&workers, worker);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    })
}

fn accept_ipc(
    listener: TcpListener,
    handler: Arc<dyn NativeHandler>,
    account: Option<AccountFacts>,
    local: SocketAddr,
    stop: Arc<AtomicBool>,
    workers: Arc<std::sync::Mutex<Vec<JoinHandle<()>>>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let handler = Arc::clone(&handler);
                    let worker = std::thread::spawn(move || {
                        let facts = TransportFacts {
                            kind: TransportKind::Ipc,
                            host: None,
                            origin: None,
                            local: Some(local),
                            peer: Some(peer),
                            account,
                        };
                        let mut reader = std::io::BufReader::new(match stream.try_clone() {
                            Ok(stream) => stream,
                            Err(_) => return,
                        });
                        let mut writer = stream;
                        let _ = ipc::serve(&mut reader, &mut writer, handler.as_ref(), &facts);
                    });
                    push_worker(&workers, worker);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    })
}

fn push_worker(workers: &std::sync::Mutex<Vec<JoinHandle<()>>>, worker: JoinHandle<()>) {
    if let Ok(mut guard) = workers.lock() {
        guard.retain(|worker| !worker.is_finished());
        guard.push(worker);
    }
}

/// Serve MCP 2025-11-25 on a stdio pair until EOF.
pub fn serve_mcp<R, W>(
    input: &mut R,
    output: &mut W,
    handler: &dyn NativeHandler,
    account: Option<AccountFacts>,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    mcp::serve(input, output, handler, &TransportFacts::mcp_stdio(account))
}

/// Serve newline-delimited service envelopes until EOF.
pub fn serve_ipc<R, W>(
    input: &mut R,
    output: &mut W,
    handler: &dyn NativeHandler,
    facts: &TransportFacts,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    ipc::serve(input, output, handler, facts)
}

/// Proxy MCP stdio to a daemon IPC address.
pub fn proxy_mcp<R, W>(input: &mut R, output: &mut W, daemon: SocketAddr) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    ipc::proxy(input, output, daemon)
}

/// Format the bound account, or the explicit unbound fact.
pub fn format_account(account: Option<AccountFacts>) -> String {
    match account {
        None => "process account not bound".to_owned(),
        Some(account) => format!(
            "memory_budget_bytes={} memory_reserved_bytes={} storage_budget_bytes={} storage_reserved_bytes={} waiting_for_memory={}",
            account.memory_budget_bytes,
            account.memory_reserved_bytes,
            account.storage_budget_bytes,
            account.storage_reserved_bytes,
            account.waiting_for_memory
        ),
    }
}

/// Refuse product startup when the native handler is absent.
pub fn require_handler<T>(handler: Option<T>) -> Result<T, UnboundHandler> {
    handler.ok_or(UnboundHandler)
}
