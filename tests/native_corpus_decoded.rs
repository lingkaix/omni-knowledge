//! Native corpus Office decoding uses its documented finite limit, independently of ZIP size.
use omni_service::SearchResult;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Processing owner: direct input and derived text default to 8 MiB, including expansion bounds.
const DECODED_LIMIT: usize = 8 * 1024 * 1024;
const XML_START: &str = "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>";
const XML_END: &str = "</w:t></w:r></w:p></w:body></w:document>";

/// Keep all process-generated files within one exclusively owned synthetic environment.
struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
}
impl Fixture {
    /// Use the built product, its actual sibling, and private HOME/cache/TMP directories.
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omni"));
        command
            .env("HOME", self.directory.path().join("home"))
            .env("XDG_CACHE_HOME", self.directory.path().join("os-cache"))
            .env("TMPDIR", self.directory.path().join("temporary"))
            .env("OMNI_CACHE_DIR", self.directory.path().join("native-cache"));
        command
    }

    /// Exercise cold actual native Refresh and the canonical whole-library Query for each request.
    fn search(&self, word: &str, method: &str) -> SearchResult {
        let output = self
            .command()
            .arg("--root")
            .arg(&self.root)
            .args([
                "search",
                word,
                "--mode",
                method,
                "--limit",
                "5",
                "--canonical",
                "--json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "native {method} query: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: SearchResult = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            result.applied.budget.max_memory_bytes.get(),
            512 * 1024 * 1024
        );
        assert_eq!(result.applied.budget.max_bytes.get(), 64 * 1024 * 1024);
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].library_id, result.applied.library_ids[0]);
        assert!(result.groups[0].generation_id.is_some());
        assert_eq!(
            result.groups[0].coverage.enumeration,
            omni_service::Enumeration::Complete
        );
        assert_eq!(
            result.groups[0].returned_files.get(),
            result.groups[0].files.len() as u64
        );
        result
    }
}

/// Encode valid Office XML using actual DEFLATE and independently confirm its declared expansion.
fn deflated_docx(path: &Path, xml: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "word/document.xml",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(xml.as_bytes()).unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let member = archive.by_name("word/document.xml").unwrap();
    assert_eq!(member.compression(), CompressionMethod::Deflated);
    assert_eq!(member.size(), xml.len() as u64);
    drop(member);
    drop(archive);
    assert!(bytes.len() < xml.len());
    fs::write(path, &bytes).unwrap();
    bytes
}

/// Benign compressed expansion succeeds; an actual over-limit member remains failed but discoverable.
#[test]
fn native_corpus_decodes_small_deflate_office_and_refuses_expansion_past_limit() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["home", "os-cache", "temporary"] {
        fs::create_dir(directory.path().join(name)).unwrap();
    }
    let fixture = Fixture {
        root: directory.path().join("library"),
        directory,
    };
    let initialized = fixture
        .command()
        .arg("init")
        .arg(&fixture.root)
        .args(["--name", "Synthetic decoded bound", "--json"])
        .output()
        .unwrap();
    assert!(
        initialized.status.success(),
        "actual init: {}",
        String::from_utf8_lossy(&initialized.stderr)
    );

    // Exactly the N02 acceptance document.xml body, including its original 2,000 filler phrases.
    let ordinary_xml = format!(
        "{XML_START}installeddocxword {}{XML_END}",
        "synthetic parser filler ".repeat(2000)
    );
    assert!(ordinary_xml.len() < DECODED_LIMIT);
    let ordinary = fixture.root.join("raw/installed.docx");
    let ordinary_bytes = deflated_docx(&ordinary, &ordinary_xml);
    assert!(
        ordinary_bytes.len() < 2048,
        "benign control must retain small real compressed input"
    );
    let found = fixture.search("installeddocxword", "literal");
    let group = &found.groups[0];
    assert_eq!(group.coverage.failed.get(), 0);
    assert_eq!(group.files.len(), 1);
    assert_eq!(group.files[0].source.path.as_str(), "raw/installed.docx");
    assert!(
        group.files[0]
            .evidence
            .iter()
            .any(
                |evidence| evidence.method == omni_service::SearchMethod::Literal
                    && evidence.snippet.contains("installeddocxword")
            )
    );
    assert_eq!(fs::read(&ordinary).unwrap(), ordinary_bytes);

    let oversized_xml = format!(
        "{XML_START}decodedbombword {}{XML_END}",
        "x".repeat(DECODED_LIMIT + 1)
    );
    assert!(oversized_xml.len() > DECODED_LIMIT);
    let oversized = fixture.root.join("raw/over-decoded.docx");
    let oversized_bytes = deflated_docx(&oversized, &oversized_xml);
    assert!(
        oversized_bytes.len() < DECODED_LIMIT,
        "refusal must concern expanded XML, not original input size"
    );
    let refused = fixture.search("decodedbombword", "literal");
    assert!(
        refused.groups[0].files.is_empty(),
        "over-limit XML content must not enter the searchable representation"
    );
    assert_eq!(refused.groups[0].coverage.failed.get(), 1);
    let metadata = fixture.search("over-decoded", "files");
    assert_eq!(metadata.groups[0].files.len(), 1);
    assert_eq!(
        metadata.groups[0].files[0].source.path.as_str(),
        "raw/over-decoded.docx"
    );
    // Files reports current M1 metadata observations; the indexed failure is asserted above.
    assert_eq!(fs::read(&ordinary).unwrap(), ordinary_bytes);
    assert_eq!(fs::read(&oversized).unwrap(), oversized_bytes);
}
