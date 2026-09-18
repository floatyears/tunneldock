use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, TimeZone, Utc};

const LEGACY_TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn local_now_rfc3339() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

pub fn normalize_timestamp_to_local(value: &str) -> String {
    normalize_timestamp_in_timezone(value, &Local)
}

fn normalize_timestamp_in_timezone<Tz>(value: &str, timezone: &Tz) -> String
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let timestamp = if let Ok(timestamp) = DateTime::parse_from_rfc3339(value) {
        timestamp.with_timezone(timezone)
    } else if let Ok(timestamp) = NaiveDateTime::parse_from_str(value, LEGACY_TIMESTAMP_FORMAT) {
        Utc.from_utc_datetime(&timestamp).with_timezone(timezone)
    } else {
        return value.to_string();
    };

    timestamp.to_rfc3339_opts(SecondsFormat::Secs, false)
}

#[cfg(test)]
mod tests {
    use super::normalize_timestamp_in_timezone;
    use chrono::FixedOffset;

    #[test]
    fn converts_legacy_utc_timestamp_to_requested_local_offset() {
        let timezone = FixedOffset::east_opt(8 * 60 * 60).expect("valid UTC+8 offset");

        assert_eq!(
            normalize_timestamp_in_timezone("2026-09-18 03:04:04", &timezone),
            "2026-09-18T11:04:04+08:00"
        );
    }

    #[test]
    fn normalizes_offset_timestamp_to_requested_local_offset() {
        let timezone = FixedOffset::east_opt(8 * 60 * 60).expect("valid UTC+8 offset");

        assert_eq!(
            normalize_timestamp_in_timezone("2026-09-18T03:04:04Z", &timezone),
            "2026-09-18T11:04:04+08:00"
        );
    }
}
