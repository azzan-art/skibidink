use std::sync::Arc;

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, Method, header},
    response::Response,
    routing::get,
};
use serde_json::json;

use crate::{
    App, account,
    pages::Meta,
    public,
    web::{self, Head, Res},
};

pub fn routes() -> Router<Arc<App>> {
    Router::new().route("/", get(home)).route("/book", get(book)).route("/api/ready", get(ready)).route("/api/health", get(health))
}

/// Signed out while accounts exist. Without a database (GitHub Pages, a local run with no
/// `DATABASE_URL`) nobody can sign in, so nobody is a guest and the pages stay as written.
async fn is_guest(app: &App, headers: &HeaderMap) -> bool {
    app.db.is_some() && account::current(app, headers).await.map_or(true, |user| user.is_none())
}

/// A guest's Book links go to sign-in instead: `href="/book" data-guest-href="X"` becomes
/// `href="X"`, and `<span data-guest="X">…</span>` becomes `<span>X</span>`.
fn for_guest(html: &str) -> String {
    static HREF: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static LABEL: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let href = HREF.get_or_init(|| regex::Regex::new(r#"href="[^"]*" data-guest-href="([^"]*)""#).unwrap());
    let label = LABEL.get_or_init(|| regex::Regex::new(r#"<span data-guest="([^"]*)">[^<]*</span>"#).unwrap());
    label.replace_all(&href.replace_all(html, r#"href="$1""#), "<span>$1</span>").into_owned()
}

async fn send_page(app: &App, file: &str, meta: Meta, guest: bool) -> Res {
    let path = app.root.join("public").join(file);
    let html = app
        .pages
        .cached(&format!("{file}|{}|{}", meta.path, if guest { "guest" } else { "member" }), || async {
            let body = tokio::fs::read_to_string(&path).await.map_err(|e| format!("{}: {e}", path.display()))?;
            let page = app.pages.compose(&meta, &body);
            Ok(if guest { for_guest(&page) } else { page })
        })
        .await?;
    Ok(web::html(200, html.as_str().to_owned()).head("Cache-Control", "private, no-cache"))
}

async fn home(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    let meta = Meta::new("/")
        .title("SkibiDink | Pickleball")
        .description("SkibiDink is a place to play pickleball, booked by the hour.");
    send_page(&app, "index.html", meta, is_guest(&app, &headers).await).await
}

async fn book(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    if is_guest(&app, &headers).await {
        return Ok(web::redirect(302, "/account?next=%2Fbook"));
    }
    let meta = Meta::new("/book")
        .title("Book a court | SkibiDink")
        .description("See which hours are open at SkibiDink and send a booking request.");
    send_page(&app, "book.html", meta, false).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guests_get_sign_in_links() {
        let html = r#"<a href="/book" data-guest-href="/account?next=%2Fbook" class="b"><span data-guest="Sign in to book">Book now</span></a>"#;
        assert_eq!(for_guest(html), r#"<a href="/account?next=%2Fbook" class="b"><span>Sign in to book</span></a>"#);
    }
}
