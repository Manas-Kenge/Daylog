//! `Cached<T>` — load-bearing data-layer primitive.
//!
//! Each Cached entry is one logical query (e.g. "top apps for today"). It
//! tracks staleness, dedupes concurrent refetches, and implements
//! exponential backoff with an "offline" surfacing flag per decision 1C
//! from the eng review:
//!
//! * 5s → 10s → 20s → 40s → 60s capped on consecutive failures.
//! * Resets to base interval on first success.
//! * `is_offline()` flips true after `OFFLINE_THRESHOLD` consecutive
//!   failures so the footer can render "○ tracker offline".
//!
//! All time inputs are parameterized via `Instant` so tests can inject
//! deterministic clocks instead of sleeping.

use std::time::{Duration, Instant};

use chrono::{Datelike, Local, NaiveDate, Weekday};
use daylog_core::aggregate::{
    fetch_afk_events, parse_categorized_events, summarize_afk, unwrap_first_array,
    CategorizedEvent, CategorySummary, HourBucket,
};
use daylog_core::kpi::KpiSummary;
use daylog_core::queries::TrailingDayPayload;
use daylog_core::time::TimeRange;
use serde_json::Value;

use crate::app::Tab;

/// Live cadence: 5s.
pub const REFRESH_LIVE: Duration = Duration::from_secs(5);

/// Past-day cadence: 5min (past days don't change until midnight).
pub const REFRESH_PAST_DAYS: Duration = Duration::from_secs(5 * 60);

/// After this many consecutive failures, the cache surfaces as offline.
/// Tuned so a single transient blip doesn't flicker the indicator: at
/// the 5s base interval, three failures = ~35s of confirmed unreachable
/// before users see "offline".
pub const OFFLINE_THRESHOLD: u32 = 3;

/// Hard cap on per-entry retry interval. Without a cap, backoff blows
/// past useful refresh rates after ~6 failures.
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// One in-memory cache entry. Generic over the payload so the same logic
/// drives every widget query.
#[derive(Debug)]
pub struct Cached<T> {
    value: Option<T>,
    fetched_at: Option<Instant>,
    in_flight: bool,
    last_error: Option<String>,
    base_interval: Duration,
    consecutive_failures: u32,
}

