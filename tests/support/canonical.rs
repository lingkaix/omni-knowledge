// Read retained annotation facts directly; never manufacture an M1 UUID in fixture setup.
use std::fs;
use std::path::{Path, PathBuf};

/// Locate exactly one UUID-named visible pointer for the selected imported destination.
/// These fixture observations preserve old tag oracles at the new retained location.
pub fn pointer(root: &Path, source: &str) -> (PathBuf, serde_yaml::Value) {
    let mut matches = Vec::new();
    for entry in fs::read_dir(root.join("sources")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        uuid::Uuid::parse_str(path.file_stem().unwrap().to_str().unwrap()).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let frontmatter = text
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---")
            .unwrap()
            .0;
        let fields: serde_yaml::Value = serde_yaml::from_str(frontmatter).unwrap();
        if fields["path"].as_str() == Some(source) {
            matches.push((path, fields));
        }
    }
    assert_eq!(
        matches.len(),
        1,
        "one actual canonical pointer for {source}"
    );
    matches.pop().unwrap()
}
