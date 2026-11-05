//! Opt-in loopback HTTP using the locked hyper HTTP/1 stack.
//!
//! Host and Origin are literal allowlist checks. A match forwards those header
//! bytes to the handler and does not mark the peer as a person or an owner.

use std::convert::Infallible;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request as HttpRequest, Response as HttpResponse, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use omni_service::{
    Command, ErrorCode, JobState, Response, Result as WireResult, decode_request, encode_response,
};

use crate::NativeHandler;
use crate::facts::{AccountFacts, TransportFacts, TransportKind};
use crate::limits::{self, StreamLimits};
use crate::mcp;
use crate::reply::{codec_code, error_response};

const COMMAND_DEADLINE: Duration = Duration::from_secs(5);
const COMMAND_CONNECTIONS: usize = 32;
const HEADER_LIMIT: usize = 16 * 1024;
const MAX_HEADERS: usize = 32;

type BoxError = Box<dyn Error + Send + Sync>;
type ReplyBody = http_body_util::combinators::BoxBody<Bytes, BoxError>;

pub(crate) struct Shared {
    handler: Arc<dyn NativeHandler>,
    account: Option<AccountFacts>,
    host: String,
    origin: String,
    local: SocketAddr,
    status_slots: Arc<AtomicUsize>,
    command_slots: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
}

struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Serve one accepted HTTP stream until the status-stream deadline.
pub(crate) async fn connection(
    stream: tokio::net::TcpStream,
    peer: SocketAddr,
    shared: Arc<Shared>,
) {
    let service = service_fn(move |request: HttpRequest<Incoming>| {
        let shared = Arc::clone(&shared);
        async move { Ok::<_, Infallible>(route(request, peer, shared).await) }
    });
    let deadline = StreamLimits::specified().max_duration + StreamLimits::specified().write_timeout;
    let _ = tokio::time::timeout(deadline, async {
        let io = TokioIo::new(stream);
        let connection = http1::Builder::new()
            .timer(TokioTimer::new())
            .keep_alive(false)
            .half_close(true)
            .max_headers(MAX_HEADERS)
            .max_buf_size(HEADER_LIMIT)
            .header_read_timeout(COMMAND_DEADLINE)
            .serve_connection(io, service);
        let _ = connection.await;
    })
    .await;
}

/// Build the shared state for one bound listener.
pub(crate) fn shared(
    handler: Arc<dyn NativeHandler>,
    account: Option<AccountFacts>,
    local: SocketAddr,
    stop: Arc<AtomicBool>,
) -> Arc<Shared> {
    let host = format!("127.0.0.1:{}", local.port());
    Arc::new(Shared {
        handler,
        account,
        origin: format!("http://{host}"),
        host,
        local,
        status_slots: Arc::new(AtomicUsize::new(0)),
        command_slots: Arc::new(AtomicUsize::new(0)),
        stop,
    })
}

async fn route(
    request: HttpRequest<Incoming>,
    peer: SocketAddr,
    shared: Arc<Shared>,
) -> HttpResponse<ReplyBody> {
    if shared.stop.load(Ordering::Acquire) {
        return empty(StatusCode::SERVICE_UNAVAILABLE);
    }
    if request.method() != Method::POST || request.uri().query().is_some() {
        return empty(StatusCode::BAD_REQUEST);
    }
    if request
        .headers()
        .get(hyper::header::TRANSFER_ENCODING)
        .is_some()
        || request.headers().get(hyper::header::EXPECT).is_some()
    {
        return empty(StatusCode::BAD_REQUEST);
    }
    let hosts: Vec<_> = request
        .headers()
        .get_all(hyper::header::HOST)
        .iter()
        .cloned()
        .collect();
    if hosts.len() != 1 || hosts[0].as_bytes() != shared.host.as_bytes() {
        return empty(StatusCode::FORBIDDEN);
    }
    let origins: Vec<_> = request
        .headers()
        .get_all(hyper::header::ORIGIN)
        .iter()
        .cloned()
        .collect();
    if origins.len() > 1
        || origins
            .first()
            .is_some_and(|value| value.as_bytes() != shared.origin.as_bytes())
    {
        return empty(StatusCode::FORBIDDEN);
    }
    let length = match content_length(request.headers()) {
        Ok(length) => length,
        Err(status) => return empty(status),
    };
    let facts = TransportFacts {
        kind: TransportKind::Http,
        host: Some(shared.host.clone()),
        origin: origins
            .first()
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned()),
        local: Some(shared.local),
        peer: Some(peer),
        account: shared.account,
    };
    let path = request.uri().path().to_owned();
    let body = request.into_body();
    if length > omni_service::MAX_CONTROL_FRAME_BYTES {
        return empty(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let bytes = match Limited::new(body, omni_service::MAX_CONTROL_FRAME_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            let status = if error
                .downcast_ref::<http_body_util::LengthLimitError>()
                .is_some()
            {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            };
            return empty(status);
        }
    };
    match path.as_str() {
        "/v1/command" => command(bytes, &shared, facts).await,
        "/mcp" => mcp_route(bytes, &shared, facts),
        "/v1/status-stream" => status_stream(bytes, &shared, facts),
        _ => empty(StatusCode::BAD_REQUEST),
    }
}

