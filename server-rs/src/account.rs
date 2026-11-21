use std::{collections::BTreeMap, sync::Arc};

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    Extension, Router,
    body::Bytes,
    extract::{Query, State},
    http::HeaderMap,
    response::Response,
    routing::{get, post},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    App,
    book::bad,
    db::Db,
    mail::Email,
    pages::{Meta, escape, site_url},
    web::{self, Ctx, Head, Res, production},
};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS users (
    id bigserial PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    email text NOT NULL,
    password_hash text NOT NULL,
    name text NOT NULL,
    phone text NOT NULL DEFAULT '',
    facebook text NOT NULL DEFAULT '',
    verified_at timestamptz
);
CREATE UNIQUE INDEX IF NOT EXISTS users_email ON users (lower(email));
CREATE TABLE IF NOT EXISTS user_sessions (
    token_hash text PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);
CREATE TABLE IF NOT EXISTS email_tokens (
    token_hash text PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind text NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at timestamptz
);
ALTER TABLE reservations ADD COLUMN IF NOT EXISTS user_id bigint REFERENCES users (id) ON DELETE SET NULL;
ALTER TABLE users ADD COLUMN IF NOT EXISTS banned_at timestamptz;
ALTER TABLE users ADD COLUMN IF NOT EXISTS ban_reason text NOT NULL DEFAULT '';
";

const COOKIE: &str = "sd_user";
const SESSION_DAYS: i32 = 30;
const VERIFY_HOURS: i32 = 48;
const RESET_HOURS: i32 = 1;
const EMAIL_LOGO: &str = "https://azzan-art.github.io/skibidink/img/logo.png";

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/account", get(page))
        .route("/api/account/me", get(me))
        .route("/api/account/register", post(register))
        .route("/api/account/verify", get(verify))
        .route("/api/account/resend", post(resend))
        .route("/api/account/login", post(login))
        .route("/api/account/logout", post(logout))
        .route("/api/account/forgot", post(forgot))
        .route("/api/account/reset", post(reset))
        .route("/api/account/profile", post(profile))
        .route("/api/account/reservations", get(my_reservations))
}

pub struct User {
    pub id: i64,
    pub name: String,
    pub phone: String,
    pub facebook: String,
    pub banned: bool,
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(hex::encode(bytes))
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn hash_password(password: &str) -> Result<String, String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| e.to_string())?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| e.to_string())?;
    Argon2::default().hash_password(password.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| e.to_string())
}

fn decoy_hash() -> &'static str {
    static DECOY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DECOY.get_or_init(|| hash_password("decoy password").unwrap_or_default())
}

fn password_matches(password: &str, stored: &str) -> bool {
    PasswordHash::new(stored).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

/// Argon2 is deliberately slow; it runs off the async workers.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())
}

fn clean_email(s: &str) -> Option<String> {
    let e = s.trim().to_lowercase();
    let (local, domain) = e.split_once('@')?;
    let ok = !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && e.len() <= 254 && !e.chars().any(char::is_whitespace);
    ok.then_some(e)
}

async fn dns_has_answer(http: &reqwest::Client, name: &str, kind: &str) -> Option<bool> {
    let res = http
        .get("https://dns.google/resolve")
        .query(&[("name", name), ("type", kind)])
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .ok()?;
    let v: Value = res.json().await.ok()?;
    let status = v["Status"].as_i64()?;
    Some(status == 0 && v["Answer"].as_array().is_some_and(|a| !a.is_empty()))
}

async fn domain_takes_mail(http: &reqwest::Client, domain: &str) -> bool {
    match dns_has_answer(http, domain, "MX").await {
        Some(true) | None => true,
        Some(false) => dns_has_answer(http, domain, "A").await != Some(false),
    }
}

fn password_problem(p: &str) -> Option<&'static str> {
    match p.chars().count() {
        0..8 => Some("Use at least 8 characters for the password."),
        129.. => Some("That password is too long."),
        _ => None,
    }
}

