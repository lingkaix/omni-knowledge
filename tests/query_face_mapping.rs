//! Real cold CLI/MCP callers mapped to native labelled Query results and refusal data.
use omni_knowledge::{CONFIG_FILE, KnowledgeBase};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// One exclusively owned process environment; no real home, cache or library is contacted.
struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    id: String,
}
impl Fixture {
    /// Seed through the actual legacy CLI init, then add sources WITHOUT a legacy reindex.
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        for name in ["process-home", "os-cache", "temporary"] {
            fs::create_dir(directory.path().join(name)).unwrap();
        }
        let root = directory.path().join("library");
        let mut fixture = Self {
            directory,
            root,
            id: String::new(),
        };
        fixture.initialize(&fixture.root);
        let kb = KnowledgeBase::open(&fixture.root).unwrap();
        fixture.id = kb.config.library_id.clone().unwrap();
        for (name, tags, body) in [
            ("alpha", "AlphaTag, Both", "facealpha\nExactCase needle\n"),
            ("beta", "AlphaTag", "facebeta\nExactCase needle\n"),
            (
                "prefix",
                "AlphaTagSuffix, Both",
                "facealpha\nExactCase needle\n",
            ),
            ("lower", "alphatag, Both", "facelower\nexactcase needle\n"),
        ] {
            fs::write(
                fixture.root.join(format!("pages/face-{name}.md")),
                format!("---\ntags: [{tags}]\n---\n{body}"),
            )
            .unwrap();
        }
        fixture
    }
    /// Every product child uses the same fixture-selected namespace and isolated OS directories.
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omni"));
        self.environment(&mut command);
        command
    }
    /// Apply fixture isolation to the real binary or an exact public-SDK test subprocess.
    fn environment(&self, command: &mut Command) {
        command
            .env("OMNI_CACHE_DIR", self.directory.path().join("native-cache"))
            .env("HOME", self.directory.path().join("process-home"))
            .env("XDG_CACHE_HOME", self.directory.path().join("os-cache"))
            .env("TMPDIR", self.directory.path().join("temporary"));
    }
    /// Create a real independently identified home and explicitly omit any model from setup.
    fn initialize(&self, root: &Path) {
        let output = self
            .command()
            .arg("init")
            .arg(root)
            .args(["--name", "Synthetic query face"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "init: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let marker = root.join(CONFIG_FILE);
        let mut document: Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
        document["embedding"] = Value::Null;
        fs::write(marker, serde_json::to_vec(&document).unwrap()).unwrap();
    }
    /// Execute one cold search and require the full public labelled result shape.
    fn search(&self, arguments: &[&str]) -> Value {
        let output = self
            .command()
            .arg("--root")
            .arg(&self.root)
            .args(["search"])
            .args(arguments)
            .arg("--canonical")
            .arg("--json")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "search {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: omni_service::SearchResult = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| {
                panic!(
                    "CLI must emit labelled SearchResult: {error}; {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        serde_json::to_value(result).unwrap()
    }
}

/// Extract path data while checking every group/file remains labelled with its actual library.
fn paths(search: &Value, id: &str) -> BTreeSet<String> {
    let group = search["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["library_id"] == id)
        .expect("actual selected library group");
    assert!(
        group["generation_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert_eq!(group["coverage"]["enumeration"], "complete");
    let files = group["files"].as_array().unwrap();
    assert_eq!(
        group["returned_files"].as_u64().unwrap(),
        files.len() as u64
    );
    files
        .iter()
        .map(|file| {
            assert_eq!(file["source"]["library_id"], id);
            file["source"]["path"].as_str().unwrap().to_owned()
        })
        .collect()
}

/// Compare sets without making cross-library score or incidental ranking assumptions.
fn expected(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Cold ordinary CLI searches preserve whole-tag AND, explicit case and legacy grep=regex.
#[test]
fn cold_cli_query_preserves_labels_tags_case_and_grep() {
    let fixture = Fixture::new();
    let insensitive = fixture.search(&[
        "face-", "--mode", "files", "--tag", "alphatag", "--tag", "Both", "--limit", "5",
    ]);
    assert_eq!(insensitive["applied"]["methods"], json!(["files"]));
    assert_eq!(
        insensitive["applied"]["required_tags"],
        json!(["alphatag", "Both"])
    );
    assert_eq!(
        paths(&insensitive, &fixture.id),
        expected(&["pages/face-alpha.md", "pages/face-lower.md"])
    );
    let sensitive = fixture.search(&[
        "face-",
        "--mode",
        "files",
        "--tag",
        "alphatag",
        "--tag",
        "Both",
        "--case-sensitive",
        "--limit",
        "5",
    ]);
    assert_eq!(sensitive["applied"]["case_sensitive"], true);
    assert_eq!(
        paths(&sensitive, &fixture.id),
        expected(&["pages/face-lower.md"])
    );
    let regex = fixture.search(&[
        "^ExactCase needle$",
        "--mode",
        "grep",
        "--tag",
        "AlphaTag",
        "--tag",
        "Both",
        "--case-sensitive",
        "--limit",
        "5",
    ]);
    assert_eq!(regex["applied"]["methods"], json!(["regex"]));
    assert_eq!(regex["applied"]["keywords"], json!(["^ExactCase needle$"]));
    assert_eq!(
        paths(&regex, &fixture.id),
        expected(&["pages/face-alpha.md"])
    );
    assert!(
        regex["groups"][0]["files"][0]["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|evidence| evidence["method"] == "regex" && evidence["kind"] == "verified_match")
    );
    let lexical = fixture.search(&[
        "facealpha",
        "--mode",
        "bm25",
        "--tag",
        "AlphaTag",
        "--tag",
        "Both",
        "--limit",
        "5",
    ]);
    assert_eq!(
        paths(&lexical, &fixture.id),
        expected(&["pages/face-alpha.md"])
    );
    let legacy = fixture
        .command()
        .arg("--root")
        .arg(&fixture.root)
        .args([
            "search",
            "facealpha",
            "--mode",
            "bm25",
            "--tag",
            "AlphaTag",
            "--tag",
            "Both",
            "--limit",
            "5",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        legacy.status.success(),
        "default CLI array: {}",
        String::from_utf8_lossy(&legacy.stderr)
    );
    let hits: Value = serde_json::from_slice(&legacy.stdout).unwrap();
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["path"], "pages/face-alpha.md");

    let child = fixture.root.join("nested");
    fixture.initialize(&child);
    fs::write(
        child.join("pages/face-child.md"),
        "---\ntags: [AlphaTag, Both]\n---\nfacechild\n",
    )
    .unwrap();
    let child_id = KnowledgeBase::open(&child)
        .unwrap()
        .config
        .library_id
        .unwrap();
    let recursive = fixture.search(&[
        "face-",
        "--mode",
        "files",
        "--tag",
        "alphatag",
        "--tag",
        "Both",
        "--recursive",
        "--limit",
        "5",
    ]);
    assert_eq!(recursive["groups"].as_array().unwrap().len(), 2);
    assert_eq!(
        paths(&recursive, &fixture.id),
        expected(&["pages/face-alpha.md", "pages/face-lower.md"])
    );
    assert_eq!(
        paths(&recursive, &child_id),
        expected(&["pages/face-child.md"])
    );
}

/// Real stdio child plus bounded response acquisition; this carries no substitute service handler.
struct Mcp {
    child: Option<Child>,
    input: Option<ChildStdin>,
    replies: Receiver<std::io::Result<String>>,
    reader: Option<std::thread::JoinHandle<()>>,
    stderr: File,
    next_id: u64,
}
impl Mcp {
    /// Start the actual product with either retained legacy framing or canonical service framing.
    fn start(fixture: &Fixture, legacy: bool) -> Self {
        Self::start_selected(fixture, legacy, false)
    }
    /// Select native child libraries through the actual canonical product flag.
    fn start_selected(fixture: &Fixture, legacy: bool, recursive: bool) -> Self {
        assert!(!recursive || !legacy);
        let stderr = tempfile::tempfile().unwrap();
        let mut command = fixture.command();
        command.arg("--root").arg(&fixture.root);
        if legacy {
            command.arg("serve-mcp");
        } else {
            command.args(["serve", "--stdio"]);
            if recursive {
                command.arg("--recursive");
            }
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr.try_clone().unwrap()))
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, replies) = channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut session = Self {
            child: Some(child),
            input: Some(input),
            replies,
            reader: Some(reader),
            stderr,
            next_id: 0,
        };
        let revision = if legacy { "2024-11-05" } else { "2025-11-25" };
        let initialized = session.request("initialize", json!({"protocolVersion":revision,"capabilities":{},"clientInfo":{"name":"synthetic-query-face","version":"1"}}));
        assert_eq!(initialized["result"]["protocolVersion"], revision);
        writeln!(
            session.input.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        session
    }
    /// Read only this owned child's task-local diagnostic file.
    fn diagnostic(&mut self) -> String {
        self.stderr.seek(SeekFrom::Start(0)).unwrap();
        let mut diagnostic = String::new();
        self.stderr.read_to_string(&mut diagnostic).unwrap();
        diagnostic
    }
    /// Round-trip one actual JSON-RPC message with a finite native response wait.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        writeln!(
            self.input.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )
        .unwrap();
        self.input.as_mut().unwrap().flush().unwrap();
        let line = self
            .replies
            .recv_timeout(Duration::from_secs(180))
            .unwrap_or_else(|error| {
                panic!(
                    "product MCP response {method}: {error}; {}",
                    self.diagnostic()
                )
            })
            .unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["id"], id);
        reply
    }
    /// Call one catalog tool through the real MCP transport.
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name":name,"arguments":arguments}))
    }
    /// Require typed canonical structured content and its exact text mirror, including errors.
    fn canonical(&mut self, name: &str, arguments: Value) -> Value {
        let reply = self.tool(name, arguments);
        assert!(
            reply.get("error").is_none(),
            "canonical tool envelope: {reply}"
        );
        let content = &reply["result"]["structuredContent"];
        let text: Value =
            serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(&text, content);
        serde_json::from_value::<omni_service::Response>(content.clone()).unwrap();
        assert_eq!(
            reply["result"]["isError"],
            content["result"]["type"] == "error"
        );
        content["result"].clone()
    }
    /// Close input and require normal product shutdown; this is not a native settlement receipt.
    fn finish(mut self) {
        self.input.take();
        let status = self.child.as_mut().unwrap().wait().unwrap();
        assert!(status.success(), "product MCP exit: {}", self.diagnostic());
        self.child.take();
        self.reader.take().unwrap().join().unwrap();
    }
}
impl Drop for Mcp {
    /// Failure cleanup signals only the exact task-owned Child, never a process-name search.
    fn drop(&mut self) {
        self.input.take();
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Exact frozen Query DTO arguments; data ceilings stay explicit on every request.
fn query(id: &str, keywords: &[&str], methods: &[&str], tags: &[&str], sensitive: bool) -> Value {
    json!({"query":{"library_ids":[id],"keywords":keywords,"methods":methods,"required_tags":tags,"case_sensitive":sensitive,"limit":5,
        "budget":{"max_files":1000,"max_bytes":"67108864","max_millis":60000,"max_memory_bytes":"536870912"},"image_query":null}})
}

/// Canonical stdio discovers actual selected IDs, preserves its catalog and refuses write widening.
#[test]
fn canonical_mcp_catalog_selection_and_readonly_are_real() {
    let fixture = Fixture::new();
    let foreign = fixture.directory.path().join("foreign-library");
    fixture.initialize(&foreign);
    fs::write(
        foreign.join("pages/foreign-secret.md"),
        "syntheticforeignsecret\n",
    )
    .unwrap();
    let foreign_id = KnowledgeBase::open(&foreign)
        .unwrap()
        .config
        .library_id
        .unwrap();
    let mut session = Mcp::start(&fixture, false);
    let catalog = session.request("tools/list", json!({}));
    let actual = catalog["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    let wanted = knowledge_daemon::mcp_tool_names()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, wanted);
    assert!(!actual.contains("exposure_set"));
    assert!(!actual.contains("library_open"));
    let libraries = session.canonical("library_list", json!({}));
    let selected = libraries["libraries"].as_array().unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0]["library_id"], fixture.id);
    assert_eq!(selected[0]["exposure"]["mode"], "read_only");
    let result = session.canonical(
        "query",
        query(
            &fixture.id,
            &["facealpha"],
            &["bm25"],
            &["AlphaTag", "Both"],
            false,
        ),
    );
    assert_eq!(
        paths(&result["search"], &fixture.id),
        expected(&["pages/face-alpha.md"])
    );
    let refused = session.canonical(
        "query",
        query(
            &foreign_id,
            &["syntheticforeignsecret"],
            &["bm25"],
            &[],
            false,
        ),
    );
    assert_eq!(refused["error"]["code"], "not_exposed");
    assert!(!refused.to_string().contains("syntheticforeignsecret"));
    let mut override_query = query(
        &fixture.id,
        &["syntheticforeignsecret"],
        &["bm25"],
        &[],
        false,
    );
    override_query
        .as_object_mut()
        .unwrap()
        .insert("root".into(), json!(foreign));
    let refused = session.tool("query", override_query);
    assert!(refused.get("error").is_some() || refused["result"]["isError"] == true);
    assert!(!refused.to_string().contains("foreign-secret.md"));
    let write = session.canonical("refresh", json!({"library_id":fixture.id,"mount_ids":[],"verification":"full_hash","embeddings":false,
        "budget":{"max_files":1000,"max_bytes":"67108864","max_millis":60000,"max_memory_bytes":"536870912"},"operation_id":"unapproved-mcp-refresh"}));
    assert_eq!(write["error"]["code"], "read_only");
    let owner = session.tool(
        "exposure_set",
        json!({"library_id":fixture.id,"mode":"read_write"}),
    );
    assert!(owner.get("error").is_some());
    session.finish();
}

/// One actual recursive stdio selection queries parent/child corpora as independent labelled groups.
#[test]
fn canonical_mcp_recursive_selection_preserves_independent_library_groups() {
    let fixture = Fixture::new();
    let child = fixture.root.join("nested");
    fixture.initialize(&child);
    fs::write(
        child.join("pages/face-child.md"),
        "---\ntags: [AlphaTag, Both]\n---\nfacechild\n",
    )
    .unwrap();
    let child_id = KnowledgeBase::open(&child)
        .unwrap()
        .config
        .library_id
        .unwrap();
    let foreign = fixture.directory.path().join("foreign-library");
    fixture.initialize(&foreign);
    fs::write(
        foreign.join("pages/foreign-secret.md"),
        "syntheticrecursiveforeignsecret\n",
    )
    .unwrap();
    let foreign_id = KnowledgeBase::open(&foreign)
        .unwrap()
        .config
        .library_id
        .unwrap();
    let mut session = Mcp::start_selected(&fixture, false, true);
    let libraries = session.canonical("library_list", json!({}));
    let selected = libraries["libraries"].as_array().unwrap();
    assert_eq!(selected.len(), 2);
    let selected_ids = selected
        .iter()
        .map(|library| {
            assert_eq!(library["exposure"]["mode"], "read_only");
            library["library_id"].as_str().unwrap().to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected_ids.iter().cloned().collect::<BTreeSet<_>>(),
        expected(&[&fixture.id, &child_id])
    );
    let child_status = session.canonical("library_status", json!({"library_id":child_id}));
    assert_eq!(child_status["type"], "status");
    assert_eq!(child_status["status"]["library"]["library_id"], child_id);
    let mut arguments = query(
        &fixture.id,
        &["facealpha", "facechild"],
        &["bm25"],
        &["AlphaTag", "Both"],
        false,
    );
    arguments["query"]["library_ids"] = json!(selected_ids);
    let result = session.canonical("query", arguments.clone());
    assert_eq!(result["type"], "search");
    assert_eq!(result["search"]["applied"], arguments["query"]);
    assert_eq!(result["search"]["groups"].as_array().unwrap().len(), 2);
    assert_eq!(
        paths(&result["search"], &fixture.id),
        expected(&["pages/face-alpha.md"])
    );
    assert_eq!(
        paths(&result["search"], &child_id),
        expected(&["pages/face-child.md"])
    );
    let refused = session.canonical(
        "query",
        query(
            &foreign_id,
            &["syntheticrecursiveforeignsecret"],
            &["bm25"],
            &[],
            false,
        ),
    );
    assert_eq!(refused["error"]["code"], "not_exposed");
    assert!(!refused.to_string().contains("foreign-secret.md"));
    session.finish();
}

/// Failed methods retain typed data and do not erase or poison a previous applied result.
#[test]
fn canonical_mcp_repeated_queries_preserve_keywords_errors_and_prior_results() {
    let fixture = Fixture::new();
    let mut session = Mcp::start(&fixture, false);
    let arguments = query(
        &fixture.id,
        &["facealpha", "facelower"],
        &["literal", "bm25"],
        &["alphatag", "Both"],
        false,
    );
    let first = session.canonical("query", arguments.clone());
    assert_eq!(first["type"], "search");
    assert_eq!(first["search"]["applied"], arguments["query"]);
    assert_eq!(
        paths(&first["search"], &fixture.id),
        expected(&["pages/face-alpha.md", "pages/face-lower.md"])
    );
    let keywords = first["search"]["groups"][0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|file| file["evidence"].as_array().unwrap())
        .filter_map(|evidence| evidence["keyword"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(keywords, BTreeSet::from(["facealpha", "facelower"]));
    for (methods, keywords, code, method) in [
        (
            vec!["literal", "regex"],
            vec!["["],
            "invalid_request",
            "regex",
        ),
        (
            vec!["bm25", "semantic"],
            vec!["facealpha"],
            "provider_unavailable",
            "semantic",
        ),
    ] {
        let refusal =
            session.canonical("query", query(&fixture.id, &keywords, &methods, &[], false));
        assert_eq!(refusal["type"], "error");
        assert_eq!(refusal["error"]["code"], code);
        assert_eq!(refusal["error"]["method"], method);
        let again = session.canonical("query", arguments.clone());
        assert_eq!(
            again, first,
            "failed query changed the previous applied result or selected generation"
        );
    }
    let sensitive = session.canonical(
        "query",
        query(
            &fixture.id,
            &["face-"],
            &["files"],
            &["alphatag", "Both"],
            true,
        ),
    );
    assert_eq!(
        paths(&sensitive["search"], &fixture.id),
        expected(&["pages/face-lower.md"])
    );
    let missing = session.canonical(
        "query",
        query(
            &fixture.id,
            &["face-"],
            &["files"],
            &["AlphaTag", "absent-tag"],
            false,
        ),
    );
    assert!(paths(&missing["search"], &fixture.id).is_empty());
    session.finish();

    let mut legacy = Mcp::start(&fixture, true);
    let catalog = legacy.request("tools/list", json!({}));
    assert_eq!(catalog["result"]["tools"].as_array().unwrap().len(), 7);
    let response = legacy.tool(
        "search",
        json!({"query":"facealpha","mode":"bm25","tags":["AlphaTag","Both"],"limit":5}),
    );
    assert_eq!(response["result"]["isError"], false);
    let hits: Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["path"], "pages/face-alpha.md");
    legacy.finish();
}

/// An actual output consumer that changes a winner after accepting only the first byte.
struct BoundaryWriter {
    mutate: Option<PathBuf>,
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    fail_flush: bool,
}
impl Write for BoundaryWriter {
    /// Observe the real active Claim and make the next controlled write require fresh validation.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        assert_eq!(
            knowledge_core::admission::Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            1
        );
        self.writes += 1;
        if let Some(source) = &self.mutate {
            assert_eq!(
                self.writes, 1,
                "changed source reached another uncontrolled output write"
            );
            assert!(!bytes.is_empty());
            self.bytes.push(bytes[0]);
            fs::write(source, "changed by the actual first output consumer\n")?;
            Ok(1)
        } else {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
    }
    /// Fail the real final consumer boundary after the encoded bytes have reached it.
    fn flush(&mut self) -> std::io::Result<()> {
        assert_eq!(
            knowledge_core::admission::Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            1
        );
        self.flushes += 1;
        assert!(
            self.mutate.is_none(),
            "changed source reached the output flush"
        );
        if self.fail_flush {
            Err(std::io::Error::other("synthetic output flush failure"))
        } else {
            Ok(())
        }
    }
}

/// Build requests only through the frozen public codec, preserving actual library selection.
fn direct_request(id: &str, request_id: &str, keyword: &str) -> omni_service::Request {
    let mut arguments = query(id, &[keyword], &["bm25"], &["AlphaTag"], false);
    arguments
        .as_object_mut()
        .unwrap()
        .insert("op".into(), json!("query"));
    omni_service::decode_request(
        &serde_json::to_vec(&json!({
            "wire_version":1,"request_id":request_id,"command":arguments
        }))
        .unwrap(),
    )
    .unwrap()
}

/// Deliver a real healthy query after a failed output consumer, without substituting its owner.
fn healthy_direct(host: &omni_knowledge::mcp_native::NativeQueryHost, id: &str, name: &str) {
    let request = direct_request(id, name, "facebeta");
    let mut bytes = Vec::new();
    host.with_query(&request, |reply| {
        assert!(!reply.is_error());
        reply.write_wire(&mut bytes)
    })
    .unwrap();
    let wire: omni_service::Response = serde_json::from_slice(&bytes).unwrap();
    let value = serde_json::to_value(wire).unwrap();
    assert_eq!(
        paths(&value["result"]["search"], id),
        expected(&["pages/face-beta.md"])
    );
    assert_eq!(
        knowledge_core::admission::Account::current()
            .unwrap()
            .status()
            .unwrap()
            .queue
            .unsettled,
        0
    );
}

/// Observe the ONE account's actual outstanding memory and generated-storage promises.
fn account_reservation() -> (u64, u64) {
    let reserved = knowledge_core::admission::Account::current()
        .unwrap()
        .status()
        .unwrap()
        .reserved;
    (reserved.memory_bytes, reserved.temporary_bytes)
}

/// Per-write source checks stop a changed winner, and either output failure collects its actual Claim.
#[test]
fn public_native_reply_rechecks_writes_and_collects_after_flush_failure() {
    const CHILD_ROOT: &str = "OMNI_QUERY_FACE_WRITER_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        use knowledge_core::admission::Account;
        use omni_knowledge::mcp_native::NativeQueryHost;
        assert!(Account::current().is_err());
        let root = PathBuf::from(root);
        let host = NativeQueryHost::open(vec![KnowledgeBase::open(&root).unwrap()], true).unwrap();
        assert_eq!(
            Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            0
        );
        let id = host.library_ids().pop().unwrap();
        let baseline = account_reservation();
        let mut arguments = query(id.as_str(), &["["], &["regex"], &[], false);
        arguments
            .as_object_mut()
            .unwrap()
            .insert("op".into(), json!("query"));
        let regex = omni_service::decode_request(
            &serde_json::to_vec(&json!({
                "wire_version":1,"request_id":"actual-regex-failure","command":arguments
            }))
            .unwrap(),
        )
        .unwrap();
        let mut bytes = Vec::new();
        host.with_query(&regex, |reply| {
            assert!(
                reply.is_error(),
                "invalid regex must reach the actual failed native run"
            );
            reply.write_wire(&mut bytes)
        })
        .unwrap();
        let response: omni_service::Response = serde_json::from_slice(&bytes).unwrap();
        let response = serde_json::to_value(response).unwrap();
        assert_eq!(response["result"]["type"], "error");
        assert_eq!(response["result"]["error"]["code"], "invalid_request");
        assert_eq!(response["result"]["error"]["method"], "regex");
        assert_eq!(
            Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            0
        );
        assert_eq!(
            account_reservation(),
            baseline,
            "failed regex retained actual native credit"
        );
        healthy_direct(&host, id.as_str(), "healthy-after-regex");
        assert_eq!(
            account_reservation(),
            baseline,
            "retry after regex retained actual native credit"
        );
        let source = root.join("pages/face-alpha.md");
        let original = fs::read(&source).unwrap();
        let request = direct_request(id.as_str(), "actual-writer-mutation", "facealpha");
        let mut writer = BoundaryWriter {
            mutate: Some(source.clone()),
            bytes: Vec::new(),
            writes: 0,
            flushes: 0,
            fail_flush: false,
        };
        let result = host.with_query(&request, |reply| {
            assert!(
                !reply.is_error(),
                "mutation fixture must first produce an actual native result"
            );
            reply.write_wire(&mut writer)
        });
        assert!(result.is_err(), "changed winner was delivered successfully");
        assert_eq!(writer.writes, 1);
        assert_eq!(
            writer.bytes.len(),
            1,
            "no subsequent bytes may pass the changed-source check"
        );
        assert_eq!(writer.flushes, 0);
        assert_eq!(
            Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            0,
            "with_query must collect its real owner before returning the write failure"
        );
        assert_eq!(
            account_reservation(),
            baseline,
            "mutation refusal retained actual native credit"
        );
        fs::write(&source, original).unwrap();
        // Restored bytes have a new native stat version. The unchanged beta source proves this
        // same host remains usable without manufacturing alpha's old version or a new Refresh.
        healthy_direct(&host, id.as_str(), "healthy-after-mutation");
        assert_eq!(
            account_reservation(),
            baseline,
            "retry after mutation retained actual native credit"
        );

        let request = direct_request(id.as_str(), "actual-flush-failure", "facebeta");
        let mut writer = BoundaryWriter {
            mutate: None,
            bytes: Vec::new(),
            writes: 0,
            flushes: 0,
            fail_flush: true,
        };
        let result = host.with_query(&request, |reply| {
            assert!(!reply.is_error());
            reply.write_wire(&mut writer)
        });
        assert!(
            result.is_err(),
            "failed output flush was reported as delivered"
        );
        assert!(writer.writes > 0);
        assert!(!writer.bytes.is_empty());
        assert_eq!(writer.flushes, 1);
        assert_eq!(
            Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            0,
            "flush failure must not strand the actual Claim"
        );
        assert_eq!(
            account_reservation(),
            baseline,
            "failed flush retained actual native credit"
        );
        healthy_direct(&host, id.as_str(), "healthy-after-flush");
        assert_eq!(
            account_reservation(),
            baseline,
            "retry after flush retained actual native credit"
        );
        return;
    }
    let fixture = Fixture::new();
    let mut command = Command::new(std::env::current_exe().unwrap());
    fixture.environment(&mut command);
    let output = command
        .args([
            "--exact",
            "public_native_reply_rechecks_writes_and_collects_after_flush_failure",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ROOT, &fixture.root)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
    assert!(
        output.status.success(),
        "actual native writer child: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
