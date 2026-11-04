//! Synthetic library behind the spike's `KnowledgeClient`.
//!
//! The values are fixtures. A node id, a parent id, or a version string
//! grants nothing. This crate does not listen as `omni serve` and it does
//! not store imported bytes.

use serde_json::{Value, json};

/// Rows returned by one `list` call.
pub const LIST_PAGE_SIZE: usize = 2;
/// Rows returned by one `search` call, so two hits require a cursor.
pub const SEARCH_PAGE_SIZE: usize = 1;
/// The text page whose first read is followed by a scripted external edit.
pub const NOTE_ID: &str = "p-note";
/// The library mount the shell selects first.
pub const LIBRARY_ID: &str = "m-library";

struct Item {
    id: &'static str,
    parent: Option<&'static str>,
    title: &'static str,
    kind: &'static str,
    media: &'static str,
    body: Option<String>,
    version: String,
    /// Path served by the shell, without a leading slash.
    content_path: Option<&'static str>,
}

/// The fixture library for one spike process.
pub struct Store {
    origin: String,
    items: Vec<Item>,
    /// `true` after the note's first read has moved its stored version.
    note_bumped: bool,
    imports: u64,
}

impl Store {
    /// `origin` is the loopback origin used as `contentUrl`, for example
    /// `http://127.0.0.1:9`. It is not an authority boundary.
    pub fn new(origin: impl Into<String>) -> Self {
        let text = |value: &str| Some(value.to_owned());
        Self {
            origin: origin.into().trim_end_matches('/').to_owned(),
            note_bumped: false,
            imports: 0,
            items: vec![
                Item {
                    id: LIBRARY_ID,
                    parent: None,
                    title: "Spike Library",
                    kind: "folder",
                    media: "",
                    body: None,
                    version: "v1".to_owned(),
                    content_path: None,
                },
                Item {
                    id: "m-notes",
                    parent: None,
                    title: "Field Notes",
                    kind: "folder",
                    media: "",
                    body: None,
                    version: "v1".to_owned(),
                    content_path: None,
                },
                Item {
                    id: "f-chapter",
                    parent: Some(LIBRARY_ID),
                    title: "Chapter",
                    kind: "folder",
                    media: "",
                    body: None,
                    version: "v1".to_owned(),
                    content_path: None,
                },
                Item {
                    id: NOTE_ID,
                    parent: Some(LIBRARY_ID),
                    title: "探针笔记",
                    kind: "page",
                    media: "text/plain",
                    body: text("hello from the fake client"),
                    version: "v1".to_owned(),
                    content_path: None,
                },
                Item {
                    id: "f-image",
                    parent: Some(LIBRARY_ID),
                    title: "Diagram",
                    kind: "file",
                    media: "image/png",
                    body: None,
                    version: "v1".to_owned(),
                    content_path: Some("fixtures/mark.png"),
                },
                Item {
                    id: "f-pdf",
                    parent: Some(LIBRARY_ID),
                    title: "Spec",
                    kind: "file",
                    media: "application/pdf",
                    body: None,
                    version: "v1".to_owned(),
                    content_path: Some("fixtures/note.pdf"),
                },
                Item {
                    id: "p-extra",
                    parent: Some(LIBRARY_ID),
                    title: "Second note",
                    kind: "page",
                    media: "text/plain",
                    body: text("second page of the fixture"),
                    version: "v1".to_owned(),
                    content_path: None,
                },
                Item {
                    id: "p-field",
                    parent: Some("m-notes"),
                    title: "Field page",
                    kind: "page",
                    media: "text/plain",
                    body: text("a note in the other mount"),
                    version: "v1".to_owned(),
                    content_path: None,
                },
            ],
        }
    }

    /// Root folders, in sidebar order: `(id, title)`.
    pub fn mounts(&self) -> Vec<(String, String)> {
        self.items
            .iter()
            .filter(|item| item.parent.is_none() && item.kind == "folder")
            .map(|item| (item.id.to_owned(), item.title.to_owned()))
            .collect()
    }

