use std::{
    collections::BTreeMap,
    fmt::Write,
    sync::{Arc, OnceLock},
};

use base64::Engine;
use resvg::{tiny_skia, usvg};

use crate::{
    book::{CLOSE, OPEN},
    date::{Day, MONTHS, WEEKDAYS, hour_label},
    pages::{escape, site_url},
};

const W: u32 = 1080;
const H: u32 = 1350;
const INK: &str = "#0f172a";
const MUTED: &str = "#64748b";
const MIST: &str = "#eef2f7";
const LINE: &str = "#e2e8f0";
const DINK: &str = "#0b3fae";
const SKIBI: &str = "#00a04b";
const PAST: &str = "#cbd5e1";
// DANGER: the static Outfit files name their family "Outfit Thin" (they are cut from the variable
// font); "Outfit" matches nothing and resvg silently drops the text.
const DISPLAY: &str = "Outfit Thin";

pub type Booked = BTreeMap<Day, [bool; 24]>;

pub struct Now {
    pub today: Day,
    pub hour: u32,
}

fn fonts() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        for data in [
            &include_bytes!("../fonts/outfit-700.ttf")[..],
            &include_bytes!("../fonts/outfit-800.ttf")[..],
            &include_bytes!("../fonts/plus-jakarta-sans-500.ttf")[..],
            &include_bytes!("../fonts/plus-jakarta-sans-700.ttf")[..],
        ] {
            db.load_font_data(data.to_vec());
        }
        Arc::new(db)
    })
    .clone()
}

pub fn render_png(svg: &str) -> Result<Vec<u8>, String> {
    let opt = usvg::Options { fontdb: fonts(), font_family: "Plus Jakarta Sans".into(), ..Default::default() };
    let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(W, H).ok_or("pixmap")?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| e.to_string())
}

fn site_host() -> String {
    let url = site_url();
    url.split_once("://").map(|(_, rest)| rest.to_string()).unwrap_or(url)
}

fn hours_of(booked: &Booked, day: Day) -> [bool; 24] {
    booked.get(&day).copied().unwrap_or([false; 24])
}

fn is_past(now: &Now, day: Day, hour: u32) -> bool {
    day < now.today || (day == now.today && hour <= now.hour)
}

fn open_count(now: &Now, booked: &Booked, day: Day) -> u32 {
    let hours = hours_of(booked, day);
    (OPEN..CLOSE).filter(|h| !hours[*h as usize] && !is_past(now, day, *h)).count() as u32
}

fn frame(logo: &[u8], eyebrow: &str, title: &str, subtitle: &str, body: &str, footer_note: &str) -> String {
    let logo = base64::engine::general_purpose::STANDARD.encode(logo);
    let mut s = String::new();
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
<rect width="{W}" height="{H}" fill="{MIST}"/>
<rect x="0" y="0" width="{W}" height="14" fill="{SKIBI}"/>
<image x="60" y="62" width="370" height="70" xlink:href="data:image/png;base64,{logo}"/>
<text x="1020" y="108" text-anchor="end" font-family="Plus Jakarta Sans" font-weight="700" font-size="24" letter-spacing="3" fill="{SKIBI}">{eyebrow}</text>
<text x="60" y="232" font-family="{DISPLAY}" font-weight="800" font-size="78" fill="{INK}">{title}</text>
<text x="60" y="288" font-family="Plus Jakarta Sans" font-weight="500" font-size="32" fill="{MUTED}">{subtitle}</text>
{body}
<line x1="60" y1="1262" x2="1020" y2="1262" stroke="{LINE}" stroke-width="2"/>
<text x="60" y="1308" font-family="Plus Jakarta Sans" font-weight="700" font-size="26" fill="{DINK}">{host}</text>
<text x="1020" y="1308" text-anchor="end" font-family="Plus Jakarta Sans" font-weight="500" font-size="24" fill="{MUTED}">{footer_note}</text>
</svg>"##,
        eyebrow = escape(eyebrow),
        title = escape(title),
        subtitle = escape(subtitle),
        host = escape(&site_host()),
        footer_note = escape(footer_note),
    );
    s
}

fn legend(y: u32, items: &[(&str, &str, &str)]) -> String {
    let mut s = String::new();
    let mut x = 60;
    for (fill, stroke, label) in items {
        let _ = write!(
            s,
            r#"<rect x="{x}" y="{y}" width="28" height="28" rx="7" fill="{fill}" stroke="{stroke}" stroke-width="2"/><text x="{tx}" y="{ty}" font-family="Plus Jakarta Sans" font-weight="500" font-size="24" fill="{MUTED}">{label}</text>"#,
            tx = x + 40,
            ty = y + 22,
        );
        x += 84 + label.len() as u32 * 13;
    }
    s
}

