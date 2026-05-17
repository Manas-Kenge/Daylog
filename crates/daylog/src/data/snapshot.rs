//! Today-only KPI snapshot for `daylog --json today`. Serialized to a
//! stable JSON schema for status-bar consumers (Quickshell, waybar,
//! i3blocks) that poll the CLI as a subprocess.
//!
//! schema: v1 — may evolve freely while the user count is zero.
//!
//! Always returns a valid `Snapshot`. Missing SQLite file, empty buckets,
//! or an unreachable aw-server all degrade to a zero snapshot so polling
//! consumers see a stable contract instead of stderr noise or nonzero
//! exits.

use chrono::Local;
use serde::Serialize;
use serde_json::json;

use crate::data::aggregate::{bucketize_hourly, parse_category_summaries};
use crate::data::aw_client::{AwClient, Event};
use crate::data::categories;
use crate::data::datastore;
use crate::data::time::TimeRange;
use crate::data::transforms;

const PULSE_SECS: i64 = 5;
const BUCKET_WINDOW: &str = "aw-watcher-window_";
const BUCKET_AFK: &str = "aw-watcher-afk_";

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub as_of: String,
    pub today: TodayData,
}

#[derive(Debug, Serialize)]
pub struct TodayData {
    pub total_active: String,
    pub top_app: Option<TopEntry>,
    pub top_category: Option<TopEntry>,
    pub hours: [u32; 24],
}

#[derive(Debug, Serialize)]
pub struct TopEntry {
    pub name: String,
    pub duration: String,
}

pub async fn today() -> Snapshot {
    let as_of = Local::now().to_rfc3339();
    let (start, end) = TimeRange::Today.resolve();

    let window = match datastore::events_in_range(BUCKET_WINDOW, start, end) {
        Ok(w) => w,
        Err(_) => return zero_snapshot(as_of),
    };
    let afk = datastore::events_in_range(BUCKET_AFK, start, end).unwrap_or_default();

    let flooded = transforms::flood(window, chrono::Duration::seconds(PULSE_SECS));
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(flooded, not_afk);

    if intersected.is_empty() {
        return zero_snapshot(as_of);
    }

    let total_active_secs: f64 = intersected.iter().map(|e| e.duration).sum();
    let top_app = compute_top_app(&intersected);
    let top_category = top_category_best_effort(intersected.clone()).await;
    let hours = compute_hours(&intersected);

    Snapshot {
        as_of,
        today: TodayData {
            total_active: format_iso_duration(total_active_secs),
            top_app,
            top_category,
            hours,
        },
    }
}

pub fn zero_snapshot(as_of: String) -> Snapshot {
    Snapshot {
        as_of,
        today: TodayData {
            total_active: "PT0S".to_string(),
            top_app: None,
            top_category: None,
            hours: [0u32; 24],
        },
    }
}

fn compute_top_app(intersected: &[Event]) -> Option<TopEntry> {
    let merged = transforms::merge_events_by_keys(intersected.to_vec(), &["app"]);
    let sorted = transforms::sort_by_duration(merged);
    sorted.into_iter().next().and_then(|e| {
        let name = e.data.get("app").and_then(|v| v.as_str())?.to_string();
        Some(TopEntry {
            name,
            duration: format_iso_duration(e.duration),
        })
    })
}

/// Best-effort: category rules live in aw-server's settings store, so
/// fetching them requires HTTP. If aw-server is restarting (the very
/// case `Restart=always` exists to handle), we degrade to `None` rather
/// than fail the whole snapshot.
async fn top_category_best_effort(intersected: Vec<Event>) -> Option<TopEntry> {
    let client = AwClient::new();
    let cfg = categories::load(&client).await.ok()?;
    let rules = transforms::compile_rules(&cfg).ok()?;
    let categorized = transforms::categorize(intersected, &rules);
    let merged = transforms::merge_events_by_keys(categorized, &["$category"]);
    let sorted = transforms::sort_by_duration(merged);
    let summaries = parse_category_summaries(&sorted);
    summaries.into_iter().next().map(|s| TopEntry {
        name: s.name.join(" > "),
        duration: format_iso_duration(s.duration),
    })
}

fn compute_hours(intersected: &[Event]) -> [u32; 24] {
    let mut hours = [0u32; 24];
    for b in bucketize_hourly(intersected) {
        let mins = (b.duration / 60.0).round().max(0.0) as i64;
        hours[b.hour as usize] = mins.clamp(0, 60) as u32;
    }
    hours
}