impl<T> Cached<T> {
    /// Create a new cache entry. `base_interval` is the steady-state
    /// refetch cadence (e.g. 5s for live tab data, 5min for trailing
    /// historical bundles).
    pub fn new(base_interval: Duration) -> Self {
        Self {
            value: None,
            fetched_at: None,
            in_flight: false,
            last_error: None,
            base_interval,
            consecutive_failures: 0,
        }
    }

    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn is_in_flight(&self) -> bool {
        self.in_flight
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Surfaces as offline after OFFLINE_THRESHOLD consecutive failures.
    /// Reset by `apply_success`.
    pub fn is_offline(&self) -> bool {
        self.consecutive_failures >= OFFLINE_THRESHOLD
    }

    /// Time-since-last-fetch threshold. Equals base_interval on success,
    /// doubles per failure, capped at MAX_BACKOFF.
    pub fn current_interval(&self) -> Duration {
        let multiplier = 1_u32 << self.consecutive_failures.min(31);
        let scaled = self.base_interval.saturating_mul(multiplier);
        scaled.min(MAX_BACKOFF)
    }

    /// True if a refetch should fire now. Three rules:
    /// 1. Never refetch while a request is in flight (dedup).
    /// 2. If never fetched, always refetch.
    /// 3. Otherwise, refetch only after current_interval has elapsed.
    pub fn should_refetch(&self, now: Instant) -> bool {
        if self.in_flight {
            return false;
        }
        match self.fetched_at {
            None => true,
            Some(t) => now.duration_since(t) >= self.current_interval(),
        }
    }

    /// Mark a refetch as dispatched. Caller must follow with either
    /// `apply_success` or `apply_failure` once the request resolves.
    pub fn mark_in_flight(&mut self) {
        self.in_flight = true;
    }

    /// Apply a successful fetch result. Resets backoff to base interval
    /// and clears the offline flag.
    pub fn apply_success(&mut self, value: T, now: Instant) {
        self.value = Some(value);
        self.fetched_at = Some(now);
        self.in_flight = false;
        self.last_error = None;
        self.consecutive_failures = 0;
    }

    /// Apply a failed fetch. Bumps the failure counter; backoff doubles
    /// at the next `current_interval` call. Keeps the previous value
    /// visible so widgets show stale data instead of blanks.
    pub fn apply_failure(&mut self, error: String, now: Instant) {
        self.fetched_at = Some(now);
        self.in_flight = false;
        self.last_error = Some(error);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    }
}

/// One row in the Top Apps table. The aw-server query returns a
/// `Vec<serde_json::Value>` of events; we parse each into this struct
/// at fetch time so the render path is allocation-free.
#[derive(Debug, Clone, PartialEq)]
pub struct TopAppRow {
    pub name: String,
    pub duration_secs: f64,
}

impl TopAppRow {
    pub fn parse_many(events: &[Value]) -> Vec<Self> {
        events
            .iter()
            .filter_map(|ev| {
                let name = ev.get("data")?.get("app")?.as_str()?.to_string();
                let duration_secs = ev.get("duration")?.as_f64()?;
                Some(Self {
                    name,
                    duration_secs,
                })
            })
            .collect()
    }
}

/// One row in the Top Domains table. Same shape as TopAppRow but pulls
/// from `data.$domain` (set by aw-watcher-web). When no web watcher is
/// installed the underlying query returns `Ok(vec![])` so an empty cache
/// value is the "no web watcher" signal — distinct from the loading /
/// error states.
#[derive(Debug, Clone, PartialEq)]
pub struct TopDomainRow {
    pub domain: String,
    pub duration_secs: f64,
}

impl TopDomainRow {
    pub fn parse_many(events: &[Value]) -> Vec<Self> {
        events
            .iter()
            .filter_map(|ev| {
                let domain = ev.get("data")?.get("$domain")?.as_str()?.to_string();
                let duration_secs = ev.get("duration")?.as_f64()?;
                Some(Self {
                    domain,
                    duration_secs,
                })
            })
            .collect()
    }
}

/// One day in the calendar-week view (Mon–Sun). Aggregates a day's
/// categorized events into per-root active seconds. `is_future` flags the
/// days that haven't elapsed yet so the renderer paints the axis label
/// without a bar.
#[derive(Debug, Clone, PartialEq)]
pub struct WeekDayBuckets {
    pub date: NaiveDate,
    pub weekday: Weekday,
    pub is_future: bool,
    /// `(category_root, active_seconds)` sorted by ROOT_ORDER then alpha
    /// for stable stacking. Empty when no events.
    pub roots: Vec<(String, f64)>,
    pub total_active_secs: f64,
}

/// Stable display order for category roots. Unlisted roots sort after, alphabetically.
pub const WEEK_ROOT_ORDER: &[&str] = &[
    "Work",
    "Comms",
    "Documents",
    "Browsing",
    "Media",
    "Uncategorized",
];

/// Result message sent back to the App after a fetch resolves. The App
/// matches on the variant to find which `Cached<T>` to update.
#[derive(Debug)]
pub enum FetchResult {
    TopApps(Result<Vec<TopAppRow>, String>),
    Hourly(Result<Vec<HourBucket>, String>),
    TopCategories(Result<Vec<CategorySummary>, String>),
    Kpi(Result<KpiSummary, String>),
    /// Today's categorized events for the 24h timeline widget. Same
    /// data shape `top_categories` is aggregated from, but kept raw so
    /// the bucketize96 step can place each event into 15-min slots.
    TimelineEvents(Result<Vec<CategorizedEvent>, String>),
    /// Top web domains. Empty `Ok(vec![])` is the "no web watcher"
    /// signal — the Rust query short-circuits when no aw-watcher-web-*
    /// bucket is registered.
    TopDomains(Result<Vec<TopDomainRow>, String>),
    /// Calendar-week (Mon → Sun) categorized totals. Always 7 entries.
    /// Future days carry `is_future = true` and empty `roots`.
    Week(Result<Vec<WeekDayBuckets>, String>),
    /// Top apps over the trailing 7 days for the Week tab's lower
    /// desktop-parity rollup row. Scope-fixed; not driven by RangeChip.
    WeekTopApps(Result<Vec<TopAppRow>, String>),
    /// Top categories over the trailing 7 days for the Week tab.
    WeekTopCategories(Result<Vec<CategorySummary>, String>),
    /// Top web domains over the trailing 7 days for the Week tab.
    WeekTopDomains(Result<Vec<TopDomainRow>, String>),
    /// Trailing 365 days of active seconds for the Month tab heatmap.
    /// Index `i` is days_ago = i + 1 (today's value composes from
    /// `kpi.active_secs`). Vec, not [_; 365], because aw-server may
    /// have less than a year of history on fresh installs.
    MonthTrailingYear(Result<Vec<f64>, String>),
    /// Top apps over the trailing 30 days. Independent of the active
    /// `RangeChip`; the Month tab's view is scope-fixed.
    MonthTopApps(Result<Vec<TopAppRow>, String>),
    /// Top categories over the trailing 30 days.
    MonthTopCategories(Result<Vec<CategorySummary>, String>),
    /// Top web domains over the trailing 30 days. Empty `Ok(vec![])`
    /// is again the "no web watcher" signal — same as `TopDomains`.
    MonthTopDomains(Result<Vec<TopDomainRow>, String>),
    /// Shared trailing-7-day payload. Apply derives `week` (paired
    /// with `timeline_events`) from this single fetch. Failure
    /// propagates to dependent slots so the UI shows consistent error
    /// state instead of one panel updating and the others freezing.
    Trailing7(Result<Vec<TrailingDayPayload>, String>),
}

/// In-memory caches keyed per logical query.
#[derive(Debug)]
pub struct DataCache {
    pub top_apps: Cached<Vec<TopAppRow>>,
    pub hourly: Cached<Vec<HourBucket>>,
    pub top_categories: Cached<Vec<CategorySummary>>,
    /// Live KPI summary. One roundtrip bundles today + trailing-7 baselines.
    pub kpi: Cached<KpiSummary>,
    /// Raw categorized events for today; consumed by the 24h timeline.
    pub timeline_events: Cached<Vec<CategorizedEvent>>,
    /// Top web domains for today. Empty value with `last_error == None`
    /// means "no web watcher installed" — render the install hint.
    pub top_domains: Cached<Vec<TopDomainRow>>,
    /// Calendar-week (Mon → Sun) per-day, per-root active seconds for the
    /// Week tab's stacked-bar chart. Past-days cadence; today's column
    /// catches up via the same 5min refresh.
    pub week: Cached<Vec<WeekDayBuckets>>,
    /// Trailing-7-day Top apps for the Week tab's rollup row.
    /// Scope-fixed; not driven by the `RangeChip`.
    pub week_top_apps: Cached<Vec<TopAppRow>>,
    /// Trailing-7-day Top categories for the Week tab.
    pub week_top_categories: Cached<Vec<CategorySummary>>,
    /// Trailing-7-day Top web domains for the Week tab.
    pub week_top_domains: Cached<Vec<TopDomainRow>>,
    /// Trailing-365-day active-seconds-per-day window driving the Month
    /// tab's year heatmap. Heavy first paint (365 fetches) — the
    /// dispatcher gates this slot on `tab == Tab::Month` so Today's
    /// cold-start budget is unaffected.
    pub month_trailing_year: Cached<Vec<f64>>,
    /// Trailing-30-day Top apps for the Month tab's rollup row.
    /// Scope-fixed; not driven by the `RangeChip`.
    pub month_top_apps: Cached<Vec<TopAppRow>>,
    /// Trailing-30-day Top categories for the Month tab.
    pub month_top_categories: Cached<Vec<CategorySummary>>,
    /// Trailing-30-day Top web domains for the Month tab. Empty-Ok
    /// means "no web watcher", same convention as `top_domains`.
    pub month_top_domains: Cached<Vec<TopDomainRow>>,
    /// Shared source of truth for the trailing-7-day window. Before
    /// this slot existed, `kpi` and `week` each dispatched their own
    /// `trailing_days_past(7)` fan-out on every refresh — duplicate
    /// 14-HTTP fan-outs through a 2-wide semaphore. Now both derive
    /// from this one cache.
    pub trailing_7: Cached<Vec<TrailingDayPayload>>,
}

impl DataCache {
    pub fn new() -> Self {
        Self {
            top_apps: Cached::new(REFRESH_LIVE),
            hourly: Cached::new(REFRESH_LIVE),
            top_categories: Cached::new(REFRESH_LIVE),
            kpi: Cached::new(REFRESH_LIVE),
            timeline_events: Cached::new(REFRESH_LIVE),
            top_domains: Cached::new(REFRESH_LIVE),
            week: Cached::new(REFRESH_PAST_DAYS),
            week_top_apps: Cached::new(REFRESH_PAST_DAYS),
            week_top_categories: Cached::new(REFRESH_PAST_DAYS),
            week_top_domains: Cached::new(REFRESH_PAST_DAYS),
            month_trailing_year: Cached::new(REFRESH_PAST_DAYS),
            month_top_apps: Cached::new(REFRESH_PAST_DAYS),
            month_top_categories: Cached::new(REFRESH_PAST_DAYS),
            month_top_domains: Cached::new(REFRESH_PAST_DAYS),
            trailing_7: Cached::new(REFRESH_PAST_DAYS),
        }
    }

