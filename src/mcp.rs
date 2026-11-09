//! Stdio Model Context Protocol tools for local agent-driven file management and retrieval.
use crate::mcp_native::{NativeQueryHost, legacy_request};
use crate::{IndexOptions, KnowledgeBase, extract};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Existing retained-product request ceiling, excluding LF or CRLF delimiters.
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

/// Serves newline-delimited JSON-RPC on stdin/stdout; diagnostics never enter protocol stdout.
/// Each request opens the selected library afresh so external edits are observed between calls.
/// Oversized or invalid UTF-8 frames terminate the transport with fixed, input-free errors.
pub fn serve(default_root: &Path) -> Result<()> {
    let input = io::stdin();
    let mut input = input.lock();
    let mut output = io::stdout().lock();
    let mut native = None;
    while let Some(line) = read_request(&mut input)? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                writeln!(
                    output,
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON-RPC request"}})
                )?;
                output.flush()?;
                continue;
            }
        };
        let Some(id) = request.get("id") else {
            continue;
        };
        if request["method"] == "tools/call" && request["params"]["name"] == "read" {
            let attempted: Result<Option<_>> = (|| {
                let arguments = &request["params"]["arguments"];
                validate_arguments(arguments)?;
                let kb = KnowledgeBase::open(&selected_root(arguments, default_root)?)?;
                if kb.config.version == 1 {
                    return Ok(None);
                }
                let path =
                    kb.resolve(arguments["path"].as_str().context("source path missing")?)?;
                let relative = path
                    .strip_prefix(&kb.root)?
                    .to_str()
                    .context("source path is not UTF-8")?
                    .to_owned();
                if native.is_none() {
                    native = Some(NativeQueryHost::open(vec![kb.clone()], true)?);
                }
                let command = legacy_request(
                    native.as_ref().expect("actual native query host"),
                    &relative,
                    "files",
                    &[],
                    30,
                    true,
                    false,
                )?;
                Ok(Some((relative, command)))
            })();
            match attempted {
                Ok(Some((path, command))) => {
                    native
                        .as_ref()
                        .expect("actual native query host")
                        .with_read_query(&command, &path, |reply| {
                            reply.write_read_mcp(&mut output, id, &path)
                        })?;
                    continue;
                }
                Ok(None) => {} // Retain version-1 inventory/preview access without native startup.
                Err(error) => {
                    writeln!(
                        output,
                        "{}",
                        json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("{error:#}")}],"isError":true}})
                    )?;
                    output.flush()?;
                    continue;
                }
            }
        }
        if request["method"] == "tools/call" && request["params"]["name"] == "search" {
            let attempted: Result<omni_service::Request> = (|| {
                let arguments = &request["params"]["arguments"];
                validate_arguments(arguments)?;
                selected_root(arguments, default_root)?;
                if native.is_none() {
                    native = Some(NativeQueryHost::open(
                        vec![KnowledgeBase::open(default_root)?],
                        true,
                    )?);
                }
                let host = native.as_ref().expect("actual native query host");
                let tags = tags(arguments)?;
                let command = legacy_request(
                    host,
                    arguments["query"].as_str().unwrap_or(""),
                    arguments["mode"].as_str().unwrap_or("bm25"),
                    &tags,
                    arguments["limit"].as_u64().unwrap_or(30) as usize,
                    false,
                    arguments["image"].is_string(),
                )?;
                Ok(command)
            })();
            match attempted {
                Ok(command) => {
                    // Once output starts, a write/recheck failure ends this transport, never a second frame.
                    native
                        .as_ref()
                        .expect("actual native query host")
                        .with_query(&command, |reply| reply.write_mcp(&mut output, id, true))?;
                }
                Err(error) => {
                    writeln!(
                        output,
                        "{}",
                        json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("{error:#}")}],"isError":true}})
                    )?;
                    output.flush()?;
                }
            }
            continue;
        }
        if request["method"] == "tools/call"
            && matches!(
                request["params"]["name"].as_str(),
                Some("index" | "metadata" | "import")
            )
        {
            native = None;
        }
        let response = match dispatch(&request, default_root) {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err(error) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":format!("{error:#}")}})
            }
        };
        writeln!(output, "{response}")?;
        output.flush()?;
    }
    Ok(())
}

