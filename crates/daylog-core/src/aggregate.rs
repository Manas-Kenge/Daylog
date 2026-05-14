use chrono::{DateTime, Local, Timelike, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::aw_client::{queries, AwClient, AwError};
use crate::time::TimeRange;

pub async fn fetch_window_events(
    client: &AwClient,
    range: &TimeRange,
) -> Result<Vec<Value>, AwError> {
    let res = client
        .query(&queries::timeline(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

pub async fn fetch_afk_events(
    client: &AwClient,
    range: &TimeRange,
) -> Result<Vec<Value>, AwError> {
    let res = client
        .query(&queries::afk_events(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

pub fn unwrap_first_array(res: Vec<Value>) -> Vec<Value> {
    res.into_iter()
        .next()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

#[derive(Debug, Serialize, PartialEq)]
pub struct HourBucket {
    pub hour: u8,
    pub duration: f64,
}

pub fn bucketize_hourly(events: &[Value]) -> Vec<HourBucket> {
    let mut totals = [0.0_f64; 24];
    for ev in events {
        let timestamp = ev
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let duration = ev.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if let Some(start) = timestamp {
            split_event_into_hours(start, duration, &mut totals);
        }
    }
    totals
        .iter()
        .enumerate()
        .map(|(hour, &duration)| HourBucket {
            hour: hour as u8,
            duration,
        })
        .collect()
}

fn split_event_into_hours(start_utc: DateTime<Utc>, duration: f64, totals: &mut [f64; 24]) {
    if duration <= 0.0 {
        return;
    }
    let mut remaining = duration;
    let mut cursor = start_utc;
    // Cap iterations defensively (a 30-day event would be 720 hours; allow up to that).
    for _ in 0..(24 * 31) {
        if remaining <= 0.0 {
            break;
        }
        let local = cursor.with_timezone(&Local);
        let hour = local.hour() as usize;
        let next_local_hour = (local + chrono::Duration::hours(1))
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0));
        let Some(next_local) = next_local_hour else {
            break;
        };
        let next_utc = next_local.with_timezone(&Utc);
        let span_secs = (next_utc - cursor).num_milliseconds() as f64 / 1000.0;
        if span_secs <= 0.0 {
            break;
        }
        let chunk = remaining.min(span_secs);
        totals[hour] += chunk;
        remaining -= chunk;
        cursor = next_utc;
    }
}

#[derive(Debug, Serialize)]
pub struct AfkInterval {
    pub timestamp: DateTime<Utc>,
    pub duration: f64,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct AfkSummary {
    pub active_seconds: f64,
    pub afk_seconds: f64,
    pub active_ratio: f64,
    pub intervals: Vec<AfkInterval>,
}

pub fn summarize_afk(events: &[Value], include_intervals: bool) -> AfkSummary {
    let mut active = 0.0_f64;
    let mut afk = 0.0_f64;
    let mut intervals = Vec::new();
    for ev in events {
        let duration = ev.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let status = ev
            .get("data")
            .and_then(|d| d.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match status {
            "not-afk" => active += duration,
            "afk" => afk += duration,
            _ => {}
        }
        if include_intervals {
            if let Some(ts) = ev
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            {
                intervals.push(AfkInterval {
                    timestamp: ts.with_timezone(&Utc),
                    duration,
                    status: status.to_string(),
                });
            }
        }
    }
    let total = active + afk;
    let active_ratio = if total > 0.0 { active / total } else { 0.0 };
    AfkSummary {
        active_seconds: active,
        afk_seconds: afk,
        active_ratio,
        intervals,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategorizedEvent {
    pub timestamp: DateTime<Utc>,
    pub duration: f64,
    pub data: Value,
    pub category: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CategorySummary {
    pub name: Vec<String>,
    pub duration: f64,
}

/// Pull `data.$category` off each event. Server always writes
/// `["Uncategorized"]` for unmatched events. Events lacking a parseable
/// timestamp are dropped.
pub fn parse_categorized_events(events: &[Value]) -> Vec<CategorizedEvent> {
    events
        .iter()
        .filter_map(|ev| {
            let timestamp = ev
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))?;
            let duration = ev.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let data = ev.get("data").cloned().unwrap_or(Value::Null);
            let category = data
                .get("$category")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec!["Uncategorized".into()]);
            Some(CategorizedEvent {
                timestamp,
                duration,
                data,
                category,
            })
        })
        .collect()
}

/// Server-merged events from `merge_events_by_keys(events, ["$category"])`
/// arrive shaped as `{data: {"$category": [..]}, duration: <sum>}`. Flatten
/// to `{name, duration}` pairs sorted by duration desc (server already
/// sorts, but we re-sort defensively in case a future upstream change
/// removes it).
pub fn parse_category_summaries(events: &[Value]) -> Vec<CategorySummary> {
    let mut out: Vec<CategorySummary> = events
        .iter()
        .filter_map(|ev| {
            let duration = ev.get("duration").and_then(|v| v.as_f64())?;
            let name = ev
                .get("data")
                .and_then(|d| d.get("$category"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })?;
            if name.is_empty() {
                return None;
            }
            Some(CategorySummary { name, duration })
        })
        .collect();
    out.sort_by(|a, b| {
        b.duration
            .partial_cmp(&a.duration)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hourly_returns_24_buckets_for_empty() {
        let out = bucketize_hourly(&[]);
        assert_eq!(out.len(), 24);
        for (i, b) in out.iter().enumerate() {
            assert_eq!(b.hour as usize, i);
            assert_eq!(b.duration, 0.0);
        }
    }

    #[test]
    fn hourly_attributes_to_local_hour() {
        let now_local = Local::now();
        let start_local = now_local
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();
        let start_utc = start_local.with_timezone(&Utc);
        let ev = json!({
            "timestamp": start_utc.to_rfc3339(),
            "duration": 1800.0,
            "data": {},
        });
        let out = bucketize_hourly(&[ev]);
        let h = start_local.hour() as usize;
        assert_eq!(out[h].duration, 1800.0);
        let total: f64 = out.iter().map(|b| b.duration).sum();
        assert_eq!(total, 1800.0);
    }

    #[test]
    fn hourly_splits_across_boundary() {
        let now_local = Local::now();
        let base = now_local
            .with_minute(45)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();
        let start_utc = base.with_timezone(&Utc);
        let ev = json!({
            "timestamp": start_utc.to_rfc3339(),
            "duration": 1800.0,
            "data": {},
        });
        let out = bucketize_hourly(&[ev]);
        let h = base.hour() as usize;
        let h_next = (h + 1) % 24;
        assert!((out[h].duration - 900.0).abs() < 1.0);
        assert!((out[h_next].duration - 900.0).abs() < 1.0);
    }

    #[test]
    fn hourly_ignores_zero_and_negative_duration() {
        let ev = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "duration": 0.0,
            "data": {},
        });
        let out = bucketize_hourly(&[ev]);
        let total: f64 = out.iter().map(|b| b.duration).sum();
        assert_eq!(total, 0.0);
    }

    #[test]
    fn afk_summary_basic_ratio() {
        let events = vec![
            json!({"timestamp": "2026-01-01T10:00:00Z", "duration": 300.0, "data": {"status": "not-afk"}}),
            json!({"timestamp": "2026-01-01T10:05:00Z", "duration": 100.0, "data": {"status": "afk"}}),
        ];
        let s = summarize_afk(&events, false);
        assert_eq!(s.active_seconds, 300.0);
        assert_eq!(s.afk_seconds, 100.0);
        assert!((s.active_ratio - 0.75).abs() < 1e-9);
        assert!(s.intervals.is_empty());
    }

    #[test]
    fn afk_summary_empty_is_nan_safe() {
        let s = summarize_afk(&[], true);
        assert_eq!(s.active_seconds, 0.0);
        assert_eq!(s.afk_seconds, 0.0);
        assert_eq!(s.active_ratio, 0.0);
    }

    #[test]
    fn afk_summary_include_intervals() {
        let events = vec![
            json!({"timestamp": "2026-01-01T10:00:00Z", "duration": 60.0, "data": {"status": "not-afk"}}),
            json!({"timestamp": "2026-01-01T10:01:00Z", "duration": 60.0, "data": {"status": "afk"}}),
        ];
        let s = summarize_afk(&events, true);
        assert_eq!(s.intervals.len(), 2);
        assert_eq!(s.intervals[0].status, "not-afk");
        assert_eq!(s.intervals[1].status, "afk");
    }

    #[test]
    fn parse_categorized_events_pulls_category_from_data() {
        let events = vec![
            json!({
                "timestamp": "2026-01-01T10:00:00Z",
                "duration": 10.0,
                "data": {"app": "vim", "$category": ["Work", "Programming"]}
            }),
            json!({
                "timestamp": "2026-01-01T10:01:00Z",
                "duration": 20.0,
                "data": {"app": "weirdapp", "$category": ["Uncategorized"]}
            }),
        ];
        let out = parse_categorized_events(&events);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].category, vec!["Work", "Programming"]);
        assert_eq!(out[1].category, vec!["Uncategorized"]);
    }

    #[test]
    fn parse_categorized_events_drops_events_without_timestamp() {
        let events = vec![json!({"duration": 10.0, "data": {"$category": ["Work"]}})];
        let out = parse_categorized_events(&events);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_categorized_events_falls_back_when_category_missing() {
        let events = vec![json!({
            "timestamp": "2026-01-01T10:00:00Z",
            "duration": 10.0,
            "data": {"app": "x"}
        })];
        let out = parse_categorized_events(&events);
        assert_eq!(out[0].category, vec!["Uncategorized"]);
    }

    #[test]
    fn parse_category_summaries_groups_and_sorts() {
        let events = vec![
            json!({"data": {"$category": ["Work", "Programming"]}, "duration": 150.0}),
            json!({"data": {"$category": ["Media", "Music"]}, "duration": 200.0}),
            json!({"data": {"$category": ["Uncategorized"]}, "duration": 25.0}),
        ];
        let out = parse_category_summaries(&events);
        assert_eq!(out[0].name, vec!["Media", "Music"]);
        assert_eq!(out[0].duration, 200.0);
        assert_eq!(out[1].name, vec!["Work", "Programming"]);
        assert_eq!(out[1].duration, 150.0);
        assert_eq!(out[2].name, vec!["Uncategorized"]);
    }

    #[test]
    fn unwrap_first_array_handles_empty() {
        assert!(unwrap_first_array(vec![]).is_empty());
        assert!(unwrap_first_array(vec![json!(null)]).is_empty());
        assert_eq!(unwrap_first_array(vec![json!([1, 2, 3])]).len(), 3);
    }
}
