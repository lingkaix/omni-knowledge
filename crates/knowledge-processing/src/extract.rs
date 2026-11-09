//! Bounded local text extraction and inert document preview fragments.
//!
//! Originals remain authoritative; unsupported formats return metadata-only coverage.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use pulldown_cmark::{Event, Options, Parser, html};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::admission::{self, Admitted};
use crate::funding::{Credits, Funding, read_fd};
use crate::worker::WorkerFailure;

pub use crate::admission::establish_process_account;

/// Keep source-line markup bounded even when a small source contains many empty lines.
const MAX_PREVIEW_LINES: usize = 20_000;

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
/// Requires [`admission::current`] before the open. The returned text's live-memory
/// holding is not finished: [`knowledge_core::admission::Holding`] does not release
/// on drop, so this compatibility path keeps the charge until process exit.
/// Hosts that can finish the holding call [`extract_file`].
pub fn extract(path: &Path, max_bytes: usize) -> Result<Extraction> {
    Ok(extract_path(path, max_bytes)?.value)
}

/// Extract one path and return the live-text holdings with the result.
pub fn extract_path(path: &Path, max_bytes: usize) -> Result<Admitted<Extraction>> {
    admission::current()?;
    let file = File::open(path).with_context(|| format!("Read {}", path.display()))?;
    crate::worker::retain_source(extract_file(&file, path, max_bytes, None), file)
}

/// Extract an already admitted file. `hint` supplies the extension only.
///
/// `child_rss_bytes` is the live promise for an opaque parser child. `None`
/// uses the account's current `memory_available` after the input reservation.
/// It does not skip the watchdog.
pub fn extract_file(
    file: &File,
    hint: &Path,
    max_bytes: usize,
    child_rss_bytes: Option<u64>,
) -> Result<Admitted<Extraction>> {
    extract_file_funded(
        file,
        hint,
        max_bytes,
        child_rss_bytes,
        &mut Funding::BeforeVerification,
    )
}

/// Extract from the admitted descriptor using existing claim credit or explicit pre-verification funding.
/// The native caller must establish current source version before and after this read; a live FD alone is not version proof.
pub fn extract_file_funded(
    file: &File,
    hint: &Path,
    max_bytes: usize,
    child_rss_bytes: Option<u64>,
    funding: &mut Funding<'_>,
) -> Result<Admitted<Extraction>> {
    admission::current()?;
    funding.checkpoint()?;
    if max_bytes == 0 {
        bail!("The extraction byte limit must be positive");
    }
    let metadata = file
        .metadata()
        .with_context(|| format!("Read {}", hint.display()))?;
    if !metadata.is_file() {
        bail!("Extraction requires a regular file: {}", hint.display());
    }
    let extension = hint
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = classify(&extension);
    if metadata.len() > max_bytes as u64 {
        return Ok(Admitted {
            value: metadata_only(
                &format,
                format!("Source exceeds the {max_bytes}-byte extraction limit"),
            ),
            retained: Vec::new(),
        });
    }
    let capacity = usize::try_from(metadata.len())?
        .checked_add(1)
        .context("Source size overflow")?;
    let mut magic = [0u8; 5];
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        let _ = file.read_at(&mut magic, 0)?;
    }
    let detected = if &magic == b"%PDF-" {
        "pdf".to_owned()
    } else {
        format
    };
    let child_needed = opaque_mode(&detected) || matches!(detected.as_str(), "doc" | "rtf");
    let cap = crate::opaque::transport_cap(max_bytes);
    let output = if child_needed {
        cap.checked_mul(8)
            .ok_or(admission::AdmitError::TooLargeMemory)?
    } else {
        metadata.len()
    };
    let mut credits = Credits::new(
        funding,
        capacity as u64,
        output,
        if child_needed {
            crate::opaque::capture_memory(cap)
        } else {
            0
        },
        if child_needed {
            child_rss_bytes
        } else {
            Some(0)
        },
        0,
    )?;
    let bytes = read_fd(file, capacity).with_context(|| format!("Read {}", hint.display()))?;
    if bytes.len() > metadata.len() as usize {
        drop(bytes);
        credits.release_input()?;
        return Ok(Admitted {
            value: metadata_only(
                &detected,
                format!("Source grew during the bounded {max_bytes}-byte extraction read"),
            ),
            retained: Vec::new(),
        });
    }
    let extraction = if detected == "doc" || detected == "rtf" {
        let text = legacy_office_text(file, max_bytes, &mut credits, funding)?;
        text_extraction(&detected, text, false, max_bytes)
    } else if opaque_mode(&detected) {
        let mode = if detected == "html" || detected == "epub" || detected == "pdf" {
            detected.clone()
        } else {
            format!("office:{detected}")
        };
        match crate::opaque::parse_in_worker(&mode, &bytes, max_bytes, &mut credits, funding) {
            Ok(extraction) => extraction,
            Err(error)
                if error.chain().any(|cause| {
                    cause.is::<admission::AdmitError>()
                        || cause.is::<WorkerFailure>()
                        || cause.is::<crate::worker::UnsettledChild>()
                }) =>
            {
                return Err(error);
            }
            Err(error) => Extraction {
                text: String::new(),
                format: detected.clone(),
                status: "failed".into(),
                warnings: vec![format!("{error:#}")],
            },
        }
    } else {
        parse_loaded(&detected, &extension, &bytes, max_bytes)?
    };
    drop(bytes);
    credits.release_input()?;
    let retained =
        credits.retained_for(!extraction.text.is_empty() || !extraction.warnings.is_empty());
    Ok(Admitted {
        value: extraction,
        retained,
    })
}

