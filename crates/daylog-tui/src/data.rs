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

use daylog_core::aggregate::{CategorySummary, HourBucket};
use daylog_core::time::TimeRange;
use serde_json::Value;

/// Live cadence for today's slot — matches `useAw.ts` REFRESH_MS = 5_000.
pub const REFRESH_LIVE: Duration = Duration::from_secs(5);

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

/// Result message sent back to the App after a fetch resolves. The App
/// matches on the variant to find which `Cached<T>` to update.
#[derive(Debug)]
pub enum FetchResult {
    TopApps(Result<Vec<TopAppRow>, String>),
    Hourly(Result<Vec<HourBucket>, String>),
    TopCategories(Result<Vec<CategorySummary>, String>),
}

/// Bundle of every cache entry the Overview tab reads. Future tabs add
/// their own fields to this struct.
#[derive(Debug)]
pub struct DataCache {
    pub top_apps: Cached<Vec<TopAppRow>>,
    pub hourly: Cached<Vec<HourBucket>>,
    pub top_categories: Cached<Vec<CategorySummary>>,
}

impl DataCache {
    pub fn new() -> Self {
        Self {
            top_apps: Cached::new(REFRESH_LIVE),
            hourly: Cached::new(REFRESH_LIVE),
            top_categories: Cached::new(REFRESH_LIVE),
        }
    }

    /// True if any tracked cache has crossed the offline threshold.
    /// Drives the footer's "○ tracker offline" indicator.
    pub fn any_offline(&self) -> bool {
        self.top_apps.is_offline() || self.hourly.is_offline() || self.top_categories.is_offline()
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
        assert!(!dc.any_offline());
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
