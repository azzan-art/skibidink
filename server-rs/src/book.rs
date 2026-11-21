use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Extension, Router,
    body::Bytes,
    extract::{Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};

use crate::{
    App, account,
    date::{Day, hour_label, manila_now},
    db::Db,
    pay,
    web::{self, Ctx, Res},
};

pub const OPEN: u32 = 6;
pub const CLOSE: u32 = 23;
const BOOK_AHEAD_DAYS: i64 = 60;
const MAX_HOURS: u32 = 4;
const MAX_PICKED_HOURS: usize = 8;
const ONLINE_PAY_WINDOW_MINS: i32 = 60;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS reservations (
    id bigserial PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    name text NOT NULL,
    phone text NOT NULL DEFAULT '',
    facebook text NOT NULL DEFAULT '',
    day date NOT NULL,
    start_hour smallint NOT NULL,
    hours smallint NOT NULL,
    players smallint NOT NULL,
    note text NOT NULL DEFAULT '',
    status text NOT NULL DEFAULT 'requested',
    pay_method text NOT NULL DEFAULT 'arrange',
    pay_by timestamptz,
    paid_at timestamptz,
    amount integer,
    checkout_id text,
    decided_at timestamptz,
    ip text NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS reservations_day ON reservations (day, start_hour);
CREATE INDEX IF NOT EXISTS reservations_checkout ON reservations (checkout_id);
ALTER TABLE reservations ADD COLUMN IF NOT EXISTS bundle bigint;
ALTER TABLE reservations ADD COLUMN IF NOT EXISTS taken_at timestamptz;
";

pub const STATUSES: [&str; 5] = ["requested", "awaiting", "confirmed", "declined", "cancelled"];

pub fn routes() -> Router<Arc<App>> {
    Router::new().route("/api/availability", get(availability)).route("/api/reservations", post(request))
}

pub fn bad(msg: &str) -> axum::response::Response {
    web::json(400, json!({ "success": false, "error": msg }))
}

pub struct Input {
    pub name: String,
    pub phone: String,
    pub facebook: String,
    pub day: Day,
    pub hour: u32,
    pub hours: u32,
    pub players: u32,
    pub note: String,
}

fn field(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// `walk_in` relaxes the window: staff may record a past day or an hour that already started.
pub fn parse_input(v: &Value, walk_in: bool) -> Result<Input, &'static str> {
    let name = field(v, "name");
    let phone = field(v, "phone");
    let facebook = field(v, "facebook");
    let note = field(v, "note");
    if name.chars().count() < 2 || name.chars().count() > 80 {
        return Err("Enter your name.");
    }
    if phone.is_empty() && facebook.is_empty() {
        return Err("Leave a phone number or a Facebook name so we can reach you.");
    }
    if phone.len() > 30 || !phone.chars().all(|c| c.is_ascii_digit() || " +-()".contains(c)) {
        return Err("That phone number doesn't look right.");
    }
    if facebook.chars().count() > 200 || note.chars().count() > 500 {
        return Err("That's too long.");
    }
    let day = Day::parse(&field(v, "date")).ok_or("Pick a day.")?;
    let hour: u32 = field(v, "hour").parse().map_err(|_| "Pick an hour.")?;
    let hours: u32 = field(v, "hours").parse().unwrap_or(1);
    let players: u32 = field(v, "players").parse().map_err(|_| "How many players?")?;
    if !(1..=16).contains(&players) {
        return Err("How many players?");
    }
    let max_hours = if walk_in { CLOSE - OPEN } else { MAX_HOURS };
    if !(OPEN..CLOSE).contains(&hour) || !(1..=max_hours).contains(&hours) || hour + hours > CLOSE {
        return Err("Pick hours inside opening time.");
    }
    if !walk_in {
        let (today, now_hour) = manila_now();
        let ahead = today.days_until(day);
        if ahead < 0 || ahead > BOOK_AHEAD_DAYS || (ahead == 0 && hour <= now_hour) {
            return Err("That hour can't be booked anymore. Pick another.");
        }
    }
    Ok(Input { name, phone, facebook, day, hour, hours, players, note })
}

/// `slots` is a list of `{date, hour}`; runs of back-to-back hours on one day become one block.
fn picked_blocks(v: &Value) -> Result<Vec<(String, u32, u32)>, &'static str> {
    let mut picks: Vec<(String, u32)> = v["slots"]
        .as_array()
        .ok_or("Pick an hour.")?
        .iter()
        .map(|s| Some((field(s, "date"), field(s, "hour").parse().ok()?)))
        .collect::<Option<_>>()
        .ok_or("Pick an hour.")?;
    picks.sort();
    picks.dedup();
    if picks.is_empty() {
        return Err("Pick an hour.");
    }
    if picks.len() > MAX_PICKED_HOURS {
        return Err("That's too many hours for one request.");
    }
    let mut blocks: Vec<(String, u32, u32)> = Vec::new();
    for (date, hour) in picks {
        match blocks.last_mut() {
            Some((d, start, hours)) if *d == date && *start + *hours == hour && *hours < MAX_HOURS => *hours += 1,
            _ => blocks.push((date, hour, 1)),
        }
    }
    Ok(blocks)
}

