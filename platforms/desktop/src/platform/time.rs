use chrono::{DateTime, Local, NaiveDate, SecondsFormat, Utc};
use sona_core::dashboard::DashboardSnapshotTime;

pub fn utc_now_rfc3339() -> String {
    format_utc_rfc3339(Utc::now())
}

pub fn utc_now_rfc3339_millis() -> String {
    format_utc_rfc3339_millis(Utc::now())
}

pub fn unix_timestamp_secs() -> u64 {
    unix_timestamp_secs_at(Utc::now())
}

pub fn unix_timestamp_millis() -> u64 {
    unix_timestamp_millis_at(Utc::now())
}

pub fn dashboard_snapshot_time_now() -> DashboardSnapshotTime {
    let now = Utc::now();
    let local_now = now.with_timezone(&Local);
    dashboard_snapshot_time_from_parts(
        now,
        local_now.date_naive(),
        local_now.offset().local_minus_utc(),
    )
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

fn unix_timestamp_millis_at(timestamp: DateTime<Utc>) -> u64 {
    u64::try_from(timestamp.timestamp_millis()).unwrap_or_default()
}

fn dashboard_snapshot_time_from_parts(
    timestamp: DateTime<Utc>,
    today: NaiveDate,
    local_utc_offset_seconds: i32,
) -> DashboardSnapshotTime {
    DashboardSnapshotTime {
        generated_at: format_utc_rfc3339_millis(timestamp),
        today,
        local_utc_offset_seconds,
    }
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

    #[test]
    fn converts_utc_timestamp_to_unix_milliseconds() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 9, 6, 30, 15)
            .unwrap()
            .with_nanosecond(123_000_000)
            .unwrap();

        assert_eq!(unix_timestamp_millis_at(timestamp), 1_783_578_615_123);
    }

    #[test]
    fn builds_dashboard_snapshot_time_from_adapter_values() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 9, 16, 30, 15)
            .unwrap()
            .with_nanosecond(987_654_321)
            .unwrap();
        let today = chrono::NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        let time = dashboard_snapshot_time_from_parts(timestamp, today, 28_800);

        assert_eq!(time.generated_at, "2026-07-09T16:30:15.987Z");
        assert_eq!(time.today, today);
        assert_eq!(time.local_utc_offset_seconds, 28_800);
    }
}
