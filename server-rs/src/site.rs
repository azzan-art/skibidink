use std::sync::Arc;

use axum::{
    Router,
    extract::{Request, State},
    http::{Method, header},
    response::Response,
    routing::get,
};
use serde_json::json;

use crate::{
    App,
    pages::Meta,
    public,
    web::{self, Head, Res},
};

pub fn routes() -> Router<Arc<App>> {
    Router::new().route("/", get(home)).route("/api/ready", get(ready)).route("/api/health", get(health))
}

async fn send_page(app: &App, file: &str, meta: Meta) -> Res {
    let path = app.root.join("public").join(file);
    let html = app
        .pages
        .cached(&format!("{file}|{}", meta.path), || async {
            let body = tokio::fs::read_to_string(&path).await.map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(app.pages.compose(&meta, &body))
        })
        .await?;
    Ok(web::html(200, html.as_str().to_owned()).head("Cache-Control", "private, no-cache"))
}

async fn home(State(app): State<Arc<App>>) -> Res {
    let meta = Meta::new("/")
        .title("SkibiDink | Pickleball")
        .description("SkibiDink is a place to play pickleball, booked by the hour.");
    send_page(&app, "index.html", meta).await
}

async fn ready() -> Response {
    web::json(200, json!({ "ok": true }))
}

async fn health(State(app): State<Arc<App>>) -> Response {
    web::json(200, json!({ "ok": true, "version": app.pages.version, "db": app.db.is_some() }))
}

/// `/_v/<hash>/…` for the current hash, then a file under `public/`, else GET → `/`, other → 404.
pub async fn fallback(State(app): State<Arc<App>>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let public_root = app.root.join("public");
    if let Some(rest) = path.strip_prefix("/_v/") {
        let (hash, file) = rest.split_once('/').unwrap_or((rest, ""));
        // DANGER: only the current hash. Today's bytes under another tree's hash are kept for a year.
        if app.pages.assets.as_ref().is_some_and(|t| t.hash == hash) {
            if let Some(f) = public::lookup(&public_root, &format!("/{file}")) {
                let mut res = public::serve(f, req).await;
                res.headers_mut().insert(header::CACHE_CONTROL, "public, max-age=31536000, immutable".parse().unwrap());
                return res;
            }
        }
        return web::text(404, "Not Found");
    }
    if let Some(f) = public::lookup(&public_root, &path) {
        return public::serve(f, req).await;
    }
    if req.method() == Method::GET {
        web::redirect(302, "/")
    } else {
        web::text(404, "Not Found")
    }
}