/// Parse bytes that are already in memory. The parser worker calls this.
///
/// Hosts use [`extract_file`]. This function does not reserve on the account.
pub fn parse_loaded(
    mode: &str,
    extension: &str,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<Extraction> {
    if max_bytes == 0 {
        bail!("The extraction byte limit must be positive");
    }
    let detected_format = mode.strip_prefix("office:").unwrap_or(mode).to_string();
    let extension = extension.to_string();
    let mut resolved_format = detected_format.to_string();
    let mut output_truncated = false;
    let parsed: Result<Option<String>> = match detected_format.as_str() {
        "pdf" => std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes))
            .map_err(|_| anyhow!("PDF parser failed on this document"))
            .and_then(|result| result.map(Some).map_err(anyhow::Error::from)),
        "epub" => crate::epub::read_epub_bytes(bytes, max_bytes).map(Some),
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => office_text(
            bytes,
            detected_format.as_str(),
            max_bytes,
            &mut output_truncated,
        )
        .map(Some),
        "doc" | "rtf" => Err(anyhow!(
            "Legacy DOC/RTF extraction requires macOS textutil or an optional parser"
        )),
        "html" => String::from_utf8(bytes.to_vec())
            .map_err(anyhow::Error::from)
            .and_then(|source| {
                html2text::from_read(source.as_bytes(), 120)
                    .map(Some)
                    .map_err(anyhow::Error::from)
            }),
        "markdown" => String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(anyhow::Error::from),
        _ => match String::from_utf8(bytes.to_vec()) {
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
            let truncated = truncate_utf8(&mut text, max_bytes) || output_truncated;
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
        Err(error)
            if error
                .downcast_ref::<crate::admission::AdmitError>()
                .is_some()
                || error
                    .downcast_ref::<crate::worker::WorkerFailure>()
                    .is_some() =>
        {
            Err(error)
        }
        Err(error) => Ok(Extraction {
            text: String::new(),
            format: resolved_format,
            status: "failed".into(),
            warnings: vec![format!("{error:#}")],
        }),
    }
}

fn classify(extension: &str) -> String {
    match extension {
        "md" | "markdown" | "mdown" => "markdown",
        "html" | "htm" | "xhtml" => "html",
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub" | "pdf" | "doc" | "rtf" => {
            extension
        }
        _ => "unknown",
    }
    .to_string()
}

fn opaque_mode(format: &str) -> bool {
    matches!(
        format,
        "pdf" | "html" | "epub" | "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp"
    )
}

fn metadata_only(format: &str, warning: String) -> Extraction {
    Extraction {
        text: String::new(),
        format: format.to_string(),
        status: "metadata-only".into(),
        warnings: vec![warning],
    }
}

