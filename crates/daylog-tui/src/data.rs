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

use daylog_core::aggregate::{CategorizedEvent, CategorySummary, HourBucket};
use daylog_core::kpi::KpiSummary;
use daylog_core::time::TimeRange;
use serde_json::Value;

use crate::app::Tab;

/// Live cadence for today's slot — matches `useAw.ts` REFRESH_MS = 5_000.
pub const REFRESH_LIVE: Duration = Duration::from_secs(5);

/// Cadence for past-day data. Matches desktop's `PAST_DAY_STALE_MS`.
/// Past days don't change until midnight; a 5min staleness check is
/// effectively cached-for-the-day after the first paint.
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

/// Result message sent back to the App after a fetch resolves. The App
/// matches on the variant to find which `Cached<T>` to update.
#[derive(Debug)]
pub enum FetchResult {
    TopApps(Result<Vec<TopAppRow>, String>),
    Hourly(Result<Vec<HourBucket>, String>),
    TopCategories(Result<Vec<CategorySummary>, String>),
    Kpi(Result<KpiSummary, String>),
    /// Past 7 days of active seconds. Index `i` is days_ago = i + 1, so
    /// index 0 = yesterday, index 6 = 7 days ago. The sparkline widget
    /// composes this with today's `kpi.active_secs` at render time.
    TrailingActive(Result<[f64; 7], String>),
    /// Today's categorized events for the 24h timeline widget. Same
    /// data shape `top_categories` is aggregated from, but kept raw so
    /// the bucketize96 step can place each event into 15-min slots.
    TimelineEvents(Result<Vec<CategorizedEvent>, String>),
    /// Top web domains. Empty `Ok(vec![])` is the "no web watcher"
    /// signal — the Rust query short-circuits when no aw-watcher-web-*
    /// bucket is registered.
    TopDomains(Result<Vec<TopDomainRow>, String>),
    /// Trailing 365 days of active seconds for the Month tab heatmap.
    /// Index `i` is days_ago = i + 1 (matches `TrailingActive`'s
    /// convention so today's value composes from `kpi.active_secs`).
    /// Vec, not [_; 365], because aw-server may have less than a year
    /// of history on fresh installs.
    MonthTrailingYear(Result<Vec<f64>, String>),
    /// Top apps over the trailing 30 days. Independent of the active
    /// `RangeChip`; the Month tab's view is scope-fixed.
    MonthTopApps(Result<Vec<TopAppRow>, String>),
    /// Top categories over the trailing 30 days.
    MonthTopCategories(Result<Vec<CategorySummary>, String>),
    /// Top web domains over the trailing 30 days. Empty `Ok(vec![])`
    /// is again the "no web watcher" signal — same as `TopDomains`.
    MonthTopDomains(Result<Vec<TopDomainRow>, String>),
}

/// Bundle of every cache entry the Today tab reads. Future tabs add
/// their own fields to this struct.
#[derive(Debug)]
pub struct DataCache {
    pub top_apps: Cached<Vec<TopAppRow>>,
    pub hourly: Cached<Vec<HourBucket>>,
    pub top_categories: Cached<Vec<CategorySummary>>,
    /// Live KPI summary backing the compact strip (Active · Longest ·
    /// pattern shift). One IPC roundtrip pulls today + trailing-7
    /// baselines so the strip never needs to recompose.
    pub kpi: Cached<KpiSummary>,
    /// Past-7-day active seconds for the sparkline. 5min cadence —
    /// past days don't change within a day.
    pub trailing_active: Cached<[f64; 7]>,
    /// Raw categorized events for today; consumed by the 24h timeline.
    pub timeline_events: Cached<Vec<CategorizedEvent>>,
    /// Top web domains for today. Empty value with `last_error == None`
    /// means "no web watcher installed" — render the install hint.
    pub top_domains: Cached<Vec<TopDomainRow>>,
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
}

