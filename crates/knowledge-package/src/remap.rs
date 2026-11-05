//! Parsed destination-only rewriting: native path relocation and explicit pinned logical references.
use crate::source::BoundedBytes;
use crate::{Error, RelativePath, Work, gate, preallocate};
use pulldown_cmark::{Event, LinkType, Parser, Tag, TagEnd};
use std::ops::Range;

/// Explicit old-to-new package path selection, never an inferred root or permission grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V1PathMap {
    /// Exact verified original member name.
    pub from: RelativePath,
    /// Confined destination member name; caller-selected native paths may differ from v1 families.
    pub to: RelativePath,
}

/// Explicit canonical logical reference remint; SHA-256 pins cannot change through this map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V1ReferenceMap {
    /// Exact original `source:` or `openkit:page/raw` reference, not a prose search string.
    pub from: String,
    /// Same reference family with the same optional content digest and a new explicit identity.
    pub to: String,
}

/// Resolved path identities used by both preview and actual byte production.
pub(crate) struct PathPair<'a> {
    pub(crate) from: &'a str,
    pub(crate) to: &'a str,
    pub(crate) digest: &'a crate::ContentDigest,
}

/// Validate canonical logical URI families and forbid changing a digest pin or reference family.
pub(crate) fn validate_reference(map: &V1ReferenceMap) -> Result<(), Error> {
    let (family, pin) = reference(&map.from)?;
    let (target, target_pin) = reference(&map.to)?;
    if family != target || pin != target_pin {
        return Err(Error::Invalid);
    }
    Ok(())
}

/// Parse data-only canonical references without resolving scopes or contacting any host.
fn reference(value: &str) -> Result<(&str, Option<&str>), Error> {
    let (bare, pin) = value
        .split_once('@')
        .map_or((value, None), |(a, b)| (a, Some(b)));
    if let Some(pin) = pin {
        crate::ContentDigest::new(pin.strip_prefix("sha256:").ok_or(Error::Invalid)?)
            .map_err(|_| Error::Invalid)?;
    }
    if let Some(id) = bare.strip_prefix("source:") {
        if !crate::foreign::entity(id, "ks", false) {
            return Err(Error::Invalid);
        }
        return Ok(("source", pin));
    }
    let rest = bare.strip_prefix("openkit:").ok_or(Error::Invalid)?;
    let mut parts = rest.split('/');
    let family = parts.next().ok_or(Error::Invalid)?;
    if !matches!(family, "raw" | "page") {
        return Err(Error::Invalid);
    }
    match parts.next() {
        Some("server") => {}
        Some("workspace") => {
            if !parts
                .next()
                .is_some_and(|id| crate::foreign::entity(id, "ws", true))
            {
                return Err(Error::Invalid);
            }
        }
        Some("user") => {
            if !parts
                .next()
                .is_some_and(|id| crate::foreign::entity(id, "usr", false))
            {
                return Err(Error::Invalid);
            }
        }
        _ => return Err(Error::Invalid),
    }
    if !parts
        .next()
        .is_some_and(|id| crate::foreign::entity(id, "kb", false))
    {
        return Err(Error::Invalid);
    }
    let scope = if rest.split('/').nth(1) == Some("server") {
        3
    } else {
        4
    };
    let object = rest
        .splitn(scope + 1, '/')
        .nth(scope)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid)?;
    let decoded = decode(object)?;
    crate::manifest::path(&decoded)?;
    if encode(&decoded) != object {
        return Err(Error::Invalid);
    }
    Ok((family, pin))
}

/// One parser-confirmed destination replacement; ranges refer to original UTF-8 bytes.
struct Edit {
    range: Range<usize>,
    target: String,
}

