//! Local EPUB spine extraction and portable knowledge-library packaging.
//!
//! Archive members are read in memory; source files are never modified or unpacked.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

use crate::admission::{self, Charge};

use crate::extract::{
    escape_html, is_relative_resource, markdown_frontmatter, read_zip_text, render_markdown,
    sanitize_html, valid_xml_character,
};

/// One source chapter and its stable publication location.
struct Chapter {
    /// Canonical source file used to resolve contained relative links.
    source: PathBuf,
    /// User-facing title from front matter or the source filename.
    title: String,
    /// Archive path relative to the publication directory.
    href: String,
    /// Sanitized body before XML-compatible serialization.
    html: String,
}

/// A local media file referenced by an exported chapter.
struct Media {
    /// Canonical source path for bounded copying.
    source: PathBuf,
    /// Archive path relative to the publication directory.
    href: String,
    /// MIME type declared in the publication manifest.
    mime: String,
}

/// Read UTF-8 EPUB chapters in package spine order within an expanded-byte budget.
///
/// Returns readable Markdown-style chapter headings and text for search and preview.
/// Returns an error for malformed containers, missing chapters, encrypted chapter content,
/// escaping archive paths, or expanded content above `max_bytes`.
pub fn read_epub(path: &Path, max_bytes: usize) -> Result<String> {
    admission::with_legacy(Some(max_bytes as u64), || read_epub_body(path, max_bytes))
}

/// Read one EPUB after the host binds the shared account.
pub fn read_epub_admitted(
    path: &Path,
    max_bytes: usize,
    account: &mut impl admission::SharedAccount,
) -> Result<String> {
    admission::with_host(account, admission::Bound::default(), || {
        read_epub_body(path, max_bytes)
    })
    .map_err(anyhow::Error::from)?
}

fn read_epub_body(path: &Path, max_bytes: usize) -> Result<String> {
    admission::checkpoint()?;
    if max_bytes == 0 {
        bail!("The EPUB expanded-byte limit must be positive");
    }
    let length = fs::metadata(path)?.len();
    if length > max_bytes as u64 {
        bail!("EPUB source exceeds the byte limit");
    }
    admission::before(Charge::Input { bytes: length })?;
    let source = File::open(path)?;
    let mut archive = zip::ZipArchive::new(source)?;
    if archive.len() > 100_000 {
        bail!("EPUB contains too many archive members");
    }
    let mut remaining = max_bytes;
    let container = read_zip_text(&mut archive, "META-INF/container.xml", &mut remaining)?;
    let document = roxmltree::Document::parse(&container)?;
    let package = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "rootfile")
        .and_then(|node| node.attribute("full-path"))
        .context("EPUB has no package rootfile")?;
    let package = archive_path("", package)?;
    let package_xml = read_zip_text(&mut archive, &package, &mut remaining)?;
    let package_document = roxmltree::Document::parse(&package_xml)?;
    let directory = package.rsplit_once('/').map_or("", |(parent, _)| parent);
    let manifest: HashMap<&str, (&str, &str)> = package_document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
        .filter_map(|node| {
            Some((
                node.attribute("id")?,
                (
                    node.attribute("href")?,
                    node.attribute("media-type").unwrap_or(""),
                ),
            ))
        })
        .collect();
    let encrypted = encrypted_paths(&mut archive, &mut remaining)?;
    let title = package_document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "title")
        .and_then(|node| node.text())
        .unwrap_or("EPUB");
    let mut text = format!("# {title}\n\n");
    let mut chapter_count = 0;
    for reference in package_document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "itemref")
    {
        if reference.attribute("linear") == Some("no") {
            continue;
        }
        let id = reference
            .attribute("idref")
            .context("EPUB spine item has no idref")?;
        let (href, mime) = manifest
            .get(id)
            .with_context(|| format!("EPUB spine references missing manifest item {id}"))?;
        if !matches!(*mime, "application/xhtml+xml" | "text/html" | "") {
            continue;
        }
        let member = archive_path(directory, href)?;
        if encrypted.contains(&member) {
            bail!("EPUB chapter is encrypted: {member}");
        }
        let html = read_zip_text(&mut archive, &member, &mut remaining)?;
        let chapter_text = html2text::from_read(html.as_bytes(), 120)?;
        text.push_str(&format!("## {member}\n\n{chapter_text}\n\n"));
        chapter_count += 1;
        if text.len() > max_bytes {
            bail!("EPUB extracted text exceeds the byte limit");
        }
    }
    if chapter_count == 0 {
        bail!("EPUB has no supported readable spine chapters");
    }
    Ok(text)
}