    /// Answer one bridge call.
    ///
    /// `request` is the page's JSON object. The returned value is a
    /// `reply` envelope. `saveText` errors use `{kind, currentVersion}`
    /// and no other fields when the kind is `version-conflict`.
    pub fn reply_to(&mut self, request: &Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        match self.execute(request) {
            Ok(ok) => json!({"kind": "reply", "id": id, "ok": ok}),
            Err(error) => json!({"kind": "reply", "id": id, "error": error}),
        }
    }

    fn execute(&mut self, request: &Value) -> Result<Value, Value> {
        let op = request
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| error("bad-request", "missing op"))?;
        match op {
            "list" => self.list(parent_of(request)?, cursor_of(request)?),
            "read" => self.read(required_str(request, "nodeId")?),
            "search" => self.search(required_str(request, "query")?, cursor_of(request)?),
            "saveText" => self.save_text(
                required_str(request, "nodeId")?,
                required_str(request, "body")?,
                required_str(request, "expectedVersion")?,
            ),
            "importFile" => self.import_file(
                parent_of(request)?,
                required_str(request, "name")?,
                required_str(request, "mediaType")?,
                request.get("byteSize").and_then(Value::as_u64).unwrap_or(0),
            ),
            _ => Err(error("bad-request", "unknown op")),
        }
    }

    fn list(&self, parent: Option<&str>, cursor: Option<&str>) -> Result<Value, Value> {
        self.page(self.children(parent), cursor, LIST_PAGE_SIZE, false)
    }

    fn search(&self, query: &str, cursor: Option<&str>) -> Result<Value, Value> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Err(error("bad-request", "empty query"));
        }
        let hits: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| item.kind != "folder")
            .filter(|item| {
                item.title.to_lowercase().contains(&needle)
                    || item
                        .body
                        .as_ref()
                        .is_some_and(|body| body.to_lowercase().contains(&needle))
            })
            .collect();
        self.page(hits, cursor, SEARCH_PAGE_SIZE, true)
    }

    fn page(
        &self,
        items: Vec<&Item>,
        cursor: Option<&str>,
        size: usize,
        search: bool,
    ) -> Result<Value, Value> {
        let start = match cursor {
            None => 0,
            Some(text) => text
                .parse::<usize>()
                .map_err(|_| error("bad-request", "cursor is not an index"))?,
        };
        if start > items.len() {
            return Err(error("bad-request", "cursor is past the end"));
        }
        let end = (start + size).min(items.len());
        let next = if end < items.len() {
            Value::String(end.to_string())
        } else {
            Value::Null
        };
        let nodes: Vec<Value> = items[start..end]
            .iter()
            .map(|item| {
                if search {
                    json!({"node": self.node(item), "snippet": snippet(item)})
                } else {
                    self.node(item)
                }
            })
            .collect();
        Ok(json!({"nodes": nodes, "nextCursor": next}))
    }

    fn read(&mut self, id: &str) -> Result<Value, Value> {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return Err(error("not-found", id));
        };
        let version = self.items[index].version.clone();
        let body = self.items[index].body.clone();
        let content = self.items[index]
            .content_path
            .map(|path| format!("{}/{}", self.origin, path));
        let node = self.node(&self.items[index]);
        if id == NOTE_ID && !self.note_bumped {
            self.note_bumped = true;
            self.items[index].version = "v2".to_owned();
        }
        let mut read = json!({"node": node, "body": body, "version": version});
        if let Some(url) = content {
            read["contentUrl"] = Value::String(url);
        }
        Ok(read)
    }

    fn save_text(&mut self, id: &str, body: &str, expected: &str) -> Result<Value, Value> {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return Err(error("not-found", id));
        };
        if item.kind != "page" {
            return Err(error("bad-request", "only a page has text to save"));
        }
        if item.version != expected {
            return Err(json!({
                "kind": "version-conflict",
                "currentVersion": item.version,
            }));
        }
        item.body = Some(body.to_owned());
        item.version = bump(&item.version);
        let version = item.version.clone();
        Ok(json!({"version": version}))
    }

    fn import_file(
        &mut self,
        parent: Option<&str>,
        name: &str,
        media: &str,
        byte_size: u64,
    ) -> Result<Value, Value> {
        if name.is_empty() || name.contains(['/', '\\']) {
            return Err(error("bad-request", "import name is empty or has a slash"));
        }
        if let Some(parent_id) = parent {
            let Some(folder) = self.items.iter().find(|item| item.id == parent_id) else {
                return Err(error("not-found", parent_id));
            };
            if folder.kind != "folder" {
                return Err(error("bad-request", "import parent is not a folder"));
            }
        }
        self.imports += 1;
        let id = format!("import-{}", self.imports);
        let node = json!({
            "id": id,
            "parentId": parent,
            "title": name,
            "kind": "file",
            "mediaType": media,
            "updatedAt": "2026-11-05T00:00:00Z",
        });
        self.items.push(Item {
            id: Box::leak(id.into_boxed_str()),
            parent: parent.map(|value| Box::leak(value.to_owned().into_boxed_str()) as &str),
            title: Box::leak(name.to_owned().into_boxed_str()),
            kind: "file",
            media: Box::leak(media.to_owned().into_boxed_str()),
            body: None,
            version: "v1".to_owned(),
            content_path: None,
        });
        let _ = byte_size;
        Ok(node)
    }

    fn children(&self, parent: Option<&str>) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|item| item.parent == parent)
            .collect()
    }

    fn node(&self, item: &Item) -> Value {
        json!({
            "id": item.id,
            "parentId": item.parent,
            "title": item.title,
            "kind": item.kind,
            "mediaType": item.media,
            "updatedAt": "2026-11-04T00:00:00Z",
        })
    }
}

