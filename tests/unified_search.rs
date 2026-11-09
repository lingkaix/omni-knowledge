//! Contract checks for unified literal keywords, peer retrieval methods, and source evidence.

use omni_knowledge as fixture_library;
#[path = "support/retained.rs"]
mod retained;
use retained::legacy_init;

use omni_knowledge::{
    IndexOptions, KnowledgeBase, SearchMethod, SearchMode, SearchOptions, UnifiedSearchOptions,
};
use std::collections::BTreeSet;
use std::fs;

/// Create an explicit v1 retrieval component fixture with independent text/tag/filename sources.
fn library() -> (tempfile::TempDir, KnowledgeBase) {
    let directory = tempfile::tempdir().unwrap();
    let kb = legacy_init(directory.path(), "Unified retrieval fixture").unwrap();
    fs::write(kb.root.join("raw/content.txt"), "needle\nNEEDLE and a+b\n").unwrap();
    fs::write(kb.root.join("raw/other.txt"), "secondkeyword\n").unwrap();
    fs::write(kb.root.join("raw/needle.bin"), [0, 255, 0]).unwrap();
    fs::write(kb.root.join("raw/tagged.bin"), [0, 255, 1]).unwrap();
    kb.set_metadata("raw/tagged.bin", &["NeEdLe".into()], None)
        .unwrap();
    kb.set_metadata("raw/content.txt", &["literalfixture".into()], None)
        .unwrap();
    kb.index(&IndexOptions::default()).unwrap();
    (directory, kb)
}

/// Query a method set through the public API with a generous evidence bound.
fn options(query: &str, methods: &[SearchMethod]) -> UnifiedSearchOptions {
    UnifiedSearchOptions {
        query: query.into(),
        methods: methods.into(),
        limit: 100,
    }
}

/// Different keyword spellings and separators name one alternative each, never extra rank votes.
#[test]
fn keywords_normalize_commas_case_and_repetition() {
    let request = options(
        " Needle, needle ， SECONDkeyword,,  ",
        &[SearchMethod::Grep],
    );
    assert_eq!(request.keywords(), vec!["needle", "secondkeyword"]);
    let (_directory, kb) = library();
    let hits = kb.search_unified(&request).unwrap();
    let paths: BTreeSet<_> = hits.iter().map(|hit| hit.path.as_str()).collect();
    assert_eq!(
        paths,
        BTreeSet::from(["raw/content.txt", "raw/needle.bin", "raw/other.txt"])
    );
    let once = kb
        .search_unified(&options("needle", &[SearchMethod::Grep]))
        .unwrap();
    let repeated = kb
        .search_unified(&options(
            "NEEDLE, needle，Needle",
            &[SearchMethod::Grep, SearchMethod::Grep],
        ))
        .unwrap();
    assert_eq!(
        serde_json::to_value(once).unwrap(),
        serde_json::to_value(repeated).unwrap()
    );
}

