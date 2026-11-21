use std::sync::Arc;

use axum::{Router, body::Bytes, extract::State, http::HeaderMap, routing::post};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::{
    App,
    book::{self, Confirmed, Input},
    date::hour_label,
    pages::site_url,
    web::{self, Res},
};

const API: &str = "https://api.paymongo.com/v2/checkout_sessions";

pub fn routes() -> Router<Arc<App>> {
    Router::new().route("/api/pay/paymongo", post(webhook))
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Whole pesos per court hour. Unset means there is no price, so online payment stays off.
pub fn price_per_hour() -> Option<u32> {
    env("PRICE_PER_HOUR").and_then(|v| v.parse().ok()).filter(|p| *p > 0)
}

pub fn enabled() -> bool {
    price_per_hour().is_some() && env("PAYMONGO_SECRET_KEY").is_some() && env("PAYMONGO_WEBHOOK_SECRET").is_some()
}

/// Opens one PayMongo hosted checkout for the bundle `id`, a line item per block. `hourly` is in centavos.
pub async fn checkout(http: &reqwest::Client, id: i64, inputs: &[Input], hourly: i32) -> Result<(String, String), String> {
    let key = env("PAYMONGO_SECRET_KEY").ok_or("PAYMONGO_SECRET_KEY is not set")?;
    let methods: Vec<String> = env("PAYMONGO_METHODS").unwrap_or_else(|| "qrph".into()).split(',').map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).collect();
    let site = site_url();
    let items: Vec<Value> = inputs
        .iter()
        .map(|i| {
            let label = format!("Court, {} {}–{}", i.day.long(), hour_label(i.hour), hour_label(i.hour + i.hours));
            json!({ "name": label, "amount": hourly * i.hours as i32, "currency": "PHP", "quantity": 1 })
        })
        .collect();
    let body = json!({ "data": { "attributes": {
        "line_items": items,
        "payment_method_types": methods,
        "description": format!("SkibiDink reservation #{id}"),
        "reference_number": reference(id),
        "success_url": format!("{site}/book?paid={id}"),
        "cancel_url": format!("{site}/book?unpaid={id}"),
    } } });
    let res = http.post(API).basic_auth(key, Some("")).json(&body).send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    let v: Value = res.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("PayMongo {status}: {v}"));
    }
    let session = v["data"]["id"].as_str().ok_or("no session id")?.to_string();
    let url = v["data"]["attributes"]["checkout_url"].as_str().ok_or("no checkout_url")?.to_string();
    Ok((session, url))
}

fn reference(id: i64) -> String {
    format!("SD-{id}")
}

/// `t=<unix>,te=<test sig>,li=<live sig>`; the signature is HMAC-SHA256 of `"{t}.{raw body}"`.
fn signature_ok(header: &str, body: &[u8], secret: &str) -> bool {
    let mut t = "";
    let mut sigs = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", v)) => t = v,
            Some(("te" | "li", v)) if !v.is_empty() => sigs.push(v.to_ascii_lowercase()),
            _ => {}
        }
    }
    if t.is_empty() || sigs.is_empty() {
        return false;
    }
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(t.as_bytes());
    mac.update(b".");
    mac.update(body);
    let want = hex::encode(mac.finalize().into_bytes());
    sigs.iter().any(|s| s.len() == want.len() && s.bytes().zip(want.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0)
}

// SECURITY: this is the only route that marks a reservation paid, so nothing reaches the
// database until the PayMongo signature over the raw body checks out.
async fn webhook(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Res {
    let Some(secret) = env("PAYMONGO_WEBHOOK_SECRET") else { return Ok(web::json(404, json!({ "ok": false }))) };
    let sig = headers.get("paymongo-signature").and_then(|v| v.to_str().ok()).unwrap_or("");
    if !signature_ok(sig, &body, &secret) {
        return Ok(web::json(401, json!({ "ok": false })));
    }
    let Ok(v) = serde_json::from_slice::<Value>(&body) else { return Ok(web::json(400, json!({ "ok": false }))) };
    let event = &v["data"]["attributes"];
    if event["type"].as_str() != Some("checkout_session.payment.paid") {
        return Ok(web::json(200, json!({ "ok": true })));
    }
    let session = event["data"]["id"].as_str().unwrap_or("");
    let reference = event["data"]["attributes"]["reference_number"].as_str().unwrap_or("");
    let db = app.db()?;
    let ids: Vec<i64> = db
        .texts(
            "SELECT id::text FROM reservations WHERE checkout_id = $1
             UNION SELECT id::text FROM reservations WHERE NOT EXISTS (SELECT 1 FROM reservations WHERE checkout_id = $1)
               AND ('SD-' || coalesce(bundle, id)) = $2",
            &[&session, &reference],
        )
        .await?
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();
    if ids.is_empty() {
        eprintln!("[pay] paid session {session} ({reference}) matches no reservation");
    }
    for id in ids {
        match book::confirm(&app, id, true).await? {
            Confirmed::Done(_) => println!("[pay] #{id} paid and confirmed"),
            Confirmed::Taken => eprintln!("[pay] #{id} paid, but its hours were already confirmed for someone else: refund or move it"),
            Confirmed::Missing => {}
        }
    }
    Ok(web::json(200, json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_the_paymongo_signature() {
        let body = br#"{"data":{}}"#;
        let mut mac = Hmac::<Sha256>::new_from_slice(b"whsk_test").unwrap();
        mac.update(b"1700000000.");
        mac.update(body);
        let sig = hex::encode(mac.finalize().into_bytes());
        assert!(signature_ok(&format!("t=1700000000,te={sig},li="), body, "whsk_test"));
        assert!(signature_ok(&format!("t=1700000000,te=,li={sig}"), body, "whsk_test"));
        assert!(!signature_ok(&format!("t=1700000001,te={sig},li="), body, "whsk_test"));
        assert!(!signature_ok(&format!("t=1700000000,te={sig},li="), body, "other"));
        assert!(!signature_ok("t=1700000000,te=,li=", body, "whsk_test"));
    }
}
