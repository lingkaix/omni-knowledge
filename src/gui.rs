//! Native GPUI controls with a confined WKWebView document preview on macOS.

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs::File,
    io::{Read, Seek, SeekFrom},
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow, bail};
use gpui::{
    AnyWindowHandle, App, Application, Bounds, ClipboardItem, Context, Entity, ExternalPaths,
    FocusHandle, Focusable, KeyBinding, Menu, MenuItem, MouseButton, PathPromptOptions,
    SystemMenuType, Window, WindowBounds, WindowOptions, actions, canvas, div, img, prelude::*, px,
    rgb, size,
};
use gpui_component::{
    Disableable, Icon, IconName, Root, Sizable, Theme, ThemeMode, WindowExt,
    button::{Button, ButtonVariant, ButtonVariants},
    checkbox::Checkbox,
    dialog::DialogButtonProps,
    input::{Input, InputEvent, InputState},
    list::ListItem,
    menu::{PopupMenu, PopupMenuItem},
    popover::Popover,
    tree::{TreeItem, TreeState, tree},
};
use wry::{
    WebView, WebViewBuilder, WebViewExtMacOS,
    dpi::{LogicalPosition, LogicalSize},
    http::{Request, Response},
};

use crate::file_actions::{self, DirectoryEntry};
use crate::runtime::{self, BrewPackage, InstallLaunch, RuntimeStatus};
use crate::status::{FolderState, FolderStatus, ServiceState, StatusProbe, StatusReport};
use crate::{IndexOptions, IndexReport, KnowledgeBase, SearchHit, SearchMode, SearchOptions};

/// Shared visible border and pointer-target height for native inputs and toolbar buttons.
const CONTROL_HEIGHT: f32 = 28.;

/// Maximum bytes read for a single preview resource or text extraction.
const PREVIEW_LIMIT: usize = 32 * 1024 * 1024;

/// Period between independent, metadata-only environment and index observations.
const STATUS_INTERVAL: Duration = Duration::from_secs(12);

/// Available retrieval choices in the visible and keyboard menu order.
const SEARCH_MODES: [(SearchMode, &str); 5] = [
    (SearchMode::Bm25, "Full text"),
    (SearchMode::Grep, "Text pattern"),
    (SearchMode::Semantic, "Meaning"),
    (SearchMode::Hybrid, "Text + meaning"),
    (SearchMode::Files, "File names & notes"),
];

/// Trusted preview initialization; document source cannot supply executable scripts.
const RENDERER_JS: &str = r#"
document.querySelectorAll('pre > code').forEach(code => {
  if (code.classList.contains('language-mermaid')) {
    const diagram = document.createElement('div');
    diagram.className = 'mermaid'; diagram.textContent = code.textContent;
    code.parentNode.replaceWith(diagram);
  } else { hljs.highlightElement(code); }
});
mermaid.initialize({startOnLoad:false,securityLevel:'strict',theme:'neutral',maxTextSize:100000});
/** Render only visible diagrams, then retain the selected source line after their layout settles. */
function renderDiagrams() {
  const nodes = Array.from(document.querySelectorAll('.mermaid:not([data-processed])')).filter(node => node.getClientRects().length);
  return nodes.length ? mermaid.run({nodes,suppressErrors:true}).catch(() => {}) : Promise.resolve();
}
/** Reveal all containing disclosures and position the exact indexed text row. */
function scrollEvidence() {
  const target = document.querySelector('[data-preview-target="true"]');
  if (!target) return;
  for (let parent = target.parentElement; parent; parent = parent.parentElement) {
    if (parent.tagName === 'DETAILS') parent.open = true;
  }
  target.scrollIntoView({block:'center'});
}
renderDiagrams().then(() => { if (!window.location.hash) scrollEvidence(); });
window.addEventListener('load', () => { if (!window.location.hash) scrollEvidence(); }, {once:true});
document.querySelectorAll('details.line-preview').forEach(detail => detail.addEventListener('toggle', () => {
  if (!detail.open) renderDiagrams();
}));
const ids = new Set();
for (const heading of document.querySelectorAll('h1,h2,h3,h4,h5,h6')) {
  const base = heading.id || heading.textContent.trim().toLowerCase().replace(/[^\p{L}\p{N} _-]/gu, '').replace(/\s+/g, '-') || 'section';
  let id = base, suffix = 0;
  while (ids.has(id)) id = `${base}-${++suffix}`;
  ids.add(id); heading.id = id;
}
function scrollAnchor(fragment) {
  try { const id = decodeURIComponent(fragment.replace(/^#/, '')); const target = document.getElementById(id); if (target) target.scrollIntoView({block:'start'}); } catch (_) {}
}
scrollAnchor(window.location.hash);
document.addEventListener('click', event => {
  const anchor = event.target.closest('a');
  if (!anchor) return;
  event.preventDefault();
  if (!event.isTrusted) return;
  const raw = anchor.getAttribute('href') || '';
  let target;
  try { target = new URL(raw, document.baseURI); } catch (_) { return; }
  if (!['omni:', 'https:', 'http:'].includes(target.protocol) || target.username || target.password) return;
  const current = new URL(document.baseURI);
  if (target.protocol === 'omni:' && target.host !== current.host) return;
  if (target.origin === current.origin && target.pathname === current.pathname && target.hash) { scrollAnchor(target.hash); return; }
  window.location.href = 'omni://nav/open?target=' + encodeURIComponent(target.href) + '&raw=' + encodeURIComponent(raw) + '&issuer=' + encodeURIComponent(current.host);
});
"#;

actions!(
    omni_gui,
    [
        Quit, CopyFile, PasteFiles, FileUp, FileDown, FileLeft, FileRight, FileOpen
    ]
);

/// Render single-line inputs with the same physical border height as adjacent buttons.
/// Input::h controls multi-line fields only; Styled refinement overrides the built-in rem height.
fn input(state: &Entity<InputState>) -> Input {
    let mut input = Input::new(state).small().text_size(px(14.));
    input.style().size.height = Some(px(CONTROL_HEIGHT).into());
    input
}

/// Shared read-only state for the local preview resource protocol.
#[derive(Clone, Default)]
struct PreviewState {
    root: Option<PathBuf>,
    issuer: String,
    relative: Option<String>,
    body: Arc<str>,
}

/// Files displayed per page; evidence is grouped before applying this presentation bound.
const RESULT_PAGE_SIZE: usize = 50;

/// One source with its best returned evidence first and every additional occurrence retained.
struct RankedFile {
    path: String,
    indices: Vec<usize>,
}

/// Group occurrences in their returned order without introducing directory or filename sorting.
fn ranked_files(hits: &[SearchHit]) -> Vec<RankedFile> {
    let mut positions = HashMap::new();
    let mut files: Vec<RankedFile> = Vec::new();
    for (index, hit) in hits.iter().enumerate() {
        let position = *positions.entry(hit.path.as_str()).or_insert_with(|| {
            files.push(RankedFile {
                path: hit.path.clone(),
                indices: Vec::new(),
            });
            files.len() - 1
        });
        files[position].indices.push(index);
    }
    files
}

/// Describe returned file candidates separately from evidence fragments and unknown corpus totals.
fn search_result_summary(hits: &[SearchHit], mode: SearchMode, shown: usize) -> String {
    let files = ranked_files(hits).len();
    let shown = shown.min(files);
    let noun = if matches!(mode, SearchMode::Semantic | SearchMode::Hybrid) {
        "candidate files"
    } else {
        "files"
    };
    let bound = if hits.len() == 10_000 {
        " · result limit reached"
    } else {
        ""
    };
    format!(
        "{shown} of {files} {noun} · {} evidence fragments{bound}",
        hits.len()
    )
}

/// Describe an evidence location without treating similarity as a confidence percentage.
fn evidence_label(hit: &SearchHit) -> String {
    hit.line
        .map(|line| format!("Line {line}"))
        .or_else(|| hit.start_seconds.map(|time| format!("At {time:.1}s")))
        .unwrap_or_else(|| "File evidence".into())
}

/// Clear retained expansion identities only when the canonical library root changes.
fn reset_tree_on_root_change(
    previous: Option<&Path>,
    next: &Path,
    collapsed: &mut BTreeSet<String>,
) {
    if previous != Some(next) {
        collapsed.clear();
    }
}

/// Validate a captured transfer destination before any clipboard read or filesystem write.
fn transfer_target_current(actual: Option<&Path>, captured: &Path) -> bool {
    actual == Some(captured)
}

/// Reject an old status callback after the explicitly selected folder changes.
fn status_target_matches(requested: &Path, selected: Option<&Path>) -> bool {
    requested == selected.unwrap_or_else(|| Path::new(""))
}

/// Attach transient help to a stable native control identity instead of its changing diagnostic text.
#[derive(Default)]
struct HoverHint {
    active: Option<(String, String)>,
}

impl HoverHint {
    /// Replace help on entry; an exit can clear only the control that still owns the visible hint.
    fn update(&mut self, owner: &str, hovered: bool, detail: &str) {
        if hovered {
            self.active = Some((owner.to_owned(), detail.to_owned()));
        } else if self
            .active
            .as_ref()
            .is_some_and(|(active, _)| active == owner)
        {
            self.clear();
        }
    }

    /// Return the current full diagnostic for layout, without exposing ownership to rendering code.
    fn detail(&self) -> Option<&str> {
        self.active.as_ref().map(|(_, detail)| detail.as_str())
    }

    /// Remove help when navigation replaces the relevant controls.
    fn clear(&mut self) {
        self.active = None;
    }
}

/// Separate explicit indexing of an opened root from checking or managing an unopened draft.
#[derive(Clone, Debug, PartialEq, Eq)]
enum FooterIndexAction {
    Index(PathBuf),
    Check,
    Library,
}

/// Permit publication only for a classified root that is also the actual opened library.
/// Missing observations never fall back to another open library, including explicit refresh clicks.
fn footer_index_action(
    opened: Option<&Path>,
    folder: Option<&FolderStatus>,
    explicit_refresh: bool,
) -> FooterIndexAction {
    let Some(folder) = folder else {
        return FooterIndexAction::Check;
    };
    let Some(opened) = opened.filter(|opened| folder.root.as_deref() == Some(*opened)) else {
        return FooterIndexAction::Library;
    };
    match folder.state {
        FolderState::Stale | FolderState::NeedsIndex => {
            FooterIndexAction::Index(opened.to_path_buf())
        }
        FolderState::Current | FolderState::Checking if explicit_refresh => {
            FooterIndexAction::Index(opened.to_path_buf())
        }
        FolderState::Current | FolderState::Checking => FooterIndexAction::Check,
        _ => FooterIndexAction::Library,
    }
}

/// Immutable source identity for native actions, drag payloads, and confirmations.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceTarget {
    root: PathBuf,
    relative: String,
}

/// A tree node keeps its filesystem identity and optional ranked occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TreeTarget {
    relative: String,
    directory: bool,
    nested: bool,
    hit_index: Option<usize>,
}

/// Keep dismissal events attached to the menu that issued them after a right-click retarget.
#[derive(Default)]
struct MenuSession {
    revision: u64,
    active: Option<u64>,
}

impl MenuSession {
    /// Replace the current menu and return its callback identity.
    fn open(&mut self) -> u64 {
        self.revision += 1;
        self.active = Some(self.revision);
        self.revision
    }

    /// Accept an outside-click or Escape only for the menu that is still displayed.
    fn dismiss(&mut self, revision: u64) -> bool {
        if self.active != Some(revision) {
            return false;
        }
        self.active = None;
        true
    }
}

/// Observe this preview's native view hierarchy without changing focus or reading source content.
/// Foreground-only AppKit access also covers WebKit-owned PDF/media descendants; an unmounted view is inert.
fn preview_has_native_focus(preview: &WebView) -> bool {
    let webview = preview.webview();
    let Some(window) = webview.window() else {
        return false;
    };
    let Some(responder) = window.firstResponder() else {
        return false;
    };
    let preview_view: &objc2_app_kit::NSView = &webview;
    responder
        .downcast_ref::<objc2_app_kit::NSView>()
        .is_some_and(|view| std::ptr::eq(view, preview_view) || view.isDescendantOf(preview_view))
}

/// Read-side navigation accepted from a trusted renderer click.
#[derive(Clone, Debug)]
struct LinkEvent {
    root: PathBuf,
    relative: String,
    url: String,
    raw: String,
    issuer: String,
}

/// Build a lazy filesystem tree with uncapped immediate children and explicit child roots.
fn browse_items(
    cache: &BTreeMap<String, Vec<DirectoryEntry>>,
    folder: &str,
    expanded: &BTreeSet<String>,
    targets: &mut BTreeMap<String, TreeTarget>,
) -> Vec<TreeItem> {
    cache
        .get(folder)
        .into_iter()
        .flatten()
        .map(|entry| {
            let id = format!("source:{}", entry.relative);
            targets.insert(
                id.clone(),
                TreeTarget {
                    relative: entry.relative.clone(),
                    directory: entry.is_directory,
                    nested: entry.is_nested_root,
                    hit_index: None,
                },
            );
            let label = Path::new(&entry.relative)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let mut item = TreeItem::new(id.clone(), label);
            if entry.is_directory && !entry.is_nested_root {
                let children = if cache.contains_key(&entry.relative) {
                    browse_items(cache, &entry.relative, expanded, targets)
                } else {
                    vec![
                        TreeItem::new(format!("loading:{}", entry.relative), "Expand to load")
                            .disabled(true),
                    ]
                };
                item = item
                    .children(children)
                    .expanded(expanded.contains(&entry.relative));
            }
            item
        })
        .collect()
}

/// Build ranked file rows, with additional occurrences collapsed beneath their source.
fn match_items(
    hits: &[SearchHit],
    expanded: &BTreeSet<String>,
    limit: usize,
    targets: &mut BTreeMap<String, TreeTarget>,
) -> Vec<TreeItem> {
    ranked_files(hits)
        .into_iter()
        .enumerate()
        .take(limit)
        .map(|(rank, file)| {
            let mut occurrences = file.indices.iter().map(|&index| {
                let hit = &hits[index];
                let id = format!("match:{index}:{}", hit.path);
                targets.insert(
                    id.clone(),
                    TreeTarget {
                        relative: hit.path.clone(),
                        directory: false,
                        nested: false,
                        hit_index: Some(index),
                    },
                );
                TreeItem::new(
                    id,
                    format!(
                        "{} · {}",
                        evidence_label(hit),
                        hit.snippet
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .chars()
                            .take(90)
                            .collect::<String>()
                    ),
                )
            });
            let best_index = file.indices[0];
            let best = &hits[best_index];
            let mut parent = occurrences.next().expect("ranked files contain evidence");
            let location = if best.line.is_some() || best.start_seconds.is_some() {
                format!(" · {}", evidence_label(best))
            } else {
                String::new()
            };
            let other = if file.indices.len() > 1 {
                format!(" · {} more", file.indices.len() - 1)
            } else {
                String::new()
            };
            parent.label = format!("#{} {}{location}{other}", rank + 1, file.path).into();
            parent
                .children(occurrences)
                .expanded(expanded.contains(&file.path))
        })
        .collect()
}

/// Visit retained items to persist expansion state after the component changes it.
fn visit_items(items: &[TreeItem], visitor: &mut impl FnMut(&TreeItem)) {
    for item in items {
        visitor(item);
        visit_items(&item.children, visitor);
    }
}

/// Locate a retained selected identity among expanded rows after inserts, removals, or refresh.
fn restored_tree_selection(items: &[TreeItem], selected: Option<&str>) -> Option<usize> {
    /// Search only visible rows while maintaining the component's flattened row index.
    fn locate(items: &[TreeItem], selected: &str, index: &mut usize) -> Option<usize> {
        for item in items {
            let current = *index;
            *index += 1;
            if item.id.as_ref() == selected {
                return Some(current);
            }
            if item.is_expanded()
                && let Some(found) = locate(&item.children, selected, index)
            {
                return Some(found);
            }
        }
        None
    }
    selected.and_then(|selected| locate(items, selected, &mut 0))
}

/// Select the exact right-clicked occurrence, including a duplicate match in the same file.
fn context_tree_selection(
    items: &[TreeItem],
    targets: &BTreeMap<String, TreeTarget>,
    target: &TreeTarget,
) -> Option<usize> {
    let selected = targets
        .iter()
        .find_map(|(identity, item)| (item == target).then_some(identity.as_str()));
    restored_tree_selection(items, selected)
}

/// A lightweight native drag image with no filesystem side effect.
struct FileDrag(String);

impl Render for FileDrag {
    /// Show the selected source name while the native drag is active.
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .rounded_md()
            .bg(rgb(0xe2ebdf))
            .child(self.0.clone())
    }
}

/// Bound a read lane to one active worker and one replacement intent.
struct CoalescedRead<T> {
    running: bool,
    pending: Option<T>,
}

impl<T> Default for CoalescedRead<T> {
    /// Start with no active worker or queued intent, independent of the payload's default.
    fn default() -> Self {
        Self {
            running: false,
            pending: None,
        }
    }
}

impl<T> CoalescedRead<T> {
    /// Replace the queued intent without interrupting the one active filesystem operation.
    fn enqueue(&mut self, request: T) {
        self.pending = Some(request);
    }
    /// Dispatch only after the previous worker completes; intermediate queued intents disappear.
    fn dispatch(&mut self) -> Option<T> {
        if self.running {
            return None;
        }
        let request = self.pending.take()?;
        self.running = true;
        Some(request)
    }
    /// Release the active slot before testing whether the callback is still current.
    fn complete(&mut self) {
        self.running = false;
    }
    /// Cancel undispatched work while a stale active callback finishes harmlessly.
    fn clear(&mut self) {
        self.pending = None;
    }
}

/// Source coordinates are independent of the native selection and the document's rendered layout.
#[derive(Clone, Default)]
struct PreviewLocation {
    start_seconds: Option<f64>,
    line: Option<usize>,
    anchor: Option<String>,
}

/// One immutable preview intent; only the latest root and request may update the window.
#[derive(Clone)]
struct PreviewRequest {
    root: PathBuf,
    relative: String,
    location: PreviewLocation,
    show_metadata: bool,
    revision: u64,
    metadata_revision: u64,
    issuer: String,
}

/// User-selected query restrictions captured separately from editable search controls.
/// Normalized tags compare meaning rather than comma spacing or order.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SearchFilter {
    query: String,
    mode: SearchMode,
    tags: Vec<String>,
}

impl SearchFilter {
    /// Capture query text and selected mode while normalizing exact-tag set equality.
    fn new(query: String, mode: SearchMode, mut tags: Vec<String>) -> Self {
        tags.sort();
        tags.dedup();
        Self { query, mode, tags }
    }

    /// Distinguish an actual tree restriction from the next query's mode setting.
    fn restricts_files(&self) -> bool {
        !self.query.trim().is_empty() || !self.tags.is_empty()
    }

    /// Empty text uses exact-tag file browsing even when a semantic mode remains selected.
    fn effective_mode(&self) -> SearchMode {
        if self.query.trim().is_empty() {
            SearchMode::Files
        } else {
            self.mode
        }
    }
}

/// One immutable indexed query, coalesced while an earlier search finishes.
struct SearchRequest {
    root: PathBuf,
    filter: SearchFilter,
    options: SearchOptions,
    revision: u64,
}

/// Keep local index reads independent of a slower embedding provider request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SearchLane {
    Local,
    Model,
}

impl SearchLane {
    /// Route the effective mode, including tag-only queries already converted to Files.
    fn for_mode(mode: SearchMode) -> Self {
        match mode {
            SearchMode::Semantic | SearchMode::Hybrid => Self::Model,
            SearchMode::Files | SearchMode::Grep | SearchMode::Bm25 => Self::Local,
        }
    }
}

/// Bound searches to two active workers and one latest queued intent across both lanes.
#[derive(Default)]
struct SearchLanes {
    local: CoalescedRead<SearchRequest>,
    model: CoalescedRead<SearchRequest>,
    current_revision: Option<u64>,
}

impl SearchLanes {
    /// Select one worker slot without changing the other lane's active request.
    fn lane(&mut self, lane: SearchLane) -> &mut CoalescedRead<SearchRequest> {
        match lane {
            SearchLane::Local => &mut self.local,
            SearchLane::Model => &mut self.model,
        }
    }

    /// Replace all undispatched work and return the lane for the newest user intent.
    fn enqueue(&mut self, request: SearchRequest) -> SearchLane {
        self.clear();
        let lane = SearchLane::for_mode(request.options.mode);
        self.current_revision = Some(request.revision);
        self.lane(lane).enqueue(request);
        lane
    }

    /// Dispatch only when this lane's previous blocking operation has finished.
    fn dispatch(&mut self, lane: SearchLane) -> Option<SearchRequest> {
        self.lane(lane).dispatch()
    }

    /// Release this worker without letting a stale completion clear the newest progress state.
    fn complete(&mut self, lane: SearchLane, revision: u64) {
        self.lane(lane).complete();
        if self.current_revision == Some(revision) {
            self.current_revision = None;
        }
    }

    /// Drop queued intent; active blocking operations retain their bounded slots until completion.
    fn clear(&mut self) {
        self.local.clear();
        self.model.clear();
        self.current_revision = None;
    }

    /// Show progress for the current intent, excluding obsolete workers finishing in the background.
    fn is_searching(&self) -> bool {
        self.current_revision.is_some()
    }
}

/// Compare callback identities independently of unrelated mutation or indexing activity.
fn current_read_request(
    current_root: Option<&Path>,
    root: &Path,
    current_revision: u64,
    revision: u64,
) -> bool {
    current_root == Some(root) && current_revision == revision
}

/// Refresh clean same-source values only if neither editing nor a completed save superseded the read.
fn refresh_authored_metadata(
    new_source: bool,
    dirty: bool,
    read_revision: u64,
    current_revision: u64,
) -> bool {
    new_source || (!dirty && read_revision == current_revision)
}

/// Show the in-field reset affordance for typed, tag-only, and already-published searches.
fn search_filter_is_clearable(query: &str, tags: &[String], published_filter: bool) -> bool {
    !query.is_empty() || !tags.is_empty() || published_filter
}

/// Keep optional restrictions discoverable when their disclosure is closed.
fn search_options_label(mode: SearchMode, tag_count: usize) -> String {
    let mut label = "Search options".to_owned();
    if mode != SearchMode::Bm25
        && let Some((_, name)) = SEARCH_MODES
            .iter()
            .find(|(candidate, _)| *candidate == mode)
    {
        label.push_str(" · ");
        label.push_str(name);
    }
    if tag_count > 0 {
        label.push_str(&format!(
            " · {tag_count} {}",
            if tag_count == 1 { "tag" } else { "tags" }
        ));
    }
    label
}

