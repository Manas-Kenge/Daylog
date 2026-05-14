//! High-level read API. Each fn takes a borrowed `AwClient`, runs the
//! AQL query, parses the response, and returns a plain Rust type.

use std::sync::OnceLock;

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

/// Memoized "does any bucket id start with `prefix`?" lookup. Watcher
/// presence doesn't change at runtime — installing aw-watcher-web or
/// aw-watcher-afk requires restarting the watcher tree, at which point
/// daylog also typically restarts. Without memoization, `top_domains`
/// re-fetches the bucket list every 5s on the live cadence purely to
/// re-confirm a fact that can't have changed.
pub async fn bucket_prefix_present(
    client: &AwClient,
    prefix: &'static str,
    memo: &OnceLock<bool>,
) -> Result<bool, AwError> {
    if let Some(v) = memo.get() {
        return Ok(*v);
    }
    let buckets = client.buckets().await?;
    let v = buckets.iter().any(|b| b.id.starts_with(prefix));
    // Race-safe: concurrent first-callers may both Set, only one wins;
    // both compute the same value so either outcome is correct.
    let _ = memo.set(v);
    Ok(v)
}

fn web_watcher_memo() -> &'static OnceLock<bool> {
    static MEMO: OnceLock<bool> = OnceLock::new();
    &MEMO
}

fn afk_watcher_memo() -> &'static OnceLock<bool> {
    static MEMO: OnceLock<bool> = OnceLock::new();
    &MEMO
}

/// Manually serializes to a string so the enum shape isn't part of the wire format.
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

/// One day's categorized events + AFK summary. `days_ago = 1` is
/// yesterday. Today isn't bundled here — it refreshes on a different cadence.
#[derive(Debug, Clone, Serialize)]
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
    if !bucket_prefix_present(client, "aw-watcher-afk_", afk_watcher_memo()).await? {
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

    // Bound the fan-out. Each per-day task issues 2 concurrent HTTP
    // calls (categorized query + AFK events), so a cap of 2 here means
    // at most 4 in-flight HTTP requests, which aw-server-rust handles
    // reliably. Higher caps (4+) intermittently overwhelm its accept
    // queue and reqwest reports the connect failures as `is_connect()`
    // -> AwError::Unreachable, even though the server is up. The retry
    // below handles the rare residual blip without surfacing it.
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
    let mut handles = Vec::with_capacity(days as usize);
    for n in 1..=days {
        let permit = sem.clone().acquire_owned().await.map_err(|e| {
            QueryError::Join(format!("semaphore closed: {e}"))
        })?;
        handles.push(tokio::spawn(async move {
            let _permit = permit; // released when task ends
            // One retry on connect failure: the cap above prevents most
            // overload, but a single Unreachable can still slip through
            // under load. Refusing to swallow non-connect errors keeps
            // real outages visible.
            match fetch_trailing_day(n).await {
                Err(QueryError::Aw(AwError::Unreachable(_))) => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    fetch_trailing_day(n).await
                }
                other => other,
            }
        }));
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

/// Pure-compute KPI synthesis. Takes today's payload + the trailing
/// window directly so the caller controls how the trailing data is
/// fetched (or — in the TUI path — derived from a shared cache slot
/// instead of re-fetched per call).
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

/// One-shot KPI payload: today + trailing-7. Past days are fetched concurrently.
/// Thin orchestrator around `kpi_from_parts` — kept for the non-TUI callers
/// (CLI tools, future external consumers) that want a one-call API.
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

    Ok(kpi_from_parts(&today_events, &today_afk, &past))
}

pub async fn has_web_watcher(client: &AwClient) -> Result<bool, AwError> {
    bucket_prefix_present(client, "aw-watcher-web-", web_watcher_memo()).await
}

pub async fn top_domains(client: &AwClient, range: TimeRange) -> Result<Vec<Value>, AwError> {
    if !has_web_watcher(client).await? {
        return Ok(vec![]);
    }
    let res = client
        .query(&queries::web_top_domains(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

pub async fn top_urls(client: &AwClient, range: TimeRange) -> Result<Vec<Value>, AwError> {
    if !has_web_watcher(client).await? {
        return Ok(vec![]);
    }
    let res = client
        .query(&queries::web_top_urls(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::AfkSummary;

    #[test]
    fn kpi_from_parts_forwards_today_afk_into_summary() {
        // Forwarding pin: `active_secs` and `afk_secs` flow through
        // unchanged; `past_active` slices the AFK active_seconds out of
        // each TrailingDayPayload in order. If kpi_from_parts ever
        // accidentally re-derives these from today's events, the
        // assertion below will catch it.
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

        // Direct passthrough of today's AFK numbers — this is the
        // contract that must not regress when the caller switches from
        // a fresh `trailing_days_past(7)` fetch to a shared cache slot.
        assert_eq!(summary.active_secs, 12_345.0);
        assert_eq!(summary.afk_secs, 678.0);
        // Baseline was computed across all 7 past days (some kpi math
        // may filter, so we don't pin the exact median — just that
        // the baseline was populated from non-empty past data).
        assert!(summary.active_baseline.effective_days > 0);
        assert!(summary.active_baseline.median > 0.0);
    }

    #[tokio::test]
    async fn bucket_prefix_present_returns_memo_without_calling_client() {
        // Pre-seed a local memo. The function must short-circuit on the
        // cached value and never touch the client — proved by passing a
        // client pointed at the default localhost URL and asserting the
        // call resolves synchronously without a Network error.
        let memo: OnceLock<bool> = OnceLock::new();
        let _ = memo.set(true);

        let client = AwClient::new();
        let result = bucket_prefix_present(&client, "aw-watcher-web-", &memo).await;
        assert_eq!(result.unwrap(), true);

        // Confirm a `false` memo also short-circuits.
        let memo_off: OnceLock<bool> = OnceLock::new();
        let _ = memo_off.set(false);
        let result = bucket_prefix_present(&client, "aw-watcher-web-", &memo_off).await;
        assert_eq!(result.unwrap(), false);
    }
}
