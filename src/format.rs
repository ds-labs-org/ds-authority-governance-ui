//! Display helpers shared by the views: human-readable UTC timestamps,
//! `<input type="date">` values, durations and comma-separated lists.
//! Pure functions, unit-tested on the host.

/// `1792017886` -> `2026-10-14 22:44:46 UTC`.
pub fn timestamp(secs: u64) -> String {
    let _ = secs;
    todo!()
}

/// [`timestamp`] for an optional value, `fallback` when absent.
pub fn timestamp_or(secs: Option<u64>, fallback: &str) -> String {
    let _ = (secs, fallback);
    todo!()
}

/// Unix seconds -> `YYYY-MM-DD` (UTC), the value format of
/// `<input type="date">`.
pub fn date_input(secs: u64) -> String {
    let _ = secs;
    todo!()
}

/// `YYYY-MM-DD` -> unix seconds at 00:00 UTC that day. Empty input is
/// `Ok(None)` (the field is optional).
pub fn parse_date_input(value: &str) -> Result<Option<u64>, String> {
    let _ = value;
    todo!()
}

/// `3600` -> `1 hour`, `5400` -> `90 minutes`, `172800` -> `2 days`.
pub fn duration(secs: u64) -> String {
    let _ = secs;
    todo!()
}

/// `"A, B,,A "` -> `["A", "B"]`: trimmed, empties and duplicates dropped,
/// order kept.
pub fn split_list(value: &str) -> Vec<String> {
    let _ = value;
    todo!()
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
