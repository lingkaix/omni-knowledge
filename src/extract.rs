//! Bounded local text extraction and inert document preview fragments.
//!
//! Originals remain authoritative; unsupported formats return metadata-only coverage.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use pulldown_cmark::{Event, Options, Parser, html};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Derived text and explicit coverage for one source file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Extraction {
    /// Searchable content, empty when no parser covers the file.
    pub text: String,
    /// Detected family or extension used to choose a parser.
    pub format: String,
    /// `extracted`, `partial`, `metadata-only`, or `failed` coverage.
    pub status: String,
    /// Missing capabilities, parse failures, or bounded-output notices.
    pub warnings: Vec<String>,
}

/// Read one regular file within `max_bytes` and derive text without changing the source.
///
/// Unknown binary formats remain metadata-only; parser errors become explicit failed coverage.
/// Files over the input limit remain metadata-only, and output truncation uses UTF-8 boundaries.
/// Returns an error for unreadable sources, non-regular files, or a zero limit.
pub fn extract(path: &Path, max_bytes: usize) -> Result<Extraction> {
    if max_bytes == 0 {
        bail!("The extraction byte limit must be positive");
    }
    let metadata = fs::metadata(path).with_context(|| format!("Read {}", path.display()))?;
    if !metadata.is_file() {
        bail!("Extraction requires a regular file: {}", path.display());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = match extension.as_str() {
        "md" | "markdown" | "mdown" => "markdown",
        "html" | "htm" | "xhtml" => "html",
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub" | "pdf" | "doc" | "rtf" => {
            &extension
        }
        _ => "unknown",
    }
    .to_string();
    if metadata.len() > max_bytes as u64 {
        return Ok(Extraction {
            text: String::new(),
            format,
            status: "metadata-only".into(),
            warnings: vec![format!(
                "Source exceeds the {max_bytes}-byte extraction limit"
            )],
        });
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("Read {}", path.display()))?;
    if bytes.len() > max_bytes {
        return Ok(Extraction {
            text: String::new(),
            format,
            status: "metadata-only".into(),
            warnings: vec![format!(
                "Source grew beyond the {max_bytes}-byte extraction limit"
            )],
        });
    }
    let detected_format = if bytes.starts_with(b"%PDF-") {
        "pdf"
    } else {
        format.as_str()
    };
    let mut resolved_format = detected_format.to_string();
    let parsed: Result<Option<String>> = match detected_format {
        "pdf" => std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes))
            .map_err(|_| anyhow!("PDF parser failed on this document"))
            .and_then(|result| result.map(Some).map_err(anyhow::Error::from)),
        "epub" => crate::epub::read_epub(path, max_bytes).map(Some),
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => {
            office_text(path, detected_format, max_bytes).map(Some)
        }
        "doc" | "rtf" => legacy_office_text(path, max_bytes).map(Some),
        "html" => String::from_utf8(bytes)
            .map_err(anyhow::Error::from)
            .and_then(|source| {
                html2text::from_read(source.as_bytes(), 120)
                    .map(Some)
                    .map_err(anyhow::Error::from)
            }),
        "markdown" => String::from_utf8(bytes)
            .map(Some)
            .map_err(anyhow::Error::from),
        _ => match String::from_utf8(bytes) {
            Ok(text) if looks_like_text(&text) => {
                resolved_format = if extension.is_empty() {
                    "text".into()
                } else {
                    extension
                };
                Ok(Some(text))
            }
            _ => Ok(None),
        },
    };
    match parsed {
        Ok(Some(mut text)) => {
            let truncated = truncate_utf8(&mut text, max_bytes);
            let empty = text.trim().is_empty();
            let mut warnings = Vec::new();
            if truncated {
                warnings.push(format!("Extracted text was limited to {max_bytes} bytes"));
            }
            if empty && detected_format == "pdf" {
                warnings.push(
                    "No native PDF text was found; scanned pages need an optional OCR parser"
                        .into(),
                );
            }
            Ok(Extraction {
                text,
                format: resolved_format,
                status: if truncated {
                    "partial"
                } else if empty {
                    "metadata-only"
                } else {
                    "extracted"
                }
                .into(),
                warnings,
            })
        }
        Ok(None) => Ok(Extraction {
            text: String::new(),
            format: resolved_format,
            status: "metadata-only".into(),
            warnings: Vec::new(),
        }),
        Err(error) => Ok(Extraction {
            text: String::new(),
            format: resolved_format,
            status: "failed".into(),
            warnings: vec![format!("{error:#}")],
        }),
    }
}

