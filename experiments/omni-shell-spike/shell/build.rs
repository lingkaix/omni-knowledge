//! Embed the built WebView page and the synthetic preview bytes.

use std::fs;
use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dist = manifest.join("../ui/dist");
    let fixtures = manifest.join("../fixtures");
    println!("cargo:rerun-if-changed={}", dist.display());
    println!("cargo:rerun-if-changed={}", fixtures.display());
    if !dist.join("index.html").is_file() || !dist.join("main.js").is_file() {
        panic!(
            "missing UI build in {}. From experiments/omni-shell-spike/ui run: npm ci && npm run build",
            dist.display()
        );
    }
    let out_dir_owned = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out_dir = Path::new(&out_dir_owned);
    let mut assets = String::from("pub static ASSETS: &[(&str, &[u8], &str)] = &[\n");
    push_tree(&mut assets, &dist, &dist);
    push_file(
        &mut assets,
        "/fixtures/mark.png",
        &fixtures.join("mark.png"),
        "image/png",
    );
    push_file(
        &mut assets,
        "/fixtures/note.pdf",
        &fixtures.join("note.pdf"),
        "application/pdf",
    );
    assets.push_str("];\n");
    fs::write(out_dir.join("assets.rs"), assets).expect("write assets.rs");
}

fn push_tree(assets: &mut String, dist: &Path, dir: &Path) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("dist entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            push_tree(assets, dist, &path);
            continue;
        }
        let relative = path
            .strip_prefix(dist)
            .expect("dist prefix")
            .to_string_lossy()
            .replace('\\', "/");
        let route = format!("/{relative}");
        push_file(assets, &route, &path, mime(&relative));
        if relative == "index.html" {
            push_file(assets, "/", &path, "text/html; charset=utf-8");
        }
    }
}

fn push_file(assets: &mut String, route: &str, path: &Path, mime: &str) {
    let absolute =
        fs::canonicalize(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assets.push_str(&format!(
        "    ({route:?}, include_bytes!({abs:?}), {mime:?}),\n",
        route = route,
        abs = absolute,
        mime = mime,
    ));
}

fn mime(relative: &str) -> &'static str {
    if relative.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if relative.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if relative.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if relative.ends_with(".svg") {
        "image/svg+xml"
    } else if relative.ends_with(".map") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}
