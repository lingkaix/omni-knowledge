//! Allocation admission for JSON nodes, containers, strings, and semantic clones.
//! Encoded length alone cannot bound a graph of many small JSON values.
use crate::{Capture, Error, Input, Manifest, Work, charge_fresh, preallocate};
use serde_json::Value;

/// A conservative representation envelope, not an independent account.
/// Each node funds Value/semantic slots, Vec spare slots, B-tree edges, and JCS sorting.
/// String bytes fund decoded, escaped, UTF-16 sort keys, and simultaneous copies.
#[derive(Default)]
pub(crate) struct Shape {
    nodes: u64,
    bytes: u64,
}

impl Shape {
    /// Add a logical string and its encoded/decoded workspace.
    fn text(&mut self, text: &str) {
        self.nodes = self.nodes.saturating_add(1);
        self.bytes = self.bytes.saturating_add(text.len() as u64);
    }

    /// Count optional JSON content without allocating another graph.
    fn value(&mut self, value: &Value) {
        self.nodes = self.nodes.saturating_add(1);
        match value {
            Value::String(text) => self.text(text),
            Value::Array(items) => items.iter().for_each(|item| self.value(item)),
            Value::Object(items) => items.iter().for_each(|(key, value)| {
                self.text(key);
                self.value(value);
            }),
            _ => {}
        }
    }

    /// Upper envelope for one complete graph and its associated output.
    pub(crate) fn bound(&self) -> Result<u64, Error> {
        self.nodes
            .checked_mul(1024)
            .and_then(|n| self.bytes.checked_mul(32).and_then(|b| n.checked_add(b)))
            .and_then(|n| n.checked_add(4096))
            .ok_or(Error::TooLarge)
    }
}

/// Lexically count JSON structure before serde or JCS can allocate.
/// Serde remains the syntax/schema authority; this pass only bounds representations.
pub(crate) fn scan(bytes: &[u8], work: &Work<'_>) -> Result<Shape, Error> {
    let mut shape = Shape {
        nodes: 1,
        bytes: bytes.len() as u64,
    };
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0u32;
    for (index, byte) in bytes.iter().enumerate() {
        if index % 8192 == 0 {
            crate::gate(work)?;
        }
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => {
                    quoted = true;
                    shape.nodes += 1;
                }
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 32 {
                        return Err(Error::Invalid);
                    }
                    shape.nodes += 1;
                }
                b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                }
                b',' | b':' => {
                    shape.nodes += 1;
                }
                _ => {}
            }
        }
    }
    Ok(shape)
}

/// Count the actual semantic source before emit clones it into a Value graph.
pub(crate) fn manifest(value: &Manifest) -> Shape {
    let mut shape = Shape {
        nodes: 48,
        bytes: 1024,
    };
    for text in [
        &value.library_id,
        &value.name,
        &value.text_root,
        &value.window.start,
        &value.window.end,
    ] {
        shape.text(text);
    }
    for text in &value.required_features {
        shape.text(text);
    }
    for item in &value.mounts {
        shape.nodes += 10;
        for text in [&item.mount_id, &item.name, &item.backend, &item.location] {
            shape.text(text);
        }
    }
    for item in &value.entries {
        shape.nodes += 24;
        shape.text(item.path.as_str());
        shape.text(item.sha256.as_str());
        for text in [&item.mount_id, &item.source_id].into_iter().flatten() {
            shape.text(text);
        }
    }
    for item in &value.external {
        shape.nodes += 24;
        for text in [
            item.source_id.as_str(),
            item.mount_id.as_str(),
            item.path.as_str(),
        ] {
            shape.text(text);
        }
        if let Some(digest) = &item.digest {
            shape.text(digest.as_str());
        }
    }
    if let Some(index) = &value.indexes {
        shape.nodes += 8;
        for path in &index.paths {
            shape.text(path.as_str());
        }
        if let Some(reason) = &index.fallback {
            shape.text(reason);
        }
        for (key, value) in &index.metadata {
            shape.text(key);
            shape.value(value);
        }
    }
    for blob in [&value.history, &value.journal].into_iter().flatten() {
        shape.nodes += 6;
        shape.text(blob.path.as_str());
        shape.text(&blob.scope);
    }
    for (key, value) in &value.extra {
        shape.text(key);
        shape.value(value);
    }
    shape
}

/// Admit capture's retained semantic clones, including both inventories and ZIP directory slots.
pub(crate) fn capture(
    value: &Capture<'_>,
    inputs: &[Input<'_>],
    work: &mut Work<'_>,
) -> Result<(), Error> {
    let mut shape = Shape {
        nodes: 48,
        bytes: 1024,
    };
    for text in [
        value.library_id,
        value.name,
        value.text_root,
        &value.window.start,
        &value.window.end,
    ] {
        shape.text(text);
    }
    for text in &value.required_features {
        shape.text(text);
    }
    for item in &value.mounts {
        shape.nodes += 10;
        for text in [&item.mount_id, &item.name, &item.backend, &item.location] {
            shape.text(text);
        }
    }
    for item in &value.external {
        shape.nodes += 24;
        for text in [
            item.source_id.as_str(),
            item.mount_id.as_str(),
            item.path.as_str(),
        ] {
            shape.text(text);
        }
        if let Some(digest) = &item.digest {
            shape.text(digest.as_str());
        }
    }
    for item in inputs {
        shape.nodes += 48;
        shape.text(item.path.as_str());
        for text in [&item.mount_id, &item.source_id].into_iter().flatten() {
            shape.text(text);
        }
    }
    charge_fresh(work, shape.bound()?, 0)
}

/// Reserve transient parse/JCS memory and fund the returned semantic graph separately.
pub(crate) fn parsing(bytes: &[u8], work: &mut Work<'_>) -> Result<crate::Scratch, Error> {
    let bound = scan(bytes, work)?.bound()?;
    let scratch = preallocate(work, bound)?;
    charge_fresh(work, bound, 0)?;
    Ok(scratch)
}
