//! Stdio Model Context Protocol tools for local agent-driven file management and retrieval.
use crate::{IndexOptions, KnowledgeBase, SearchMode, SearchOptions, extract};
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
        if count > MAX_REQUEST_BYTES + 1 - bytes.len() {
            bail!("MCP request exceeds 4 MiB");
        }
        bytes.extend_from_slice(&available[..count]);
        if bytes.len() > MAX_REQUEST_BYTES && bytes.last() != Some(&b'\r') {
            bail!("MCP request exceeds 4 MiB");
        }
        input.consume(count + usize::from(newline.is_some()));
        if newline.is_some() {
            break true;
        }
    };
    if terminated && bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > MAX_REQUEST_BYTES {
        bail!("MCP request exceeds 4 MiB");
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
    let root = arguments["root"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root.to_path_buf());
    let kb = KnowledgeBase::open(&root)?;
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
        "search" => {
            let mode = match arguments["mode"].as_str().unwrap_or("bm25") {
                "bm25" => SearchMode::Bm25,
                "grep" => SearchMode::Grep,
                "semantic" => SearchMode::Semantic,
                "hybrid" => SearchMode::Hybrid,
                "files" => SearchMode::Files,
                _ => bail!("unknown search mode"),
            };
            let limit = arguments["limit"].as_u64().unwrap_or(30) as usize;
            serde_json::to_value(kb.search(&SearchOptions {
                query: arguments["query"].as_str().unwrap_or("").into(),
                mode,
                tags,
                limit,
                case_sensitive: false,
                query_image: arguments["image"].as_str().map(PathBuf::from),
            })?)
            .map_err(Into::into)
        }
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
            if extracted.text.len() > 128 * 1024 {
                let mut end = 128 * 1024;
                while !extracted.text.is_char_boundary(end) {
                    end -= 1;
                }
                extracted.text.truncate(end);
                extracted.status = "partial".into();
                extracted.warnings.push("Read output limited to 128 KiB; use the original file or CLI conversion for the rest".into());
            }
            Ok(
                json!({"path":path,"text":extracted.text,"status":extracted.status,"warnings":extracted.warnings,"metadata":kb.metadata(path)?}),
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