pub async fn overlaps_confirmed(db: &Db, day: Day, hour: u32, hours: u32, except: i64) -> Result<bool, String> {
    let n = db
        .text(
            "SELECT count(*)::text FROM reservations WHERE day = $1::text::date AND status = 'confirmed'
             AND start_hour < $3::int AND start_hour + hours > $2::int AND id <> $4::bigint",
            &[&day.iso(), &(hour as i32), &((hour + hours) as i32), &except],
        )
        .await?;
    Ok(n.as_deref() != Some("0"))
}

pub struct New<'a> {
    pub input: &'a Input,
    pub status: &'a str,
    pub pay_method: &'a str,
    pub pay_window_mins: i32,
    pub paid: bool,
    pub amount: Option<i32>,
    pub ip: &'a str,
    pub user_id: Option<i64>,
}

pub async fn insert(db: &Db, n: New<'_>) -> Result<i64, String> {
    let i = n.input;
    let id = db
        .text(
            "INSERT INTO reservations (name, phone, facebook, day, start_hour, hours, players, note, status, pay_method, pay_by, paid_at, amount, decided_at, ip, user_id)
             VALUES ($1, $2, $3, $4::text::date, $5::int, $6::int, $7::int, $8, $9, $10,
                     CASE WHEN $11::int > 0 THEN now() + make_interval(mins => $11::int) END,
                     CASE WHEN $12::bool THEN now() END, $13::int,
                     CASE WHEN $9 = 'confirmed' THEN now() END, $14, $15::bigint)
             RETURNING id::text",
            &[&i.name, &i.phone, &i.facebook, &i.day.iso(), &(i.hour as i32), &(i.hours as i32), &(i.players as i32), &i.note, &n.status, &n.pay_method, &n.pay_window_mins, &n.paid, &n.amount, &n.ip, &n.user_id],
        )
        .await?
        .ok_or("insert returned nothing")?;
    id.parse().map_err(|_| "bad id".to_string())
}

pub enum Confirmed {
    /// With how many open requests it took.
    Done(usize),
    Taken,
    Missing,
}

