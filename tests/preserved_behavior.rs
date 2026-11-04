//! Independent preservation checks derived from the committed storage and search contracts.
//!
//! These checks use only public library operations and synthetic authored files.
//! Replacing coupled coverage of still-required behavior needs an OLD/NEW passing pair.
//! The separately named writer-release module is a defect regression, not a passing OLD replacement.

#[cfg(unix)]
#[path = "contracts/writer_release.rs"]
mod writer_release;

use omni_knowledge::{
    IndexOptions, KnowledgeBase, SearchMethod, SearchMode, SearchOptions, UnifiedSearchOptions,
};
use std::collections::BTreeSet;
use std::fs;
use std::sync::{Arc, Barrier};

/// Create a disposable real library; fixture creation failures fail the test.
fn library() -> (tempfile::TempDir, KnowledgeBase) {
    let directory = tempfile::tempdir().unwrap();
    let library = KnowledgeBase::init(directory.path(), "Preservation contracts").unwrap();
    (directory, library)
}

/// Return source coordinates without depending on snippets or incidental diagnostic wording.
fn coordinates(hits: &[omni_knowledge::SearchHit]) -> Vec<(&str, Option<usize>)> {
    hits.iter()
        .map(|hit| (hit.path.as_str(), hit.line))
        .collect()
}

/// Competing imports must preserve both originals and install at most one complete destination.
/// Storage / Mutations owns exclusive import and source preservation, including retry refusal.
#[test]
fn competing_imports_preserve_sources_and_the_winning_destination() {
    let (_home, kb) = library();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let sources = [
        left.path().join("collision.bin"),
        right.path().join("collision.bin"),
    ];
    let contents = [vec![0x19; 8192], vec![0xe7; 8192]];
    for (path, bytes) in sources.iter().zip(&contents) {
        fs::write(path, bytes).unwrap();
    }
    let start = Arc::new(Barrier::new(2));
    let outcomes = std::thread::scope(|scope| {
        let tasks: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let start = Arc::clone(&start);
                let root = kb.root.clone();
                scope.spawn(move || {
                    let writer = KnowledgeBase::open(&root).unwrap();
                    start.wait();
                    writer.import(source, &[format!("Author{index}")])
                })
            })
            .collect();
        tasks
            .into_iter()
            .map(|task| task.join().unwrap())
            .collect::<Vec<_>>()
    });
    // No-clobber allows either writer to win, never two successful copies to one destination.
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    let winner = outcomes.iter().position(Result::is_ok).unwrap();
    let destination = outcomes[winner].as_ref().unwrap();
    assert_eq!(
        fs::read(kb.resolve(destination).unwrap()).unwrap(),
        contents[winner]
    );
    let reopened = KnowledgeBase::open(&kb.root).unwrap();
    let before = reopened.metadata(destination).unwrap();
    assert_eq!(before.tags, vec![format!("Author{winner}")]);
    assert!(
        reopened
            .import(&sources[1 - winner], &["Replacement".into()])
            .is_err()
    );
    assert_eq!(
        fs::read(reopened.resolve(destination).unwrap()).unwrap(),
        contents[winner]
    );
    assert_eq!(reopened.metadata(destination).unwrap().tags, before.tags);
    for (source, bytes) in sources.iter().zip(&contents) {
        assert_eq!(&fs::read(source).unwrap(), bytes);
    }
}