    /// True if any tracked LIVE cache has crossed the offline threshold.
    /// Drives the footer's "○ tracker offline" indicator. The trailing
    /// past-days slot is excluded — its 5min cadence would create false
    /// positives during transient blips. `top_domains` is also excluded
    /// because an empty-result is its normal state on machines without
    /// a web watcher.
    pub fn any_offline(&self) -> bool {
        self.top_apps.is_offline()
            || self.hourly.is_offline()
            || self.top_categories.is_offline()
            || self.kpi.is_offline()
            || self.timeline_events.is_offline()
    }

    /// Apply an incoming fetch result to the matching cache entry.
    pub fn apply(&mut self, msg: FetchResult, now: Instant) {
        match msg {
            FetchResult::TopApps(Ok(v)) => self.top_apps.apply_success(v, now),
            FetchResult::TopApps(Err(e)) => self.top_apps.apply_failure(e, now),
            FetchResult::Hourly(Ok(v)) => self.hourly.apply_success(v, now),
            FetchResult::Hourly(Err(e)) => self.hourly.apply_failure(e, now),
            FetchResult::TopCategories(Ok(v)) => self.top_categories.apply_success(v, now),
            FetchResult::TopCategories(Err(e)) => self.top_categories.apply_failure(e, now),
            FetchResult::Kpi(Ok(v)) => self.kpi.apply_success(v, now),
            FetchResult::Kpi(Err(e)) => self.kpi.apply_failure(e, now),
            FetchResult::TimelineEvents(Ok(v)) => {
                self.timeline_events.apply_success(v, now);
                self.try_rebuild_week(now);
            }
            FetchResult::TimelineEvents(Err(e)) => self.timeline_events.apply_failure(e, now),
            FetchResult::TopDomains(Ok(v)) => self.top_domains.apply_success(v, now),
            FetchResult::TopDomains(Err(e)) => self.top_domains.apply_failure(e, now),
            FetchResult::Week(Ok(v)) => self.week.apply_success(v, now),
            FetchResult::Week(Err(e)) => self.week.apply_failure(e, now),
            FetchResult::WeekTopApps(Ok(v)) => self.week_top_apps.apply_success(v, now),
            FetchResult::WeekTopApps(Err(e)) => self.week_top_apps.apply_failure(e, now),
            FetchResult::WeekTopCategories(Ok(v)) => self.week_top_categories.apply_success(v, now),
            FetchResult::WeekTopCategories(Err(e)) => {
                self.week_top_categories.apply_failure(e, now)
            }
            FetchResult::WeekTopDomains(Ok(v)) => self.week_top_domains.apply_success(v, now),
            FetchResult::WeekTopDomains(Err(e)) => self.week_top_domains.apply_failure(e, now),
            FetchResult::MonthTrailingYear(Ok(v)) => self.month_trailing_year.apply_success(v, now),
            FetchResult::MonthTrailingYear(Err(e)) => {
                self.month_trailing_year.apply_failure(e, now)
            }
            FetchResult::MonthTopApps(Ok(v)) => self.month_top_apps.apply_success(v, now),
            FetchResult::MonthTopApps(Err(e)) => self.month_top_apps.apply_failure(e, now),
            FetchResult::MonthTopCategories(Ok(v)) => {
                self.month_top_categories.apply_success(v, now)
            }
            FetchResult::MonthTopCategories(Err(e)) => {
                self.month_top_categories.apply_failure(e, now)
            }
            FetchResult::MonthTopDomains(Ok(v)) => self.month_top_domains.apply_success(v, now),
            FetchResult::MonthTopDomains(Err(e)) => self.month_top_domains.apply_failure(e, now),
            FetchResult::Trailing7(Ok(v)) => {
                self.trailing_7.apply_success(v, now);
                self.try_rebuild_week(now);
            }
            FetchResult::Trailing7(Err(e)) => {
                // Propagate the failure to every dependent slot so the
                // UI shows consistent error state. `Cached::apply_failure`
                // preserves the previous value (the week chart freezes
                // at last good values, not blank).
                self.trailing_7.apply_failure(e.clone(), now);
                self.week.apply_failure(e, now);
            }
        }
    }