/// Rewrite only parser-confirmed inline/image/reference-definition destinations, never prose/code/HTML.
/// Front matter remains authored/inert; source identity and asset maps are separately retained facts.
pub(crate) fn rewrite(
    text: &[u8],
    source: &str,
    target: &str,
    paths: &[PathPair<'_>],
    refs: &[V1ReferenceMap],
    managed_mount: Option<&str>,
    work: &mut Work<'_>,
) -> Result<BoundedBytes, Error> {
    let original = std::str::from_utf8(text).map_err(|_| Error::Invalid)?;
    let largest = paths
        .iter()
        .map(|p| p.from.len() + p.to.len())
        .chain(refs.iter().map(|r| r.from.len() + r.to.len()))
        .max()
        .unwrap_or(0);
    // Retained replacements are bounded by the admitted final entry plus removed original bytes.
    // Runtime checks below enforce that bound while collecting, before a replacement can be retained.
    let targets = (largest as u64)
        .checked_mul(8)
        .and_then(|n| n.checked_mul(text.len() as u64))
        .and_then(|n| n.checked_add(text.len() as u64))
        .ok_or(Error::TooLarge)?
        .min(
            work.bounds
                .quotas
                .max_entry_expanded
                .saturating_add(text.len() as u64),
        );
    let _parse = preallocate(
        work,
        (text.len() as u64)
            .checked_mul(128)
            .and_then(|n| n.checked_add(largest as u64 * 16 + 8192))
            .and_then(|n| n.checked_add(targets))
            .ok_or(Error::TooLarge)?,
    )?;
    let body = body_start(original)?;
    let content = &original[body..];
    let parser = Parser::new(content);
    let mut length = 0u64;
    let mut edits: Vec<Edit> = facts(
        original,
        source,
        target,
        paths,
        refs,
        managed_mount,
        &mut length,
        work,
    )?;
    for (_, definition) in parser.reference_definitions().iter() {
        gate(work)?;
        let span = definition.span.clone();
        let raw = &content[span.clone()];
        let close = label_end(raw.as_bytes(), raw.find('[').ok_or(Error::Invalid)?)?;
        let mut at = close + 1;
        if raw.as_bytes().get(at) != Some(&b':') {
            return Err(Error::Invalid);
        }
        at += 1;
        while raw.as_bytes().get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        let dest = destination(raw, at)?;
        retain(
            &mut edits,
            original,
            body + span.start + dest.start..body + span.start + dest.end,
            &definition.dest,
            &mut length,
            source,
            target,
            paths,
            refs,
            work,
        )?;
    }
    let mut image_depth = 0usize;
    for (event, span) in parser.into_offset_iter() {
        gate(work)?;
        let (kind, destination_url) = match event {
            Event::End(TagEnd::Image) => {
                image_depth = image_depth.checked_sub(1).ok_or(Error::Invalid)?;
                continue;
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                ..
            }) => {
                image_depth += 1;
                if image_depth != 1 {
                    continue;
                }
                (link_type, dest_url)
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) if image_depth == 0 => (link_type, dest_url),
            _ => continue,
        };
        if kind != LinkType::Inline {
            continue;
        }
        let raw = &content[span.clone()];
        let start = usize::from(raw.starts_with('!'));
        if raw.as_bytes().get(start) != Some(&b'[') {
            return Err(Error::Invalid);
        }
        let close = label_end(raw.as_bytes(), start)?;
        if raw.as_bytes().get(close + 1) != Some(&b'(') {
            return Err(Error::Invalid);
        }
        let mut at = close + 2;
        while raw.as_bytes().get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        let dest = destination(raw, at)?;
        retain(
            &mut edits,
            original,
            body + span.start + dest.start..body + span.start + dest.end,
            &destination_url,
            &mut length,
            source,
            target,
            paths,
            refs,
            work,
        )?;
    }
    edits.sort_by_key(|e| e.range.start);
    edits.dedup_by(|a, b| a.range == b.range && a.target == b.target);
    for pair in edits.windows(2) {
        if pair[0].range.end > pair[1].range.start {
            return Err(Error::Invalid);
        }
    }
    // Confirm the post-sort exact total; only parser-confirmed disjoint spans are allowed.
    let length = edits.iter().try_fold(text.len() as u64, |n, edit| {
        n.checked_sub((edit.range.end - edit.range.start) as u64)
            .and_then(|n| n.checked_add(edit.target.len() as u64))
            .ok_or(Error::TooLarge)
    })?;
    if length > work.bounds.quotas.max_entry_expanded {
        return Err(Error::Quota);
    }
    let mut output = BoundedBytes::reserve(length, work)?;
    let mut at = 0;
    for edit in edits {
        output.bytes.extend_from_slice(&text[at..edit.range.start]);
        output.bytes.extend_from_slice(edit.target.as_bytes());
        at = edit.range.end;
    }
    output.bytes.extend_from_slice(&text[at..]);
    Ok(output)
}

