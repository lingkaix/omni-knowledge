//! File-level orchestration over Root's one admitted kernel facade and disk vector generations.
use crate::{
    FileRecord, KnowledgeBase, SearchHit, SearchMethod, SearchMode, SearchOptions, Snapshot,
    UnifiedSearchOptions, embedding,
};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

/// Publishes the kernel store inside the owned index directory.
/// The signature stays so the existing publisher can record every generated file.
pub(crate) fn build_bm25(directory: &Path, records: &[FileRecord]) -> Result<()> {
    let _ = records;
    crate::admitted::write_reference(directory)
}

/// Trigram postings are not a lexical engine. Exact and regex scans read every admitted source.
pub(crate) fn build_trigrams(_records: &[FileRecord]) -> BTreeMap<String, Vec<usize>> {
    BTreeMap::new()
}

/// Executes the selected strategy; empty lexical queries return filtered file navigation.
pub(crate) fn search(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    directory: &Path,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    if options.query_image.is_some()
        && !matches!(options.mode, SearchMode::Semantic | SearchMode::Hybrid)
    {
        bail!("image queries require semantic or hybrid mode");
    }
    if options.query.trim().is_empty()
        && options.query_image.is_none()
        && !matches!(options.mode, SearchMode::Semantic | SearchMode::Hybrid)
    {
        return files(kb, snapshot, options);
    }
    match options.mode {
        SearchMode::Files => files(kb, snapshot, options),
        SearchMode::Grep => grep(kb, snapshot, directory, options),
        SearchMode::Bm25 => bm25(kb, snapshot, directory, options),
        SearchMode::Semantic => semantic(kb, snapshot, options),
        SearchMode::Hybrid => {
            let expanded = SearchOptions {
                limit: options.limit.saturating_mul(5).clamp(50, 10_000),
                ..options.clone()
            };
            let lexical = if options.query.trim().is_empty() {
                vec![]
            } else {
                bm25(kb, snapshot, directory, &expanded)?
            };
            let semantic = semantic(kb, snapshot, &expanded)?;
            let mut fused: HashMap<String, (f32, SearchHit)> = HashMap::new();
            for list in [lexical, semantic] {
                let mut seen = HashSet::new();
                for (rank, hit) in list.into_iter().enumerate() {
                    // Fusion ranks files once; semantic fragments keep their best evidence.
                    if !seen.insert(hit.path.clone()) {
                        if hit.start_seconds.is_some()
                            && let Some(entry) = fused.get_mut(&hit.path)
                            && entry.1.start_seconds.is_none()
                        {
                            entry.1 = hit;
                        }
                        continue;
                    }
                    let contribution = 1.0 / (60.0 + rank as f32 + 1.0);
                    let entry = fused
                        .entry(hit.path.clone())
                        .or_insert_with(|| (0.0, hit.clone()));
                    if hit.start_seconds.is_some()
                        || (entry.1.mode != "semantic" && hit.mode == "semantic")
                    {
                        entry.1 = hit;
                    }
                    entry.0 += contribution;
                }
            }
            let mut output: Vec<_> = fused
                .into_values()
                .map(|(score, mut hit)| {
                    hit.score = score;
                    hit.mode = "hybrid".into();
                    hit
                })
                .collect();
            output.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.path.cmp(&b.path)));
            output.truncate(options.limit);
            Ok(output)
        }
    }
}

/// Requires every complete tag, ignoring case unless explicitly requested otherwise.
fn tagged(record: &FileRecord, options: &SearchOptions) -> bool {
    options.tags.iter().all(|tag| {
        if options.case_sensitive {
            record.tags.contains(tag)
        } else {
            let folded = tag.to_lowercase();
            record
                .tags
                .iter()
                .any(|stored| stored.to_lowercase() == folded)
        }
    })
}

/// Runs keyword alternatives against peer methods without reacquiring or reparsing the snapshot.
pub(crate) fn search_unified(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    directory: &Path,
    options: &UnifiedSearchOptions,
) -> Result<Vec<SearchHit>> {
    let keywords = options.keywords();
    if keywords.is_empty() {
        return files(
            kb,
            snapshot,
            &SearchOptions {
                limit: options.limit,
                ..Default::default()
            },
        );
    }
    let methods: BTreeSet<_> = options.methods.iter().copied().collect();
    let single_list = methods.len() == 1 && keywords.len() == 1;
    let candidate_limit = if single_list {
        options.limit
    } else {
        options.limit.saturating_mul(5).clamp(50, 10_000)
    };
    let mut fused = HashMap::new();
    for method in methods {
        for keyword in &keywords {
            let mut request = SearchOptions {
                query: keyword.clone(),
                limit: candidate_limit,
                ..Default::default()
            };
            let hits = match method {
                SearchMethod::Bm25 => bm25(kb, snapshot, directory, &request)?,
                SearchMethod::Grep => {
                    // Desktop keywords are literal; the advanced CLI regex contract is unchanged.
                    request.query = regex::escape(keyword);
                    grep(kb, snapshot, directory, &request)?
                }
                SearchMethod::Semantic => semantic(kb, snapshot, &request)?,
                SearchMethod::Files => files(kb, snapshot, &request)?,
                SearchMethod::Tags => {
                    request.query.clear();
                    request.tags.push(keyword.clone());
                    let mut hits = files(kb, snapshot, &request)?;
                    for hit in &mut hits {
                        hit.mode = "tags".into();
                    }
                    hits
                }
            };
            if single_list {
                return Ok(hits);
            }
            add_ranked_list(&mut fused, hits);
        }
    }
    Ok(fused_results(fused, options.limit))
}

