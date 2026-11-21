use std::time::{SystemTime, UNIX_EPOCH};

const MANILA_OFFSET_SECS: i64 = 8 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day(i64);

pub const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

impl Day {
    pub fn from_ymd(y: i64, m: u32, d: u32) -> Option<Self> {
        if !(1..=12).contains(&m) || d == 0 || d > month_len(y, m) {
            return None;
        }
        let (y, m, d) = (if m <= 2 { y - 1 } else { y }, m as i64, d as i64);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Some(Day(era * 146_097 + doe - 719_468))
    }

    pub fn parse(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        Self::from_ymd(s[0..4].parse().ok()?, s[5..7].parse().ok()?, s[8..10].parse().ok()?)
    }

    pub fn ymd(self) -> (i64, u32, u32) {
        let z = self.0 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
    }

    pub fn add(self, n: i64) -> Self {
        Day(self.0 + n)
    }

    pub fn days_until(self, other: Day) -> i64 {
        other.0 - self.0
    }

    /// 0 = Monday.
    pub fn weekday(self) -> usize {
        (self.0 + 3).rem_euclid(7) as usize
    }

    pub fn iso(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }

    pub fn long(self) -> String {
        let (_, m, d) = self.ymd();
        format!("{}, {} {d}", WEEKDAYS[self.weekday()], MONTHS[m as usize - 1])
    }

    pub fn short(self) -> String {
        let (_, m, d) = self.ymd();
        format!("{} {d}", &MONTHS[m as usize - 1][..3])
    }
}

pub fn month_len(y: i64, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `YYYY-MM` → the first day of that month.
pub fn parse_month(s: &str) -> Option<Day> {
    let (y, m) = s.split_once('-')?;
    if y.len() != 4 || m.len() != 2 {
        return None;
    }
    Day::from_ymd(y.parse().ok()?, m.parse().ok()?, 1)
}

/// Today and the current hour in Manila, which has no daylight saving.
pub fn manila_now() -> (Day, u32) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + MANILA_OFFSET_SECS;
    (Day(secs.div_euclid(86_400)), (secs.rem_euclid(86_400) / 3600) as u32)
}

pub fn hour_label(h: u32) -> String {
    format!("{} {}", if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 || h == 24 { "AM" } else { "PM" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_knows_weekdays() {
        let d = Day::parse("2026-11-20").unwrap();
        assert_eq!(d.iso(), "2026-11-20");
        assert_eq!(WEEKDAYS[d.weekday()], "Friday");
        assert_eq!(Day::parse("2024-02-29").unwrap().add(1).iso(), "2024-03-01");
        assert!(Day::parse("2025-02-29").is_none());
        assert!(Day::parse("2025-1-01").is_none());
        assert_eq!(parse_month("2026-11").unwrap().iso(), "2026-11-01");
    }

    #[test]
    fn labels_hours() {
        assert_eq!(hour_label(6), "6 AM");
        assert_eq!(hour_label(12), "12 PM");
        assert_eq!(hour_label(23), "11 PM");
        assert_eq!(hour_label(24), "12 AM");
    }
}
