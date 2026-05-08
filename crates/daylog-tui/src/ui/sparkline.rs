//! 7-day active-time sparkline. Composes today (from the live `kpi`
//! cache) with the past 6 entries from `trailing_active`. Today is the
//! rightmost bar. Wide mode renders a "<oldest>-<newest>" weekday range
//! suffix; Narrow drops the suffix; Stacked never reaches this widget
//! (the layout collapses).

use chrono::{Datelike, Local, Weekday};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::theme::{LayoutMode, Theme};

/// Render the sparkline at `area`. Pulls today from `today_active_secs`
/// (None → 0) and indices 1..=6 from `trailing_active` (yesterday through
/// 6 days ago, per data.rs convention). Bars are scaled to minutes so
/// heights are stable at typical terminal sizes.
pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    layout: LayoutMode,
    today_active_secs: Option<f64>,
    trailing_active: Option<&[f64; 7]>,
) {
    if matches!(layout, LayoutMode::Stacked) {
        // Stacked layouts hide the sparkline; the caller routes around
        // this fn but we guard defensively in case of misuse.
        return;
    }

    let series = build_series(today_active_secs, trailing_active);
    let total_minutes: u64 = series.iter().sum();

    if total_minutes == 0 {
        // Empty state: leave the area blank rather than rendering a
        // flat zero-height bar. Mirrors the "no data" feel of the
        // existing per-panel skeletons.
        let p = Paragraph::new(Line::from(Span::styled(
            "no week yet",
            theme.dim_style(),
        )));
        f.render_widget(p, area);
        return;
    }

    // Wide mode reserves a small slot on the right for the weekday
    // range label (e.g. "Sat-Fri" when today is Friday).
    let (chart_area, label_area) = match layout {
        LayoutMode::Wide => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(7), Constraint::Length(8)])
                .split(area);
            (chunks[0], Some(chunks[1]))
        }
        LayoutMode::Narrow | LayoutMode::Stacked => (area, None),
    };

    let spark = Sparkline::default()
        .data(&series)
        .style(Style::default().fg(theme.ember));
    f.render_widget(spark, chart_area);

    if let Some(label_area) = label_area {
        let label = weekday_range_label(Local::now().weekday());
        let p = Paragraph::new(Line::from(Span::styled(
            format!(" {}", label),
            theme.dim_style(),
        )));
        f.render_widget(p, label_area);
    }
}

/// Build a 7-element series of active minutes, oldest-to-newest. Today
/// goes at the end so the rightmost bar is "now." Missing slots fall
/// back to 0 — sparkline shows a flat segment instead of crashing.
fn build_series(today_secs: Option<f64>, past: Option<&[f64; 7]>) -> Vec<u64> {
    let to_min = |secs: f64| (secs.max(0.0) / 60.0).round() as u64;
    let today = today_secs.map(to_min).unwrap_or(0);
    // past[0] = yesterday, past[5] = 6 days ago. Reverse so oldest is first.
    let mut out: Vec<u64> = (0..6)
        .rev()
        .map(|i| past.map(|p| to_min(p[i])).unwrap_or(0))
        .collect();
    out.push(today);
    out
}

/// Three-letter weekday range: oldest day - today's weekday. e.g. when
/// today is Friday, the 7-day window starts the previous Saturday and
/// the label reads `Sat-Fri`.
fn weekday_range_label(today: Weekday) -> String {
    let oldest = today_weekday_minus(today, 6);
    format!("{}-{}", short_name(oldest), short_name(today))
}

fn today_weekday_minus(today: Weekday, days: i64) -> Weekday {
    let n_today = today.num_days_from_monday() as i64;
    let n = (n_today - days).rem_euclid(7) as u32;
    Weekday::try_from(n as u8).unwrap_or(Weekday::Mon)
}

fn short_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_series_places_today_rightmost() {
        let past: [f64; 7] = [
            60.0,    // index 0 = yesterday → 1m
            120.0,   // 2 days ago → 2m
            180.0,   // 3 → 3m
            240.0,   // 4 → 4m
            300.0,   // 5 → 5m
            360.0,   // 6 → 6m
            420.0,   // 7 days ago → ignored, only 6 past slots used
        ];
        let series = build_series(Some(30.0 * 60.0), Some(&past));
        assert_eq!(series.len(), 7);
        // Oldest first: 6m, 5m, 4m, 3m, 2m, 1m, then today = 30m.
        assert_eq!(series, vec![6, 5, 4, 3, 2, 1, 30]);
    }

    #[test]
    fn build_series_handles_missing_inputs() {
        let series = build_series(None, None);
        assert_eq!(series, vec![0; 7]);
    }

    #[test]
    fn weekday_range_label_is_oldest_to_today() {
        assert_eq!(weekday_range_label(Weekday::Fri), "Sat-Fri");
        assert_eq!(weekday_range_label(Weekday::Mon), "Tue-Mon");
        assert_eq!(weekday_range_label(Weekday::Sun), "Mon-Sun");
    }
}