/// The one way a reservation becomes `confirmed`. A per-day advisory lock serialises it, so two
/// confirmations for the same hours cannot both pass the overlap check. Open requests it overlaps
/// are marked `taken_at` and their accounts are emailed.
pub async fn confirm(app: &Arc<App>, id: i64, paid: bool) -> Result<Confirmed, String> {
    let db = app.db.as_ref().ok_or("no database")?;
    let mut client = db.client().await?;
    let tx = client.transaction().await.map_err(|e| e.to_string())?;
    let Some(row) = tx
        .query_opt("SELECT day::text, start_hour::int, hours::int FROM reservations WHERE id = $1::bigint", &[&id])
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(Confirmed::Missing);
    };
    let (day, start, hours): (String, i32, i32) = (row.get(0), row.get(1), row.get(2));
    tx.execute("SELECT pg_advisory_xact_lock(hashtext('reservations:' || $1::text))", &[&day]).await.map_err(|e| e.to_string())?;
    let taken: i64 = tx
        .query_one(
            "SELECT count(*) FROM reservations WHERE day = $1::text::date AND status = 'confirmed'
             AND start_hour < $3::int AND start_hour + hours > $2::int AND id <> $4::bigint",
            &[&day, &start, &(start + hours), &id],
        )
        .await
        .map_err(|e| e.to_string())?
        .get(0);
    let mut bumped: Vec<i64> = Vec::new();
    let result = if taken > 0 {
        if paid {
            tx.execute("UPDATE reservations SET paid_at = coalesce(paid_at, now()) WHERE id = $1::bigint", &[&id]).await.map_err(|e| e.to_string())?;
        }
        Confirmed::Taken
    } else {
        tx.execute(
            "UPDATE reservations SET status = 'confirmed', decided_at = now(),
             paid_at = CASE WHEN $2::bool THEN coalesce(paid_at, now()) ELSE paid_at END WHERE id = $1::bigint",
            &[&id, &paid],
        )
        .await
        .map_err(|e| e.to_string())?;
        bumped = tx
            .query(
                "UPDATE reservations SET taken_at = now() WHERE day = $1::text::date AND status IN ('requested', 'awaiting')
                 AND taken_at IS NULL AND start_hour < $3::int AND start_hour + hours > $2::int AND id <> $4::bigint RETURNING id",
                &[&day, &start, &(start + hours), &id],
            )
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .map(|r| r.get(0))
            .collect();
        Confirmed::Done(bumped.len())
    };
    tx.commit().await.map_err(|e| e.to_string())?;
    if !bumped.is_empty() {
        let app = app.clone();
        tokio::spawn(async move {
            if let Err(err) = tell_taken(&app, &bumped).await {
                eprintln!("[mail] taken notice for {bumped:?} failed: {err}");
            }
        });
    }
    Ok(result)
}

/// One email per account whose open requests `ids` were just taken by a confirmed booking.
async fn tell_taken(app: &App, ids: &[i64]) -> Result<(), String> {
    let db = app.db.as_ref().ok_or("no database")?;
    let rows = db
        .texts(
            "SELECT json_build_object('email', u.email, 'name', u.name, 'day', r.day::text, 'hour', r.start_hour, 'hours', r.hours)::text
               FROM reservations r JOIN users u ON u.id = r.user_id WHERE r.id = ANY($1::bigint[]) ORDER BY u.email, r.day, r.start_hour",
            &[&ids],
        )
        .await?;
    let mut by_user: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for row in rows {
        let v: Value = serde_json::from_str(&row).map_err(|e| e.to_string())?;
        let (Some(email), Some(day)) = (v["email"].as_str(), v["day"].as_str().and_then(Day::parse)) else { continue };
        let (hour, hours) = (v["hour"].as_u64().unwrap_or(0) as u32, v["hours"].as_u64().unwrap_or(0) as u32);
        let entry = by_user.entry(email.to_string()).or_insert_with(|| (v["name"].as_str().unwrap_or("").to_string(), Vec::new()));
        entry.1.push(format!("{}, {}–{}", day.long(), hour_label(hour), hour_label(hour + hours)));
    }
    for (email, (name, slots)) in by_user {
        let first = name.split_whitespace().next().unwrap_or("there");
        account::send_taken(app, &email, first, &slots).await?;
    }
    Ok(())
}

