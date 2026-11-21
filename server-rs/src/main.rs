mod account;
mod admin;
mod book;
mod date;
mod db;
mod mail;
mod pages;
mod pay;
mod poster;
mod public;
mod site;
mod web;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::{Router, ServiceExt};
use tower::Layer;
use tower_http::compression::CompressionLayer;

use crate::{db::Db, web::Fail};

pub struct App {
    pub root: PathBuf,
    pub db: Option<Db>,
    pub pages: pages::Pages,
    pub limiter: web::Limiter,
    pub request_buckets: web::Buckets,
    pub login_buckets: web::Buckets,
    pub account_buckets: web::Buckets,
    pub sessions: admin::Sessions,
    pub mailer: mail::Mailer,
    pub http: reqwest::Client,
}

impl App {
    pub fn db(&self) -> Result<&Db, Fail> {
        self.db.as_ref().ok_or_else(|| Fail("DATABASE_URL is not set.".into()))
    }
}

/// `KEY=value` lines from `.env`, for a local run. A variable already set always wins, and the
/// deployed image has no `.env` at all.
fn load_env_file(path: &std::path::Path) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim().trim_start_matches("export ").trim(), v.trim());
        let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\''))).unwrap_or(v);
        if std::env::var_os(k).is_none() {
            // SAFETY: called once at startup, before the runtime spawns any other thread.
            unsafe { std::env::set_var(k, v) };
        }
    }
}

async fn shutdown_signal() {
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term => {}
    }
    println!("[shutdown] draining");
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(10)).await;
        std::process::exit(0);
    });
}

#[tokio::main]
async fn main() {
    let root = PathBuf::from(std::env::var("SKIBIDINK_ROOT").unwrap_or_else(|_| ".".into()));
    load_env_file(&root.join(".env"));

    let db = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()).and_then(|u| match Db::connect(&u) {
        Ok(db) => Some(db),
        Err(err) => {
            eprintln!("[db] not connected: {err}");
            None
        }
    });
    match &db {
        Some(db) => match db.batch(&format!("{}{}", book::SCHEMA, account::SCHEMA)).await {
            Ok(()) => {
                println!("[db] schema ready");
                if let Err(err) = account::seed_demo(db).await {
                    eprintln!("[demo] {err}");
                }
            }
            Err(err) => eprintln!("\n[db] SCHEMA FAILED: {err}\n"),
        },
        None => println!("[db] DATABASE_URL unset: pages work, database routes answer 500"),
    }

    let app = Arc::new(App {
        pages: pages::Pages::new(&root, db.is_none() && !web::production()),
        root,
        db,
        limiter: web::Limiter::new(),
        request_buckets: web::Buckets::new(5.0, 0.2),
        login_buckets: web::Buckets::new(5.0, 2.0),
        account_buckets: web::Buckets::new(10.0, 5.0),
        sessions: admin::Sessions::new(),
        mailer: mail::Mailer::from_env(),
        http: reqwest::Client::builder().timeout(Duration::from_secs(20)).build().expect("http client"),
    });

    let app_mailer_name = app.mailer.name();
    let sweeper = app.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        let mut n: u64 = 0;
        loop {
            tick.tick().await;
            n += 1;
            if n % 60 == 0
                && let Some(db) = &sweeper.db
                && let Err(err) = account::sweep(db).await
            {
                eprintln!("[sweep] {err}");
            }
            sweeper.limiter.sweep();
            sweeper.account_buckets.sweep();
            sweeper.request_buckets.sweep();
            sweeper.login_buckets.sweep();
        }
    });

    let router = Router::new()
        .merge(site::routes())
        .merge(book::routes())
        .merge(admin::routes())
        .merge(account::routes())
        .merge(pay::routes())
        .fallback(site::fallback)
        .with_state(app.clone())
        .layer(axum::middleware::from_fn(web::etag))
        .layer(CompressionLayer::new());
    // The middleware wraps the router rather than being layered on it, so the path it rewrites is
    // the path routing sees.
    let service = axum::middleware::from_fn_with_state(app, web::middleware).layer(router);

    println!("[mail] sending through {}", app_mailer_name);
    let port = std::env::var("PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(3000u16);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind PORT");
    println!("[server] listening on http://localhost:{port}");
    if let Err(err) = axum::serve(listener, service.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown_signal()).await {
        eprintln!("[server] {err}");
        std::process::exit(1);
    }
}
