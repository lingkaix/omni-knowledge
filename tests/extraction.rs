//! Contract regressions for local extraction, inert previews, and EPUB packaging.

fn ensure_processing_account() -> std::sync::MutexGuard<'static, ()> {
    // These format/preview assertions share one account and use the documented
    // None opaque-RSS promise (all currently available credit). Test their
    // formats serially; dedicated Core/M3 tests retain concurrency coverage.
    static FORMATS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = FORMATS.lock().unwrap_or_else(|error| error.into_inner());
    omni_knowledge::extract::establish_process_account().expect("process account");
    guard
}

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use omni_knowledge::{epub, extract};
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

/// Write a small local archive without invoking external conversion tools.
fn archive(path: &Path, entries: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, contents) in entries {
        zip.start_file(
            *name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

/// Arbitrary binary files remain admitted without pretending their bytes are text.
#[test]
fn binary_and_extensionless_text_have_distinct_coverage() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let binary = dir.path().join("firmware.unknown");
    fs::write(&binary, [0, 255, 1, 2]).unwrap();
    let result = extract::extract(&binary, 1024).unwrap();
    assert_eq!(result.status, "metadata-only");
    assert!(result.text.is_empty());
    let note = dir.path().join("note");
    fs::write(&note, "知识库 searchable Unicode").unwrap();
    let result = extract::extract(&note, 1024).unwrap();
    assert_eq!(result.status, "extracted");
    assert!(result.text.contains("知识库"));
}

/// Create a PDF with a standard font and correct object offsets for the native parser.
fn native_pdf(text: &str) -> Vec<u8> {
    let content = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    pdf
}

/// PDF native text is searchable, and oversized sources keep honest metadata coverage.
#[test]
fn pdf_extracts_native_text_and_reports_source_limit() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.pdf");
    fs::write(&path, native_pdf("Searchable PDF content")).unwrap();
    let result = extract::extract(&path, 65536).unwrap();
    assert_eq!(result.status, "extracted");
    assert!(result.text.contains("Searchable PDF content"));
    let limited = extract::extract(&path, 64).unwrap();
    assert_eq!(limited.status, "metadata-only");
    assert!(limited.text.is_empty());
    assert!(!limited.warnings.is_empty());
}

/// Front matter stays discoverable and imported scripts never become executable preview markup.
#[test]
fn frontmatter_and_preview_preserve_unicode_without_active_content() {
    let _format = ensure_processing_account();
    let source = "---\r\ntitle: 知识库\r\ntags: [研究, rust]\r\n---\r\n# Body\n```mermaid\ngraph LR; A-->B\n```\n<script>alert(1)</script>";
    let (metadata, body) = extract::markdown_frontmatter(source);
    assert_eq!(metadata["title"], "知识库");
    assert_eq!(metadata["tags"][0], "研究");
    assert!(body.starts_with("# Body"));
    let html = extract::render_markdown(source);
    assert!(html.contains("知识库"));
    assert!(html.contains("language-mermaid"));
    assert!(!html.contains("<script>"));
    let dir = tempdir().unwrap();
    let path = dir.path().join("unsafe.html");
    fs::write(&path, "<h1>标题</h1><script>alert(1)</script><img src=\"https://example.com/tracker\" onerror=\"alert(2)\"><img src=\" //remote.example/tracker\"><a href=\"javascript:alert(3)\">Click</a>").unwrap();
    let html = extract::preview_html(&path, 4096).unwrap();
    assert!(html.contains("<h1>标题</h1>"));
    assert!(!html.contains("<script"));
    assert!(!html.contains("onerror"));
    assert!(!html.contains("href=\"javascript:"));
    assert!(!html.contains("src=\"javascript:"));
    assert!(html.contains("javascript:alert(3)"));
    assert!(!html.contains("https://example.com/tracker"));
    assert!(!html.contains("remote.example"));
}