/// Parse leading YAML front matter and return its JSON value plus the remaining Markdown.
///
/// Invalid or unterminated front matter remains visible in the original text.
pub fn markdown_frontmatter(text: &str) -> (Value, String) {
    let source = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = source.split_inclusive('\n');
    if lines.next().map(str::trim) != Some("---") {
        return (Value::Null, text.to_string());
    }
    let first_end = source.find('\n').map_or(source.len(), |index| index + 1);
    let mut end = first_end;
    for line in lines {
        if matches!(line.trim(), "---" | "...") {
            let yaml = &source[first_end..end];
            if let Ok(value) = serde_yaml::from_str::<Value>(yaml) {
                return (value, source[end + line.len()..].to_string());
            }
            break;
        }
        end += line.len();
    }
    (Value::Null, text.to_string())
}

/// Render Markdown and front matter as a sanitized HTML fragment.
///
/// Raw HTML becomes inert text; fenced language classes remain available to the GUI renderer.
pub fn render_markdown(text: &str) -> String {
    let (metadata, body) = markdown_frontmatter(text);
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let events = Parser::new_ext(&body, options).map(|event| match event {
        Event::Html(value) | Event::InlineHtml(value) => Event::Text(value),
        other => other,
    });
    let mut result = String::new();
    if !metadata.is_null() {
        let json = serde_json::to_string_pretty(&metadata).unwrap_or_default();
        result.push_str("<details class=\"frontmatter\" open><summary>Front matter</summary><pre>");
        result.push_str(&escape_html(&json));
        result.push_str("</pre></details>\n");
    }
    html::push_html(&mut result, events);
    sanitize_html(&result)
}

/// Produce a sanitized fragment for local Markdown, HTML, EPUB, or extracted text preview.
///
/// Relative resource paths stay relative for the GUI's contained local protocol.
/// Network images, active HTML, event handlers, and dangerous URL schemes are removed.
/// Returns an error for an unreadable source; parser coverage notices remain visible.
pub fn preview_html(path: &Path, max_bytes: usize) -> Result<String> {
    let result = extract(path, max_bytes)?;
    if matches!(result.format.as_str(), "markdown" | "epub") && result.status == "extracted" {
        return Ok(render_markdown(&result.text));
    }
    if result.format == "html" && result.status != "metadata-only" {
        let mut bytes = Vec::new();
        File::open(path)?
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes {
            bail!("HTML source grew beyond the preview limit");
        }
        let source = String::from_utf8(bytes)?;
        return Ok(sanitize_html(&source));
    }
    let mut preview = format!(
        "<h2>{}</h2>",
        escape_html(&path.file_name().unwrap_or_default().to_string_lossy())
    );
    if !result.warnings.is_empty() {
        preview.push_str(&format!(
            "<p class=\"notice\">{}</p>",
            escape_html(&result.warnings.join("; "))
        ));
    }
    if result.text.is_empty() {
        preview
            .push_str("<p>Metadata-only coverage. Use the native preview to open this format.</p>");
    } else {
        preview.push_str("<pre class=\"extracted-text\">");
        preview.push_str(&escape_html(&result.text));
        preview.push_str("</pre>");
    }
    Ok(preview)
}

/// Remove executable markup while preserving common document structure and local resources.
pub(crate) fn sanitize_html(source: &str) -> String {
    let mut cleaner = ammonia::Builder::default();
    cleaner.add_tags(["details", "summary", "audio", "video", "source"]);
    cleaner.add_generic_attributes(["class"]);
    cleaner.add_tag_attributes("audio", ["controls", "src"]);
    cleaner.add_tag_attributes("video", ["controls", "src", "poster"]);
    cleaner.add_tag_attributes("source", ["src", "type"]);
    cleaner.url_schemes(HashSet::from(["http", "https", "mailto"]));
    cleaner.attribute_filter(|_, attribute, value| {
        if matches!(attribute, "src" | "poster")
            && (!is_relative_resource(value) || value.starts_with('#'))
        {
            None
        } else {
            Some(value.into())
        }
    });
    cleaner.clean(source).to_string()
}