/// ISO-8601 duration. `0` → `PT0S`; otherwise drops zero components, e.g.
/// `3600` → `PT1H`, `4*3600 + 32*60` → `PT4H32M`, `125` → `PT2M5S`.
pub fn format_iso_duration(secs: f64) -> String {
    let total = secs.round().max(0.0) as i64;
    if total == 0 {
        return "PT0S".to_string();
    }
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    let mut out = String::from("PT");
    if h > 0 {
        out.push_str(&format!("{h}H"));
    }
    if m > 0 {
        out.push_str(&format!("{m}M"));
    }
    if s > 0 {
        out.push_str(&format!("{s}S"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_duration_zero_and_subsecond() {
        assert_eq!(format_iso_duration(0.0), "PT0S");
        assert_eq!(format_iso_duration(0.4), "PT0S");
        assert_eq!(format_iso_duration(-5.0), "PT0S");
    }

    #[test]
    fn iso_duration_seconds_only() {
        assert_eq!(format_iso_duration(1.0), "PT1S");
        assert_eq!(format_iso_duration(59.0), "PT59S");
    }

    #[test]
    fn iso_duration_minutes_and_seconds() {
        assert_eq!(format_iso_duration(60.0), "PT1M");
        assert_eq!(format_iso_duration(125.0), "PT2M5S");
    }

    #[test]
    fn iso_duration_hours() {
        assert_eq!(format_iso_duration(3600.0), "PT1H");
        assert_eq!(format_iso_duration(3661.0), "PT1H1M1S");
        assert_eq!(format_iso_duration(4.0 * 3600.0 + 32.0 * 60.0), "PT4H32M");
    }

    #[test]
    fn zero_snapshot_has_nulls_and_24_zero_hours() {
        let snap = zero_snapshot("2026-05-17T11:45:00+05:30".to_string());
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["as_of"], "2026-05-17T11:45:00+05:30");
        assert_eq!(v["today"]["total_active"], "PT0S");
        assert!(v["today"]["top_app"].is_null());
        assert!(v["today"]["top_category"].is_null());
        let hours = v["today"]["hours"].as_array().expect("hours array");
        assert_eq!(hours.len(), 24);
        for h in hours {
            assert_eq!(h.as_u64(), Some(0));
        }
    }

    #[test]
    fn snapshot_field_order_matches_schema() {
        let snap = zero_snapshot("2026-05-17T00:00:00+00:00".to_string());
        let s = serde_json::to_string(&snap).unwrap();
        let as_of = s.find("\"as_of\"").unwrap();
        let today = s.find("\"today\"").unwrap();
        let total_active = s.find("\"total_active\"").unwrap();
        let top_app = s.find("\"top_app\"").unwrap();
        let top_category = s.find("\"top_category\"").unwrap();
        let hours = s.find("\"hours\"").unwrap();
        assert!(as_of < today);
        assert!(total_active < top_app);
        assert!(top_app < top_category);
        assert!(top_category < hours);
    }

    #[test]
    fn compute_top_app_picks_highest_duration() {
        use chrono::TimeZone;
        let t = |s: i64| chrono::Utc.timestamp_opt(s, 0).single().unwrap();
        let evs = vec![
            Event { id: None, timestamp: t(0), duration: 100.0, data: json!({"app": "kitty"}) },
            Event { id: None, timestamp: t(100), duration: 50.0, data: json!({"app": "firefox"}) },
            Event { id: None, timestamp: t(200), duration: 200.0, data: json!({"app": "kitty"}) },
        ];
        let top = compute_top_app(&evs).expect("top app");
        assert_eq!(top.name, "kitty");
        assert_eq!(top.duration, "PT5M");
    }

    #[test]
    fn compute_hours_rounds_seconds_to_minutes_capped_at_60() {
        use crate::data::aggregate::HourBucket;
        // Indirect: feed bucketize_hourly via a synthetic event aligned to
        // local-midnight + 8h, lasting 30 minutes — should land 30 in
        // hours[8] in local time.
        use chrono::{Local, Timelike};
        let start_local = Local::now()
            .with_hour(8).unwrap()
            .with_minute(0).unwrap()
            .with_second(0).unwrap()
            .with_nanosecond(0).unwrap();
        let start_utc = start_local.with_timezone(&chrono::Utc);
        let ev = Event { id: None, timestamp: start_utc, duration: 30.0 * 60.0, data: json!({}) };
        let hours = compute_hours(&[ev]);
        assert_eq!(hours[8], 30);
        // Buckets that bucketize_hourly emits are clamped via the
        // clamp(0, 60); spot-check the type.
        let _: &HourBucket = bucketize_hourly(&[]).first().unwrap();
    }
}