/// Export Markdown and HTML under `root` to a new EPUB without changing any sources.
///
/// Excludes `.omni`, `.git`, symlink entries, and nested knowledge-library roots.
/// Optional media includes only referenced files contained in the same library.
/// Creates the output atomically without overwriting an existing path and returns chapter count.
/// Returns an error for unreadable chapters, invalid XML, size limits, or output collisions.
pub fn export_epub(root: &Path, output: &Path, title: &str, include_media: bool) -> Result<usize> {
    admission::with_legacy(Some(512 * 1024 * 1024), || {
        export_epub_body(root, output, title, include_media)
    })
}

/// Export one EPUB after the host binds the shared account.
pub fn export_epub_admitted(
    root: &Path,
    output: &Path,
    title: &str,
    include_media: bool,
    account: &mut impl admission::SharedAccount,
) -> Result<usize> {
    admission::with_host(account, admission::Bound::default(), || {
        export_epub_body(root, output, title, include_media)
    })
    .map_err(anyhow::Error::from)?
}

fn export_epub_body(root: &Path, output: &Path, title: &str, include_media: bool) -> Result<usize> {
    admission::checkpoint()?;
    if title.trim().is_empty() {
        bail!("EPUB title must not be empty");
    }
    if output.exists() {
        bail!("EPUB output already exists: {}", output.display());
    }
    let root = fs::canonicalize(root).context("Resolve knowledge-library root")?;
    if !root.is_dir() {
        bail!("EPUB export requires a directory");
    }
    let mut sources = Vec::new();
    let entries = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            if matches!(name.as_ref(), ".omni" | ".git") {
                return false;
            }
            !(entry.file_type().is_dir() && entry.path().join("omni-knowledge.json").is_file())
        });
    for entry in entries {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            extension.as_str(),
            "md" | "markdown" | "mdown" | "html" | "htm" | "xhtml"
        ) {
            sources.push(entry.into_path());
        }
    }
    sources.sort();
    if sources.is_empty() {
        bail!("The library contains no Markdown or HTML chapters");
    }
    let mut chapters = Vec::new();
    let mut total_bytes = 0usize;
    for (index, source) in sources.into_iter().enumerate() {
        let source_text = read_bounded_file(&source, 64 * 1024 * 1024)?;
        total_bytes = total_bytes
            .checked_add(source_text.len())
            .context("EPUB source size overflow")?;
        if total_bytes > 512 * 1024 * 1024 {
            bail!("EPUB chapter content exceeds the 512 MiB export limit");
        }
        let source_text = String::from_utf8(source_text)
            .with_context(|| format!("Chapter is not UTF-8: {}", source.display()))?;
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let (metadata, _) = markdown_frontmatter(&source_text);
        let chapter_title = metadata
            .get("title")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                source
                    .strip_prefix(&root)
                    .unwrap_or(&source)
                    .to_string_lossy()
                    .into_owned()
            });
        let html = if matches!(extension.as_str(), "html" | "htm" | "xhtml") {
            sanitize_html(&source_text)
        } else {
            render_markdown(&source_text)
        };
        chapters.push(Chapter {
            source,
            title: chapter_title,
            href: format!("chapter{:05}.xhtml", index + 1),
            html,
        });
    }
    let chapter_paths: BTreeMap<PathBuf, String> = chapters
        .iter()
        .map(|chapter| (chapter.source.clone(), chapter.href.clone()))
        .collect();
    let mut media = Vec::new();
    let mut media_paths = HashMap::new();
    let mut bodies = Vec::new();
    for chapter in &chapters {
        let dom = html2text::config::plain().parse_html(chapter.html.as_bytes())?;
        let mut body = String::new();
        write_xhtml(
            &dom.document,
            &mut body,
            &root,
            chapter,
            &chapter_paths,
            include_media,
            &mut media,
            &mut media_paths,
            0,
        )?;
        let xhtml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"und\" xml:lang=\"und\"><head><title>{}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head><body>{body}</body></html>",
            escape_html(&chapter.title)
        );
        roxmltree::Document::parse(&xhtml)
            .with_context(|| format!("Validate EPUB chapter {}", chapter.href))?;
        bodies.push(xhtml);
    }
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    admission::checkpoint()?;
    admission::before(Charge::Storage {
        bytes: u64::try_from(total_bytes).unwrap_or(u64::MAX).max(1),
    })?;
    let temporary = tempfile::NamedTempFile::new_in(parent).context("Create EPUB staging file")?;
    {
        let mut zip = zip::ZipWriter::new(temporary.as_file());
        write_member(
            &mut zip,
            "mimetype",
            b"application/epub+zip",
            zip::CompressionMethod::Stored,
        )?;
        write_member(&mut zip, "META-INF/container.xml", b"<?xml version=\"1.0\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>", zip::CompressionMethod::Deflated)?;
        write_member(&mut zip, "OEBPS/style.css", b"body{font-family:serif;line-height:1.5}pre{white-space:pre-wrap}img,video{max-width:100%}table{border-collapse:collapse}td,th{border:1px solid #aaa;padding:.3em}", zip::CompressionMethod::Deflated)?;
        write_member(
            &mut zip,
            "OEBPS/nav.xhtml",
            navigation(&chapters, title).as_bytes(),
            zip::CompressionMethod::Deflated,
        )?;
        write_member(
            &mut zip,
            "OEBPS/content.opf",
            package_document(&chapters, &media, title).as_bytes(),
            zip::CompressionMethod::Deflated,
        )?;
        for (chapter, body) in chapters.iter().zip(&bodies) {
            write_member(
                &mut zip,
                &format!("OEBPS/{}", chapter.href),
                body.as_bytes(),
                zip::CompressionMethod::Deflated,
            )?;
        }
        for asset in &media {
            let bytes = read_bounded_file(&asset.source, 128 * 1024 * 1024)?;
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .context("EPUB media size overflow")?;
            if total_bytes > 512 * 1024 * 1024 {
                bail!("EPUB content and media exceed the 512 MiB export limit");
            }
            write_member(
                &mut zip,
                &format!("OEBPS/{}", asset.href),
                &bytes,
                zip::CompressionMethod::Deflated,
            )?;
        }
        zip.finish()?;
    }
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(output)
        .map_err(|error| error.error)
        .with_context(|| format!("Publish new EPUB {}", output.display()))?;
    Ok(chapters.len())
}