/// Summarize the displayed results, not unapplied edits, and explain their exact restrictions on hover.
/// With no results filter, a nondefault mode configures the next query without implying filtered files.
fn search_options_presentation(
    draft: &SearchFilter,
    applied: Option<&SearchFilter>,
) -> (String, String) {
    let showing = applied.unwrap_or(draft);
    let mode = if applied.is_some() && showing.query.trim().is_empty() {
        SearchMode::Bm25
    } else {
        showing.mode
    };
    let mut label = search_options_label(mode, showing.tags.len());
    let pending = applied.map_or_else(|| draft.restricts_files(), |applied| applied != draft);
    if pending {
        label.push_str(" · changes not applied");
    }
    let mut hint = if let Some(applied) = applied {
        let selected_mode = SEARCH_MODES
            .iter()
            .find(|(mode, _)| *mode == applied.mode)
            .map(|(_, label)| *label)
            .unwrap_or("Full text");
        format!(
            "Showing results for: {}\nSearch mode: {}\nRequired tags: {}",
            if applied.query.trim().is_empty() {
                "all indexed files"
            } else {
                &applied.query
            },
            if applied.query.trim().is_empty() {
                "Tags only"
            } else {
                selected_mode
            },
            if applied.tags.is_empty() {
                "None".into()
            } else {
                applied.tags.join(", ")
            }
        )
    } else {
        "Showing all files in the library.".into()
    };
    if pending {
        hint.push_str("\nChanges are not applied. Select Search or Apply to update the results.");
    } else {
        hint.push_str("\nChoose a search mode or exact tags in Search options.");
    }
    (label, hint)
}

/// Keep the compact footer label on one physical line while callers retain the full hover diagnostic.
fn single_line_status_label(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Present decimal file units like Finder while retaining the precise byte count for small files.
fn human_file_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["bytes", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1000 {
        return format!("{bytes} {}", if bytes == 1 { "byte" } else { "bytes" });
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000. && unit + 1 < UNITS.len() {
        value /= 1000.;
        unit += 1;
    }
    let digits = bytes.to_string();
    let exact = digits
        .chars()
        .enumerate()
        .fold(String::new(), |mut formatted, (index, digit)| {
            if index > 0 && (digits.len() - index).is_multiple_of(3) {
                formatted.push(',');
            }
            formatted.push(digit);
            formatted
        });
    format!("{value:.1} {} ({exact} bytes)", UNITS[unit])
}

/// Describe common source formats with familiar names; unknown extensions remain honest file labels.
fn human_file_type(relative: &str, mime: &str) -> String {
    let extension = Path::new(relative)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let name = match extension.to_ascii_lowercase().as_str() {
        "md" | "markdown" => "Markdown",
        "txt" | "text" => "Plain text",
        "html" | "htm" => "HTML document",
        "pdf" => "PDF document",
        "doc" | "docx" | "odt" => "Text document",
        "xls" | "xlsx" | "ods" | "csv" => "Spreadsheet",
        "ppt" | "pptx" | "odp" => "Presentation",
        "epub" => "EPUB book",
        "jpg" | "jpeg" => "JPEG image",
        "png" => "PNG image",
        "gif" => "GIF image",
        "webp" => "WebP image",
        "svg" => "SVG image",
        "heic" | "heif" => "HEIF image",
        "mp3" => "MP3 audio",
        "wav" => "WAV audio",
        "m4a" => "M4A audio",
        "mp4" | "m4v" => "MP4 video",
        "mov" => "QuickTime video",
        "webm" => "WebM video",
        "json" => "JSON data",
        "yaml" | "yml" => "YAML data",
        "zip" => "ZIP archive",
        _ => {
            if mime.starts_with("image/") {
                "Image"
            } else if mime.starts_with("video/") {
                "Video"
            } else if mime.starts_with("audio/") {
                "Audio"
            } else if mime.starts_with("text/") {
                "Text file"
            } else {
                ""
            }
        }
    };
    if name.is_empty() {
        if extension.is_empty() {
            "File".into()
        } else {
            format!("{} file", extension.to_uppercase())
        }
    } else if extension.is_empty() {
        name.into()
    } else {
        format!("{name} (.{extension})")
    }
}

/// Convert an observed filesystem timestamp to local calendar time without inventing missing dates.
/// `localtime_r` writes caller-owned storage and handles the offset at that date, including DST.
fn local_file_date(milliseconds: Option<u64>) -> String {
    let Some(seconds) = milliseconds.and_then(|value| libc::time_t::try_from(value / 1000).ok())
    else {
        return "Unavailable".into();
    };
    let mut calendar = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both arguments point to valid storage; localtime_r initializes tm on a nonnull return.
    let result = unsafe { libc::localtime_r(&seconds, calendar.as_mut_ptr()) };
    if result.is_null() {
        return "Unavailable".into();
    }
    // SAFETY: the successful localtime_r call initialized every tm field.
    let calendar = unsafe { calendar.assume_init() };
    let offset = calendar.tm_gmtoff;
    let sign = if offset < 0 { '-' } else { '+' };
    let offset = offset.unsigned_abs();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} (UTC{sign}{:02}:{:02})",
        calendar.tm_year + 1900,
        calendar.tm_mon + 1,
        calendar.tm_mday,
        calendar.tm_hour,
        calendar.tm_min,
        offset / 3600,
        offset % 3600 / 60
    )
}

/// Recompute draft feedback for every edit, including undo back to the saved baseline.
fn metadata_edit_feedback(dirty: bool, saving: bool, previously_saved: bool) -> &'static str {
    if saving && dirty {
        "Saving earlier changes · current edits are not saved"
    } else if saving {
        "Saving tags and notes…"
    } else if dirty {
        "Unsaved changes"
    } else if previously_saved {
        "Saved in .omni/metadata/"
    } else {
        "No saved tags or notes yet"
    }
}

/// Human-readable filesystem observations prepared by the existing background preview worker.
#[derive(Default)]
struct FilePropertyLabels {
    file_type: String,
    size: String,
    created: String,
    modified: String,
}

/// Values last loaded from or successfully committed to one authored sidecar.
#[derive(Clone)]
struct MetadataDraft {
    target: SourceTarget,
    tags: Vec<String>,
    description: String,
}

/// Compare authored tag meaning without treating comma spacing or duplicates as unsaved changes.
fn authored_tags(value: &str) -> Vec<String> {
    let mut tags = parse_tags(value);
    tags.sort();
    tags.dedup();
    tags
}

/// Commit one authored sidecar only; derived indexes are deliberately refreshed by a separate action.
fn save_authored_metadata(
    target: &SourceTarget,
    tags: &[String],
    description: &str,
) -> Result<PathBuf> {
    let kb = file_actions::open_exact_root(&target.root)?;
    let path = kb.metadata_path(&target.relative)?;
    kb.set_metadata(&target.relative, tags, Some(description))?;
    Ok(path)
}

/// Compare only directory identities used by the tree, without refreshing unchanged component items.
fn same_directory_entries(previous: &[DirectoryEntry], next: &[DirectoryEntry]) -> bool {
    previous.len() == next.len()
        && previous.iter().zip(next).all(|(a, b)| {
            a.relative == b.relative
                && a.is_directory == b.is_directory
                && a.is_nested_root == b.is_nested_root
        })
}

/// Native surface updates are applied once per actual geometry or visibility change.
#[derive(Default)]
struct PreviewSurface {
    bounds: Option<[f64; 4]>,
    visible: Option<bool>,
}

/// Apply a physical native rectangle only when it differs from the last successful update.
fn update_preview_bounds(
    webview: &WebView,
    surface: &RefCell<PreviewSurface>,
    bounds: Bounds<gpui::Pixels>,
) -> Result<()> {
    let rect = [
        f64::from(bounds.origin.x),
        f64::from(bounds.origin.y),
        f64::from(bounds.size.width.max(px(1.))),
        f64::from(bounds.size.height.max(px(1.))),
    ];
    if surface.borrow().bounds == Some(rect) {
        return Ok(());
    }
    webview.set_bounds(wry::Rect {
        position: LogicalPosition::new(rect[0], rect[1]).into(),
        size: LogicalSize::new(rect[2], rect[3]).into(),
    })?;
    surface.borrow_mut().bounds = Some(rect);
    Ok(())
}

/// Completed work from a background thread; no GPUI handles cross this boundary.
enum JobResult {
    Library {
        root: PathBuf,
        name: String,
        hits: Vec<SearchHit>,
        message: String,
        initialize_index: bool,
    },
    FileAction {
        root: PathBuf,
        removed: Option<String>,
        changed: bool,
        message: String,
    },
    Preview {
        relative: String,
        body: Arc<str>,
        info: Box<crate::file_info::SourceInfo>,
        properties: FilePropertyLabels,
        anchor: Option<String>,
        show_metadata: bool,
    },
    FolderNavigation {
        relative: String,
        entries: Vec<DirectoryEntry>,
    },
    Runtime(RuntimeStatus),
    Installation(InstallLaunch),
    InstallationError(String),
}

/// Main window state, native controls, current result list, and embedded preview.
struct LibraryWindow {
    window: AnyWindowHandle,
    root_input: Entity<InputState>,
    query_input: Entity<InputState>,
    tags_input: Entity<InputState>,
    edit_tags_input: Entity<InputState>,
    description_input: Entity<InputState>,
    root: Option<PathBuf>,
    library_name: String,
    hits: Rc<Vec<SearchHit>>,
    selected: Option<String>,
    mode: SearchMode,
    search_options_open: bool,
    mode_menu_open: bool,
    mode_popup: Option<Entity<PopupMenu>>,
    embeddings: bool,
    busy: bool,
    status: String,
    preview: Rc<WebView>,
    preview_state: Arc<RwLock<PreviewState>>,
    generation: u64,
    logo: Arc<gpui::Image>,
    guide_open: bool,
    guide_anchor: Option<&'static str>,
    library_panel_open: bool,
    metadata_open: bool,
    expanded_result_files: BTreeSet<String>,
    result_file_limit: usize,
    hover_hint: HoverHint,
    runtime_status: Option<RuntimeStatus>,
    runtime_log: String,
    status_target: Option<PathBuf>,
    status_probe: StatusProbe,
    status_probe_running: bool,
    status_probe_pending: bool,
    status_report: Option<StatusReport>,
    tree_state: Entity<TreeState>,
    tree_items: Vec<TreeItem>,
    tree_targets: Rc<BTreeMap<String, TreeTarget>>,
    directory_cache: BTreeMap<String, Vec<DirectoryEntry>>,
    directory_queue: VecDeque<String>,
    directory_running: bool,
    directory_epoch: u64,
    expanded_folders: BTreeSet<String>,
    expand_all: bool,
    search_active: bool,
    applied_search: Option<SearchFilter>,
    search_revision: u64,
    file_focus: FocusHandle,
    file_menu: Option<Entity<PopupMenu>>,
    file_menu_session: MenuSession,
    selected_folder: String,
    file_info: Option<crate::file_info::SourceInfo>,
    file_properties: FilePropertyLabels,
    link_events: Arc<Mutex<VecDeque<LinkEvent>>>,
    document_anchor: Option<String>,
    preview_surface: Rc<RefCell<PreviewSurface>>,
    preview_revision: u64,
    preview_lane: CoalescedRead<PreviewRequest>,
    preview_target: Option<SourceTarget>,
    search_lane: SearchLanes,
    indexing: Option<PathBuf>,
    index_failure: Option<(PathBuf, String)>,
    index_warning: Option<(PathBuf, String)>,
    metadata_saving: Option<SourceTarget>,
    metadata_feedback: String,
    metadata_baseline: Option<MetadataDraft>,
    metadata_revision: u64,
}

