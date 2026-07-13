use chrono::{DateTime, SecondsFormat, Utc};

pub fn diagnostics_scanned_at_now() -> String {
    diagnostics_scanned_at(Utc::now())
}

fn diagnostics_scanned_at(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Timelike, Utc};

    #[test]
    fn formats_diagnostics_timestamp_as_utc_milliseconds() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 13, 20, 30, 15)
            .unwrap()
            .with_nanosecond(987_654_321)
            .unwrap();

        assert_eq!(
            super::diagnostics_scanned_at(timestamp),
            "2026-07-13T20:30:15.987Z"
        );
    }
}
