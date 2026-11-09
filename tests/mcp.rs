//! Protocol and mutation checks through the actual stdio agent interface.
use omni_knowledge::{IndexOptions, KnowledgeBase};
#[path = "support/canonical.rs"]
mod canonical;
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

/// Exercises negotiation, tool discovery, source search, and invalid mutation rejection.
#[test]
fn stdio_tools_preserve_metadata_on_invalid_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(directory.path(), "Agent library").unwrap();
    let source = tempfile::tempdir().unwrap();
    let original = source.path().join("note.txt");
    std::fs::write(&original, "agentprotocolword").unwrap();
    assert_eq!(
        kb.import(&original, &["keep".into()]).unwrap(),
        "raw/note.txt"
    );
    // Retain the same initial authored description in the current visible fixture format.
    let (pointer, mut annotations) = canonical::pointer(&kb.root, "raw/note.txt");
    annotations["description"] = serde_yaml::Value::String("description".into());
    std::fs::write(
        pointer,
        format!("---\n{}---\n", serde_yaml::to_string(&annotations).unwrap()),
    )
    .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search","arguments":{"query":"agentprotocolword"}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"metadata","arguments":{"path":"raw/note.txt","tags":"erase"}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"metadata","arguments":{"path":"raw/note.txt","tags":[],"description":0}}}),
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"read","arguments":{"path":"../outside"}}}),
    ];
    let mut process = Command::new(env!("CARGO_BIN_EXE_omni"))
        .args(["--root", kb.root.to_str().unwrap(), "serve-mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = process.stdin.take().unwrap();
    for request in requests {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let result = process.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let responses: Vec<Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 6);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 7);
    assert_eq!(responses[2]["result"]["isError"], false);
    let search: Value = serde_json::from_str(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(search[0]["path"], "raw/note.txt");
    for response in &responses[3..] {
        assert_eq!(response["result"]["isError"], true);
    }
    let (_, annotations) = canonical::pointer(&kb.root, "raw/note.txt");
    assert_eq!(
        annotations["tags"],
        serde_yaml::to_value(vec!["keep".to_string()]).unwrap()
    );
}

/// Agent discovery must omit document bodies, while bounded reads still accept larger source files.
#[test]
fn discovery_is_compact_and_read_limits_output_instead_of_source() {
    let directory = tempfile::tempdir().unwrap();
    let kb = KnowledgeBase::init(directory.path(), "Bounded reader").unwrap();
    let source = format!(
        "# Bounded knowledge\n{}",
        "知识 retrieval 🧠\n".repeat(20000)
    );
    std::fs::write(kb.root.join("raw/long.md"), &source).unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_omni"))
        .args(["--root", kb.root.to_str().unwrap(), "serve-mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for (id, name, arguments) in [
        (1, "list", json!({})),
        (2, "read", json!({"path":"raw/long.md"})),
    ] {
        writeln!(input,"{}",json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let list: Value = serde_json::from_str(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .all(|record| record.get("text").is_none())
    );
    let read: Value = serde_json::from_str(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let text = read["text"].as_str().unwrap();
    assert!(text.starts_with("# Bounded knowledge\n"));
    assert!(text.len() <= 128 * 1024 && text.len() > 127 * 1024);
    assert_eq!(read["status"], "partial");
}