fn text_extraction(
    format: &str,
    mut text: String,
    truncated: bool,
    max_bytes: usize,
) -> Extraction {
    let truncated = truncate_utf8(&mut text, max_bytes) || truncated;
    let empty = text.trim().is_empty();
    let mut warnings = Vec::new();
    if truncated {
        warnings.push(format!("Extracted text was limited to {max_bytes} bytes"));
    }
    Extraction {
        text,
        format: format.to_string(),
        status: if truncated {
            "partial"
        } else if empty {
            "metadata-only"
        } else {
            "extracted"
        }
        .into(),
        warnings,
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
    preview_html_at_line(path, max_bytes, None)
}

/// Produce a safe preview with numbered source or extracted lines and optional line evidence.
///
/// Markdown uses original source lines, including front matter; HTML, EPUB, PDF, and Office
/// use the same derived text lines that enter the index, rather than invented source positions.
/// Formatted documents retain their rendered view and expose their numbered text in a disclosure.
/// A requested line opens that disclosure and marks exactly one available row for the GUI renderer.
/// Large text uses a bounded row window around the evidence; missing lines produce a notice,
/// without highlighting a substitute. Returns the same source-read errors as `preview_html`.
pub fn preview_html_at_line(path: &Path, max_bytes: usize, line: Option<usize>) -> Result<String> {
    preview_html_body(path, max_bytes, line)
}

fn preview_html_body(path: &Path, max_bytes: usize, line: Option<usize>) -> Result<String> {
    admission::current()?;
    let file = File::open(path).with_context(|| format!("Read {}", path.display()))?;
    let result = preview_file_funded(
        &file,
        path,
        max_bytes,
        line,
        None,
        &mut Funding::BeforeVerification,
    );
    Ok(crate::worker::retain_source(result, file)?.value)
}

/// Render an admitted file in the same real parser process, with prepaid input/transport/result lifetimes.
/// Caller ownership/version obligations are identical to extract_file_funded; this does not authorize paths.
pub fn preview_file_funded(
    file: &File,
    hint: &Path,
    max_bytes: usize,
    line: Option<usize>,
    child_rss: Option<u64>,
    funding: &mut Funding<'_>,
) -> Result<Admitted<String>> {
    admission::current()?;
    funding.checkpoint()?;
    ensure_preview_limit(max_bytes)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        bail!("Preview requires a regular file");
    }
    let oversized = metadata.len() > max_bytes as u64;
    let capacity = if oversized {
        0
    } else {
        usize::try_from(metadata.len())?
            .checked_add(1)
            .context("Preview source overflow")?
    };
    let extension = hint
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = classify(&extension);
    let mut magic = [0u8; 5];
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        let _ = file.read_at(&mut magic, 0)?;
    }
    let effective = if &magic == b"%PDF-" { "pdf" } else { &format };
    let legacy = matches!(effective, "doc" | "rtf");
    let size = if opaque_mode(effective) || legacy {
        max_bytes as u64
    } else {
        metadata.len().min(max_bytes as u64)
    };
    let cap = size
        .checked_mul(32)
        .and_then(|n| n.checked_add(MAX_PREVIEW_LINES as u64 * 512 + 65_536))
        .context("Preview output overflow")?;
    let mut credits = Credits::new(
        funding,
        capacity as u64,
        cap,
        crate::opaque::capture_memory(cap),
        child_rss,
        0,
    )?;
    let bytes = read_fd(file, capacity)?;
    if !oversized && bytes.len() as u64 > metadata.len() {
        bail!("Preview source grew during bounded descriptor read");
    }
    let legacy_text = if legacy && !oversized {
        Some(legacy_office_text(file, max_bytes, &mut credits, funding)?)
    } else {
        None
    };
    let kind = if oversized {
        "oversized"
    } else if legacy {
        "legacy"
    } else {
        "source"
    };
    let payload = legacy_text
        .as_ref()
        .map_or(bytes.as_slice(), |text| text.as_bytes());
    let filename = hint.file_name().unwrap_or_default().to_string_lossy();
    let mut command = Command::new(crate::worker::parser_worker_path()?);
    command.args([
        "preview",
        kind,
        &extension,
        &filename,
        &max_bytes.to_string(),
        &line.map_or_else(|| "none".to_owned(), |value| value.to_string()),
    ]);
    let capture = crate::worker::run(
        command,
        Some(payload),
        cap,
        Duration::from_secs(120),
        &mut credits,
        funding,
        None,
    )?;
    let value =
        String::from_utf8(capture.bytes).context("Preview worker returned invalid UTF-8")?;
    drop(legacy_text);
    drop(bytes);
    credits.release_input()?;
    let retained = credits.retained();
    Ok(Admitted { value, retained })
}

