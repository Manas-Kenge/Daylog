//! Compact 1-line KPI strip — `Active 5h 30m  Longest 1h 12m  Best 10:00  +2h Browsing vs typical Tue`.
//!
//! Per DESIGN.md D1: matches the desktop's headline numbers in the same
//! order, surfaced as labels + values + an optional pattern-shift suffix.
//! Width-driven: Wide includes `Best <hour>:00` and the "vs typical
//! <weekday>" suffix, Narrow drops the suffix, Stacked omits the pattern
//! shift entirely.
//!
//! On error or first load, render a *shape-stable* skeleton with em-dash
//! placeholders rather than a banner — errors surface as a footer pill.
//! The previous "kpi unavailable" red text was a banner-level alarm for
//! a routine first-paint condition.

use daylog_core::kpi::KpiSummary;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::{LayoutMode, Theme};
use crate::ui::format_duration;

/// Render the KPI strip into `area`. Reads the cached payload directly so
/// the caller doesn't have to build span lists. `last_error` is unused
/// today (errors live in the footer) but kept on the signature so future
/// wiring can re-introduce a panel-local indicator without touching every
/// call site.
pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    layout: LayoutMode,
    kpi: Option<&KpiSummary>,
    _last_error: Option<&str>,
) {
    let line = match kpi {
        Some(k) => build_line(theme, layout, k),
        None => build_skeleton(theme, layout),
    };
    let p = Paragraph::new(line).alignment(Alignment::Left);
    f.render_widget(p, area);
}

fn build_line(theme: &Theme, layout: LayoutMode, k: &KpiSummary) -> Line<'static> {
    let separator = match layout {
        LayoutMode::Wide => "   ",
        _ => "  \u{00b7}  ",
    };

    let label = theme.kpi_label_style();
    let value = theme.kpi_value_style();

    let mut spans: Vec<Span<'static>> = Vec::new();

    spans.push(Span::styled("Active ", label));
    spans.push(Span::styled(format_duration(k.active_secs), value));

    if let Some(longest) = &k.longest_stretch {
        spans.push(Span::raw(separator));
        spans.push(Span::styled("Longest ", label));
        spans.push(Span::styled(format_duration(longest.seconds), value));
    }

    // Best-window slot is Wide-only — it lives between Longest and the
    // pattern-shift suffix and would crowd the line at Narrow.
    if layout == LayoutMode::Wide {
        if let Some(best) = &k.best_window {
            spans.push(Span::raw(separator));
            spans.push(Span::styled("Best ", label));
            spans.push(Span::styled(
                format!("{:02}:00", best.start_hour),
                value,
            ));
        }
    }

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
            // Pattern-shift body: bold + theme.fg, no colour. Ember is
            // reserved for the active tab in the chrome above; using it
            // again here was the second-loudest signal in the top half
            // of the screen and competed with navigation for attention.
            spans.push(Span::styled(
                body,
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ));
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

/// Shape-stable skeleton: same span layout as a populated render, just
/// with `…` / `—` placeholders. Stable shape means values don't shift
/// horizontally when the data lands.
fn build_skeleton(theme: &Theme, layout: LayoutMode) -> Line<'static> {
    let separator = match layout {
        LayoutMode::Wide => "   ",
        _ => "  \u{00b7}  ",
    };
    let label = theme.kpi_label_style();
    let placeholder = Style::default().fg(theme.dim);
    let mut spans: Vec<Span<'static>> = Vec::new();

    spans.push(Span::styled("Active ", label));
    spans.push(Span::styled("\u{2026}", placeholder));

    spans.push(Span::raw(separator));
    spans.push(Span::styled("Longest ", label));
    spans.push(Span::styled("\u{2026}", placeholder));

    if layout == LayoutMode::Wide {
        spans.push(Span::raw(separator));
        spans.push(Span::styled("Best ", label));
        spans.push(Span::styled("\u{2014}", placeholder));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use daylog_core::kpi::{BaselineStats, BestWindow, KpiSummary, LongestStretch, PatternShift};
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
            best_window: Some(BestWindow {
                start_hour: 10,
                end_hour: 13,
                seconds: 3.0 * 3600.0,
            }),
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
    fn wide_includes_active_longest_best_and_pattern_shift_with_weekday() {
        let line = render_line(LayoutMode::Wide, Some(&fixture()), 120);
        assert!(line.contains("Active "), "wide should label Active: {line}");
        assert!(line.contains("5h 30m"), "wide should show active value: {line}");
        assert!(line.contains("Longest "), "wide should label Longest: {line}");
        assert!(line.contains("1h 12m"), "wide should show longest value: {line}");
        assert!(line.contains("Best "), "wide should label Best: {line}");
        assert!(line.contains("10:00"), "wide should show best-window start hour: {line}");
        assert!(line.contains("Browsing"), "wide should show shifted root: {line}");
        assert!(line.contains("vs typical Tue"), "wide should keep weekday suffix: {line}");
    }

    #[test]
    fn narrow_drops_best_and_vs_typical_suffix() {
        let line = render_line(LayoutMode::Narrow, Some(&fixture()), 90);
        assert!(line.contains("Active "), "narrow still labels Active: {line}");
        assert!(line.contains("Browsing"), "narrow keeps the root delta: {line}");
        assert!(
            !line.contains("vs typical"),
            "narrow must drop 'vs typical' suffix: {line}"
        );
        assert!(
            !line.contains("Best "),
            "narrow must drop the Best slot (Wide-only): {line}"
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
    fn skeleton_has_stable_shape_when_kpi_absent() {
        // Skeleton mirrors the populated layout — labels render even
        // before values land. No "kpi unavailable" banner. No "loading"
        // marker (errors flow to the footer pill).
        let line = render_line(LayoutMode::Wide, None, 80);
        assert!(line.contains("Active "), "skeleton still labels Active: {line}");
        assert!(line.contains("Longest "), "skeleton still labels Longest: {line}");
        assert!(
            !line.contains("kpi unavailable"),
            "skeleton must NOT show the old red banner: {line}"
        );
    }
}