/// Source evidence uses the same CRLF, blank-line, and Unicode positions as indexed text.
#[test]
fn text_preview_numbers_and_targets_exact_source_lines() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let path = dir.path().join("lines.txt");
    fs::write(
        &path,
        "First\r\n\r\n帽子 <script>unsafe</script>\r\nLast\r\n",
    )
    .unwrap();
    let html = extract::preview_html_at_line(&path, 4096, Some(3)).unwrap();
    assert_eq!(html.matches("class=\"line-number\"").count(), 4);
    assert!(html.contains("id=\"source-line-2\""));
    assert!(html.contains("id=\"source-line-3\" data-preview-target=\"true\""));
    assert_eq!(html.matches("data-preview-target=\"true\"").count(), 1);
    assert!(html.contains("帽子 &lt;script&gt;unsafe&lt;/script&gt;"));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("source-line-5"));
}

/// Markdown remains rendered by default, while a source hit includes original front-matter lines.
#[test]
fn markdown_preview_retains_rendering_and_targets_source_not_rendered_lines() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let path = dir.path().join("note.md");
    fs::write(&path, "---\ntitle: 帽子\n---\n# Heading\n\nA matching sentence.\n\n```mermaid\ngraph LR; A-->B\n```\n").unwrap();
    let normal = extract::preview_html(&path, 4096).unwrap();
    assert!(normal.contains("<details class=\"line-preview\">"));
    assert!(normal.contains("<h1>Heading</h1>"));
    assert!(normal.contains("language-mermaid"));
    assert!(!normal.contains("data-preview-target=\"true\""));
    let targeted = extract::preview_html_at_line(&path, 4096, Some(6)).unwrap();
    assert!(targeted.contains("<details class=\"line-preview\" open>"));
    assert!(targeted.contains("id=\"source-line-6\" data-preview-target=\"true\""));
    assert!(targeted.contains("Source lines"));
    assert!(targeted.contains("A matching sentence."));
}

/// Converted HTML evidence numbers extracted lines, and imported attributes cannot forge a hit.
#[test]
fn html_preview_targets_extracted_lines_without_accepting_source_targets() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let path = dir.path().join("document.html");
    fs::write(&path, "<h1>Title</h1><p>First paragraph.</p><p data-preview-target=\"true\">Second paragraph.</p>").unwrap();
    let text = extract::extract(&path, 4096).unwrap().text;
    let target = text
        .lines()
        .position(|line| line.contains("Second"))
        .unwrap()
        + 1;
    let html = extract::preview_html_at_line(&path, 4096, Some(target)).unwrap();
    assert!(html.contains("Extracted text lines"));
    assert!(html.contains(&format!(
        "id=\"source-line-{target}\" data-preview-target=\"true\""
    )));
    assert_eq!(html.matches("data-preview-target=\"true\"").count(), 1);
    assert!(html.contains("<h1>Title</h1>"));
    let stale = extract::preview_html_at_line(&path, 4096, Some(999)).unwrap();
    assert!(stale.contains("Line 999 is not available in the current text"));
    assert!(!stale.contains("data-preview-target=\"true\""));
}

/// Distant evidence remains visible in a bounded row window without highlighting a wrong line.
#[test]
fn numbered_preview_bounds_rows_and_rejects_unavailable_line_evidence() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let path = dir.path().join("many-lines.txt");
    let text: String = (1..=20_050).map(|line| format!("Line {line}\n")).collect();
    fs::write(&path, text).unwrap();
    let html = extract::preview_html_at_line(&path, 512_000, Some(20_040)).unwrap();
    assert_eq!(html.matches("class=\"line-number\"").count(), 20_000);
    assert!(html.contains("id=\"source-line-20040\" data-preview-target=\"true\""));
    assert!(html.contains("Showing lines 51–20050 of 20050"));
    assert!(!html.contains("id=\"source-line-1\""));
    for missing in [0, 20_051] {
        let stale = extract::preview_html_at_line(&path, 512_000, Some(missing)).unwrap();
        assert!(stale.contains(&format!("Line {missing} is not available")));
        assert!(!stale.contains("data-preview-target=\"true\""));
    }
}