impl DataCache {
    pub fn new() -> Self {
        Self {
            top_apps: Cached::new(REFRESH_LIVE),
            hourly: Cached::new(REFRESH_LIVE),
            top_categories: Cached::new(REFRESH_LIVE),
            kpi: Cached::new(REFRESH_LIVE),
            trailing_active: Cached::new(REFRESH_PAST_DAYS),
            timeline_events: Cached::new(REFRESH_LIVE),
            top_domains: Cached::new(REFRESH_LIVE),
            month_trailing_year: Cached::new(REFRESH_PAST_DAYS),
            month_top_apps: Cached::new(REFRESH_PAST_DAYS),
            month_top_categories: Cached::new(REFRESH_PAST_DAYS),
            month_top_domains: Cached::new(REFRESH_PAST_DAYS),
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
            FetchResult::TrailingActive(Ok(v)) => self.trailing_active.apply_success(v, now),
            FetchResult::TrailingActive(Err(e)) => self.trailing_active.apply_failure(e, now),
            FetchResult::TimelineEvents(Ok(v)) => self.timeline_events.apply_success(v, now),
            FetchResult::TimelineEvents(Err(e)) => self.timeline_events.apply_failure(e, now),
            FetchResult::TopDomains(Ok(v)) => self.top_domains.apply_success(v, now),
            FetchResult::TopDomains(Err(e)) => self.top_domains.apply_failure(e, now),
            FetchResult::MonthTrailingYear(Ok(v)) => self.month_trailing_year.apply_success(v, now),
            FetchResult::MonthTrailingYear(Err(e)) => self.month_trailing_year.apply_failure(e, now),
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
        }
    }
}

impl Default for DataCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawn fetches for any cache entry that's stale and not in-flight.
/// Each fetch resolves into a FetchResult sent back over `tx`.
///
/// The Tauri app's TanStack Query layer does the same thing; this is the
/// hand-written equivalent for the TUI process. Spawning per-fetch (vs a
/// fetcher pool) keeps the dispatch logic readable and lets tokio's
/// scheduler handle concurrency. With ~3 entries refetching every 5s,
/// task creation overhead is irrelevant.
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

    if cache.kpi.should_refetch(now) {
        cache.kpi.mark_in_flight();
        let tx = tx.clone();
        let range = range.clone();
        tokio::spawn(async move {
            let client = daylog_core::aw_client::AwClient::new();
            let result = daylog_core::queries::kpi(&client, range)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::Kpi(result));
        });
    }

    if cache.trailing_active.should_refetch(now) {
        cache.trailing_active.mark_in_flight();
        let tx = tx.clone();
        tokio::spawn(async move {
            // trailing_days_past returns days 1..=7. Project to active
            // seconds and drop the events array — the sparkline only
            // needs daily totals.
            let result = daylog_core::queries::trailing_days_past(7)
                .await
                .map(|days| {
                    let mut out = [0.0_f64; 7];
                    for d in days {
                        let idx = d.days_ago.saturating_sub(1) as usize;
                        if idx < 7 {
                            out[idx] = d.afk.active_seconds;
                        }
                    }
                    out
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(FetchResult::TrailingActive(result));
        });
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
            let result =
                daylog_core::queries::top_apps(&client, TimeRange::LastNDays { days: 30 })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
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
        assert!(c.should_refetch(t0()), "fresh cache should always refetch first");
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

    // --- DataCache + fetch dispatch tests live below the Cached<T> tests ---

    #[test]
    fn data_cache_starts_with_no_values() {
        let dc = DataCache::new();
        assert!(dc.top_apps.value().is_none());
        assert!(dc.hourly.value().is_none());
        assert!(dc.top_categories.value().is_none());
        assert!(dc.kpi.value().is_none());
        assert!(dc.trailing_active.value().is_none());
        assert!(!dc.any_offline());
    }

    #[test]
    fn data_cache_kpi_offline_flips_aggregate_flag() {
        let mut dc = DataCache::new();
        let now = Instant::now();
        for _ in 0..OFFLINE_THRESHOLD {
            dc.kpi.apply_failure("err".into(), now);
        }
        assert!(dc.any_offline(), "kpi failures must flip the aggregate flag");
    }

    #[test]
    fn data_cache_trailing_offline_does_not_flip_aggregate_flag() {
        // trailing_active runs at 5min cadence; surfacing offline on
        // its blips would create false positives. Live slots are the
        // signal for the footer indicator.
        let mut dc = DataCache::new();
        let now = Instant::now();
        for _ in 0..(OFFLINE_THRESHOLD + 2) {
            dc.trailing_active.apply_failure("err".into(), now);
        }
        assert!(
            !dc.any_offline(),
            "trailing_active failures alone must not flag the tracker offline"
        );
    }

    #[test]
    fn data_cache_apply_routes_kpi_and_trailing_results() {
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

        let trailing: [f64; 7] = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0];
        dc.apply(FetchResult::TrailingActive(Ok(trailing)), now);
        assert_eq!(dc.trailing_active.value().copied(), Some(trailing));
    }

    #[test]
    fn data_cache_any_offline_aggregates_individual_states() {
        let mut dc = DataCache::new();
        let now = Instant::now();
        for _ in 0..OFFLINE_THRESHOLD {
            dc.top_apps.apply_failure("err".into(), now);
        }
        assert!(dc.any_offline(), "one offline cache flips the aggregate flag");
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
