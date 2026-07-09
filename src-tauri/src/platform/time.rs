use chrono::{DateTime, SecondsFormat, Utc};

pub fn utc_now_rfc3339() -> String {
    format_utc_rfc3339(Utc::now())
}

pub fn utc_now_rfc3339_millis() -> String {
    format_utc_rfc3339_millis(Utc::now())
}

fn format_utc_rfc3339(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339()
}

fn format_utc_rfc3339_millis(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
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
}
