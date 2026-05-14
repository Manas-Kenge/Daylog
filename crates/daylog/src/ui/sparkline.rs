//! 7-day active sparkline. Today is the rightmost bar; Stacked layouts never reach this widget.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::theme::{LayoutMode, Theme};

pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    layout: LayoutMode,
    today_active_secs: Option<f64>,
    trailing_active: Option<&[f64; 7]>,
) {
    if matches!(layout, LayoutMode::Stacked) {
        // Stacked is supposed to skip this fn entirely; defensive guard.
        return;
    }

    let series = build_series(today_active_secs, trailing_active);
    let total_minutes: u64 = series.iter().sum();

    if total_minutes == 0 {
        // Empty: render label instead of a flat zero-bar (reads as broken).
        let p = Paragraph::new(Line::from(Span::styled(
            "no week yet",
            theme.dim_style(),
        )));
        f.render_widget(p, area);
        return;
    }

    let spark = Sparkline::default()
        .data(&series)
        .style(Style::default().fg(theme.dim));
    f.render_widget(spark, area);
}

/// 7 active-minute slots, oldest→newest. Missing → 0.
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
        assert_eq!(series, vec![6, 5, 4, 3, 2, 1, 30]);
    }

    #[test]
    fn build_series_handles_missing_inputs() {
        let series = build_series(None, None);
        assert_eq!(series, vec![0; 7]);
    }
}
