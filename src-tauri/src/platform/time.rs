use chrono::{DateTime, SecondsFormat, Utc};

pub fn utc_now_rfc3339() -> String {
    format_utc_rfc3339(Utc::now())
}

pub fn utc_now_rfc3339_millis() -> String {
    format_utc_rfc3339_millis(Utc::now())
}

pub fn unix_timestamp_secs() -> u64 {
    unix_timestamp_secs_at(Utc::now())
}

fn format_utc_rfc3339(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339()
}

fn format_utc_rfc3339_millis(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn unix_timestamp_secs_at(timestamp: DateTime<Utc>) -> u64 {
    u64::try_from(timestamp.timestamp()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Timelike, Utc};

    use super::*;

    #[test]
    fn formats_utc_timestamp_without_fractional_seconds() {
        let timestamp = Utc.with_ymd_and_hms(2026, 7, 9, 6, 30, 15).unwrap();

        assert_eq!(format_utc_rfc3339(timestamp), "2026-07-09T06:30:15+00:00");
    }

    #[test]
    fn formats_utc_timestamp_with_millisecond_precision() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 9, 6, 30, 15)
            .unwrap()
            .with_nanosecond(123_456_789)
            .unwrap();

        assert_eq!(
            format_utc_rfc3339_millis(timestamp),
            "2026-07-09T06:30:15.123Z"
        );
    }

    #[test]
    fn converts_utc_timestamp_to_unix_seconds() {
        let timestamp = Utc.with_ymd_and_hms(2026, 7, 9, 6, 30, 15).unwrap();

        assert_eq!(unix_timestamp_secs_at(timestamp), 1_783_578_615);
    }
}