/// Acquire one bounded byte frame before UTF-8/JSON parsing; None means empty EOF.
/// Retain at most the request ceiling plus one possible CR; only LF can make that CR a delimiter.
/// A valid final EOF frame is preserved, while oversized/invalid UTF-8 input returns fixed errors.
/// Refuse an overrun without draining to a delimiter or dispatching any accepted prefix.
/// Retry interrupted reads like the former line reader; propagate other native read failures.
fn read_request(input: &mut impl BufRead) -> Result<Option<String>> {
    read_bounded(input, MAX_REQUEST_BYTES)
}
fn read_bounded(input: &mut impl BufRead, max: usize) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    let terminated = loop {
        let available = match input.fill_buf() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result.context("Read MCP request")?,
        };
        if available.is_empty() {
            break false;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.unwrap_or(available.len());
        if count > max + 1 - bytes.len() {
            bail!(if max == MAX_REQUEST_BYTES {
                "MCP request exceeds 4 MiB"
            } else {
                "MCP request exceeds 1 MiB"
            });
        }
        bytes.extend_from_slice(&available[..count]);
        if bytes.len() > max && bytes.last() != Some(&b'\r') {
            bail!(if max == MAX_REQUEST_BYTES {
                "MCP request exceeds 4 MiB"
            } else {
                "MCP request exceeds 1 MiB"
            });
        }
        input.consume(count + usize::from(newline.is_some()));
        if newline.is_some() {
            break true;
        }
    };
    if terminated && bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > max {
        bail!(if max == MAX_REQUEST_BYTES {
            "MCP request exceeds 4 MiB"
        } else {
            "MCP request exceeds 1 MiB"
        });
    }
    if bytes.is_empty() && !terminated {
        return Ok(None);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| anyhow::anyhow!("MCP request is not UTF-8"))
}

/// Routes initialization, discovery, and supported local tools without implicit provider startup.
fn dispatch(request: &Value, default_root: &Path) -> Result<Value> {
    match request["method"]
        .as_str()
        .context("MCP method is missing")?
    {
        "initialize" => Ok(
            json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"omni-knowledge","version":env!("CARGO_PKG_VERSION")}}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools":tools()})),
        "tools/call" => {
            let name = request["params"]["name"]
                .as_str()
                .context("tool name missing")?;
            let arguments = &request["params"]["arguments"];
            match call(name, arguments, default_root) {
                Ok(value) => Ok(
                    json!({"content":[{"type":"text","text":serde_json::to_string(&value)?}],"isError":false}),
                ),
                Err(error) => Ok(
                    json!({"content":[{"type":"text","text":format!("{error:#}")}],"isError":true}),
                ),
            }
        }
        method => bail!("unsupported MCP method {method}"),
    }
}

/// Describes focused tool inputs with source-linked, bounded responses.
fn tools() -> Vec<Value> {
    let root = json!({"type":"string","description":"Library path; defaults to process --root"});
    vec![
        json!({"name":"search","description":"Search indexed evidence by BM25, regex, exact tags, or local multimodal vectors.","inputSchema":{"type":"object","properties":{"root":root,"query":{"type":"string"},"mode":{"type":"string","enum":["bm25","grep","semantic","hybrid","files"]},"tags":{"type":"array","items":{"type":"string"}},"limit":{"type":"integer","minimum":1,"maximum":10000},"image":{"type":"string"}},"required":["query"]}}),
        json!({"name":"index","description":"Reconcile owned files; optional embeddings require an already running local provider.","inputSchema":{"type":"object","properties":{"root":root,"embeddings":{"type":"boolean"},"force":{"type":"boolean"}}}}),
        json!({"name":"list","description":"List indexed file metadata, including unsupported binary assets.","inputSchema":{"type":"object","properties":{"root":root}}}),
        json!({"name":"status","description":"Report published index coverage and warnings.","inputSchema":{"type":"object","properties":{"root":root}}}),
        json!({"name":"read","description":"Read bounded extracted content and metadata for an owned source.","inputSchema":{"type":"object","properties":{"root":root,"path":{"type":"string"}},"required":["path"]}}),
        json!({"name":"metadata","description":"Read or replace exact editable tags and descriptions of an owned source.","inputSchema":{"type":"object","properties":{"root":root,"path":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}},"description":{"type":"string"}},"required":["path"]}}),
        json!({"name":"import","description":"Copy a source into raw without replacing destinations or deleting originals.","inputSchema":{"type":"object","properties":{"root":root,"source":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}}},"required":["source"]}}),
    ]
}

