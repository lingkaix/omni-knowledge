//! Indexed lexical search, verified grep, and local vector rank fusion.
use crate::{FileRecord, KnowledgeBase, SearchHit, SearchMode, SearchOptions, Snapshot, embedding};
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use regex::RegexBuilder;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{IndexRecordOption, STORED, STRING, Schema, TEXT, TantivyDocument, Value};
use tantivy::{Index, IndexWriter, doc};

/// Adds separate Chinese characters and bigrams while preserving ordinary alphanumeric words.
fn tokens(text: &str) -> String {
    let mut output = Vec::new();
    let mut word = String::new();
    let mut previous = None;
    for character in text.chars().flat_map(char::to_lowercase) {
        let cjk = matches!(character as u32,0x3400..=0x9fff|0xf900..=0xfaff|0x20000..=0x2fa1f);
        if cjk {
            if !word.is_empty() {
                output.push(std::mem::take(&mut word));
            }
            output.push(character.to_string());
            if let Some(last) = previous {
                output.push(format!("{last}{character}"));
            }
            previous = Some(character);
        } else {
            previous = None;
            if character.is_alphanumeric() {
                word.push(character);
            } else if !word.is_empty() {
                output.push(std::mem::take(&mut word));
            }
        }
    }
    if !word.is_empty() {
        output.push(word);
    }
    output.join(" ")
}

/// Creates a new Tantivy index from this generation's records and commits its complete document set.
pub(crate) fn build_bm25(directory: &Path, records: &[FileRecord]) -> Result<()> {
    let mut builder = Schema::builder();
    let path = builder.add_text_field("path", STRING | STORED);
    let title = builder.add_text_field("title", TEXT);
    let body = builder.add_text_field("body", TEXT);
    let tags = builder.add_text_field("tags", STRING);
    let index = Index::create_in_dir(directory, builder.build())?;
    let mut writer: IndexWriter = index.writer_with_num_threads(2, 50_000_000)?;
    for record in records {
        let mut document = doc!(path=>record.path.clone(),title=>tokens(&format!("{} {}",record.title,record.path)),body=>tokens(&format!("{} {} {}",record.description,record.tags.join(" "),record.text)));
        for tag in &record.tags {
            document.add_text(tags, tag);
        }
        writer.add_document(document)?;
    }
    writer.commit()?;
    writer.wait_merging_threads()?;
    Ok(())
}

/// Constructs literal trigram postings; queries still verify every selected candidate.
pub(crate) fn build_trigrams(records: &[FileRecord]) -> BTreeMap<String, Vec<usize>> {
    let mut postings: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        let text = format!("{}\n{}\n{}", record.path, record.description, record.text);
        let chars: Vec<char> = text.chars().collect();
        let unique: HashSet<String> = chars.windows(3).map(|w| w.iter().collect()).collect();
        for trigram in unique {
            postings.entry(trigram).or_default().push(index);
        }
    }
    postings
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
        SearchMode::Grep => grep(kb, snapshot, options),
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

/// Requires all selected exact tags, without prefix or case folding.
fn tagged(record: &FileRecord, options: &SearchOptions) -> bool {
    options.tags.iter().all(|tag| record.tags.contains(tag))
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
    Ok(snapshot
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
        .map(|r| hit(kb, r, "files", 1.0, r.description.clone(), None))
        .collect())
}

/// Searches BM25 with exact tag terms included before result limiting.
fn bm25(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    directory: &Path,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let projected = tokens(&options.query);
    if projected.is_empty() {
        return Ok(vec![]);
    }
    let index = Index::open_in_dir(directory.join("tantivy"))?;
    let schema = index.schema();
    let title = schema.get_field("title")?;
    let body = schema.get_field("body")?;
    let tags = schema.get_field("tags")?;
    let path = schema.get_field("path")?;
    let mut parser = QueryParser::for_index(&index, vec![title, body]);
    parser.set_field_boost(title, 2.0);
    let parsed = parser.parse_query(&projected)?;
    let mut parts: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Must, parsed)];
    for tag in &options.tags {
        parts.push((
            Occur::Must,
            Box::new(TermQuery::new(
                tantivy::Term::from_field_text(tags, tag),
                IndexRecordOption::Basic,
            )),
        ));
    }
    let query = BooleanQuery::new(parts);
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let top = searcher.search(&query, &TopDocs::with_limit(options.limit).order_by_score())?;
    let lookup: HashMap<_, _> = snapshot
        .records
        .iter()
        .map(|r| (r.path.as_str(), r))
        .collect();
    let mut result = vec![];
    for (score, address) in top {
        let document: TantivyDocument = searcher.doc(address)?;
        let source = document
            .get_first(path)
            .and_then(|value| value.as_str())
            .context("indexed source path missing")?;
        let record = lookup.get(source).context("index and manifest disagree")?;
        let (snippet, line) = lexical_snippet(record, &options.query);
        result.push(hit(kb, record, "bm25", score, snippet, line));
    }
    Ok(result)
}

/// Finds a useful lexical source line, then falls back to description or start of content.
fn lexical_snippet(record: &FileRecord, query: &str) -> (String, Option<usize>) {
    let query = query.to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    for (line, text) in record.text.lines().enumerate() {
        let lower = text.to_lowercase();
        if lower.contains(&query) || words.iter().any(|word| lower.contains(word)) {
            return (text.into(), Some(line + 1));
        }
    }
    if !record.description.is_empty() {
        (record.description.clone(), None)
    } else {
        (record.text.chars().take(500).collect(), Some(1))
    }
}

/// Verifies regex matches; only literal case-sensitive patterns use conservative trigram candidates.
fn grep(
    kb: &KnowledgeBase,
    snapshot: &Snapshot,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let regex = RegexBuilder::new(&options.query)
        .case_insensitive(!options.case_sensitive)
        .size_limit(10 * 1024 * 1024)
        .build()
        .context("invalid grep regular expression")?;
    let literal =
        options.case_sensitive && !options.query.chars().any(|c| ".[](){}*+?^$|\\".contains(c));
    let mut candidates: Option<BTreeSet<usize>> = None;
    if literal {
        let chars: Vec<char> = options.query.chars().collect();
        for window in chars.windows(3) {
            let gram: String = window.iter().collect();
            let postings: BTreeSet<_> = snapshot
                .trigrams
                .get(&gram)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();
            candidates = Some(match candidates {
                None => postings,
                Some(previous) => previous.intersection(&postings).copied().collect(),
            });
        }
    }
    let mut hits: Vec<_> = snapshot
        .records
        .par_iter()
        .enumerate()
        .filter(|(i, r)| {
            tagged(r, options) && candidates.as_ref().is_none_or(|set| set.contains(i))
        })
        .flat_map_iter(|(_, record)| {
            let mut result = vec![];
            for (line, text) in record.text.lines().enumerate() {
                if regex.is_match(text) {
                    result.push(hit(kb, record, "grep", 1.0, text.into(), Some(line + 1)));
                }
                if result.len() >= options.limit {
                    break;
                }
            }
            if result.is_empty()
                && regex.is_match(&format!("{}\n{}", record.path, record.description))
            {
                result.push(hit(
                    kb,
                    record,
                    "grep",
                    1.0,
                    record.description.clone(),
                    None,
                ));
            }
            result.into_iter()
        })
        .collect();
    hits.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    hits.truncate(options.limit);
    Ok(hits)
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
    client.health()?;
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
