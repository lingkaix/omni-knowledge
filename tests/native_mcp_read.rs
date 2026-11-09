//! Actual stdio preview metadata comes from the same canonical Source projection as native Query.
use omni_service::{DigestState, Enumeration, SearchResult, Source};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread::JoinHandle;
use std::time::Duration;

const BODY: &str = "nativepreviewword original authored text\n";
const SOURCE_PATH: &str = "raw/document.txt";

/// Keep the library, external original, OS directories and all process logs synthetic.
struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
}
impl Fixture {
    /// Initialize and import through the real public CLI, without manufacturing a source UUID.
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        for name in ["home", "os-cache", "temporary"] {
            fs::create_dir(directory.path().join(name)).unwrap();
        }
        let fixture = Self {
            root: directory.path().join("library"),
            directory,
        };
        fixture.run(fixture.command().arg("init").arg(&fixture.root).args([
            "--name",
            "Canonical MCP preview",
            "--json",
        ]));
        let original = fixture.directory.path().join("document.txt");
        fs::write(&original, BODY).unwrap();
        let added = fixture.run(
            fixture
                .command()
                .arg("--root")
                .arg(&fixture.root)
                .arg("add")
                .arg(&original)
                .args(["--tag", "Research", "--tag", "Both", "--json"]),
        );
        assert_eq!(added["imported"], json!([SOURCE_PATH]));
        fixture
    }

    /// Select private home/cache/TMP paths for the actual built CLI and its real sibling helper.
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omni"));
        command
            .env("HOME", self.directory.path().join("home"))
            .env("XDG_CACHE_HOME", self.directory.path().join("os-cache"))
            .env("TMPDIR", self.directory.path().join("temporary"))
            .env("OMNI_CACHE_DIR", self.directory.path().join("native-cache"));
        command
    }

    /// Require successful machine-readable product output with real diagnostics on failure.
    fn run(&self, command: &mut Command) -> Value {
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "CLI failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    /// Observe the full public native Source independently of the MCP read adapter.
    fn query_source(&self) -> Source {
        let value = self.run(self.command().arg("--root").arg(&self.root).args([
            "search",
            "nativepreviewword",
            "--mode",
            "literal",
            "--canonical",
            "--json",
        ]));
        let result: SearchResult = serde_json::from_value(value).unwrap();
        assert_eq!(
            result.applied.budget.max_memory_bytes.get(),
            512 * 1024 * 1024
        );
        assert_eq!(result.applied.budget.max_bytes.get(), 64 * 1024 * 1024);
        assert_eq!(result.groups.len(), 1);
        let group = &result.groups[0];
        assert_eq!(group.coverage.enumeration, Enumeration::Complete);
        assert_eq!(group.coverage.failed.get(), 0);
        assert_eq!(group.files.len(), 1);
        assert_eq!(group.files[0].source.library_id, group.library_id);
        group.files[0].source.clone()
    }
}