/// Executes a tool against the explicit or default root and returns serializable evidence.
fn call(name: &str, arguments: &Value, default_root: &Path) -> Result<Value> {
    validate_arguments(arguments)?;
    let root = selected_root(arguments, default_root)?;
    let kb = KnowledgeBase::open(&root)?;
    anyhow::ensure!(
        kb.config.version == 2 || matches!(name, "list" | "read" | "status"),
        "libraries created before 7852835 are pre-release format: re-run init and re-import; use a new empty library; inventory and preview remain available"
    );
    let tags: Vec<String> = arguments["tags"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .context("tag must be a string")
                })
                .collect::<Result<_>>()
        })
        .transpose()?
        .unwrap_or_default();
    match name {
        "search" => bail!("native search delivery requires its foreground owner"),
        "index" => serde_json::to_value(kb.index(&IndexOptions {
            embeddings: arguments["embeddings"].as_bool().unwrap_or(false),
            force: arguments["force"].as_bool().unwrap_or(false),
        })?)
        .map_err(Into::into),
        "list" => Ok(Value::Array(
            kb.list()?
                .iter()
                .map(crate::FileRecord::metadata_summary)
                .collect(),
        )),
        "status" => kb.status(),
        "read" => {
            let path = arguments["path"].as_str().context("source path missing")?;
            let mut extracted = extract::extract(&kb.resolve(path)?, crate::MAX_TEXT_BYTES)?;
            limit_read(&mut extracted);
            let metadata = kb.metadata(path)?;
            Ok(
                json!({"path":path,"text":extracted.text,"status":extracted.status,"warnings":extracted.warnings,"metadata":metadata}),
            )
        }
        "metadata" => {
            let path = arguments["path"].as_str().context("source path missing")?;
            let current = kb.metadata(path)?;
            if arguments.get("tags").is_some() || arguments.get("description").is_some() {
                kb.set_metadata(
                    path,
                    if arguments.get("tags").is_some() {
                        &tags
                    } else {
                        &current.tags
                    },
                    arguments["description"].as_str(),
                )?;
            }
            serde_json::to_value(kb.metadata(path)?).map_err(Into::into)
        }
        "import" => Ok(
            json!({"path":kb.import(Path::new(arguments["source"].as_str().context("import source missing")?),&tags)?,"hint":"run index after imports to publish new search coverage"}),
        ),
        _ => bail!("unknown tool {name}"),
    }
}

/// Keep both retained and native MCP previews within the same UTF-8 output bound.
pub(crate) fn limit_read(extracted: &mut extract::Extraction) {
    if extracted.text.len() > 128 * 1024 {
        let mut end = 128 * 1024;
        while !extracted.text.is_char_boundary(end) {
            end -= 1;
        }
        extracted.text.truncate(end);
        extracted.status = "partial".into();
        extracted.warnings.push(
            "Read output limited to 128 KiB; use the original file or CLI conversion for the rest"
                .into(),
        );
    }
}

fn validate_arguments(arguments: &Value) -> Result<()> {
    if !arguments.is_object() && !arguments.is_null() {
        bail!("tool arguments must be an object");
    }
    for field in [
        "root",
        "path",
        "source",
        "query",
        "mode",
        "description",
        "image",
    ] {
        if let Some(value) = arguments.get(field)
            && !value.is_string()
        {
            bail!("{field} must be a string");
        }
    }
    if let Some(value) = arguments.get("tags")
        && !value.is_array()
    {
        bail!("tags must be an array of strings");
    }
    if let Some(value) = arguments.get("limit")
        && value.as_u64().is_none_or(|n| n == 0 || n > 10000)
    {
        bail!("limit must be an integer from 1 through 10000");
    }
    for field in ["embeddings", "force"] {
        if let Some(value) = arguments.get(field)
            && !value.is_boolean()
        {
            bail!("{field} must be a boolean");
        }
    }
    Ok(())
}
fn tags(arguments: &Value) -> Result<Vec<String>> {
    arguments["tags"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .context("tag must be a string")
                })
                .collect()
        })
        .transpose()
        .map(Option::unwrap_or_default)
}
fn selected_root(arguments: &Value, default_root: &Path) -> Result<PathBuf> {
    let root = arguments["root"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root.to_path_buf());
    anyhow::ensure!(
        KnowledgeBase::open(&root)?.root == KnowledgeBase::open(default_root)?.root,
        "tool root is not the owner-selected library"
    );
    Ok(root)
}