/// Identifies a previewable occurrence independently of the method or keyword that found it.
#[derive(Hash, PartialEq, Eq)]
enum EvidenceLocation {
    /// One original or extracted text line.
    Line(usize),
    /// A timed media range represented by its exact finite indexed values.
    Time(u64, Option<u64>),
    /// Filename, tag, image, or other whole-file evidence without a source position.
    File,
}

impl EvidenceLocation {
    /// Derives a stable location key, preferring recording time to text coordinates.
    fn from_hit(hit: &SearchHit) -> Self {
        if let Some(start) = hit.start_seconds {
            Self::Time(start.to_bits(), hit.end_seconds.map(f64::to_bits))
        } else if let Some(line) = hit.line {
            Self::Line(line)
        } else {
            Self::File
        }
    }
}

/// Accumulates one reciprocal-rank vote per file and distinct actionable occurrences.
#[derive(Default)]
struct FusedFile {
    /// Sum of file-level reciprocal-rank contributions.
    score: f32,
    /// Evidence in its first returned order across method and keyword lists.
    hits: Vec<SearchHit>,
    /// Already retained preview locations.
    locations: HashSet<EvidenceLocation>,
}

/// Adds a method/keyword list using dense distinct-file ranks; repeated fragments add no votes.
fn add_ranked_list(fused: &mut HashMap<String, FusedFile>, hits: Vec<SearchHit>) {
    let mut seen = HashSet::new();
    for hit in hits {
        let fresh_file = seen.insert(hit.path.clone());
        let rank = seen.len();
        let entry = fused.entry(hit.path.clone()).or_default();
        if fresh_file {
            entry.score += 1.0 / (60.0 + rank as f32);
        }
        let location = EvidenceLocation::from_hit(&hit);
        if location == EvidenceLocation::File && !entry.locations.is_empty() {
            continue;
        }
        if location != EvidenceLocation::File && entry.locations.remove(&EvidenceLocation::File) {
            entry.hits.clear();
        }
        if entry.locations.insert(location) {
            entry.hits.push(hit);
        }
    }
}

/// Orders complete file groups by fused relevance, then applies the requested evidence bound.
fn fused_results(fused: HashMap<String, FusedFile>, limit: usize) -> Vec<SearchHit> {
    let mut files: Vec<_> = fused.into_iter().collect();
    files.sort_by(|(path_a, a), (path_b, b)| b.score.total_cmp(&a.score).then(path_a.cmp(path_b)));
    files
        .into_iter()
        .flat_map(|(_, file)| {
            file.hits.into_iter().map(move |mut hit| {
                hit.score = file.score;
                hit.mode = "combined".into();
                hit
            })
        })
        .take(limit)
        .collect()
}

/// Makes a bounded source hit from an indexed file.
fn hit(
    kb: &KnowledgeBase,
    record: &FileRecord,
    mode: &str,
    score: f32,
    snippet: String,
    line: Option<usize>,
) -> SearchHit {
    let speech = if line.is_some() {
        record
            .transcript
            .iter()
            .find(|item| item.text.contains(&snippet))
    } else {
        None
    };
    SearchHit {
        root: kb.root.clone(),
        path: record.path.clone(),
        title: record.title.clone(),
        snippet: snippet.chars().take(500).collect(),
        mode: mode.into(),
        score,
        line: if !record.transcript.is_empty() {
            None
        } else {
            line
        },
        start_seconds: speech.map(|item| item.start_seconds),
        end_seconds: speech.map(|item| item.end_seconds),
        tags: record.tags.clone(),
    }
}

/// Browses all types by filename, display title, descriptions, and exact tag filters.
fn files(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let query = if options.case_sensitive {
        options.query.clone()
    } else {
        options.query.to_lowercase()
    };
    snapshot
        .records
        .iter()
        .filter(|r| tagged(r, options))
        .filter(|r| {
            let text = format!("{} {} {}", r.path, r.title, r.description);
            if options.case_sensitive {
                text.contains(&query)
            } else {
                text.to_lowercase().contains(&query)
            }
        })
        .take(options.limit)
        .map(|r| {
            crate::admitted::charge_hit(kb, r, r.description.len())?;
            Ok(hit(kb, r, "files", 1.0, r.description.clone(), None))
        })
        .collect()
}