/// Keep preview and extraction's existing positive byte-limit rule identical.
fn ensure_preview_limit(max_bytes: usize) -> Result<()> {
    if max_bytes == 0 {
        bail!("The extraction byte limit must be positive");
    }
    Ok(())
}

/// Pure mature rendering inside the watched worker; source markup remains inert and line clocks stay unchanged.
pub fn preview_loaded(
    kind: &str,
    extension: &str,
    filename: &str,
    bytes: &[u8],
    max_bytes: usize,
    line: Option<usize>,
) -> Result<String> {
    let result = if kind == "oversized" {
        metadata_only(
            &classify(extension),
            format!("Source exceeds the {max_bytes}-byte extraction limit"),
        )
    } else if kind == "legacy" {
        text_extraction(
            extension,
            String::from_utf8(bytes.to_vec())?,
            false,
            max_bytes,
        )
    } else {
        let detected = if bytes.starts_with(b"%PDF-") {
            "pdf".to_string()
        } else {
            classify(extension)
        };
        parse_loaded(&detected, extension, bytes, max_bytes)?
    };
    if matches!(result.format.as_str(), "markdown" | "epub") && result.status == "extracted" {
        return Ok(rendered_line_preview(
            &render_markdown(&result.text),
            &result.text,
            line,
            if result.format == "markdown" {
                "Source lines"
            } else {
                "Extracted text lines"
            },
        ));
    }
    if result.format == "html" && result.status != "metadata-only" {
        let source = std::str::from_utf8(bytes)?;
        return Ok(rendered_line_preview(
            &sanitize_html(source),
            &result.text,
            line,
            "Extracted text lines",
        ));
    }
    let mut preview = format!("<h2>{}</h2>", escape_html(filename));
    if !result.warnings.is_empty() {
        preview.push_str(&format!(
            "<p class=\"notice\">{}</p>",
            escape_html(&result.warnings.join("; "))
        ));
    }
    if result.text.is_empty() {
        if let Some(line) = line {
            preview.push_str(&unavailable_line_notice(line));
        }
        preview
            .push_str("<p>Metadata-only coverage. Use the native preview to open this format.</p>");
    } else {
        if matches!(
            result.format.as_str(),
            "pdf" | "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "doc" | "rtf"
        ) {
            preview.push_str("<p class=\"line-origin\">Extracted text lines</p>");
        }
        preview.push_str(&numbered_text(&result.text, line));
    }
    Ok(preview)
}

/// Preserve a formatted document while providing exact numbered text in a separate reading view.
/// Only trusted extraction output can create the target marker; imported markup stays sanitized.
fn rendered_line_preview(rendered: &str, text: &str, line: Option<usize>, label: &str) -> String {
    format!(
        "<div class=\"text-preview\"><details class=\"line-preview\"{}><summary>{label}<span class=\"line-view-hint\"> · Close to return to formatted view</span></summary>{}</details><section class=\"rendered-document\">{rendered}</section></div>",
        if line.is_some() { " open" } else { "" },
        numbered_text(text, line),
    )
}

/// Explain stale or invalid line evidence without pretending that another row is its location.
fn unavailable_line_notice(line: usize) -> String {
    format!(
        "<p class=\"warning\">Line {line} is not available in the current text. The file or index may have changed.</p>"
    )
}

