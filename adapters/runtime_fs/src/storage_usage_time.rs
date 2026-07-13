use chrono::{DateTime, Utc};

pub fn storage_usage_generated_at_now() -> String {
    storage_usage_generated_at(Utc::now())
}

fn storage_usage_generated_at(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Timelike, Utc};

    #[test]
    fn preserves_the_existing_storage_usage_timestamp_format() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 13, 20, 30, 15)
            .unwrap()
            .with_nanosecond(123_456_789)
            .unwrap();

        assert_eq!(
            super::storage_usage_generated_at(timestamp),
            "2026-07-13T20:30:15.123456789+00:00"
        );
    }
}
