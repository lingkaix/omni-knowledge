//! MCP 2025-11-25 JSON-RPC over a byte sink.
//!
//! Tool names are frozen command names that the service owner places in the MCP catalog.
//! Arguments are those command fields. The codec accepts or refuses them. This module does
//! not approve exposure, confirmation, or a selected library.

use std::io::{self, BufRead, Write};

use omni_service::{Response, decode_request};
use serde_json::{Value, json};

use crate::NativeHandler;
use crate::catalog::{command_op, mcp_tool_names, mcp_visible};
use crate::facts::TransportFacts;
use crate::frame::read_frame;

const PROTOCOL: &str = "2025-11-25";

/// Serve newline-delimited MCP until EOF.
///
/// Notifications are consumed and not answered. A framing failure ends the session
/// after one JSON-RPC error when an id is known. The handler runs only for a
/// catalog tool whose arguments decode as that command.
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
    let mut open = false;
    while let Some(line) = read_frame(input, omni_service::MAX_CONTROL_FRAME_BYTES)? {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let request: Value = match serde_json::from_slice(&line) {
            Ok(value) => value,
            Err(_) => {
                write_value(
                    output,
                    &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON-RPC request"}}),
                )?;
                continue;
            }
        };
        if request.get("id").is_none() {
            continue;
        }
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let response = match request.get("method").and_then(Value::as_str) {
            Some("initialize") => initialize(&request, &mut open),
            Some("tools/list") if open => tools_list(),
            Some("tools/call") if open => tools_call(&request, handler, facts),
            Some("ping") if open => json!({}),
            Some(_) if !open => {
                json!({"code":-32600,"message":"MCP session is not initialized"})
            }
            _ => json!({"code":-32601,"message":"MCP method is not supported"}),
        };
        let message = if response.get("code").is_some() {
            json!({"jsonrpc":"2.0","id":id,"error":response})
        } else {
            json!({"jsonrpc":"2.0","id":id,"result":response})
        };
        write_value(output, &message)?;
    }
    Ok(())
}

fn initialize(request: &Value, open: &mut bool) -> Value {
    let version = request
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str);
    if version != Some(PROTOCOL) {
        return json!({"code":-32602,"message":"MCP protocol revision is not supported"});
    }
    *open = true;
    json!({
        "protocolVersion": PROTOCOL,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": "knowledge-daemon", "version": env!("CARGO_PKG_VERSION")}
    })
}

fn tools_list() -> Value {
    let tools: Vec<Value> = mcp_tool_names()
        .map(|name| {
            json!({
                "name": name,
                "description": "Service command fields excluding op. The frozen codec validates them.",
                "inputSchema": {"type": "object"}
            })
        })
        .collect();
    json!({"tools": tools})
}

fn tools_call(request: &Value, handler: &dyn NativeHandler, facts: &TransportFacts) -> Value {
    let name = request.pointer("/params/name").and_then(Value::as_str);
    let Some(name) = name else {
        return json!({"code":-32602,"message":"MCP tool name is missing"});
    };
    if !mcp_visible(name) {
        return json!({"code":-32602,"message":"MCP tool is not in the catalog"});
    }
    let arguments = request
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return json!({"code":-32602,"message":"MCP tool arguments must be an object"});
    }
    if arguments.get("op").is_some() {
        return json!({"code":-32602,"message":"MCP tool arguments cannot set op"});
    }
    let mut command = arguments;
    let Some(object) = command.as_object_mut() else {
        return json!({"code":-32602,"message":"MCP tool arguments must be an object"});
    };
    object.insert("op".to_owned(), Value::String(name.to_owned()));
    let request_id = request_id(request.get("id"));
    let envelope = json!({
        "wire_version": 1,
        "request_id": request_id,
        "command": command
    });
    let bytes = match serde_json::to_vec(&envelope) {
        Ok(bytes) => bytes,
        Err(_) => return json!({"code":-32602,"message":"MCP tool arguments are not encodable"}),
    };
    let decoded = match decode_request(&bytes) {
        Ok(decoded) => decoded,
        Err(_) => {
            return json!({"code":-32602,"message":"MCP tool arguments are not a service command"});
        }
    };
    if command_op(&decoded.command) != name {
        return json!({"code":-32602,"message":"MCP tool arguments are not a service command"});
    }
    let response = handler.handle(&decoded, facts);
    tool_result(&response)
}

fn request_id(id: Option<&Value>) -> String {
    match id {
        Some(Value::String(text)) if !text.is_empty() => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => "mcp".to_owned(),
    }
}

fn tool_result(response: &Response) -> Value {
    let encoded = serde_json::to_value(response).unwrap_or(Value::Null);
    let text = serde_json::to_string(response).unwrap_or_else(|_| "{}".to_owned());
    let is_error = matches!(response.result, omni_service::Result::Error { .. });
    json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": encoded,
        "isError": is_error
    })
}

fn write_value(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value).map_err(io::Error::other)?;
    output.write_all(b"\n")?;
    output.flush()
}

/// Run one already-buffered MCP message and return the JSON-RPC object.
pub fn once(
    message: &Value,
    open: &mut bool,
    handler: &dyn NativeHandler,
    facts: &TransportFacts,
) -> Value {
    if message.get("id").is_none() {
        return Value::Null;
    }
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let response = match message.get("method").and_then(Value::as_str) {
        Some("initialize") => initialize(message, open),
        Some("tools/list") if *open => tools_list(),
        Some("tools/call") if *open => tools_call(message, handler, facts),
        Some("ping") if *open => json!({}),
        Some(_) if !*open => json!({"code":-32600,"message":"MCP session is not initialized"}),
        _ => json!({"code":-32601,"message":"MCP method is not supported"}),
    };
    if response.get("code").is_some() {
        json!({"jsonrpc":"2.0","id":id,"error":response})
    } else {
        json!({"jsonrpc":"2.0","id":id,"result":response})
    }
}