    /// Rebuild the calendar-week chart when both inputs are available.
    /// Early-return if either is missing; the next `apply` for the
    /// missing piece will retry.
    fn try_rebuild_week(&mut self, now: Instant) {
        let (Some(past), Some(today_events)) =
            (self.trailing_7.value(), self.timeline_events.value())
        else {
            return;
        };
        let today = Local::now().date_naive();
        let weeks = build_week_buckets(today, today_events, past);
        self.week.apply_success(weeks, now);
    }
}

impl Default for DataCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawn fetches for any stale, not-in-flight cache. Each fetch resolves
/// into a FetchResult sent back over `tx`.
pub fn dispatch_refetches(
    cache: &mut DataCache,
    range: TimeRange,
    tab: Tab,
    tx: &tokio::sync::mpsc::UnboundedSender<FetchResult>,
    now: Instant,
) {
    if cache.top_apps.should_refetch(now) {
        cache.top_apps.mark_in_flight();
        let tx = tx.clone();
        let range = range.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result = daylog_core::queries::top_apps(&client, range)
                .await
                .map(|raw| TopAppRow::parse_many(&raw))
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::TopApps(result));
        });
    }

    if cache.hourly.should_refetch(now) {
        cache.hourly.mark_in_flight();
        let tx = tx.clone();
        let range = range.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result = daylog_core::queries::hourly(&client, range)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::Hourly(result));
        });
    }

    if cache.top_categories.should_refetch(now) {
        cache.top_categories.mark_in_flight();
        let tx = tx.clone();
        let range = range.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result = daylog_core::queries::top_categories(&client, range)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::TopCategories(result));
        });
    }

    // Shared trailing-7-day fetch. Drives the kpi baselines (cloned
    // into the kpi task below) and the week chart (paired with
    // `timeline_events` in `try_rebuild_week`). Previously each
    // consumer fanned out its own copy, paying the 14-HTTP cost twice
    // on cold start.
    if cache.trailing_7.should_refetch(now) {
        cache.trailing_7.mark_in_flight();
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = daylog_core::queries::trailing_days_past(7)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::Trailing7(result));
        });
    }

    // kpi waits on the shared trailing_7 cache to be primed. Until then,
    // baselines and pattern-shift can't be computed. Once primed, each
    // kpi tick fetches only today's slice (events + AFK) and synthesizes
    // locally via kpi_from_parts — no trailing re-fetch on the 5s cadence.
    if cache.kpi.should_refetch(now) {
        if let Some(trailing) = cache.trailing_7.value() {
            cache.kpi.mark_in_flight();
            let tx = tx.clone();
            let range = range.clone();
            let trailing = trailing.clone();
            tokio::spawn(async move {
                let client = daylog_core::aw_client::AwClient::new();
                let cfg = match daylog_core::categories::load(&client).await {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(FetchResult::Kpi(Err(e.to_string())));
                        return;
                    }
                };
                let classes_json = daylog_core::categories::classes_to_aql(&cfg);
                let timeperiods = [range.as_aw_timeperiod()];
                let aql = daylog_core::aw_client::queries::categorized_events(&classes_json);
                let (today_events_res, today_afk_events_res) = tokio::join!(
                    client.query(&aql, &timeperiods),
                    fetch_afk_events(&client, &range),
                );
                let today_events = match today_events_res {
                    Ok(r) => parse_categorized_events(&unwrap_first_array(r)),
                    Err(e) => {
                        let _ = tx.send(FetchResult::Kpi(Err(e.to_string())));
                        return;
                    }
                };
                let today_afk_events = match today_afk_events_res {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(FetchResult::Kpi(Err(e.to_string())));
                        return;
                    }
                };
                let today_afk = summarize_afk(&today_afk_events, false);
                let summary = daylog_core::queries::kpi_from_parts(
                    &today_events,
                    &today_afk,
                    &trailing,
                );
                let _ = tx.send(FetchResult::Kpi(Ok(summary)));
            });
        }
    }

    if cache.timeline_events.should_refetch(now) {
        cache.timeline_events.mark_in_flight();
        let tx = tx.clone();
        let range = range.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result = daylog_core::queries::categorized_events(&client, range)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::TimelineEvents(result));
        });
    }

    if cache.top_domains.should_refetch(now) {
        cache.top_domains.mark_in_flight();
        let tx = tx.clone();
        let range = range.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result = daylog_core::queries::top_domains(&client, range)
                .await
                .map(|raw| TopDomainRow::parse_many(&raw))
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::TopDomains(result));
        });
    }

    // Week rollups mirror the desktop page's fixed Last 7 Days panels.
    // They do not follow the active range chip. The main `week` slot
    // (calendar-week stacked bars) is no longer dispatched here — it's
    // derived in `DataCache::apply` from the shared `trailing_7` cache
    // paired with today's `timeline_events`, which fire on every tab.
    if tab == Tab::Week {
        if cache.week_top_apps.should_refetch(now) {
            cache.week_top_apps.mark_in_flight();
            let tx = tx.clone();
            tokio::spawn(async move {
                let client = daylog_core::aw_client::AwClient::new();
                let result =
                    daylog_core::queries::top_apps(&client, TimeRange::LastNDays { days: 7 })
                        .await
                        .map(|raw| TopAppRow::parse_many(&raw))
                        .map_err(|e| e.to_string());
                let _ = tx.send(FetchResult::WeekTopApps(result));
            });
        }

        if cache.week_top_categories.should_refetch(now) {
            cache.week_top_categories.mark_in_flight();
            let tx = tx.clone();
            tokio::spawn(async move {
                let client = daylog_core::aw_client::AwClient::new();
                let result =
                    daylog_core::queries::top_categories(&client, TimeRange::LastNDays { days: 7 })
                        .await
                        .map_err(|e| e.to_string());
                let _ = tx.send(FetchResult::WeekTopCategories(result));
            });
        }

        if cache.week_top_domains.should_refetch(now) {
            cache.week_top_domains.mark_in_flight();
            let tx = tx.clone();
            tokio::spawn(async move {
                let client = daylog_core::aw_client::AwClient::new();
                let result =
                    daylog_core::queries::top_domains(&client, TimeRange::LastNDays { days: 7 })
                        .await
                        .map(|raw| TopDomainRow::parse_many(&raw))
                        .map_err(|e| e.to_string());
                let _ = tx.send(FetchResult::WeekTopDomains(result));
            });
        }
    }

    // Month-tab fetches are gated on the active tab so Today's
    // cold-start budget isn't taxed by the 365-day fan-out. Bouncing
    // back to Today doesn't invalidate already-fetched month caches —
    // the gate only suppresses *new* dispatches, not in-flight or
    // cached values.
    if tab != Tab::Month {
        return;
    }

    if cache.month_trailing_year.should_refetch(now) {
        cache.month_trailing_year.mark_in_flight();
        let tx = tx.clone();
        tokio::spawn(async move {
            // 365 concurrent per-day fetches under the hood. First paint
            // is the dominant cost; staleness is 5min thereafter.
            let result = daylog_core::queries::trailing_days_past(365)
                .await
                .map(|days| {
                    let mut out = vec![0.0_f64; 365];
                    for d in days {
                        let idx = d.days_ago.saturating_sub(1) as usize;
                        if idx < out.len() {
                            out[idx] = d.afk.active_seconds;
                        }
                    }
                    out
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::MonthTrailingYear(result));
        });
    }

    if cache.month_top_apps.should_refetch(now) {
        cache.month_top_apps.mark_in_flight();
        let tx = tx.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result = daylog_core::queries::top_apps(&client, TimeRange::LastNDays { days: 30 })
                .await
                .map(|raw| TopAppRow::parse_many(&raw))
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::MonthTopApps(result));
        });
    }

    if cache.month_top_categories.should_refetch(now) {
        cache.month_top_categories.mark_in_flight();
        let tx = tx.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result =
                daylog_core::queries::top_categories(&client, TimeRange::LastNDays { days: 30 })
                    .await
                    .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::MonthTopCategories(result));
        });
    }

    if cache.month_top_domains.should_refetch(now) {
        cache.month_top_domains.mark_in_flight();
        let tx = tx.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result =
                daylog_core::queries::top_domains(&client, TimeRange::LastNDays { days: 30 })
                    .await
                    .map(|raw| TopDomainRow::parse_many(&raw))
                    .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::MonthTopDomains(result));
        });
    }
}

