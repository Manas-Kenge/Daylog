//! Compact 1-line KPI strip — `Active 5h 30m  Longest 1h 12m  +2h Browsing vs typical Tue`.
//!
//! Per DESIGN.md D1: matches the desktop's headline numbers in the same
//! order, surfaced as labels + values + an optional pattern-shift suffix.
//! Width-driven: Wide includes the "vs typical <weekday>" suffix, Narrow
//! drops it, Stacked omits the pattern shift entirely.

use daylog_core::kpi::KpiSummary;
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::{LayoutMode, Theme};
use crate::ui::format_duration;

/// Render the KPI strip into `area`. Reads the cached payload directly so
/// the caller doesn't have to build span lists.
pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    layout: LayoutMode,
    kpi: Option<&KpiSummary>,
    last_error: Option<&str>,
) {
    let line = if let Some(k) = kpi {
        build_line(theme, layout, k)
    } else if last_error.is_some() {
        Line::from(Span::styled("kpi unavailable", theme.error_style()))
    } else {
        Line::from(Span::styled("loading\u{2026}", theme.dim_style()))
    };
    let p = Paragraph::new(line).alignment(Alignment::Left);
    f.render_widget(p, area);
}

fn build_line(theme: &Theme, layout: LayoutMode, k: &KpiSummary) -> Line<'static> {
    let separator = match layout {
        // Wide leans on whitespace between groups; Narrow uses a mid-dot
        // because the columns are tighter.
        LayoutMode::Wide => "  ",
        LayoutMode::Narrow => "  \u{00b7}  ",
        LayoutMode::Stacked => "  \u{00b7}  ",
    };

    let label = theme.kpi_label_style();
    let value = theme.kpi_value_style();

    let mut spans: Vec<Span<'static>> = Vec::new();

    // Active <duration>
    spans.push(Span::styled("Active ", label));
    spans.push(Span::styled(format_duration(k.active_secs), value));

    // Longest <duration>  (omitted when no qualifying stretch today)
    if let Some(longest) = &k.longest_stretch {
        spans.push(Span::raw(separator));
        spans.push(Span::styled("Longest ", label));
        spans.push(Span::styled(format_duration(longest.seconds), value));
    }

    // Pattern shift suffix: Wide includes the weekday, Narrow drops it,
    // Stacked omits the pattern shift entirely (no horizontal room).
    if layout != LayoutMode::Stacked {
        if let Some(shift) = &k.pattern_shift {
            let sign = if shift.delta_secs >= 0.0 { "+" } else { "\u{2212}" };
            let body = format!(
                "{}{} {}",
                sign,
                format_duration(shift.delta_secs.abs()),
                shift.category_root
            );
            spans.push(Span::raw(separator));
            spans.push(Span::styled(body, Style::default().fg(theme.fg)));
            if layout == LayoutMode::Wide {
                spans.push(Span::styled(
                    format!(" vs typical {}", shift.weekday_label),
                    label,
                ));
            }
        }
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use daylog_core::kpi::{BaselineStats, KpiSummary, LongestStretch, PatternShift};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn empty_baseline() -> BaselineStats {
        BaselineStats {
            effective_days: 0,
            median: 0.0,
            mean: 0.0,
            stdev: 0.0,
        }
    }

    fn fixture() -> KpiSummary {
        KpiSummary {
            active_secs: 5.0 * 3600.0 + 30.0 * 60.0,
            afk_secs: 30.0 * 60.0,
            active_ratio: 0.9,
            longest_stretch: Some(LongestStretch {
                seconds: 3600.0 + 12.0 * 60.0,
                category_root: "Work".into(),
            }),
            best_window: None,
            pattern_shift: Some(PatternShift {
                category_root: "Browsing".into(),
                delta_secs: 2.0 * 3600.0,
                weekday_label: "Tue".into(),
            }),
            focus_by_hour: [0.0; 24],
            active_baseline: empty_baseline(),
            longest_baseline: empty_baseline(),
            best_window_baseline: empty_baseline(),
        }
    }

    fn render_line(layout: LayoutMode, kpi: Option<&KpiSummary>, width: u16) -> String {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = Rect {
                    x: 0,
                    y: 0,
                    width,
                    height: 1,
                };
                render(f, area, &theme, layout, kpi, None);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut row = String::new();
        for x in 0..buf.area.width {
            row.push_str(buf[(x, 0)].symbol());
        }
        row
    }

    #[test]
    fn wide_includes_active_longest_and_pattern_shift_with_weekday() {
        let line = render_line(LayoutMode::Wide, Some(&fixture()), 120);
        assert!(line.contains("Active "), "wide should label Active: {line}");
        assert!(line.contains("5h 30m"), "wide should show active value: {line}");
        assert!(line.contains("Longest "), "wide should label Longest: {line}");
        assert!(line.contains("1h 12m"), "wide should show longest value: {line}");
        assert!(line.contains("Browsing"), "wide should show shifted root: {line}");
        assert!(line.contains("vs typical Tue"), "wide should keep weekday suffix: {line}");
    }

    #[test]
    fn narrow_drops_vs_typical_suffix() {
        let line = render_line(LayoutMode::Narrow, Some(&fixture()), 90);
        assert!(line.contains("Active "), "narrow still labels Active: {line}");
        assert!(line.contains("Browsing"), "narrow keeps the root delta: {line}");
        assert!(
            !line.contains("vs typical"),
            "narrow must drop 'vs typical' suffix: {line}"
        );
    }

    #[test]
    fn stacked_omits_pattern_shift() {
        let line = render_line(LayoutMode::Stacked, Some(&fixture()), 70);
        assert!(line.contains("Active "), "stacked still labels Active: {line}");
        assert!(line.contains("5h 30m"));
        assert!(
            !line.contains("Browsing"),
            "stacked must drop the pattern shift: {line}"
        );
    }

    #[test]
    fn renders_loading_when_kpi_absent() {
        let line = render_line(LayoutMode::Wide, None, 80);
        assert!(line.contains("loading"), "expected loading skeleton: {line}");
    }
}
