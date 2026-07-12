use chrono::{DateTime, FixedOffset, Local, Offset, SecondsFormat, Utc};
use sona_core::dashboard::DashboardSnapshotTime;

pub fn dashboard_snapshot_time_now() -> DashboardSnapshotTime {
    let now = Utc::now();
    let local_now = now.with_timezone(&Local);
    dashboard_snapshot_time_at_offset(now, local_now.offset().fix())
}

fn dashboard_snapshot_time_at_offset(
    timestamp: DateTime<Utc>,
    offset: FixedOffset,
) -> DashboardSnapshotTime {
    DashboardSnapshotTime {
        generated_at: timestamp.to_rfc3339_opts(SecondsFormat::Millis, true),
        today: timestamp.with_timezone(&offset).date_naive(),
        local_utc_offset_seconds: offset.local_minus_utc(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, NaiveDate, TimeZone, Timelike, Utc};

    #[test]
    fn derives_next_local_date_from_positive_offset() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 7, 13, 20, 30, 15)
            .unwrap()
            .with_nanosecond(987_654_321)
            .unwrap();
        let offset = FixedOffset::east_opt(28_800).unwrap();

        let time = super::dashboard_snapshot_time_at_offset(timestamp, offset);

        assert_eq!(time.generated_at, "2026-07-13T20:30:15.987Z");
        assert_eq!(time.today, NaiveDate::from_ymd_opt(2026, 7, 14).unwrap());
        assert_eq!(time.local_utc_offset_seconds, 28_800);
    }

    #[test]
    fn derives_previous_local_date_from_negative_offset() {
        let timestamp = Utc.with_ymd_and_hms(2026, 7, 13, 4, 30, 15).unwrap();
        let offset = FixedOffset::west_opt(28_800).unwrap();

        let time = super::dashboard_snapshot_time_at_offset(timestamp, offset);

        assert_eq!(time.generated_at, "2026-07-13T04:30:15.000Z");
        assert_eq!(time.today, NaiveDate::from_ymd_opt(2026, 7, 12).unwrap());
        assert_eq!(time.local_utc_offset_seconds, -28_800);
    }
}
