//! Downstream feasibility probe through the pinned kernel's public filesystem API.
//! This is build evidence, not an independent full retrieval conformance suite.

use retrieval_kernel::{Kernel, Locator, Mode, directory::Directory};
use std::{error::Error, fs};

/// Publish synthetic files, query all modes, reopen the store, and reject stale hits.
///
/// Returns a bounded success summary or a filesystem/kernel error.
/// Assertions enforce the accepted exact-byte, LF-line regex, multilingual lexical,
/// restart, and stale-withholding behavior; temporary roots are removed on return.
pub fn run_probe() -> Result<&'static str, Box<dyn Error>> {
    let temporary = tempfile::tempdir()?;
    let sources = temporary.path().join("sources");
    let store = temporary.path().join("store");
    fs::create_dir(&sources)?;
    fs::create_dir(&store)?;
    fs::write(sources.join("english.txt"), "reliable retrieval library\n")?;
    fs::write(sources.join("chinese.txt"), "知识检索与文档管理\n")?;
    fs::write(
        sources.join("japanese.txt"),
        "図書館で日本語の資料を検索します。\n",
    )?;
    fs::write(sources.join("notes.txt"), "Alpha\nalpha\nbanana\n")?;

    let kernel = Kernel::new(&sources, &store)?;
    let directory = Directory::new(&sources, &store)?;
    let generation = kernel.publish(directory.inventory()?, None)?;
    assert_eq!(
        kernel.generation(directory.scope())?,
        Some(generation.clone())
    );

    for (query, filename) in [
        ("retrieval", "english.txt"),
        ("知识", "chinese.txt"),
        ("図書館", "japanese.txt"),
    ] {
        let result = directory.search(&kernel, Mode::Lexical, query.into(), 20)?;
        assert!(result.complete, "multilingual lexical coverage");
        assert_eq!(result.hits.len(), 1, "one matching fixture for {query}");
        assert_eq!(result.hits[0].key.id, filename);
    }

    // The accepted kernel contract preserves overlapping canonical byte matches.
    let exact = directory.search(&kernel, Mode::Exact, "ana".into(), 20)?;
    assert!(exact.complete);
    let offsets: Vec<_> = exact.hits.iter().map(|hit| &hit.locator).collect();
    assert_eq!(
        offsets,
        vec![
            &Locator::Text {
                start: 13,
                end: 16,
                original: None
            },
            &Locator::Text {
                start: 15,
                end: 18,
                original: None
            },
        ]
    );
    // LF-line regex is case-sensitive and excludes the LF from its coordinates.
    let regex = directory.search(&kernel, Mode::Regex, "^alpha$".into(), 20)?;
    assert!(regex.complete);
    assert_eq!(regex.hits.len(), 1);
    assert_eq!(
        regex.hits[0].locator,
        Locator::Text {
            start: 6,
            end: 11,
            original: None
        }
    );
    drop(kernel);

    let reopened = Kernel::new(&sources, &store)?;
    assert_eq!(
        reopened.generation(directory.scope())?,
        Some(generation.clone())
    );
    assert_eq!(
        reopened.publish(directory.inventory()?, Some(generation.clone()))?,
        generation,
        "unchanged inventory publication is a no-op"
    );
    assert_eq!(
        directory
            .search(&reopened, Mode::Exact, "ana".into(), 20)?
            .hits
            .len(),
        2
    );
    fs::write(sources.join("notes.txt"), "replacement\n")?;
    let stale = directory.search(&reopened, Mode::Exact, "ana".into(), 20)?;
    assert!(
        !stale.complete,
        "changed source is incomplete until reconciliation"
    );
    assert!(
        stale.hits.is_empty(),
        "changed source cannot disclose stale matches"
    );
    Ok(
        "PASS: publish; English/Chinese/Japanese lexical; overlapping exact bytes; LF regex; reopen/no-op; stale withholding",
    )
}

#[cfg(test)]
mod tests {
    /// Exercise the actual downstream library against disposable filesystem state.
    #[test]
    fn downstream_filesystem_consumer() -> Result<(), Box<dyn std::error::Error>> {
        super::run_probe()?;
        Ok(())
    }
}