/// Escape indexed text into bounded, one-based rows with an unselectable number gutter.
/// The active evidence line receives a trusted marker for scrolling after document layout.
fn numbered_text(text: &str, line: Option<usize>) -> String {
    let count = text.lines().count();
    let target = line.filter(|line| *line > 0 && *line <= count);
    let start = target
        .map(|line| line.saturating_sub(MAX_PREVIEW_LINES / 2).max(1))
        .unwrap_or(1)
        .min(count.saturating_sub(MAX_PREVIEW_LINES) + 1);
    let end = start.saturating_add(MAX_PREVIEW_LINES - 1).min(count);
    let mut result = String::new();
    if let Some(line) = line.filter(|_| target.is_none()) {
        result.push_str(&unavailable_line_notice(line));
    }
    if count > MAX_PREVIEW_LINES {
        result.push_str(&format!(
            "<p class=\"warning\">Showing lines {start}–{end} of {count}. Use Open original to read the remaining lines.</p>"
        ));
    }
    result.push_str(&format!(
        "<div class=\"numbered-text\" role=\"region\" aria-label=\"Text with line numbers\" style=\"--line-number-width:{}ch\">",
        end.to_string().len() + 3,
    ));
    for (index, text) in text
        .lines()
        .enumerate()
        .skip(start - 1)
        .take(MAX_PREVIEW_LINES)
    {
        let number = index + 1;
        result.push_str(&format!(
            "<div class=\"text-line{}\" id=\"source-line-{number}\"{}><span class=\"line-number\" aria-hidden=\"true\">{number}</span><span class=\"line-content\">{}</span></div>",
            if target == Some(number) { " is-search-hit" } else { "" },
            if target == Some(number) { " data-preview-target=\"true\"" } else { "" },
            escape_html(text),
        ));
    }
    result.push_str("</div>");
    result
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
pub(crate) fn read_zip_text<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
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

/// Decode selected ZIP names without discarding malformed member names or allocating an unselected inventory.
fn selected_zip_names<R: Read + Seek>(
    archive: &zip::ZipArchive<R>,
    select: impl Fn(&str) -> bool,
) -> Result<Vec<String>> {
    archive
        .file_names()
        .filter_map(|name| match name {
            Ok(name) if select(&name) => Some(Ok(name.into_owned())),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<zip::result::ZipResult<Vec<_>>>()
        .map_err(Into::into)
}

/// Extract Office text against the shared XML byte budget; return read/parse/admission errors.
/// Set output_truncated when XLSX applies max_bytes during assembly rather than after return.
fn office_text(
    bytes: &[u8],
    format: &str,
    max_bytes: usize,
    output_truncated: &mut bool,
) -> Result<String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    if archive.len() > 100_000 {
        bail!("Office archive contains too many members");
    }
    archive.validate_header_consistency()?;
    let mut remaining = max_bytes;
    match format {
        "docx" => {
            let mut names = selected_zip_names(&archive, |name| {
                name == "word/document.xml"
                    || (name.starts_with("word/")
                        && (name.contains("header")
                            || name.contains("footer")
                            || name.contains("footnotes")
                            || name.contains("endnotes"))
                        && name.ends_with(".xml"))
            })?;
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
            let mut names = selected_zip_names(&archive, |name| {
                name.starts_with("ppt/slides/slide")
                    && name.ends_with(".xml")
                    && !name.contains("/_rels/")
            })?;
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
        "xlsx" => {
            let (text, truncated) = spreadsheet_text(&mut archive, &mut remaining, max_bytes)?;
            *output_truncated = truncated;
            Ok(text)
        }
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

/// One XLSX representation prefix; no per-cell owned text or full row/output staging.
struct SpreadsheetOutput {
    /// Longest fitting UTF-8 prefix of the original ordered representation.
    text: String,
    /// Existing caller-selected representation ceiling, not an independent format limit.
    max_bytes: usize,
    /// A nonempty suffix was omitted; later runs must not fill a split character's unused bytes.
    truncated: bool,
}

impl SpreadsheetOutput {
    /// Choose the fitting UTF-8 prefix before copying text; remember any omitted suffix.
    fn push(&mut self, text: &str) {
        if self.truncated {
            return;
        }
        let end = text.floor_char_boundary((self.max_bytes - self.text.len()).min(text.len()));
        self.text.push_str(&text[..end]);
        self.truncated = end < text.len();
    }

    /// Append borrowed rich/inline text runs in document order without joining or cloning them.
    fn push_runs(&mut self, node: roxmltree::Node<'_, '_>) {
        for child in node.descendants() {
            if self.truncated {
                break;
            }
            if child.is_text()
                && child
                    .parent()
                    .is_some_and(|parent| parent.tag_name().name() == "t")
            {
                self.push(child.text().unwrap_or(""));
            }
        }
    }
}

/// Resolve borrowed XLSX shared/inline runs and numeric values into a bounded ordered prefix.
/// Charge all selected XML to remaining, even after output truncation, retaining late failures.
/// Return text plus omission state for public partial coverage; propagate ZIP/XML/budget errors.
fn spreadsheet_text<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    remaining: &mut usize,
    max_bytes: usize,
) -> Result<(String, bool)> {
    let shared_xml = if archive.index_for_name("xl/sharedStrings.xml").is_some() {
        Some(read_zip_text(archive, "xl/sharedStrings.xml", remaining)?)
    } else {
        None
    };
    let shared_document = shared_xml
        .as_deref()
        .map(roxmltree::Document::parse)
        .transpose()?;
    let shared = shared_document
        .as_ref()
        .map(|document| {
            document
                .descendants()
                .filter(|node| node.is_element() && node.tag_name().name() == "si")
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut names = selected_zip_names(archive, |name| {
        name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml")
    })?;
    names.sort_by_key(|name| numeric_member_order(name));
    let mut output = SpreadsheetOutput {
        text: String::new(),
        max_bytes,
        truncated: false,
    };
    for name in names {
        output.push("\n## ");
        output.push(&name);
        output.push("\n\n");
        let xml = read_zip_text(archive, &name, remaining)?;
        let document = roxmltree::Document::parse(&xml)?;
        for row in document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "row")
        {
            for (index, cell) in row
                .children()
                .filter(|node| node.is_element() && node.tag_name().name() == "c")
                .enumerate()
            {
                if index != 0 {
                    output.push("\t");
                }
                let value = cell
                    .descendants()
                    .find(|node| node.is_element() && node.tag_name().name() == "v")
                    .and_then(|node| node.text())
                    .unwrap_or("");
                match cell.attribute("t") {
                    Some("s") => {
                        if let Some(node) = value
                            .parse::<usize>()
                            .ok()
                            .and_then(|index| shared.get(index))
                        {
                            output.push_runs(*node);
                        }
                    }
                    Some("inlineStr") => output.push_runs(cell),
                    _ => output.push(value),
                }
            }
            output.push("\n");
        }
    }
    Ok((output.text, output.truncated))
}

/// Use the existing macOS converter for legacy Office and RTF with bounded output and timeout.
fn legacy_office_text(
    file: &File,
    max_bytes: usize,
    credits: &mut Credits,
    funding: &Funding<'_>,
) -> Result<String> {
    if !cfg!(target_os = "macos") {
        bail!("Legacy DOC/RTF extraction requires macOS textutil or an optional parser");
    }
    let _inherited = crate::worker::InheritedFd::new(file);
    let mut command = Command::new("/usr/bin/textutil");
    command
        .args(["-convert", "txt", "-stdout"])
        .arg(fd_path(file));
    // The same nonblocking loop continuously samples RSS, including while textutil is silent.
    let capture = crate::worker::run(
        command,
        None,
        max_bytes as u64,
        Duration::from_secs(30),
        credits,
        funding,
        None,
    )?;
    String::from_utf8(capture.bytes).context("textutil output is not UTF-8")
}

fn fd_path(file: &File) -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd())
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::io::AsRawFd;
        format!("/dev/fd/{}", file.as_raw_fd())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = file;
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn xlsx_assembly_keeps_shared_strings_and_original_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sheet.xlsx");
        let file = File::create(&path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        archive.start_file("xl/sharedStrings.xml", options).unwrap();
        archive
            .write_all(br#"<sst><si><t>Alpha</t></si><si><t>Beta</t></si></sst>"#)
            .unwrap();
        archive
            .start_file("xl/worksheets/sheet1.xml", options)
            .unwrap();
        archive
            .write_all(
                br#"<worksheet><sheetData><row><c t="s"><v>0</v></c><c t="s"><v>1</v></c></row></sheetData></worksheet>"#,
            )
            .unwrap();
        archive.finish().unwrap();
        let original = fs::read(&path).unwrap();
        let extracted = parse_loaded("office:xlsx", "xlsx", &original, 65536).unwrap();
        assert!(extracted.text.contains("Alpha"), "{}", extracted.text);
        assert!(extracted.text.contains("Beta"), "{}", extracted.text);
        assert_ne!(extracted.status, "failed");
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}