impl LibraryWindow {
    /// Create the controls and a child WKWebView constrained to local content.
    fn new(root: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        let status_target = root.clone();
        let initial = root.unwrap_or_else(|| PathBuf::from("~/Documents/Omni Knowledge"));
        let root_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Knowledge root folder")
                .default_value(initial.to_string_lossy().into_owned())
        });
        let query_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search your knowledge…"));
        let tags_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("For example: research, travel"));
        let edit_tags_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Separate tags with commas"));
        let description_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(2)
                .soft_wrap(true)
                .placeholder("Add a note about this file…")
        });
        for field in [
            &root_input,
            &query_input,
            &tags_input,
            &edit_tags_input,
            &description_input,
        ] {
            cx.subscribe_in(field, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Focus) {
                    this.focus_native_input(cx);
                }
            })
            .detach();
        }
        cx.subscribe_in(
            &root_input,
            window,
            |this, input, event: &InputEvent, _, cx| match event {
                InputEvent::PressEnter { .. } => this.open(false, cx),
                InputEvent::Change => {
                    this.status_target = Some(expand_home(input.read(cx).value().as_ref()));
                    this.refresh_status(cx);
                }
                _ => {}
            },
        )
        .detach();
        for field in [&query_input, &tags_input] {
            cx.subscribe_in(
                field,
                window,
                |this, _, event: &InputEvent, _, cx| match event {
                    InputEvent::PressEnter { .. } => this.search(cx),
                    InputEvent::Change => cx.notify(),
                    _ => {}
                },
            )
            .detach();
        }
        for field in [&edit_tags_input, &description_input] {
            cx.subscribe_in(field, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.metadata_revision += 1;
                    this.metadata_feedback = metadata_edit_feedback(
                        this.metadata_is_dirty(cx),
                        this.metadata_saving.is_some(),
                        this.file_info
                            .as_ref()
                            .is_some_and(|info| info.metadata_saved),
                    )
                    .into();
                    this.status = this.metadata_feedback.clone();
                    cx.notify();
                }
            })
            .detach();
        }
        let tree_state = cx.new(|cx| TreeState::new(cx));
        cx.observe(&tree_state, |this, _, cx| this.tree_changed(cx))
            .detach();
        let link_events = Arc::new(Mutex::new(VecDeque::new()));
        let preview_state = Arc::new(RwLock::new(PreviewState { body: "<h1>Your knowledge, together.</h1><p>Open a knowledge folder, or initialize a new one.</p><p>Search exact tags, exact text, indexed words, and multimodal content from one place.</p>".into(), ..Default::default() }));
        let protocol_state = preview_state.clone();
        let navigation_state = preview_state.clone();
        let navigation_events = link_events.clone();
        let protocol_executor = cx.background_executor().clone();
        let preview = WebViewBuilder::new()
            .with_asynchronous_custom_protocol("omni".into(), move |_, request, responder| {
                let state = protocol_snapshot(&protocol_state);
                protocol_executor
                    .spawn(async move {
                        responder.respond(match state {
                            Ok(state) => protocol_response(request, &Arc::new(RwLock::new(state))),
                            Err(error) => protocol_error(&error.to_string()),
                        });
                    })
                    .detach();
            })
            .with_navigation_handler(move |value| {
                if internal_navigation(&value) {
                    return true;
                }
                if let Ok(url) = url::Url::parse(&value)
                    && url.scheme() == "omni"
                    && url.host_str() == Some("nav")
                    && url.path() == "/open"
                    && let Ok(state) = navigation_state.read()
                    && let (Some(root), Some(relative)) = (&state.root, &state.relative)
                    && let Some(target) = url
                        .query_pairs()
                        .find(|(key, _)| key == "target")
                        .map(|(_, value)| value.into_owned())
                    && let Some(issuer) = url
                        .query_pairs()
                        .find(|(key, _)| key == "issuer")
                        .map(|(_, value)| value.into_owned())
                    && issuer == state.issuer
                    && let Some(raw) = url
                        .query_pairs()
                        .find(|(key, _)| key == "raw")
                        .map(|(_, value)| value.into_owned())
                    && let Ok(mut events) = navigation_events.lock()
                    && events.len() < 32
                {
                    events.push_back(LinkEvent {
                        root: root.clone(),
                        relative: relative.clone(),
                        url: target,
                        raw,
                        issuer,
                    });
                }
                false
            })
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_url("omni://app/preview")
            .with_bounds(wry::Rect {
                position: LogicalPosition::new(500., 150.).into(),
                size: LogicalSize::new(600., 550.).into(),
            })
            .build_as_child(window)
            .context("create WKWebView preview")?;
        let timer_executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |this, cx| this.refresh_status(cx));
            loop {
                timer_executor.timer(STATUS_INTERVAL).await;
                if this.update(cx, |this, cx| this.refresh_status(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let link_executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                link_executor.timer(Duration::from_millis(100)).await;
                if this.update(cx, |this, cx| this.receive_links(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Ok(Self {
            window: window.window_handle(),
            root_input,
            query_input,
            tags_input,
            edit_tags_input,
            description_input,
            root: None,
            library_name: String::new(),
            hits: Rc::new(vec![]),
            selected: None,
            mode: SearchMode::Bm25,
            search_options_open: false,
            mode_menu_open: false,
            mode_popup: None,
            embeddings: false,
            busy: false,
            status: "Open a folder to begin".into(),
            preview: Rc::new(preview),
            preview_state,
            generation: 0,
            logo: Arc::new(gpui::Image::from_bytes(
                gpui::ImageFormat::Png,
                include_bytes!("../assets/brand/omni-mark.png").to_vec(),
            )),
            guide_open: false,
            guide_anchor: None,
            library_panel_open: false,
            metadata_open: false,
            expanded_result_files: BTreeSet::new(),
            result_file_limit: RESULT_PAGE_SIZE,
            hover_hint: HoverHint::default(),
            runtime_status: None,
            runtime_log:
                "依赖检查只读取本机工具的位置，不联网、不修改知识库。安装操作需要你点击对应的按钮。"
                    .into(),
            status_target,
            status_probe: StatusProbe::default(),
            status_probe_running: false,
            status_probe_pending: false,
            status_report: None,
            tree_state,
            tree_items: vec![],
            tree_targets: Rc::new(BTreeMap::new()),
            directory_cache: BTreeMap::new(),
            directory_queue: VecDeque::new(),
            directory_running: false,
            directory_epoch: 0,
            expanded_folders: BTreeSet::new(),
            expand_all: false,
            search_active: false,
            applied_search: None,
            search_revision: 0,
            file_focus: cx.focus_handle().tab_stop(true),
            file_menu: None,
            file_menu_session: MenuSession::default(),
            selected_folder: String::new(),
            file_info: None,
            file_properties: FilePropertyLabels::default(),
            link_events,
            document_anchor: None,
            preview_surface: Rc::new(RefCell::new(PreviewSurface::default())),
            preview_revision: 0,
            preview_lane: CoalescedRead::default(),
            preview_target: None,
            search_lane: SearchLanes::default(),
            indexing: None,
            index_failure: None,
            index_warning: None,
            metadata_saving: None,
            metadata_feedback: String::new(),
            metadata_baseline: None,
            metadata_revision: 0,
        })
    }

    /// Return Cocoa keyboard ownership to the GPUI parent for explicit input, file, or menu focus.
    /// Runs only on the main thread after focus or pointer interaction; API errors remain visible.
    fn focus_native_input(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.preview.focus_parent() {
            self.status = format!("Cannot restore native input focus: {error}");
            cx.notify();
        }
    }

    /// Wrap a Kit input to restore native focus even when its logical focus did not change.
    /// A preview click can retain the same GPUI focus ID while WebKit owns the Cocoa responder.
    fn native_input(
        &self,
        state: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(("native-input", state.entity_id()))
            .w_full()
            .h(px(CONTROL_HEIGHT))
            .min_w_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.focus_native_input(cx)),
            )
            .child(input(state))
    }

    /// Change native visibility only when a dialog actually changes the desired surface state.
    fn update_preview_visibility(&self, visible: bool) -> Result<()> {
        if self.preview_surface.borrow().visible == Some(visible) {
            return Ok(());
        }
        self.preview.set_visible(visible)?;
        self.preview_surface.borrow_mut().visible = Some(visible);
        Ok(())
    }

    /// Run filesystem and search work off the UI thread and show exact failures.
    fn work(
        &mut self,
        message: &str,
        job: impl FnOnce() -> Result<JobResult> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.file_menu = None;
        self.mode_menu_open = false;
        self.mode_popup = None;
        self.status = message.into();
        cx.notify();
        let window_handle = self.window;
        let task = cx.background_executor().spawn(async move { job() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = cx.update_window(window_handle, |_, _window, cx| this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(JobResult::Library { root, name, hits, message, initialize_index }) => {
                        reset_tree_on_root_change(this.root.as_deref(), &root, &mut this.expanded_result_files);
                        let changed = this.root.as_ref() != Some(&root);
                        this.file_menu = None;
                        this.preview_revision+=1;
                        this.preview_lane.clear();this.preview_target=None;this.search_lane.clear();
                        this.root = Some(root.clone());
                        this.library_name = name;
                        this.index_failure = None;
                        this.index_warning = None;
                        this.hits = Rc::new(hits);
                        this.search_active = false;
                        this.applied_search = None;
                        this.search_revision += 1;
                        if changed {
                            this.directory_epoch += 1;
                            this.directory_running = false;
                            this.directory_cache.clear();
                            this.directory_queue.clear();
                            this.expanded_folders.clear();
                            this.expand_all = false;
                            this.selected_folder.clear();
                        }
                        this.refresh_directories(cx);
                        this.rebuild_tree(cx);
                        this.selected = None;
                        this.metadata_open = false;
                        if let Ok(mut preview) = this.preview_state.write() { preview.root = Some(root); preview.issuer=format!("kb-{}",uuid::Uuid::new_v4().simple()); preview.relative = None; preview.body = "<h2>Library ready</h2><p>Select a file to preview its contents.</p>".into(); }
                        this.reload_preview();
                        this.status = message;
                        this.refresh_status(cx);
                        if initialize_index && let Some(root)=this.root.clone(){this.start_index(root,false,cx);}
                    }
                    Ok(JobResult::FileAction { root, removed, changed, message }) => {
                        this.status = message;
                        if changed && this.root.as_ref() == Some(&root) {
                            if removed.as_ref().is_some_and(|relative|this.preview_target.as_ref().is_some_and(|target|&target.relative==relative)) {this.preview_revision+=1;this.preview_lane.clear();this.preview_target=None;}
                            if removed.as_ref() == this.selected.as_ref() && removed.is_some() {
                                this.selected = None; this.metadata_open = false;
                                if let Ok(mut preview) = this.preview_state.write() { preview.relative = None; preview.body = "<h2>Source moved</h2><p>Select its new location to preview. Refresh the index to update indexed search results.</p>".into(); }
                                this.reload_preview();
                            }
                            this.search_active = false; this.applied_search = None; this.search_lane.clear(); this.search_revision += 1;
                            this.refresh_directories(cx); this.rebuild_tree(cx); this.refresh_status(cx);
                        }
                    }
                    Ok(JobResult::Preview { .. } | JobResult::FolderNavigation { .. }) => { this.status="Unexpected read result in mutation lane".into(); }
                    Ok(JobResult::Runtime(status)) => { this.runtime_status = Some(status); this.status = "本机依赖检查完成".into(); }
                    Ok(JobResult::Installation(launch)) => {
                        this.runtime_log = format!("已把固定安装命令交给 macOS 终端：\n{} install {}\n\n启动脚本：{}\n\n这只确认终端启动请求已接受，尚未确认安装完成。请在终端查看进度、输出和交互提示；完成后回到这里点击“重新检查”。关闭知识库应用不会中断终端中的安装。", launch.brew_path.display(), launch.package.formula(), launch.command_path.display());
                        this.status = "安装命令已在终端打开；完成后重新检查".into();
                    }
                    Ok(JobResult::InstallationError(error)) => { this.runtime_log = format!("终端启动请求失败或未确认：\n{error}"); this.status = "未能确认终端启动，请查看交接记录".into(); }
                    Err(error) => { this.status = format!("{error:#}"); }
                }
                cx.notify();
            }));
        }).detach();
    }

    /// Reload the confined preview page after changing its shared document state.
    fn reload_preview(&mut self) {
        self.generation += 1;
        if let Err(error) = self.preview.load_url(&format!(
            "omni://app/{}?v={}{}",
            if self.guide_open { "guide" } else { "preview" },
            self.generation,
            if self.guide_open {
                self.guide_anchor
                    .map(|anchor| format!("#{anchor}"))
                    .unwrap_or_default()
            } else {
                self.document_anchor
                    .as_ref()
                    .map(|anchor| {
                        let mut url =
                            url::Url::parse("omni://app/preview").expect("static preview URL");
                        url.set_fragment(Some(anchor));
                        format!("#{}", url.fragment().unwrap_or_default())
                    })
                    .unwrap_or_default()
            }
        )) {
            self.status = error.to_string();
        }
    }

    /// Open an existing root or explicitly initialize the entered folder.
    fn open(&mut self, initialize: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let path = expand_home(self.root_input.read(cx).value().as_ref());
        self.status_target = Some(path.clone());
        self.refresh_status(cx);
        self.work(
            if initialize {
                "Initializing knowledge folder…"
            } else {
                "Opening knowledge folder…"
            },
            move || {
                let kb = if initialize {
                    KnowledgeBase::init(
                        &path,
                        path.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Knowledge library"),
                    )?
                } else {
                    KnowledgeBase::open(&path)?
                };
                let hits = Vec::new();
                let message = if initialize {
                    "Knowledge folder initialized · first lexical index queued".into()
                } else {
                    "Knowledge folder opened · live files".into()
                };
                Ok(JobResult::Library {
                    name: kb.config.name,
                    root: kb.root,
                    hits,
                    message,
                    initialize_index: initialize,
                })
            },
            cx,
        );
    }

    /// Start one explicit index or first lexical index without using the root-opening mutation lane.
    fn start_index(&mut self, root: PathBuf, embeddings: bool, cx: &mut Context<Self>) {
        if let Some(active) = &self.indexing {
            self.status = format!(
                "Index refresh already running for {}; browse while it completes",
                active.display()
            );
            cx.notify();
            return;
        }
        self.indexing = Some(root.clone());
        self.index_failure = None;
        self.index_warning = None;
        let source = root.clone();
        let task = cx.background_executor().spawn(async move {
            file_actions::open_exact_root(&source)?.index(&IndexOptions {
                embeddings,
                force: false,
            })
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.indexing = None;
                if this.root.as_ref() == Some(&root) {
                    this.status = match result {
                        Ok(report) => {
                            this.index_warning = index_warning_feedback(&root, &report);
                            index_message(&report)
                        }
                        Err(error) => {
                            let message = format!("Index refresh failed: {error:#}");
                            this.index_failure = Some((root.clone(), message.clone()));
                            message
                        }
                    };
                    this.refresh_status(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Read editable search intent without changing the applied filter or dispatching background work.
    fn search_draft(&self, cx: &App) -> SearchFilter {
        SearchFilter::new(
            self.query_input.read(cx).value().to_string(),
            self.mode,
            parse_tags(self.tags_input.read(cx).value().as_ref()),
        )
    }

    /// Queue only the newest query while preserving access to the previous published snapshot.
    fn search(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            self.status = "Open a knowledge folder first".into();
            cx.notify();
            return;
        };
        let filter = self.search_draft(cx);
        self.search_revision += 1;
        if !filter.restricts_files() {
            self.search_active = false;
            self.applied_search = None;
            self.search_lane.clear();
            self.status = "All files · live folder browsing".into();
            self.rebuild_tree(cx);
            self.refresh_directories(cx);
            return;
        }
        let mode = filter.effective_mode();
        let lane = self.search_lane.enqueue(SearchRequest {
            root,
            revision: self.search_revision,
            filter: filter.clone(),
            options: SearchOptions {
                query: filter.query,
                mode,
                tags: filter.tags,
                limit: 10000,
                case_sensitive: false,
                query_image: None,
            },
        });
        self.launch_search(lane, cx);
    }

    /// Run bounded local and model workers independently, publishing only the current root/query.
    fn launch_search(&mut self, lane: SearchLane, cx: &mut Context<Self>) {
        let Some(request) = self.search_lane.dispatch(lane) else {
            return;
        };
        self.status = "Searching the published index…".into();
        cx.notify();
        let root = request.root.clone();
        let revision = request.revision;
        let filter = request.filter.clone();
        let task = cx.background_executor().spawn(async move {
            file_actions::open_exact_root(&request.root)?.search(&request.options)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.search_lane.complete(lane, revision);
                if current_read_request(this.root.as_deref(), &root, this.search_revision, revision)
                {
                    match result {
                        Ok(hits) => {
                            this.search_active = true;
                            this.applied_search = Some(filter.clone());
                            this.result_file_limit = RESULT_PAGE_SIZE;
                            this.expanded_result_files.clear();
                            this.status = search_result_summary(
                                &hits,
                                filter.effective_mode(),
                                this.result_file_limit,
                            );
                            this.hits = Rc::new(hits);
                            this.rebuild_tree(cx);
                        }
                        Err(error) => this.status = format!("Search failed: {error:#}"),
                    }
                }
                this.launch_search(lane, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Prepare a safe preview fragment for a selected search result.
    fn select(
        &mut self,
        relative: String,
        start_seconds: Option<f64>,
        line: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.select_document(
            relative,
            PreviewLocation {
                start_seconds,
                line,
                anchor: None,
            },
            false,
            cx,
        );
    }

    /// Resolve local navigation on a worker and preserve a requested heading or inspector state.
    fn select_document(
        &mut self,
        relative: String,
        location: PreviewLocation,
        show_metadata: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.preview_revision += 1;
        if self.selected.as_ref() != Some(&relative) {
            self.metadata_open = false;
        }
        self.preview_target = Some(SourceTarget {
            root: root.clone(),
            relative: relative.clone(),
        });
        self.preview_lane.enqueue(PreviewRequest {
            root,
            relative,
            location,
            show_metadata,
            revision: self.preview_revision,
            metadata_revision: self.metadata_revision,
            issuer: self
                .preview_state
                .read()
                .map(|state| state.issuer.clone())
                .unwrap_or_default(),
        });
        self.launch_preview(cx);
    }

    /// Run one preview worker, coalescing later clicks without occupying the mutation or index lane.
    fn launch_preview(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.preview_lane.dispatch() else {
            return;
        };
        self.status = format!("Loading preview: {}", request.relative);
        cx.notify();
        let root = request.root.clone();
        let revision = request.revision;
        let metadata_revision = request.metadata_revision;
        let task = cx.background_executor().spawn(async move {
            let kb = file_actions::open_exact_root(&request.root)?;
            let path = if request.relative.is_empty() {
                kb.root.clone()
            } else {
                kb.resolve(&request.relative)?
            };
            if path.is_dir() {
                return Ok(JobResult::FolderNavigation {
                    relative: request.relative.clone(),
                    entries: file_actions::directory_children(&kb.root, &request.relative)?,
                });
            }
            let info = crate::file_info::source_info(&request.root, &request.relative)?;
            let body = preview_body(
                &info.path,
                &info.relative,
                request.location.start_seconds,
                request.location.line,
                &request.issuer,
            )?;
            let properties = FilePropertyLabels {
                file_type: human_file_type(&info.relative, &info.media_type),
                size: human_file_size(info.size),
                created: local_file_date(info.created_ms),
                modified: local_file_date(info.modified_ms),
            };
            Ok(JobResult::Preview {
                relative: info.relative.clone(),
                body: body.into(),
                info: Box::new(info),
                properties,
                anchor: request.location.anchor,
                show_metadata: request.show_metadata,
            })
        });
        let window_handle = self.window;
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = cx.update_window(window_handle, |_, window, cx| {
                this.update(cx, |this, cx| {
                    this.preview_lane.complete();
                    if current_read_request(
                        this.root.as_deref(),
                        &root,
                        this.preview_revision,
                        revision,
                    ) {
                        this.apply_preview(result, metadata_revision, window, cx);
                    }
                    this.launch_preview(cx);
                    cx.notify();
                })
            });
        })
        .detach();
    }

    /// Commit a current read result, preserving authored drafts when only a media timestamp changes.
    fn apply_preview(
        &mut self,
        result: Result<JobResult>,
        metadata_revision: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(JobResult::Preview {
                relative,
                body,
                info,
                properties,
                anchor,
                show_metadata,
            }) => {
                let mut info = *info;
                let tags = info.tags.clone();
                let description = info.description.clone();
                let changed = self.selected.as_ref() != Some(&relative);
                if changed {
                    self.metadata_open = false;
                    self.metadata_feedback.clear();
                }
                self.selected = Some(relative.clone());
                if !changed
                    && metadata_revision != self.metadata_revision
                    && let Some(previous) = &self.file_info
                {
                    info.metadata_saved = previous.metadata_saved;
                }
                self.file_info = Some(info);
                self.file_properties = properties;
                self.document_anchor = anchor;
                if show_metadata {
                    self.metadata_open = true;
                }
                if refresh_authored_metadata(
                    changed,
                    self.metadata_is_dirty(cx),
                    metadata_revision,
                    self.metadata_revision,
                ) {
                    self.metadata_revision += 1;
                    self.metadata_baseline = self.root.clone().map(|root| MetadataDraft {
                        target: SourceTarget {
                            root,
                            relative: relative.clone(),
                        },
                        tags: {
                            let mut values = tags.clone();
                            values.sort();
                            values.dedup();
                            values
                        },
                        description: description.clone(),
                    });
                    self.edit_tags_input
                        .update(cx, |input, cx| input.set_value(tags.join(", "), window, cx));
                    self.description_input
                        .update(cx, |input, cx| input.set_value(description, window, cx));
                }
                if let Ok(mut preview) = self.preview_state.write() {
                    preview.relative = Some(relative);
                    preview.body = body;
                }
                self.reload_preview();
                self.status = "Preview ready".into();
            }
            Ok(JobResult::FolderNavigation { relative, entries }) => {
                self.selected_folder = relative.clone();
                let mut path = String::new();
                for part in relative.split('/').filter(|part| !part.is_empty()) {
                    if !path.is_empty() {
                        path.push('/');
                    }
                    path.push_str(part);
                    self.expanded_folders.insert(path.clone());
                }
                self.directory_cache.insert(relative.clone(), entries);
                self.search_active = false;
                self.applied_search = None;
                self.search_lane.clear();
                self.search_revision += 1;
                self.rebuild_tree(cx);
                self.refresh_directories(cx);
                self.status = format!("Folder selected: {relative}");
            }
            Ok(_) => self.status = "Unexpected preview result".into(),
            Err(error) => self.status = format!("Preview failed: {error:#}"),
        }
    }

    /// Choose a knowledge directory through macOS's native folder picker.
    fn choose_root(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open knowledge folder".into()),
        });
        let window_handle = self.window;
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picker.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = cx.update_window(window_handle, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        this.root_input.update(cx, |input, cx| {
                            input.set_value(path.to_string_lossy().into_owned(), window, cx)
                        });
                        this.open(false, cx);
                    })
                });
            }
        })
        .detach();
    }

    /// Select files for a collision-safe copy into the selected tree folder or root.
    fn import(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.root.is_none() {
            return;
        }
        let folder = self.selected_folder.clone();
        let root = self.root.clone().expect("checked active root");
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Copy files into selected folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picker.await {
                let _ = this.update(cx, |this, cx| {
                    this.copy_sources_into(paths, root, folder, cx)
                });
            }
        })
        .detach();
    }

    /// Detect edits against the last loaded or successfully persisted values of this selected source.
    fn metadata_is_dirty(&self, cx: &App) -> bool {
        let Some(saved) = &self.metadata_baseline else {
            return false;
        };
        self.root.as_ref() == Some(&saved.target.root)
            && self.selected.as_ref() == Some(&saved.target.relative)
            && (authored_tags(self.edit_tags_input.read(cx).value().as_ref()) != saved.tags
                || self.description_input.read(cx).value().as_ref() != saved.description)
    }

    /// Atomically save authored tags and description without implicitly indexing any source.
    fn save_tags(&mut self, cx: &mut Context<Self>) {
        if self.metadata_saving.is_some() {
            self.metadata_feedback =
                "A save is already running; your current edits are kept here".into();
            cx.notify();
            return;
        }
        let (Some(root), Some(relative)) = (self.root.clone(), self.selected.clone()) else {
            self.status = "Select a file first".into();
            cx.notify();
            return;
        };
        let saved = MetadataDraft {
            target: SourceTarget { root, relative },
            tags: authored_tags(self.edit_tags_input.read(cx).value().as_ref()),
            description: self.description_input.read(cx).value().to_string(),
        };
        self.metadata_saving = Some(saved.target.clone());
        let source = saved.clone();
        self.metadata_feedback = "Saving tags and notes…".into();
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            save_authored_metadata(&source.target, &source.tags, &source.description)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.metadata_saving = None;
                if this.root.as_ref() == Some(&saved.target.root)
                    && this.selected.as_ref() == Some(&saved.target.relative)
                {
                    this.metadata_feedback = match result {
                        Ok(path) => {
                            if let Some(info) = this.file_info.as_mut() {
                                info.metadata_saved = true;
                                info.metadata_path = path;
                            }
                            this.metadata_revision += 1;
                            this.metadata_baseline = Some(saved.clone());
                            if this.metadata_is_dirty(cx) {
                                "Saved earlier changes · newer edits are not saved".into()
                            } else {
                                "Saved in this library · refresh the index to update search".into()
                            }
                        }
                        Err(error) => {
                            format!("Not saved: {error:#}. Your edits remain in the fields.")
                        }
                    };
                    this.status = this.metadata_feedback.clone();
                }
                if this.root.as_ref() == Some(&saved.target.root) {
                    this.refresh_status(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Open the trusted guide without losing the selected document or search results.
    fn toggle_guide(&mut self, cx: &mut Context<Self>) {
        self.mode_menu_open = false;
        self.mode_popup = None;
        self.file_menu = None;
        self.hover_hint.clear();
        self.guide_anchor = None;
        self.guide_open = !self.guide_open;
        self.reload_preview();
        if self.guide_open && self.runtime_status.is_none() && !self.busy {
            self.refresh_runtime(cx);
        }
        cx.notify();
    }

    /// Detect local tool locations on a worker without invoking installers or model services.
    fn refresh_runtime(&mut self, cx: &mut Context<Self>) {
        self.work(
            "正在检查本机依赖…",
            || Ok(JobResult::Runtime(runtime::detect_runtime())),
            cx,
        );
    }

    /// Inspect the selected or edited folder independently of search and indexing work.
    fn refresh_status(&mut self, cx: &mut Context<Self>) {
        if self.status_probe_running {
            self.status_probe_pending = true;
            return;
        }
        self.status_probe_running = true;
        let path = self.status_target.clone().unwrap_or_default();
        let probe = self.status_probe.clone();
        let task = cx
            .background_executor()
            .spawn(async move { probe.inspect(&path) });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let report = task.await;
            let _ = this.update(cx, |this, cx| {
                this.status_probe_running = false;
                let rerun = std::mem::take(&mut this.status_probe_pending);
                if !status_target_matches(&report.folder.requested, this.status_target.as_deref()) {
                    this.refresh_status(cx);
                    return;
                }
                this.runtime_status = Some(report.tools.clone());
                this.status_report = Some(report);
                if !this.search_active {
                    this.refresh_directories(cx);
                }
                cx.notify();
                if rerun {
                    this.refresh_status(cx);
                }
            });
        })
        .detach();
    }

    /// Open a specific inert guide section while preserving the search and selected source.
    fn show_guide(&mut self, anchor: &'static str, cx: &mut Context<Self>) {
        self.mode_menu_open = false;
        self.mode_popup = None;
        self.file_menu = None;
        self.hover_hint.clear();
        self.guide_anchor = Some(anchor);
        self.guide_open = true;
        self.reload_preview();
        cx.notify();
    }

    /// Render a hover observation; callers add a pointer cursor only when they also add an action.
    fn status_badge(
        &self,
        id: &'static str,
        label: String,
        detail: String,
        warning: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .px_2()
            .py_1()
            .max_w(px(390.))
            .min_w_0()
            .rounded_md()
            .text_size(px(11.))
            .text_color(rgb(if warning { 0x8a6a39 } else { 0x477458 }))
            .hover(|style| style.bg(rgb(0xe2ebdf)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.hover_hint.update(id, *hovered, &detail);
                cx.notify();
            }))
            .child(
                div()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(single_line_status_label(&label)),
            )
    }

    /// Revalidate the displayed observation and opened owner before an explicit footer action.
    fn activate_footer_index(
        &mut self,
        action: FooterIndexAction,
        requested: Option<PathBuf>,
        explicit_refresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let folder = self
            .status_report
            .as_ref()
            .filter(|report| {
                status_target_matches(&report.folder.requested, self.status_target.as_deref())
            })
            .map(|report| &report.folder);
        if requested != self.status_target
            || footer_index_action(self.root.as_deref(), folder, explicit_refresh) != action
        {
            self.refresh_status(cx);
            return;
        }
        match action {
            FooterIndexAction::Index(root) => self.start_index(root, self.embeddings, cx),
            FooterIndexAction::Check => self.refresh_status(cx),
            FooterIndexAction::Library => {
                self.return_to_library(cx);
                self.library_panel_open = true;
                self.focus_native_input(cx);
                window.focus(&self.root_input.read(cx).focus_handle(cx));
                cx.notify();
            }
        }
    }

    /// Keep one compact bottom row; full action/status hints temporarily expand above its fixed targets.
    fn status_footer(&self, cx: &mut Context<Self>) -> gpui::Div {
        let report = self.status_report.as_ref().filter(|report| {
            status_target_matches(&report.folder.requested, self.status_target.as_deref())
        });
        let folder = report.map(|report| &report.folder);
        let mut row = div()
            .flex()
            .items_center()
            .gap_1()
            .h(px(30.))
            .flex_shrink_0()
            .px_2()
            .text_size(px(11.))
            .bg(rgb(0xecf2e8));
        if let Some(root) = self.root.clone() {
            let name = if self.library_name.is_empty() {
                root.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            } else {
                self.library_name.clone()
            };
            let detail = format!(
                "{name} · {} · Click to open this knowledge root in Finder",
                root.display()
            );
            let target = SourceTarget {
                root: root.clone(),
                relative: String::new(),
            };
            row = row.child(
                div()
                    .id("actual-root")
                    .flex()
                    .items_center()
                    .gap_1()
                    .max_w(px(320.))
                    .min_w_0()
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hover_hint.update("actual-root", *hovered, &detail);
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.source_action(target.clone(), "open-root", cx)
                    }))
                    .child(
                        div()
                            .max_w(px(110.))
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_color(rgb(0x6a7f6d))
                            .child(root.display().to_string()),
                    ),
            );
        } else {
            row = row.child(
                button("footer-open-library", "Open library…", false).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.return_to_library(cx);
                        this.library_panel_open = true;
                        cx.notify();
                    },
                )),
            );
        }
        let destination = if self.selected_folder.is_empty() {
            "/"
        } else {
            &self.selected_folder
        };
        let destination_detail = self
            .root
            .as_ref()
            .map(|root| {
                format!(
                    "Paste / drop destination: {}",
                    root.join(&self.selected_folder).display()
                )
            })
            .unwrap_or_else(|| "Open a library before copying files into it".into());
        row = row.child(
            self.status_badge(
                "transfer-destination",
                format!("Paste: {destination}"),
                destination_detail,
                false,
                cx,
            )
            .max_w(px(160.)),
        );
        if let Some(report) = report {
            let mut missing = Vec::new();
            if report.tools.homebrew.is_none() {
                missing.push("Homebrew");
            }
            if report.tools.ffmpeg.is_none() {
                missing.push("FFmpeg");
            }
            if report.tools.ffprobe.is_none() {
                missing.push("ffprobe");
            }
            if report.tools.whisper_cli.is_none() && report.asr.state != ServiceState::Available {
                missing.push("whisper.cpp");
            }
            let tools_label = if missing.is_empty() {
                "工具 ✓".into()
            } else {
                format!("工具缺 {}", missing.len())
            };
            let tools_detail = format!(
                "未发现：{}。按标准 Homebrew 与 PATH 位置检查可选工具；点击查看用途与安装。安装工具不包含模型。",
                if missing.is_empty() {
                    "无".into()
                } else {
                    missing.join("、")
                }
            );
            row = row.child(
                self.status_badge(
                    "tools-status",
                    tools_label,
                    tools_detail,
                    !missing.is_empty(),
                    cx,
                )
                .flex_shrink_0()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.show_guide("environment", cx))),
            );
            let provider_label = match report.provider.state {
                ServiceState::Available => "模型 ✓",
                ServiceState::NotConfigured => "模型 —",
                ServiceState::Unavailable => "模型 ×",
            };
            row = row.child(
                self.status_badge(
                    "provider-status",
                    provider_label.into(),
                    report.provider.detail.clone(),
                    report.provider.state != ServiceState::Available,
                    cx,
                )
                .flex_shrink_0()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.show_guide("local-model", cx))),
            );
            let asr_label = match report.asr.state {
                ServiceState::Available => "ASR 文件 ✓",
                ServiceState::NotConfigured => "ASR —",
                ServiceState::Unavailable => "ASR ×",
            };
            row = row.child(
                self.status_badge(
                    "asr-status",
                    asr_label.into(),
                    report.asr.detail.clone(),
                    report.asr.state != ServiceState::Available,
                    cx,
                )
                .flex_shrink_0()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.show_guide("asr", cx))),
            );
        }
        let routine = self.status == "Saved in .omni/metadata/"
            || self.status == "No saved tags or notes yet"
            || self.status == "Preview ready"
            || self.status.starts_with("Folder selected:")
            || self.status.starts_with("All files ·")
            || self.status.starts_with("Knowledge folder opened")
            || self.status.starts_with("Knowledge folder initialized")
            || self.status.starts_with("Indexed ")
            || self.status.starts_with("Index refresh failed:")
            || self.status == "Open a folder to begin";
        let feedback = if self.metadata_saving.is_some() {
            Some("Saving metadata…")
        } else if self.busy {
            Some(self.status.as_str())
        } else if self.search_lane.is_searching() {
            Some("Searching…")
        } else if self.preview_lane.running {
            Some("Loading preview…")
        } else if !routine {
            Some(self.status.as_str())
        } else {
            None
        };
        if let Some(feedback) = feedback {
            row = row.child(
                self.status_badge(
                    "operation-status",
                    feedback.into(),
                    feedback.into(),
                    self.busy,
                    cx,
                )
                .max_w(px(180.)),
            );
        }
        row = row.child(div().flex_1().min_w_0());
        let failure = self
            .index_failure
            .as_ref()
            .filter(|(root, _)| self.root.as_ref() == Some(root));
        let coverage_warning = self.index_warning.as_ref().filter(|(root, _)| {
            self.root.as_ref() == Some(root)
                && folder.is_some_and(|folder| folder.root.as_ref() == Some(root))
        });
        let retry = failure.is_some();
        let action = footer_index_action(self.root.as_deref(), folder, retry);
        let (mut label, mut warning) = if action == FooterIndexAction::Library {
            match folder.map(|folder| folder.state) {
                Some(FolderState::Uninitialized) => ("初始化所选目录…".into(), true),
                Some(FolderState::Missing) => ("所选目录不存在…".into(), true),
                Some(FolderState::NotDirectory) => ("需要选择文件夹…".into(), true),
                _ => ("打开所选目录…".into(), true),
            }
        } else if let Some(root) = &self.indexing {
            (
                format!(
                    "索引中 · {}",
                    root.file_name().unwrap_or_default().to_string_lossy()
                ),
                false,
            )
        } else if failure.is_some() {
            ("索引失败 · 重试".into(), true)
        } else {
            match folder.map(|folder| folder.state) {
                Some(FolderState::Stale) => ("索引过期 · 刷新".into(), true),
                Some(FolderState::NeedsIndex) => ("需要建立索引".into(), true),
                Some(FolderState::Current) if coverage_warning.is_some() => {
                    ("索引含警告".into(), true)
                }
                Some(FolderState::Current) => ("索引未见变化".into(), false),
                Some(FolderState::Checking) => ("索引检查未完成".into(), true),
                _ => ("正在检查…".into(), false),
            }
        };
        if coverage_warning.is_some()
            && self.indexing.is_none()
            && failure.is_none()
            && !folder.is_some_and(|folder| folder.state == FolderState::Current)
        {
            label.push_str(" · ⚠");
            warning = true;
        }
        let mut detail = folder
            .map(|folder| format!("{} · {}", folder.requested.display(), folder.detail))
            .unwrap_or_else(|| "正在检查所选目录；未确认的目录不会启动索引。".into());
        if action == FooterIndexAction::Library {
            detail.push_str(" · 点击展开 Library，选择 Open 或 Initialize；当前知识库保持不变。");
        } else if let Some((_, failure)) = failure {
            detail.push_str(&format!(" · {failure} · 点击重试索引。"));
        } else if matches!(action, FooterIndexAction::Index(_)) {
            detail.push_str(" · 点击直接刷新当前知识库的索引。");
        } else {
            detail.push_str(" · 点击重新检查；右侧 Refresh index 显式刷新索引。");
        }
        if let Some((_, warning)) = coverage_warning {
            detail.push_str(&format!(" · {warning}"));
        }
        let requested = self.status_target.clone();
        row = row.child(
            self.status_badge("folder-status", label, detail, warning, cx)
                .max_w(px(190.))
                .flex_shrink_0()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate_footer_index(
                        action.clone(),
                        requested.clone(),
                        retry,
                        window,
                        cx,
                    );
                })),
        );
        let refresh_action = footer_index_action(self.root.as_deref(), folder, true);
        let enabled =
            matches!(refresh_action, FooterIndexAction::Index(_)) && self.indexing.is_none();
        let requested = self.status_target.clone();
        row = row.child(button("footer-index-refresh", "Refresh index", false).icon(IconName::Redo).flex_shrink_0()
            .disabled(!enabled)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hover_hint.update("footer-index-refresh", *hovered,
                    "Refresh index · 显式刷新当前已打开且已确认的知识库；若修改了 Library 路径，请先 Open。");
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.activate_footer_index(refresh_action.clone(), requested.clone(), true, window, cx);
            })));
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgb(0xd4dfd6))
            .bg(rgb(0xecf2e8))
            .when(self.hover_hint.detail().is_some(), |footer| {
                footer.child(
                    div()
                        .id("status-details")
                        .max_h(px(108.))
                        .overflow_y_scroll()
                        .px_3()
                        .py_1()
                        .text_size(px(11.))
                        .text_color(rgb(0x526b59))
                        .whitespace_normal()
                        .child(self.hover_hint.detail().unwrap_or_default().to_owned()),
                )
            })
            .child(row)
    }

    /// Hand off one allowlisted package to visible Terminal after an explicit native button click.
    fn install_runtime(&mut self, package: BrewPackage, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self
            .runtime_status
            .as_ref()
            .is_none_or(|status| status.homebrew.is_none())
        {
            self.runtime_log = "未找到 Homebrew。请先打开官方安装说明，在终端完成安装，再点击“重新检查”。应用不会执行 sudo 或自动运行远程安装脚本。".into();
            cx.notify();
            return;
        }
        self.runtime_log = format!(
            "正在为 brew install {} 打开 macOS 终端。安装进度、输出和交互提示在终端中显示；完成后请回到这里重新检查。不会下载模型或修改知识库配置。",
            package.formula()
        );
        self.work(
            "正在打开终端安装命令…",
            move || match runtime::launch_install(package) {
                Ok(launch) => Ok(JobResult::Installation(launch)),
                Err(error) => Ok(JobResult::InstallationError(format!("{error:#}"))),
            },
            cx,
        );
    }

    /// Return from the guide through the logo without resetting the library or editor state.
    fn return_to_library(&mut self, cx: &mut Context<Self>) {
        self.mode_menu_open = false;
        self.mode_popup = None;
        self.file_menu = None;
        self.hover_hint.clear();
        self.guide_anchor = None;
        if self.guide_open {
            self.guide_open = false;
            self.reload_preview();
            cx.notify();
        }
    }

    /// Provide visible hover help for an icon without covering the native document view.
    fn action_icon(
        &self,
        id: &'static str,
        glyph: &'static str,
        hint: &'static str,
        active: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        button(id, "", active)
            .icon(match glyph {
                "＋" => IconName::Plus,
                "−" => IconName::Minus,
                "↻" => IconName::Redo,
                "?" => IconName::Info,
                "✎" => IconName::Settings2,
                "↗" => IconName::ExternalLink,
                "×" => IconName::Close,
                _ => IconName::FolderOpen,
            })
            .w(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(16.))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.hover_hint.update(id, *hovered, hint);
                cx.notify();
            }))
    }

    /// Select a menu choice and return typing focus to the existing query input.
    fn choose_mode(&mut self, mode: SearchMode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        self.mode_menu_open = false;
        self.mode_popup = None;
        window.focus(&self.query_input.read(cx).focus_handle(cx));
        cx.notify();
    }

    /// Build the official Kit popup with a captured query focus and selected mode.
    fn open_mode_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        let focus = self.query_input.read(cx).focus_handle(cx);
        let selected = self.mode;
        let menu = PopupMenu::build(window, cx, move |mut menu, _, _| {
            menu = menu.min_w(px(180.)).max_w(px(180.)).action_context(focus);
            for (mode, label) in SEARCH_MODES {
                let owner = owner.clone();
                menu = menu.item(
                    PopupMenuItem::new(label)
                        .checked(selected == mode)
                        .on_click(move |_, window, cx| {
                            let _ = owner.update(cx, |this, cx| this.choose_mode(mode, window, cx));
                        }),
                );
            }
            menu
        });
        cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
            this.mode_menu_open = false;
            this.mode_popup = None;
            cx.notify();
        })
        .detach();
        self.mode_popup = Some(menu);
    }

    /// Use a controlled Kit popover so help and library navigation dismiss the dropdown.
    fn mode_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.mode_popup.clone();
        let label = SEARCH_MODES
            .iter()
            .find(|(mode, _)| *mode == self.mode)
            .map(|(_, label)| *label)
            .unwrap_or("Full text");
        Popover::new("mode-popover")
            .open(self.mode_menu_open)
            .appearance(false)
            .trigger(
                button("mode-selector", label, false)
                    .w(px(164.))
                    .dropdown_caret(true)
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.hover_hint.update(
                            "mode-selector",
                            *hovered,
                            "Choose what to search: file contents, text patterns, meaning, or names and notes",
                        );
                        cx.notify();
                    })),
            )
            .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                this.mode_menu_open = *open;
                if *open {
                    this.open_mode_popup(window, cx);
                } else {
                    this.mode_popup = None;
                }
                cx.notify();
            }))
            .content(move |_, window, cx| {
                if let Some(menu) = &menu {
                    menu.focus_handle(cx).focus(window);
                }
                div().children(menu.clone())
            })
    }

    /// Keep one primary search field; expose optional restrictions only when requested.
    fn header(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut header = div()
            .flex()
            .items_center()
            .gap_2()
            .h(px(44.))
            .flex_shrink_0()
            .px_3()
            .border_b_1()
            .border_color(rgb(0xd4dfd6))
            .child(
                div()
                    .id("home-logo")
                    .size(px(32.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0xe2ede0)))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.hover_hint.update(
                            "home-logo",
                            *hovered,
                            "Home · Return to your files",
                        );
                        cx.notify();
                    }))
                    .child(
                        img(self.logo.clone())
                            .size(px(28.))
                            .relative()
                            .left(px(-1.))
                            .top(px(-1.)),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.return_to_library(cx))),
            );
        if self.guide_open {
            header = header.child(div().flex_1().text_color(rgb(0x526b59)).child("说明与环境"));
        } else {
            let draft = self.search_draft(cx);
            let (options_label, options_hint) =
                search_options_presentation(&draft, self.applied_search.as_ref());
            let clearable = search_filter_is_clearable(
                &draft.query,
                &draft.tags,
                self.search_active || self.search_lane.is_searching(),
            );
            let options_active = self.search_options_open
                || self.applied_search.is_some()
                || !draft.tags.is_empty()
                || self.mode != SearchMode::Bm25;
            let suffix = div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    button("search-options", "Search options", options_active)
                        .label(options_label)
                        .h(px(22.))
                        .dropdown_caret(true)
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hover_hint
                                .update("search-options", *hovered, &options_hint);
                            cx.notify();
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search_options_open = !this.search_options_open;
                            this.mode_menu_open = false;
                            this.mode_popup = None;
                            this.hover_hint.clear();
                            cx.notify();
                            cx.stop_propagation();
                        })),
                )
                .when(clearable, |suffix| {
                    suffix.child(
                        Button::new("clear-search")
                            .icon(IconName::Close)
                            .ghost()
                            .compact()
                            .size(px(22.))
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                this.hover_hint.update(
                                    "clear-search",
                                    *hovered,
                                    "Clear search and tag filters · Show all files",
                                );
                                cx.notify();
                            }))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.clear_search(window, cx);
                                cx.stop_propagation();
                            })),
                    )
                });
            header = header
                .child(
                    div()
                        .id("search-field")
                        .flex_1()
                        .min_w_0()
                        .h(px(CONTROL_HEIGHT))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.focus_native_input(cx)),
                        )
                        .child(input(&self.query_input).suffix(suffix)),
                )
                .child(
                    button("search", "Search", true)
                        .icon(IconName::Search)
                        .on_click(cx.listener(|this, _, _, cx| this.search(cx))),
                )
                .child(
                    self.action_icon(
                        "library-panel",
                        "▤",
                        "Library · Choose a folder and configure the next index refresh",
                        self.library_panel_open,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.library_panel_open = !this.library_panel_open;
                        cx.notify();
                    })),
                );
        }
        if self.guide_open {
            header.child(
                button("guide", "返回知识库", true)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_guide(cx))),
            )
        } else {
            header.child(
                self.action_icon(
                    "guide",
                    "?",
                    "Help · Usage guide and optional tools",
                    false,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_guide(cx))),
            )
        }
    }

    /// Group retrieval scope and exact-tag restrictions under the primary search field.
    fn search_options(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .bg(rgb(0xe9f0e5))
            .child(div().text_size(px(12.)).child("Search in"))
            .child(self.mode_selector(cx))
            .child(
                div()
                    .ml_2()
                    .text_size(px(12.))
                    .child("Tags (all must match)"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.native_input(&self.tags_input, cx)),
            )
            .child(
                button("apply-search-options", "Apply", true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.search_options_open = false;
                        this.search(cx);
                    },
                )),
            )
            .child(
                button("reset-search-options", "Reset options", false).on_click(cx.listener(
                    |this, _, window, cx| {
                        this.tags_input
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        this.mode = SearchMode::Bm25;
                        this.mode_menu_open = false;
                        this.mode_popup = None;
                        this.search(cx);
                        cx.notify();
                    },
                )),
            )
    }

    /// Keep folder lifecycle and the next explicit refresh's model option out of daily retrieval.
    fn library_panel(&self, cx: &mut Context<Self>) -> gpui::Div {
        div().flex().flex_col().gap_2().px_3().py_2().flex_shrink_0().bg(rgb(0xe9f0e5))
            .child(div().flex().items_center().gap_2()
                .child(div().text_size(px(12.)).child("Knowledge folder"))
                .child(div().flex_1().min_w_0().child(self.native_input(&self.root_input, cx)))
                .child(button("choose", "Choose…", false).on_click(cx.listener(|this, _, _, cx| this.choose_root(cx))))
                .child(button("open", "Open", false).on_click(cx.listener(|this, _, _, cx| this.open(false, cx))))
                .child(button("init", "Initialize", false).on_click(cx.listener(|this, _, _, cx| this.open(true, cx)))))
            .child(div().flex().items_center().gap_2()
                .child(Checkbox::new("embedding").label("Build semantic search index").checked(self.embeddings).small()
                    .on_click(cx.listener(|this, checked: &bool, _, cx| { this.embeddings = *checked; cx.notify(); })))
                .child(div().flex_1().min_w_0().text_size(px(11.)).text_color(rgb(0x526758))
                    .child("Include text, images and video in the next index refresh. Requires the local model service. This choice lasts until the app closes.")))
    }

    /// Render selected-file actions and an optional, labelled details and notes editor.
    fn preview_controls(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut controls = div().flex().flex_col().flex_shrink_0();
        let mut toolbar = div()
            .flex()
            .items_center()
            .gap_2()
            .h(px(34.))
            .px_3()
            .border_b_1()
            .border_color(rgb(0xd4dfd6))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .text_color(rgb(0x526b59))
                    .text_ellipsis()
                    .child(
                        self.selected
                            .clone()
                            .unwrap_or_else(|| "Select a file to preview".into()),
                    ),
            );
        if let (Some(root), Some(relative)) = (self.root.clone(), self.selected.clone()) {
            for (id, label, action) in [
                ("quick-look", "Quick Look", "quick-look"),
                ("original", "Open original", "open"),
                ("reveal", "Finder", "reveal"),
            ] {
                let target = SourceTarget {
                    root: root.clone(),
                    relative: relative.clone(),
                };
                toolbar = toolbar.child(button(id, label, false).on_click(cx.listener(
                    move |this, _, _, cx| this.source_action(target.clone(), action, cx),
                )));
            }
            toolbar = toolbar.child(
                button("metadata-panel", "File details", self.metadata_open)
                    .icon(IconName::Info)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.metadata_open = !this.metadata_open;
                        cx.notify();
                    })),
            );
        }
        controls = controls.child(toolbar);
        if self.selected.is_some() && self.metadata_open {
            let Some(info) = self.file_info.as_ref().filter(|info| {
                self.root.as_ref() == Some(&info.root)
                    && self.selected.as_ref() == Some(&info.relative)
            }) else {
                return controls;
            };
            let absolute = info.path.to_string_lossy().into_owned();
            let relative = info.relative.clone();
            let path_hint = absolute.clone();
            let mime_hint = format!("MIME type: {}", info.media_type);
            let storage_hint = info.metadata_path.to_string_lossy().into_owned();
            let target = SourceTarget {
                root: info.root.clone(),
                relative: relative.clone(),
            };
            let feedback = if self.metadata_feedback.is_empty() {
                metadata_edit_feedback(
                    self.metadata_is_dirty(cx),
                    self.metadata_saving.is_some(),
                    info.metadata_saved,
                )
                .to_owned()
            } else {
                self.metadata_feedback.clone()
            };
            let mut notes = Input::new(&self.description_input)
                .small()
                .text_size(px(14.))
                .h(px(52.));
            notes.style().size.height = Some(px(52.).into());
            let details = div().flex().flex_col().gap_2().px_3().py_2().bg(rgb(0xeaf0e6))
                .child(div().flex().items_center().gap_2()
                    .child(div().text_size(px(11.)).text_color(rgb(0x627668)).child("Path"))
                    .child(div().id("file-details-path").flex_1().min_w_0().text_size(px(12.)).text_ellipsis().child(absolute.clone())
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hover_hint.update("file-details-path", *hovered, &path_hint); cx.notify();
                        })))
                    .child(button("copy-details-path", "Copy path", false).on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(absolute.clone()));
                        this.status = "File path copied".into(); cx.notify();
                    })))
                    .child(button("copy-details-relative", "Copy relative path", false).on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(relative.clone()));
                        this.status = "Relative file path copied".into(); cx.notify();
                    }))))
                .child(div().flex().flex_wrap().gap_3().text_size(px(11.)).text_color(rgb(0x526758))
                    .child(div().id("file-details-type").child(format!("Type: {}", self.file_properties.file_type))
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hover_hint.update("file-details-type", *hovered, &mime_hint); cx.notify();
                        })))
                    .child(format!("Size: {}", self.file_properties.size))
                    .child(format!("Created: {}", self.file_properties.created))
                    .child(format!("Modified: {}", self.file_properties.modified)))
                .child(div().flex().items_start().gap_2()
                    .child(div().w(px(230.)).flex_shrink_0().flex().flex_col().gap_1()
                        .child(div().text_size(px(12.)).child("Tags"))
                        .child(self.native_input(&self.edit_tags_input, cx)))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                        .child(div().text_size(px(12.)).child("Notes"))
                        .child(div().id("notes-input").h(px(52.)).w_full()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.focus_native_input(cx)))
                            .child(notes))))
                .child(div().text_size(px(11.)).text_color(rgb(0x627668))
                    .child("Tags and notes are saved as JSON in this library’s .omni/metadata/ folder; the original file stays unchanged. Refresh the index to include changes in search."))
                .child(div().flex().items_center().gap_2()
                    .child(div().flex_1().min_w_0().text_size(px(11.)).text_color(rgb(0x526758)).child(feedback))
                    .when(info.metadata_saved, |row| row.child(
                        button("show-saved-metadata", "Show saved metadata", false)
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                this.hover_hint.update("show-saved-metadata", *hovered, &format!("Reveal the saved tags and notes in Finder: {storage_hint}")); cx.notify();
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| this.reveal_metadata(target.clone(), cx)))
                    ))
                    .child(button("save-metadata", "Save tags and notes", true).disabled(self.metadata_saving.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.save_tags(cx)))));
            controls = controls.child(details);
        }
        controls
    }

    /// Reveal an existing validated authored sidecar without granting previews an internal-file route.
    /// Validation and Finder invocation run off-thread; a missing or replaced record is an explicit error.
    fn reveal_metadata(&mut self, target: SourceTarget, cx: &mut Context<Self>) {
        self.work(
            "Opening saved metadata in Finder…",
            move || {
                file_actions::reveal_metadata(&target.root, &target.relative)?;
                Ok(JobResult::FileAction {
                    root: target.root,
                    removed: None,
                    changed: false,
                    message: "Saved tags and notes revealed in Finder".into(),
                })
            },
            cx,
        );
    }

    /// Drain trusted read-side clicks; stale document identities cannot redirect a later preview.
    fn receive_links(&mut self, cx: &mut Context<Self>) {
        if self.file_menu.is_some() && preview_has_native_focus(&self.preview) {
            self.file_menu = None;
            self.file_menu_session.active = None;
            cx.notify();
        }
        let events = self
            .link_events
            .lock()
            .map(|mut events| events.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            if self.guide_open
                || !self
                    .preview_state
                    .read()
                    .is_ok_and(|state| state.issuer == event.issuer)
                || self.root.as_ref() != Some(&event.root)
                || self.selected.as_ref() != Some(&event.relative)
            {
                continue;
            }
            match classify_link(&event.relative, &event.url, &event.raw, &event.issuer) {
                Ok(LinkRoute::External(url)) => cx.open_url(&url),
                Ok(LinkRoute::Anchor(anchor)) => {
                    self.document_anchor = Some(anchor);
                    self.reload_preview();
                }
                Ok(LinkRoute::Local { relative, anchor }) => {
                    self.select_document(
                        relative,
                        PreviewLocation {
                            anchor,
                            ..Default::default()
                        },
                        false,
                        cx,
                    );
                }
                Err(error) => self.status = format!("Link blocked: {error:#}"),
            }
            cx.notify();
        }
    }

    /// Rebuild component items from either filesystem children or every indexed occurrence.
    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let selected = self
            .tree_state
            .read(cx)
            .selected_entry()
            .map(|entry| entry.item().id.to_string());
        let mut targets = BTreeMap::new();
        self.tree_items = if self.search_active {
            match_items(
                &self.hits,
                &self.expanded_result_files,
                self.result_file_limit,
                &mut targets,
            )
        } else {
            browse_items(
                &self.directory_cache,
                "",
                &self.expanded_folders,
                &mut targets,
            )
        };
        self.tree_targets = Rc::new(targets);
        let selected = restored_tree_selection(&self.tree_items, selected.as_deref());
        let items = self.tree_items.clone();
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
            state.set_selected_index(selected, cx);
        });
        cx.notify();
    }

    /// Persist the component's expansion state and load newly expanded physical folders.
    fn tree_changed(&mut self, cx: &mut Context<Self>) {
        let mut loads = Vec::new();
        visit_items(&self.tree_items, &mut |item| {
            if let Some(target) = self.tree_targets.get(item.id.as_ref()) {
                if self.search_active && item.is_folder() {
                    if item.is_expanded() {
                        self.expanded_result_files.insert(target.relative.clone());
                    } else {
                        self.expanded_result_files.remove(&target.relative);
                    }
                } else if !self.search_active && target.directory && !target.nested {
                    if item.is_expanded() {
                        self.expanded_folders.insert(target.relative.clone());
                        if !self.directory_cache.contains_key(&target.relative) {
                            loads.push(target.relative.clone());
                        }
                    } else {
                        self.expanded_folders.remove(&target.relative);
                    }
                }
            }
        });
        for folder in loads {
            self.queue_directory(folder, cx);
        }
        cx.notify();
    }

    /// Queue uncapped immediate children while keeping only one filesystem browser job active.
    fn queue_directory(&mut self, folder: String, cx: &mut Context<Self>) {
        if !self.directory_queue.contains(&folder) {
            self.directory_queue.push_back(folder);
        }
        self.next_directory(cx);
    }

    /// Read one directory outside the UI thread and discard callbacks from a prior root.
    fn next_directory(&mut self, cx: &mut Context<Self>) {
        if self.directory_running {
            return;
        }
        let (Some(root), Some(folder)) = (self.root.clone(), self.directory_queue.pop_front())
        else {
            return;
        };
        self.directory_running = true;
        let epoch = self.directory_epoch;
        let source = folder.clone();
        let task = cx
            .background_executor()
            .spawn(async move { file_actions::directory_children(&root, &source) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if epoch != this.directory_epoch {
                    return;
                }
                this.directory_running = false;
                if epoch == this.directory_epoch {
                    match result {
                        Ok(entries) => {
                            if this.expand_all {
                                for entry in &entries {
                                    if entry.is_directory && !entry.is_nested_root {
                                        this.expanded_folders.insert(entry.relative.clone());
                                        if !this.directory_cache.contains_key(&entry.relative)
                                            && !this.directory_queue.contains(&entry.relative)
                                        {
                                            this.directory_queue.push_back(entry.relative.clone());
                                        }
                                    }
                                }
                            }
                            let changed = this
                                .directory_cache
                                .get(&folder)
                                .is_none_or(|previous| !same_directory_entries(previous, &entries));
                            if changed {
                                this.directory_cache.insert(folder, entries);
                                if !this.search_active {
                                    this.rebuild_tree(cx);
                                }
                            }
                        }
                        Err(error) => {
                            this.status = format!("Folder browse: {error:#}");
                            cx.notify();
                        }
                    }
                }
                this.next_directory(cx);
            });
        })
        .detach();
    }

    /// Refresh the root and already loaded folders without recursively scanning hidden subtrees.
    fn refresh_directories(&mut self, cx: &mut Context<Self>) {
        if self.root.is_none() {
            return;
        }
        let mut folders = vec![String::new()];
        folders.extend(
            self.directory_cache
                .keys()
                .filter(|folder| !folder.is_empty())
                .cloned(),
        );
        for folder in folders {
            if !self.directory_queue.contains(&folder) {
                self.directory_queue.push_back(folder);
            }
        }
        self.next_directory(cx);
    }

    /// Explicitly remove both query and tag filters and restore the live filesystem view.
    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.tags_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.search_active = false;
        self.applied_search = None;
        self.search_lane.clear();
        self.search_revision += 1;
        self.hover_hint.clear();
        self.status = "All files · live folder browsing".into();
        self.rebuild_tree(cx);
        self.refresh_directories(cx);
        self.query_input.read(cx).focus_handle(cx).focus(window);
    }

    /// Expand all physical folders on demand, or collapse them without losing the selected preview.
    fn expand_folders(&mut self, expand: bool, cx: &mut Context<Self>) {
        if self.search_active {
            self.expanded_result_files.clear();
            if expand {
                self.expanded_result_files
                    .extend(self.hits.iter().map(|hit| hit.path.clone()));
            }
        } else {
            self.expand_all = expand;
            self.expanded_folders.clear();
            if expand {
                let folders = self
                    .directory_cache
                    .values()
                    .flatten()
                    .filter(|entry| entry.is_directory && !entry.is_nested_root)
                    .map(|entry| entry.relative.clone())
                    .collect::<Vec<_>>();
                for folder in folders {
                    self.expanded_folders.insert(folder.clone());
                    self.queue_directory(folder, cx);
                }
            }
        }
        self.rebuild_tree(cx);
    }

    /// Dispatch a confined native source action; clipboard operations remain on the main thread.
    fn source_action(
        &mut self,
        target: SourceTarget,
        action: &'static str,
        cx: &mut Context<Self>,
    ) {
        match action {
            "copy-file" => {
                self.status = match file_actions::copy_file(&target.root, &target.relative) {
                    Ok(()) => "File copied to macOS clipboard".into(),
                    Err(error) => format!("{error:#}"),
                };
                cx.notify();
            }
            "copy-relative" => {
                cx.write_to_clipboard(ClipboardItem::new_string(target.relative));
                self.status = "Relative path copied".into();
                cx.notify();
            }
            "copy-absolute" => {
                match file_actions::source_path(&target.root, &target.relative) {
                    Ok(path) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            path.to_string_lossy().into_owned(),
                        ));
                        self.status = "Absolute path copied".into();
                    }
                    Err(error) => self.status = format!("{error:#}"),
                };
                cx.notify();
            }
            "metadata" => {
                if self.root.as_ref() == Some(&target.root) {
                    if self.selected.as_ref() == Some(&target.relative) {
                        self.metadata_open = true;
                        cx.notify();
                    } else {
                        self.select_document(target.relative, PreviewLocation::default(), true, cx);
                    }
                }
            }
            _ => self.work(
                "Opening native viewer…",
                move || {
                    match action {
                        "quick-look" => file_actions::quick_look(&target.root, &target.relative)?,
                        "reveal" => file_actions::reveal(&target.root, &target.relative)?,
                        "open-root" => file_actions::open_root(&target.root)?,
                        _ => file_actions::open_original(&target.root, &target.relative)?,
                    }
                    Ok(JobResult::FileAction {
                        root: target.root,
                        removed: None,
                        changed: false,
                        message:
                            "Native launch request accepted · viewer availability depends on macOS"
                                .into(),
                    })
                },
                cx,
            ),
        }
    }

    /// Confirm the immutable source identity before a worker requests native Move to Trash.
    fn confirm_trash(&mut self, target: SourceTarget, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let weak = cx.weak_entity();
        let close = weak.clone();
        let path = target
            .root
            .join(&target.relative)
            .to_string_lossy()
            .into_owned();
        let _ = self.update_preview_visibility(false);
        window.open_dialog(cx, move |dialog, _, _| {
            let target=target.clone();let weak=weak.clone();let close=close.clone();
            dialog.title("Move file to Trash?").confirm()
            .button_props(DialogButtonProps::default().ok_text("Move to Trash").ok_variant(ButtonVariant::Danger).cancel_text("Cancel"))
            .child(div().text_size(px(13.)).child(format!("{path}\nThe original file will move to macOS Trash. Its knowledge metadata remains. Refresh the index afterwards.")))
            .on_ok(move |_, _, cx| {
                let target=target.clone();
                let _=weak.update(cx, |this,cx| this.work("Moving source to Trash…", move || {
                    let receipt=file_actions::trash_file(&target.root,&target.relative)?;
                    let message=receipt.trashed_path.map(|path|format!("Moved to Trash: {} · refresh index",path.display())).unwrap_or_else(||"Moved to macOS Trash · refresh index".into());
                    Ok(JobResult::FileAction {root:target.root,removed:Some(target.relative),changed:true,message})
                },cx)); true
            })
            .on_close(move |_, _, cx| { let _=close.update(cx,|this,cx| { let _=this.update_preview_visibility(true); cx.notify(); }); })
        });
        cx.notify();
    }

    /// Show actual component menu actions within the native sidebar, clear of the WebKit child.
    fn file_context(&mut self, target: TreeTarget, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.selected_folder = if target.directory {
            target.relative.clone()
        } else {
            parent_folder(&target.relative)
        };
        let selected = context_tree_selection(&self.tree_items, &self.tree_targets, &target);
        self.tree_state
            .update(cx, |state, cx| state.set_selected_index(selected, cx));
        if !target.directory {
            let start = target
                .hit_index
                .and_then(|index| self.hits.get(index))
                .and_then(|hit| hit.start_seconds);
            self.select(
                target.relative.clone(),
                start,
                target
                    .hit_index
                    .and_then(|index| self.hits.get(index))
                    .and_then(|hit| hit.line),
                cx,
            );
        }
        self.focus_native_input(cx);
        let revision = self.file_menu_session.open();
        let weak = cx.weak_entity();
        let source = SourceTarget {
            root,
            relative: target.relative.clone(),
        };
        let menu = PopupMenu::build(window, cx, |mut menu, _, _| {
            menu = menu
                .min_w(px(260.))
                .max_w(px(260.))
                .action_context(self.file_focus.clone());
            if !target.directory {
                for (label, action) in [
                    ("Quick Look", "quick-look"),
                    ("Open original (default app)", "open"),
                    ("Reveal in Finder", "reveal"),
                    ("File details", "metadata"),
                    ("Copy file · ⌘C", "copy-file"),
                    ("Copy absolute path", "copy-absolute"),
                    ("Copy relative path", "copy-relative"),
                ] {
                    let weak = weak.clone();
                    let source = source.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.source_action(source.clone(), action, cx)
                        });
                    }));
                }
                let weak = weak.clone();
                let source = source.clone();
                menu = menu
                    .separator()
                    .item(
                        PopupMenuItem::new("Move to Trash…").on_click(move |_, window, cx| {
                            let weak = weak.clone();
                            let source = source.clone();
                            window.defer(cx, move |window, cx| {
                                let _ = weak
                                    .update(cx, |this, cx| this.confirm_trash(source, window, cx));
                            });
                        }),
                    );
            } else if target.nested {
                menu = menu
                    .item(PopupMenuItem::new("Independent child knowledge root").disabled(true));
            } else {
                let weak = weak.clone();
                let folder = target.relative.clone();
                let destination_root = source.root.clone();
                menu = menu.item(PopupMenuItem::new("Paste files here · ⌘V").on_click(
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.paste_into(destination_root.clone(), folder.clone(), cx)
                        });
                    },
                ));
            }
            for (label, expand) in [
                ("Expand all folders", true),
                ("Collapse all folders", false),
            ] {
                let weak = weak.clone();
                menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.expand_folders(expand, cx));
                }));
            }
            menu
        });
        cx.subscribe(&menu, move |this, _, _: &gpui::DismissEvent, cx| {
            if this.file_menu_session.dismiss(revision) {
                this.file_menu = None;
                cx.notify();
            }
        })
        .detach();
        menu.focus_handle(cx).focus(window);
        self.file_menu = Some(menu);
        cx.notify();
    }

    /// Paste only native file URLs, keeping ordinary text clipboard input in the input component.
    fn paste_files(&mut self, folder: String, cx: &mut Context<Self>) {
        match file_actions::read_files_from_clipboard() {
            Ok(paths) if !paths.is_empty() => self.copy_sources(paths, folder, cx),
            Ok(_) => {
                self.status = "Clipboard contains no native files; Copy path is plain text".into();
                cx.notify();
            }
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
            }
        }
    }

    /// Refuse a delayed native paste before reading the clipboard if its original root changed.
    fn paste_into(&mut self, root: PathBuf, folder: String, cx: &mut Context<Self>) {
        if !transfer_target_current(self.root.as_deref(), &root) {
            self.status = "Paste target changed; select the destination again".into();
            cx.notify();
            return;
        }
        self.paste_files(folder, cx);
    }

    /// Retain an external drop or file-picker destination across asynchronous root transitions.
    fn copy_sources_into(
        &mut self,
        sources: Vec<PathBuf>,
        root: PathBuf,
        folder: String,
        cx: &mut Context<Self>,
    ) {
        if !transfer_target_current(self.root.as_deref(), &root) {
            self.status = "Copy target changed; select the destination again".into();
            cx.notify();
            return;
        }
        self.copy_sources(sources, folder, cx);
    }

    /// Retain the destination root as well as the typed internal drag source.
    fn move_source_into(
        &mut self,
        source: SourceTarget,
        root: PathBuf,
        folder: String,
        cx: &mut Context<Self>,
    ) {
        if !transfer_target_current(self.root.as_deref(), &root) {
            self.status = "Move target changed; select the destination again".into();
            cx.notify();
            return;
        }
        self.move_source(source, folder, cx);
    }

    /// Copy an explicit batch into a folder and report each failure without overwriting a source.
    fn copy_sources(&mut self, sources: Vec<PathBuf>, folder: String, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.work(
            "Copying files…",
            move || {
                let mut completed = 0;
                let mut failures = Vec::new();
                for source in sources {
                    match file_actions::copy_into(&root, &source, &folder) {
                        Ok(_) => completed += 1,
                        Err(error) => failures.push(format!("{}: {error:#}", source.display())),
                    }
                }
                Ok(JobResult::FileAction {
                    root,
                    removed: None,
                    changed: completed > 0,
                    message: format!(
                        "Copied {completed} files · {} failed · refresh index{}",
                        failures.len(),
                        if failures.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", failures.join("; "))
                        }
                    ),
                })
            },
            cx,
        );
    }

    /// Move only an immutable source from this root to an explicitly dropped owned folder.
    fn move_source(&mut self, source: SourceTarget, folder: String, cx: &mut Context<Self>) {
        if self.root.as_ref() != Some(&source.root) {
            self.status = "Internal move requires the same knowledge root".into();
            cx.notify();
            return;
        }
        self.work(
            "Moving file…",
            move || {
                let receipt = file_actions::move_within(&source.root, &source.relative, &folder)?;
                Ok(JobResult::FileAction {
                    root: source.root,
                    removed: Some(source.relative),
                    changed: true,
                    message: format!("Moved to {} · refresh index", receipt.relative),
                })
            },
            cx,
        );
    }

    /// Copy the keyboard-selected regular file, never a preview left selected after folder browsing.
    fn copy_tree_selection(&mut self, cx: &mut Context<Self>) {
        let relative = self
            .tree_state
            .read(cx)
            .selected_entry()
            .and_then(|entry| self.tree_targets.get(entry.item().id.as_ref()))
            .filter(|target| !target.directory)
            .map(|target| target.relative.clone());
        if let (Some(root), Some(relative)) = (self.root.clone(), relative) {
            self.source_action(SourceTarget { root, relative }, "copy-file", cx);
        } else {
            self.status = "Select a regular file in the tree to copy".into();
            cx.notify();
        }
    }

    /// Navigate visible rows while preserving the component's retained expansion states.
    fn file_key(&mut self, direction: i8, cx: &mut Context<Self>) {
        /// Flatten currently expanded rows in the component display order.
        fn visible(items: &[TreeItem], rows: &mut Vec<TreeItem>) {
            for item in items {
                rows.push(item.clone());
                if item.is_expanded() {
                    visible(&item.children, rows);
                }
            }
        }
        let mut rows = Vec::new();
        visible(&self.tree_items, &mut rows);
        if rows.is_empty() {
            return;
        }
        let previous = self.tree_state.read(cx).selected_index();
        let index = match direction {
            -1 => previous
                .map(|index| index.saturating_sub(1))
                .unwrap_or(rows.len() - 1),
            1 => previous
                .map(|index| (index + 1).min(rows.len() - 1))
                .unwrap_or(0),
            _ => previous.unwrap_or(0).min(rows.len() - 1),
        };
        let item = &rows[index];
        if matches!(direction, -2 | 2) && item.is_folder() {
            item.clone().expanded(direction == 2);
            let items = self.tree_items.clone();
            self.tree_state
                .update(cx, |state, cx| state.set_items(items, cx));
        }
        self.tree_state.update(cx, |state, cx| {
            state.set_selected_index(Some(index), cx);
            state.scroll_to_item(index, gpui::ScrollStrategy::Center);
        });
        if let Some(target) = self.tree_targets.get(item.id.as_ref()).cloned() {
            self.selected_folder = if target.directory {
                target.relative.clone()
            } else {
                parent_folder(&target.relative)
            };
            if direction == 0 && !target.directory && !item.is_disabled() {
                let time = target
                    .hit_index
                    .and_then(|index| self.hits.get(index))
                    .and_then(|hit| hit.start_seconds);
                self.select(
                    target.relative,
                    time,
                    target
                        .hit_index
                        .and_then(|index| self.hits.get(index))
                        .and_then(|hit| hit.line),
                    cx,
                );
            }
        }
        cx.notify();
    }

    /// Render only visible tree rows through the component's uniform-list virtualization.
    fn source_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let targets = self.tree_targets.clone();
        let root = self.root.clone();
        let hits = self.hits.clone();
        let selected = self
            .preview_target
            .as_ref()
            .filter(|target| self.root.as_ref() == Some(&target.root))
            .map(|target| target.relative.clone())
            .or_else(|| self.selected.clone());
        tree(&self.tree_state, move |ix, entry, row_selected, _, _| {
            let target = targets.get(entry.item().id.as_ref()).cloned();
            let mut row = div()
                .id(("source-content", ix))
                .flex()
                .items_center()
                .gap_1()
                .w_full()
                .h(px(32.))
                .cursor_pointer();
            row = row.child(
                Icon::new(if entry.is_folder() {
                    if entry.is_expanded() {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    }
                } else {
                    IconName::File
                })
                .size(px(14.)),
            );
            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .child(entry.item().label.clone()),
            );
            if let (Some(target), Some(root)) = (target, root.clone()) {
                let folder = if target.directory {
                    target.relative.clone()
                } else {
                    parent_folder(&target.relative)
                };
                let source = SourceTarget {
                    root,
                    relative: target.relative.clone(),
                };
                let hint_id = format!("source-row:{}", entry.item().id);
                let detail = target
                    .hit_index
                    .and_then(|index| hits.get(index))
                    .map(|hit| format!("{} · {} · {}", hit.path, evidence_label(hit), hit.snippet))
                    .unwrap_or_else(|| target.relative.clone());
                let hover_weak = weak.clone();
                row = row.on_hover(move |hovered, _, cx| {
                    let _ = hover_weak.update(cx, |this, cx| {
                        this.hover_hint.update(&hint_id, *hovered, &detail);
                        cx.notify();
                    });
                });
                let selected = if target.hit_index.is_some() {
                    row_selected
                } else {
                    selected.as_ref() == Some(&target.relative) && !target.directory
                };
                if target.nested {
                    row = row.child(div().text_size(px(10.)).child("child KB"));
                }
                let menu_weak = weak.clone();
                let menu_target = target.clone();
                row = row.on_mouse_down(MouseButton::Right, move |_, window, cx| {
                    let _ = menu_weak.update(cx, |this, cx| {
                        this.file_context(menu_target.clone(), window, cx)
                    });
                    cx.stop_propagation();
                });
                if !target.nested {
                    let drop_weak = weak.clone();
                    let dest = folder.clone();
                    let destination_root = source.root.clone();
                    row = row.on_drop(move |paths: &ExternalPaths, _, cx| {
                        let _ = drop_weak.update(cx, |this, cx| {
                            this.copy_sources_into(
                                paths.paths().to_vec(),
                                destination_root.clone(),
                                dest.clone(),
                                cx,
                            )
                        });
                        cx.stop_propagation();
                    });
                    let move_weak = weak.clone();
                    let dest = folder.clone();
                    let destination_root = source.root.clone();
                    row = row.on_drop(move |source: &SourceTarget, _, cx| {
                        let _ = move_weak.update(cx, |this, cx| {
                            this.move_source_into(
                                source.clone(),
                                destination_root.clone(),
                                dest.clone(),
                                cx,
                            )
                        });
                        cx.stop_propagation();
                    });
                }
                if !target.directory {
                    let name = entry.item().label.to_string();
                    row = row.on_drag(source, move |_, _, _, cx| {
                        cx.new(|_| FileDrag(name.clone()))
                    });
                }
                let click_weak = weak.clone();
                let start = target
                    .hit_index
                    .and_then(|index| hits.get(index))
                    .and_then(|hit| hit.start_seconds);
                let line = target
                    .hit_index
                    .and_then(|index| hits.get(index))
                    .and_then(|hit| hit.line);
                let folder_target = folder.clone();
                let click = target.clone();
                row = row.on_click(move |_, window, cx| {
                    let _ = click_weak.update(cx, |this, cx| {
                        this.file_focus.focus(window);
                        this.selected_folder = folder_target.clone();
                        if !click.directory {
                            this.select(click.relative.clone(), start, line, cx);
                        }
                        cx.notify();
                    });
                });
                ListItem::new(ix)
                    .h(px(32.))
                    .pl(px(8. + entry.depth() as f32 * 14.))
                    .pr_2()
                    .bg(rgb(if selected { 0xdfeade } else { 0xf8faf7 }))
                    .child(row)
            } else {
                ListItem::new(ix)
                    .h(px(32.))
                    .pl(px(8. + entry.depth() as f32 * 14.))
                    .child(row)
            }
        })
        .size_full()
    }

    /// Render native environment actions beside an inert, offline Chinese usage guide.
    fn guide(&self, cx: &mut Context<Self>) -> gpui::Div {
        let webview = self.preview.clone();
        let surface = self.preview_surface.clone();
        let runtime = self.runtime_status.as_ref();
        let brew = runtime.and_then(|status| status.homebrew.as_deref());
        let ffmpeg = runtime.and_then(|status| status.ffmpeg.as_deref());
        let ffprobe = runtime.and_then(|status| status.ffprobe.as_deref());
        let whisper = runtime.and_then(|status| status.whisper_cli.as_deref());
        let checked = runtime.is_some();
        div().size_full().flex().flex_col().bg(rgb(0xf2f5ef)).text_color(rgb(0x263a2d)).text_size(px(14.))
            .child(self.header(cx))
            .child(div().flex().flex_1().min_h_0()
                .child(div().w(px(380.)).flex_shrink_0().border_r_1().border_color(rgb(0xd4dfd6)).id("environment").overflow_y_scroll().p_4()
                    .child(div().text_size(px(21.)).font_weight(gpui::FontWeight::BOLD).child("本机环境"))
                    .child(div().mt_2().mb_4().text_size(px(12.)).text_color(rgb(0x657b6b)).child("检索和文档预览内置。下面的工具只为视频切片、音频转换和可选语音识别提供支持。"))
                    .child(environment_card("Homebrew", brew, checked, "用于安装以下两项可选工具。"))
                    .child(button("brew-guide", "打开 Homebrew 官方说明", false).mt_2().on_click(cx.listener(|_, _, _, cx| cx.open_url(runtime::HOMEBREW_INSTALL_URL))))
                    .child(environment_card("FFmpeg", ffmpeg, checked, "视频切片和音频转换；安装后同时检查 ffprobe。"))
                    .child(div().mt_1().text_size(px(11.)).text_color(rgb(0x657b6b)).child(format!("ffprobe：{}", tool_location(ffprobe, checked))))
                    .child(button("install-ffmpeg", "在终端安装 FFmpeg", false).mt_2().opacity(if self.busy { 0.45 } else { 1.0 }).on_click(cx.listener(|this, _, _, cx| this.install_runtime(BrewPackage::FFmpeg, cx))))
                    .child(environment_card("whisper.cpp", whisper, checked, "可选本地语音识别。找到 whisper-cli 不代表已经配置语音模型。"))
                    .child(button("install-whisper", "在终端安装 whisper.cpp", false).mt_2().opacity(if self.busy { 0.45 } else { 1.0 }).on_click(cx.listener(|this, _, _, cx| this.install_runtime(BrewPackage::WhisperCpp, cx))))
                    .child(button("refresh-runtime", "重新检查", false).mt_4().opacity(if self.busy { 0.45 } else { 1.0 }).on_click(cx.listener(|this, _, _, cx| this.refresh_runtime(cx))))
                    .child(div().mt_5().p_3().rounded_md().bg(rgb(0xe5eee6)).text_size(px(12.)).child("安装在可见的终端窗口中运行，那里提供进度、输出和交互输入。它与本应用独立运行；完成后点击“重新检查”。本地模型仍需单独配置，按钮不会下载模型、启动服务或修改知识库。"))
                    .child(div().mt_5().font_weight(gpui::FontWeight::SEMIBOLD).child("依赖检查与安装交接"))
                    .child(div().mt_2().text_size(px(11.)).text_color(rgb(0x52665a)).child(self.runtime_log.clone())))
                .child(canvas(|_, _, _| (), move |bounds, _, _, _| {let _=update_preview_bounds(&webview,&surface,bounds);}).flex_1().h_full()))
            .child(self.status_footer(cx))
    }
}