/// Retain only a real changed destination, in the original byte coordinate space.
#[allow(clippy::too_many_arguments)]
fn retain(
    edits: &mut Vec<Edit>,
    content: &str,
    range: Range<usize>,
    href: &str,
    length: &mut u64,
    source: &str,
    target: &str,
    paths: &[PathPair<'_>],
    refs: &[V1ReferenceMap],
    work: &mut Work<'_>,
) -> Result<(), Error> {
    if let Some(target) = relocate(href, source, target, paths, refs)? {
        let raw = content.get(range.clone()).ok_or(Error::Invalid)?;
        let mut confirmed = false;
        for spelling in [format!("[x]({raw})"), format!("[x](<{raw}>)")] {
            if Parser::new(&spelling).into_offset_iter().any(|(event,span)|matches!(event,Event::Start(Tag::Link{dest_url,..}) if dest_url.as_ref()==href && span== (0..spelling.len()))) {confirmed=true;break;}
        }
        if !confirmed {
            return Err(Error::Invalid);
        }
        record(edits, length, range, target, work)?;
    }
    Ok(())
}

/// Skip one exact YAML front-matter block rather than parsing data labels as Markdown links.
fn body_start(text: &str) -> Result<usize, Error> {
    let Some(first) = text.lines().next() else {
        return Ok(0);
    };
    if first != "---" {
        return Ok(0);
    }
    let mut at = 0;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        at += line.len();
        if i > 0 && matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
            return Ok(at);
        }
    }
    Err(Error::Invalid)
}

/// Find a parser-confirmed outer label, respecting escaped brackets and opaque code spans.
fn label_end(bytes: &[u8], start: usize) -> Result<usize, Error> {
    let mut depth = 0;
    let mut at = start;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => {
                at += 2;
                continue;
            }
            b'`' => {
                let n = bytes[at..].iter().take_while(|b| **b == b'`').count();
                let mut next = at + n;
                let mut found = None;
                while next < bytes.len() {
                    if bytes[next] == b'`' {
                        let run = bytes[next..].iter().take_while(|b| **b == b'`').count();
                        if run == n {
                            found = Some(next + run);
                            break;
                        }
                        next += run;
                    } else {
                        next += 1;
                    }
                }
                if let Some(end) = found {
                    at = end;
                    continue;
                }
                at += n;
                continue;
            }
            b'<' => {
                let mut next = at + 1;
                let mut quote = None;
                let mut found = None;
                while next < bytes.len() {
                    let c = bytes[next];
                    if let Some(q) = quote {
                        if c == q {
                            quote = None;
                        }
                    } else if matches!(c, b'\'' | b'\"') {
                        quote = Some(c);
                    } else if c == b'>' {
                        found = Some(next + 1);
                        break;
                    }
                    next += 1;
                }
                if let Some(end) = found {
                    at = end;
                    continue;
                }
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(at);
                }
            }
            _ => {}
        }
        at += 1;
    }
    Err(Error::Invalid)
}

/// Locate URL bytes without including angle delimiters, title, whitespace, or surrounding syntax.
fn destination(text: &str, start: usize) -> Result<Range<usize>, Error> {
    let b = text.as_bytes();
    let angled = b.get(start) == Some(&b'<');
    let first = start + usize::from(angled);
    let mut at = first;
    let mut depth = 0;
    while let Some(&c) = b.get(at) {
        if c == b'\\' {
            at += 2;
            continue;
        }
        if angled {
            if c == b'>' {
                return Ok(first..at);
            }
            if matches!(c, b'\r' | b'\n' | b'<') {
                return Err(Error::Invalid);
            }
        } else {
            match c {
                b'(' => depth += 1,
                b')' if depth == 0 => return Ok(first..at),
                b')' => depth -= 1,
                c if c.is_ascii_whitespace() && depth == 0 => return Ok(first..at),
                _ => {}
            }
        }
        at += 1;
    }
    if !angled && depth == 0 {
        Ok(first..at)
    } else {
        Err(Error::Invalid)
    }
}

/// Rebase confined local destinations, retaining missing/orphan locations without resolving them.
fn relocate(
    href: &str,
    source: &str,
    target: &str,
    paths: &[PathPair<'_>],
    refs: &[V1ReferenceMap],
) -> Result<Option<String>, Error> {
    if let Some(mapping) = refs.iter().find(|r| r.from == href) {
        return Ok((mapping.to != href).then(|| mapping.to.clone()));
    }
    if href.is_empty()
        || href.starts_with(['#', '/', '?'])
        || href
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|p| p.contains(':'))
    {
        return Ok(None);
    }
    let end = href.find(['?', '#']).unwrap_or(href.len());
    let path = decode(&href[..end])?;
    let mut old: Vec<&str> = source.split('/').collect();
    old.pop();
    for component in path.split('/') {
        match component {
            "." | "" => {}
            ".." => {
                if old.pop().is_none() {
                    return Ok(None);
                }
            }
            c => old.push(c),
        }
    }
    let old = old.join("/");
    let mapped = paths
        .iter()
        .find(|p| p.from == old)
        .map_or(old.as_str(), |p| p.to);
    if mapped == old && source == target {
        return Ok(None);
    }
    let mut base: Vec<&str> = target.split('/').collect();
    base.pop();
    let to: Vec<&str> = mapped.split('/').collect();
    let common = base.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut result = String::new();
    for _ in common..base.len() {
        result.push_str("../");
    }
    result.push_str(&encode(&to[common..].join("/")));
    if path.ends_with('/') && !result.ends_with('/') {
        result.push('/');
    }
    result.push_str(&href[end..]);
    Ok((result != href).then_some(result))
}

