use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Extension, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Response,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{
    App,
    book::{self, CLOSE, Confirmed, New, OPEN, STATUSES, bad},
    date::{Day, manila_now, parse_month},
    pages::Meta,
    poster,
    web::{self, Ctx, Head, Res, production},
};

const COOKIE: &str = "sd_admin";
const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);

pub struct Sessions(Mutex<HashMap<String, Instant>>);

impl Sessions {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    fn start(&self) -> Result<String, String> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        let token = hex::encode(bytes);
        let mut map = self.0.lock().unwrap();
        let now = Instant::now();
        map.retain(|_, at| now.duration_since(*at) < SESSION_TTL);
        map.insert(token.clone(), now);
        Ok(token)
    }

    fn valid(&self, token: &str) -> bool {
        self.0.lock().unwrap().get(token).is_some_and(|at| at.elapsed() < SESSION_TTL)
    }

    fn end(&self, token: &str) {
        self.0.lock().unwrap().remove(token);
    }
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
}

fn signed_in(app: &App, headers: &HeaderMap) -> bool {
    cookie_token(headers).is_some_and(|t| app.sessions.valid(&t))
}

fn denied() -> Response {
    web::json(401, json!({ "success": false, "error": "Sign in again." }))
}

macro_rules! guard {
    ($app:expr, $headers:expr) => {
        if !signed_in(&$app, &$headers) {
            return Ok(denied());
        }
    };
}

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/admin", get(page))
        .route("/api/admin/login", post(login))
        .route("/api/admin/logout", post(logout))
        .route("/api/admin/me", get(me))
        .route("/api/admin/reservations", get(list).post(walk_in))
        .route("/api/admin/reservations/{id}", post(update))
        .route("/api/admin/users/{id}/ban", post(ban))
        .route("/api/admin/calendar", get(calendar))
        .route("/api/admin/stats", get(stats))
        .route("/api/admin/poster/{kind}", get(poster_png))
}

async fn page(State(app): State<Arc<App>>) -> Res {
    let mut meta = Meta::new("/admin").title("SkibiDink | Admin").description("Reservations admin.");
    meta.robots = "noindex, nofollow".into();
    let path = app.root.join("public/admin.html");
    let html = app
        .pages
        .cached("admin.html|/admin", || async {
            let body = tokio::fs::read_to_string(&path).await.map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(app.pages.compose(&meta, &body))
        })
        .await?;
    Ok(web::html(200, html.as_str().to_owned()).head("Cache-Control", "private, no-cache"))
}

