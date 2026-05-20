use chrono::Duration as ChronoDuration;
use serde::Serialize;
use serde_json::{json, Value};

use crate::data::aggregate::{
    bucketize_hourly, parse_categorized_events, parse_category_summaries, summarize_afk,
    AfkSummary, CategorizedEvent, CategorySummary, HourBucket,
};
use crate::data::aw_client::{AwClient, AwError, Event};
use crate::data::categories::{self, CategoryError};
use crate::data::datastore::{self, DatastoreError};
use crate::data::kpi::{self, KpiSummary};
use crate::data::time::TimeRange;
use crate::data::transforms;

const PULSE_SECS: i64 = 5;

const BUCKET_WINDOW: &str = "aw-watcher-window_";
const BUCKET_AFK: &str = "aw-watcher-afk_";
const BUCKET_WEB: &str = "aw-watcher-web-";

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("{0}")]
    Aw(#[from] AwError),
    #[error("{0}")]
    Category(#[from] CategoryError),
    #[error("{0}")]
    Datastore(#[from] DatastoreError),
    #[error("regex: {0}")]
    Regex(String),
}

impl From<fancy_regex::Error> for QueryError {
    fn from(e: fancy_regex::Error) -> Self {
        QueryError::Regex(e.to_string())
    }
}

impl serde::Serialize for QueryError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TrailingDayPayload {
    pub days_ago: u32,
    pub events: Vec<CategorizedEvent>,
    pub afk: AfkSummary,
}

fn range_bounds(range: &TimeRange) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    range.resolve()
}

/// Returns the raw JSON shape `TopAppRow::parse_many` consumes.
pub async fn top_apps(_client: &AwClient, range: TimeRange) -> Result<Vec<Value>, QueryError> {
    let (start, end) = range_bounds(&range);
    let window = datastore::events_in_range(BUCKET_WINDOW, start, end)?;
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    let flooded = transforms::flood(window, ChronoDuration::seconds(PULSE_SECS));
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(flooded, not_afk);
    let merged = transforms::merge_events_by_keys(intersected, &["app"]);
    let sorted = transforms::sort_by_duration(merged);
    Ok(sorted.into_iter().map(event_to_value).collect())
}

pub async fn timeline(_client: &AwClient, range: TimeRange) -> Result<Vec<Value>, QueryError> {
    let (start, end) = range_bounds(&range);
    let window = datastore::events_in_range(BUCKET_WINDOW, start, end)?;
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    let flooded = transforms::flood(window, ChronoDuration::seconds(PULSE_SECS));
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(flooded, not_afk);
    Ok(intersected.into_iter().map(event_to_value).collect())
}

pub async fn top_categories(
    client: &AwClient,
    range: TimeRange,
) -> Result<Vec<CategorySummary>, QueryError> {
    let cfg = categories::load(client).await?;
    let rules = transforms::compile_rules(&cfg)?;
    let (start, end) = range_bounds(&range);
    let window = datastore::events_in_range(BUCKET_WINDOW, start, end)?;
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    let flooded = transforms::flood(window, ChronoDuration::seconds(PULSE_SECS));
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(flooded, not_afk);
    let categorized = transforms::categorize(intersected, &rules);
    let merged = transforms::merge_events_by_keys(categorized, &["$category"]);
    let sorted = transforms::sort_by_duration(merged);
    Ok(parse_category_summaries(&sorted))
}

pub async fn hourly(_client: &AwClient, range: TimeRange) -> Result<Vec<HourBucket>, QueryError> {
    let (start, end) = range_bounds(&range);
    let window = datastore::events_in_range(BUCKET_WINDOW, start, end)?;
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    let flooded = transforms::flood(window, ChronoDuration::seconds(PULSE_SECS));
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(flooded, not_afk);
    Ok(bucketize_hourly(&intersected))
}

pub async fn categorized_events(
    client: &AwClient,
    range: TimeRange,
) -> Result<Vec<CategorizedEvent>, QueryError> {
    let cfg = categories::load(client).await?;
    let rules = transforms::compile_rules(&cfg)?;
    let (start, end) = range_bounds(&range);
    let window = datastore::events_in_range(BUCKET_WINDOW, start, end)?;
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    let flooded = transforms::flood(window, ChronoDuration::seconds(PULSE_SECS));
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(flooded, not_afk);
    let categorized = transforms::categorize(intersected, &rules);
    Ok(parse_categorized_events(&categorized))
}

pub async fn afk_summary(
    _client: &AwClient,
    range: TimeRange,
    include_intervals: bool,
) -> Result<AfkSummary, QueryError> {
    let (start, end) = range_bounds(&range);
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    Ok(summarize_afk(&afk, include_intervals))
}

pub async fn top_domains(_client: &AwClient, range: TimeRange) -> Result<Vec<Value>, QueryError> {
    let (start, end) = range_bounds(&range);
    let web = datastore::events_in_range(BUCKET_WEB, start, end)?;
    if web.is_empty() {
        return Ok(Vec::new());
    }
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    let split = transforms::split_url_events(web);
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(split, not_afk);
    let merged = transforms::merge_events_by_keys(intersected, &["$domain"]);
    let sorted = transforms::sort_by_duration(merged);
    Ok(sorted.into_iter().map(event_to_value).collect())
}

pub async fn top_urls(_client: &AwClient, range: TimeRange) -> Result<Vec<Value>, QueryError> {
    let (start, end) = range_bounds(&range);
    let web = datastore::events_in_range(BUCKET_WEB, start, end)?;
    if web.is_empty() {
        return Ok(Vec::new());
    }
    let afk = datastore::events_in_range(BUCKET_AFK, start, end)?;
    let not_afk = transforms::filter_keyvals(afk, "status", &[json!("not-afk")]);
    let intersected = transforms::filter_period_intersect(web, not_afk);
    let merged = transforms::merge_events_by_keys(intersected, &["url"]);
    let sorted = transforms::sort_by_duration(merged);
    Ok(sorted.into_iter().map(event_to_value).collect())
}

pub async fn trailing_days_past(days: u32) -> Result<Vec<TrailingDayPayload>, QueryError> {
    if days == 0 {
        return Ok(Vec::new());
    }
    let client = AwClient::new();
    let cfg = categories::load(&client).await?;
    let rules = transforms::compile_rules(&cfg)?;

    let mut out = Vec::with_capacity(days as usize);
    for n in 1..=days {
        let range = TimeRange::DaysAgo { days: n };
        let (start, end) = range_bounds(&range);
        let window = datastore::events_in_range(BUCKET_WINDOW, start, end)?;
        let afk_raw = datastore::events_in_range(BUCKET_AFK, start, end)?;
        let flooded = transforms::flood(window, ChronoDuration::seconds(PULSE_SECS));
        let not_afk = transforms::filter_keyvals(
            afk_raw.clone(),
            "status",
            &[json!("not-afk")],
        );
        let intersected = transforms::filter_period_intersect(flooded, not_afk);
        let categorized = transforms::categorize(intersected, &rules);
        let events = parse_categorized_events(&categorized);
        let afk = summarize_afk(&afk_raw, false);
        out.push(TrailingDayPayload {
            days_ago: n,
            events,
            afk,
        });
    }
    Ok(out)
}

pub fn kpi_from_parts(
    today_events: &[CategorizedEvent],
    today_afk: &AfkSummary,
    past: &[TrailingDayPayload],
) -> KpiSummary {
    let past_days: Vec<Vec<CategorizedEvent>> = past.iter().map(|d| d.events.clone()).collect();
    let past_active: Vec<f64> = past.iter().map(|d| d.afk.active_seconds).collect();
    let weekday = kpi::weekday_label(chrono::Utc::now());
    kpi::summarize(
        today_events,
        &past_days,
        &past_active,
        today_afk.active_seconds,
        today_afk.afk_seconds,
        &weekday,
    )
}

pub async fn kpi(client: &AwClient, range: TimeRange) -> Result<KpiSummary, QueryError> {
    let today_events = categorized_events(client, range.clone()).await?;
    let today_afk = afk_summary(client, range, false).await?;
    let past = trailing_days_past(7).await?;
    Ok(kpi_from_parts(&today_events, &today_afk, &past))
}

fn event_to_value(e: Event) -> Value {
    let mut m = serde_json::Map::with_capacity(4);
    m.insert("timestamp".to_string(), Value::String(e.timestamp.to_rfc3339()));
    m.insert(
        "duration".to_string(),
        serde_json::Number::from_f64(e.duration)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    );
    m.insert("data".to_string(), e.data);
    if let Some(id) = e.id {
        m.insert("id".to_string(), Value::Number(id.into()));
    }
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::aggregate::AfkSummary;

    #[test]
    fn kpi_from_parts_forwards_today_afk_into_summary() {
        let today_afk = AfkSummary {
            active_seconds: 12_345.0,
            afk_seconds: 678.0,
            active_ratio: 0.95,
            intervals: Vec::new(),
        };
        let past: Vec<TrailingDayPayload> = (1..=7)
            .map(|n| TrailingDayPayload {
                days_ago: n,
                events: Vec::new(),
                afk: AfkSummary {
                    active_seconds: 1000.0 * n as f64,
                    afk_seconds: 0.0,
                    active_ratio: 1.0,
                    intervals: Vec::new(),
                },
            })
            .collect();

        let summary = kpi_from_parts(&[], &today_afk, &past);
        assert_eq!(summary.active_secs, 12_345.0);
        assert_eq!(summary.afk_secs, 678.0);
        assert!(summary.active_baseline.effective_days > 0);
        assert!(summary.active_baseline.median > 0.0);
    }
}
