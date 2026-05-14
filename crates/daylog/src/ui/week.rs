//! Week tab — calendar-week (Mon → Sun) stacked-bar chart with a
//! "highest Work day" callout. TUI port of `src/pages/WeekPage.tsx`. Like
//! the desktop page, future days inside the current week render as empty
//! columns (axis label only). Daylog is observational, so the callout
//! describes facts ("highest"), not motivation ("strongest").

use chrono::{Datelike, NaiveDate, Weekday};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::data::{WeekDayBuckets, WEEK_ROOT_ORDER};
use crate::theme::{self, Theme};
use crate::ui::stacked_bars::StackedBars;
use crate::ui::{
    format_duration,
    overview::{
        panel_title, render_top_apps_panel, render_top_categories_panel, render_top_domains_panel,
    },
    render_skeleton_body,
};

/// Root displayed in the Work-callout. Matches the desktop's WORK_ROOT.
const WORK_ROOT: &str = "Work";

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let week = app.data.week.value().map(|v| v.as_slice());
    let in_flight = app.data.week.is_in_flight();
    let last_error = app.data.week.last_error();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(14), // chart + stats card
            Constraint::Min(8),     // 7-day rollups
        ])
        .split(area);

    render_top_row(f, chunks[0], app, week, in_flight, last_error);
    render_rollup_row(f, chunks[1], app);
}

fn render_top_row(
    f: &mut Frame,
    area: Rect,
    app: &App,
    week: Option<&[WeekDayBuckets]>,
    in_flight: bool,
    last_error: Option<&str>,
) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);
    render_activity_card(f, cols[0], &app.theme, week, in_flight, last_error);
    render_this_week_card(f, cols[1], &app.theme, week, in_flight, &app.throbber);
}

fn render_rollup_row(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);
    render_top_apps_panel(
        f,
        cols[0],
        &app.theme,
        &app.data.week_top_apps,
        " Top apps · 7 days ",
        &app.throbber,
    );
    render_top_categories_panel(
        f,
        cols[1],
        &app.theme,
        &app.data.week_top_categories,
        " Top categories · 7 days ",
        &app.throbber,
    );
    render_top_domains_panel(
        f,
        cols[2],
        &app.theme,
        &app.data.week_top_domains,
        " Top domains · 7 days ",
        &app.throbber,
    );
}

fn render_this_week_card(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    week: Option<&[WeekDayBuckets]>,
    in_flight: bool,
    throbber: &throbber_widgets_tui::ThrobberState,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING)
        .title(panel_title(theme, " This week ", in_flight));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(days) = week else {
        render_skeleton_body(f, inner, theme, throbber, in_flight);
        return;
    };

    let stats = week_stats(days);
    let label = theme.kpi_label_style();
    let value = theme.kpi_value_style();

    let stat_line = |label_text: &'static str, value_text: String, hint: Option<String>| {
        let mut spans = vec![
            Span::styled(format!(" {:<13}", label_text), label),
            Span::styled(value_text, value),
        ];
        if let Some(h) = hint {
            spans.push(Span::styled(format!("  {}", h), theme.dim_style()));
        }
        Line::from(spans)
    };

    let avg = if stats.days_elapsed > 0 {
        format_duration(stats.avg_secs)
    } else {
        "\u{2014}".to_string()
    };
    let best_value = stats
        .best
        .as_ref()
        .map(|b| format_duration(b.hours_secs))
        .unwrap_or_else(|| "\u{2014}".to_string());
    let best_hint = stats
        .best
        .as_ref()
        .map(|b| format!("{} {}", short_weekday(b.weekday), format_month_day(b.date)));

    let lines = vec![
        stat_line("Total active", format_duration(stats.total_secs), None),
        stat_line(
            "Daily avg",
            avg,
            (stats.days_elapsed > 0).then_some(format!("over {} days", stats.days_elapsed)),
        ),
        stat_line("Best day", best_value, best_hint),
        stat_line(
            "Active days",
            format!("{}/{}", stats.active_days, stats.days_elapsed),
            None,
        ),
    ];
    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