/// Serve the canonical MCP revision through the same foreground read/write loop and native owners.
/// No daemon Response seam can detach the real query controller from its final consumer.
pub fn serve_canonical(default_root: &Path, recursive: bool) -> Result<()> {
    use omni_service as wire;
    let root = KnowledgeBase::open(default_root)?;
    let mut libraries = vec![root.clone()];
    if recursive {
        libraries.extend(root.children()?);
    }
    let host = NativeQueryHost::open(libraries, true)?;
    // The existing process account owns the whole concurrently live ingress parse window.
    // Reserve before reading/decoding JSON; the SDK's separately retained frame remains its own loan.
    let _ingress = match knowledge_retrieval::Work::reserve(
        knowledge_core::jobs::Reservation {
            memory_bytes: (wire::MAX_CONTROL_FRAME_BYTES as u64) * 32,
            temporary_bytes: 0,
        },
        knowledge_core::admission::Origin::BeforeVerification,
    )? {
        knowledge_core::admission::Decision::Ready(work) => work,
        knowledge_core::admission::Decision::Waiting(_) => {
            bail!("MCP ingress is waiting for memory")
        }
    };
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    let mut open = false;
    while let Some(line) = read_bounded(&mut input, wire::MAX_CONTROL_FRAME_BYTES)? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                write_json(
                    &mut output,
                    &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON-RPC request"}}),
                )?;
                continue;
            }
        };
        let Some(id) = request.get("id") else {
            continue;
        };
        let mut failure = None;
        let result = match request["method"].as_str() {
            Some("initialize") => {
                if request["params"]["protocolVersion"] != "2025-11-25" {
                    failure = Some(
                        json!({"code":-32602,"message":"MCP protocol revision is not supported"}),
                    );
                    json!(null)
                } else {
                    open = true;
                    json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"omni-knowledge","version":env!("CARGO_PKG_VERSION")}})
                }
            }
            _ if !open => {
                failure = Some(json!({"code":-32600,"message":"MCP session is not initialized"}));
                json!(null)
            }
            Some("ping") => json!({}),
            Some("tools/list") => {
                json!({"tools":knowledge_daemon::mcp_tool_names().map(|name|json!({"name":name,"description":"Frozen native service command fields excluding op","inputSchema":{"type":"object"}})).collect::<Vec<_>>()})
            }
            Some("tools/call") => match canonical_command(&request) {
                Ok(command) => {
                    if matches!(command.command, wire::Command::Query { .. }) {
                        host.with_query(&command, |reply| reply.write_mcp(&mut output, id, false))?;
                    } else {
                        host.write_command_mcp(&command, &mut output, id)?;
                    }
                    continue;
                }
                Err(_) => {
                    failure = Some(
                        json!({"code":-32602,"message":"MCP tool arguments are not a service command"}),
                    );
                    json!(null)
                }
            },
            _ => {
                failure = Some(json!({"code":-32601,"message":"MCP method is not supported"}));
                json!(null)
            }
        };
        write_json(
            &mut output,
            &match failure {
                Some(error) => json!({"jsonrpc":"2.0","id":id,"error":error}),
                None => json!({"jsonrpc":"2.0","id":id,"result":result}),
            },
        )?;
    }
    Ok(())
}
fn write_json(output: &mut impl Write, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}
fn canonical_command(request: &Value) -> Result<omni_service::Request> {
    let name = request["params"]["name"]
        .as_str()
        .context("tool name missing")?;
    anyhow::ensure!(
        knowledge_daemon::mcp_tool_names().any(|candidate| candidate == name),
        "tool not in catalog"
    );
    let mut arguments = request["params"]
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let fields = arguments
        .as_object_mut()
        .context("tool arguments must be object")?;
    anyhow::ensure!(!fields.contains_key("op"), "tool cannot set op");
    fields.insert("op".into(), json!(name));
    let id = match &request["id"] {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => "mcp".into(),
    };
    omni_service::decode_request(&serde_json::to_vec(
        &json!({"wire_version":1,"request_id":id,"command":arguments}),
    )?)
    .map_err(Into::into)
}