/// Clears `taken_at` on the day of `id` for requests no confirmed booking overlaps any more.
pub async fn reopen_taken(db: &Db, id: i64) -> Result<(), String> {
    db.execute(
        "UPDATE reservations r SET taken_at = NULL WHERE r.taken_at IS NOT NULL AND r.day = (SELECT day FROM reservations WHERE id = $1::bigint)
         AND NOT EXISTS (SELECT 1 FROM reservations c WHERE c.day = r.day AND c.status = 'confirmed'
                         AND c.start_hour < r.start_hour + r.hours AND c.start_hour + c.hours > r.start_hour)",
        &[&id],
    )
    .await?;
    Ok(())
}

/// Confirmed hours per day from `from` for `days` days.
pub async fn booked_hours(db: &Db, from: Day, days: i64) -> Result<BTreeMap<Day, [bool; 24]>, String> {
    let rows = db
        .texts(
            "SELECT day::text || ' ' || start_hour || ' ' || hours FROM reservations
             WHERE status = 'confirmed' AND day >= $1::text::date AND day < $1::text::date + $2::int",
            &[&from.iso(), &(days as i32)],
        )
        .await?;
    let mut map = BTreeMap::new();
    for row in rows {
        let mut parts = row.split(' ');
        let (Some(day), Some(start), Some(hours)) = (parts.next().and_then(Day::parse), parts.next().and_then(|s| s.parse::<usize>().ok()), parts.next().and_then(|s| s.parse::<usize>().ok())) else { continue };
        let hours_of_day: &mut [bool; 24] = map.entry(day).or_insert([false; 24]);
        for h in start..(start + hours).min(24) {
            hours_of_day[h] = true;
        }
    }
    Ok(map)
}

async fn availability(State(app): State<Arc<App>>, Query(q): Query<BTreeMap<String, String>>) -> Res {
    let db = app.db()?;
    let (today, hour) = manila_now();
    let from = q.get("from").and_then(|s| Day::parse(s)).unwrap_or(today);
    let days = q.get("days").and_then(|s| s.parse::<i64>().ok()).unwrap_or(7).clamp(1, 62);
    let booked = booked_hours(db, from, days).await?;
    let booked: serde_json::Map<String, Value> = booked
        .into_iter()
        .map(|(d, hours)| (d.iso(), json!((0..24).filter(|h| hours[*h]).collect::<Vec<_>>())))
        .collect();
    Ok(web::json(
        200,
        json!({ "open": OPEN, "close": CLOSE, "maxHours": MAX_HOURS, "today": today.iso(), "hour": hour, "online": pay::enabled(), "pricePerHour": pay::price_per_hour(), "booked": booked }),
    ))
}

