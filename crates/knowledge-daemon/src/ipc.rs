//! Newline-delimited service envelopes and the MCP proxy that speaks them.
//!
//! The proxy copies a decoded command. It does not add an owner, confirmation, or exposure field.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use omni_service::{Request, Response, decode_request, encode_response};
use serde_json::{Value, json};

use crate::NativeHandler;
use crate::catalog::{command_op, mcp_visible};
use crate::facts::{TransportFacts, TransportKind};
use crate::frame::read_frame;
use crate::mcp;

/// Serve IPC frames until EOF.
///
/// A frame that fails the codec ends the connection before any handler call.
/// EOF after a complete response is the end of input, not cancellation of a job.
pub fn serve<R, W>(
    input: &mut R,
    output: &mut W,
    handler: &dyn NativeHandler,
    facts: &TransportFacts,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    while let Some(frame) = read_frame(input, omni_service::MAX_CONTROL_FRAME_BYTES)? {
        if frame.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let request = decode_request(&frame)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid service frame"))?;
        let response = handler.handle(&request, facts);
        let line = encode_response(&response).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "service response is not encodable",
            )
        })?;
        output.write_all(&line)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

/// Proxy one MCP stdio session to a daemon IPC address.
pub fn proxy<R, W>(input: &mut R, output: &mut W, daemon: SocketAddr) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let socket = TcpStream::connect(daemon)?;
    socket.set_nodelay(true)?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut writer = socket.try_clone()?;
    let mut reader = BufReader::new(socket);
    let mut open = false;
    while let Some(line) = read_frame(input, omni_service::MAX_CONTROL_FRAME_BYTES)? {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let message: Value = match serde_json::from_slice(&line) {
            Ok(value) => value,
            Err(_) => {
                write_json(
                    output,
                    &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON-RPC request"}}),
                )?;
                continue;
            }
        };
        if message.get("id").is_none() {
            continue;
        }
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        if message.get("method").and_then(Value::as_str) == Some("tools/call") && open {
            match call_daemon(&message, &mut writer, &mut reader) {
                Ok(value) => write_json(output, &json!({"jsonrpc":"2.0","id":id,"result":value}))?,
                Err(error) => write_json(
                    output,
                    &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":error}}),
                )?,
            }
            continue;
        }
        let local = Rejecting;
        let facts = TransportFacts {
            kind: TransportKind::McpStdio,
            host: None,
            origin: None,
            local: None,
            peer: None,
            account: None,
        };
        let response = mcp::once(&message, &mut open, &local, &facts);
        if response.is_null() {
            continue;
        }
        write_json(output, &response)?;
    }
    Ok(())
}

struct Rejecting;

impl NativeHandler for Rejecting {
    fn handle(&self, request: &Request, _facts: &TransportFacts) -> Response {
        crate::reply::error_response(
            request.request_id.clone(),
            omni_service::ErrorCode::InvalidRequest,
            "MCP proxy does not execute tools locally",
            false,
        )
    }
}

fn call_daemon(
    message: &Value,
    writer: &mut impl Write,
    reader: &mut impl BufRead,
) -> Result<Value, &'static str> {
    let name = message
        .pointer("/params/name")
        .and_then(Value::as_str)
        .ok_or("MCP tool name is missing")?;
    if !mcp_visible(name) {
        return Err("MCP tool is not in the catalog");
    }
    let arguments = message
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() || arguments.get("op").is_some() {
        return Err("MCP tool arguments are not a service command");
    }
    let mut command = arguments;
    let Some(object) = command.as_object_mut() else {
        return Err("MCP tool arguments are not a service command");
    };
    object.insert("op".to_owned(), Value::String(name.to_owned()));
    let request_id = match message.get("id") {
        Some(Value::String(text)) if !text.is_empty() => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => "mcp".to_owned(),
    };
    let bytes = serde_json::to_vec(&json!({
        "wire_version": 1,
        "request_id": request_id,
        "command": command
    }))
    .map_err(|_| "MCP tool arguments are not encodable")?;
    let decoded =
        decode_request(&bytes).map_err(|_| "MCP tool arguments are not a service command")?;
    if command_op(&decoded.command) != name {
        return Err("MCP tool arguments are not a service command");
    }
    writer
        .write_all(&bytes)
        .map_err(|_| "daemon IPC write failed")?;
    writer
        .write_all(b"\n")
        .map_err(|_| "daemon IPC write failed")?;
    writer.flush().map_err(|_| "daemon IPC write failed")?;
    let frame = read_frame(reader, omni_service::MAX_CONTROL_FRAME_BYTES)
        .map_err(|_| "daemon IPC read failed")?
        .ok_or("daemon IPC closed")?;
    let response: Response = serde_json::from_slice(&frame)
        .map_err(|_| "daemon IPC response is not a service result")?;
    let text =
        serde_json::to_string(&response).map_err(|_| "daemon IPC response is not encodable")?;
    let structured =
        serde_json::to_value(&response).map_err(|_| "daemon IPC response is not encodable")?;
    let is_error = matches!(response.result, omni_service::Result::Error { .. });
    Ok(json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": structured,
        "isError": is_error
    }))
}

fn write_json(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value).map_err(io::Error::other)?;
    output.write_all(b"\n")?;
    output.flush()
}