/// ISO Monday of the calendar week containing `today`.
pub fn iso_monday(today: NaiveDate) -> NaiveDate {
    let days_from_monday = today.weekday().num_days_from_monday() as i64;
    today - chrono::Duration::days(days_from_monday)
}

/// Build the 7-day Mon → Sun calendar week from a today-slice and the
/// trailing-7 past-day payloads. Days strictly after `today` carry
/// `is_future = true` and empty roots.
pub fn build_week_buckets(
    today: NaiveDate,
    today_events: &[CategorizedEvent],
    past: &[daylog_core::queries::TrailingDayPayload],
) -> Vec<WeekDayBuckets> {
    let monday = iso_monday(today);
    (0..7)
        .map(|i| {
            let date = monday + chrono::Duration::days(i);
            if date > today {
                return WeekDayBuckets {
                    date,
                    weekday: date.weekday(),
                    is_future: true,
                    roots: Vec::new(),
                    total_active_secs: 0.0,
                };
            }
            let roots = if date == today {
                bucketize_roots(today_events)
            } else {
                let days_ago = (today - date).num_days() as u32;
                past.iter()
                    .find(|d| d.days_ago == days_ago)
                    .map(|d| bucketize_roots(&d.events))
                    .unwrap_or_default()
            };
            let total_active_secs = roots.iter().map(|(_, s)| *s).sum();
            WeekDayBuckets {
                date,
                weekday: date.weekday(),
                is_future: false,
                roots,
                total_active_secs,
            }
        })
        .collect()
}