impl Render for LibraryWindow {
    /// Lay out compact retrieval, a live virtualized file tree, and confined source previews.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let _ = self.update_preview_visibility(!window.has_active_dialog(cx));
        if self.guide_open {
            return self.guide(cx).children(dialog_layer).into_any_element();
        }
        let webview = self.preview.clone();
        let surface = self.preview_surface.clone();
        let files = if self.search_active {
            self.hits
                .iter()
                .map(|hit| &hit.path)
                .collect::<BTreeSet<_>>()
                .len()
        } else {
            self.directory_cache
                .values()
                .flatten()
                .filter(|entry| !entry.is_directory)
                .count()
        };
        let popup = self.file_menu.clone();
        let menu_revision = self.file_menu_session.active;
        let drop_root = self.root.clone();
        let move_root = drop_root.clone();
        let sidebar = div()
            .w(px(300.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .relative()
            .border_r_1()
            .border_color(rgb(0xd4dfd6))
            .id("file-sidebar")
            .key_context("OmniFiles")
            .track_focus(&self.file_focus)
            .on_action(cx.listener(|this, _: &CopyFile, _, cx| this.copy_tree_selection(cx)))
            .on_action(cx.listener(|this, _: &PasteFiles, _, cx| {
                this.paste_files(this.selected_folder.clone(), cx)
            }))
            .on_action(cx.listener(|this, _: &FileUp, _, cx| this.file_key(-1, cx)))
            .on_action(cx.listener(|this, _: &FileDown, _, cx| this.file_key(1, cx)))
            .on_action(cx.listener(|this, _: &FileLeft, _, cx| this.file_key(-2, cx)))
            .on_action(cx.listener(|this, _: &FileRight, _, cx| this.file_key(2, cx)))
            .on_action(cx.listener(|this, _: &FileOpen, _, cx| this.file_key(0, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_native_input(cx);
                    this.file_focus.focus(window);
                }),
            )
            .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                if let Some(root) = &drop_root {
                    this.copy_sources_into(paths.paths().to_vec(), root.clone(), String::new(), cx);
                }
            }))
            .on_drop(cx.listener(move |this, source: &SourceTarget, _, cx| {
                if let Some(root) = &move_root {
                    this.move_source_into(source.clone(), root.clone(), String::new(), cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .h(px(34.))
                    .flex_shrink_0()
                    .px_2()
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, window, cx| {
                            this.file_context(
                                TreeTarget {
                                    relative: String::new(),
                                    directory: true,
                                    nested: false,
                                    hit_index: None,
                                },
                                window,
                                cx,
                            )
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .text_size(px(12.))
                            .child(if self.search_active {
                                "Results"
                            } else {
                                "Files"
                            }),
                    )
                    .child(
                        div()
                            .id("result-count")
                            .text_size(px(10.))
                            .text_color(rgb(0x748a79))
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                let detail = if this.search_active {
                                    search_result_summary(&this.hits, this.applied_search.as_ref().map(|s| s.effective_mode()).unwrap_or(this.mode), this.result_file_limit)
                                } else {
                                    format!("{files} live files loaded from opened folders · Expand folders to load their contents")
                                };
                                this.hover_hint.update("result-count", *hovered, &detail);
                                cx.notify();
                            }))
                            .child(if self.search_active {
                                format!(
                                    "{} / {files}{}",
                                    self.result_file_limit.min(files),
                                    if self.applied_search.as_ref().is_some_and(|search| matches!(
                                        search.effective_mode(),
                                        SearchMode::Semantic | SearchMode::Hybrid
                                    )) {
                                        " candidates"
                                    } else {
                                        " files"
                                    }
                                )
                            } else {
                                format!("{files} loaded")
                            }),
                    )
                    .child(
                        button("import", "", false)
                            .icon(IconName::Plus)
                            .disabled(self.root.is_none() || self.busy)
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                this.hover_hint.update(
                                    "import",
                                    *hovered,
                                    "Copy files into the selected folder · Originals stay in place",
                                );
                                cx.notify();
                            }))
                            .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
                    )
                    .child(
                        self.action_icon(
                            "tree-expand",
                            "＋",
                            "Expand all folders or result evidence",
                            false,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.expand_folders(true, cx))),
                    )
                    .child(
                        self.action_icon(
                            "tree-collapse",
                            "−",
                            "Collapse all folders or result evidence",
                            false,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.expand_folders(false, cx))),
                    ),
            )
            .child(div().flex_1().min_h_0().child(self.source_tree(cx)))
            .when(
                self.search_active && files > self.result_file_limit,
                |sidebar| {
                    sidebar.child(
                        button("more-results", "Show 50 more files", false)
                            .m_2()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.result_file_limit =
                                    this.result_file_limit.saturating_add(RESULT_PAGE_SIZE);
                                this.status = search_result_summary(&this.hits, this.applied_search.as_ref().map(|s| s.effective_mode()).unwrap_or(this.mode), this.result_file_limit);
                                this.rebuild_tree(cx);
                            })),
                    )
                },
            )
            .when(popup.is_some(), |sidebar| {
                sidebar.child(
                    div()
                        .id("file-context-popover")
                        .absolute()
                        .top(px(36.))
                        .left(px(6.))
                        .w(px(260.))
                        .children(popup)
                        .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                            if menu_revision
                                .is_some_and(|revision| this.file_menu_session.dismiss(revision))
                            {
                                this.file_menu = None;
                                cx.notify();
                            }
                        })),
                )
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f5ef))
            .text_color(rgb(0x263a2d))
            .text_size(px(14.))
            .child(self.header(cx))
            .when(self.library_panel_open, |layout| {
                layout.child(self.library_panel(cx))
            })
            .when(self.search_options_open, |layout| {
                layout.child(self.search_options(cx))
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .border_t_1()
                    .border_color(rgb(0xd4dfd6))
                    .child(sidebar)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(self.preview_controls(cx))
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, _, _, _| {
                                        let _ = update_preview_bounds(&webview, &surface, bounds);
                                    },
                                )
                                .flex_1()
                                .min_h_0()
                                .w_full(),
                            ),
                    ),
            )
            .child(self.status_footer(cx))
            .children(dialog_layer)
            .into_any_element()
    }
}

