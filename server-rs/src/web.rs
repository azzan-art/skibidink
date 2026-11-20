use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    body::{Body, to_bytes},
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::App;

/// Who is asking, as the edge middleware resolves it. Every handler reads the client from here.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Ctx {
    pub ip: String,
}

pub struct Fail(pub String);

const INTERNAL: char = '\u{1}';

// SECURITY: `?` on a database or parser error lands here. Its text names tables and constraints,
// so it is marked and never sent; only messages written as `Fail("...")` reach the client.
impl From<String> for Fail {
    fn from(s: String) -> Self {
        Fail(format!("{INTERNAL}{s}"))
    }
}

impl Fail {
    pub fn public(&self) -> String {
        if self.0.starts_with(INTERNAL) { "Something went wrong.".into() } else { self.0.clone() }
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> Response {
        eprintln!("[ERROR] {}", self.0.trim_start_matches(INTERNAL));
        json(500, json!({ "success": false, "error": self.public() }))
    }
}

pub type Res = Result<Response, Fail>;

pub fn respond(status: u16, content_type: &str, body: impl Into<Body>) -> Response {
    let mut res = Response::new(body.into());
    *res.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if let Ok(v) = HeaderValue::from_str(content_type) {
        res.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    res
}

pub fn json(status: u16, v: Value) -> Response {
    respond(status, "application/json; charset=utf-8", v.to_string())
}

pub fn html(status: u16, body: impl Into<Body>) -> Response {
    respond(status, "text/html; charset=utf-8", body)
}

pub fn text(status: u16, body: impl Into<Body>) -> Response {
    respond(status, "text/plain; charset=utf-8", body)
}

pub fn redirect(code: u16, location: &str) -> Response {
    let reason = StatusCode::from_u16(code).ok().and_then(|s| s.canonical_reason()).unwrap_or("");
    text(code, format!("{reason}. Redirecting to {location}")).head("Location", location)
}

pub trait Head {
    fn head(self, name: &'static str, value: impl AsRef<str>) -> Self;
}

impl Head for Response {
    fn head(mut self, name: &'static str, value: impl AsRef<str>) -> Self {
        // DANGER: from_bytes, never from_static. from_static panics on an uppercase name.
        if let (Ok(n), Ok(v)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value.as_ref())) {
            self.headers_mut().insert(n, v);
        }
        self
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

pub fn production() -> bool {
    std::env::var("NODE_ENV").as_deref() == Ok("production")
}

pub fn env_number(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|n| *n > 0.0).unwrap_or(default)
}

const BUCKET_MAX_IPS: usize = 5_000;

/// A token bucket per client address: `burst` to start, refilled at `per_min`.
#[allow(dead_code)]
pub struct Buckets {
    burst: f64,
    per_ms: f64,
    map: Mutex<IndexMap<String, (f64, Instant)>>,
}

#[allow(dead_code)]
impl Buckets {
    pub fn new(burst: f64, per_min: f64) -> Self {
        Self { burst, per_ms: per_min / 60_000.0, map: Mutex::new(IndexMap::new()) }
    }

    pub fn allows(&self, ip: &str) -> bool {
        let now = Instant::now();
        let mut map = self.map.lock().unwrap();
        if !map.contains_key(ip) && map.len() >= BUCKET_MAX_IPS {
            map.shift_remove_index(0);
        }
        let b = map.entry(ip.to_string()).or_insert((self.burst, now));
        let elapsed = now.duration_since(b.1).as_secs_f64() * 1000.0;
        b.0 = self.burst.min(b.0 + elapsed * self.per_ms);
        b.1 = now;
        if b.0 < 1.0 {
            return false;
        }
        b.0 -= 1.0;
        true
    }

    pub fn sweep(&self) {
        let full = Duration::from_secs_f64(self.burst / self.per_ms / 1000.0);
        let now = Instant::now();
        self.map.lock().unwrap().retain(|_, b| now.duration_since(b.1) <= full);
    }
}

struct Client {
    count: u32,
    window_start: Instant,
    last_seen: Instant,
}

pub struct Limiter {
    clients: Mutex<IndexMap<String, Client>>,
    limit: u32,
}

const WINDOW: Duration = Duration::from_secs(1);
const MAX_TRACKED_IPS: usize = 20_000;
const STATIC_EXT: [&str; 12] = ["js", "css", "png", "jpg", "jpeg", "gif", "ico", "svg", "woff", "woff2", "ttf", "map"];

fn cost(path: &str) -> u32 {
    let ext = path.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.contains('/'));
    if ext.is_some_and(|e| STATIC_EXT.contains(&e)) { 1 } else { 5 }
}

impl Limiter {
    pub fn new() -> Self {
        Self { clients: Mutex::new(IndexMap::new()), limit: env_number("RATE_LIMIT", 100.0) as u32 }
    }

    fn allows(&self, ip: &str, path: &str) -> bool {
        let now = Instant::now();
        let mut clients = self.clients.lock().unwrap();
        if !clients.contains_key(ip) && clients.len() >= MAX_TRACKED_IPS {
            clients.shift_remove_index(0);
        }
        let c = clients.entry(ip.to_string()).or_insert(Client { count: 0, window_start: now, last_seen: now });
        if now.duration_since(c.window_start) > WINDOW {
            c.count = 0;
            c.window_start = now;
        }
        c.count += cost(path);
        c.last_seen = now;
        c.count <= self.limit
    }

    pub fn sweep(&self) {
        let now = Instant::now();
        self.clients.lock().unwrap().retain(|_, c| now.duration_since(c.last_seen) <= WINDOW);
    }
}

pub fn edge_secret_matches(given: &str) -> bool {
    let secret = std::env::var("EDGE_SECRET").unwrap_or_default();
    if secret.is_empty() || given.len() != secret.len() {
        return false;
    }
    given.bytes().zip(secret.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

// SECURITY: the client address and host are taken from the router only when it proves itself
// with EDGE_SECRET. Trusting the headers bare lets anyone pick their own address and reset their
// rate limits.
fn context(headers: &mut HeaderMap, peer: Option<SocketAddr>) -> Ctx {
    let mut ip = None;
    if edge_secret_matches(header_str(headers, "x-skibidink-edge").unwrap_or("")) {
        ip = header_str(headers, "x-skibidink-client-ip").filter(|v| !v.is_empty()).map(str::to_string);
        if let Some(v) = header_str(headers, "x-skibidink-host").filter(|v| !v.is_empty()).and_then(|h| HeaderValue::from_str(h).ok()) {
            headers.insert(header::HOST, v.clone());
            headers.insert("x-forwarded-host", v);
        }
    }
    let ip = ip.unwrap_or_else(|| {
        header_str(headers, "x-forwarded-for")
            .and_then(|xff| xff.split(',').map(str::trim).filter(|s| !s.is_empty()).last())
            .map(str::to_string)
            .or_else(|| peer.map(|p| p.ip().to_string()))
            .unwrap_or_default()
    });
    Ctx { ip }
}

// SECURITY: a write the browser marks as coming from another site never reaches a route, so a page
// elsewhere cannot spend a visitor's session cookie here.
fn same_origin(headers: &HeaderMap) -> bool {
    if let Some(site) = header_str(headers, "sec-fetch-site") {
        return site == "same-origin" || site == "none";
    }
    let Some(origin) = header_str(headers, "origin") else { return true };
    let Ok(url) = url::Url::parse(origin) else { return false };
    let Some(host) = url.host_str() else { return false };
    let origin_host = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    Some(origin_host.as_str()) == header_str(headers, "host")
}

fn rewrite_path(req: &mut Request, path: &str) {
    let pq = match req.uri().query() {
        Some(q) => format!("{path}?{q}"),
        None => path.to_string(),
    };
    if let Ok(uri) = pq.parse::<Uri>() {
        *req.uri_mut() = uri;
    }
}

pub async fn middleware(State(app): State<Arc<App>>, mut req: Request, next: Next) -> Response {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    let ctx = context(req.headers_mut(), peer);
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let write = matches!(method, Method::POST | Method::PUT | Method::PATCH | Method::DELETE);
    if write && !same_origin(req.headers()) {
        return finish(json(403, json!({ "error": "Cross-site request blocked" })), &path);
    }
    if !app.limiter.allows(&ctx.ip, &path) {
        return finish(json(429, json!({ "error": "Rate limit exceeded. Try again in a minute." })), &path);
    }
    if path.len() > 1 && path.ends_with('/') && !path.ends_with("//") {
        let trimmed = path[..path.len() - 1].to_string();
        rewrite_path(&mut req, &trimmed);
    }

    req.extensions_mut().insert(ctx);
    finish(next.run(req).await, &path)
}

fn finish(mut res: Response, path: &str) -> Response {
    let headers = res.headers_mut();
    if production() {
        headers.insert(header::STRICT_TRANSPORT_SECURITY, HeaderValue::from_static("max-age=31536000"));
    }
    let api = path == "/api" || path.starts_with("/api/");
    if api && !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("strict-origin-when-cross-origin"));
    headers.entry(header::X_FRAME_OPTIONS).or_insert(HeaderValue::from_static("SAMEORIGIN"));
    if api {
        headers.insert("x-robots-tag", HeaderValue::from_static("noindex, nofollow"));
    }
    res
}

const ETAG_MAX: usize = 4 * 1024 * 1024;

// DANGER: must sit inside CompressionLayer. Outside it every body is a stream with no exact size,
// and nothing is ever tagged.
pub async fn etag(req: Request, next: Next) -> Response {
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        return next.run(req).await;
    }
    let asked = req.headers().get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let res = next.run(req).await;
    let h = res.headers();
    if res.status() != StatusCode::OK || h.contains_key(header::ETAG) || h.contains_key(header::SET_COOKIE) || h.contains_key(header::CONTENT_ENCODING) {
        return res;
    }
    let (mut parts, body) = res.into_parts();
    if !matches!(axum::body::HttpBody::size_hint(&body).exact(), Some(n) if n as usize <= ETAG_MAX) {
        return Response::from_parts(parts, body);
    }
    let Ok(bytes) = to_bytes(body, ETAG_MAX).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let tag = weak_tag(&bytes);
    let fresh = asked.as_deref().is_some_and(|a| tag_matches(a, &tag));
    parts.headers.insert(header::ETAG, HeaderValue::from_str(&tag).expect("base64 is a valid header value"));
    if fresh {
        parts.status = StatusCode::NOT_MODIFIED;
        parts.headers.remove(header::CONTENT_LENGTH);
        return Response::from_parts(parts, Body::empty());
    }
    Response::from_parts(parts, Body::from(bytes))
}

fn weak_tag(bytes: &[u8]) -> String {
    use base64::Engine;
    use sha1::{Digest, Sha1};
    format!("W/\"{}\"", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha1::digest(bytes)))
}

fn tag_matches(asked: &str, tag: &str) -> bool {
    let bare = |t: &str| t.trim().trim_start_matches("W/").to_owned();
    asked.split(',').any(|t| t.trim() == "*" || bare(t) == bare(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn if_none_match_compares_weakly_and_reads_lists() {
        let tag = weak_tag(b"{}");
        assert!(tag_matches(&tag, &tag));
        assert!(tag_matches(tag.trim_start_matches("W/"), &tag));
        assert!(tag_matches(&format!("W/\"x\", {tag}"), &tag));
        assert!(tag_matches("*", &tag));
        assert!(!tag_matches("W/\"x\"", &tag));
    }

    #[test]
    fn charges_static_files_less() {
        assert_eq!(cost("/js/site.js"), 1);
        assert_eq!(cost("/api/ready"), 5);
        assert_eq!(cost("/a.js/b"), 5);
    }

    #[test]
    fn internal_errors_never_reach_the_client() {
        assert_eq!(Fail::from("relation \"x\" does not exist".to_string()).public(), "Something went wrong.");
        assert_eq!(Fail("Court not found".into()).public(), "Court not found");
    }
}