/// ZIP document extraction reads text values instead of archive-order or shared-string indices.
#[test]
fn office_text_is_native_and_ordered() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let doc = dir.path().join("note.docx");
    archive(
        &doc,
        &[(
            "word/document.xml",
            "<w:document xmlns:w=\"urn:word\"><w:p><w:r><w:t>First</w:t></w:r><w:r><w:t> paragraph</w:t></w:r></w:p><w:p><w:r><w:t>第二段</w:t></w:r></w:p></w:document>",
        )],
    );
    let text = extract::extract(&doc, 65536).unwrap().text;
    assert!(text.contains("First paragraph"));
    assert!(text.contains("第二段"));
    let sheet = dir.path().join("data.xlsx");
    archive(
        &sheet,
        &[
            (
                "xl/sharedStrings.xml",
                "<sst><si><t>词条</t></si><si><t>Value</t></si></sst>",
            ),
            (
                "xl/worksheets/sheet1.xml",
                "<worksheet><sheetData><row><c t=\"s\"><v>0</v></c><c><v>42</v></c><c t=\"inlineStr\"><is><t>内联</t></is></c></row></sheetData></worksheet>",
            ),
        ],
    );
    let text = extract::extract(&sheet, 65536).unwrap().text;
    assert!(text.contains("词条"));
    assert!(text.contains("42"));
    assert!(text.contains("内联"));
    let slides = dir.path().join("deck.pptx");
    archive(
        &slides,
        &[
            (
                "ppt/slides/slide10.xml",
                "<slide><p><t>Tenth</t></p></slide>",
            ),
            (
                "ppt/slides/slide2.xml",
                "<slide><p><t>Second</t></p></slide>",
            ),
        ],
    );
    let text = extract::extract(&slides, 65536).unwrap().text;
    assert!(text.find("Second").unwrap() < text.find("Tenth").unwrap());
    #[cfg(target_os = "macos")]
    {
        let rtf = dir.path().join("legacy.rtf");
        fs::write(&rtf, r"{\rtf1\ansi Native RTF searchable content\par}").unwrap();
        let result = extract::extract(&rtf, 65536).unwrap();
        assert_eq!(result.status, "extracted");
        assert!(result.text.contains("Native RTF searchable content"));
    }
}

/// The EPUB spine determines reading order independently of ZIP insertion order.
#[test]
fn epub_reads_spine_order_and_rejects_expansion_over_budget() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let book = dir.path().join("ordered.epub");
    archive(
        &book,
        &[
            ("mimetype", "application/epub+zip"),
            (
                "META-INF/container.xml",
                "<container><rootfiles><rootfile full-path=\"OEBPS/content.opf\"/></rootfiles></container>",
            ),
            (
                "OEBPS/content.opf",
                "<package><metadata><title>书</title></metadata><manifest><item id=\"one\" href=\"one.xhtml\" media-type=\"application/xhtml+xml\"/><item id=\"two\" href=\"two.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"two\"/><itemref idref=\"one\"/></spine></package>",
            ),
            ("OEBPS/one.xhtml", "<html><body><p>第一章</p></body></html>"),
            ("OEBPS/two.xhtml", "<html><body><p>第二章</p></body></html>"),
        ],
    );
    let text = epub::read_epub(&book, 65536).unwrap();
    assert!(text.find("第二章").unwrap() < text.find("第一章").unwrap());
    assert!(epub::read_epub(&book, 64).is_err());
    let expanded = dir.path().join("expanded.epub");
    let large_chapter = format!(
        "<html><body><p>{}</p></body></html>",
        "Repeated text ".repeat(2000)
    );
    archive(
        &expanded,
        &[
            (
                "META-INF/container.xml",
                "<container><rootfile full-path=\"content.opf\"/></container>",
            ),
            (
                "content.opf",
                "<package><manifest><item id=\"one\" href=\"one.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"one\"/></spine></package>",
            ),
            ("one.xhtml", &large_chapter),
        ],
    );
    assert!(fs::metadata(&expanded).unwrap().len() < 4096);
    let error = epub::read_epub(&expanded, 4096).unwrap_err();
    assert!(error.to_string().contains("expanded-byte limit"));
}