fn render_activity_card(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    week: Option<&[WeekDayBuckets]>,
    in_flight: bool,
    last_error: Option<&str>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING_TIGHT)
        .title(panel_title(theme, " 7-Day Activity Breakdown ", in_flight));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if week.is_none() && last_error.is_some() {
        let p = Paragraph::new("fetch error \u{00b7} check footer")
            .style(theme.dim_style())
            .alignment(Alignment::Left);
        f.render_widget(p, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // legend
            Constraint::Min(5),    // chart
            Constraint::Length(2), // callout
        ])
        .split(inner);

    render_legend(f, chunks[0], theme, week);
    f.render_widget(
        StackedBars {
            theme,
            days: week,
            in_flight,
        },
        chunks[1],
    );
    render_callout(f, chunks[2], theme, week);
}

fn render_legend(f: &mut Frame, area: Rect, theme: &Theme, week: Option<&[WeekDayBuckets]>) {
    let Some(days) = week else {
        return;
    };
    let roots = present_roots(days);
    if roots.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            "Stacked by category · hours",
            theme.dim_style(),
        )));
        f.render_widget(p, area);
        return;
    }

    let mut spans = vec![Span::styled("Stacked by category  ", theme.dim_style())];
    for root in roots {
        spans.push(Span::styled("\u{25a0}", category_root_style(theme, &root)));
        spans.push(Span::styled(format!(" {}  ", root), theme.dim_style()));
    }
    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Left);
    f.render_widget(p, area);
}

fn present_roots(days: &[WeekDayBuckets]) -> Vec<String> {
    let mut present = std::collections::BTreeSet::new();
    for day in days.iter().filter(|d| !d.is_future) {
        for (root, secs) in &day.roots {
            if *secs > 0.0 {
                present.insert(root.clone());
            }
        }
    }
    let mut roots: Vec<String> = present.into_iter().collect();
    roots.sort_by(|a, b| {
        let ai = WEEK_ROOT_ORDER.iter().position(|r| *r == a.as_str());
        let bi = WEEK_ROOT_ORDER.iter().position(|r| *r == b.as_str());
        match (ai, bi) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.cmp(b),
        }
    });
    roots
}

fn render_callout(f: &mut Frame, area: Rect, theme: &Theme, week: Option<&[WeekDayBuckets]>) {
    let Some(days) = week else {
        // Loading — keep the slot blank so the callout doesn't flicker
        // a transient "no Work yet" message before the first frame.
        return;
    };
    let total: f64 = days.iter().map(|d| d.total_active_secs).sum();
    let line: Line = if let Some(best) = highest_work_day(days) {
        let highlight = theme.kpi_value_style();
        Line::from(vec![
            Span::styled(
                format!(
                    "{} ({})",
                    short_weekday(best.weekday),
                    format_month_day(best.date)
                ),
                highlight,
            ),
            Span::raw(" had your highest Work hours this week \u{2014} "),
            Span::styled(format!("{:.1}h", best.hours_secs / 3600.0), highlight),
            Span::raw("."),
        ])
    } else if total <= 0.0 {
        Line::from(Span::styled(
            "No tracked activity yet this week. Pattern callouts appear once Daylog has data.",
            theme.dim_style(),
        ))
    } else {
        Line::from(Span::styled(
            "No Work-categorized time this week \u{2014} set up category rules in Settings to enable Work callouts.",
            theme.dim_style(),
        ))
    };
    let p = Paragraph::new(line).alignment(Alignment::Left);
    f.render_widget(p, area);
}

