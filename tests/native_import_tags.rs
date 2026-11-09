//! Imported annotations use visible M1 pointers; legacy query refusal preserves the home.
use omni_service::{Enumeration, SearchMethod, SearchResult};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// All product processes use an exclusively owned synthetic home and generated namespace.
struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
}
impl Fixture {
    /// Create isolation without initializing the library or fabricating source identities.
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        for name in ["home", "os-cache", "temporary"] {
            fs::create_dir(directory.path().join(name)).unwrap();
        }
        Self {
            root: directory.path().join("library"),
            directory,
        }
    }

    /// Invoke the built public CLI and its real sibling parser with private storage paths.
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omni"));
        command
            .env("HOME", self.directory.path().join("home"))
            .env("XDG_CACHE_HOME", self.directory.path().join("os-cache"))
            .env("TMPDIR", self.directory.path().join("temporary"))
            .env("OMNI_CACHE_DIR", self.directory.path().join("native-cache"));
        command
    }

    /// Fail with actual product diagnostics rather than accepting a partial initialization/import.
    fn success(output: Output, operation: &str) -> Output {
        assert!(
            output.status.success(),
            "{operation}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    /// Cold startup and actual native literal Query must preserve the default finite limits.
    fn search(&self, tags: &[&str], case_sensitive: bool) -> SearchResult {
        let mut command = self.command();
        command.arg("--root").arg(&self.root).args([
            "search",
            "nativeimportword",
            "--mode",
            "literal",
            "--canonical",
            "--json",
        ]);
        for tag in tags {
            command.args(["--tag", tag]);
        }
        if case_sensitive {
            command.arg("--case-sensitive");
        }
        let output = Self::success(command.output().unwrap(), "cold native query");
        let result: SearchResult = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            result.applied.budget.max_memory_bytes.get(),
            512 * 1024 * 1024
        );
        assert_eq!(result.applied.budget.max_bytes.get(), 64 * 1024 * 1024);
        assert_eq!(result.applied.methods, vec![SearchMethod::Literal]);
        assert_eq!(
            result.applied.required_tags,
            tags.iter().map(|tag| (*tag).to_owned()).collect::<Vec<_>>()
        );
        assert_eq!(result.groups.len(), 1);
        let group = &result.groups[0];
        assert_eq!(group.library_id, result.applied.library_ids[0]);
        assert!(group.generation_id.is_some());
        assert_eq!(group.coverage.enumeration, Enumeration::Complete);
        assert_eq!(group.coverage.failed.get(), 0);
        assert_eq!(group.returned_files.get(), group.files.len() as u64);
        result
    }
}

/// A real compressed Office original requires the actual parser, not filename-only evidence.
fn docx_bytes() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "word/document.xml",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
    writer
        .write_all(b"<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>nativeimportword actual imported Office evidence</w:t></w:r></w:p></w:body></w:document>")
        .unwrap();
    writer.finish().unwrap().into_inner()
}

