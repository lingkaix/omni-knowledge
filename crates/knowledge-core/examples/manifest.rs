//! Bounded stdin-to-stdout demonstration of the actual public codec, with no filesystem effects.
use knowledge_core::manifest::{ManifestDocument, ManifestLimits};
use std::io::{Read, Write};

/// Inspect/copy a synthetic manifest, or make a pure name-only candidate with `--rename NAME`.
///
/// The example's 1 MiB budget is a demonstration choice, not a manifest-format limit.
/// Read-only input is copied verbatim without a rename; rename refusal emits no stdout.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    const EXAMPLE_BYTES: usize = 1_048_576;
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let name = match arguments.as_slice() {
        [] => None,
        [flag, name] if flag == "--rename" => Some(name.as_str()),
        _ => return Err("usage: manifest [--rename NAME]".into()),
    };
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(EXAMPLE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let document = ManifestDocument::decode(
        &bytes,
        ManifestLimits {
            max_bytes: EXAMPLE_BYTES,
        },
    )?;
    eprintln!("state: {:?}", document.state());
    let candidate = name.map(|name| document.with_name(name)).transpose()?;
    let output = candidate.as_ref().unwrap_or(&document);
    std::io::stdout()
        .lock()
        .write_all(output.original_bytes())?;
    Ok(())
}