/// Decode exactly once without admitting encoded separators or non-UTF-8 aliases.
fn decode(value: &str) -> Result<String, Error> {
    let mut result = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let raw = bytes.get(at + 1..at + 3).ok_or(Error::Invalid)?;
            let n = u8::from_str_radix(std::str::from_utf8(raw).map_err(|_| Error::Invalid)?, 16)
                .map_err(|_| Error::Invalid)?;
            if matches!(n, 0 | b'/' | b'\\') {
                return Err(Error::Hostile);
            }
            result.push(n);
            at += 3;
        } else {
            result.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(result).map_err(|_| Error::Invalid)
}

/// Rewrite selected parsed Source locator/refs scalars without reserializing unrelated YAML.
#[allow(clippy::too_many_arguments)]
fn facts(
    text: &str,
    source: &str,
    target: &str,
    paths: &[PathPair<'_>],
    refs: &[V1ReferenceMap],
    managed_mount: Option<&str>,
    length: &mut u64,
    work: &mut Work<'_>,
) -> Result<Vec<Edit>, Error> {
    if !text.starts_with("---\n") && !text.starts_with("---\r\n") {
        return Ok(Vec::new());
    }
    let mut at = 0;
    let mut first = 0;
    let mut end = None;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if i == 0 {
            first = line.len();
        } else if matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
            end = Some(at);
            break;
        }
        at += line.len();
    }
    let yaml = &text[first..end.ok_or(Error::Invalid)?];
    let source_mapping = (source.starts_with("sources/") || target.starts_with("sources/"))
        && paths
            .iter()
            .any(|p| p.from.starts_with("raw/") && p.from != p.to);
    if !source_mapping && refs.is_empty() {
        return Ok(Vec::new());
    }
    let _scratch = preallocate(work, yaml.len() as u64 * 1024 + 8192)?;
    // Alias expansion is not byte-linear. Never guess quote/comment state to admit an anchor.
    // Permit embedded plain URI/path punctuation, but refuse token-leading alias-like text even
    // inside an authored quote/comment; unsupported remapping is safer than alias amplification.
    let bytes = yaml.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        if i % 1024 == 0 {
            gate(work)?;
        }
        if matches!(byte, b'&' | b'*')
            && (i == 0
                || !(bytes[i - 1].is_ascii_alphanumeric()
                    || matches!(bytes[i - 1], b'/' | b'.' | b'_' | b'=' | b'%')))
        {
            return Err(Error::Unsupported);
        }
    }
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).map_err(|_| Error::Invalid)?;
    let mut edits = Vec::new();
    if source_mapping {
        let locator = &value["contentRef"]["locator"];
        if let Some(path) = locator["path"].as_str() {
            let old = format!("raw/{path}");
            if let Some(pair) = paths.iter().find(|p| p.from == old && p.from != p.to) {
                let mount = managed_mount.ok_or(Error::CompatibilityRefused)?;
                if locator["mount"].as_str() == Some(mount) {
                    let replacement = pair.to.strip_prefix("raw/").ok_or(Error::Invalid)?;
                    if let Some(pin) = value["contentRef"]["contentDigest"].as_str()
                        && pin.strip_prefix("sha256:") != Some(pair.digest.as_str())
                    {
                        return Err(Error::Checksum);
                    }
                    let span = yaml_scalar(yaml, &["contentRef", "locator", "path"])?;
                    yaml_edit(
                        &mut edits,
                        text,
                        first + span.start..first + span.end,
                        path,
                        replacement,
                        length,
                        work,
                    )?;
                }
            }
        }
    }
    for key in ["refs", "references"] {
        if let Some(values) = value[key].as_sequence() {
            for (i, value) in values.iter().enumerate() {
                gate(work)?;
                if let Some(original) = value.as_str()
                    && let Some(mapping) =
                        refs.iter().find(|m| m.from == original && m.from != m.to)
                {
                    let span = yaml_sequence(yaml, key, i)?;
                    yaml_edit(
                        &mut edits,
                        text,
                        first + span.start..first + span.end,
                        original,
                        &mapping.to,
                        length,
                        work,
                    )?;
                }
            }
        }
    }
    Ok(edits)
}