/// Construct a consistently styled native button with a stable identifier.
fn button(id: &'static str, label: &'static str, active: bool) -> Button {
    let button = Button::new(id)
        .label(label)
        .small()
        .compact()
        .h(px(CONTROL_HEIGHT))
        .text_size(px(12.));
    if active {
        button.primary()
    } else {
        button.outline()
    }
}

/// Show a detected executable path without claiming that a model or runtime was tested.
fn tool_location(path: Option<&Path>, checked: bool) -> String {
    match path {
        Some(path) => format!("已找到 · {}", path.display()),
        None if checked => "未找到".into(),
        None => "尚未检查".into(),
    }
}

/// Render one optional dependency's detected location and actual purpose.
fn environment_card(
    name: &'static str,
    path: Option<&Path>,
    checked: bool,
    purpose: &'static str,
) -> gpui::Div {
    div()
        .mt_4()
        .p_3()
        .bg(rgb(0xfafcf8))
        .rounded_md()
        .border_1()
        .border_color(rgb(0xd9e3db))
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(name))
        .child(
            div()
                .mt_1()
                .text_size(px(11.))
                .text_color(rgb(if path.is_some() { 0x447553 } else { 0x8b713e }))
                .child(tool_location(path, checked)),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(12.))
                .text_color(rgb(0x657b6b))
                .child(purpose),
        )
}