pub fn day(logo: &[u8], now: &Now, booked: &Booked, day: Day) -> String {
    let hours = hours_of(booked, day);
    let total = CLOSE - OPEN;
    let rows = total.div_ceil(2);
    let (top, bottom) = (350u32, 1200u32);
    let row_h = (bottom - top) / rows;
    let mut body = String::new();
    for (i, h) in (OPEN..CLOSE).enumerate() {
        let (col, row) = (i as u32 / rows, i as u32 % rows);
        let x = 60 + col * 490;
        let y = top + row * row_h;
        let past = is_past(now, day, h);
        let busy = hours[h as usize];
        let (fill, stroke, text, label, label_fill) = match (busy, past) {
            (true, _) => (DINK, DINK, "#ffffff", "BOOKED", "#ffffff"),
            (false, true) => ("#f1f5f9", LINE, PAST, "PAST", PAST),
            (false, false) => ("#ffffff", LINE, INK, "OPEN", SKIBI),
        };
        let _ = write!(
            body,
            r#"<rect x="{x}" y="{y}" width="470" height="{bh}" rx="18" fill="{fill}" stroke="{stroke}" stroke-width="2"/>
<text x="{tx}" y="{ty}" font-family="{DISPLAY}" font-weight="700" font-size="34" fill="{text}">{from} – {to}</text>
<text x="{lx}" y="{ty}" text-anchor="end" font-family="Plus Jakarta Sans" font-weight="700" font-size="22" letter-spacing="2" fill="{label_fill}">{label}</text>"#,
            bh = row_h - 14,
            tx = x + 28,
            ty = y + (row_h - 14) / 2 + 12,
            lx = x + 446,
            from = hour_label(h),
            to = hour_label(h + 1),
        );
    }
    let open = open_count(now, booked, day);
    let subtitle = match open {
        0 => format!("{} · Fully booked", day.long()),
        1 => format!("{} · 1 hour open", day.long()),
        n => format!("{} · {n} hours open", day.long()),
    };
    frame(logo, "COURT SCHEDULE", if day == now.today { "Today on court" } else { "On court" }, &subtitle, &body, "Message us to book an open hour")
}