fn parent_of(request: &Value) -> Result<Option<&str>, Value> {
    match request.get("parentId") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(error("bad-request", "parentId is not a string")),
    }
}

fn cursor_of(request: &Value) -> Result<Option<&str>, Value> {
    match request.get("cursor") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(error("bad-request", "cursor is not a string")),
    }
}

fn required_str<'a>(request: &'a Value, key: &str) -> Result<&'a str, Value> {
    request
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| error("bad-request", key))
}

fn error(kind: &str, detail: &str) -> Value {
    json!({"kind": kind, "detail": detail})
}

fn snippet(item: &Item) -> String {
    item.body
        .as_deref()
        .map(|body| body.chars().take(40).collect())
        .unwrap_or_else(|| item.title.to_owned())
}

fn bump(version: &str) -> String {
    let Some(number) = version
        .strip_prefix('v')
        .and_then(|text| text.parse::<u64>().ok())
    else {
        return format!("{version}-2");
    };
    format!("v{}", number + 1)
}

/// The first page of the library lists two children and a cursor.
#[test]
fn list_pages_the_library() {
    let mut store = Store::new("http://127.0.0.1:9");
    let first = store.reply_to(&json!({
        "id": 1, "op": "list", "parentId": LIBRARY_ID, "cursor": null
    }));
    assert_eq!(first["ok"]["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(first["ok"]["nodes"][1]["title"], "探针笔记");
    assert_eq!(first["ok"]["nextCursor"], "2");
    let second = store.reply_to(&json!({
        "id": 2, "op": "list", "parentId": LIBRARY_ID, "cursor": "2"
    }));
    assert_eq!(second["ok"]["nodes"][0]["title"], "Diagram");
    assert_eq!(second["ok"]["nodes"][1]["mediaType"], "application/pdf");
    assert_eq!(second["ok"]["nextCursor"], "4");
    let last = store.reply_to(&json!({
        "id": 3, "op": "list", "parentId": LIBRARY_ID, "cursor": "4"
    }));
    assert_eq!(last["ok"]["nodes"][0]["title"], "Second note");
    assert!(last["ok"]["nextCursor"].is_null());
}

/// An image read carries `contentUrl` and a null body.
#[test]
fn image_read_uses_content_url_not_a_body() {
    let mut store = Store::new("http://127.0.0.1:9");
    let read = store.reply_to(&json!({"id": 4, "op": "read", "nodeId": "f-image"}));
    assert!(read["ok"]["body"].is_null());
    assert_eq!(
        read["ok"]["contentUrl"],
        "http://127.0.0.1:9/fixtures/mark.png"
    );
    assert!(!read["ok"].to_string().contains("base64"));
}

/// A PDF read uses the same content URL rule.
#[test]
fn pdf_read_uses_content_url_not_a_body() {
    let mut store = Store::new("http://127.0.0.1:9");
    let read = store.reply_to(&json!({"id": 5, "op": "read", "nodeId": "f-pdf"}));
    assert!(read["ok"]["body"].is_null());
    assert_eq!(
        read["ok"]["contentUrl"],
        "http://127.0.0.1:9/fixtures/note.pdf"
    );
}

/// The note's first read is `v1`, then an external edit makes a `v1` save conflict.
#[test]
fn save_reports_a_typed_version_conflict() {
    let mut store = Store::new("http://127.0.0.1:9");
    let read = store.reply_to(&json!({"id": 6, "op": "read", "nodeId": NOTE_ID}));
    assert_eq!(read["ok"]["version"], "v1");
    assert_eq!(read["ok"]["body"], "hello from the fake client");
    let conflict = store.reply_to(&json!({
        "id": 7,
        "op": "saveText",
        "nodeId": NOTE_ID,
        "body": "edited",
        "expectedVersion": "v1"
    }));
    let error = &conflict["error"];
    assert_eq!(
        error,
        &json!({"kind": "version-conflict", "currentVersion": "v2"})
    );
    assert_eq!(error.as_object().unwrap().len(), 2);
    let saved = store.reply_to(&json!({
        "id": 8,
        "op": "saveText",
        "nodeId": NOTE_ID,
        "body": "edited",
        "expectedVersion": "v2"
    }));
    assert_eq!(saved["ok"]["version"], "v3");
}

/// Search returns one hit per page.
#[test]
fn search_pages_note_hits() {
    let mut store = Store::new("http://127.0.0.1:9");
    let first = store.reply_to(&json!({
        "id": 9, "op": "search", "query": "note", "cursor": null
    }));
    assert_eq!(first["ok"]["nodes"].as_array().unwrap().len(), 1);
    assert!(first["ok"]["nodes"][0]["snippet"].is_string());
    assert_eq!(first["ok"]["nextCursor"], "1");
}

/// Import records the file's name and media type and does not require bytes.
#[test]
fn import_records_metadata_only() {
    let mut store = Store::new("http://127.0.0.1:9");
    let imported = store.reply_to(&json!({
        "id": 10,
        "op": "importFile",
        "parentId": LIBRARY_ID,
        "name": "dropped.txt",
        "mediaType": "text/plain",
        "byteSize": 4
    }));
    assert_eq!(imported["ok"]["title"], "dropped.txt");
    assert_eq!(imported["ok"]["kind"], "file");
    assert_eq!(imported["ok"]["parentId"], LIBRARY_ID);
    let listed = store.reply_to(&json!({
        "id": 11, "op": "list", "parentId": LIBRARY_ID, "cursor": "4"
    }));
    assert_eq!(listed["ok"]["nodes"][1]["title"], "dropped.txt");
}

/// Mount ids are ordinary fixture data.
#[test]
fn mounts_are_root_folders() {
    let store = Store::new("http://127.0.0.1:9");
    assert_eq!(
        store.mounts(),
        vec![
            ("m-library".to_owned(), "Spike Library".to_owned()),
            ("m-notes".to_owned(), "Field Notes".to_owned()),
        ]
    );
}
