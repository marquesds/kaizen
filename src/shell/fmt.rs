// SPDX-License-Identifier: AGPL-3.0-or-later
//! Timestamp formatting — no chrono dep, pure stdlib.

/// Ms since epoch → `YYYY-MM-DD HH:MM`. Returns `(unknown)` when ms is 0.
pub fn fmt_ts(ms: u64) -> String {
    if ms == 0 {
        return "(unknown)".to_string();
    }
    let secs = ms / 1000;
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let h = time_secs / 3600;
    let m = (time_secs % 3600) / 60;
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}

/// Gregorian date from days since 1970-01-01.
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_returns_unknown() {
        assert_eq!(fmt_ts(0), "(unknown)");
    }

    #[test]
    fn known_epoch() {
        // 2026-04-22 00:00 UTC = 1776816000000 ms
        assert_eq!(fmt_ts(1776816000000), "2026-04-22 00:00");
    }
}