pub fn week(logo: &[u8], now: &Now, booked: &Booked, start: Day) -> String {
    let (left, top, right, bottom) = (150u32, 420u32, 1020u32, 1190u32);
    let col_w = (right - left) / 7;
    let row_h = (bottom - top) / (CLOSE - OPEN);
    let mut body = String::new();
    for c in 0..7u32 {
        let d = start.add(c as i64);
        let x = left + c * col_w;
        let open = open_count(now, booked, d);
        let full = open == 0 && d >= now.today;
        let _ = write!(
            body,
            r#"<text x="{cx}" y="340" text-anchor="middle" font-family="Plus Jakarta Sans" font-weight="700" font-size="22" letter-spacing="2" fill="{MUTED}">{wd}</text>
<text x="{cx}" y="382" text-anchor="middle" font-family="{DISPLAY}" font-weight="800" font-size="36" fill="{INK}">{dn}</text>"#,
            cx = x + col_w / 2,
            wd = &WEEKDAYS[d.weekday()][..3].to_uppercase(),
            dn = d.ymd().2,
        );
        let hours = hours_of(booked, d);
        for (r, h) in (OPEN..CLOSE).enumerate() {
            let y = top + r as u32 * row_h;
            let fill = if hours[h as usize] { DINK } else if is_past(now, d, h) { "#f1f5f9" } else { "#ffffff" };
            let _ = write!(body, r#"<rect x="{rx}" y="{ry}" width="{w}" height="{hh}" rx="6" fill="{fill}" stroke="{LINE}" stroke-width="1.5"/>"#, rx = x + 4, ry = y + 2, w = col_w - 8, hh = row_h - 4);
        }
        if full {
            let _ = write!(
                body,
                r##"<rect x="{rx}" y="{ry}" width="{w}" height="44" rx="22" fill="#ffffff"/><text x="{cx}" y="{ty}" text-anchor="middle" font-family="Plus Jakarta Sans" font-weight="700" font-size="20" letter-spacing="2" fill="{DINK}">FULL</text>"##,
                rx = x + 14,
                ry = top + (bottom - top) / 2 - 22,
                w = col_w - 28,
                cx = x + col_w / 2,
                ty = top + (bottom - top) / 2 + 7,
            );
        }
    }
    for (r, h) in (OPEN..CLOSE).enumerate() {
        if r % 2 == 0 {
            let _ = write!(
                body,
                r#"<text x="{x}" y="{y}" text-anchor="end" font-family="Plus Jakarta Sans" font-weight="500" font-size="20" fill="{MUTED}">{l}</text>"#,
                x = left - 14,
                y = top + r as u32 * row_h + row_h / 2 + 7,
                l = hour_label(h),
            );
        }
    }
    body.push_str(&legend(1212, &[(DINK, DINK, "Booked"), ("#ffffff", LINE, "Open"), ("#f1f5f9", LINE, "Past")]));
    let end = start.add(6);
    let full_days = (0..7).map(|i| start.add(i)).filter(|d| *d >= now.today && open_count(now, booked, *d) == 0).count();
    let subtitle = format!(
        "{} – {}{}",
        start.short(),
        end.short(),
        match full_days {
            0 => String::new(),
            1 => " · 1 day fully booked".into(),
            n => format!(" · {n} days fully booked"),
        }
    );
    let title = if start <= now.today && now.today <= end { "This week".to_string() } else { format!("Week of {}", start.short()) };
    frame(logo, "WEEKLY SCHEDULE", &title, &subtitle, &body, "Message us to book an open hour")
}

pub fn month(logo: &[u8], now: &Now, booked: &Booked, first: Day) -> String {
    let (y, m, _) = first.ymd();
    let len = crate::date::month_len(y, m);
    let lead = first.weekday() as u32;
    let weeks = (lead + len).div_ceil(7);
    let (left, top, right) = (60u32, 370u32, 1020u32);
    let col_w = (right - left) / 7;
    let cell_h = ((1190 - top) / weeks).min(150);
    let mut body = String::new();
    for (i, wd) in WEEKDAYS.iter().enumerate() {
        let _ = write!(
            body,
            r#"<text x="{x}" y="342" text-anchor="middle" font-family="Plus Jakarta Sans" font-weight="700" font-size="22" letter-spacing="2" fill="{MUTED}">{w}</text>"#,
            x = left + i as u32 * col_w + col_w / 2,
            w = wd[..3].to_uppercase(),
        );
    }
    let mut full_days = 0;
    for n in 0..len {
        let d = first.add(n as i64);
        let slot = lead + n;
        let (x, yy) = (left + (slot % 7) * col_w + 5, top + (slot / 7) * cell_h + 5);
        let (w, h) = (col_w - 10, cell_h - 10);
        let past = d < now.today;
        let open = open_count(now, booked, d);
        let full = !past && open == 0;
        if full {
            full_days += 1;
        }
        let (fill, stroke, num, tag, tag_fill) = if past {
            ("#f1f5f9", LINE, PAST, String::new(), PAST)
        } else if full {
            (DINK, DINK, "#ffffff", "FULL".to_string(), "#ffffff")
        } else if hours_of(booked, d).iter().any(|b| *b) {
            ("#ffffff", LINE, INK, format!("{open} open"), SKIBI)
        } else {
            ("#ffffff", LINE, INK, "Open".to_string(), SKIBI)
        };
        let _ = write!(
            body,
            r#"<rect x="{x}" y="{yy}" width="{w}" height="{h}" rx="16" fill="{fill}" stroke="{stroke}" stroke-width="2"/>
<text x="{nx}" y="{ny}" font-family="{DISPLAY}" font-weight="800" font-size="34" fill="{num}">{dn}</text>
<text x="{nx}" y="{ty}" font-family="Plus Jakarta Sans" font-weight="700" font-size="19" fill="{tag_fill}">{tag}</text>"#,
            nx = x + 16,
            ny = yy + 44,
            ty = yy + h - 18,
            dn = n + 1,
        );
    }
    body.push_str(&legend(1212, &[(DINK, DINK, "Fully booked"), ("#ffffff", LINE, "Hours open"), ("#f1f5f9", LINE, "Past")]));
    let subtitle = match full_days {
        0 => "Every day still has open hours".to_string(),
        1 => "1 day fully booked".to_string(),
        n => format!("{n} days fully booked"),
    };
    frame(logo, "MONTHLY SCHEDULE", &format!("{} {y}", MONTHS[m as usize - 1]), &subtitle, &body, "Message us to book an open day")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_every_poster() {
        let now = Now { today: Day::parse("2026-11-20").unwrap(), hour: 10 };
        let mut booked = Booked::new();
        booked.insert(Day::parse("2026-11-21").unwrap(), [true; 24]);
        let mut some = [false; 24];
        some[18] = true;
        booked.insert(Day::parse("2026-11-20").unwrap(), some);
        let logo = std::fs::read("../public/img/logo.png").unwrap_or_default();
        let out = std::env::var("POSTER_OUT").ok();
        for (name, svg) in [("day", day(&logo, &now, &booked, now.today)), ("week", week(&logo, &now, &booked, now.today)), ("month", month(&logo, &now, &booked, crate::date::parse_month("2026-11").unwrap()))] {
            let png = render_png(&svg).unwrap();
            assert_eq!(&png[1..4], b"PNG");
            if let Some(dir) = &out {
                std::fs::write(format!("{dir}/{name}.png"), png).unwrap();
            }
        }
    }
}
