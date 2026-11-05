//! Bounded partitions through the mature native KernelAdapter, never another kernel invocation path.
use crate::{AccountGrant, GrantError, before_work, fuse_ranks, projection_bytes};
use knowledge_retrieval::{
    Content, Control, Document, KernelAdapter, Mode, Query, ScopeView, SourceKey,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{Duration, Instant},
};
#[derive(Clone, Debug)]
pub struct AdmittedSource {
    pub path: String,
    pub title: String,
    pub media_type: String,
    pub source_sha256: String,
    pub text: String,
    pub tags: String,
    pub coverage: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalMode {
    Lexical,
    Exact,
    Regex,
}
#[derive(Clone, Debug)]
pub struct LexicalHit {
    pub path: String,
    pub snippet: String,
    pub score: f32,
    pub start: u64,
}
#[derive(Clone, Debug)]
pub struct LexicalOutcome {
    pub hits: Vec<LexicalHit>,
    pub complete: bool,
    pub searched_representations: usize,
    pub partitions: usize,
}
pub struct LexicalQuery<'a> {
    pub sources: &'a [AdmittedSource],
    pub eligible: &'a BTreeSet<String>,
    pub mode: LexicalMode,
    pub query: &'a str,
    pub limit: usize,
    pub grant: &'a AccountGrant,
    pub deadline: Option<Instant>,
}
fn control(deadline: Option<Instant>) -> Control {
    Control::new(deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(60)))
}
fn kernel(root: &Path, store: &Path) -> Result<KernelAdapter, LexicalError> {
    KernelAdapter::open(
        root,
        store,
        &format!(
            "knowledge-search:{}:{:x}",
            env!("CARGO_PKG_VERSION"),
            Sha256::digest(include_bytes!("lexical.rs"))
        ),
    )
    .map_err(kernel_error)
}
fn preflight(
    sources: &[AdmittedSource],
    grant: &AccountGrant,
    deadline: Option<Instant>,
    publish: bool,
) -> Result<(), LexicalError> {
    let memory = projection_bytes(sources)
        .checked_mul(16)
        .and_then(|n| n.checked_add(256 * 1024 * 1024))
        .ok_or_else(|| LexicalError::Kernel("inventory arithmetic overflow".into()))?;
    before_work(grant, memory, deadline).map_err(LexicalError::Grant)?;
    grant
        .covers(memory, if publish { 512 * 1024 * 1024 } else { 0 })
        .map_err(LexicalError::Grant)
}
fn views(
    library: Option<&str>,
    sources: &[AdmittedSource],
) -> Result<Vec<ScopeView>, LexicalError> {
    let mut out = vec![ScopeView {
        scope: format!("{}:part:0", library.unwrap_or("local")),
        documents: vec![],
        unavailable_objects: 0,
    }];
    let mut encoded = 0;
    for source in sources {
        if source.source_sha256.len() != 64
            || !source.source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(LexicalError::Kernel("source has no verified digest".into()));
        }
        let mut start = 0;
        loop {
            let mut end = (start + 512 * 1024).min(source.text.len());
            while !source.text.is_char_boundary(end) {
                end -= 1;
            }
            let text = &source.text[start..end];
            let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
            let document = Document {
                key: SourceKey {
                    namespace: "admitted-source".into(),
                    id: format!("{}:{start}", source.path),
                },
                revision: format!("{}:{digest}", source.source_sha256),
                digest,
                size: text.len() as u64,
                name: source.path.clone(),
                title: format!("{}\n{}", source.title, source.tags),
                media_type: source.media_type.clone(),
                content: Content::InlineText {
                    text: Some(text.into()),
                },
                text_ranges: None,
                language: None,
                metadata: BTreeMap::from([
                    ("source_path".into(), source.path.clone()),
                    ("source_sha256".into(), source.source_sha256.clone()),
                    ("base_offset".into(), start.to_string()),
                    ("coverage".into(), source.coverage.clone()),
                ]),
                locator: Some(source.path.clone()),
            };
            let size = serde_json::to_vec(&document)
                .map_err(|e| LexicalError::Kernel(e.to_string()))?
                .len();
            if !out.last().unwrap().documents.is_empty()
                && (encoded + size > 4 * 1024 * 1024 || out.last().unwrap().documents.len() >= 8000)
            {
                out.push(ScopeView {
                    scope: format!("{}:part:{}", library.unwrap_or("local"), out.len()),
                    documents: vec![],
                    unavailable_objects: 0,
                });
                encoded = 0;
            }
            encoded += size;
            out.last_mut().unwrap().documents.push(document);
            if end == source.text.len() {
                break;
            }
            start = end;
        }
    }
    Ok(out)
}
fn publish(
    kernel: &KernelAdapter,
    views: &[ScopeView],
    grant: &AccountGrant,
    control: &Control,
) -> Result<(), LexicalError> {
    for view in views {
        control.check().map_err(kernel_error)?;
        let expected = kernel.generation(&view.scope).map_err(kernel_error)?;
        grant
            .run(|work| {
                kernel
                    .publish(view.clone(), expected, work, control)
                    .map(|_| ())
                    .map_err(|e| GrantError::Kernel(e.to_string()))
            })
            .map_err(LexicalError::Grant)?;
    }
    Ok(())
}
pub fn publish_lexical(
    root: &Path,
    store: &Path,
    library: Option<&str>,
    sources: &[AdmittedSource],
    grant: &AccountGrant,
) -> Result<(), LexicalError> {
    preflight(sources, grant, None, true)?;
    let views = views(library, sources)?;
    publish(&kernel(root, store)?, &views, grant, &control(None))
}
pub fn query_lexical(
    root: &Path,
    store: &Path,
    library: Option<&str>,
    request: &LexicalQuery<'_>,
) -> Result<LexicalOutcome, LexicalError> {
    preflight(request.sources, request.grant, request.deadline, false)?;
    let views = views(library, request.sources)?;
    query(&kernel(root, store)?, views, request)
}
pub fn search_lexical(
    root: &Path,
    store: &Path,
    library: &str,
    request: &LexicalQuery<'_>,
) -> Result<LexicalOutcome, LexicalError> {
    preflight(request.sources, request.grant, request.deadline, true)?;
    let views = views(Some(library), request.sources)?;
    let kernel = kernel(root, store)?;
    publish(&kernel, &views, request.grant, &control(request.deadline))?;
    query(&kernel, views, request)
}
fn query(
    kernel: &KernelAdapter,
    views: Vec<ScopeView>,
    request: &LexicalQuery<'_>,
) -> Result<LexicalOutcome, LexicalError> {
    let mut outcome = LexicalOutcome {
        hits: vec![],
        complete: true,
        searched_representations: 0,
        partitions: views.len(),
    };
    let mut lists = vec![];
    let control = if request.mode == LexicalMode::Lexical {
        control(request.deadline)
    } else {
        control(request.deadline).body_scans()
    };
    for mut view in views {
        view.documents
            .retain(|d| request.eligible.contains(&d.name));
        if view.documents.is_empty() {
            continue;
        }
        let result = request
            .grant
            .run(|work| {
                kernel
                    .search(
                        Query {
                            mode: match request.mode {
                                LexicalMode::Lexical => Mode::Lexical,
                                LexicalMode::Exact => Mode::Exact,
                                LexicalMode::Regex => Mode::Regex,
                            },
                            query: request.query.into(),
                            scopes: vec![view],
                            limit: request.limit,
                        },
                        work,
                        &control,
                    )
                    .map_err(|e| GrantError::Kernel(e.to_string()))
            })
            .map_err(LexicalError::Grant)?;
        outcome.complete &= result.complete;
        outcome.searched_representations += result.coverage.searched_representations;
        lists.push(
            result
                .hits
                .into_iter()
                .map(|h| {
                    let start = match h.locator {
                        knowledge_retrieval::Locator::Text { start, .. }
                        | knowledge_retrieval::Locator::Title { start, .. }
                        | knowledge_retrieval::Locator::Metadata { start, .. } => start,
                    };
                    LexicalHit {
                        path: h.provenance["source_path"].clone(),
                        snippet: h.snippet,
                        score: h.score.unwrap_or(1.0),
                        start: start + h.provenance["base_offset"].parse::<u64>().unwrap_or(0),
                    }
                })
                .collect::<Vec<_>>(),
        );
    }
    if request.mode == LexicalMode::Lexical && lists.len() > 1 {
        let ranks = lists
            .iter()
            .map(|l| l.iter().map(|h| h.path.clone()).collect())
            .collect::<Vec<_>>();
        let mut by_path = BTreeMap::new();
        for h in lists.into_iter().flatten() {
            by_path.entry(h.path.clone()).or_insert(h);
        }
        outcome.hits = fuse_ranks(&ranks)
            .into_iter()
            .filter_map(|(p, s)| {
                by_path.remove(&p).map(|mut h| {
                    h.score = s;
                    h
                })
            })
            .take(request.limit)
            .collect();
    } else {
        outcome.hits = lists.into_iter().flatten().collect();
        outcome.hits.sort_by(|a, b| {
            if request.mode == LexicalMode::Lexical {
                b.score.total_cmp(&a.score).then(a.path.cmp(&b.path))
            } else {
                a.path.cmp(&b.path).then(a.start.cmp(&b.start))
            }
        });
        outcome.hits.truncate(request.limit);
    }
    Ok(outcome)
}
fn kernel_error(error: impl std::fmt::Display) -> LexicalError {
    LexicalError::Kernel(error.to_string())
}
#[derive(Debug)]
pub enum LexicalError {
    Grant(GrantError),
    Kernel(String),
}
impl std::fmt::Display for LexicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grant(e) => write!(f, "{e}"),
            Self::Kernel(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for LexicalError {}