/// One real line-framed MCP process, retained across external pointer edits.
struct Mcp {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
    diagnostics: PathBuf,
}
impl Mcp {
    /// Launch the public seven-tool stdio face with bounded response waits and owned stderr.
    fn start(fixture: &Fixture) -> Self {
        let diagnostics = fixture.directory.path().join("mcp.stderr");
        let mut child = fixture
            .command()
            .arg("--root")
            .arg(&fixture.root)
            .arg("serve-mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(&diagnostics).unwrap())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, replies) = channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let value = serde_json::from_str(&line.unwrap()).unwrap();
                if send.send(value).is_err() {
                    break;
                }
            }
        });
        let mut mcp = Self {
            child,
            input: Some(input),
            replies,
            reader: Some(reader),
            diagnostics,
        };
        let initialized = mcp.request(
            1,
            "initialize",
            json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"canonical-read-test","version":"1"}}),
        );
        assert_eq!(initialized["result"]["protocolVersion"], "2024-11-05");
        mcp
    }

    /// Send a real JSON-RPC frame and require the matching response from the same process.
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        let input = self.input.as_mut().unwrap();
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )
        .unwrap();
        input.flush().unwrap();
        let response = self
            .replies
            .recv_timeout(Duration::from_secs(60))
            .unwrap_or_else(|error| {
                panic!(
                    "MCP response unavailable: {error}; {}",
                    fs::read_to_string(&self.diagnostics).unwrap()
                )
            });
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], id);
        response
    }

    /// Preview text must stay usable while metadata is the complete canonical typed Source.
    fn read(&mut self, id: u64) -> Source {
        let response = self.request(
            id,
            "tools/call",
            json!({"name":"read","arguments":{"path":SOURCE_PATH}}),
        );
        assert_eq!(response["result"]["isError"], false, "{response}");
        let read: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(read["path"], SOURCE_PATH);
        assert_eq!(read["text"], BODY);
        assert_eq!(read["status"], "extracted");
        assert!(read["warnings"].as_array().unwrap().is_empty());
        assert!(
            read["metadata"].is_object(),
            "read must return canonical annotations, not unavailable metadata: {read}"
        );
        serde_json::from_value(read["metadata"].clone()).unwrap()
    }

    /// Close input and collect the actual product process and its stdout reader normally.
    fn finish(&mut self) {
        drop(self.input.take());
        let status = self.child.wait().unwrap();
        self.reader.take().unwrap().join().unwrap();
        assert!(
            status.success(),
            "MCP failed: {}",
            fs::read_to_string(&self.diagnostics).unwrap()
        );
    }
}
impl Drop for Mcp {
    /// Failure cleanup targets only the exact child owned by this fixture.
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Import, live visible edits and hidden-sidecar conflicts all use canonical Query annotations.
#[test]
fn stdio_read_returns_live_canonical_import_annotations() {
    let fixture = Fixture::new();
    // More than the normal Query result bound can match this full path in metadata.
    // They sort before raw/document.txt, but none contains the body-query word.
    for index in 0..35 {
        fs::write(
            fixture
                .root
                .join(format!("pages/a-reference-{index:02}.md")),
            format!(
                "---\ndescription: Reference to {SOURCE_PATH}\n---\nIndependent reference page.\n"
            ),
        )
        .unwrap();
    }
    let queried = fixture.query_source();
    uuid::Uuid::parse_str(queried.source_id.as_str()).unwrap();
    assert_eq!(queried.path.as_str(), SOURCE_PATH);
    assert_eq!(queried.logical_path.as_str(), SOURCE_PATH);
    assert_eq!(queried.mount_id, None);
    assert_eq!(queried.title, "document.txt");
    assert_eq!(queried.description, "");
    assert_eq!(queried.tags, vec!["Both", "Research"]);
    assert_eq!(queried.digest_state, DigestState::Verified);
    assert_eq!(
        queried.digest.as_ref().unwrap().as_str(),
        hex::encode(Sha256::digest(BODY.as_bytes()))
    );
    let relative = format!("sources/{}.md", queried.source_id.as_str());
    assert_eq!(queried.annotation_path.as_ref().unwrap().as_str(), relative);
    let pointer = fixture.root.join(&relative);
    let original_pointer = fs::read_to_string(&pointer).unwrap();

    let mut mcp = Mcp::start(&fixture);
    assert_eq!(mcp.read(2), queried);
    assert_eq!(fs::read_to_string(&pointer).unwrap(), original_pointer);

    // An external authored edit must be observed without restarting this MCP process.
    let frontmatter = original_pointer
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---")
        .unwrap()
        .0;
    let mut fields: serde_yaml::Value = serde_yaml::from_str(frontmatter).unwrap();
    fields["title"] = serde_yaml::Value::String("Visible edited title".into());
    fields["description"] = serde_yaml::Value::String("Visible edited description".into());
    fields["tags"] = serde_yaml::to_value(vec!["LiveCase", "Both"]).unwrap();
    let edited = format!(
        "---\n{}---\nVisible authored summary.\n",
        serde_yaml::to_string(&fields).unwrap()
    );
    fs::write(&pointer, &edited).unwrap();
    let sidecar = fixture.root.join(format!(
        ".omni/metadata/{}.json",
        hex::encode(Sha256::digest(SOURCE_PATH.as_bytes()))
    ));
    fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    let hidden = serde_json::to_vec(&json!({"version":1,"path":SOURCE_PATH,"tags":["HiddenWrong"],"description":"Hidden wrong description"})).unwrap();
    fs::write(&sidecar, &hidden).unwrap();
    let updated = mcp.read(3);
    assert_eq!(updated.source_id, queried.source_id);
    assert_eq!(updated.library_id, queried.library_id);
    assert_eq!(updated.path, queried.path);
    assert_eq!(updated.logical_path, queried.logical_path);
    assert_eq!(updated.annotation_path, queried.annotation_path);
    assert_eq!(updated.digest, queried.digest);
    assert_eq!(updated.digest_state, queried.digest_state);
    assert_eq!(updated.title, "Visible edited title");
    assert_eq!(
        updated.description,
        "Visible edited description\nVisible authored summary."
    );
    assert_eq!(updated.tags, vec!["LiveCase", "Both"]);
    assert_eq!(updated, fixture.query_source());
    assert_eq!(fs::read_to_string(&pointer).unwrap(), edited);
    assert_eq!(fs::read(&sidecar).unwrap(), hidden);
    assert_eq!(
        fs::read_to_string(fixture.directory.path().join("document.txt")).unwrap(),
        BODY
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join(SOURCE_PATH)).unwrap(),
        BODY
    );
    mcp.finish();
}