/// Find only a block-mapping scalar at the requested parsed key path; never search by value text.
fn yaml_scalar(yaml: &str, path: &[&str]) -> Result<Range<usize>, Error> {
    let mut parents: Vec<(usize, &str)> = Vec::new();
    let mut offset = 0;
    for line in yaml.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let indent = body.len() - body.trim_start_matches(' ').len();
        let body = &body[indent..];
        if body.is_empty() || body.starts_with('#') {
            offset += line.len();
            continue;
        }
        while parents.last().is_some_and(|p| p.0 >= indent) {
            parents.pop();
        }
        let Some((key, rest)) = body.split_once(':') else {
            offset += line.len();
            continue;
        };
        let key = key.trim();
        parents.push((indent, key));
        if parents.len() == path.len() && parents.iter().map(|p| p.1).eq(path.iter().copied()) {
            let start =
                offset + indent + key.len() + 1 + (rest.len() - rest.trim_start_matches(' ').len());
            return Ok(start
                ..scalar_end(
                    yaml,
                    start,
                    offset + line.trim_end_matches(['\r', '\n']).len(),
                )?);
        }
        offset += line.len();
    }
    Err(Error::Unsupported)
}

/// Locate a block-sequence scalar by its parsed array index, preserving labels and comments.
fn yaml_sequence(yaml: &str, key: &str, index: usize) -> Result<Range<usize>, Error> {
    let mut offset = 0;
    let mut active = false;
    let mut found = 0;
    for line in yaml.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let trimmed = body.trim_start_matches(' ');
        if !active {
            if body.strip_prefix(key).is_some_and(|r| r.starts_with(':')) {
                active = true;
            }
            offset += line.len();
            continue;
        }
        if trimmed.starts_with("- ") || trimmed == "-" {
            if found == index {
                let start = offset
                    + (body.len() - trimmed.len())
                    + 1
                    + (trimmed[1..].len() - trimmed[1..].trim_start_matches(' ').len());
                return Ok(start..scalar_end(yaml, start, offset + body.len())?);
            }
            found += 1;
        } else if !body.is_empty() && !body.starts_with(' ') && !body.starts_with('#') {
            break;
        }
        offset += line.len();
    }
    Err(Error::Unsupported)
}

/// Exclude a scalar's own trailing comment without treating a quoted hash as comment syntax.
fn scalar_end(text: &str, start: usize, end: usize) -> Result<usize, Error> {
    let b = text.as_bytes();
    let mut quote = None;
    let mut at = start;
    while at < end {
        let c = b[at];
        if let Some(q) = quote {
            if q == b'\"' && c == b'\\' {
                at += 2;
                continue;
            }
            if c == q {
                if q == b'\'' && b.get(at + 1) == Some(&q) {
                    at += 2;
                    continue;
                }
                quote = None;
            }
        } else if matches!(c, b'\'' | b'\"') {
            quote = Some(c);
        } else if c == b'#' && (at == start || b[at - 1].is_ascii_whitespace()) {
            return Ok(text[..at].trim_end_matches(' ').len());
        }
        at += 1;
    }
    if quote.is_some() {
        return Err(Error::Unsupported);
    }
    Ok(text[..end].trim_end_matches(' ').len())
}

/// Confirm the exact source scalar's parsed value before replacing only that scalar, with safe quoting.
fn yaml_edit(
    edits: &mut Vec<Edit>,
    text: &str,
    span: Range<usize>,
    expected: &str,
    replacement: &str,
    length: &mut u64,
    work: &Work<'_>,
) -> Result<(), Error> {
    let source = text.get(span.clone()).ok_or(Error::Invalid)?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(source).map_err(|_| Error::Invalid)?;
    if parsed.as_str() != Some(expected) {
        return Err(Error::Unsupported);
    }
    let target = yaml_string_scalar(source, replacement)?;
    record(edits, length, span, target, work)
}