fn content_length(headers: &hyper::HeaderMap) -> Result<usize, StatusCode> {
    let values: Vec<_> = headers
        .get_all(hyper::header::CONTENT_LENGTH)
        .iter()
        .collect();
    if values.len() != 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let text = values[0].to_str().map_err(|_| StatusCode::BAD_REQUEST)?;
    if text.len() > 1 && text.starts_with('0') {
        return Err(StatusCode::BAD_REQUEST);
    }
    let length: usize = text.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    if length > omni_service::MAX_CONTROL_FRAME_BYTES {
        Err(StatusCode::PAYLOAD_TOO_LARGE)
    } else {
        Ok(length)
    }
}

async fn command(bytes: Bytes, shared: &Shared, facts: TransportFacts) -> HttpResponse<ReplyBody> {
    let request = match decode_request(&bytes) {
        Ok(request) => request,
        Err(error) => return empty(status_for_codec(error)),
    };
    if !acquire(&shared.command_slots, COMMAND_CONNECTIONS) {
        let response = error_response(
            request.request_id,
            ErrorCode::BudgetExhausted,
            "HTTP command connections are exhausted",
            true,
        );
        return json_response(StatusCode::OK, &response);
    }
    let _slot = Slot(Arc::clone(&shared.command_slots));
    let response = shared.handler.handle(&request, &facts);
    match encode_response(&response) {
        Ok(encoded) => json_bytes(StatusCode::OK, encoded),
        Err(error) => {
            let response = error_response(
                request.request_id,
                codec_code(error),
                "service response was refused",
                false,
            );
            json_response(StatusCode::BAD_REQUEST, &response)
        }
    }
}

fn mcp_route(bytes: Bytes, shared: &Shared, facts: TransportFacts) -> HttpResponse<ReplyBody> {
    let message: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => {
            return json_raw(
                StatusCode::BAD_REQUEST,
                br#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON-RPC request"}}"#,
            );
        }
    };
    if !acquire(&shared.command_slots, COMMAND_CONNECTIONS) {
        return empty(StatusCode::SERVICE_UNAVAILABLE);
    }
    let _slot = Slot(Arc::clone(&shared.command_slots));
    let mut session_open =
        message.get("method").and_then(|method| method.as_str()) != Some("initialize");
    let response = mcp::once(&message, &mut session_open, shared.handler.as_ref(), &facts);
    if response.is_null() {
        return empty(StatusCode::BAD_REQUEST);
    }
    match serde_json::to_vec(&response) {
        Ok(encoded) => json_bytes(StatusCode::OK, encoded),
        Err(_) => empty(StatusCode::BAD_REQUEST),
    }
}