/// Read a bounded source snapshot without trusting a prior file-size observation.
fn read_bounded_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    admission::checkpoint()?;
    let observed = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let ceiling = u64::try_from(limit).unwrap_or(u64::MAX);
    admission::before(Charge::Input {
        bytes: observed.min(ceiling),
    })?;
    let file = File::open(path).with_context(|| format!("Read {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        bail!(
            "File exceeds the {limit}-byte export limit: {}",
            path.display()
        );
    }
    Ok(bytes)
}

/// Resolve a percent-encoded package href to a contained ZIP member name.
fn archive_path(directory: &str, href: &str) -> Result<String> {
    let path = href.split(['#', '?']).next().unwrap_or("");
    let path = decode_percent(path)?;
    if path.starts_with('/') || path.contains(':') || path.contains('\\') || path.contains('\0') {
        bail!("EPUB resource path is not local: {href}");
    }
    let mut parts: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    bail!("EPUB resource escapes its archive: {href}");
                }
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        bail!("EPUB resource path is empty");
    }
    Ok(parts.join("/"))
}

/// Decode URL path bytes without accepting invalid escapes or invalid UTF-8.
fn decode_percent(value: &str) -> Result<String> {
    let mut bytes = Vec::new();
    let mut source = value.bytes();
    while let Some(byte) = source.next() {
        if byte == b'%' {
            let high = source
                .next()
                .and_then(|value| (value as char).to_digit(16))
                .context("Invalid percent escape")?;
            let low = source
                .next()
                .and_then(|value| (value as char).to_digit(16))
                .context("Invalid percent escape")?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).context("Resource path is not UTF-8")
}

/// Identify protected content while allowing EPUB's standard font obfuscation schemes.
fn encrypted_paths(
    archive: &mut zip::ZipArchive<File>,
    remaining: &mut usize,
) -> Result<Vec<String>> {
    if !archive
        .file_names()
        .any(|name| name == "META-INF/encryption.xml")
    {
        return Ok(Vec::new());
    }
    let xml = read_zip_text(archive, "META-INF/encryption.xml", remaining)?;
    let document = roxmltree::Document::parse(&xml)?;
    let mut encrypted = Vec::new();
    for data in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "EncryptedData")
    {
        let algorithm = data
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == "EncryptionMethod")
            .and_then(|node| node.attribute("Algorithm"))
            .unwrap_or("");
        if matches!(
            algorithm,
            "http://www.idpf.org/2008/embedding" | "http://ns.adobe.com/pdf/enc#RC"
        ) {
            continue;
        }
        for reference in data
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "CipherReference")
        {
            if let Some(uri) = reference.attribute("URI") {
                encrypted.push(archive_path("", uri)?);
            }
        }
    }
    Ok(encrypted)
}