/// A literal plus sign cannot become a regex repetition, and matching ignores source case.
#[test]
fn unified_text_is_literal_while_cli_grep_keeps_regex() {
    let (_directory, kb) = library();
    let literal = kb
        .search_unified(&options("A+B", &[SearchMethod::Grep]))
        .unwrap();
    assert_eq!(literal.len(), 1);
    assert_eq!(literal[0].path, "raw/content.txt");
    assert_eq!(literal[0].line, Some(2));
    assert!(
        kb.search(&SearchOptions {
            query: "A+B".into(),
            mode: SearchMode::Grep,
            tags: vec!["literalfixture".into()],
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
}

/// Enabling tags adds exact alternatives to other methods instead of filtering their results.
#[test]
fn peer_tags_and_text_are_case_insensitive_alternatives() {
    let (_directory, kb) = library();
    let hits = kb
        .search_unified(&options(
            "NEEDLE",
            &[SearchMethod::Grep, SearchMethod::Tags],
        ))
        .unwrap();
    let paths: BTreeSet<_> = hits.iter().map(|hit| hit.path.as_str()).collect();
    assert!(paths.contains("raw/content.txt"));
    assert!(paths.contains("raw/tagged.bin"));
    assert!(paths.contains("raw/needle.bin"));
    let tagged = kb
        .search_unified(&options("needle", &[SearchMethod::Tags]))
        .unwrap();
    assert_eq!(tagged.len(), 1);
    assert_eq!(tagged[0].path, "raw/tagged.bin");
    assert!(
        kb.search_unified(&options("need", &[SearchMethod::Tags]))
            .unwrap()
            .is_empty()
    );
}

/// Default exact filters match old mixed-case stored tags before limiting every lexical mode.
#[test]
fn existing_tag_filters_fold_case_without_reindexing() {
    let (_directory, kb) = library();
    for mode in [SearchMode::Bm25, SearchMode::Grep, SearchMode::Files] {
        let request = SearchOptions {
            query: if mode == SearchMode::Files {
                "tagged.bin".into()
            } else {
                "needle".into()
            },
            tags: vec!["NEEDLE".into()],
            mode,
            limit: 1,
            ..Default::default()
        };
        // Grep searches content and descriptions, not tags, so an empty query supplies navigation.
        let request = if mode == SearchMode::Grep {
            SearchOptions {
                query: String::new(),
                ..request
            }
        } else {
            request
        };
        assert_eq!(kb.search(&request).unwrap()[0].path, "raw/tagged.bin");
        assert!(
            kb.search(&SearchOptions {
                case_sensitive: true,
                ..request
            })
            .unwrap()
            .is_empty()
        );
    }
}

/// Fusion keeps each real text location once and never invents a line for a binary filename hit.
#[test]
fn combined_results_preserve_unique_locations_and_real_coordinates() {
    let (_directory, kb) = library();
    let hits = kb
        .search_unified(&options(
            "needle",
            &[SearchMethod::Bm25, SearchMethod::Grep],
        ))
        .unwrap();
    let content_lines: Vec<_> = hits
        .iter()
        .filter(|hit| hit.path == "raw/content.txt")
        .map(|hit| hit.line)
        .collect();
    assert_eq!(content_lines, vec![Some(1), Some(2)]);
    let binary = hits
        .iter()
        .find(|hit| hit.path == "raw/needle.bin")
        .unwrap();
    assert_eq!(binary.line, None);
    assert!(binary.start_seconds.is_none());
    let legacy = kb
        .search(&SearchOptions {
            query: "needle.bin".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(legacy[0].line, None);
    let filename_only = kb
        .search(&SearchOptions {
            query: "content.txt".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(filename_only[0].path, "raw/content.txt");
    assert_eq!(filename_only[0].line, None);
}

/// One ordinary keyword and one method preserve the underlying mode's complete result order.
#[test]
fn single_method_preserves_existing_retrieval_ranking() {
    let (_directory, kb) = library();
    for (method, mode) in [
        (SearchMethod::Bm25, SearchMode::Bm25),
        (SearchMethod::Grep, SearchMode::Grep),
        (SearchMethod::Files, SearchMode::Files),
    ] {
        let unified = kb.search_unified(&options("needle", &[method])).unwrap();
        let existing = kb
            .search(&SearchOptions {
                query: "needle".into(),
                mode,
                limit: 100,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(unified).unwrap(),
            serde_json::to_value(existing).unwrap()
        );
    }
}

/// Empty alternatives restore indexed navigation even when every method chip is disabled.
#[test]
fn blank_queries_browse_without_selected_methods() {
    let (_directory, kb) = library();
    let unified = kb.search_unified(&options(" ,， ", &[])).unwrap();
    let browsing = kb
        .search(&SearchOptions {
            mode: SearchMode::Files,
            limit: 100,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        serde_json::to_value(unified).unwrap(),
        serde_json::to_value(browsing).unwrap()
    );
}

/// Invalid selections fail explicitly, including semantic failure after local methods could match.
#[test]
fn invalid_methods_limits_and_semantic_failures_are_explicit() {
    let (_directory, kb) = library();
    assert!(kb.search_unified(&options("needle", &[])).is_err());
    assert!(
        kb.search_unified(&UnifiedSearchOptions {
            limit: 10_001,
            ..options("needle", &[SearchMethod::Files])
        })
        .is_err()
    );
    assert!(
        kb.search_unified(&options(
            "needle",
            &[SearchMethod::Files, SearchMethod::Semantic]
        ))
        .unwrap_err()
        .to_string()
        .contains("semantic")
    );
    assert!(
        kb.search_unified(&UnifiedSearchOptions {
            limit: 0,
            ..options("needle", &[SearchMethod::Files])
        })
        .unwrap()
        .is_empty()
    );
}