/// Authored case variants survive readback; all exact filters apply before a result bound.
/// Storage / Authority owns the tag union; Search / Strategies owns case and AND filtering.
#[test]
fn authored_tag_union_keeps_spelling_and_requires_every_complete_filter() {
    let (_home, kb) = library();
    let source = "---\ntags: [MiXeD]\n---\ncontracttagneedle\n";
    fs::write(kb.root.join("raw/z-eligible.md"), source).unwrap();
    kb.set_metadata(
        "raw/z-eligible.md",
        &["mIxEd".into(), "PhaseTwo".into()],
        None,
    )
    .unwrap();
    for (path, tags) in [
        ("raw/a-missing.md", vec!["MiXeD".into()]),
        (
            "raw/b-prefix.md",
            vec!["MiXeD-extra".into(), "PhaseTwo".into()],
        ),
    ] {
        fs::write(kb.root.join(path), "contracttagneedle\n").unwrap();
        kb.set_metadata(path, &tags, None).unwrap();
    }
    kb.index(&IndexOptions::default()).unwrap();
    let reopened = KnowledgeBase::open(&kb.root).unwrap();
    assert_eq!(
        fs::read_to_string(kb.root.join("raw/z-eligible.md")).unwrap(),
        source
    );
    assert_eq!(
        reopened
            .metadata("raw/z-eligible.md")
            .unwrap()
            .tags
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["mIxEd".to_owned(), "PhaseTwo".to_owned()])
    );
    let request = SearchOptions {
        query: "contracttagneedle".into(),
        mode: SearchMode::Grep,
        tags: vec!["MIXED".into(), "phasetwo".into()],
        limit: 1,
        ..Default::default()
    };
    let hits = reopened.search(&request).unwrap();
    assert_eq!(coordinates(&hits), vec![("raw/z-eligible.md", Some(4))]);
    assert_eq!(
        hits[0]
            .tags
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["MiXeD", "mIxEd", "PhaseTwo"])
    );
    assert!(
        reopened
            .search(&SearchOptions {
                case_sensitive: true,
                ..request.clone()
            })
            .unwrap()
            .is_empty()
    );
    assert!(
        reopened
            .search(&SearchOptions {
                tags: vec!["mix".into(), "phasetwo".into()],
                ..request.clone()
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        coordinates(
            &reopened
                .search(&SearchOptions {
                    tags: vec!["MiXeD".into(), "PhaseTwo".into()],
                    case_sensitive: true,
                    ..request
                })
                .unwrap()
        ),
        vec![("raw/z-eligible.md", Some(4))]
    );
}

/// Canonical line scans must not lose substring, short, alternative, or case-folded matches.
/// Search / Strategies owns deterministic path/line ordering and conservative trigram filtering.
#[test]
fn regex_enumerates_all_fixture_lines_and_literal_punctuation_stays_literal() {
    let (_home, kb) = library();
    for (path, text) in [
        ("raw/scan-a.txt", "precontractneedlepost\nab\n甲\nKQ+a\n"),
        ("raw/scan-b.txt", "CONTRACTNEEDLE\nbeta\n乙甲\nKQ+a\n"),
        ("raw/scan-c.txt", "unrelated\nKQQa\n"),
    ] {
        fs::write(kb.root.join(path), text).unwrap();
        kb.set_metadata(path, &["ScanFixture".into()], None)
            .unwrap();
    }
    kb.index(&IndexOptions::default()).unwrap();
    let cases = [
        ("contractneedle", true, vec![("raw/scan-a.txt", Some(1))]),
        (
            "contractneedle",
            false,
            vec![("raw/scan-a.txt", Some(1)), ("raw/scan-b.txt", Some(1))],
        ),
        (
            "^ab$|^beta$",
            true,
            vec![("raw/scan-a.txt", Some(2)), ("raw/scan-b.txt", Some(2))],
        ),
        (
            "甲",
            true,
            vec![("raw/scan-a.txt", Some(3)), ("raw/scan-b.txt", Some(3))],
        ),
        ("KQ+a", true, vec![("raw/scan-c.txt", Some(2))]),
        ("contractneedle.*ab", false, vec![]),
    ];
    for (query, case_sensitive, expected) in cases {
        let hits = kb
            .search(&SearchOptions {
                query: query.into(),
                mode: SearchMode::Grep,
                tags: vec!["ScanFixture".into()],
                case_sensitive,
                limit: 100,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            coordinates(&hits),
            expected,
            "query={query}, case_sensitive={case_sensitive}"
        );
    }
    let literal = kb
        .search_unified(&UnifiedSearchOptions {
            query: "kq+A".into(),
            methods: vec![SearchMethod::Grep],
            limit: 100,
        })
        .unwrap();
    assert_eq!(
        coordinates(&literal),
        vec![("raw/scan-a.txt", Some(4)), ("raw/scan-b.txt", Some(4))]
    );
}

/// Fragment multiplicity changes evidence, not the dense file vote in each method list.
/// Search / Unified Desktop Query owns RRF sum(1/(60+rank)) and retained line evidence.
#[test]
fn fusion_votes_once_per_file_while_retaining_every_distinct_line() {
    let (_home, kb) = library();
    fs::write(kb.root.join("raw/b-tagged.txt"), "contractfusionneedle\n").unwrap();
    kb.set_metadata("raw/b-tagged.txt", &["contractfusionneedle".into()], None)
        .unwrap();
    for occurrences in [8, 1] {
        fs::write(
            kb.root.join("raw/a-many.txt"),
            "contractfusionneedle\n".repeat(occurrences),
        )
        .unwrap();
        kb.index(&IndexOptions::default()).unwrap();
        let hits = kb
            .search_unified(&UnifiedSearchOptions {
                query: "contractfusionneedle".into(),
                methods: vec![SearchMethod::Grep, SearchMethod::Tags],
                limit: 100,
            })
            .unwrap();
        // Grep ranks a then b; the tag list contributes one additional rank-one vote to b.
        assert_eq!(hits[0].path, "raw/b-tagged.txt");
        let expected = [
            ("raw/b-tagged.txt", 1.0_f32 / 62.0 + 1.0 / 61.0),
            ("raw/a-many.txt", 1.0 / 61.0),
        ];
        for (path, score) in expected {
            let evidence: Vec<_> = hits.iter().filter(|hit| hit.path == path).collect();
            assert!(!evidence.is_empty());
            assert!(evidence.iter().all(|hit| (hit.score - score).abs() < 1e-6));
        }
        let lines: Vec<_> = hits
            .iter()
            .filter(|hit| hit.path == "raw/a-many.txt")
            .map(|hit| hit.line)
            .collect();
        assert_eq!(lines, (1..=occurrences).map(Some).collect::<Vec<_>>());
        assert_eq!(
            hits.iter()
                .map(|hit| hit.path.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["raw/a-many.txt", "raw/b-tagged.txt"])
        );
    }
}