/// Resolve a relative resource without crossing the library root or a nested library.
fn local_resource(root: &Path, chapter: &Chapter, value: &str) -> Option<PathBuf> {
    if !is_relative_resource(value) || value.starts_with('#') {
        return None;
    }
    let value = value.split(['#', '?']).next()?;
    let decoded = decode_percent(value).ok()?;
    let mut candidate = chapter.source.parent()?.to_path_buf();
    for component in Path::new(&decoded).components() {
        match component {
            std::path::Component::Normal(name) => {
                if matches!(name.to_str(), Some(".omni" | ".git")) {
                    return None;
                }
                candidate.push(name);
                if fs::symlink_metadata(&candidate)
                    .ok()?
                    .file_type()
                    .is_symlink()
                {
                    return None;
                }
            }
            std::path::Component::ParentDir => {
                candidate.pop();
            }
            std::path::Component::CurDir => {}
            _ => return None,
        }
        if !candidate.starts_with(root) {
            return None;
        }
    }
    let path = fs::canonicalize(candidate).ok()?;
    if !path.starts_with(root) || !path.is_file() {
        return None;
    }
    if path
        .strip_prefix(root)
        .ok()?
        .components()
        .any(|component| matches!(component.as_os_str().to_str(), Some(".omni" | ".git")))
    {
        return None;
    }
    for ancestor in path
        .parent()?
        .ancestors()
        .take_while(|ancestor| *ancestor != root)
    {
        if ancestor.join("omni-knowledge.json").is_file() {
            return None;
        }
    }
    Some(path)
}

/// Serialize the sanitized HTML DOM as XML, rewriting contained chapter and media references.
#[allow(clippy::too_many_arguments)]
fn write_xhtml(
    node: &html2text::Handle,
    output: &mut String,
    root: &Path,
    chapter: &Chapter,
    chapter_paths: &BTreeMap<PathBuf, String>,
    include_media: bool,
    media: &mut Vec<Media>,
    media_paths: &mut HashMap<PathBuf, String>,
    depth: usize,
) -> Result<()> {
    if depth > 512 {
        bail!("HTML nesting exceeds the EPUB serialization limit");
    }
    match &node.data {
        html2text::Document => {
            for child in node.children.borrow().iter() {
                write_xhtml(
                    child,
                    output,
                    root,
                    chapter,
                    chapter_paths,
                    include_media,
                    media,
                    media_paths,
                    depth + 1,
                )?;
            }
        }
        html2text::Element { name, attrs, .. } => {
            let tag = name.local.as_ref();
            if tag == "head" {
                return Ok(());
            }
            let wrapper = matches!(tag, "html" | "body");
            if !wrapper {
                output.push('<');
                output.push_str(tag);
                for attribute in attrs.borrow().iter() {
                    let key = attribute.name.local.as_ref();
                    let mut value = attribute.value.to_string();
                    if matches!(key, "src" | "poster") {
                        let Some(source) =
                            local_resource(root, chapter, &value).filter(|_| include_media)
                        else {
                            continue;
                        };
                        if let Some(href) = media_paths.get(&source) {
                            value = href.clone();
                        } else {
                            let mime = mime_guess::from_path(&source)
                                .first_or_octet_stream()
                                .to_string();
                            if !(mime.starts_with("image/")
                                || mime.starts_with("audio/")
                                || mime.starts_with("video/"))
                            {
                                continue;
                            }
                            let extension = source
                                .extension()
                                .and_then(|value| value.to_str())
                                .filter(|value| {
                                    value
                                        .chars()
                                        .all(|character| character.is_ascii_alphanumeric())
                                })
                                .unwrap_or("bin");
                            value = format!("media/asset{:05}.{extension}", media.len() + 1);
                            media_paths.insert(source.clone(), value.clone());
                            media.push(Media {
                                source,
                                href: value.clone(),
                                mime,
                            });
                        }
                    } else if key == "href"
                        && is_relative_resource(&value)
                        && !value.starts_with('#')
                    {
                        let fragment = value
                            .split_once('#')
                            .map(|(_, fragment)| format!("#{fragment}"))
                            .unwrap_or_default();
                        value = local_resource(root, chapter, &value)
                            .and_then(|source| chapter_paths.get(&source).cloned())
                            .map(|href| format!("{href}{fragment}"))
                            .unwrap_or_else(|| "#".into());
                    }
                    output.push(' ');
                    output.push_str(key);
                    output.push_str("=\"");
                    output.push_str(&escape_html(&value));
                    output.push('"');
                }
                output.push('>');
            }
            for child in node.children.borrow().iter() {
                write_xhtml(
                    child,
                    output,
                    root,
                    chapter,
                    chapter_paths,
                    include_media,
                    media,
                    media_paths,
                    depth + 1,
                )?;
            }
            if !wrapper {
                output.push_str("</");
                output.push_str(tag);
                output.push('>');
            }
        }
        html2text::Comment { .. } => {}
        _ => {
            let mut bytes = Vec::new();
            node.serialize(&mut bytes)?;
            let serialized = String::from_utf8(bytes)?;
            if !serialized.starts_with("<!") && !serialized.starts_with("<?") {
                output.extend(
                    serialized
                        .replace("&nbsp;", "&#160;")
                        .chars()
                        .filter(|character| valid_xml_character(*character)),
                );
            }
        }
    }
    Ok(())
}