/// Recognize relative resources without interpreting file or network schemes.
pub(crate) fn is_relative_resource(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains(':')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
}

/// Escape text or attribute values for HTML and XML markup.
pub(crate) fn escape_html(text: &str) -> String {
    text.chars()
        .filter(|character| valid_xml_character(*character))
        .fold(String::new(), |mut escaped, character| {
            match character {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&#39;"),
                other => escaped.push(other),
            }
            escaped
        })
}

/// Return whether a code point is permitted by XML 1.0.
pub(crate) fn valid_xml_character(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\r')
        || (character >= '\u{20}' && character != '\u{fffe}' && character != '\u{ffff}')
}

/// Remove bytes beyond a limit without splitting a UTF-8 code point.
fn truncate_utf8(text: &mut String, max_bytes: usize) -> bool {
    if text.len() <= max_bytes {
        return false;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    true
}

/// Reject NUL and binary control bytes while allowing normal text whitespace.
fn looks_like_text(text: &str) -> bool {
    !text.chars().any(|character| {
        character == '\0'
            || (character.is_control() && !matches!(character, '\n' | '\r' | '\t' | '\u{c}'))
    })
}

/// Read one ZIP member against a shared expanded-byte budget without extracting paths.
pub(crate) fn read_zip_text(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
    remaining: &mut usize,
) -> Result<String> {
    let mut member = archive
        .by_name(name)
        .with_context(|| format!("Read archive member {name}"))?;
    if member.size() > *remaining as u64 {
        bail!("Archive text exceeds the expanded-byte limit at {name}");
    }
    let mut bytes = Vec::new();
    member
        .by_ref()
        .take(*remaining as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > *remaining {
        bail!("Archive text exceeds the expanded-byte limit at {name}");
    }
    *remaining -= bytes.len();
    String::from_utf8(bytes).with_context(|| format!("Archive member is not UTF-8: {name}"))
}

/// Extract text runs and paragraphs from DOCX, PPTX, XLSX, and OpenDocument containers.
fn office_text(path: &Path, format: &str, max_bytes: usize) -> Result<String> {
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    if archive.len() > 100_000 {
        bail!("Office archive contains too many members");
    }
    let mut remaining = max_bytes;
    match format {
        "docx" => {
            let mut names: Vec<String> = archive
                .file_names()
                .filter(|name| {
                    *name == "word/document.xml"
                        || (name.starts_with("word/")
                            && (name.contains("header")
                                || name.contains("footer")
                                || name.contains("footnotes")
                                || name.contains("endnotes"))
                            && name.ends_with(".xml"))
                })
                .map(str::to_owned)
                .collect();
            names.sort_by_key(|name| (name != "word/document.xml", name.clone()));
            let mut output = String::new();
            for name in names {
                output.push_str(&xml_paragraph_text(
                    &read_zip_text(&mut archive, &name, &mut remaining)?,
                    "t",
                )?);
            }
            Ok(output)
        }
        "pptx" => {
            let mut names: Vec<String> = archive
                .file_names()
                .filter(|name| {
                    name.starts_with("ppt/slides/slide")
                        && name.ends_with(".xml")
                        && !name.contains("/_rels/")
                })
                .map(str::to_owned)
                .collect();
            names.sort_by_key(|name| numeric_member_order(name));
            let mut output = String::new();
            for (index, name) in names.iter().enumerate() {
                output.push_str(&format!("\n## Slide {}\n\n", index + 1));
                output.push_str(&xml_paragraph_text(
                    &read_zip_text(&mut archive, name, &mut remaining)?,
                    "t",
                )?);
            }
            Ok(output)
        }
        "xlsx" => spreadsheet_text(&mut archive, &mut remaining),
        _ => xml_paragraph_text(
            &read_zip_text(&mut archive, "content.xml", &mut remaining)?,
            "",
        ),
    }
}

/// Return the numeric suffix for natural ordering of archive slide or worksheet names.
fn numeric_member_order(name: &str) -> usize {
    let stem = name
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .trim_end_matches(".xml");
    let number: String = stem
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .collect();
    number.parse().unwrap_or(usize::MAX)
}

/// Extract each paragraph once while preserving adjacent text runs within it.
fn xml_paragraph_text(source: &str, text_tag: &str) -> Result<String> {
    let document = roxmltree::Document::parse(source)?;
    let mut output = String::new();
    for paragraph in document
        .descendants()
        .filter(|node| node.is_element() && matches!(node.tag_name().name(), "p" | "h"))
    {
        if paragraph.ancestors().skip(1).any(|ancestor| {
            ancestor.is_element() && matches!(ancestor.tag_name().name(), "p" | "h")
        }) {
            continue;
        }
        for node in paragraph.descendants() {
            if node.is_text()
                && (text_tag.is_empty()
                    || node
                        .parent()
                        .is_some_and(|parent| parent.tag_name().name() == text_tag))
            {
                output.push_str(node.text().unwrap_or(""));
            } else if node.is_element() && matches!(node.tag_name().name(), "tab" | "s") {
                output.push('\t');
            } else if node.is_element() && matches!(node.tag_name().name(), "br" | "line-break") {
                output.push('\n');
            }
        }
        output.push('\n');
    }
    Ok(output)
}

/// Resolve XLSX shared and inline strings and retain numeric values in worksheet order.
fn spreadsheet_text(archive: &mut zip::ZipArchive<File>, remaining: &mut usize) -> Result<String> {
    let shared = if archive
        .file_names()
        .any(|name| name == "xl/sharedStrings.xml")
    {
        let xml = read_zip_text(archive, "xl/sharedStrings.xml", remaining)?;
        let document = roxmltree::Document::parse(&xml)?;
        document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "si")
            .map(|node| {
                node.descendants()
                    .filter(|child| {
                        child.is_text()
                            && child
                                .parent()
                                .is_some_and(|parent| parent.tag_name().name() == "t")
                    })
                    .filter_map(|child| child.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut names: Vec<String> = archive
        .file_names()
        .filter(|name| name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml"))
        .map(str::to_owned)
        .collect();
    names.sort_by_key(|name| numeric_member_order(name));
    let mut output = String::new();
    for name in names {
        output.push_str(&format!("\n## {name}\n\n"));
        let xml = read_zip_text(archive, &name, remaining)?;
        let document = roxmltree::Document::parse(&xml)?;
        for row in document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "row")
        {
            let mut cells = Vec::new();
            for cell in row
                .children()
                .filter(|node| node.is_element() && node.tag_name().name() == "c")
            {
                let value = cell
                    .descendants()
                    .find(|node| node.is_element() && node.tag_name().name() == "v")
                    .and_then(|node| node.text())
                    .unwrap_or("");
                let text = match cell.attribute("t") {
                    Some("s") => value
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| shared.get(index))
                        .cloned()
                        .unwrap_or_default(),
                    Some("inlineStr") => cell
                        .descendants()
                        .filter(|node| {
                            node.is_text()
                                && node
                                    .parent()
                                    .is_some_and(|parent| parent.tag_name().name() == "t")
                        })
                        .filter_map(|node| node.text())
                        .collect(),
                    _ => value.to_string(),
                };
                cells.push(text);
            }
            output.push_str(&cells.join("\t"));
            output.push('\n');
        }
    }
    Ok(output)
}

/// Use the existing macOS converter for legacy Office and RTF with bounded output and timeout.
fn legacy_office_text(path: &Path, max_bytes: usize) -> Result<String> {
    if !cfg!(target_os = "macos") {
        bail!("Legacy DOC/RTF extraction requires macOS textutil or an optional parser");
    }
    let source = fs::canonicalize(path)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut child = Command::new("/usr/bin/textutil")
        .args(["-convert", "txt", "-stdout"])
        .arg(source)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Start macOS textutil")?;
    let stdout = child.stdout.take().context("Read textutil output")?;
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let result = receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    let mut timed_out = matches!(result, Err(mpsc::RecvTimeoutError::Timeout));
    if !result
        .as_ref()
        .is_ok_and(|value| value.as_ref().is_ok_and(|bytes| bytes.len() <= max_bytes))
    {
        let _ = child.kill();
    }
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let _ = reader.join();
    if timed_out {
        bail!("textutil exceeded its 30-second conversion timeout");
    }
    let bytes = result.context("textutil exceeded its 30-second conversion timeout")??;
    if bytes.len() > max_bytes {
        bail!("textutil output exceeds the extraction limit");
    }
    if !status.success() {
        bail!("textutil could not convert this document");
    }
    String::from_utf8(bytes).context("textutil output is not UTF-8")
}