/// Map a category root to the spectrum colour that paints its bar segment.
/// Mirrors the desktop's `categoryColor` from `src/lib/category-colors.ts`.
/// `Comms` deliberately routes through `theme.chart_2_style()` so the
/// ANSI-16 BOLD-collision lift comes along for free (per `theme.rs:200-205`).
pub fn category_root_style(theme: &Theme, root: &str) -> Style {
    match root {
        "Work" | "Programming" => Style::default().fg(theme.chart_1),
        "Comms" => theme.chart_2_style(),
        "Media" => Style::default().fg(theme.chart_3),
        "Browsing" => Style::default().fg(theme.chart_4),
        "Documents" => Style::default().fg(theme.chart_5),
        _ => theme.dim_style(),
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WeekStats {
    pub total_secs: f64,
    pub avg_secs: f64,
    pub days_elapsed: usize,
    pub active_days: usize,
    pub best: Option<DayHours>,
}

#[derive(Debug, Clone)]
pub(crate) struct DayHours {
    pub date: NaiveDate,
    pub weekday: Weekday,
    pub hours_secs: f64,
}

pub(crate) fn week_stats(week: &[WeekDayBuckets]) -> WeekStats {
    let elapsed: Vec<&WeekDayBuckets> = week.iter().filter(|d| !d.is_future).collect();
    let days_elapsed = elapsed.len();
    let total_secs: f64 = elapsed.iter().map(|d| d.total_active_secs).sum();
    let avg_secs = if days_elapsed > 0 {
        total_secs / days_elapsed as f64
    } else {
        0.0
    };
    let active_days = elapsed.iter().filter(|d| d.total_active_secs > 0.0).count();
    let best = elapsed
        .iter()
        .filter(|d| d.total_active_secs > 0.0)
        .max_by(|a, b| {
            a.total_active_secs
                .partial_cmp(&b.total_active_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|d| DayHours {
            date: d.date,
            weekday: d.weekday,
            hours_secs: d.total_active_secs,
        });
    WeekStats {
        total_secs,
        avg_secs,
        days_elapsed,
        active_days,
        best,
    }
}

pub(crate) fn highest_work_day(week: &[WeekDayBuckets]) -> Option<DayHours> {
    week.iter()
        .filter(|d| !d.is_future)
        .filter_map(|d| {
            let work = d
                .roots
                .iter()
                .find(|(name, _)| name == WORK_ROOT)
                .map(|(_, secs)| *secs)?;
            if work <= 0.0 {
                return None;
            }
            Some(DayHours {
                date: d.date,
                weekday: d.weekday,
                hours_secs: work,
            })
        })
        .max_by(|a, b| {
            a.hours_secs
                .partial_cmp(&b.hours_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn short_weekday(w: Weekday) -> &'static str {
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

fn format_month_day(date: NaiveDate) -> String {
    let month = match date.month() {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    };
    format!("{} {}", month, date.day())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Tab};
    use crate::theme::Theme;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::time::Instant;

    fn day(date: (i32, u32, u32), is_future: bool, roots: &[(&str, f64)]) -> WeekDayBuckets {
        let date = NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap();
        let weekday = date.weekday();
        let roots: Vec<(String, f64)> = roots.iter().map(|(n, s)| ((*n).to_string(), *s)).collect();
        let total = roots.iter().map(|(_, s)| *s).sum();
        WeekDayBuckets {
            date,
            weekday,
            is_future,
            roots,
            total_active_secs: total,
        }
    }

    fn fixture_week() -> Vec<WeekDayBuckets> {
        vec![
            day(
                (2026, 5, 4),
                false,
                &[("Work", 3600.0 * 4.0), ("Comms", 1800.0)],
            ),
            day(
                (2026, 5, 5),
                false,
                &[("Work", 3600.0 * 6.0), ("Browsing", 3600.0)],
            ),
            day((2026, 5, 6), false, &[("Work", 3600.0 * 5.0)]),
            day((2026, 5, 7), false, &[]), // empty past day
            day((2026, 5, 8), true, &[]),  // future
            day((2026, 5, 9), true, &[]),
            day((2026, 5, 10), true, &[]),
        ]
    }

    #[test]
    fn highest_work_day_picks_largest() {
        let week = fixture_week();
        let best = highest_work_day(&week).expect("should pick a Work day");
        assert_eq!(best.date, NaiveDate::from_ymd_opt(2026, 5, 5).unwrap());
        assert_eq!(best.hours_secs, 3600.0 * 6.0);
    }

    #[test]
    fn highest_work_day_returns_none_when_no_work() {
        let week = vec![
            day((2026, 5, 4), false, &[("Comms", 3600.0)]),
            day((2026, 5, 5), false, &[("Browsing", 3600.0)]),
            day((2026, 5, 6), true, &[]),
            day((2026, 5, 7), true, &[]),
            day((2026, 5, 8), true, &[]),
            day((2026, 5, 9), true, &[]),
            day((2026, 5, 10), true, &[]),
        ];
        assert!(highest_work_day(&week).is_none());
    }

    #[test]
    fn week_stats_skips_future_days_in_average() {
        let week = fixture_week();
        let stats = week_stats(&week);
        // Elapsed = 4 (Mon, Tue, Wed, Thu); active = 3 (Mon-Wed).
        assert_eq!(stats.days_elapsed, 4);
        assert_eq!(stats.active_days, 3);
        let total = 4.5 * 3600.0 + 7.0 * 3600.0 + 5.0 * 3600.0;
        assert!((stats.total_secs - total).abs() < 1e-6);
        assert!((stats.avg_secs - total / 4.0).abs() < 1e-6);
        let best = stats.best.expect("Tue is biggest");
        assert_eq!(best.date, NaiveDate::from_ymd_opt(2026, 5, 5).unwrap());
    }

    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn week_renders_full_layout() {
        use crate::data::{TopAppRow, TopDomainRow};
        use daylog_core::aggregate::CategorySummary;

        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let mut app = App::with_theme(theme);
        app.tab = Tab::Week;
        let now = Instant::now();
        app.data.week.apply_success(fixture_week(), now);
        app.data.week_top_apps.apply_success(
            vec![TopAppRow {
                name: "kitty".into(),
                duration_secs: 100_000.0,
            }],
            now,
        );
        app.data.week_top_categories.apply_success(
            vec![CategorySummary {
                name: vec!["Work".into(), "Programming".into()],
                duration: 200_000.0,
            }],
            now,
        );
        app.data.week_top_domains.apply_success(
            vec![TopDomainRow {
                domain: "github.com".into(),
                duration_secs: 50_000.0,
            }],
            now,
        );

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);

        assert!(
            rendered.contains("7-Day Activity Breakdown"),
            "chart panel title missing\n{rendered}"
        );
        assert!(
            rendered.contains("This week"),
            "stats card missing\n{rendered}"
        );
        assert!(
            rendered.contains("Total active"),
            "stats missing Total active"
        );
        assert!(
            rendered.contains("Stacked by category"),
            "legend missing\n{rendered}"
        );
        assert!(
            rendered.contains("Top apps") && rendered.contains("7 days"),
            "week top apps rollup missing\n{rendered}"
        );
        assert!(
            rendered.contains("Top categories") && rendered.contains("7 days"),
            "week top categories rollup missing\n{rendered}"
        );
        assert!(
            rendered.contains("Top domains") && rendered.contains("7 days"),
            "week top domains rollup missing\n{rendered}"
        );
        assert!(rendered.contains("kitty"), "fixture top app missing");
        assert!(
            rendered.contains("github.com"),
            "fixture top domain missing"
        );
        assert!(
            rendered.contains("Tue") || rendered.contains("(May 5)"),
            "callout missing weekday/date hint\n{rendered}"
        );
        assert!(
            rendered.contains("highest Work"),
            "callout text missing\n{rendered}"
        );

        // Stacked bars should paint at least two distinct chart bands.
        let mut saw_chart_1 = false;
        let mut saw_other = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let fg = buf[(x, y)].style().fg;
                if fg == Some(theme.chart_1) {
                    saw_chart_1 = true;
                }
                if matches!(
                    fg,
                    Some(c) if c == theme.chart_2 || c == theme.chart_4
                ) {
                    saw_other = true;
                }
            }
        }
        assert!(saw_chart_1, "Work bars should paint chart_1");
        assert!(
            saw_other,
            "expected Comms (chart_2) or Browsing (chart_4) bars"
        );
    }

    #[test]
    fn week_renders_skeleton_when_cache_empty() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let mut app = App::with_theme(theme);
        app.tab = Tab::Week;
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let rendered = buffer_to_string(terminal.backend().buffer());
        // Title should still appear; loading callout is suppressed.
        assert!(
            rendered.contains("7-Day Activity Breakdown"),
            "title still painted: {rendered}"
        );
        assert!(
            rendered.contains("Top apps") && rendered.contains("Top categories"),
            "rollup panel shells still painted: {rendered}"
        );
    }

    #[test]
    fn week_callout_when_no_activity_says_no_tracked_activity() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let mut app = App::with_theme(theme);
        app.tab = Tab::Week;
        let week: Vec<WeekDayBuckets> = vec![
            day((2026, 5, 4), false, &[]),
            day((2026, 5, 5), false, &[]),
            day((2026, 5, 6), false, &[]),
            day((2026, 5, 7), true, &[]),
            day((2026, 5, 8), true, &[]),
            day((2026, 5, 9), true, &[]),
            day((2026, 5, 10), true, &[]),
        ];
        app.data.week.apply_success(week, Instant::now());
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(
            rendered.contains("No tracked activity yet this week"),
            "expected the empty-week callout: {rendered}"
        );
    }
}