/// Emit one YAML scalar whose parse is exactly `replacement`, using the source quote style when it round-trips.
/// A plain token is kept only when a mapping parse already yields that string. Otherwise the scalar is
/// `serde_json` double-quoted text, which YAML reads as a string. The parser decides; there is no reserved-word list.
fn yaml_string_scalar(source: &str, replacement: &str) -> Result<String, Error> {
    let preferred = if source.starts_with('\'') {
        format!("'{}'", replacement.replace('\'', "''"))
    } else if source.starts_with('"') {
        serde_json::to_string(replacement).map_err(|_| Error::Invalid)?
    } else {
        replacement.to_owned()
    };
    if yaml_scalar_is_string(&preferred, replacement) {
        return Ok(preferred);
    }
    let quoted = serde_json::to_string(replacement).map_err(|_| Error::Invalid)?;
    if yaml_scalar_is_string(&quoted, replacement) {
        return Ok(quoted);
    }
    Err(Error::Invalid)
}

/// Parse `scalar` as the value of one mapping entry and require the YAML string `expected`.
fn yaml_scalar_is_string(scalar: &str, expected: &str) -> bool {
    if scalar.is_empty() || scalar.contains(['\n', '\r']) {
        return false;
    }
    let document = format!("v: {scalar}\n");
    serde_yaml::from_str::<serde_yaml::Value>(&document)
        .ok()
        .is_some_and(|value| value["v"].as_str() == Some(expected))
}

/// Admit accumulated replacement bytes before retaining a target or growing its table.
/// Do not reject a valid final entry merely because a later replacement removes more input.
fn record(
    edits: &mut Vec<Edit>,
    length: &mut u64,
    range: Range<usize>,
    target: String,
    work: &Work<'_>,
) -> Result<(), Error> {
    gate(work)?;
    let next = length
        .checked_add(target.len() as u64)
        .ok_or(Error::TooLarge)?;
    if next > work.bounds.quotas.max_entry_expanded {
        return Err(Error::Quota);
    }
    edits.try_reserve_exact(1).map_err(|_| Error::TooLarge)?;
    edits.push(Edit { range, target });
    *length = next;
    Ok(())
}

/// Canonical uppercase percent encoding for URI path bytes; separators remain structural.
fn encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            output.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(output, "%{byte:02X}");
        }
    }
    output
}

#[cfg(test)]
mod scalar_tests {
    use super::{yaml_scalar_is_string, yaml_string_scalar};

    /// Parser control for remapped scalars: plain non-strings become JSON quotes, and safe names stay plain.
    #[test]
    fn yaml_string_scalar_preserves_string_type() {
        let samples = [
            "null",
            "true",
            "false",
            "1",
            "01",
            "3",
            "2.5",
            "1e2",
            "0x10",
            "yes",
            "n",
            "on",
            "~",
            ".inf",
            "Null",
            "NO",
            "a #b.bin",
            "say\"hi\".bin",
            "kept.bin",
            "",
        ];
        for word in ["null", "true", "false", "1", "2.5"] {
            assert!(
                !yaml_scalar_is_string(word, word),
                "{word} parsed as that string"
            );
        }
        for replacement in samples {
            let scalar = yaml_string_scalar("a.bin", replacement).unwrap();
            assert!(
                yaml_scalar_is_string(&scalar, replacement),
                "{replacement} => {scalar}"
            );
            let plain_ok = yaml_scalar_is_string(replacement, replacement);
            if plain_ok {
                assert_eq!(scalar, replacement, "{replacement}");
            } else {
                assert_ne!(scalar, replacement, "{replacement}");
            }
        }
        let ordinary = yaml_string_scalar("a.bin", "kept.bin").unwrap();
        assert_eq!(ordinary, "kept.bin");
        assert!(yaml_scalar_is_string(&ordinary, "kept.bin"));
        assert_eq!(
            yaml_string_scalar("'a.bin'", "moved.bin").unwrap(),
            "'moved.bin'"
        );
        assert_eq!(
            yaml_string_scalar("'a.bin'", "it's.bin").unwrap(),
            "'it''s.bin'"
        );
        assert!(yaml_scalar_is_string(
            &yaml_string_scalar("'a.bin'", "it's.bin").unwrap(),
            "it's.bin"
        ));
        let escaped = yaml_string_scalar("\"a.bin\"", "say\"hi\".bin").unwrap();
        assert!(yaml_scalar_is_string(&escaped, "say\"hi\".bin"));
        let newline = yaml_string_scalar("a.bin", "line\nname").unwrap();
        assert!(!newline.contains('\n'));
        assert!(yaml_scalar_is_string(&newline, "line\nname"));
    }
}