// SECURITY: constant-time compare, and a per-address bucket so the password cannot be guessed fast.
async fn login(State(app): State<Arc<App>>, Extension(ctx): Extension<Ctx>, body: Bytes) -> Res {
    if !app.login_buckets.allows(&ctx.ip) {
        return Ok(web::json(429, json!({ "success": false, "error": "Too many tries. Wait a minute." })));
    }
    let password = std::env::var("ADMIN_PASSWORD").unwrap_or_default();
    if password.len() < 8 {
        return Ok(web::json(503, json!({ "success": false, "error": "ADMIN_PASSWORD is not set on the server." })));
    }
    let given = serde_json::from_slice::<Value>(&body).ok().and_then(|v| v["password"].as_str().map(str::to_string)).unwrap_or_default();
    let same = given.len() == password.len() && given.bytes().zip(password.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0;
    if !same {
        return Ok(web::json(401, json!({ "success": false, "error": "Wrong password." })));
    }
    let token = app.sessions.start()?;
    let secure = if production() { "; Secure" } else { "" };
    Ok(web::json(200, json!({ "success": true })).head("Set-Cookie", &format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{secure}", SESSION_TTL.as_secs())))
}

async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    if let Some(t) = cookie_token(&headers) {
        app.sessions.end(&t);
    }
    Ok(web::json(200, json!({ "success": true })).head("Set-Cookie", &format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0")))
}

async fn me(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    guard!(app, headers);
    let (today, hour) = manila_now();
    Ok(web::json(
        200,
        json!({ "success": true, "today": today.iso(), "hour": hour, "open": OPEN, "close": CLOSE, "db": app.db.is_some(), "online": crate::pay::enabled() }),
    ))
}

const ROW: &str = "r.id, r.created_at, r.name, r.phone, r.facebook, r.day::text AS day, r.start_hour, r.hours, r.players,
    r.note, r.status, r.pay_method, r.pay_by, r.paid_at, r.amount, r.decided_at,
    r.user_id, r.taken_at, u.email, u.banned_at, u.ban_reason";

async fn list(State(app): State<Arc<App>>, headers: HeaderMap, Query(q): Query<BTreeMap<String, String>>) -> Res {
    guard!(app, headers);
    let db = app.db()?;
    let status = q.get("status").map(String::as_str).filter(|s| STATUSES.contains(s)).unwrap_or("").to_string();
    let scope = q.get("scope").map(String::as_str).filter(|s| ["upcoming", "past", "all"].contains(s)).unwrap_or("upcoming").to_string();
    let search = q.get("q").map(|s| s.trim().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")).unwrap_or_default();
    let (today, _) = manila_now();
    let sql = format!(
        "SELECT coalesce(json_agg(x ORDER BY x.day, x.start_hour, x.created_at), '[]')::text FROM (
           SELECT {ROW},
             (SELECT coalesce(json_agg(json_build_object('id', o.id, 'name', o.name, 'phone', o.phone, 'facebook', o.facebook,
                       'status', o.status, 'start_hour', o.start_hour, 'hours', o.hours) ORDER BY o.created_at), '[]')
                FROM reservations o
               WHERE o.id <> r.id AND o.day = r.day AND o.status IN ('requested', 'awaiting', 'confirmed')
                 AND o.start_hour < r.start_hour + r.hours AND o.start_hour + o.hours > r.start_hour) AS competing
             FROM reservations r LEFT JOIN users u ON u.id = r.user_id
            WHERE ($1::text = '' OR r.status = $1::text)
              AND ($2::text = '' OR r.name ILIKE '%' || $2::text || '%' OR r.phone ILIKE '%' || $2::text || '%'
                   OR r.facebook ILIKE '%' || $2::text || '%' OR r.note ILIKE '%' || $2::text || '%'
                   OR r.day::text LIKE '%' || $2::text || '%' OR r.id::text = $2::text)
              AND ($3::text = 'all' OR ($3::text = 'upcoming' AND r.day >= $4::text::date) OR ($3::text = 'past' AND r.day < $4::text::date))
            ORDER BY CASE WHEN $3::text = 'past' THEN -(r.day - DATE '2000-01-01') ELSE r.day - DATE '2000-01-01' END, r.start_hour, r.created_at
            LIMIT 500) x"
    );
    let rows = db.text(&sql, &[&status, &search, &scope, &today.iso()]).await?.unwrap_or_else(|| "[]".into());
    Ok(web::respond(200, "application/json; charset=utf-8", format!(r#"{{"success":true,"items":{rows}}}"#)))
}

async fn walk_in(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Res {
    guard!(app, headers);
    let db = app.db()?;
    let Ok(v) = serde_json::from_slice::<Value>(&body) else { return Ok(bad("Send the form again.")) };
    let input = match book::parse_input(&v, true) {
        Ok(i) => i,
        Err(msg) => return Ok(bad(msg)),
    };
    let paid = v["paid"].as_bool().unwrap_or(false);
    let id = book::insert(db, New { input: &input, status: "requested", pay_method: "walkin", pay_window_mins: 0, paid: false, amount: None, ip: "admin", user_id: None }).await?;
    match book::confirm(&app, id, paid).await? {
        Confirmed::Done(taken) => Ok(web::json(200, json!({ "success": true, "id": id, "taken": taken }))),
        _ => {
            db.execute("DELETE FROM reservations WHERE id = $1::bigint", &[&id]).await?;
            Ok(web::json(409, json!({ "success": false, "error": "Those hours are already confirmed for someone else." })))
        }
    }
}

async fn update(State(app): State<Arc<App>>, headers: HeaderMap, Path(id): Path<i64>, body: Bytes) -> Res {
    guard!(app, headers);
    let db = app.db()?;
    let Ok(v) = serde_json::from_slice::<Value>(&body) else { return Ok(bad("Send it again.")) };
    if let Some(paid) = v["paid"].as_bool() {
        db.execute("UPDATE reservations SET paid_at = CASE WHEN $2::bool THEN coalesce(paid_at, now()) END WHERE id = $1::bigint", &[&id, &paid]).await?;
    }
    let Some(status) = v["status"].as_str() else { return Ok(web::json(200, json!({ "success": true }))) };
    if !STATUSES.contains(&status) {
        return Ok(bad("Unknown status."));
    }
    if status == "confirmed" {
        let paid = v["paid"].as_bool().unwrap_or(false);
        return Ok(match book::confirm(&app, id, paid).await? {
            Confirmed::Done(taken) => web::json(200, json!({ "success": true, "taken": taken })),
            Confirmed::Taken => web::json(409, json!({ "success": false, "error": "Those hours are already confirmed for someone else. Decline one first." })),
            Confirmed::Missing => web::json(404, json!({ "success": false, "error": "Not found." })),
        });
    }
    let pay_by_hours = v["payByHours"].as_i64().unwrap_or(0).clamp(0, 168) as i32;
    let n = db
        .execute(
            "UPDATE reservations SET status = $2, decided_at = now(),
               pay_by = CASE WHEN $2 = 'awaiting' AND $3::int > 0 THEN now() + make_interval(hours => $3::int)
                             WHEN $2 = 'awaiting' THEN pay_by END
             WHERE id = $1::bigint",
            &[&id, &status, &pay_by_hours],
        )
        .await?;
    book::reopen_taken(db, id).await?;
    Ok(if n == 0 { web::json(404, json!({ "success": false, "error": "Not found." })) } else { web::json(200, json!({ "success": true })) })
}

async fn ban(State(app): State<Arc<App>>, headers: HeaderMap, Path(id): Path<i64>, body: Bytes) -> Res {
    guard!(app, headers);
    let db = app.db()?;
    let Ok(v) = serde_json::from_slice::<Value>(&body) else { return Ok(bad("Send it again.")) };
    let on = v["ban"].as_bool().unwrap_or(true);
    let reason: String = v["reason"].as_str().unwrap_or("").trim().chars().take(200).collect();
    let n = db
        .execute(
            "UPDATE users SET banned_at = CASE WHEN $2::bool THEN coalesce(banned_at, now()) END, ban_reason = CASE WHEN $2::bool THEN $3 ELSE '' END WHERE id = $1::bigint",
            &[&id, &on, &reason],
        )
        .await?;
    Ok(if n == 0 { web::json(404, json!({ "success": false, "error": "Not found." })) } else { web::json(200, json!({ "success": true })) })
}

fn range(q: &BTreeMap<String, String>) -> Result<(Day, Day), &'static str> {
    let from = q.get("from").and_then(|s| Day::parse(s)).ok_or("Bad from date.")?;
    let to = q.get("to").and_then(|s| Day::parse(s)).ok_or("Bad to date.")?;
    if to < from || from.days_until(to) > 62 {
        return Err("Pick at most two months.");
    }
    Ok((from, to))
}

async fn calendar(State(app): State<Arc<App>>, headers: HeaderMap, Query(q): Query<BTreeMap<String, String>>) -> Res {
    guard!(app, headers);
    let db = app.db()?;
    let (from, to) = match range(&q) {
        Ok(r) => r,
        Err(msg) => return Ok(bad(msg)),
    };
    let sql = format!(
        "SELECT coalesce(json_agg(x ORDER BY x.day, x.start_hour), '[]')::text FROM (
           SELECT {ROW} FROM reservations r LEFT JOIN users u ON u.id = r.user_id WHERE r.status = 'confirmed' AND r.day BETWEEN $1::text::date AND $2::text::date) x"
    );
    let rows = db.text(&sql, &[&from.iso(), &to.iso()]).await?.unwrap_or_else(|| "[]".into());
    Ok(web::respond(200, "application/json; charset=utf-8", format!(r#"{{"success":true,"items":{rows}}}"#)))
}

async fn stats(State(app): State<Arc<App>>, headers: HeaderMap) -> Res {
    guard!(app, headers);
    let db = app.db()?;
    let (today, _) = manila_now();
    let sql = format!(
        r"WITH c AS (SELECT * FROM reservations WHERE status = 'confirmed'),
         people AS (
           SELECT coalesce('u' || user_id, nullif(regexp_replace(phone, '\D', '', 'g'), ''), nullif(lower(trim(facebook)), ''), lower(trim(name))) AS who,
                  count(*) AS n
             FROM c GROUP BY 1)
         SELECT json_build_object(
           'requests', (SELECT count(*) FROM reservations),
           'requested', (SELECT count(*) FROM reservations WHERE status = 'requested'),
           'awaiting', (SELECT count(*) FROM reservations WHERE status = 'awaiting'),
           'confirmed', (SELECT count(*) FROM c),
           'declined', (SELECT count(*) FROM reservations WHERE status IN ('declined', 'cancelled')),
           'players', (SELECT coalesce(sum(players), 0) FROM c),
           'playersPlayed', (SELECT coalesce(sum(players), 0) FROM c WHERE day <= $1::text::date),
           'hours', (SELECT coalesce(sum(hours), 0) FROM c),
           'customers', (SELECT count(*) FROM people),
           'returning', (SELECT count(*) FROM people WHERE n > 1),
           'online', (SELECT count(*) FROM c WHERE pay_method = 'online'),
           'walkIns', (SELECT count(*) FROM c WHERE pay_method = 'walkin'),
           'upcoming', (SELECT count(*) FROM c WHERE day >= $1::text::date),
           'months', (SELECT json_agg(json_build_object(
                         'month', to_char(m, 'YYYY-MM'),
                         'bookings', (SELECT count(*) FROM c WHERE date_trunc('month', c.day)::date = m::date),
                         'players', (SELECT coalesce(sum(players), 0) FROM c WHERE date_trunc('month', c.day)::date = m::date)) ORDER BY m)
                        FROM generate_series(date_trunc('month', $1::text::date) - interval '5 months', date_trunc('month', $1::text::date), interval '1 month') m),
           'byHour', (SELECT json_agg((SELECT count(*) FROM c WHERE c.start_hour <= h AND c.start_hour + c.hours > h) ORDER BY h)
                        FROM generate_series({OPEN}, {last}) h),
           'byWeekday', (SELECT json_agg((SELECT count(*) FROM c WHERE extract(isodow FROM c.day) = d) ORDER BY d) FROM generate_series(1, 7) d)
         )::text",
        last = CLOSE - 1,
    );
    let body = db.text(&sql, &[&today.iso()]).await?.unwrap_or_else(|| "{}".into());
    Ok(web::respond(200, "application/json; charset=utf-8", format!(r#"{{"success":true,"open":{OPEN},"stats":{body}}}"#)))
}

async fn poster_png(State(app): State<Arc<App>>, headers: HeaderMap, Path(kind): Path<String>, Query(q): Query<BTreeMap<String, String>>) -> Res {
    guard!(app, headers);
    let db = app.db()?;
    let (today, hour) = manila_now();
    let now = poster::Now { today, hour };
    let (from, days, name) = match kind.as_str() {
        "day" => {
            let Some(d) = q.get("date").and_then(|s| Day::parse(s)) else { return Ok(bad("Bad date.")) };
            (d, 1, format!("skibidink-{}", d.iso()))
        }
        "week" => {
            let Some(d) = q.get("start").and_then(|s| Day::parse(s)) else { return Ok(bad("Bad start date.")) };
            (d, 7, format!("skibidink-week-{}", d.iso()))
        }
        "month" => {
            let Some(d) = q.get("month").and_then(|s| parse_month(s)) else { return Ok(bad("Bad month.")) };
            let (y, m, _) = d.ymd();
            (d, crate::date::month_len(y, m) as i64, format!("skibidink-{}", &d.iso()[..7]))
        }
        _ => return Ok(web::text(404, "Not Found")),
    };
    let booked = book::booked_hours(db, from, days).await?;
    let logo = tokio::fs::read(app.root.join("public/img/logo.png")).await.unwrap_or_default();
    let png = tokio::task::spawn_blocking(move || {
        let svg = match kind.as_str() {
            "day" => poster::day(&logo, &now, &booked, from),
            "week" => poster::week(&logo, &now, &booked, from),
            _ => poster::month(&logo, &now, &booked, from),
        };
        poster::render_png(&svg)
    })
    .await
    .map_err(|e| e.to_string())??;
    let disposition = if q.contains_key("download") { format!("attachment; filename=\"{name}.png\"") } else { format!("inline; filename=\"{name}.png\"") };
    Ok(web::respond(200, "image/png", png).head("Content-Disposition", &disposition).head("Cache-Control", "private, no-store"))
}
