//! KPI strip math. Discovery-shaped (PLAN.md §1.0). All bucketing uses local time.

use chrono::{DateTime, Local, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::data::aggregate::CategorizedEvent;

/// Below this, a contiguous run on a single root isn't "focus."
pub const FOCUS_FLOOR_SECS: f64 = 120.0;

/// Days below this overall-active threshold are excluded from the
/// trailing baseline so vacation/paused days don't drag the median to zero.
pub const QUIET_DAY_FLOOR_SECS: f64 = 30.0 * 60.0;

/// Suppress "vs typical" deltas tinier than this. "+47 seconds" is noise.
pub const PATTERN_SHIFT_NOISE_FLOOR_SECS: f64 = 15.0 * 60.0;

/// 3-hour focus window.
pub const WINDOW_HOURS: usize = 3;

/// Fallback root when an event's category path is empty.
pub const UNCATEGORIZED_ROOT: &str = "Uncategorized";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongestStretch {
    pub seconds: f64,
    pub category_root: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BestWindow {
    pub start_hour: u8,
    pub end_hour: u8,
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternShift {
    pub category_root: String,
    pub delta_secs: f64,
    pub weekday_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineStats {
    pub effective_days: u32,
    pub median: f64,
    pub mean: f64,
    /// Sample stdev. 0 with fewer than 2 effective days.
    pub stdev: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KpiSummary {
    pub active_secs: f64,
    pub afk_secs: f64,
    /// `active_secs / (active_secs + afk_secs)`, clamped to 0 when both are zero.
    pub active_ratio: f64,
    pub longest_stretch: Option<LongestStretch>,
    pub best_window: Option<BestWindow>,
    /// Dominant-root delta vs the trailing-7 median for that root.
    pub pattern_shift: Option<PatternShift>,
    pub focus_by_hour: [f64; 24],
    /// Trailing-7 baseline of total active seconds.
    pub active_baseline: BaselineStats,
    /// Trailing-7 baseline of `longest_stretch.seconds` per past day.
    pub longest_baseline: BaselineStats,
    /// Trailing-7 baseline of `best_window.seconds` per past day.
    pub best_window_baseline: BaselineStats,
}

/// First segment of the category path; "Uncategorized" when empty.
pub fn category_root(path: &[String]) -> &str {
    path.first().map(String::as_str).unwrap_or(UNCATEGORIZED_ROOT)
}

/// Longest contiguous run sharing a category root. Runs below `floor_secs`
/// don't qualify as focus.
pub fn longest_focus(events: &[CategorizedEvent], floor_secs: f64) -> Option<LongestStretch> {
    if events.is_empty() {
        return None;
    }
    let mut sorted: Vec<&CategorizedEvent> = events.iter().collect();
    sorted.sort_by_key(|e| e.timestamp);

    let mut best: Option<(f64, String)> = None;
    let mut run_secs = 0.0_f64;
    let mut run_root: Option<String> = None;

    let consider = |best: &mut Option<(f64, String)>, secs: f64, root: &Option<String>| {
        if secs >= floor_secs {
            if let Some(r) = root {
                if best.as_ref().map_or(true, |(b, _)| secs > *b) {
                    *best = Some((secs, r.clone()));
                }
            }
        }
    };

    for ev in sorted {
        let root = category_root(&ev.category).to_string();
        if run_root.as_deref() != Some(&root) {
            consider(&mut best, run_secs, &run_root);
            run_root = Some(root);
            run_secs = ev.duration;
        } else {
            run_secs += ev.duration;
        }
    }
    consider(&mut best, run_secs, &run_root);

    best.map(|(seconds, category_root)| LongestStretch {
        seconds,
        category_root,
    })
}

/// 24 buckets of focused-time per hour. Only counts duration that lies
/// inside a run ≥ `floor_secs` on a single root. Hour is the event's
/// start hour in local time.
pub fn focus_by_hour(events: &[CategorizedEvent], floor_secs: f64) -> [f64; 24] {
    let mut out = [0.0_f64; 24];
    if events.is_empty() {
        return out;
    }
    let mut sorted: Vec<&CategorizedEvent> = events.iter().collect();
    sorted.sort_by_key(|e| e.timestamp);

    // Walk runs of equal root; flush each qualifying run by attributing
    // each member event's duration to its own start-hour bucket.
    let n = sorted.len();
    let mut run_start = 0_usize;
    let mut run_root: Option<String> = Some(category_root(&sorted[0].category).to_string());
    let mut run_secs = 0.0_f64;

    for i in 0..n {
        let root = category_root(&sorted[i].category).to_string();
        if run_root.as_deref() != Some(&root) {
            flush_run(&sorted, run_start, i, run_secs, floor_secs, &mut out);
            run_start = i;
            run_root = Some(root);
            run_secs = sorted[i].duration;
        } else {
            run_secs += sorted[i].duration;
        }
    }
    flush_run(&sorted, run_start, n, run_secs, floor_secs, &mut out);
    out
}

fn flush_run(
    sorted: &[&CategorizedEvent],
    start: usize,
    end: usize,
    run_secs: f64,
    floor_secs: f64,
    out: &mut [f64; 24],
) {
    if run_secs < floor_secs {
        return;
    }
    for ev in &sorted[start..end] {
        let h = ev.timestamp.with_timezone(&Local).hour() as usize;
        if h < 24 {
            out[h] += ev.duration;
        }
    }
}

/// Densest contiguous `WINDOW_HOURS`-hour slice in the per-hour focus
/// spark. Ties break to the earliest start. Returns `None` when nothing
/// qualifies.
pub fn best_window(focus_by_hour: &[f64; 24]) -> Option<BestWindow> {
    let mut best_start = 0_usize;
    let mut best_sum = 0.0_f64;
    for start in 0..=(24 - WINDOW_HOURS) {
        let sum: f64 = focus_by_hour[start..start + WINDOW_HOURS].iter().sum();
        if sum > best_sum {
            best_sum = sum;
            best_start = start;
        }
    }
    if best_sum == 0.0 {
        return None;
    }
    Some(BestWindow {
        start_hour: best_start as u8,
        end_hour: (best_start + WINDOW_HOURS) as u8,
        seconds: best_sum,
    })
}

/// Median/mean/stdev over the per-day metric, with quiet days filtered
/// by the parallel daily-active array. Caller picks which metric they
/// want a baseline for; we just need the active totals to gate.
pub fn trailing_stats(daily_totals: &[f64], daily_active_totals: &[f64]) -> BaselineStats {
    let len = daily_totals.len().min(daily_active_totals.len());
    let mut samples: Vec<f64> = Vec::with_capacity(len);
    for i in 0..len {
        if daily_active_totals[i] >= QUIET_DAY_FLOOR_SECS {
            samples.push(daily_totals[i]);
        }
    }
    if samples.is_empty() {
        return BaselineStats {
            effective_days: 0,
            median: 0.0,
            mean: 0.0,
            stdev: 0.0,
        };
    }
    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    let median = if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    };
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance = if samples.len() >= 2 {
        let sum_sq: f64 = samples.iter().map(|v| (v - mean).powi(2)).sum();
        sum_sq / (samples.len() - 1) as f64
    } else {
        0.0
    };
    BaselineStats {
        effective_days: samples.len() as u32,
        median,
        mean,
        stdev: variance.sqrt(),
    }
}

/// Per-root totals for one day, keyed by category root.
fn root_totals(events: &[CategorizedEvent]) -> std::collections::HashMap<String, f64> {
    let mut out: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for ev in events {
        let root = category_root(&ev.category).to_string();
        *out.entry(root).or_insert(0.0) += ev.duration;
    }
    out
}

fn median_of(mut xs: Vec<f64>) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = xs.len() / 2;
    if xs.len() % 2 == 0 {
        (xs[mid - 1] + xs[mid]) / 2.0
    } else {
        xs[mid]
    }
}

/// Largest absolute delta between today's per-root totals and the
/// trailing-7 median for that root. Suppressed below the noise floor.
/// Weekday label is descriptive (today's name); the baseline is not
/// same-weekday-filtered.
pub fn pattern_shift(
    today: &[CategorizedEvent],
    past_days: &[Vec<CategorizedEvent>],
    today_weekday_label: &str,
) -> Option<PatternShift> {
    // No baseline → no signal (don't compare against implicit zero).
    if past_days.is_empty() {
        return None;
    }

    let today_totals = root_totals(today);

    // Roots to consider = anything seen today or in the past window.
    let mut roots: std::collections::BTreeSet<String> = today_totals.keys().cloned().collect();
    for day in past_days {
        for ev in day {
            roots.insert(category_root(&ev.category).to_string());
        }
    }

    let mut best: Option<(String, f64)> = None;
    for root in roots {
        let today_secs = today_totals.get(&root).copied().unwrap_or(0.0);
        let past_samples: Vec<f64> = past_days
            .iter()
            .map(|day| root_totals(day).get(&root).copied().unwrap_or(0.0))
            .collect();
        let baseline = median_of(past_samples);
        let delta = today_secs - baseline;
        if best.as_ref().map_or(true, |(_, d)| delta.abs() > d.abs()) {
            best = Some((root, delta));
        }
    }

    let (root, delta) = best?;
    if delta.abs() < PATTERN_SHIFT_NOISE_FLOOR_SECS {
        return None;
    }
    Some(PatternShift {
        category_root: root,
        delta_secs: delta,
        weekday_label: today_weekday_label.to_string(),
    })
}

/// "Mon"/"Tue"/… for a UTC instant, evaluated in local time.
pub fn weekday_label(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local)
        .format("%a")
        .to_string()
}

/// Compose the whole strip in one call. `active_secs` / `afk_secs` /
/// `past_active_secs` come from `summarize_afk` so AFK accounting has
/// one source of truth. `past_active_secs` is parallel to `past_days`:
/// index `i` is the active seconds for day `past_days[i]`.
pub fn summarize(
    today: &[CategorizedEvent],
    past_days: &[Vec<CategorizedEvent>],
    past_active_secs: &[f64],
    active_secs: f64,
    afk_secs: f64,
    today_weekday_label: &str,
) -> KpiSummary {
    let focus_by_hour_arr = focus_by_hour(today, FOCUS_FLOOR_SECS);

    let past_longest: Vec<f64> = past_days
        .iter()
        .map(|d| longest_focus(d, FOCUS_FLOOR_SECS).map(|s| s.seconds).unwrap_or(0.0))
        .collect();
    let past_best_window: Vec<f64> = past_days
        .iter()
        .map(|d| {
            let fbh = focus_by_hour(d, FOCUS_FLOOR_SECS);
            best_window(&fbh).map(|w| w.seconds).unwrap_or(0.0)
        })
        .collect();

    let tracked = active_secs + afk_secs;
    let active_ratio = if tracked > 0.0 { active_secs / tracked } else { 0.0 };

    KpiSummary {
        active_secs,
        afk_secs,
        active_ratio,
        longest_stretch: longest_focus(today, FOCUS_FLOOR_SECS),
        best_window: best_window(&focus_by_hour_arr),
        pattern_shift: pattern_shift(today, past_days, today_weekday_label),
        focus_by_hour: focus_by_hour_arr,
        active_baseline: trailing_stats(past_active_secs, past_active_secs),
        longest_baseline: trailing_stats(&past_longest, past_active_secs),
        best_window_baseline: trailing_stats(&past_best_window, past_active_secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::Value;

    fn ev(hour_local: u32, dur: f64, category: &[&str]) -> CategorizedEvent {
        // Build the timestamp in local time, then convert to UTC, so
        // focus_by_hour buckets land on `hour_local` regardless of the
        // host's timezone.
        let local = Local
            .with_ymd_and_hms(2026, 5, 8, hour_local, 0, 0)
            .single()
            .expect("valid local time");
        CategorizedEvent {
            timestamp: local.with_timezone(&Utc),
            duration: dur,
            data: Value::Null,
            category: category.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn category_root_first_segment() {
        let path = vec!["Work".to_string(), "Programming".to_string()];
        assert_eq!(category_root(&path), "Work");
    }

    #[test]
    fn category_root_falls_back_on_empty() {
        let path: Vec<String> = vec![];
        assert_eq!(category_root(&path), "Uncategorized");
    }

    #[test]
    fn longest_focus_skips_below_floor() {
        // Single 60s event is below the 120s floor.
        let events = vec![ev(14, 60.0, &["Work"])];
        assert_eq!(longest_focus(&events, FOCUS_FLOOR_SECS), None);
    }

    #[test]
    fn longest_focus_picks_largest_run() {
        let events = vec![
            // 10:00 Work, 600s — qualifies on its own.
            ev(10, 600.0, &["Work"]),
            // 11:00 Browsing, breaks the Work run.
            ev(11, 300.0, &["Browsing"]),
            // 12:00 + 13:00 Work — contiguous 1800s run.
            ev(12, 900.0, &["Work"]),
            ev(13, 900.0, &["Work"]),
        ];
        let got = longest_focus(&events, FOCUS_FLOOR_SECS).expect("some");
        assert_eq!(got.seconds, 1800.0);
        assert_eq!(got.category_root, "Work");
    }

    #[test]
    fn longest_focus_returns_none_when_all_below_floor() {
        let events = vec![
            ev(9, 60.0, &["Work"]),
            ev(10, 30.0, &["Browsing"]),
            ev(11, 90.0, &["Work"]),
        ];
        assert_eq!(longest_focus(&events, FOCUS_FLOOR_SECS), None);
    }

    #[test]
    fn focus_by_hour_buckets_to_start_hour() {
        let events = vec![
            ev(9, 600.0, &["Work"]),
            ev(14, 1800.0, &["Work"]),
            ev(15, 1800.0, &["Work"]),
        ];
        let h = focus_by_hour(&events, FOCUS_FLOOR_SECS);
        assert_eq!(h[9], 600.0);
        assert_eq!(h[14], 1800.0);
        assert_eq!(h[15], 1800.0);
        assert_eq!(h[10], 0.0);
    }

    #[test]
    fn focus_by_hour_handles_midnight_spanning_events() {
        // Two events at 23 and 00 the next day, on the same root, contiguous.
        // Each event's duration counts in its own start hour.
        let late = Local
            .with_ymd_and_hms(2026, 5, 8, 23, 0, 0)
            .single()
            .unwrap();
        let early = Local
            .with_ymd_and_hms(2026, 5, 9, 0, 0, 0)
            .single()
            .unwrap();
        let events = vec![
            CategorizedEvent {
                timestamp: late.with_timezone(&Utc),
                duration: 1800.0,
                data: Value::Null,
                category: vec!["Work".into()],
            },
            CategorizedEvent {
                timestamp: early.with_timezone(&Utc),
                duration: 1800.0,
                data: Value::Null,
                category: vec!["Work".into()],
            },
        ];
        let h = focus_by_hour(&events, FOCUS_FLOOR_SECS);
        assert_eq!(h[23], 1800.0);
        assert_eq!(h[0], 1800.0);
    }

    #[test]
    fn best_window_returns_none_when_no_focus() {
        let perhour = [0.0_f64; 24];
        assert_eq!(best_window(&perhour), None);
    }

    #[test]
    fn best_window_finds_densest_three_hours() {
        let events = vec![
            ev(9, 600.0, &["Work"]),
            ev(14, 1800.0, &["Work"]),
            ev(15, 1800.0, &["Work"]),
            ev(16, 1800.0, &["Work"]),
        ];
        let h = focus_by_hour(&events, FOCUS_FLOOR_SECS);
        let win = best_window(&h).expect("some");
        assert_eq!(win.start_hour, 14);
        assert_eq!(win.end_hour, 17);
    }

    #[test]
    fn best_window_breaks_ties_by_earliest_start() {
        let events = vec![
            ev(9, 1200.0, &["Work"]),
            ev(10, 1200.0, &["Work"]),
            ev(11, 1200.0, &["Work"]),
            // Equal-magnitude run later in the day.
            ev(20, 1200.0, &["Work"]),
            ev(21, 1200.0, &["Work"]),
            ev(22, 1200.0, &["Work"]),
        ];
        let h = focus_by_hour(&events, FOCUS_FLOOR_SECS);
        let win = best_window(&h).expect("some");
        assert_eq!(win.start_hour, 9);
    }

    #[test]
    fn trailing_stats_zeros_on_empty_input() {
        let s = trailing_stats(&[], &[]);
        assert_eq!(s.effective_days, 0);
        assert_eq!(s.median, 0.0);
        assert_eq!(s.mean, 0.0);
        assert_eq!(s.stdev, 0.0);
    }

    #[test]
    fn trailing_stats_excludes_quiet_days() {
        // Three days, last one quiet (active < 30m).
        let totals = [3600.0, 7200.0, 100.0];
        let active = [4.0 * 3600.0, 5.0 * 3600.0, 60.0];
        let s = trailing_stats(&totals, &active);
        assert_eq!(s.effective_days, 2);
        assert_eq!(s.median, 5400.0);
    }

    #[test]
    fn trailing_stats_median_odd_length() {
        let totals = [100.0, 200.0, 300.0];
        let active = [10000.0; 3];
        assert_eq!(trailing_stats(&totals, &active).median, 200.0);
    }

    #[test]
    fn trailing_stats_stdev_zero_with_one_sample() {
        let s = trailing_stats(&[1000.0], &[10000.0]);
        assert_eq!(s.stdev, 0.0);
    }

    #[test]
    fn pattern_shift_suppresses_below_noise_floor() {
        // Today: 10 minutes Work. Past: 5 minutes Work. Delta = 300s,
        // well under the 900s noise floor.
        let today = vec![ev(10, 600.0, &["Work"])];
        let past = vec![vec![ev(10, 300.0, &["Work"])]];
        assert_eq!(pattern_shift(&today, &past, "Tue"), None);
    }

    #[test]
    fn pattern_shift_picks_dominant_root_delta() {
        // Today: 2h Browsing, 0 Work. Past 3 days: ~0 Browsing,
        // ~1h Work each. Browsing delta (+7200s) beats Work delta (-3600s)
        // in magnitude, so Browsing wins.
        let today = vec![ev(10, 7200.0, &["Browsing"])];
        let past = vec![
            vec![ev(10, 3600.0, &["Work"])],
            vec![ev(10, 3600.0, &["Work"])],
            vec![ev(10, 3600.0, &["Work"])],
        ];
        let shift = pattern_shift(&today, &past, "Tue").expect("some");
        assert_eq!(shift.category_root, "Browsing");
        assert!(shift.delta_secs > 0.0);
        assert_eq!(shift.weekday_label, "Tue");
    }

    #[test]
    fn summarize_composes_a_hand_crafted_day() {
        let today = vec![
            ev(9, 600.0, &["Work"]),
            ev(14, 1800.0, &["Work"]),
            ev(15, 1800.0, &["Work"]),
            ev(16, 1800.0, &["Work"]),
        ];
        let past: Vec<Vec<CategorizedEvent>> = vec![];
        let past_active: Vec<f64> = vec![];
        let s = summarize(&today, &past, &past_active, 6000.0, 1200.0, "Fri");
        assert_eq!(s.active_secs, 6000.0);
        assert_eq!(s.afk_secs, 1200.0);
        // 6000 / (6000 + 1200) ≈ 0.833.
        assert!((s.active_ratio - 6000.0 / 7200.0).abs() < 1e-6);
        let stretch = s.longest_stretch.expect("some");
        // Matches desktop's longestFocus: same-root events sum regardless of
        // time gaps between them, so all four Work events fold into one run.
        assert_eq!(stretch.seconds, 6000.0);
        assert_eq!(stretch.category_root, "Work");
        let win = s.best_window.expect("some");
        assert_eq!(win.start_hour, 14);
        assert_eq!(win.end_hour, 17);
        // No past days → no pattern shift signal worth surfacing.
        assert_eq!(s.pattern_shift, None);
        assert_eq!(s.focus_by_hour[14], 1800.0);
        // No past days → empty baselines (effective_days==0).
        assert_eq!(s.active_baseline.effective_days, 0);
        assert_eq!(s.longest_baseline.effective_days, 0);
        assert_eq!(s.best_window_baseline.effective_days, 0);
    }
}