/// EPUB resources that escape the container or identify encrypted chapters are rejected.
#[test]
fn epub_rejects_escaping_or_encrypted_chapters() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    let book = dir.path().join("unsafe.epub");
    let container = "<container><rootfile full-path=\"content.opf\"/></container>";
    archive(
        &book,
        &[
            ("META-INF/container.xml", container),
            (
                "content.opf",
                "<package><manifest><item id=\"one\" href=\"%2e%2e/outside.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"one\"/></spine></package>",
            ),
        ],
    );
    assert!(
        epub::read_epub(&book, 65536)
            .unwrap_err()
            .to_string()
            .contains("escapes")
    );
    archive(
        &book,
        &[
            ("META-INF/container.xml", container),
            (
                "content.opf",
                "<package><manifest><item id=\"one\" href=\"one.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"one\"/></spine></package>",
            ),
            (
                "META-INF/encryption.xml",
                "<encryption><EncryptedData><EncryptionMethod Algorithm=\"urn:encrypted\"/><CipherData><CipherReference URI=\"one.xhtml\"/></CipherData></EncryptedData></encryption>",
            ),
            ("one.xhtml", "<html><body>Protected</body></html>"),
        ],
    );
    assert!(
        epub::read_epub(&book, 65536)
            .unwrap_err()
            .to_string()
            .contains("encrypted")
    );
}

/// Export preserves sources, excludes nested roots, writes valid XHTML, and refuses overwrite.
#[test]
fn epub_export_is_readable_contained_and_no_clobber() {
    let _format = ensure_processing_account();
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("chapter.md"),
        "---\ntitle: 第一章\n---\n# 第一章\n\nA & B.\n\n![image](picture.png)",
    )
    .unwrap();
    fs::write(
        dir.path().join("chapter.html"),
        "<h2>第二章</h2><p>A&nbsp;B<br>文字</p><img src=\".omni/private.png\"><img src=\"%2egit/private.png\"><img src=\"linked.png\">",
    )
    .unwrap();
    fs::write(dir.path().join("picture.png"), [137, 80, 78, 71]).unwrap();
    fs::create_dir(dir.path().join("child")).unwrap();
    fs::write(dir.path().join("child/omni-knowledge.json"), "{}").unwrap();
    fs::write(dir.path().join("child/private.md"), "Must not export").unwrap();
    fs::create_dir(dir.path().join(".omni")).unwrap();
    fs::write(dir.path().join(".omni/internal.md"), "Internal").unwrap();
    fs::write(
        dir.path().join(".omni/private.png"),
        b"PRIVATE INTERNAL IMAGE",
    )
    .unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/private.png"), b"PRIVATE GIT IMAGE").unwrap();
    fs::write(
        dir.path().join("linked-target.png"),
        b"SYMLINK TARGET IMAGE",
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        dir.path().join("linked-target.png"),
        dir.path().join("linked.png"),
    )
    .unwrap();
    let output = dir.path().join("library.epub");
    assert_eq!(
        epub::export_epub(dir.path(), &output, "我的知识库", true).unwrap(),
        2
    );
    let original = fs::read(&output).unwrap();
    assert!(epub::export_epub(dir.path(), &output, "Other", true).is_err());
    assert_eq!(fs::read(&output).unwrap(), original);
    let mut zip = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
    let mime = zip.by_index(0).unwrap();
    assert_eq!(mime.name().unwrap(), "mimetype");
    assert_eq!(mime.compression(), zip::CompressionMethod::Stored);
    drop(mime);
    let names: Vec<String> = zip
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    assert_eq!(
        names
            .iter()
            .filter(|name| name.starts_with("OEBPS/media/"))
            .count(),
        1
    );
    for name in names
        .iter()
        .filter(|name| name.ends_with(".xhtml") || name.ends_with(".opf"))
    {
        let mut entry = zip.by_name(name).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut entry, &mut xml).unwrap();
        roxmltree::Document::parse(&xml).unwrap();
    }
    let text = epub::read_epub(&output, 65536).unwrap();
    assert!(text.contains("第一章"));
    assert!(text.contains("第二章"));
    assert!(!text.contains("Must not export"));
    assert!(!text.contains("Internal"));
    let preview = extract::preview_html(&output, 65536).unwrap();
    assert!(preview.contains("<h1>我的知识库</h1>"));
    assert!(!preview.contains("class=\"extracted-text\""));
    assert!(
        fs::read_to_string(dir.path().join("chapter.md"))
            .unwrap()
            .contains("# 第一章")
    );
}