/// Append one archive member with explicit compression and no filesystem extraction.
fn write_member(
    zip: &mut zip::ZipWriter<&File>,
    name: &str,
    bytes: &[u8],
    method: zip::CompressionMethod,
) -> Result<()> {
    zip.start_file(
        name,
        SimpleFileOptions::default().compression_method(method),
    )?;
    zip.write_all(bytes)?;
    Ok(())
}

/// Build an EPUB navigation document with multilingual escaped chapter titles.
fn navigation(chapters: &[Chapter], title: &str) -> String {
    let links: String = chapters
        .iter()
        .map(|chapter| {
            format!(
                "<li><a href=\"{}\">{}</a></li>",
                escape_html(&chapter.href),
                escape_html(&chapter.title)
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"und\" xml:lang=\"und\"><head><title>{}</title></head><body><nav epub:type=\"toc\" id=\"toc\"><h1>{}</h1><ol>{links}</ol></nav></body></html>",
        escape_html(title),
        escape_html(title)
    )
}

/// Build publication metadata, manifest, and spine using EPUB 3 required fields.
fn package_document(chapters: &[Chapter], media: &[Media], title: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let identifier = format!(
        "urn:omni-knowledge:{}:{}",
        timestamp.as_secs(),
        timestamp.subsec_nanos()
    );
    let modified = utc_timestamp(timestamp.as_secs());
    let chapter_manifest: String = chapters
        .iter()
        .enumerate()
        .map(|(index, chapter)| {
            format!(
                "<item id=\"chapter{}\" href=\"{}\" media-type=\"application/xhtml+xml\"/>",
                index + 1,
                escape_html(&chapter.href)
            )
        })
        .collect();
    let media_manifest: String = media
        .iter()
        .enumerate()
        .map(|(index, asset)| {
            format!(
                "<item id=\"media{}\" href=\"{}\" media-type=\"{}\"/>",
                index + 1,
                escape_html(&asset.href),
                escape_html(&asset.mime)
            )
        })
        .collect();
    let spine: String = (1..=chapters.len())
        .map(|index| format!("<itemref idref=\"chapter{index}\"/>"))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"book-id\">{identifier}</dc:identifier><dc:title>{}</dc:title><dc:language>und</dc:language><meta property=\"dcterms:modified\">{modified}</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"style\" href=\"style.css\" media-type=\"text/css\"/>{chapter_manifest}{media_manifest}</manifest><spine>{spine}</spine></package>",
        escape_html(title)
    )
}

/// Format Unix seconds as UTC without introducing a date-time dependency.
fn utc_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let time = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        time % 3600 / 60,
        time % 60
    )
}