/// All lexical methods use the exact Root adapter, generation plan and Core account.
fn bm25(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    _directory: &Path,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    crate::admitted::lexical(
        kb,
        snapshot,
        &SearchOptions {
            mode: SearchMode::Bm25,
            ..options.clone()
        },
    )
}
/// Regex scans every admitted body, never BM25 candidates or metadata-only pseudo-body matches.
fn grep(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    _directory: &Path,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    crate::admitted::lexical(
        kb,
        snapshot,
        &SearchOptions {
            mode: SearchMode::Grep,
            ..options.clone()
        },
    )
}

/// Queries the validated local space and scores finite matching fragments exactly.
fn semantic(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    if snapshot.vectors.is_empty() {
        bail!("no semantic vectors indexed; start the provider and run omni index --embeddings");
    }
    let identity = snapshot
        .embedding
        .as_ref()
        .context("snapshot lacks model identity")?;
    if kb.config.embedding.as_ref() != Some(identity) {
        bail!("configured embedding identity differs from index; run index --embeddings --force");
    }
    let client = embedding::EmbeddingClient::new(identity)?;
    let query = if let Some(image) = &options.query_image {
        client.embed_image(image)?
    } else {
        if options.query.trim().is_empty() {
            bail!("semantic query must contain text or an image");
        }
        client.embed_text(&options.query)?
    };
    let lookup: HashMap<_, _> = snapshot
        .records
        .iter()
        .filter(|r| tagged(r, options))
        .map(|r| (r.path.as_str(), r))
        .collect();
    let mut output = vec![];
    for vector in &snapshot.vectors {
        if let Some(record) = lookup.get(vector.path.as_str()) {
            let score = embedding::cosine_similarity(&query, &vector.vector)?;
            crate::admitted::charge_hit(kb, record, vector.text.len())?;
            let mut item = hit(
                kb,
                record,
                "semantic",
                score,
                vector.text.clone(),
                vector.line,
            );
            item.start_seconds = vector.start_seconds;
            item.end_seconds = vector.end_seconds;
            output.push(item);
        }
    }
    output.sort_by(|a, b| {
        b.score.total_cmp(&a.score).then(a.path.cmp(&b.path)).then(
            a.start_seconds
                .partial_cmp(&b.start_seconds)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    output.truncate(options.limit);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates one synthetic occurrence to isolate file-rank fusion from extraction and scoring.
    fn evidence(path: &str, line: Option<usize>, time: Option<f64>) -> SearchHit {
        SearchHit {
            root: "/fixture".into(),
            path: path.into(),
            title: path.into(),
            snippet: "evidence".into(),
            mode: "fixture".into(),
            score: 1.0,
            line,
            start_seconds: time,
            end_seconds: time.map(|start| start + 1.0),
            tags: vec![],
        }
    }

    /// Repeated fragments neither add file votes nor push the next file down the rank scale.
    #[test]
    fn fusion_votes_once_per_dense_file_rank() {
        let mut fused = HashMap::new();
        let mut first: Vec<_> = (1..=100)
            .map(|line| evidence("a.txt", Some(line), None))
            .collect();
        first.push(evidence("b.txt", Some(1), None));
        add_ranked_list(&mut fused, first);
        add_ranked_list(
            &mut fused,
            vec![
                evidence("b.txt", Some(1), None),
                evidence("a.txt", Some(1), None),
            ],
        );
        let expected = 1.0 / 61.0 + 1.0 / 62.0;
        assert_eq!(fused["a.txt"].score, expected);
        assert_eq!(fused["b.txt"].score, expected);
        let output = fused_results(fused, 1000);
        assert_eq!(output.len(), 101);
        assert_eq!(output[0].path, "a.txt");
        assert_eq!(output[100].path, "b.txt");
    }

    /// Timed occurrences survive fusion once each, while redundant whole-file evidence is omitted.
    #[test]
    fn fusion_retains_distinct_timed_locations() {
        let mut fused = HashMap::new();
        add_ranked_list(&mut fused, vec![evidence("clip.mp4", None, None)]);
        add_ranked_list(
            &mut fused,
            vec![
                evidence("clip.mp4", None, Some(5.0)),
                evidence("clip.mp4", None, Some(5.0)),
                evidence("clip.mp4", None, Some(10.0)),
                evidence("clip.mp4", None, None),
            ],
        );
        let output = fused_results(fused, 100);
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].start_seconds, Some(5.0));
        assert_eq!(output[1].start_seconds, Some(10.0));
        assert_eq!(output[0].score, 2.0 / 61.0);
    }
}
