use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use regex::Regex;
use serde_json::{Value, json};

use crate::web;

pub fn site_url() -> String {
    std::env::var("SITE_URL").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "https://skibidink.com".into()).trim_end_matches('/').to_string()
}

pub struct Meta {
    pub path: String,
    pub title: String,
    pub description: String,
    pub robots: String,
}

impl Meta {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            title: "SkibiDink".into(),
            description: String::new(),
            robots: "index, follow, max-image-preview:large".into(),
        }
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }

    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
}

pub fn strip_comments(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while i < html.len() {
        let rest = &lower[i..];
        if rest.starts_with("<!--") {
            match rest.find("-->") {
                Some(end) => {
                    i += end + 3;
                    continue;
                }
                None => break,
            }
        }
        let raw = ["script", "style", "pre", "textarea"].iter().find(|tag| {
            rest.strip_prefix('<').and_then(|r| r.strip_prefix(*tag)).is_some_and(|r| r.starts_with(['>', ' ', '\n', '\t', '\r', '/']))
        });
        if let Some(tag) = raw {
            let close = format!("</{tag}");
            let end = rest.find(&close).map_or(html.len(), |e| i + e + close.len());
            out.push_str(&html[i..end]);
            i = end;
            continue;
        }
        let next = rest[1..].find('<').map_or(html.len(), |n| i + 1 + n);
        out.push_str(&html[i..next]);
        i = next;
    }
    out
}

pub struct Pages {
    pub version: String,
    production: bool,
    pub assets: Option<crate::public::Tree>,
    versioned_re: Regex,
    cache: Mutex<HashMap<String, Arc<String>>>,
}

// SECURITY: `<` is escaped so nothing in the graph can close the <script> it is emitted into.
fn jsonld(v: &Value) -> String {
    v.to_string().replace('<', "\\u003c")
}

impl Pages {
    pub fn new(root: &Path) -> Self {
        let version = std::env::var("RENDER_GIT_COMMIT").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis().to_string()).unwrap_or_default()
        });
        Self {
            version,
            production: web::production(),
            assets: web::production().then(|| crate::public::tree(&root.join("public"))),
            versioned_re: Regex::new(r#"((?:href|src))="(/[^"?]+\.(?:js|css|png|jpe?g|webp|avif|gif|svg|ico|woff2?|ttf))""#).expect("valid regex"),
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn versioned(&self, html: &str) -> String {
        self.versioned_re.replace_all(html, |c: &regex::Captures| format!("{}=\"{}\"", &c[1], self.asset(&c[2]))).into_owned()
    }

    pub fn asset(&self, path: &str) -> String {
        if path.starts_with("/_v/") {
            return path.to_string();
        }
        match self.assets.as_ref().and_then(|t| t.url(path)) {
            Some(url) => url,
            None => format!("{path}?v={}", self.version),
        }
    }

    fn header(&self, meta: &Meta) -> String {
        let base = site_url();
        let canonical = format!("{base}{}", meta.path);
        let logo = format!("{base}/img/logo.png");
        let org = json!({
            "@type": "SportsActivityLocation",
            "@id": format!("{base}/#organization"),
            "name": "SkibiDink",
            "url": format!("{base}/"),
            "logo": logo,
            "image": logo,
            "sport": "Pickleball"
        });
        let site = json!({
            "@type": "WebSite",
            "@id": format!("{base}/#website"),
            "url": format!("{base}/"),
            "name": "SkibiDink",
            "publisher": { "@id": format!("{base}/#organization") },
            "inLanguage": "en"
        });
        let page = json!({
            "@type": "WebPage",
            "@id": format!("{canonical}#webpage"),
            "url": canonical,
            "name": meta.title,
            "description": meta.description,
            "isPartOf": { "@id": format!("{base}/#website") },
            "about": { "@id": format!("{base}/#organization") },
            "inLanguage": "en"
        });
        let structured = format!("{{\"@context\":\"https://schema.org\",\"@graph\":[{},{},{}]}}", jsonld(&org), jsonld(&site), jsonld(&page));
        let (title, description, robots) = (escape(&meta.title), escape(&meta.description), &meta.robots);
        format!(
            r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>{title}</title>
<meta name="description" content="{description}">
<meta name="robots" content="{robots}">
<link rel="canonical" href="{canonical}">
<script type="application/ld+json">{structured}</script>
<meta property="og:type" content="website">
<meta property="og:url" content="{canonical}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:image" content="{logo}">
<meta property="og:site_name" content="SkibiDink">
<meta property="og:locale" content="en_PH">
<meta name="twitter:card" content="summary_large_image">
<meta name="theme-color" content="#f8fafc">
<link rel="icon" type="image/png" href="/img/favicon.png">
<link rel="apple-touch-icon" href="/img/favicon.png">
<link rel="preload" href="/vendor/fonts/outfit-latin.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="/vendor/fonts/jakarta-latin.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/styles/output.css">
<script defer src="/js/site.js"></script>
<script defer src="/js/book.js"></script>
</head>
"##
        )
    }

    /// The shared head plus the body fragment, versioned, comments stripped.
    pub fn compose(&self, meta: &Meta, body: &str) -> String {
        strip_comments(&self.versioned(&(self.header(meta) + body)))
    }

    pub async fn cached<F, Fut>(&self, key: &str, build: F) -> Result<Arc<String>, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        if self.production {
            if let Some(hit) = self.cache.lock().unwrap().get(key) {
                return Ok(hit.clone());
            }
        }
        let page = Arc::new(build().await?);
        if self.production {
            self.cache.lock().unwrap().insert(key.to_string(), page.clone());
        }
        Ok(page)
    }
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comments_but_not_scripts() {
        assert_eq!(strip_comments("<p>a<!-- x -->b</p><script>/*<!-- y -->*/</script>"), "<p>ab</p><script>/*<!-- y -->*/</script>");
    }

    #[test]
    fn versions_local_assets_only() {
        let p = Pages::new(Path::new("."));
        let out = p.versioned(r#"<link href="/styles/output.css"><script src="https://cdn.example/x.js"></script>"#);
        assert!(out.contains(&format!("/styles/output.css?v={}", p.version)));
        assert!(out.contains("https://cdn.example/x.js\""));
    }
}
