use std::path::{Path, PathBuf};

use axum::{
    extract::Request,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use percent_encoding::percent_decode_str;
use tower::ServiceExt;
use tower_http::services::ServeFile;

// .html is never served from here: every page in public/ is a fragment a route composes, and the
// raw file at a second URL would be a duplicate for crawlers with no `?v=` on its assets.
const TYPES: [(&str, &str); 21] = [
    ("js", "application/javascript; charset=UTF-8"),
    ("mjs", "application/javascript; charset=UTF-8"),
    ("css", "text/css; charset=UTF-8"),
    ("json", "application/json; charset=UTF-8"),
    ("webmanifest", "application/manifest+json"),
    ("map", "application/json; charset=UTF-8"),
    ("md", "text/markdown; charset=UTF-8"),
    ("png", "image/png"),
    ("webp", "image/webp"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("ico", "image/x-icon"),
    ("svg", "image/svg+xml"),
    ("woff2", "font/woff2"),
    ("woff", "font/woff"),
    ("otf", "font/otf"),
    ("ttf", "font/ttf"),
    ("pdf", "application/pdf"),
    ("txt", "text/plain; charset=UTF-8"),
    ("xml", "application/xml"),
];

pub struct File {
    path: PathBuf,
    rel: String,
    content_type: &'static str,
}

pub fn lookup(root: &Path, raw: &str) -> Option<File> {
    let decoded = percent_decode_str(raw).decode_utf8().ok()?;
    // SECURITY: every segment is checked after decoding, so `%2e%2e` cannot climb out of root and
    // dotfiles stay hidden.
    let rel = decoded.strip_prefix('/')?;
    if rel.contains('\\') || rel.contains('\0') {
        return None;
    }
    if rel.split('/').any(|seg| seg.is_empty() || seg.starts_with('.')) {
        return None;
    }
    let ext = rel.rsplit_once('.')?.1.to_ascii_lowercase();
    let content_type = TYPES.iter().find(|(e, _)| *e == ext)?.1;
    let path = root.join(rel);
    path.is_file().then(|| File { path, rel: rel.to_string(), content_type })
}

pub struct Tree {
    pub hash: String,
    files: std::collections::HashSet<String>,
}

impl Tree {
    pub fn url(&self, path: &str) -> Option<String> {
        let rel = path.strip_prefix('/')?;
        (self.files.contains(rel) && fixed_url(rel).is_none()).then(|| format!("/_v/{}{path}", self.hash))
    }
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            walk(&entry.path(), &child, out);
        } else if kind.is_file() && !name.ends_with(".html") {
            out.push(child);
        }
    }
}

// DANGER: must produce the same hash as `assets=` in router/deploy.sh (shasum over `find . -type f
// ! -name '*.html' ! -path '*/.*'`, sorted). A different hash sends every asset of every page to
// Render instead of Cloudflare.
pub fn tree(root: &Path) -> Tree {
    use sha2::{Digest, Sha256};
    let mut files = Vec::new();
    walk(root, "", &mut files);
    files.sort_by(|a, b| format!("./{a}").as_bytes().cmp(format!("./{b}").as_bytes()));
    let mut all = Sha256::new();
    for rel in &files {
        let bytes = std::fs::read(root.join(rel)).unwrap_or_default();
        all.update(format!("{}  ./{rel}\n", hex::encode(Sha256::digest(&bytes))).as_bytes());
    }
    Tree { hash: hex::encode(all.finalize())[..12].to_string(), files: files.into_iter().collect() }
}

fn fixed_url(rel: &str) -> Option<&'static str> {
    if rel.ends_with("sw.js") { Some("no-cache") } else { None }
}

fn cache_control(rel: &str) -> &'static str {
    if let Some(fixed) = fixed_url(rel) {
        fixed
    } else if crate::web::production() {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

pub async fn serve(file: File, req: Request) -> Response {
    let mut res = match ServeFile::new(&file.path).oneshot(req).await {
        Ok(res) => res.into_response(),
        Err(never) => match never {},
    };
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(file.content_type));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache_control(&file.rel)));
    if file.rel.ends_with("sw.js") {
        headers.insert("service-worker-allowed", HeaderValue::from_static("/"));
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_traversal_dotfiles_and_pages() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("public");
        assert!(lookup(&root, "/robots.txt").is_some());
        assert!(lookup(&root, "/index.html").is_none());
        assert!(lookup(&root, "/%2e%2e/server-rs/Cargo.toml").is_none());
        assert!(lookup(&root, "/.DS_Store").is_none());
        assert!(lookup(&root, "//robots.txt").is_none());
    }

    #[test]
    fn hashes_only_files_whose_url_may_change() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("public");
        let t = tree(&root);
        assert_eq!(t.hash.len(), 12);
        assert_eq!(t.url("/robots.txt"), Some(format!("/_v/{}/robots.txt", t.hash)));
        assert_eq!(t.url("/index.html"), None);
        assert_eq!(t.url("/nope.js"), None);
    }
}