fn status_stream(bytes: Bytes, shared: &Shared, facts: TransportFacts) -> HttpResponse<ReplyBody> {
    let request = match decode_request(&bytes) {
        Ok(request) => request,
        Err(error) => return empty(status_for_codec(error)),
    };
    if !matches!(request.command, Command::JobStatus { .. }) {
        let response = error_response(
            request.request_id,
            ErrorCode::InvalidRequest,
            "status stream accepts only job_status",
            false,
        );
        return json_response(StatusCode::BAD_REQUEST, &response);
    }
    if !acquire(
        &shared.status_slots,
        StreamLimits::specified().max_connections,
    ) {
        let response = error_response(
            request.request_id.clone(),
            ErrorCode::BudgetExhausted,
            "status stream connections are exhausted",
            true,
        );
        return json_response(StatusCode::OK, &response);
    }
    let limits = StreamLimits::specified();
    let state = StreamState {
        request,
        facts,
        handler: Arc::clone(&shared.handler),
        started: Instant::now(),
        frames: 0,
        last: None,
        slot: Slot(Arc::clone(&shared.status_slots)),
        terminal: false,
    };
    let stream = futures_util::stream::unfold(state, move |mut state| async move {
        if state.terminal {
            return None;
        }
        loop {
            let now = Instant::now();
            match limits::next_step(limits, state.started, state.frames, state.last, now, false) {
                limits::Step::Stop => return None,
                limits::Step::Wait { at } => {
                    tokio::time::sleep(at.saturating_duration_since(Instant::now())).await;
                }
                limits::Step::Deliver => {
                    let _held = &state.slot;
                    let response = state.handler.handle(&state.request, &state.facts);
                    state.terminal = terminal(&response);
                    let Ok(mut encoded) = encode_response(&response) else {
                        return None;
                    };
                    encoded.push(b'\n');
                    state.frames += 1;
                    state.last = Some(Instant::now());
                    return Some((Ok::<_, BoxError>(Frame::data(Bytes::from(encoded))), state));
                }
            }
        }
    });
    let body = StreamBody::new(stream).boxed();
    HttpResponse::builder()
        .status(StatusCode::OK)
        .header(hyper::header::CONTENT_TYPE, "application/x-ndjson")
        .header(hyper::header::CACHE_CONTROL, "no-store")
        .body(body)
        .unwrap_or_else(|_| empty(StatusCode::BAD_REQUEST))
}

fn terminal(response: &Response) -> bool {
    match &response.result {
        WireResult::Job { job } => matches!(
            job.state,
            JobState::Completed
                | JobState::Partial
                | JobState::Failed
                | JobState::Cancelled
                | JobState::Superseded
        ),
        _ => true,
    }
}

fn status_for_codec(error: omni_service::CodecError) -> StatusCode {
    match error {
        omni_service::CodecError::FrameTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        _ => StatusCode::BAD_REQUEST,
    }
}

struct StreamState {
    request: omni_service::Request,
    facts: TransportFacts,
    handler: Arc<dyn NativeHandler>,
    started: Instant,
    frames: u32,
    last: Option<Instant>,
    slot: Slot,
    terminal: bool,
}

fn json_response(status: StatusCode, response: &Response) -> HttpResponse<ReplyBody> {
    match encode_response(response) {
        Ok(bytes) => json_bytes(status, bytes),
        Err(_) => empty(StatusCode::BAD_REQUEST),
    }
}

fn json_bytes(status: StatusCode, bytes: Vec<u8>) -> HttpResponse<ReplyBody> {
    HttpResponse::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .header(hyper::header::CACHE_CONTROL, "no-store")
        .body(box_full(Bytes::from(bytes)))
        .unwrap_or_else(|_| empty(StatusCode::BAD_REQUEST))
}

fn json_raw(status: StatusCode, bytes: &'static [u8]) -> HttpResponse<ReplyBody> {
    json_bytes(status, bytes.to_vec())
}

fn empty(status: StatusCode) -> HttpResponse<ReplyBody> {
    HttpResponse::builder()
        .status(status)
        .body(box_full(Bytes::new()))
        .unwrap_or_else(|_| HttpResponse::new(box_full(Bytes::new())))
}

fn box_full(bytes: Bytes) -> ReplyBody {
    Full::new(bytes)
        .map_err(|error: Infallible| -> BoxError { match error {} })
        .boxed()
}

fn acquire(slots: &AtomicUsize, max: usize) -> bool {
    loop {
        let current = slots.load(Ordering::Acquire);
        if current >= max {
            return false;
        }
        if slots
            .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return true;
        }
    }
}
