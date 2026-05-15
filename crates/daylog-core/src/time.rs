use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimeRange {
    Today,
    Yesterday,
    LastNDays { days: u32 },
    DaysAgo { days: u32 },
    Custom { start: DateTime<Utc>, end: DateTime<Utc> },
}

impl TimeRange {
    pub fn resolve(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        match self {
            TimeRange::Today => day_window(0),
            TimeRange::Yesterday => day_window(1),
            TimeRange::DaysAgo { days } => day_window(*days as i64),
            TimeRange::LastNDays { days } => {
                let end = Utc::now();
                let start = end - Duration::days(*days as i64);
                (start, end)
            }
            TimeRange::Custom { start, end } => (*start, *end),
        }
    }

}

fn local_midnight(date: chrono::NaiveDate) -> DateTime<Utc> {
    Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .single()
        .unwrap_or_else(|| {
            Local
                .from_local_datetime(&date.and_time(NaiveTime::MIN))
                .earliest()
                .expect("local midnight resolvable")
        })
        .with_timezone(&Utc)
}

fn day_window(days_ago: i64) -> (DateTime<Utc>, DateTime<Utc>) {
    let today_local_date = Local::now().date_naive();
    let target_date = today_local_date - chrono::Duration::days(days_ago);
    let start = local_midnight(target_date);
    let end = local_midnight(target_date + chrono::Duration::days(1));
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_24_hours() {
        let (start, end) = TimeRange::Today.resolve();
        let span = end - start;
        // 23..=25 hours to allow DST.
        assert!(span >= Duration::hours(23) && span <= Duration::hours(25));
    }

    #[test]
    fn yesterday_ends_where_today_starts() {
        let (_, y_end) = TimeRange::Yesterday.resolve();
        let (t_start, _) = TimeRange::Today.resolve();
        assert_eq!(y_end, t_start);
    }

    #[test]
    fn days_ago_one_equals_yesterday() {
        let a = TimeRange::Yesterday.resolve();
        let b = TimeRange::DaysAgo { days: 1 }.resolve();
        assert_eq!(a, b);
    }

    #[test]
    fn last_n_days_span_is_n_days() {
        let (start, end) = TimeRange::LastNDays { days: 7 }.resolve();
        let span = end - start;
        assert_eq!(span, Duration::days(7));
    }

    #[test]
    fn custom_passes_through() {
        let s = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let e = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
        let (rs, re) = TimeRange::Custom { start: s, end: e }.resolve();
        assert_eq!(rs, s);
        assert_eq!(re, e);
    }

}