/// Permit only trusted application pages; imported documents cannot navigate to external origins.
fn internal_navigation(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "omni"
            && url.host_str() == Some("app")
            && matches!(url.path(), "/preview" | "/guide")
    })
}

/// A manually selected link can scroll, open an owned document, or launch the system browser.
#[derive(Debug, PartialEq, Eq)]
enum LinkRoute {
    Anchor(String),
    Local {
        relative: String,
        anchor: Option<String>,
    },
    External(String),
}

/// Reject root escapes before URL normalization, including encoded relative parent components.
fn validate_link_path(current: &str, raw: &str, authority: &str) -> Result<()> {
    let raw = raw.split(['?', '#']).next().unwrap_or_default();
    let absolute = format!("omni://{authority}/");
    let relative_authority = format!("//{authority}/");
    let path = if let Some(rest) = raw
        .strip_prefix(&absolute)
        .or_else(|| raw.strip_prefix(&relative_authority))
    {
        format!("/{rest}")
    } else if raw.starts_with("omni:") || raw.starts_with("//") {
        bail!("link authority mismatch");
    } else {
        raw.to_owned()
    };
    let path = decode_path(&path)?;
    let mut depth = if path.starts_with('/') {
        0
    } else {
        parent_folder(current)
            .split('/')
            .filter(|part| !part.is_empty())
            .count()
    };
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if depth == 0 {
                    bail!("link escapes the knowledge root");
                }
                depth -= 1;
            }
            _ => depth += 1,
        }
    }
    Ok(())
}

/// Classify a trusted click without reading files, accepting only local root paths and plain HTTP(S).
fn classify_link(current: &str, target: &str, raw: &str, authority: &str) -> Result<LinkRoute> {
    let url = url::Url::parse(target)?;
    if !url.username().is_empty() || url.password().is_some() {
        bail!("link credentials are not allowed");
    }
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        return Ok(LinkRoute::External(url.to_string()));
    }
    if url.scheme() != "omni" || url.host_str() != Some(authority) || url.port().is_some() {
        bail!("unsupported link protocol");
    }
    validate_link_path(current, raw, authority)?;
    let relative = decode_path(url.path().trim_start_matches('/'))?;
    if relative
        .split('/')
        .any(|part| matches!(part, ".." | "." | ".omni" | ".git") || part == crate::CONFIG_FILE)
    {
        bail!("link targets protected state");
    }
    let anchor = url.fragment().map(decode_path).transpose()?;
    if relative == current
        && let Some(anchor) = anchor
    {
        return Ok(LinkRoute::Anchor(anchor));
    }
    Ok(LinkRoute::Local {
        relative: relative.trim_end_matches('/').to_owned(),
        anchor,
    })
}

/// Return an owned file's parent folder in root-relative slash notation.
fn parent_folder(relative: &str) -> String {
    relative
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_owned())
        .unwrap_or_default()
}

/// Split comma-delimited exact tag values and remove empty items.
fn parse_tags(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Summarize completed index coverage and expose the first warning when present.
fn index_message(report: &IndexReport) -> String {
    let mut message = format!(
        "Indexed {} files · {} extracted · {} metadata only · {} vectors · {} warnings",
        report.files,
        report.extracted,
        report.metadata_only,
        report.vectors,
        report.warnings.len()
    );
    if let Some(warning) = report.warnings.first() {
        message.push_str(&format!(" · {}", warning));
    }
    message
}

/// Retain coverage warnings for the root that published them, independently of preview feedback.
/// The full first warning and total count match the existing completion report without parsing text.
fn index_warning_feedback(root: &Path, report: &IndexReport) -> Option<(PathBuf, String)> {
    (!report.warnings.is_empty()).then(|| (root.to_path_buf(), index_message(report)))
}

/// Expand a leading home shortcut without changing other path components.
fn expand_home(value: &str) -> PathBuf {
    if let Some(relative) = value.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(relative);
    }
    PathBuf::from(value)
}

/// Escape text inserted into a preview HTML fragment.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Build a protocol URL by encoding each file path component independently.
fn file_url(relative: &str, authority: &str) -> Result<String> {
    if !authority.starts_with("kb-")
        || !authority
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("invalid resource authority");
    }
    let mut url = url::Url::parse(&format!("omni://{authority}/"))?;
    url.path_segments_mut()
        .map_err(|_| anyhow!("invalid preview URL"))?
        .extend(relative.split('/'));
    Ok(url.to_string())
}

/// Allow formats with a known native WebKit image decoder without guessing from image MIME alone.
fn inline_image(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "ico" | "svg"
            )
        })
}

/// Identify camera RAW files even when the generic MIME database labels them as binary data.
fn unsupported_image(path: &Path, mime: &str) -> bool {
    mime.starts_with("image/")
        || path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "raf" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "orf" | "rw2"
                )
            })
}

/// Prepare native media markup or extracted document content for the preview.
fn preview_body(
    path: &Path,
    relative: &str,
    start_seconds: Option<f64>,
    line: Option<usize>,
    authority: &str,
) -> Result<String> {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut url = file_url(relative, authority)?;
    if matches!(
        mime.type_(),
        mime_guess::mime::VIDEO | mime_guess::mime::AUDIO
    ) && let Some(start) = start_seconds.filter(|start| start.is_finite() && *start >= 0.0)
    {
        url.push_str(&format!("#t={start:.3}"));
    }
    let uri = escape(&url);
    let heading = format!("<h2>{}</h2>", escape(relative));
    let svg_text_hit = line.is_some()
        && path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("svg"));
    let body = if svg_text_hit {
        crate::extract::preview_html_at_line(path, 8 * 1024 * 1024, line)?
    } else if inline_image(path) {
        format!("<img src=\"{uri}\" alt=\"{}\">", escape(relative))
    } else if unsupported_image(path, mime.essence_str()) {
        r#"<p class="warning">This image format is not supported by the embedded preview. Use Quick Look or Open original above. The original remains available for metadata indexing.</p>"#.into()
    } else if mime.type_() == mime_guess::mime::VIDEO {
        format!("<video controls preload=\"metadata\" src=\"{uri}\"></video>")
    } else if mime.type_() == mime_guess::mime::AUDIO {
        format!("<audio controls preload=\"metadata\" src=\"{uri}\"></audio>")
    } else if mime.essence_str() == "application/pdf" {
        let text = crate::extract::preview_html_at_line(path, 8 * 1024 * 1024, line)
            .unwrap_or_else(|e| format!("<p>{}</p>", escape(&e.to_string())));
        format!(
            "<object type=\"application/pdf\" data=\"{uri}\"><p>Use Open original for the native PDF viewer.</p></object><details><summary>Extracted text</summary>{text}</details>"
        )
    } else {
        crate::extract::preview_html_at_line(path, 8 * 1024 * 1024, line)?
    };
    Ok(format!("{heading}{body}"))
}

/// Decode percent escapes while rejecting malformed and non-UTF-8 paths.
fn decode_path(path: &str) -> Result<String> {
    let mut bytes = Vec::new();
    let mut input = path.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let a = input
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(|| anyhow!("invalid URL path"))?;
            let b = input
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(|| anyhow!("invalid URL path"))?;
            bytes.push((a * 16 + b) as u8);
        } else {
            bytes.push(byte);
        }
    }
    Ok(String::from_utf8(bytes)?)
}

/// Resolve an owned regular source and reject internal state, child roots, and every source symlink.
fn confined_resource(root: &Path, encoded: &str) -> Result<PathBuf> {
    let relative = decode_path(encoded.trim_start_matches('/'))?;
    file_actions::source_path(root, &relative)
}

/// Parse a single byte range, including suffix requests, for native media playback.
fn byte_range(header: &str, length: u64) -> Result<Range<u64>> {
    if length == 0 {
        bail!("empty resource");
    }
    let value = header
        .strip_prefix("bytes=")
        .ok_or_else(|| anyhow!("unsupported range"))?;
    if value.contains(',') {
        bail!("multiple ranges are unsupported");
    }
    let (start, end) = value
        .split_once('-')
        .ok_or_else(|| anyhow!("invalid range"))?;
    let range = if start.is_empty() {
        let count = end.parse::<u64>()?;
        if count == 0 {
            bail!("empty range");
        }
        length.saturating_sub(count)..length
    } else {
        let start = start.parse::<u64>()?;
        let end = if end.is_empty() {
            length
        } else {
            end.parse::<u64>()?.saturating_add(1).min(length)
        };
        start..end
    };
    if range.start >= length || range.start >= range.end {
        bail!("range is outside resource");
    }
    Ok(range)
}

/// Capture the request's owning root and cheap shared HTML before leaving the foreground callback.
fn protocol_snapshot(state: &Arc<RwLock<PreviewState>>) -> Result<PreviewState> {
    state
        .read()
        .map(|state| state.clone())
        .map_err(|_| anyhow!("preview state unavailable"))
}

/// Return a plain-text denial without holding document state or exposing active content.
fn protocol_error(message: &str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(403)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Cow::Owned(message.as_bytes().to_vec()))
        .unwrap()
}

/// Serve trusted application assets or confined knowledge files without network access.
fn protocol_response(
    request: Request<Vec<u8>>,
    state: &Arc<RwLock<PreviewState>>,
) -> Response<Cow<'static, [u8]>> {
    let response = serve_protocol(&request, state);
    match response {
        Ok(response) => response,
        Err(error) => protocol_error(&error.to_string()),
    }
}

/// Build a protocol response with MIME and range headers; cap individual reads.
fn serve_protocol(
    request: &Request<Vec<u8>>,
    state: &Arc<RwLock<PreviewState>>,
) -> Result<Response<Cow<'static, [u8]>>> {
    let url = url::Url::parse(&request.uri().to_string())?;
    if url.scheme() != "omni" {
        bail!("unsupported protocol");
    }
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        bail!("invalid resource origin");
    }
    let build = |mime: &str, bytes: Cow<'static, [u8]>| -> Result<_> {
        Ok(Response::builder()
            .header("Content-Type", mime)
            .header("Cache-Control", "no-store")
            .body(bytes)?)
    };
    match (url.host_str(), url.path()) {
        (Some("app"), "/vendor/mermaid.min.js") => build(
            "text/javascript",
            Cow::Borrowed(include_bytes!("../assets/vendor/mermaid.min.js")),
        ),
        (Some("app"), "/vendor/highlight.min.js") => build(
            "text/javascript",
            Cow::Borrowed(include_bytes!("../assets/vendor/highlight.min.js")),
        ),
        (Some("app"), "/renderer.js") => {
            build("text/javascript", Cow::Borrowed(RENDERER_JS.as_bytes()))
        }
        (Some("app"), "/guide") => build(
            "text/html; charset=utf-8",
            Cow::Borrowed(include_bytes!("../assets/guide.html")),
        ),
        (Some("app"), "/brand/mark.png") => build(
            "image/png",
            Cow::Borrowed(include_bytes!("../assets/brand/omni-mark.png")),
        ),
        (Some("app"), "/preview") => {
            let (relative, body, issuer) = {
                let state = state
                    .read()
                    .map_err(|_| anyhow!("preview state unavailable"))?;
                (
                    state.relative.clone(),
                    state.body.clone(),
                    state.issuer.clone(),
                )
            };
            let base = if let Some(relative) = relative {
                format!("<base href=\"{}\">", escape(&file_url(&relative, &issuer)?))
            } else {
                String::new()
            };
            let html = include_str!("../assets/preview.html")
                .replace("{{BASE}}", &base)
                .replace("{{CONTENT}}", &body);
            build("text/html; charset=utf-8", Cow::Owned(html.into_bytes()))
        }
        (Some(host), _) if host.starts_with("kb-") => {
            let (root, issuer) = {
                let state = state
                    .read()
                    .map_err(|_| anyhow!("preview state unavailable"))?;
                (
                    state
                        .root
                        .clone()
                        .ok_or_else(|| anyhow!("no knowledge root"))?,
                    state.issuer.clone(),
                )
            };
            if host != issuer {
                bail!("obsolete resource origin");
            }
            let path = confined_resource(&root, url.path())?;
            let mut file = File::open(&path)?;
            let length = file.metadata()?.len();
            let range_header = request.headers().get("Range").and_then(|h| h.to_str().ok());
            let mut range = match range_header {
                Some(header) => byte_range(header, length)?,
                None => 0..length,
            };
            if range_header.is_some() {
                range.end = range
                    .end
                    .min(range.start.saturating_add(PREVIEW_LIMIT as u64));
            } else if range.end - range.start > PREVIEW_LIMIT as u64 {
                bail!("preview resource exceeds 32 MiB; open the original file");
            }
            file.seek(SeekFrom::Start(range.start))?;
            let mut bytes = Vec::new();
            file.take(range.end - range.start).read_to_end(&mut bytes)?;
            let mut response = Response::builder()
                .status(if range_header.is_some() { 206 } else { 200 })
                .header(
                    "Content-Type",
                    mime_guess::from_path(path)
                        .first_or_octet_stream()
                        .essence_str(),
                )
                .header("Accept-Ranges", "bytes")
                .header("Content-Security-Policy", "default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src omni: data:; media-src omni:; connect-src 'none'; object-src 'none'")
                .header("Content-Length", bytes.len())
                .header("Cache-Control", "no-store");
            if range_header.is_some() {
                response = response.header(
                    "Content-Range",
                    format!("bytes {}-{}/{}", range.start, range.end - 1, length),
                );
            }
            Ok(response.body(Cow::Owned(bytes))?)
        }
        _ => bail!("unknown preview resource"),
    }
}