/// Import tags reach native filtering through the exact returned M1 UUID and visible pointer.
#[test]
fn cli_import_tags_are_canonical_native_annotations() {
    let fixture = Fixture::new();
    Fixture::success(
        fixture
            .command()
            .arg("init")
            .arg(&fixture.root)
            .args(["--name", "Synthetic canonical import", "--json"])
            .output()
            .unwrap(),
        "initialize library",
    );
    let original = fixture.directory.path().join("document.docx");
    let bytes = docx_bytes();
    fs::write(&original, &bytes).unwrap();
    let added = Fixture::success(
        fixture
            .command()
            .arg("--root")
            .arg(&fixture.root)
            .arg("add")
            .arg(&original)
            .args(["--tag", "Research", "--tag", "Both", "--json"])
            .output()
            .unwrap(),
        "import tagged Office original",
    );
    let added: serde_json::Value = serde_json::from_slice(&added.stdout).unwrap();
    assert_eq!(added["imported"], json!(["raw/document.docx"]));

    // This is the deciding OLD failure: successful import must yield actual tagged body evidence.
    let found = fixture.search(&["research", "Both"], false);
    let files = &found.groups[0].files;
    assert_eq!(
        files.len(),
        1,
        "imported tags disappeared from native Query"
    );
    let source = &files[0].source;
    assert_eq!(source.path.as_str(), "raw/document.docx");
    assert_eq!(source.library_id, found.groups[0].library_id);
    assert_eq!(source.tags.len(), 2);
    assert!(source.tags.iter().any(|tag| tag == "Research"));
    assert!(source.tags.iter().any(|tag| tag == "Both"));
    assert!(files[0].evidence.iter().any(|evidence| {
        evidence.method == SearchMethod::Literal && evidence.snippet.contains("nativeimportword")
    }));
    uuid::Uuid::parse_str(source.source_id.as_str()).unwrap();
    let pointer = format!("sources/{}.md", source.source_id.as_str());
    assert_eq!(source.annotation_path.as_ref().unwrap().as_str(), pointer);
    let text = fs::read_to_string(fixture.root.join(&pointer)).unwrap();
    let frontmatter = text
        .strip_prefix("---\n")
        .expect("visible pointer frontmatter");
    let (frontmatter, _) = frontmatter.split_once("\n---").unwrap();
    let fields: serde_yaml::Value = serde_yaml::from_str(frontmatter).unwrap();
    assert_eq!(fields["path"].as_str(), Some("raw/document.docx"));
    let pointer_tags = fields["tags"].as_sequence().unwrap();
    assert_eq!(pointer_tags.len(), 2);
    assert!(
        pointer_tags
            .iter()
            .any(|tag| tag.as_str() == Some("Research"))
    );
    assert!(pointer_tags.iter().any(|tag| tag.as_str() == Some("Both")));

    // Whole tags, AND filtering and explicit case apply to imported annotations too.
    assert!(
        fixture.search(&["Resear"], false).groups[0]
            .files
            .is_empty()
    );
    assert!(
        fixture.search(&["Research", "Absent"], false).groups[0]
            .files
            .is_empty()
    );
    assert!(
        fixture.search(&["research", "Both"], true).groups[0]
            .files
            .is_empty()
    );
    let exact = fixture.search(&["Research", "Both"], true);
    assert_eq!(exact.groups[0].files.len(), 1);
    assert_eq!(exact.groups[0].files[0].source.source_id, source.source_id);
    assert_eq!(fs::read(&original).unwrap(), bytes);
    assert_eq!(
        fs::read(fixture.root.join("raw/document.docx")).unwrap(),
        bytes
    );

    // A retained compatibility record must not duplicate the canonical authored tags.
    let sidecar = fixture.root.join(format!(
        ".omni/metadata/{}.json",
        hex::encode(Sha256::digest(b"raw/document.docx"))
    ));
    if sidecar.exists() {
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(sidecar).unwrap()).unwrap();
        assert!(metadata["tags"].as_array().is_none_or(Vec::is_empty));
    }
}

/// Include empty directories, filenames and bytes so startup cannot hide newly created state.
fn tree(root: &Path) -> Vec<(PathBuf, bool, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<(PathBuf, bool, Vec<u8>)>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        entries.push((
            path.strip_prefix(root).unwrap().to_owned(),
            metadata.is_dir(),
            if metadata.is_file() {
                fs::read(path).unwrap()
            } else {
                Vec::new()
            },
        ));
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries.sort();
    entries
}

/// An explicit legacy home refuses native startup and directs users to inspection without writes.
#[test]
fn cli_v1_query_refuses_before_refresh_without_library_writes() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.root.join("raw")).unwrap();
    fs::create_dir_all(fixture.root.join(".omni/metadata")).unwrap();
    fs::write(
        fixture.root.join("omni-knowledge.json"),
        serde_json::to_vec(&json!({
            "version": 1,
            "name": "Explicit legacy preview fixture",
            "library_id": "f94b9a79-2f5c-4a86-a8fd-3923b839c13e",
            "embedding": null,
            "whisper_executable": null,
            "whisper_model": null
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        fixture.root.join("raw/original.txt"),
        b"legacy searchable original\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join(".omni/metadata").join(format!(
            "{}.json", hex::encode(Sha256::digest(b"raw/original.txt"))
        )),
        br#"{"version":1,"path":"raw/original.txt","tags":["Preserve"],"description":"Authored legacy description"}"#,
    ).unwrap();
    let before = tree(&fixture.root);
    let output = fixture
        .command()
        .arg("--root")
        .arg(&fixture.root)
        .args([
            "search",
            "legacy",
            "--mode",
            "files",
            "--canonical",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "v1 Query must clearly refuse: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr).to_lowercase();
    for required in ["migration", "inventory", "preview"] {
        assert!(
            diagnostic.contains(required),
            "missing {required} guidance: {diagnostic}"
        );
    }
    assert_eq!(tree(&fixture.root), before, "refusal wrote library state");
}
