//! Owned parser process. The library admits the child before spawning it.
//! This binary does not start an account and does not spawn another worker.

use std::env;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return ExitCode::from(2);
    };
    let result = match command.as_str() {
        "parse" => parse(args.next().as_deref(), args.next().as_deref()),
        "epub-text" => epub_text(args.next().as_deref()),
        "export-epub" => export(args),
        "export-epub-held" => export_held(args.next().as_deref()),
        "verify-held" => verify_held(),
        "transcript" => transcript(),
        "preview" => preview(args),
        _ => Err(anyhow_message("unknown parser worker command")),
    };
    match result {
        Ok(bytes) => {
            if io::stdout().write_all(&bytes).is_err() {
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            let _ = writeln!(io::stderr(), "{message}");
            ExitCode::from(2)
        }
    }
}

fn parse(mode: Option<&str>, max_bytes: Option<&str>) -> Result<Vec<u8>, String> {
    let mode = mode.ok_or_else(|| anyhow_message("parser mode is missing"))?;
    let max_bytes = parse_max(max_bytes)?;
    let bytes = read_stdin()?;
    let extraction = knowledge_processing::extract::parse_loaded(mode, "", &bytes, max_bytes)
        .map_err(|error| format!("{error:#}"))?;
    serde_json::to_vec(&extraction).map_err(|_| anyhow_message("cannot encode extraction"))
}

fn epub_text(max_bytes: Option<&str>) -> Result<Vec<u8>, String> {
    let max_bytes = parse_max(max_bytes)?;
    let bytes = read_stdin()?;
    let text = knowledge_processing::epub::read_epub_bytes(&bytes, max_bytes)
        .map_err(|error| format!("{error:#}"))?;
    Ok(text.into_bytes())
}

fn export(mut args: impl Iterator<Item = String>) -> Result<Vec<u8>, String> {
    let root = args
        .next()
        .ok_or_else(|| anyhow_message("export root is missing"))?;
    let output = args
        .next()
        .ok_or_else(|| anyhow_message("export output is missing"))?;
    let title = args
        .next()
        .ok_or_else(|| anyhow_message("export title is missing"))?;
    let include_media = args.next().as_deref() == Some("1");
    let destination = std::fs::File::options()
        .write(true)
        .open(&output)
        .map_err(|error| format!("{error}"))?;
    let count = knowledge_processing::epub::export_epub_to_file(
        Path::new(&root),
        &destination,
        &title,
        include_media,
    )
    .map_err(|error| format!("{error:#}"))?;
    Ok(count.to_string().into_bytes())
}

/// Decode the dedicated parent's inherited-FD protocol, without source/destination path lookup.
/// Only this owned worker consumes raw descriptors; the safe library API accepts owned Files.
fn export_held(output: Option<&str>) -> Result<Vec<u8>, String> {
    use knowledge_processing::held::ExportWire;
    use std::fs::File;
    use std::os::fd::FromRawFd;

    let output: i32 = output
        .ok_or("Missing output descriptor")?
        .parse()
        .map_err(|_| "Invalid output descriptor")?;
    let wire: ExportWire =
        serde_json::from_slice(&read_stdin()?).map_err(|_| "Invalid descriptor manifest")?;
    let sources = inherited_members(wire.members, Some(output))?;
    // SAFETY: unique inherited descriptor, distinct from all selected inputs and stdio.
    let destination = unsafe { File::from_raw_fd(output) };
    let count = knowledge_processing::held::export_epub_held_worker(
        &sources.lookup,
        &sources.order,
        &destination,
        &wire.title,
        wire.include_media,
    )
    .map_err(|e| format!("{e:#}"))?;
    Ok(count.to_string().into_bytes())
}