fn text(v: &Value, k: &str) -> String {
    v[k].as_str().map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Name and contact as the booking rules want them (`book::parse_input`).
fn profile_problem(name: &str, phone: &str, facebook: &str) -> Option<&'static str> {
    if name.chars().count() < 2 || name.chars().count() > 80 {
        return Some("Enter your name.");
    }
    if phone.is_empty() {
        return Some("Enter your phone number so we can reach you about payment.");
    }
    if phone.len() > 30 || !phone.chars().all(|c| c.is_ascii_digit() || " +-()".contains(c)) {
        return Some("That phone number doesn't look right.");
    }
    if facebook.chars().count() > 200 {
        return Some("That Facebook name is too long.");
    }
    None
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_string())
        .filter(|v| !v.is_empty())
}

fn session_cookie(token: &str) -> String {
    let secure = if production() { "; Secure" } else { "" };
    format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}", SESSION_DAYS as i64 * 86_400)
}

fn cleared_cookie() -> String {
    format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

async fn start_session(db: &Db, user_id: i64) -> Result<String, String> {
    let token = random_token()?;
    db.execute(
        "INSERT INTO user_sessions (token_hash, user_id, expires_at) VALUES ($1, $2::bigint, now() + make_interval(days => $3::int))",
        &[&token_hash(&token), &user_id, &SESSION_DAYS],
    )
    .await?;
    Ok(token)
}

/// The signed-in, verified user behind this request, if any.
pub async fn current(app: &App, headers: &HeaderMap) -> Result<Option<User>, String> {
    let (Some(token), Some(db)) = (cookie_token(headers), app.db.as_ref()) else { return Ok(None) };
    let row = db
        .text(
            "SELECT json_build_object('id', u.id, 'name', u.name, 'phone', u.phone, 'facebook', u.facebook, 'banned', u.banned_at IS NOT NULL)::text
               FROM user_sessions s JOIN users u ON u.id = s.user_id
              WHERE s.token_hash = $1 AND s.expires_at > now() AND u.verified_at IS NOT NULL",
            &[&token_hash(&token)],
        )
        .await?;
    Ok(row.and_then(|r| serde_json::from_str::<Value>(&r).ok()).map(|v| User {
        id: v["id"].as_i64().unwrap_or(0),
        name: text(&v, "name"),
        phone: text(&v, "phone"),
        facebook: text(&v, "facebook"),
        banned: v["banned"].as_bool().unwrap_or(false),
    }))
}

async fn email_token(db: &Db, user_id: i64, kind: &str, hours: i32) -> Result<String, String> {
    let token = random_token()?;
    db.execute("UPDATE email_tokens SET used_at = now() WHERE user_id = $1::bigint AND kind = $2 AND used_at IS NULL", &[&user_id, &kind]).await?;
    db.execute(
        "INSERT INTO email_tokens (token_hash, user_id, kind, expires_at) VALUES ($1, $2::bigint, $3, now() + make_interval(hours => $4::int))",
        &[&token_hash(&token), &user_id, &kind, &hours],
    )
    .await?;
    Ok(token)
}

/// Marks the token used and answers its user, or None when it is unknown, used or expired.
async fn use_token(db: &Db, token: &str, kind: &str) -> Result<Option<i64>, String> {
    Ok(db
        .text(
            "UPDATE email_tokens SET used_at = now() WHERE token_hash = $1 AND kind = $2 AND used_at IS NULL AND expires_at > now() RETURNING user_id::text",
            &[&token_hash(token), &kind],
        )
        .await?
        .and_then(|s| s.parse().ok()))
}

fn email_html(name: &str, lead: &str, button: &str, link: &str, after: &str) -> String {
    let (name, lead, button, after) = (escape(name), escape(lead), escape(button), escape(after));
    format!(
        r#"<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background:#f1f5f9"><tr><td align="center" style="padding:32px 16px">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="max-width:480px;background:#ffffff;border-radius:16px;font-family:Arial,sans-serif;color:#1e293b;text-align:center">
<tr><td align="center" style="padding:36px 32px 8px"><img src="{EMAIL_LOGO}" width="220" height="42" alt="SkibiDink" style="display:block;border:0;width:220px;height:auto"></td></tr>
<tr><td align="center" style="padding:20px 32px 0;font-size:18px;font-weight:bold">Hi {name},</td></tr>
<tr><td align="center" style="padding:10px 32px 0;font-size:15px;line-height:1.6">{lead}</td></tr>
<tr><td align="center" style="padding:28px 32px"><a href="{link}" style="display:inline-block;background:#00a04b;color:#ffffff;padding:14px 28px;border-radius:10px;text-decoration:none;font-weight:bold;font-size:15px">{button}</a></td></tr>
<tr><td align="center" style="padding:0 32px 8px;font-size:13px;line-height:1.5;color:#64748b">{after}</td></tr>
<tr><td align="center" style="padding:8px 32px 36px;font-size:12px;line-height:1.5;color:#94a3b8;word-break:break-all">{link}</td></tr>
</table>
</td></tr></table>"#
    )
}

async fn send_verify(app: &App, user_id: i64, email: &str, name: &str) -> Result<(), String> {
    let db = app.db.as_ref().ok_or("no database")?;
    let token = email_token(db, user_id, "verify", VERIFY_HOURS).await.map_err(|e| e.to_string())?;
    let link = format!("{}/api/account/verify?token={token}", site_url());
    let lead = "Confirm your email to finish your SkibiDink account. Then you can book the court.";
    let after = "The link works for 48 hours. If you didn't sign up, ignore this email.";
    app.mailer
        .send(&app.http, Email { to: email, subject: "Confirm your SkibiDink account", text: &format!("Hi {name},\n\n{lead}\n\n{link}\n\n{after}"), html: &email_html(name, lead, "Confirm my email", &link, after) })
        .await
}

pub async fn send_taken(app: &App, email: &str, name: &str, slots: &[String]) -> Result<(), String> {
    let link = format!("{}/book", site_url());
    let (it, them) = if slots.len() == 1 { ("this hour", "it") } else { ("these hours", "them") };
    let lead = format!("Someone else just booked and paid for {it} you asked for: {}. Your request can't go ahead for {them}, but other times are still open.", slots.join("; "));
    let after = "Nothing was charged for the taken hours. If you already paid for them, we'll refund you or move your booking.";
    app.mailer
        .send(
            &app.http,
            Email {
                to: email,
                subject: if slots.len() == 1 { "The hour you asked for was booked" } else { "The hours you asked for were booked" },
                text: &format!("Hi {name},\n\n{lead}\n\nPick another time: {link}\n\n{after}"),
                html: &email_html(name, &lead, "Pick another time", &link, after),
            },
        )
        .await
}

async fn send_reset(app: &App, user_id: i64, email: &str, name: &str) -> Result<(), String> {
    let db = app.db.as_ref().ok_or("no database")?;
    let token = email_token(db, user_id, "reset", RESET_HOURS).await.map_err(|e| e.to_string())?;
    let link = format!("{}/account?reset={token}", site_url());
    let lead = "Someone asked to reset the password of your SkibiDink account. Pick a new one here.";
    let after = "The link works for 1 hour. If it wasn't you, ignore this email: your password stays the same.";
    app.mailer
        .send(&app.http, Email { to: email, subject: "Reset your SkibiDink password", text: &format!("Hi {name},\n\n{lead}\n\n{link}\n\n{after}"), html: &email_html(name, lead, "Choose a new password", &link, after) })
        .await
}

fn limited(app: &App, ctx: &Ctx) -> Option<Response> {
    (!app.account_buckets.allows(&ctx.ip)).then(|| web::json(429, json!({ "success": false, "error": "Too many tries. Wait a minute." })))
}

async fn page(State(app): State<Arc<App>>) -> Res {
    let mut meta = Meta::new("/account").title("Your account | SkibiDink").description("Sign in or create an account to book the court.");
    meta.robots = "noindex, nofollow".into();
    let path = app.root.join("public/account.html");
    let html = app
        .pages
        .cached("account.html|/account", || async {
            let body = tokio::fs::read_to_string(&path).await.map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(app.pages.compose(&meta, &body))
        })
        .await?;
    Ok(web::html(200, html.as_str().to_owned()).head("Cache-Control", "private, no-cache"))
}

async fn me(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    let (Some(token), Ok(db)) = (cookie_token(&headers), app.db()) else { return Ok(web::json(401, json!({ "success": false }))) };
    let row = db
        .text(
            "SELECT json_build_object('email', u.email, 'name', u.name, 'phone', u.phone, 'facebook', u.facebook,
                                      'banned', u.banned_at IS NOT NULL, 'banReason', u.ban_reason)::text
               FROM user_sessions s JOIN users u ON u.id = s.user_id
              WHERE s.token_hash = $1 AND s.expires_at > now() AND u.verified_at IS NOT NULL",
            &[&token_hash(&token)],
        )
        .await?;
    Ok(match row {
        Some(user) => web::respond(200, "application/json; charset=utf-8", format!(r#"{{"success":true,"user":{user}}}"#)),
        None => web::json(401, json!({ "success": false })),
    })
}

// SECURITY: the answer is the same whether or not the email has an account, so the form cannot be
// used to find out who is registered. A taken, verified address gets an email instead.
async fn register(State(app): State<Arc<App>>, Extension(ctx): Extension<Ctx>, body: Bytes) -> Res {
    if let Some(r) = limited(&app, &ctx) {
        return Ok(r);
    }
    let db = app.db()?;
    let Ok(v) = serde_json::from_slice::<Value>(&body) else { return Ok(bad("Send the form again.")) };
    let Some(email) = clean_email(&text(&v, "email")) else { return Ok(bad("That email doesn't look right.")) };
    let password = v["password"].as_str().unwrap_or("").to_string();
    let (name, phone, facebook) = (text(&v, "name"), text(&v, "phone"), text(&v, "facebook"));
    if let Some(msg) = password_problem(&password).or(profile_problem(&name, &phone, &facebook)) {
        return Ok(bad(msg));
    }
    let domain = email.rsplit_once('@').map_or("", |(_, d)| d);
    if !domain_takes_mail(&app.http, domain).await {
        return Ok(bad("We can't send email to that address. Please use a Gmail account."));
    }
    let hash = blocking(move || hash_password(&password)).await??;
    let existing = db
        .text("SELECT json_build_object('id', id, 'name', name, 'verified', verified_at IS NOT NULL)::text FROM users WHERE lower(email) = $1", &[&email])
        .await?
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let sent = match existing {
        Some(u) if u["verified"].as_bool() == Some(true) => {
            let name = text(&u, "name");
            let link = format!("{}/account", site_url());
            let lead = "Someone tried to create a SkibiDink account with this email, but you already have one. Sign in, or use \"Forgot password\" there.";
            app.mailer
                .send(&app.http, Email { to: &email, subject: "You already have a SkibiDink account", text: &format!("Hi {name},\n\n{lead}\n\n{link}"), html: &email_html(&name, lead, "Sign in", &link, "If this wasn't you, ignore this email.") })
                .await
        }
        Some(u) => {
            let id = u["id"].as_i64().unwrap_or(0);
            db.execute("UPDATE users SET password_hash = $2, name = $3, phone = $4, facebook = $5 WHERE id = $1::bigint", &[&id, &hash, &name, &phone, &facebook]).await?;
            send_verify(&app, id, &email, &name).await
        }
        None => {
            let id: i64 = db
                .text(
                    "INSERT INTO users (email, password_hash, name, phone, facebook) VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING RETURNING id::text",
                    &[&email, &hash, &name, &phone, &facebook],
                )
                .await?
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if id == 0 { Ok(()) } else { send_verify(&app, id, &email, &name).await }
        }
    };
    if let Err(err) = sent {
        eprintln!("[mail] register email to {email} failed: {err}");
        return Ok(web::json(502, json!({ "success": false, "error": "We couldn't send the email. Try again in a few minutes." })));
    }
    Ok(web::json(200, json!({ "success": true })))
}

async fn verify(State(app): State<Arc<App>>, Query(q): Query<BTreeMap<String, String>>) -> Res {
    let db = app.db()?;
    let token = q.get("token").cloned().unwrap_or_default();
    let Some(user_id) = use_token(db, &token, "verify").await? else { return Ok(web::redirect(302, "/account?verify=expired")) };
    db.execute("UPDATE users SET verified_at = coalesce(verified_at, now()) WHERE id = $1::bigint", &[&user_id]).await?;
    let session = start_session(db, user_id).await?;
    Ok(web::redirect(302, "/account?verified=1").head("Set-Cookie", session_cookie(&session)))
}

async fn resend(State(app): State<Arc<App>>, Extension(ctx): Extension<Ctx>, body: Bytes) -> Res {
    if let Some(r) = limited(&app, &ctx) {
        return Ok(r);
    }
    let db = app.db()?;
    let v: Value = serde_json::from_slice(&body).unwrap_or_default();
    if let Some(email) = clean_email(&text(&v, "email")) {
        let row = db
            .text("SELECT json_build_object('id', id, 'name', name)::text FROM users WHERE lower(email) = $1 AND verified_at IS NULL", &[&email])
            .await?
            .and_then(|s| serde_json::from_str::<Value>(&s).ok());
        if let Some(u) = row
            && let Err(err) = send_verify(&app, u["id"].as_i64().unwrap_or(0), &email, &text(&u, "name")).await
        {
            eprintln!("[mail] verify email to {email} failed: {err}");
        }
    }
    Ok(web::json(200, json!({ "success": true })))
}

async fn login(State(app): State<Arc<App>>, Extension(ctx): Extension<Ctx>, body: Bytes) -> Res {
    if let Some(r) = limited(&app, &ctx) {
        return Ok(r);
    }
    let db = app.db()?;
    let v: Value = serde_json::from_slice(&body).unwrap_or_default();
    let email = clean_email(&text(&v, "email")).unwrap_or_default();
    let password = v["password"].as_str().unwrap_or("").to_string();
    let row = db
        .text("SELECT json_build_object('id', id, 'hash', password_hash, 'verified', verified_at IS NOT NULL)::text FROM users WHERE lower(email) = $1", &[&email])
        .await?
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    // SECURITY: an unknown email still pays for one hash, so the answer time does not reveal
    // which addresses have accounts.
    let stored = row.as_ref().map(|u| text(u, "hash"));
    let ok = blocking(move || match &stored {
        Some(hash) => password_matches(&password, hash),
        None => password_matches(&password, decoy_hash()),
    }).await?;
    let Some(user) = row.filter(|_| ok) else { return Ok(web::json(401, json!({ "success": false, "error": "Wrong email or password." }))) };
    if user["verified"].as_bool() != Some(true) {
        return Ok(web::json(403, json!({ "success": false, "unverified": true, "error": "Confirm your email first. Check your inbox (and spam)." })));
    }
    let session = start_session(db, user["id"].as_i64().unwrap_or(0)).await?;
    Ok(web::json(200, json!({ "success": true })).head("Set-Cookie", session_cookie(&session)))
}

async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    if let (Some(token), Some(db)) = (cookie_token(&headers), app.db.as_ref()) {
        db.execute("DELETE FROM user_sessions WHERE token_hash = $1", &[&token_hash(&token)]).await?;
    }
    Ok(web::json(200, json!({ "success": true })).head("Set-Cookie", cleared_cookie()))
}

async fn forgot(State(app): State<Arc<App>>, Extension(ctx): Extension<Ctx>, body: Bytes) -> Res {
    if let Some(r) = limited(&app, &ctx) {
        return Ok(r);
    }
    let db = app.db()?;
    let v: Value = serde_json::from_slice(&body).unwrap_or_default();
    if let Some(email) = clean_email(&text(&v, "email")) {
        let row = db
            .text("SELECT json_build_object('id', id, 'name', name)::text FROM users WHERE lower(email) = $1", &[&email])
            .await?
            .and_then(|s| serde_json::from_str::<Value>(&s).ok());
        if let Some(u) = row
            && let Err(err) = send_reset(&app, u["id"].as_i64().unwrap_or(0), &email, &text(&u, "name")).await
        {
            eprintln!("[mail] reset email to {email} failed: {err}");
        }
    }
    Ok(web::json(200, json!({ "success": true })))
}

/// A reset also verifies the email (the link proved it) and signs out every other session.
async fn reset(State(app): State<Arc<App>>, Extension(ctx): Extension<Ctx>, body: Bytes) -> Res {
    if let Some(r) = limited(&app, &ctx) {
        return Ok(r);
    }
    let db = app.db()?;
    let v: Value = serde_json::from_slice(&body).unwrap_or_default();
    let password = v["password"].as_str().unwrap_or("").to_string();
    if let Some(msg) = password_problem(&password) {
        return Ok(bad(msg));
    }
    let Some(user_id) = use_token(db, &text(&v, "token"), "reset").await? else {
        return Ok(bad("This reset link has expired or was already used. Ask for a new one."));
    };
    let hash = blocking(move || hash_password(&password)).await??;
    db.execute("UPDATE users SET password_hash = $2, verified_at = coalesce(verified_at, now()) WHERE id = $1::bigint", &[&user_id, &hash]).await?;
    db.execute("DELETE FROM user_sessions WHERE user_id = $1::bigint", &[&user_id]).await?;
    let session = start_session(db, user_id).await?;
    Ok(web::json(200, json!({ "success": true })).head("Set-Cookie", session_cookie(&session)))
}

async fn profile(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Res {
    let Some(user) = current(&app, &headers).await? else { return Ok(web::json(401, json!({ "success": false, "error": "Sign in again." }))) };
    let db = app.db()?;
    let v: Value = serde_json::from_slice(&body).unwrap_or_default();
    let (name, phone, facebook) = (text(&v, "name"), text(&v, "phone"), text(&v, "facebook"));
    if let Some(msg) = profile_problem(&name, &phone, &facebook) {
        return Ok(bad(msg));
    }
    db.execute("UPDATE users SET name = $2, phone = $3, facebook = $4 WHERE id = $1::bigint", &[&user.id, &name, &phone, &facebook]).await?;
    Ok(web::json(200, json!({ "success": true })))
}

async fn my_reservations(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    let Some(user) = current(&app, &headers).await? else { return Ok(web::json(401, json!({ "success": false }))) };
    let db = app.db()?;
    let rows = db
        .text(
            "SELECT coalesce(json_agg(x ORDER BY x.day DESC, x.start_hour DESC), '[]')::text FROM (
               SELECT id, day::text AS day, start_hour, hours, players, status, pay_method, pay_by, paid_at, taken_at
                 FROM reservations WHERE user_id = $1::bigint ORDER BY day DESC LIMIT 100) x",
            &[&user.id],
        )
        .await?
        .unwrap_or_else(|| "[]".into());
    Ok(web::respond(200, "application/json; charset=utf-8", format!(r#"{{"success":true,"items":{rows}}}"#)))
}

/// A verified account from `DEMO_EMAIL` / `DEMO_PASSWORD`, for trying the site locally. Its
/// password is reset to `DEMO_PASSWORD` on every boot.
pub async fn seed_demo(db: &Db) -> Result<(), String> {
    // SECURITY: never in production, where a known password would be a way in.
    if production() {
        return Ok(());
    }
    let (Ok(email), Ok(password)) = (std::env::var("DEMO_EMAIL"), std::env::var("DEMO_PASSWORD")) else { return Ok(()) };
    let Some(email) = clean_email(&email) else { return Err("DEMO_EMAIL is not an email".into()) };
    if password_problem(&password).is_some() {
        return Err("DEMO_PASSWORD needs at least 8 characters".into());
    }
    let hash = blocking(move || hash_password(&password)).await??;
    db.execute(
        "INSERT INTO users (email, password_hash, name, phone, facebook, verified_at) VALUES ($1, $2, 'Demo Player', '0917 000 0000', '', now())
         ON CONFLICT (lower(email)) DO UPDATE SET password_hash = EXCLUDED.password_hash, verified_at = coalesce(users.verified_at, now())",
        &[&email, &hash],
    )
    .await?;
    println!("[demo] account {email} ready");
    Ok(())
}

/// Expired sessions and email tokens.
pub async fn sweep(db: &Db) -> Result<(), String> {
    db.execute("DELETE FROM user_sessions WHERE expires_at < now()", &[]).await?;
    db.execute("DELETE FROM email_tokens WHERE expires_at < now() - interval '1 day'", &[]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_checks_passwords() {
        let h = hash_password("dink-dink-123").unwrap();
        assert!(password_matches("dink-dink-123", &h));
        assert!(!password_matches("dink-dink-124", &h));
        assert!(!password_matches("x", "not a hash"));
    }

    #[test]
    fn cleans_emails() {
        assert_eq!(clean_email("  Ana@Gmail.com ").as_deref(), Some("ana@gmail.com"));
        assert!(clean_email("ana@gmail").is_none());
        assert!(clean_email("ana gmail.com").is_none());
        assert!(clean_email("a b@gmail.com").is_none());
    }

    #[test]
    fn checks_profiles_and_passwords() {
        assert!(profile_problem("Ana Cruz", "0917 000 0000", "").is_none());
        assert!(profile_problem("Ana Cruz", "", "ana.fb").is_some());
        assert!(password_problem("short").is_some());
        assert!(password_problem("long enough").is_none());
    }
}