/// Group categorized events by their root category (`category[0]`,
/// defaulting to "Uncategorized" — the parser already enforces this) and
/// sum durations. Output is sorted by `WEEK_ROOT_ORDER` first, then any
/// other roots alphabetically. Sort key is stable across days so segment
/// stacks line up visually.
fn bucketize_roots(events: &[CategorizedEvent]) -> Vec<(String, f64)> {
    let mut totals: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for ev in events {
        let root = ev
            .category
            .first()
            .cloned()
            .unwrap_or_else(|| "Uncategorized".to_string());
        *totals.entry(root).or_insert(0.0) += ev.duration;
    }
    let mut out: Vec<(String, f64)> = totals.into_iter().collect();
    out.sort_by(|a, b| {
        let ai = WEEK_ROOT_ORDER.iter().position(|r| *r == a.0);
        let bi = WEEK_ROOT_ORDER.iter().position(|r| *r == b.0);
        match (ai, bi) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.0.cmp(&b.0),
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Tab;
    use tokio::sync::mpsc;

    fn t0() -> Instant {
        Instant::now()
    }

    /// `should_refetch` returns true for an untouched slot. After
    /// `dispatch_refetches` it's false either because the task is in
    /// flight or has already resolved — both mean "dispatched". So we
    /// use `!should_refetch(now)` as a synchronous "was dispatched" probe.
    fn was_dispatched<T>(c: &Cached<T>, now: Instant) -> bool {
        !c.should_refetch(now)
    }

    #[tokio::test]
    async fn dispatch_refetches_today_fires_shared_slots_not_week_rollups() {
        let mut cache = DataCache::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let now = Instant::now();
        dispatch_refetches(&mut cache, TimeRange::Today, Tab::Today, &tx, now);

        // Live Today slots fire.
        assert!(was_dispatched(&cache.top_apps, now));
        assert!(was_dispatched(&cache.timeline_events, now));

        // The shared trailing-7 slot fires on Today too — the kpi
        // baselines need it. This is the *deduplication* win: one
        // fetch instead of two.
        assert!(
            was_dispatched(&cache.trailing_7, now),
            "trailing_7 is the single source of truth for kpi/week and must fire on Today"
        );

        // The week chart itself (`cache.week`) is no longer dispatched —
        // it's derived in apply() from trailing_7 + timeline_events.
        // The Week-tab rollup tables stay tab-gated.
        assert!(!was_dispatched(&cache.week_top_apps, now));
        assert!(!was_dispatched(&cache.week_top_categories, now));
        assert!(!was_dispatched(&cache.week_top_domains, now));

        // Month already gated; pin it for symmetry.
        assert!(!was_dispatched(&cache.month_trailing_year, now));
        assert!(!was_dispatched(&cache.month_top_apps, now));
    }

    #[tokio::test]
    async fn dispatch_refetches_week_fires_week_rollups() {
        let mut cache = DataCache::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let now = Instant::now();
        dispatch_refetches(&mut cache, TimeRange::Today, Tab::Week, &tx, now);

        // Week rollups fire only on Week.
        assert!(was_dispatched(&cache.week_top_apps, now));
        assert!(was_dispatched(&cache.week_top_categories, now));
        assert!(was_dispatched(&cache.week_top_domains, now));

        // Shared trailing_7 fires here too — same single-source path.
        assert!(was_dispatched(&cache.trailing_7, now));

        // Month still gated even from Week.
        assert!(!was_dispatched(&cache.month_trailing_year, now));
    }

    #[tokio::test]
    async fn dispatch_refetches_month_fires_month_slots() {
        let mut cache = DataCache::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let now = Instant::now();
        dispatch_refetches(&mut cache, TimeRange::Today, Tab::Month, &tx, now);

        assert!(was_dispatched(&cache.month_trailing_year, now));
        assert!(was_dispatched(&cache.month_top_apps, now));
        assert!(was_dispatched(&cache.month_top_categories, now));
        assert!(was_dispatched(&cache.month_top_domains, now));
    }

    fn after(start: Instant, secs: u64) -> Instant {
        start + Duration::from_secs(secs)
    }

    #[test]
    fn cached_t_starts_empty_and_should_refetch() {
        let c: Cached<u32> = Cached::new(Duration::from_secs(5));
        assert!(c.value().is_none());
        assert!(!c.is_in_flight());
        assert!(!c.is_offline());
        assert!(
            c.should_refetch(t0()),
            "fresh cache should always refetch first"
        );
    }

    #[test]
    fn cached_t_dedupes_inflight_refetch() {
        let mut c: Cached<u32> = Cached::new(Duration::from_secs(5));
        c.mark_in_flight();
        assert!(
            !c.should_refetch(t0()),
            "in-flight cache must not dispatch another refetch"
        );
    }

    #[test]
    fn cached_t_respects_staleness() {
        let mut c: Cached<u32> = Cached::new(Duration::from_secs(5));
        let start = t0();
        c.apply_success(7, start);
        assert!(!c.should_refetch(start), "just-fetched cache is fresh");
        assert!(
            !c.should_refetch(after(start, 4)),
            "4s after a 5s-interval fetch is still fresh"
        );
        assert!(
            c.should_refetch(after(start, 5)),
            "5s after a 5s-interval fetch is stale"
        );
        assert!(c.should_refetch(after(start, 999)));
    }

    #[test]
    fn cached_t_backoff_doubles_on_failure() {
        let mut c: Cached<u32> = Cached::new(Duration::from_secs(5));
        let now = t0();
        // 0 failures → 5s
        assert_eq!(c.current_interval(), Duration::from_secs(5));
        c.apply_failure("boom".into(), now);
        // 1 failure → 10s
        assert_eq!(c.current_interval(), Duration::from_secs(10));
        c.apply_failure("boom".into(), now);
        // 2 failures → 20s
        assert_eq!(c.current_interval(), Duration::from_secs(20));
        c.apply_failure("boom".into(), now);
        // 3 failures → 40s
        assert_eq!(c.current_interval(), Duration::from_secs(40));
    }

    #[test]
    fn cached_t_backoff_resets_on_success() {
        let mut c: Cached<u32> = Cached::new(Duration::from_secs(5));
        let now = t0();
        c.apply_failure("a".into(), now);
        c.apply_failure("b".into(), now);
        c.apply_failure("c".into(), now);
        assert_eq!(c.current_interval(), Duration::from_secs(40));
        assert!(c.is_offline(), "3 failures crosses the offline threshold");

        c.apply_success(42, now);
        assert_eq!(
            c.current_interval(),
            Duration::from_secs(5),
            "success resets backoff to base"
        );
        assert!(!c.is_offline(), "success clears the offline flag");
        assert_eq!(c.value().copied(), Some(42));
        assert!(c.last_error().is_none());
    }

    #[test]
    fn cached_t_max_backoff_capped() {
        let mut c: Cached<u32> = Cached::new(Duration::from_secs(5));
        let now = t0();
        // 5s base × 2^4 = 80s, but capped at MAX_BACKOFF (60s).
        for _ in 0..10 {
            c.apply_failure("repeated failure".into(), now);
        }
        assert_eq!(
            c.current_interval(),
            MAX_BACKOFF,
            "extreme failure counts must clamp at MAX_BACKOFF"
        );
    }

    #[test]
    fn cached_t_offline_flag_after_n_failures() {
        let mut c: Cached<u32> = Cached::new(Duration::from_secs(5));
        let now = t0();
        assert!(!c.is_offline());
        c.apply_failure("1".into(), now);
        assert!(!c.is_offline(), "1 failure isn't yet offline");
        c.apply_failure("2".into(), now);
        assert!(!c.is_offline(), "2 failures isn't yet offline");
        c.apply_failure("3".into(), now);
        assert!(c.is_offline(), "3rd failure crosses threshold");
    }

    #[test]
    fn cached_t_keeps_stale_value_through_failures() {
        // Widgets should render previous values during transient
        // network blips instead of blanking out.
        let mut c: Cached<&'static str> = Cached::new(Duration::from_secs(5));
        let now = t0();
        c.apply_success("initial", now);
        c.apply_failure("blip".into(), now);
        c.apply_failure("blip".into(), now);
        assert_eq!(
            c.value().copied(),
            Some("initial"),
            "stale value remains visible during failure streak"
        );
        assert_eq!(c.last_error(), Some("blip"));
    }

    #[test]
    fn data_cache_starts_with_no_values() {
        let dc = DataCache::new();
        assert!(dc.top_apps.value().is_none());
        assert!(dc.hourly.value().is_none());
        assert!(dc.top_categories.value().is_none());
        assert!(dc.kpi.value().is_none());
        assert!(!dc.any_offline());
    }

    #[test]
    fn data_cache_kpi_offline_flips_aggregate_flag() {
        let mut dc = DataCache::new();
        let now = Instant::now();
        for _ in 0..OFFLINE_THRESHOLD {
            dc.kpi.apply_failure("err".into(), now);
        }
        assert!(
            dc.any_offline(),
            "kpi failures must flip the aggregate flag"
        );
    }

    fn fake_trailing(days: u32) -> Vec<TrailingDayPayload> {
        (1..=days)
            .map(|n| TrailingDayPayload {
                days_ago: n,
                events: Vec::new(),
                afk: daylog_core::aggregate::AfkSummary {
                    active_seconds: 100.0 * n as f64,
                    afk_seconds: 0.0,
                    active_ratio: 1.0,
                    intervals: Vec::new(),
                },
            })
            .collect()
    }

    #[test]
    fn trailing_7_failure_propagates_to_dependents() {
        // Trailing7 failure surfaces on every consumer. Stale values
        // stay visible per Cached's contract; only last_error flips.
        let mut dc = DataCache::new();
        let now = Instant::now();
        // Seed prior successful values.
        dc.trailing_7
            .apply_success(fake_trailing(7), now);
        dc.week.apply_success(Vec::new(), now);

        dc.apply(FetchResult::Trailing7(Err("aw down".into())), now);

        // Values stay (stale-during-blip contract); errors flip on.
        assert!(dc.trailing_7.value().is_some());
        assert_eq!(dc.trailing_7.last_error(), Some("aw down"));
        assert_eq!(dc.week.last_error(), Some("aw down"));
    }

    #[test]
    fn try_rebuild_week_needs_both_inputs() {
        // T5 — week derivation only fires once trailing_7 AND
        // timeline_events have both landed. Either alone leaves week
        // unset.
        let mut dc = DataCache::new();
        let now = Instant::now();

        // Only trailing_7: week stays unset.
        dc.apply(FetchResult::Trailing7(Ok(fake_trailing(7))), now);
        assert!(
            dc.week.value().is_none(),
            "week must wait for timeline_events; trailing_7 alone is insufficient"
        );

        // Adding timeline_events triggers the rebuild.
        let today_events = vec![ev(&["Work"], 1800.0)];
        dc.apply(FetchResult::TimelineEvents(Ok(today_events)), now);
        let week = dc.week.value().expect("week derived once both inputs present");
        assert_eq!(week.len(), 7);
    }

    #[tokio::test]
    async fn dispatch_refetches_does_not_redispatch_trailing_7_when_fresh() {
        // T7 — Cached's interval gate must keep trailing_7 from
        // re-firing on every tick. Without this the 5min cadence
        // collapses to per-tick.
        let mut cache = DataCache::new();
        let now = Instant::now();
        cache
            .trailing_7
            .apply_success(fake_trailing(7), now);

        let (tx, _rx) = mpsc::unbounded_channel();
        dispatch_refetches(&mut cache, TimeRange::Today, Tab::Today, &tx, now);

        // trailing_7's fetched_at == now; interval is 5min; should_refetch == false.
        assert!(
            !cache.trailing_7.should_refetch(now),
            "fresh trailing_7 must not redispatch within its interval"
        );
    }

    #[test]
    fn trailing_7_offline_does_not_flip_aggregate_flag() {
        // Same precedent as `week`: 5min cadence slots blipping
        // shouldn't flicker the footer indicator.
        let mut dc = DataCache::new();
        let now = Instant::now();
        for _ in 0..(OFFLINE_THRESHOLD + 2) {
            dc.trailing_7.apply_failure("err".into(), now);
        }
        assert!(
            !dc.any_offline(),
            "trailing_7 failures alone must not flag the tracker offline"
        );
    }

    #[test]
    fn data_cache_apply_routes_kpi_result() {
        let mut dc = DataCache::new();
        let now = Instant::now();
        let summary = daylog_core::kpi::KpiSummary {
            active_secs: 1234.0,
            afk_secs: 100.0,
            active_ratio: 1234.0 / 1334.0,
            longest_stretch: None,
            best_window: None,
            pattern_shift: None,
            focus_by_hour: [0.0; 24],
            active_baseline: daylog_core::kpi::BaselineStats {
                effective_days: 0,
                median: 0.0,
                mean: 0.0,
                stdev: 0.0,
            },
            longest_baseline: daylog_core::kpi::BaselineStats {
                effective_days: 0,
                median: 0.0,
                mean: 0.0,
                stdev: 0.0,
            },
            best_window_baseline: daylog_core::kpi::BaselineStats {
                effective_days: 0,
                median: 0.0,
                mean: 0.0,
                stdev: 0.0,
            },
        };
        dc.apply(FetchResult::Kpi(Ok(summary.clone())), now);
        assert_eq!(dc.kpi.value().map(|s| s.active_secs), Some(1234.0));
    }

    #[test]
    fn data_cache_any_offline_aggregates_individual_states() {
        let mut dc = DataCache::new();
        let now = Instant::now();
        for _ in 0..OFFLINE_THRESHOLD {
            dc.top_apps.apply_failure("err".into(), now);
        }
        assert!(
            dc.any_offline(),
            "one offline cache flips the aggregate flag"
        );
    }

    #[test]
    fn data_cache_week_offline_does_not_flip_aggregate_flag() {
        // Week runs at 5min cadence. Surfacing offline on its blips would
        // create false positives — same precedent as `trailing_7`.
        let mut dc = DataCache::new();
        let now = Instant::now();
        for _ in 0..(OFFLINE_THRESHOLD + 2) {
            dc.week.apply_failure("err".into(), now);
        }
        assert!(
            !dc.any_offline(),
            "week failures alone must not flag the tracker offline"
        );
    }

    #[test]
    fn data_cache_apply_routes_week_results() {
        let mut dc = DataCache::new();
        let now = Instant::now();
        let week = vec![WeekDayBuckets {
            date: NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
            weekday: Weekday::Mon,
            is_future: false,
            roots: vec![("Work".into(), 3600.0)],
            total_active_secs: 3600.0,
        }];
        dc.apply(FetchResult::Week(Ok(week.clone())), now);
        assert_eq!(dc.week.value(), Some(&week));
    }

    #[test]
    fn iso_monday_for_known_dates() {
        // Wednesday May 6, 2026 → Monday May 4, 2026.
        let wed = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let mon = NaiveDate::from_ymd_opt(2026, 5, 4).unwrap();
        assert_eq!(iso_monday(wed), mon);
        // Sunday lands back on the previous Monday (ISO week starts on Mon).
        let sun = NaiveDate::from_ymd_opt(2026, 5, 10).unwrap();
        assert_eq!(iso_monday(sun), mon);
        // Monday is a fixed point.
        assert_eq!(iso_monday(mon), mon);
    }

    fn ev(cat: &[&str], duration: f64) -> CategorizedEvent {
        CategorizedEvent {
            timestamp: chrono::Utc::now(),
            duration,
            data: serde_json::Value::Null,
            category: cat.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn build_week_marks_future_days() {
        // "Today" is Wed May 6; Mon-Tue are past, Wed is today, Thu-Sun are future.
        let today = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let past = vec![
            daylog_core::queries::TrailingDayPayload {
                days_ago: 1,
                events: vec![ev(&["Work"], 7200.0)],
                afk: daylog_core::aggregate::AfkSummary {
                    active_seconds: 7200.0,
                    afk_seconds: 0.0,
                    active_ratio: 1.0,
                    intervals: Vec::new(),
                },
            },
            daylog_core::queries::TrailingDayPayload {
                days_ago: 2,
                events: vec![ev(&["Browsing"], 3600.0)],
                afk: daylog_core::aggregate::AfkSummary {
                    active_seconds: 3600.0,
                    afk_seconds: 0.0,
                    active_ratio: 1.0,
                    intervals: Vec::new(),
                },
            },
        ];
        let today_events = vec![ev(&["Comms"], 1800.0)];
        let week = build_week_buckets(today, &today_events, &past);
        assert_eq!(week.len(), 7);
        assert_eq!(week[0].date, NaiveDate::from_ymd_opt(2026, 5, 4).unwrap());
        assert!(!week[0].is_future);
        assert_eq!(week[0].roots, vec![("Browsing".to_string(), 3600.0)]);
        assert!(!week[1].is_future);
        assert_eq!(week[1].roots, vec![("Work".to_string(), 7200.0)]);
        assert!(!week[2].is_future);
        assert_eq!(week[2].roots, vec![("Comms".to_string(), 1800.0)]);
        for i in 3..7 {
            assert!(week[i].is_future, "day index {i} must be future");
            assert!(week[i].roots.is_empty());
            assert_eq!(week[i].total_active_secs, 0.0);
        }
    }

    #[test]
    fn build_week_orders_roots_by_week_root_order() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).unwrap();
        // Today has Browsing, Work, ZZUnknown — expect ROOT_ORDER then alpha.
        let today_events = vec![
            ev(&["Browsing"], 1000.0),
            ev(&["Work", "Programming"], 2000.0),
            ev(&["ZZUnknown"], 500.0),
        ];
        let week = build_week_buckets(today, &today_events, &[]);
        let today_idx = week.iter().position(|d| !d.is_future).unwrap_or(0);
        let last_past = week.iter().rposition(|d| !d.is_future).unwrap();
        let today_row = &week[last_past];
        assert!(today_row.date == today);
        let names: Vec<&str> = today_row.roots.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["Work", "Browsing", "ZZUnknown"]);
        let _ = today_idx;
    }

    #[test]
    fn cached_t_in_flight_clears_on_apply() {
        let mut c: Cached<u32> = Cached::new(Duration::from_secs(5));
        let now = t0();
        c.mark_in_flight();
        assert!(c.is_in_flight());
        c.apply_success(1, now);
        assert!(!c.is_in_flight(), "success clears in_flight");

        c.mark_in_flight();
        c.apply_failure("x".into(), now);
        assert!(!c.is_in_flight(), "failure also clears in_flight");
    }
}
