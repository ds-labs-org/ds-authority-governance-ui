//! Display helpers shared by the views: human-readable UTC timestamps,
//! `<input type="date">` values, durations and comma-separated lists.
//! Pure functions, unit-tested on the host.

/// `1792017886` -> `2026-10-14 22:44:46 UTC`.
pub fn timestamp(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// [`timestamp`] for an optional value, `fallback` when absent.
pub fn timestamp_or(secs: Option<u64>, fallback: &str) -> String {
    secs.map_or_else(|| fallback.to_string(), timestamp)
}

/// Unix seconds -> `YYYY-MM-DD` (UTC), the value format of
/// `<input type="date">`.
pub fn date_input(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `YYYY-MM-DD` -> unix seconds at 00:00 UTC that day. Empty input is
/// `Ok(None)` (the field is optional).
pub fn parse_date_input(value: &str) -> Result<Option<u64>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let invalid = || format!("'{value}' is not a valid date (YYYY-MM-DD).");
    let parts: Vec<&str> = value.split('-').collect();
    let [y, m, d] = parts.as_slice() else {
        return Err(invalid());
    };
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return Err(invalid());
    }
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i64>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return Err(invalid());
    };
    if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return Err(invalid());
    }
    let days = days_from_civil(y, m, d);
    if days < 0 {
        return Err(invalid());
    }
    Ok(Some(days as u64 * 86_400))
}

/// `3600` -> `1 hour`, `5400` -> `90 minutes`, `172800` -> `2 days`.
pub fn duration(secs: u64) -> String {
    let (n, unit) = match secs {
        s if s > 0 && s % 86_400 == 0 => (s / 86_400, "day"),
        s if s > 0 && s % 3600 == 0 => (s / 3600, "hour"),
        s if s > 0 && s % 60 == 0 => (s / 60, "minute"),
        s => (s, "second"),
    };
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

/// `"A, B,,A "` -> `["A", "B"]`: trimmed, empties and duplicates dropped,
/// order kept.
pub fn split_list(value: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if !out.iter().any(|existing| existing == item) {
            out.push(item.to_string());
        }
    }
    out
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Howard Hinnant's `days_from_civil`: days since 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_render_in_utc() {
        assert_eq!(timestamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(timestamp(1792017886), "2026-10-14 22:44:46 UTC");
        assert_eq!(timestamp(951782400), "2000-02-29 00:00:00 UTC");
        assert_eq!(timestamp(4102444799), "2099-12-31 23:59:59 UTC");
    }

    #[test]
    fn timestamp_or_uses_fallback() {
        assert_eq!(timestamp_or(None, "Never"), "Never");
        assert_eq!(timestamp_or(Some(0), "Never"), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn date_input_round_trips() {
        assert_eq!(date_input(1792017886), "2026-10-14");
        assert_eq!(parse_date_input("2026-10-14"), Ok(Some(1791936000)));
        assert_eq!(parse_date_input("2000-02-29"), Ok(Some(951782400)));
        assert_eq!(parse_date_input(" "), Ok(None));
        assert_eq!(parse_date_input(""), Ok(None));
    }

    #[test]
    fn parse_date_input_rejects_garbage() {
        for bad in [
            "2026-13-01",
            "2026-02-30",
            "2001-02-29",
            "26-1-1",
            "tomorrow",
            "1969-12-31",
        ] {
            assert!(parse_date_input(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn durations_use_largest_exact_unit() {
        assert_eq!(duration(1), "1 second");
        assert_eq!(duration(45), "45 seconds");
        assert_eq!(duration(60), "1 minute");
        assert_eq!(duration(3600), "1 hour");
        assert_eq!(duration(5400), "90 minutes");
        assert_eq!(duration(7200), "2 hours");
        assert_eq!(duration(86400), "1 day");
        assert_eq!(duration(31536000), "365 days");
        assert_eq!(duration(0), "0 seconds");
    }

    #[test]
    fn split_list_trims_and_dedupes() {
        assert_eq!(split_list("A, B,,A , C"), vec!["A", "B", "C"]);
        assert_eq!(split_list(" * "), vec!["*"]);
        assert!(split_list(" , ").is_empty());
    }
}