async fn request(State(app): State<Arc<App>>, Extension(ctx): Extension<Ctx>, headers: HeaderMap, body: Bytes) -> Res {
    if !app.request_buckets.allows(&ctx.ip) {
        return Ok(web::json(429, json!({ "success": false, "error": "Too many requests from here. Try again later." })));
    }
    let db = app.db()?;
    let Some(user) = account::current(&app, &headers).await? else {
        return Ok(web::json(401, json!({ "success": false, "signIn": true, "error": "Sign in to book." })));
    };
    if user.banned {
        return Ok(web::json(403, json!({ "success": false, "banned": true, "error": "Your account is blocked from booking. Message us if you think this is a mistake." })));
    }
    let Ok(mut v) = serde_json::from_slice::<Value>(&body) else { return Ok(bad("Send the form again.")) };
    v["name"] = json!(user.name);
    v["phone"] = json!(user.phone);
    v["facebook"] = json!(user.facebook);
    let blocks = match picked_blocks(&v) {
        Ok(b) => b,
        Err(msg) => return Ok(bad(msg)),
    };
    let mut inputs = Vec::new();
    for (date, hour, hours) in blocks {
        v["date"] = json!(date);
        v["hour"] = json!(hour);
        v["hours"] = json!(hours);
        match parse_input(&v, false) {
            Ok(i) => inputs.push(i),
            Err(msg) => return Ok(bad(msg)),
        }
    }
    for i in &inputs {
        if overlaps_confirmed(db, i.day, i.hour, i.hours, 0).await? {
            return Ok(web::json(409, json!({ "success": false, "error": "Someone already booked some of those hours. Pick another time." })));
        }
    }
    let online = field(&v, "pay") == "online" && pay::enabled();
    let price = pay::price_per_hour().unwrap_or(0) as i32 * 100;
    let mut ids = Vec::new();
    for input in &inputs {
        ids.push(
            insert(
                db,
                New {
                    input,
                    status: if online { "awaiting" } else { "requested" },
                    pay_method: if online { "online" } else { "arrange" },
                    pay_window_mins: if online { ONLINE_PAY_WINDOW_MINS } else { 0 },
                    paid: false,
                    amount: online.then(|| price * input.hours as i32),
                    ip: &ctx.ip,
                    user_id: Some(user.id),
                },
            )
            .await?,
        );
    }
    let id = ids[0];
    db.execute("UPDATE reservations SET bundle = $1::bigint WHERE id = ANY($2::bigint[])", &[&id, &ids]).await?;
    if !online {
        return Ok(web::json(200, json!({ "success": true, "id": id, "ids": ids })));
    }
    match pay::checkout(&app.http, id, &inputs, price).await {
        Ok((session, url)) => {
            db.execute("UPDATE reservations SET checkout_id = $2 WHERE bundle = $1::bigint", &[&id, &session]).await?;
            Ok(web::json(200, json!({ "success": true, "id": id, "ids": ids, "checkoutUrl": url })))
        }
        Err(err) => {
            eprintln!("[pay] checkout for #{id} failed: {err}");
            db.execute("UPDATE reservations SET status = 'requested', pay_method = 'arrange', pay_by = NULL, amount = NULL WHERE bundle = $1::bigint", &[&id]).await?;
            Ok(web::json(200, json!({ "success": true, "id": id, "ids": ids, "payFailed": true })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(extra: Value) -> Value {
        let (today, _) = manila_now();
        let mut v = json!({ "name": "Ana Cruz", "phone": "0917 000 0000", "date": today.add(1).iso(), "hour": 9, "hours": 2, "players": 4 });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        v
    }

    #[test]
    fn accepts_a_good_request() {
        let i = parse_input(&form(json!({})), false).unwrap();
        assert_eq!((i.hour, i.hours, i.players), (9, 2, 4));
    }

    #[test]
    fn groups_picked_hours_into_blocks() {
        let s = |d: &str, h: u32| json!({ "date": d, "hour": h });
        let b = picked_blocks(&json!({ "slots": [s("2026-12-01", 10), s("2026-12-01", 9), s("2026-12-01", 13), s("2026-12-02", 14), s("2026-12-01", 9)] })).unwrap();
        assert_eq!(b, vec![("2026-12-01".into(), 9, 2), ("2026-12-01".into(), 13, 1), ("2026-12-02".into(), 14, 1)]);
        let run: Vec<Value> = (6..11).map(|h| s("2026-12-01", h)).collect();
        assert_eq!(picked_blocks(&json!({ "slots": run })).unwrap(), vec![("2026-12-01".into(), 6, 4), ("2026-12-01".into(), 10, 1)]);
        assert!(picked_blocks(&json!({ "slots": [] })).is_err());
        let many: Vec<Value> = (6..15).map(|h| s("2026-12-01", h)).collect();
        assert!(picked_blocks(&json!({ "slots": many })).is_err());
    }

    #[test]
    fn rejects_bad_requests() {
        assert!(parse_input(&form(json!({ "phone": "", "facebook": "" })), false).is_err());
        assert!(parse_input(&form(json!({ "hour": 22, "hours": 2 })), false).is_err());
        assert!(parse_input(&form(json!({ "hours": 5 })), false).is_err());
        assert!(parse_input(&form(json!({ "phone": "call me" })), false).is_err());
        assert!(parse_input(&form(json!({ "date": manila_now().0.add(-1).iso() })), false).is_err());
        assert!(parse_input(&form(json!({ "date": manila_now().0.add(-1).iso() })), true).is_ok());
    }
}
