//! High-level read API consumed by both the Tauri IPC handlers and the
//! TUI. Each function takes a borrowed `AwClient`, runs the AQL query +
//! parses the response, and returns a plain Rust type. The Tauri handlers
//! become one-line `#[tauri::command]` delegates to these functions; the
//! TUI calls them directly without the IPC layer.

use serde::Serialize;
use serde_json::Value;

use crate::aggregate::{
    bucketize_hourly, fetch_afk_events, fetch_window_events, parse_categorized_events,
    parse_category_summaries, summarize_afk, unwrap_first_array, AfkSummary, CategorizedEvent,
    CategorySummary, HourBucket,
};
use crate::aw_client::{queries, AwClient, AwError, Bucket, Event, ServerInfo};
use crate::categories::{self, CategoryError};
use crate::kpi::{self, KpiSummary};
use crate::time::TimeRange;

/// Errors surfaced by the high-level query API. Both Tauri IPC and the TUI
/// serialize these to strings; `serde::Serialize` is implemented manually
/// to avoid leaking enum-variant structure to consumers.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("{0}")]
    Aw(#[from] AwError),
    #[error("{0}")]
    Category(#[from] CategoryError),
    #[error("task join: {0}")]
    Join(String),
}

impl serde::Serialize for QueryError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// Bundled categorized-events + AFK summary for a contiguous past-day
/// window (days 1..=N, where 1 = yesterday). One IPC roundtrip from JS,
/// per-day queries dispatched concurrently inside Rust so HTTP keep-alive
/// against aw-server actually pays off. Today's slot is intentionally
/// not bundled here: the dashboard refreshes today every 5s while past
/// days only need a 5min staleness check.
#[derive(Debug, Serialize)]
pub struct TrailingDayPayload {
    pub days_ago: u32,
    pub events: Vec<CategorizedEvent>,
    pub afk: AfkSummary,
}

pub async fn info(client: &AwClient) -> Result<ServerInfo, AwError> {
    client.info().await
}

pub async fn buckets(client: &AwClient) -> Result<Vec<Bucket>, AwError> {
    client.buckets().await
}

pub async fn events(
    client: &AwClient,
    bucket_id: &str,
    start: Option<chrono::DateTime<chrono::Utc>>,
    end: Option<chrono::DateTime<chrono::Utc>>,
    limit: Option<u32>,
) -> Result<Vec<Event>, AwError> {
    client.events(bucket_id, start, end, limit).await
}

pub async fn raw_query(
    client: &AwClient,
    query: &str,
    timeperiods: &[String],
) -> Result<Vec<Value>, AwError> {
    client.query(query, timeperiods).await
}

pub async fn top_apps(client: &AwClient, range: TimeRange) -> Result<Vec<Value>, AwError> {
    let res = client
        .query(&queries::top_apps(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

pub async fn timeline(client: &AwClient, range: TimeRange) -> Result<Vec<Value>, AwError> {
    let res = client
        .query(&queries::timeline(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

pub async fn top_categories(
    client: &AwClient,
    range: TimeRange,
) -> Result<Vec<CategorySummary>, QueryError> {
    let cfg = categories::load(client).await?;
    let classes_json = categories::classes_to_aql(&cfg);
    let res = client
        .query(
            &queries::top_categories(&classes_json),
            &[range.as_aw_timeperiod()],
        )
        .await?;
    Ok(parse_category_summaries(&unwrap_first_array(res)))
}

pub async fn hourly(client: &AwClient, range: TimeRange) -> Result<Vec<HourBucket>, AwError> {
    let events = fetch_window_events(client, &range).await?;
    Ok(bucketize_hourly(&events))
}

pub async fn categorized_events(
    client: &AwClient,
    range: TimeRange,
) -> Result<Vec<CategorizedEvent>, QueryError> {
    let cfg = categories::load(client).await?;
    let classes_json = categories::classes_to_aql(&cfg);
    let res = client
        .query(
            &queries::categorized_events(&classes_json),
            &[range.as_aw_timeperiod()],
        )
        .await?;
    Ok(parse_categorized_events(&unwrap_first_array(res)))
}

pub async fn afk_summary(
    client: &AwClient,
    range: TimeRange,
    include_intervals: bool,
) -> Result<AfkSummary, AwError> {
    let buckets = client.buckets().await?;
    if !buckets.iter().any(|b| b.id.starts_with("aw-watcher-afk_")) {
        return Ok(summarize_afk(&[], include_intervals));
    }
    let events = fetch_afk_events(client, &range).await?;
    Ok(summarize_afk(&events, include_intervals))
}

pub async fn trailing_days_past(days: u32) -> Result<Vec<TrailingDayPayload>, QueryError> {
    if days == 0 {
        return Ok(Vec::new());
    }
    // Pre-load + cache categories once; per-day tasks below hit the cache.
    let client = AwClient::new();
    let _ = categories::load(&client).await?;

    let mut handles = Vec::with_capacity(days as usize);
    for n in 1..=days {
        handles.push(tokio::spawn(async move { fetch_trailing_day(n).await }));
    }
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        match h.await {
            Ok(res) => out.push(res?),
            Err(e) => return Err(QueryError::Join(e.to_string())),
        }
    }
    out.sort_by_key(|d| d.days_ago);
    Ok(out)
}

async fn fetch_trailing_day(n: u32) -> Result<TrailingDayPayload, QueryError> {
    let client = AwClient::new();
    let cfg = categories::load(&client).await?;
    let classes_json = categories::classes_to_aql(&cfg);
    let range = TimeRange::DaysAgo { days: n };
    let timeperiods = [range.as_aw_timeperiod()];
    let aql = queries::categorized_events(&classes_json);
    let (events_res, afk_events) = tokio::join!(
        client.query(&aql, &timeperiods),
        fetch_afk_events(&client, &range),
    );
    let events = parse_categorized_events(&unwrap_first_array(events_res?));
    let afk = summarize_afk(&afk_events?, false);
    Ok(TrailingDayPayload {
        days_ago: n,
        events,
        afk,
    })
}

/// One-shot KPI strip payload: today's active/AFK + categorized events,
/// plus trailing-7 past days for baselines and pattern-shift detection.
/// Past days are fetched concurrently. The desktop's KpiStrip becomes a
/// thin wrapper over this command; the TUI consumes the same payload via
/// `daylog_core::queries::kpi` directly without IPC.
pub async fn kpi(client: &AwClient, range: TimeRange) -> Result<KpiSummary, QueryError> {
    let cfg = categories::load(client).await?;
    let classes_json = categories::classes_to_aql(&cfg);

    // Today: events + AFK in parallel.
    let timeperiods = [range.as_aw_timeperiod()];
    let today_aql = queries::categorized_events(&classes_json);
    let (today_events_res, today_afk_events) = tokio::join!(
        client.query(&today_aql, &timeperiods),
        fetch_afk_events(client, &range),
    );
    let today_events = parse_categorized_events(&unwrap_first_array(today_events_res?));
    let today_afk = summarize_afk(&today_afk_events?, false);

    // Past 7 days for baselines + pattern shift. Reuse the existing
    // bundled query so concurrency lives in one place.
    let past = trailing_days_past(7).await?;
    let past_days: Vec<Vec<CategorizedEvent>> =
        past.iter().map(|d| d.events.clone()).collect();
    let past_active: Vec<f64> = past.iter().map(|d| d.afk.active_seconds).collect();

    let weekday = kpi::weekday_label(chrono::Utc::now());
    Ok(kpi::summarize(
        &today_events,
        &past_days,
        &past_active,
        today_afk.active_seconds,
        today_afk.afk_seconds,
        &weekday,
    ))
}

pub async fn has_web_watcher(client: &AwClient) -> Result<bool, AwError> {
    let buckets = client.buckets().await?;
    Ok(buckets.iter().any(|b| b.id.starts_with("aw-watcher-web-")))
}

pub async fn top_domains(client: &AwClient, range: TimeRange) -> Result<Vec<Value>, AwError> {
    let buckets = client.buckets().await?;
    if !buckets.iter().any(|b| b.id.starts_with("aw-watcher-web-")) {
        return Ok(vec![]);
    }
    let res = client
        .query(&queries::web_top_domains(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

pub async fn top_urls(client: &AwClient, range: TimeRange) -> Result<Vec<Value>, AwError> {
    let buckets = client.buckets().await?;
    if !buckets.iter().any(|b| b.id.starts_with("aw-watcher-web-")) {
        return Ok(vec![]);
    }
    let res = client
        .query(&queries::web_top_urls(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}