/// Quit through an application action so the native menu also works with WebKit focus.
fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

/// Start the native desktop app and optionally open a knowledge root.
pub fn run(root: Option<PathBuf>) -> Result<()> {
    Application::new()
        .with_assets(crate::gui_assets::GuiAssets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            let theme = Theme::global_mut(cx);
            theme.font_size = px(14.);
            theme.primary = rgb(0x557d61).into();
            theme.primary_hover = rgb(0x426b50).into();
            theme.primary_active = rgb(0x355a41).into();
            theme.primary_foreground = rgb(0xffffff).into();
            theme.ring = rgb(0x719c79).into();
            theme.list_active = rgb(0xdfeade).into();
            theme.list_hover = rgb(0xe8f0e5).into();
            theme.background = rgb(0xf2f5ef).into();
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-c", CopyFile, Some("OmniFiles")),
                KeyBinding::new("cmd-v", PasteFiles, Some("OmniFiles")),
                KeyBinding::new("up", FileUp, Some("OmniFiles")),
                KeyBinding::new("down", FileDown, Some("OmniFiles")),
                KeyBinding::new("left", FileLeft, Some("OmniFiles")),
                KeyBinding::new("right", FileRight, Some("OmniFiles")),
                KeyBinding::new("enter", FileOpen, Some("OmniFiles")),
            ]);
            cx.on_action(quit);
            cx.set_menus(vec![
                Menu {
                    name: "Omni Knowledge".into(),
                    items: vec![
                        MenuItem::os_submenu("Services", SystemMenuType::Services),
                        MenuItem::separator(),
                        MenuItem::action("Quit Omni Knowledge", Quit),
                    ],
                },
                Menu {
                    name: "Edit".into(),
                    items: vec![
                        MenuItem::action("Undo", gpui_component::input::Undo),
                        MenuItem::action("Redo", gpui_component::input::Redo),
                        MenuItem::separator(),
                        MenuItem::action("Cut", gpui_component::input::Cut),
                        MenuItem::action("Copy", gpui_component::input::Copy),
                        MenuItem::action("Paste", gpui_component::input::Paste),
                        MenuItem::action("Select All", gpui_component::input::SelectAll),
                    ],
                },
            ]);
            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let initial_open = root.is_some();
            let _handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(gpui::TitlebarOptions {
                            title: Some("Omni Knowledge".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    move |window, cx| {
                        let view = cx.new(|cx| {
                            LibraryWindow::new(root, window, cx)
                                .expect("create native knowledge window")
                        });
                        let wrapper = cx.new(|cx| Root::new(view.clone(), window, cx));
                        if initial_open {
                            view.update(cx, |this, cx| this.open(false, cx));
                        }
                        wrapper
                    },
                )
                .expect("open native knowledge window");
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic opened-root authority for confined protocol fixtures.
    const TEST_AUTHORITY: &str = "kb-fixture";

    /// Exit clears a dynamically changed diagnostic, but an old control cannot clear newer help.
    #[test]
    fn hover_help_uses_control_identity_across_dynamic_updates() {
        let mut hint = HoverHint::default();
        hint.update("folder-status", true, "Index is missing");
        hint.update("folder-status", false, "Index published with warnings");
        assert!(hint.detail().is_none());

        hint.update("folder-status", true, "Index is missing");
        hint.update("tools-status", true, "Optional tool is absent");
        hint.update("folder-status", false, "Index published with warnings");
        assert_eq!(hint.detail(), Some("Optional tool is absent"));
        hint.update("tools-status", false, "Optional tool was installed");
        assert!(hint.detail().is_none());

        hint.update("actual-root", true, "First root path");
        hint.update("actual-root", true, "Updated root path");
        assert_eq!(hint.detail(), Some("Updated root path"));
        hint.clear();
        assert!(hint.detail().is_none());
    }

    /// Keep a way back to the whole tree for text, tag-only, and cleared-but-published queries.
    #[test]
    fn search_reset_remains_visible_for_every_filter_kind() {
        assert!(!search_filter_is_clearable("", &[], false));
        assert!(search_filter_is_clearable("word", &[], false));
        assert!(search_filter_is_clearable("", &["research".into()], false));
        assert!(search_filter_is_clearable("", &[], true));
        assert_eq!(search_options_label(SearchMode::Bm25, 0), "Search options");
        assert!(search_options_label(SearchMode::Bm25, 1).contains("1 tag"));
        assert!(search_options_label(SearchMode::Semantic, 2).contains("Meaning · 2 tags"));
    }

    /// Keep the applied tag set and mode visible while edited restrictions await an explicit search.
    #[test]
    fn applied_search_summary_does_not_claim_draft_changes() {
        let applied = SearchFilter::new("".into(), SearchMode::Bm25, vec!["research".into()]);
        let empty = SearchFilter::new("".into(), SearchMode::Bm25, vec![]);
        let (label, hint) = search_options_presentation(&empty, Some(&applied));
        assert!(label.contains("1 tag"));
        assert!(label.contains("changes not applied"));
        assert!(hint.contains("Required tags: research"));
        let other = SearchFilter::new("".into(), SearchMode::Bm25, vec!["travel".into()]);
        let (label, hint) = search_options_presentation(&other, Some(&applied));
        assert!(label.contains("1 tag · changes not applied"));
        assert!(hint.contains("Required tags: research"));
        assert!(!hint.contains("Required tags: travel"));
        let text = SearchFilter::new("needle".into(), SearchMode::Bm25, vec![]);
        let meaning = SearchFilter::new("needle".into(), SearchMode::Semantic, vec![]);
        let (label, hint) = search_options_presentation(&meaning, Some(&text));
        assert!(label.contains("changes not applied"));
        assert!(!label.contains("Meaning"));
        assert!(hint.contains("Search mode: Full text"));
        let new_query = SearchFilter::new("different".into(), SearchMode::Bm25, vec![]);
        assert!(
            search_options_presentation(&new_query, Some(&text))
                .1
                .contains("Showing results for: needle")
        );
        assert_eq!(
            search_options_presentation(&empty, None).0,
            "Search options"
        );
        assert!(
            !search_options_presentation(&applied, Some(&applied))
                .0
                .contains("changes not applied")
        );
        assert!(
            search_options_presentation(&other, None)
                .0
                .contains("changes not applied")
        );
        let reordered = SearchFilter::new(
            "".into(),
            SearchMode::Bm25,
            vec!["research".into(), "research".into()],
        );
        assert_eq!(applied, reordered);
    }

    /// Multiline search failures retain a visible prefix and a complete, unchanged hover diagnostic.
    #[test]
    fn compact_status_keeps_error_prefix_without_multiline_clipping() {
        let diagnostic =
            "Search failed: regex parse error:\n    [\n    ^\nerror: unclosed character class";
        let label = single_line_status_label(diagnostic);
        assert!(label.starts_with("Search failed: regex parse error:"));
        assert!(!label.contains('\n'));
        assert!(!label.contains('\r'));
        let mut hint = HoverHint::default();
        hint.update("operation-status", true, diagnostic);
        assert_eq!(hint.detail(), Some(diagnostic));
        assert!(hint.detail().unwrap().contains("\n    ^\n"));
    }

    /// Match common desktop size units and retain exact bytes without asking users to calculate.
    #[test]
    fn file_sizes_show_readable_units_and_exact_evidence() {
        assert_eq!(human_file_size(0), "0 bytes");
        assert_eq!(human_file_size(1), "1 byte");
        assert_eq!(human_file_size(999), "999 bytes");
        assert_eq!(human_file_size(1000), "1.0 KB (1,000 bytes)");
        assert_eq!(human_file_size(37748), "37.7 KB (37,748 bytes)");
        assert_eq!(human_file_size(1_500_000), "1.5 MB (1,500,000 bytes)");
    }

    /// Put familiar format names before technical MIME identifiers without claiming unsupported preview.
    #[test]
    fn file_type_names_keep_unknown_formats_honest() {
        assert_eq!(
            human_file_type("raw/中文.md", "text/markdown"),
            "Markdown (.md)"
        );
        assert_eq!(
            human_file_type("photo.JPG", "image/jpeg"),
            "JPEG image (.JPG)"
        );
        assert_eq!(
            human_file_type("photo.RAF", "application/octet-stream"),
            "RAF file"
        );
        assert_eq!(
            human_file_type("no-extension", "application/octet-stream"),
            "File"
        );
    }

    /// Missing birth dates stay unavailable; known values show local calendar time and its offset.
    #[test]
    fn file_dates_use_local_calendar_time_without_fabricated_birth_dates() {
        assert_eq!(local_file_date(None), "Unavailable");
        let epoch = local_file_date(Some(0));
        assert!(epoch.starts_with("1970-") || epoch.starts_with("1969-"));
        assert!(epoch.contains(" (UTC"));
        assert!(!epoch.contains("Unix"));
    }

    /// Undo to baseline clears unsaved feedback without inventing an authored record.
    #[test]
    fn metadata_feedback_tracks_dirty_undo_and_save_overlap() {
        assert_eq!(metadata_edit_feedback(true, false, true), "Unsaved changes");
        assert_eq!(
            metadata_edit_feedback(false, false, true),
            "Saved in .omni/metadata/"
        );
        assert_eq!(
            metadata_edit_feedback(false, false, false),
            "No saved tags or notes yet"
        );
        assert!(metadata_edit_feedback(true, true, true).contains("not saved"));
        assert_eq!(
            metadata_edit_feedback(false, true, false),
            "Saving tags and notes…"
        );
    }

    /// Create a non-I/O folder observation for footer publication decisions.
    fn observed_folder(root: Option<&Path>, state: FolderState) -> FolderStatus {
        FolderStatus {
            requested: root
                .unwrap_or_else(|| Path::new("/uninitialized"))
                .to_path_buf(),
            root: root.map(Path::to_path_buf),
            state,
            detail: String::new(),
            added: 0,
            modified: 0,
            removed: 0,
            metadata_changed: false,
            indexed_at: None,
        }
    }

    /// Index only a confirmed opened root; edited, invalid, or pending folders require another action.
    #[test]
    fn footer_publication_actions_require_the_opened_owner() {
        let opened = Path::new("/library-a");
        let other = Path::new("/library-b");
        for state in [FolderState::Stale, FolderState::NeedsIndex] {
            let folder = observed_folder(Some(opened), state);
            assert_eq!(
                footer_index_action(Some(opened), Some(&folder), false),
                FooterIndexAction::Index(opened.to_path_buf())
            );
            let draft = observed_folder(Some(other), state);
            for explicit in [false, true] {
                assert_eq!(
                    footer_index_action(Some(opened), Some(&draft), explicit),
                    FooterIndexAction::Library
                );
            }
        }
        let current = observed_folder(Some(opened), FolderState::Current);
        assert_eq!(
            footer_index_action(Some(opened), Some(&current), false),
            FooterIndexAction::Check
        );
        assert_eq!(
            footer_index_action(Some(opened), Some(&current), true),
            FooterIndexAction::Index(opened.to_path_buf())
        );
        for state in [
            FolderState::Uninitialized,
            FolderState::Missing,
            FolderState::NotDirectory,
            FolderState::NotSelected,
            FolderState::Unavailable,
        ] {
            let folder = observed_folder(None, state);
            for explicit in [false, true] {
                assert_eq!(
                    footer_index_action(Some(opened), Some(&folder), explicit),
                    FooterIndexAction::Library
                );
            }
        }
        for explicit in [false, true] {
            assert_eq!(
                footer_index_action(Some(opened), None, explicit),
                FooterIndexAction::Check
            );
            assert_eq!(
                footer_index_action(None, Some(&current), explicit),
                FooterIndexAction::Library
            );
        }
        let draft_current = observed_folder(Some(other), FolderState::Current);
        assert_eq!(
            footer_index_action(Some(opened), Some(&draft_current), true),
            FooterIndexAction::Library
        );
    }

    /// Preserve a successful publication's actual coverage warning while quieting clean completion.
    #[test]
    fn index_coverage_warning_uses_the_report_and_retains_its_owner() {
        let root = Path::new("/library-a");
        let mut report = IndexReport {
            files: 2,
            extracted: 1,
            metadata_only: 1,
            vectors: 0,
            reused: 0,
            warnings: Vec::new(),
            generation: "warning-free".into(),
        };
        assert!(index_warning_feedback(root, &report).is_none());
        let warning = format!(
            "malformed.docx: {}",
            "full extraction diagnostic ".repeat(30)
        );
        report.warnings.push(warning.clone());
        let (owner, feedback) = index_warning_feedback(root, &report).unwrap();
        assert_eq!(owner, root);
        assert!(feedback.contains(&warning));
        report.warnings.clear();
        assert!(index_warning_feedback(root, &report).is_none());
    }

    /// Create immutable source evidence for tree tests without opening a library or native window.
    fn tree_hit(path: &str, line: Option<usize>, start_seconds: Option<f64>) -> SearchHit {
        SearchHit {
            root: PathBuf::from("/test-library"),
            path: path.into(),
            title: "A title different from the file name".into(),
            snippet: "Matched evidence".into(),
            mode: if line.is_some() {
                "grep"
            } else if start_seconds.is_some() {
                "semantic"
            } else {
                "files"
            }
            .into(),
            score: 1.,
            line,
            start_seconds,
            end_seconds: start_seconds.map(|start| start + 5.),
            tags: vec!["test".into()],
        }
    }

    /// Refresh external same-file changes while protecting dirty drafts and newer successful saves.
    #[test]
    fn authored_reads_respect_edit_and_save_revisions() {
        assert!(refresh_authored_metadata(false, false, 4, 4));
        assert!(!refresh_authored_metadata(false, true, 4, 4));
        assert!(!refresh_authored_metadata(false, false, 4, 5));
        assert!(refresh_authored_metadata(true, true, 4, 5));
    }

    /// Persist metadata immediately while leaving the last published search generation unchanged.
    #[test]
    fn authored_save_does_not_start_an_index() -> Result<()> {
        let root = tempfile::tempdir()?;
        let kb = KnowledgeBase::init(root.path(), "Metadata save fixture")?;
        std::fs::write(root.path().join("raw/note.md"), "# Original\n")?;
        kb.index(&IndexOptions::default())?;
        let pointer = std::fs::read(root.path().join(".omni/current.json"))?;
        let mut generations = std::fs::read_dir(root.path().join(".omni/indexes"))?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<std::io::Result<Vec<_>>>()?;
        generations.sort();
        let target = SourceTarget {
            root: kb.root.clone(),
            relative: "raw/note.md".into(),
        };
        save_authored_metadata(&target, &["research".into()], "saved without reindex")?;
        let metadata = kb.metadata("raw/note.md")?;
        assert_eq!(metadata.tags, ["research"]);
        assert_eq!(metadata.description, "saved without reindex");
        let info = crate::file_info::source_info(&kb.root, "raw/note.md")?;
        assert_eq!(info.tags, ["research"]);
        let mut after = std::fs::read_dir(root.path().join(".omni/indexes"))?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<std::io::Result<Vec<_>>>()?;
        after.sort();
        assert_eq!(after, generations);
        assert!(
            kb.list()?
                .iter()
                .find(|record| record.path == "raw/note.md")
                .unwrap()
                .tags
                .is_empty()
        );
        assert_eq!(
            std::fs::read(root.path().join(".omni/current.json"))?,
            pointer
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("raw/note.md"))?,
            "# Original\n"
        );
        kb.index(&IndexOptions::default())?;
        assert_eq!(
            kb.list()?
                .iter()
                .find(|record| record.path == "raw/note.md")
                .unwrap()
                .tags,
            ["research"]
        );
        Ok(())
    }

    /// Dispatch only C after A completes when B and C replace the pending selection.
    #[test]
    fn independent_read_callbacks_accept_only_the_latest_intent() {
        let first = Path::new("/first");
        let second = Path::new("/second");
        let mut lane = CoalescedRead::default();
        lane.enqueue((first, 1));
        assert_eq!(lane.dispatch(), Some((first, 1)));
        lane.enqueue((first, 2));
        lane.enqueue((first, 3));
        assert!(lane.dispatch().is_none());
        assert!(!current_read_request(Some(first), first, 3, 1));
        lane.complete();
        assert_eq!(lane.dispatch(), Some((first, 3)));
        assert!(lane.dispatch().is_none());
        assert!(current_read_request(Some(first), first, 3, 3));
        assert!(!current_read_request(Some(second), first, 3, 3));
        lane.enqueue((first, 4));
        lane.clear();
        lane.complete();
        assert!(lane.dispatch().is_none());
        assert_eq!(
            authored_tags(" research, code,research "),
            ["code", "research"]
        );
    }

    /// Build an immutable query intent for worker scheduling tests without accessing a library.
    fn queued_search(mode: SearchMode, revision: u64) -> SearchRequest {
        SearchRequest {
            root: PathBuf::from("/library"),
            filter: SearchFilter::new("query".into(), mode, Vec::new()),
            options: SearchOptions {
                query: "query".into(),
                mode,
                ..SearchOptions::default()
            },
            revision,
        }
    }

    /// Let local queries finish while an obsolete model query still occupies its bounded worker.
    #[test]
    fn local_search_does_not_wait_for_an_obsolete_model_request() {
        let mut lanes = SearchLanes::default();
        let model = lanes.enqueue(queued_search(SearchMode::Semantic, 1));
        assert_eq!(model, SearchLane::Model);
        assert_eq!(lanes.dispatch(model).unwrap().revision, 1);
        let local = lanes.enqueue(queued_search(SearchMode::Bm25, 2));
        assert_eq!(local, SearchLane::Local);
        assert_eq!(lanes.dispatch(local).unwrap().revision, 2);
        assert!(lanes.is_searching());
        lanes.complete(local, 2);
        assert!(!lanes.is_searching());
        assert!(lanes.model.running);
        assert!(!current_read_request(
            Some(Path::new("/library")),
            Path::new("/library"),
            2,
            1
        ));
        lanes.complete(model, 1);
        assert!(!lanes.is_searching());
        assert!(!lanes.model.running);
    }

    /// Keep only the newest pending intent globally without starting a second worker in one lane.
    #[test]
    fn search_lanes_coalesce_replacements_and_preserve_active_bounds() {
        let mut lanes = SearchLanes::default();
        let model = lanes.enqueue(queued_search(SearchMode::Semantic, 1));
        assert_eq!(lanes.dispatch(model).unwrap().revision, 1);
        lanes.enqueue(queued_search(SearchMode::Hybrid, 2));
        lanes.enqueue(queued_search(SearchMode::Hybrid, 3));
        assert!(lanes.dispatch(model).is_none());
        let local = lanes.enqueue(queued_search(SearchMode::Grep, 4));
        assert_eq!(lanes.dispatch(local).unwrap().revision, 4);
        assert!(lanes.model.pending.is_none());
        lanes.enqueue(queued_search(SearchMode::Files, 5));
        assert!(lanes.dispatch(local).is_none());
        lanes.complete(model, 1);
        assert!(lanes.is_searching());
        assert!(lanes.dispatch(model).is_none());
        lanes.complete(local, 4);
        assert!(lanes.is_searching());
        assert_eq!(lanes.dispatch(local).unwrap().revision, 5);
        lanes.enqueue(queued_search(SearchMode::Bm25, 6));
        lanes.clear();
        assert!(!lanes.is_searching());
        assert!(lanes.local.running);
        assert!(lanes.dispatch(local).is_none());
        lanes.complete(local, 5);
        assert!(lanes.dispatch(local).is_none());
        assert!(!lanes.local.running);
    }

    /// Route effective Files, grep and BM25 locally, reserving the model lane for provider queries.
    #[test]
    fn effective_search_mode_selects_the_worker_lane() {
        for mode in [SearchMode::Files, SearchMode::Grep, SearchMode::Bm25] {
            assert_eq!(SearchLane::for_mode(mode), SearchLane::Local);
        }
        for mode in [SearchMode::Semantic, SearchMode::Hybrid] {
            assert_eq!(SearchLane::for_mode(mode), SearchLane::Model);
        }
    }

    /// Avoid repainting a virtualized tree for identical periodic filesystem child snapshots.
    #[test]
    fn unchanged_directory_snapshots_do_not_invalidate_the_tree() {
        let entry = DirectoryEntry {
            relative: "raw/new.md".into(),
            is_directory: false,
            is_nested_root: false,
        };
        assert!(same_directory_entries(
            std::slice::from_ref(&entry),
            std::slice::from_ref(&entry)
        ));
        assert!(!same_directory_entries(std::slice::from_ref(&entry), &[]));
        let renamed = DirectoryEntry {
            relative: "raw/renamed.md".into(),
            ..entry.clone()
        };
        assert!(!same_directory_entries(
            std::slice::from_ref(&entry),
            &[renamed]
        ));
        let boundary = DirectoryEntry {
            is_directory: true,
            is_nested_root: true,
            ..entry.clone()
        };
        assert!(!same_directory_entries(&[entry], &[boundary]));
    }

    /// Browse every immediate file, empty folder, unindexed file, and child root without a display cap.
    #[test]
    fn live_tree_preserves_empty_folders_and_large_directories() {
        let mut entries = (0..1205)
            .map(|index| DirectoryEntry {
                relative: format!("new-{index}.dat"),
                is_directory: false,
                is_nested_root: false,
            })
            .collect::<Vec<_>>();
        entries.push(DirectoryEntry {
            relative: "empty".into(),
            is_directory: true,
            is_nested_root: false,
        });
        entries.push(DirectoryEntry {
            relative: "child".into(),
            is_directory: true,
            is_nested_root: true,
        });
        let mut cache = BTreeMap::from([(String::new(), entries), ("empty".into(), Vec::new())]);
        cache.insert(
            "child".into(),
            vec![DirectoryEntry {
                relative: "child/private.dat".into(),
                is_directory: false,
                is_nested_root: false,
            }],
        );
        let mut targets = BTreeMap::new();
        let items = browse_items(&cache, "", &BTreeSet::new(), &mut targets);
        assert_eq!(items.len(), 1207);
        assert!(
            items
                .iter()
                .any(|item| item.id.as_ref() == "source:empty" && item.children.is_empty())
        );
        assert!(
            items
                .iter()
                .any(|item| item.id.as_ref() == "source:child" && item.children.is_empty())
        );
        assert!(
            !targets
                .values()
                .any(|target| target.relative == "child/private.dat")
        );
    }

    /// Restore a keyboard file selection by identity across inserts and refuse hidden or removed rows.
    #[test]
    fn refreshed_tree_selection_tracks_visible_identity() {
        let items = vec![
            TreeItem::new("folder", "docs").expanded(true).children([
                TreeItem::new("new-file", "a.md"),
                TreeItem::new("selected-file", "b.md"),
            ]),
            TreeItem::new("other", "raw.bin"),
        ];
        assert_eq!(
            restored_tree_selection(&items, Some("selected-file")),
            Some(2)
        );
        items[0].clone().expanded(false);
        assert_eq!(restored_tree_selection(&items, Some("selected-file")), None);
        assert_eq!(restored_tree_selection(&items, Some("other")), Some(1));
        assert_eq!(restored_tree_selection(&items, Some("removed-file")), None);
        assert_eq!(restored_tree_selection(&items, None), None);
    }

    /// Keep a replacement menu open when the prior menu emits a delayed outside-click dismissal.
    #[test]
    fn right_click_retarget_rejects_obsolete_dismissals() {
        let mut session = MenuSession::default();
        let first = session.open();
        let second = session.open();
        assert!(!session.dismiss(first));
        assert_eq!(session.active, Some(second));
        assert!(session.dismiss(second));
        assert_eq!(session.active, None);
        assert!(!session.dismiss(second));
    }

    /// Right-click another file or another occurrence selects its exact visible tree row.
    #[test]
    fn context_selection_tracks_the_clicked_source_and_evidence() {
        let hits = vec![
            tree_hit("docs/a.md", Some(3), None),
            tree_hit("docs/b.md", Some(7), None),
            tree_hit("docs/b.md", Some(11), None),
        ];
        let mut targets = BTreeMap::new();
        let items = match_items(
            &hits,
            &BTreeSet::from(["docs/b.md".into()]),
            usize::MAX,
            &mut targets,
        );
        assert_eq!(
            context_tree_selection(&items, &targets, &targets["match:0:docs/a.md"]),
            Some(0)
        );
        assert_eq!(
            context_tree_selection(&items, &targets, &targets["match:1:docs/b.md"]),
            Some(1)
        );
        assert_eq!(
            context_tree_selection(&items, &targets, &targets["match:2:docs/b.md"]),
            Some(2)
        );
        items[1].clone().expanded(false);
        assert_eq!(
            context_tree_selection(&items, &targets, &targets["match:2:docs/b.md"]),
            None
        );
    }

    /// Keep every ranked line and time occurrence in the actual component adapter.
    #[test]
    fn component_tree_preserves_all_search_evidence() {
        let hits = vec![
            tree_hit("docs/a.md", Some(3), None),
            tree_hit("docs/a.md", Some(7), None),
            tree_hit("clips/one.mp4", None, Some(0.)),
            tree_hit("clips/one.mp4", None, Some(10.)),
        ];
        let mut targets = BTreeMap::new();
        let items = match_items(&hits, &BTreeSet::new(), usize::MAX, &mut targets);
        let mut indices = Vec::new();
        visit_items(&items, &mut |item| {
            if let Some(index) = targets
                .get(item.id.as_ref())
                .and_then(|target| target.hit_index)
            {
                indices.push(index);
            }
        });
        indices.sort_unstable();
        assert_eq!(indices, [0, 1, 2, 3]);
        assert_eq!(targets["match:3:clips/one.mp4"].hit_index, Some(3));
    }

    /// Keep photo relevance ahead of alphabetically earlier folders and group repeated evidence.
    #[test]
    fn search_file_rows_keep_relevance_instead_of_directory_order() {
        let hits = vec![
            tree_hit("z-photo.jpg", None, None),
            tree_hit("pictures/a-photo.jpg", None, None),
            tree_hit("z-photo.jpg", Some(7), None),
            tree_hit("raw/LICENSE.txt", Some(1), None),
        ];
        let mut targets = BTreeMap::new();
        let items = match_items(&hits, &BTreeSet::new(), usize::MAX, &mut targets);
        let indices: Vec<_> = items
            .iter()
            .map(|item| targets[item.id.as_ref()].hit_index)
            .collect();
        assert_eq!(indices, [Some(0), Some(1), Some(3)]);
        assert!(!items[0].is_expanded());
        assert_eq!(items[0].children.len(), 1);
        assert_eq!(targets[items[0].children[0].id.as_ref()].hit_index, Some(2));
        let page = match_items(&hits, &BTreeSet::new(), 2, &mut BTreeMap::new());
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].children.len(), 1);
        assert!(page[1].label.contains("pictures/a-photo.jpg"));
        let next = match_items(&hits, &BTreeSet::new(), 3, &mut BTreeMap::new());
        assert!(next[2].label.starts_with("#3 raw/LICENSE.txt"));
        assert!(match_items(&[], &BTreeSet::new(), 50, &mut BTreeMap::new()).is_empty());
        assert_eq!(
            search_result_summary(&[], SearchMode::Semantic, 50),
            "0 of 0 candidate files · 0 evidence fragments"
        );
    }

    /// Forward exact indexed lines through the GUI adapter when one source is selected again.
    #[test]
    fn preview_adapter_keeps_exact_source_line_evidence() -> Result<()> {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("lines.md");
        std::fs::write(
            &path,
            "---\ntitle: Lines\n---\n# Header\n\nfirst target\nsecond target\n",
        )?;
        let first = preview_body(&path, "lines.md", None, Some(6), TEST_AUTHORITY)?;
        let second = preview_body(&path, "lines.md", None, Some(7), TEST_AUTHORITY)?;
        assert!(first.contains("id=\"source-line-6\" data-preview-target=\"true\""));
        assert!(second.contains("id=\"source-line-7\" data-preview-target=\"true\""));
        assert!(!second.contains("id=\"source-line-6\" data-preview-target=\"true\""));
        let svg = fixture.path().join("drawing.svg");
        std::fs::write(&svg, "<svg>\n<text>source text</text>\n</svg>\n")?;
        let numbered = preview_body(&svg, "drawing.svg", None, Some(2), TEST_AUTHORITY)?;
        assert!(numbered.contains("id=\"source-line-2\" data-preview-target=\"true\""));
        assert!(!numbered.contains("<img"));
        assert!(preview_body(&svg, "drawing.svg", None, None, TEST_AUTHORITY)?.contains("<img"));
        Ok(())
    }

    /// Permit contained relative links and plain web URLs while refusing escapes and active schemes.
    #[test]
    fn navigation_classification_is_confined() -> Result<()> {
        assert_eq!(
            classify_link(
                "docs/read.md",
                "omni://kb-fixture/docs/read.md#%E7%AB%A0%E8%8A%82",
                "#章节",
                TEST_AUTHORITY
            )?,
            LinkRoute::Anchor("章节".into())
        );
        assert_eq!(
            classify_link(
                "docs/read.md",
                "omni://kb-fixture/wiki/next.md#part",
                "../wiki/next.md#part",
                TEST_AUTHORITY
            )?,
            LinkRoute::Local {
                relative: "wiki/next.md".into(),
                anchor: Some("part".into())
            }
        );
        assert_eq!(
            classify_link(
                "docs/read.md",
                "https://example.org/paper",
                "https://example.org/paper",
                TEST_AUTHORITY
            )?,
            LinkRoute::External("https://example.org/paper".into())
        );
        for (target, raw) in [
            ("file:///etc/passwd", "file:///etc/passwd"),
            ("javascript:alert(1)", "javascript:alert(1)"),
            (
                "https://name:secret@example.org/",
                "https://name:secret@example.org/",
            ),
            ("omni://kb-fixture/private", "../../private"),
            ("omni://kb-fixture/private", "omni://kb-fixture/../private"),
            ("omni://kb-fixture/private", "//kb-fixture/../private"),
            ("omni://kb-other/private", "private"),
            ("omni://kb-fixture/private", "%2e%2e/%2e%2e/private"),
            ("omni://kb-fixture/.omni/index.json", "../.omni/index.json"),
            ("omni://kb-fixture/.git/config", "../.git/config"),
        ] {
            assert!(
                classify_link("docs/read.md", target, raw, TEST_AUTHORITY).is_err(),
                "{raw}"
            );
        }
        Ok(())
    }

    /// Bind document-relative URLs to the source filename and keep heading work in trusted code.
    #[test]
    fn preview_base_uses_the_current_document() -> Result<()> {
        let state = Arc::new(RwLock::new(PreviewState {
            issuer: TEST_AUTHORITY.into(),
            relative: Some("docs/中文 note.md".into()),
            body: "<h2>章节</h2>".into(),
            ..Default::default()
        }));
        let request = Request::builder().uri("omni://app/preview").body(vec![])?;
        let response = serve_protocol(&request, &state)?;
        let body = String::from_utf8(response.body().to_vec())?;
        assert!(
            body.contains("<base href=\"omni://kb-fixture/docs/%E4%B8%AD%E6%96%87%20note.md\">")
        );
        assert!(RENDERER_JS.contains("event.isTrusted"));
        assert!(RENDERER_JS.contains("heading.id = id"));
        Ok(())
    }

    /// Explain unsupported camera formats without producing a broken embedded image request.
    #[test]
    fn raf_preview_has_explicit_native_fallback() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("camera.RAF");
        std::fs::write(&path, b"FUJIFILMCCD-RAW")?;
        let html = preview_body(&path, "camera.RAF", None, None, TEST_AUTHORITY)?;
        assert!(html.contains("not supported by the embedded preview"));
        assert!(html.contains("Quick Look"));
        assert!(!html.contains("<img"));
        assert!(inline_image(Path::new("normal.JPEG")));
        assert!(!inline_image(Path::new("camera.RAF")));
        Ok(())
    }

    /// Page whole files rather than cutting off the best file's repeated evidence fragments.
    #[test]
    fn ranked_file_counts_and_pagination_use_distinct_sources() {
        let mut hits = vec![tree_hit("photo.jpg", None, None); 80];
        hits.push(tree_hit("docs/a.md", Some(3), None));
        let files = ranked_files(&hits);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].indices.len(), 80);
        assert_eq!(files[1].indices, [80]);
        assert_eq!(
            search_result_summary(&hits, SearchMode::Semantic, 1),
            "1 of 2 candidate files · 81 evidence fragments"
        );
        assert!(!search_result_summary(&hits, SearchMode::Semantic, 50).contains("matches"));
        let mut targets = BTreeMap::new();
        let items = match_items(
            &hits,
            &BTreeSet::from(["photo.jpg".into()]),
            usize::MAX,
            &mut targets,
        );
        assert!(items[0].is_expanded());
        assert_eq!(items[0].children.len(), 79);
        assert_eq!(targets[items[1].id.as_ref()].hit_index, Some(80));
    }

    /// Retain collapsed directories across refreshes and clear them when another root is opened.
    #[test]
    fn tree_state_tracks_the_library_root_identity() {
        let root = Path::new("/first-library");
        let mut collapsed = BTreeSet::from(["raw/shared".into()]);
        reset_tree_on_root_change(Some(root), root, &mut collapsed);
        assert!(collapsed.contains("raw/shared"));
        reset_tree_on_root_change(Some(root), Path::new("/second-library"), &mut collapsed);
        assert!(collapsed.is_empty());
    }

    /// Accept the current entered path, including invalid paths, and reject obsolete callbacks.
    #[test]
    fn status_callbacks_follow_the_entered_folder_identity() {
        assert!(status_target_matches(Path::new(""), None));
        assert!(status_target_matches(
            Path::new("/missing-folder"),
            Some(Path::new("/missing-folder"))
        ));
        assert!(!status_target_matches(
            Path::new("/first-library"),
            Some(Path::new("/second-library"))
        ));
        assert!(!status_target_matches(Path::new("/first-library"), None));
    }

    /// Reject encoded traversal and symlink targets outside the selected root.
    #[test]
    fn resources_remain_inside_root() -> Result<()> {
        let root = tempfile::tempdir()?;
        KnowledgeBase::init(root.path(), "Preview fixture")?;
        let outside = tempfile::NamedTempFile::new()?;
        std::fs::write(root.path().join("safe file.txt"), "safe")?;
        assert!(confined_resource(root.path(), "/safe%20file.txt").is_ok());
        assert!(confined_resource(root.path(), "/%2e%2e/secret").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), root.path().join("escape"))?;
            assert!(confined_resource(root.path(), "/escape").is_err());
        }
        Ok(())
    }

    /// Preserve queued resource ownership and reject obsolete URLs after another library opens.
    #[test]
    fn protocol_snapshot_keeps_the_dispatching_library() -> Result<()> {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        KnowledgeBase::init(first.path(), "First resource")?;
        KnowledgeBase::init(second.path(), "Second resource")?;
        std::fs::write(first.path().join("same.txt"), "first")?;
        std::fs::write(second.path().join("same.txt"), "second")?;
        let state = Arc::new(RwLock::new(PreviewState {
            issuer: TEST_AUTHORITY.into(),
            root: Some(first.path().to_path_buf()),
            ..Default::default()
        }));
        let captured = Arc::new(RwLock::new(protocol_snapshot(&state)?));
        {
            let mut current = state.write().unwrap();
            current.root = Some(second.path().to_path_buf());
            current.issuer = "kb-second".into();
        }
        let request = Request::builder()
            .uri("omni://kb-fixture/same.txt")
            .body(vec![])?;
        assert_eq!(
            protocol_response(request, &captured).body().as_ref(),
            b"first"
        );
        for url in ["omni://kb-fixture/same.txt", "omni://kb/same.txt"] {
            let request = Request::builder().uri(url).body(vec![])?;
            assert_eq!(protocol_response(request, &state).status(), 403, "{url}");
        }
        let request = Request::builder()
            .uri("omni://kb-second/same.txt")
            .body(vec![])?;
        assert_eq!(
            protocol_response(request, &state).body().as_ref(),
            b"second"
        );
        Ok(())
    }

    /// Deny automatic media reads of internal files, child-owned sources, and in-root symlinks.
    #[test]
    fn preview_resources_obey_source_ownership() -> Result<()> {
        let root = tempfile::tempdir()?;
        KnowledgeBase::init(root.path(), "Parent fixture")?;
        std::fs::write(root.path().join("visible.png"), b"image")?;
        std::fs::write(root.path().join(".omni/private.png"), b"private")?;
        std::fs::create_dir(root.path().join(".git"))?;
        std::fs::write(root.path().join(".git/private.png"), b"private")?;
        let child = root.path().join("child");
        KnowledgeBase::init(&child, "Child fixture")?;
        std::fs::write(child.join("private.png"), b"child")?;
        let state = Arc::new(RwLock::new(PreviewState {
            issuer: TEST_AUTHORITY.into(),
            root: Some(root.path().to_path_buf()),
            ..Default::default()
        }));
        for relative in [
            ".omni/private.png",
            ".git/private.png",
            "child/private.png",
            crate::CONFIG_FILE,
        ] {
            let request = Request::builder()
                .uri(file_url(relative, TEST_AUTHORITY)?)
                .body(vec![])?;
            assert_eq!(
                protocol_response(request, &state).status(),
                403,
                "{relative}"
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                root.path().join("visible.png"),
                root.path().join("alias.png"),
            )?;
            let request = Request::builder()
                .uri("omni://kb-fixture/alias.png")
                .body(vec![])?;
            assert_eq!(protocol_response(request, &state).status(), 403);
        }
        let request = Request::builder()
            .uri("omni://kb-fixture/visible.png")
            .body(vec![])?;
        assert_eq!(protocol_response(request, &state).status(), 200);
        Ok(())
    }

    /// Refuse a captured folder destination after another root opens, even with the same folder name.
    #[test]
    fn deferred_transfers_keep_the_destination_root() {
        let first = Path::new("/library-a");
        let second = Path::new("/library-b");
        assert!(transfer_target_current(Some(first), first));
        assert!(!transfer_target_current(Some(second), first));
        assert!(!transfer_target_current(None, first));
    }

    /// Check range boundaries used by WKWebView audio and video resource requests.
    #[test]
    fn media_ranges_have_exact_bounds() -> Result<()> {
        assert_eq!(byte_range("bytes=2-5", 10)?, 2..6);
        assert_eq!(byte_range("bytes=7-", 10)?, 7..10);
        assert_eq!(byte_range("bytes=-3", 10)?, 7..10);
        assert!(byte_range("bytes=10-", 10).is_err());
        assert!(byte_range("bytes=0-1,3-4", 10).is_err());
        Ok(())
    }

    /// Serve large open-ended media ranges in bounded responses with truthful byte headers.
    #[test]
    fn large_media_ranges_are_bounded_and_readable() -> Result<()> {
        let root = tempfile::tempdir()?;
        KnowledgeBase::init(root.path(), "Preview fixture")?;
        let source = File::create(root.path().join("large.mp4"))?;
        let length = 40 * 1024 * 1024u64;
        source.set_len(length)?;
        let state = Arc::new(RwLock::new(PreviewState {
            issuer: TEST_AUTHORITY.into(),
            root: Some(root.path().to_path_buf()),
            ..Default::default()
        }));
        for start in [0u64, 2, PREVIEW_LIMIT as u64] {
            let request = Request::builder()
                .uri("omni://kb-fixture/large.mp4")
                .header("Range", format!("bytes={start}-"))
                .body(Vec::new())?;
            let response = serve_protocol(&request, &state)?;
            let count = (length - start).min(PREVIEW_LIMIT as u64);
            assert_eq!(response.status(), 206);
            assert_eq!(response.body().len(), count as usize);
            assert_eq!(
                response.headers()["Content-Length"].to_str()?,
                count.to_string()
            );
            assert_eq!(
                response.headers()["Content-Range"].to_str()?,
                format!("bytes {}-{}/{}", start, start + count - 1, length)
            );
        }
        Ok(())
    }

    /// Preserve literal plus signs and Unicode when round-tripping file protocol paths.
    #[test]
    fn preview_urls_preserve_file_names() -> Result<()> {
        let uri = file_url("notes/中文 + example.md", TEST_AUTHORITY)?;
        let url = url::Url::parse(&uri)?;
        assert_eq!(decode_path(url.path())?, "/notes/中文 + example.md");
        Ok(())
    }

    /// Seek timed audio/video evidence through a safe media fragment without injected scripts.
    #[test]
    fn timed_media_preview_starts_at_result_evidence() -> Result<()> {
        let root = tempfile::tempdir()?;
        let video = root.path().join("clip.mp4");
        let audio = root.path().join("talk.wav");
        std::fs::write(&video, [0, 1])?;
        std::fs::write(&audio, [0, 1])?;
        assert!(
            preview_body(&video, "raw/clip.mp4", Some(12.5), None, TEST_AUTHORITY)?
                .contains("omni://kb-fixture/raw/clip.mp4#t=12.500")
        );
        assert!(
            preview_body(&audio, "raw/talk.wav", Some(3.0), None, TEST_AUTHORITY)?
                .contains("omni://kb-fixture/raw/talk.wav#t=3.000")
        );
        for invalid in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(
                !preview_body(&audio, "raw/talk.wav", Some(invalid), None, TEST_AUTHORITY)?
                    .contains("#t=")
            );
        }
        Ok(())
    }

    /// Keep source scripts inert and restrict renderer scripts to the application origin.
    #[test]
    fn source_resources_cannot_execute_scripts() -> Result<()> {
        let root = tempfile::tempdir()?;
        KnowledgeBase::init(root.path(), "Preview fixture")?;
        std::fs::write(
            root.path().join("source.js"),
            "window.sourceExecuted = true",
        )?;
        let state = Arc::new(RwLock::new(PreviewState {
            issuer: TEST_AUTHORITY.into(),
            root: Some(root.path().to_path_buf()),
            ..Default::default()
        }));
        let request = Request::builder()
            .uri("omni://kb-fixture/source.js")
            .body(Vec::new())?;
        let response = serve_protocol(&request, &state)?;
        assert!(
            response.headers()["Content-Security-Policy"]
                .to_str()?
                .contains("script-src 'none'")
        );
        let request = Request::builder()
            .uri("omni://app/preview")
            .body(Vec::new())?;
        let response = serve_protocol(&request, &state)?;
        let page = std::str::from_utf8(response.body())?;
        assert!(page.contains("script-src omni://app;"));
        assert!(page.contains("connect-src 'none'"));
        Ok(())
    }

    /// Keep the offline guide inert and confine navigation to the two trusted application pages.
    #[test]
    fn guide_is_inert_and_navigation_is_confined() -> Result<()> {
        assert!(internal_navigation("omni://app/guide#start"));
        assert!(internal_navigation("omni://app/preview?v=1"));
        for denied in [
            "https://brew.sh/",
            "file:///private/tmp/example.html",
            "omni://kb-fixture/source.html",
            "omni://app/preview-evil",
        ] {
            assert!(!internal_navigation(denied));
        }
        let state = Arc::new(RwLock::new(PreviewState::default()));
        let request = Request::builder()
            .uri("omni://app/guide")
            .body(Vec::new())?;
        let response = serve_protocol(&request, &state)?;
        let html = std::str::from_utf8(response.body())?;
        assert!(html.contains("script-src 'none'"));
        assert!(html.contains("connect-src 'none'"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("window.ipc"));
        Ok(())
    }
}