/// Verify potentially blocking held-file streams under the parent's continuous claim/RSS/I/O watchdog.
fn verify_held() -> Result<Vec<u8>, String> {
    let members =
        serde_json::from_slice(&read_stdin()?).map_err(|_| "Invalid verification manifest")?;
    let sources = inherited_members(members, None)?;
    let valid = knowledge_processing::held::verify_held_worker(&sources.lookup)
        .map_err(|e| format!("{e:#}"))?;
    Ok(if valid {
        b"verified".to_vec()
    } else {
        b"changed".to_vec()
    })
}

/// Approved member sequence plus name lookup. The map does not define page order.
struct InheritedMembers {
    lookup: std::collections::BTreeMap<
        std::path::PathBuf,
        (std::fs::File, knowledge_core::data::ContentDigest),
    >,
    order: Vec<std::path::PathBuf>,
}

/// Consume only validated unique live descriptors from the dedicated inherited worker protocol.
/// `order` keeps the approved member sequence. `lookup` resolves logical names and rejects duplicates.
fn inherited_members(
    members: Vec<knowledge_processing::held::WireMember>,
    output: Option<i32>,
) -> Result<InheritedMembers, String> {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs::File;
    use std::os::fd::FromRawFd;
    use std::path::PathBuf;
    let mut fds = BTreeSet::new();
    fds.extend(output);
    if output.is_some_and(|fd| fd <= 2) || members.iter().any(|m| m.fd <= 2 || !fds.insert(m.fd)) {
        return Err(anyhow_message("Invalid or duplicate inherited descriptor"));
    }
    // SAFETY: inspect every supplied number before constructing unique File owners in this worker.
    if fds
        .iter()
        .any(|fd| unsafe { libc::fcntl(*fd, libc::F_GETFD) } < 0)
    {
        return Err(anyhow_message("Missing inherited descriptor"));
    }
    let mut lookup = BTreeMap::new();
    let mut order = Vec::with_capacity(members.len());
    for member in members {
        let name = PathBuf::from(member.name.as_str());
        // SAFETY: dedicated process; validated unique inherited descriptor, distinct from stdio/output.
        let file = unsafe { File::from_raw_fd(member.fd) };
        if lookup
            .insert(name.clone(), (file, member.version))
            .is_some()
        {
            return Err(anyhow_message("Duplicate logical publication name"));
        }
        order.push(name);
    }
    Ok(InheritedMembers { lookup, order })
}

/// Parse the mature transcript formats within the parent-admitted resident envelope.
fn transcript() -> Result<Vec<u8>, String> {
    let bytes = read_stdin()?;
    let segments = knowledge_processing::media::parse_transcript_bytes(&bytes)
        .map_err(|e| format!("{e:#}"))?;
    serde_json::to_vec(&segments).map_err(|_| anyhow_message("cannot encode transcript"))
}

/// Render original/source or extracted line evidence in the same real watched worker.
fn preview(mut args: impl Iterator<Item = String>) -> Result<Vec<u8>, String> {
    let kind = args
        .next()
        .ok_or_else(|| anyhow_message("preview kind is missing"))?;
    let extension = args
        .next()
        .ok_or_else(|| anyhow_message("preview extension is missing"))?;
    let filename = args
        .next()
        .ok_or_else(|| anyhow_message("preview filename is missing"))?;
    let maximum = args.next();
    let max_bytes = parse_max(maximum.as_deref())?;
    let line = args.next().and_then(|s| s.parse::<usize>().ok());
    let bytes = read_stdin()?;
    knowledge_processing::extract::preview_loaded(
        &kind, &extension, &filename, &bytes, max_bytes, line,
    )
    .map(String::into_bytes)
    .map_err(|e| format!("{e:#}"))
}

fn parse_max(value: Option<&str>) -> Result<usize, String> {
    value
        .ok_or_else(|| anyhow_message("byte limit is missing"))?
        .parse()
        .map_err(|_| anyhow_message("byte limit is invalid"))
}

fn read_stdin() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow_message("cannot read parser input"))?;
    Ok(bytes)
}

fn anyhow_message(text: &str) -> String {
    text.to_string()
}
