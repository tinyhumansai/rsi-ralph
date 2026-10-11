//! RFC 3339 timestamps for ingestion events, without a date-time dependency.

/// Formats Unix milliseconds as `YYYY-MM-DDTHH:MM:SS.mmmZ` in UTC.
pub(super) fn iso8601(unix_ms: u64) -> String {
    let millis = unix_ms % 1000;
    let secs = unix_ms / 1000;
    let (days, day_secs) = (secs / 86_400, secs % 86_400);
    let (hour, minute, second) = (day_secs / 3600, day_secs % 3600 / 60, day_secs % 60);
    // Civil-from-days (Howard Hinnant), shifted so the era starts 0000-03-01.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Milliseconds since the Unix epoch, or zero for a clock set before it.
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
