//! Month tab — desktop-parity port of `src/pages/MonthPage.tsx`.
//!
//! Two vertical bands inside the body:
//!   1. Year heatmap (~53 weeks × 7 weekdays) + "This month" stat card
//!      — Length(11), 70 / 30 horizontal split.
//!   2. Top apps · 30d + Top categories · 30d + Top domains · 30d
//!      — Min(8), 34 / 33 / 33 horizontal split.
//!
//! The heatmap is GitHub-style: weekday rows (Sun..Sat) × ~53 weekly
//! columns covering the trailing 365 days. Density is binned into four
//! ember-styled Unicode block characters so the gradient survives both
//! truecolor and 256-colour terminals; today's cell is REVERSED+BOLD so
//! it survives even when its underlying intensity is empty.
//!
//! Range chips don't drive this tab — `ui.rs::render_range_chips` dims
//! the row and appends a "trailing 30d · year overview" hint when the
//! Month tab is active. The four backing cache slots
//! (`month_trailing_year`, `month_top_apps`, `month_top_categories`,
//! `month_top_domains`) live on `DataCache` and are dispatched only
//! when `app.tab == Tab::Month` so Today's TTFF isn't taxed by the
//! 365-day fan-out fetch.

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, RangeChip};
use crate::theme::{self, Theme};
use crate::ui::{
    format_duration,
    overview::{
        panel_title, render_top_apps_panel, render_top_categories_panel, render_top_domains_panel,
    },
    render_skeleton_body,
};

/// Width reserved on the heatmap's left for the Mon/Wed/Fri labels —
/// "M  ", "W  ", "F  ", or "   " — so cells line up below the month
/// abbreviations on the top row.
const HEATMAP_LABEL_GUTTER: usize = 3;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11), // year heatmap (1 month-label + 7 weekdays + 2 borders + slack)
            Constraint::Min(8),     // 30-day rollup row
        ])
        .split(area);

    render_top_row(f, chunks[0], app);
    render_rollup_row(f, chunks[1], app);
}

fn render_top_row(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);
    render_heatmap(f, cols[0], app);
    render_this_month(f, cols[1], app);
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
        &app.data.month_top_apps,
        " Top apps · 30d ",
        &app.throbber,
    );
    render_top_categories_panel(
        f,
        cols[1],
        &app.theme,
        &app.data.month_top_categories,
        " Top categories · 30d ",
        &app.throbber,
    );
    render_top_domains_panel(
        f,
        cols[2],
        &app.theme,
        &app.data.month_top_domains,
        " Top domains · 30d ",
        &app.throbber,
    );
}

fn render_heatmap(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let in_flight = app.data.month_trailing_year.is_in_flight();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING_TIGHT)
        .title(panel_title(theme, " Year heatmap ", in_flight));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(trailing) = app.data.month_trailing_year.value() else {
        render_skeleton_body(f, inner, theme, &app.throbber, in_flight);
        return;
    };

    let today = Local::now().date_naive();
    let today_active = today_active_secs(app);
    let columns = build_columns(today, today_active, trailing);
    let max_secs = columns
        .iter()
        .flat_map(|c| c.cells.iter().filter_map(|c| c.as_ref()))
        .map(|c| c.active_secs)
        .fold(0.0_f64, f64::max);

    let lines = render_heatmap_lines(theme, &columns, max_secs, inner.width as usize);
    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

/// Today's value comes from the live KPI slot when the user has the
/// Today chip selected (the common case). Otherwise the heatmap
/// renders today at zero — better to under-report by half a day than
/// to splice in an unrelated chip's KPI value.
fn today_active_secs(app: &App) -> f64 {
    if app.range_chip == RangeChip::Today {
        app.data.kpi.value().map(|k| k.active_secs).unwrap_or(0.0)
    } else {
        0.0
    }
}

#[derive(Debug, Clone)]
struct HeatmapCell {
    active_secs: f64,
    is_today: bool,
}

#[derive(Debug, Clone)]
struct HeatmapColumn {
    /// Month abbreviation if this column starts a new month vs. the
    /// previous labeled column. Drives the top label row.
    month_label: Option<&'static str>,
    /// Index 0 = Sunday … 6 = Saturday. `None` is a cell outside the
    /// 365-day window (future, or before the window's start).
    cells: [Option<HeatmapCell>; 7],
}

fn build_columns(today: NaiveDate, today_active_secs: f64, trailing: &[f64]) -> Vec<HeatmapColumn> {
    let start = today - ChronoDuration::days(364); // inclusive: 365-day window
    let end = last_day_of_month(today);
    let start_dow = start.weekday().num_days_from_sunday() as i64;
    let week_start = start - ChronoDuration::days(start_dow);
    let total_days = (end - week_start).num_days() + 1;
    let num_cols = ((total_days + 6) / 7) as usize;

    let mut cols: Vec<HeatmapColumn> = Vec::with_capacity(num_cols);
    let mut prev_month: Option<u32> = None;

    for c in 0..num_cols {
        let col_start = week_start + ChronoDuration::days(c as i64 * 7);
        let mut cells: [Option<HeatmapCell>; 7] = [None, None, None, None, None, None, None];
        for r in 0..7 {
            let date = col_start + ChronoDuration::days(r as i64);
            if date < start || date > today {
                continue;
            }
            let secs = if date == today {
                today_active_secs
            } else {
                let days_ago = (today - date).num_days() as usize;
                trailing.get(days_ago - 1).copied().unwrap_or(0.0)
            };
            cells[r] = Some(HeatmapCell {
                active_secs: secs,
                is_today: date == today,
            });
        }
        // Label this column if any cell falls inside the visible window
        // AND its month differs from the previous labeled column.
        let first_in_window = (0..7).find_map(|r| {
            let d = col_start + ChronoDuration::days(r);
            (d >= start && d <= today).then_some(d)
        });
        let label = match first_in_window {
            Some(d) if Some(d.month()) != prev_month => {
                prev_month = Some(d.month());
                Some(month_abbr(d.month()))
            }
            _ => None,
        };
        cols.push(HeatmapColumn {
            month_label: label,
            cells,
        });
    }
    cols
}

fn last_day_of_month(date: NaiveDate) -> NaiveDate {
    let (year, month) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).unwrap() - ChronoDuration::days(1)
}

fn month_abbr(m: u32) -> &'static str {
    match m {
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
        12 => "Dec",
        _ => "???",
    }
}

fn render_heatmap_lines(
    theme: &Theme,
    columns: &[HeatmapColumn],
    max_secs: f64,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(8);

    // Month label row spans `width` chars; cells live in column index
    // `gutter + col_index` so labels align with the column's leftmost
    // cell. Wider terminals pad with trailing spaces; narrower ones
    // clip — the heatmap below clips the same way.
    let mut label_chars = vec![' '; width.max(HEATMAP_LABEL_GUTTER)];
    for (c, col) in columns.iter().enumerate() {
        if let Some(label) = col.month_label {
            let start = HEATMAP_LABEL_GUTTER + c;
            for (i, ch) in label.chars().enumerate() {
                if let Some(cell) = label_chars.get_mut(start + i) {
                    *cell = ch;
                }
            }
        }
    }
    let label_str: String = label_chars.into_iter().collect();
    lines.push(Line::from(Span::styled(
        label_str,
        Style::default().fg(theme.dim),
    )));

    // Seven weekday rows, gutter-prefixed. M / W / F labels only — the
    // canonical GitHub heatmap convention. Tue / Thu / Sat / Sun read
    // by position so labelling all seven would be redundant noise.
    for weekday in 0..7 {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(columns.len() + 1);
        let gutter = match weekday {
            1 => "M  ",
            3 => "W  ",
            5 => "F  ",
            _ => "   ",
        };
        spans.push(Span::styled(gutter, Style::default().fg(theme.dim)));
        for col in columns {
            let cell = col.cells[weekday].as_ref();
            let span = match cell {
                Some(c) => density_span(theme, c.active_secs, max_secs, c.is_today),
                None => Span::styled(" ", Style::default().fg(theme.border_dim)),
            };
            spans.push(span);
        }
        lines.push(Line::from(spans));
    }

    lines
}

/// Map a cell's active-seconds intensity to one of four Unicode block
/// densities painted in `theme.ember`. Empty-but-tracked days render
/// `·` in `border_dim` so they read as "tracked, no activity" — visually
/// distinct from out-of-window blanks. Today gets REVERSED+BOLD on top
/// of whatever density it landed on so it remains findable on a
/// zero-activity morning.
fn density_span(theme: &Theme, secs: f64, max_secs: f64, is_today: bool) -> Span<'static> {
    let mut style = Style::default();
    let ch: char = if secs <= 0.0 || max_secs <= 0.0 {
        style = style.fg(theme.border_dim);
        '\u{00b7}'
    } else {
        let ratio = (secs / max_secs).min(1.0);
        let c = if ratio < 0.25 {
            '\u{2591}'
        } else if ratio < 0.5 {
            '\u{2592}'
        } else if ratio < 0.75 {
            '\u{2593}'
        } else {
            '\u{2588}'
        };
        style = style.fg(theme.ember);
        c
    };
    if is_today {
        style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
    }
    Span::styled(ch.to_string(), style)
}

fn render_this_month(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let in_flight = app.data.month_trailing_year.is_in_flight();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING)
        .title(panel_title(theme, " This month ", in_flight));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(trailing) = app.data.month_trailing_year.value() else {
        render_skeleton_body(f, inner, theme, &app.throbber, in_flight);
        return;
    };

    let stats = month_stats(today_active_secs(app), trailing);
    let today = Local::now().date_naive();
    let label_style = Style::default().fg(theme.dim);
    let value_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);

    let row = |label: &'static str, value: String, hint: Option<String>| {
        let mut spans = vec![
            Span::styled(format!(" {:<14}", label), label_style),
            Span::styled(value, value_style),
        ];
        if let Some(h) = hint {
            spans.push(Span::styled(
                format!("  {}", h),
                Style::default().fg(theme.dim),
            ));
        }
        Line::from(spans)
    };

    let avg_hint =
        (stats.active_days > 0).then_some(format!("over {} active days", stats.active_days));
    let best_value = stats
        .best
        .as_ref()
        .map(|b| format_duration(b.active_secs))
        .unwrap_or_else(|| "\u{2014}".to_string());
    let best_hint = stats
        .best
        .as_ref()
        .map(|b| format_day_hint(today - ChronoDuration::days(b.days_ago as i64)));
    let streak_value = if stats.streak > 0 {
        format!("{} days", stats.streak)
    } else {
        "\u{2014}".to_string()
    };

    let lines = vec![
        row("Total active", format_duration(stats.total_secs), None),
        row("Daily avg", format_duration(stats.avg_secs), avg_hint),
        row("Best day", best_value, best_hint),
        row("Current streak", streak_value, None),
        row("Active days", format!("{}/30", stats.active_days), None),
    ];
    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

#[derive(Debug, Clone, PartialEq)]
struct MonthStats {
    total_secs: f64,
    avg_secs: f64,
    active_days: usize,
    best: Option<BestDay>,
    streak: u32,
}

#[derive(Debug, Clone, PartialEq)]
struct BestDay {
    days_ago: usize,
    active_secs: f64,
}

/// Headline stats for the trailing 30 days. Index 0 is `today_active`;
/// `trailing[i]` is days_ago = i + 1 (so `trailing[0]` = yesterday).
/// Average is over **active** days only — a pile of zero-activity
/// weekend days shouldn't drag down the headline metric.
fn month_stats(today_active: f64, trailing: &[f64]) -> MonthStats {
    let mut days: Vec<f64> = Vec::with_capacity(30);
    days.push(today_active);
    for i in 0..29 {
        days.push(trailing.get(i).copied().unwrap_or(0.0));
    }
    let total: f64 = days.iter().sum();
    let active_count = days.iter().filter(|&&v| v > 0.0).count();
    let avg = if active_count > 0 {
        total / active_count as f64
    } else {
        0.0
    };
    let best = days
        .iter()
        .enumerate()
        .filter(|(_, secs)| **secs > 0.0)
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(days_ago, secs)| BestDay {
            days_ago,
            active_secs: *secs,
        });

    // Streak counts consecutive non-zero days walking back from today.
    // Mirrors the desktop's strict rule: a fresh-launched morning with
    // no events yet starts the streak at 0.
    let mut streak = 0_u32;
    for v in &days {
        if *v > 0.0 {
            streak += 1;
        } else {
            break;
        }
    }

    MonthStats {
        total_secs: total,
        avg_secs: avg,
        active_days: active_count,
        best,
        streak,
    }
}

fn format_day_hint(date: NaiveDate) -> String {
    format!(
        "{} {}",
        short_weekday(date.weekday()),
        format_month_day(date)
    )
}

fn short_weekday(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    }
}

fn format_month_day(date: NaiveDate) -> String {
    format!("{} {}", month_abbr(date.month()), date.day())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, NaiveDate};

    #[test]
    fn build_columns_covers_full_year_window() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).unwrap();
        let trailing = vec![0.0; 365];
        let cols = build_columns(today, 0.0, &trailing);
        let start = today - ChronoDuration::days(364);
        let week_start =
            start - ChronoDuration::days(start.weekday().num_days_from_sunday() as i64);
        let end = last_day_of_month(today);
        let expected_cols = (((end - week_start).num_days() + 1) + 6) / 7;
        assert_eq!(cols.len(), expected_cols as usize);

        // Today's cell must be flagged somewhere in the visible grid.
        // It is not necessarily in the final column because the desktop
        // layout reserves future cells through the end of this month.
        let today_dow = today.weekday().num_days_from_sunday() as usize;
        assert!(
            cols.iter()
                .any(|col| col.cells[today_dow].as_ref().is_some_and(|c| c.is_today)),
            "today cell missing from grid"
        );
    }

    #[test]
    fn build_columns_reserves_future_cells_through_current_month() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).unwrap();
        let trailing = vec![0.0; 365];
        let cols = build_columns(today, 0.0, &trailing);
        let start = today - ChronoDuration::days(364);
        let week_start =
            start - ChronoDuration::days(start.weekday().num_days_from_sunday() as i64);
        let without_future_end = (((today - week_start).num_days() + 1) + 6) / 7;
        assert!(
            cols.len() as i64 > without_future_end,
            "current-month future cells should add at least one column"
        );
    }

    #[test]
    fn build_columns_emits_at_least_twelve_month_labels() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).unwrap();
        let trailing = vec![0.0; 365];
        let cols = build_columns(today, 0.0, &trailing);
        let labels: Vec<&'static str> = cols.iter().filter_map(|c| c.month_label).collect();
        // 365 days always crosses 12+ month boundaries.
        assert!(
            labels.len() >= 12,
            "expected ≥12 month labels, got {:?}",
            labels
        );
    }

    #[test]
    fn month_stats_averages_only_over_active_days() {
        // today=2h, yesterday=4h, rest=0.
        let mut trailing = vec![0.0; 29];
        trailing[0] = 4.0 * 3600.0;
        let stats = month_stats(2.0 * 3600.0, &trailing);
        assert_eq!(stats.total_secs, 6.0 * 3600.0);
        // Average over 2 active days = 3h.
        assert_eq!(stats.avg_secs, 3.0 * 3600.0);
        assert_eq!(stats.active_days, 2);
        assert_eq!(
            stats.best,
            Some(BestDay {
                days_ago: 1,
                active_secs: 4.0 * 3600.0,
            })
        );
        assert_eq!(stats.streak, 2);
    }

    #[test]
    fn month_stats_empty_input_is_zero() {
        let trailing: Vec<f64> = vec![];
        let stats = month_stats(0.0, &trailing);
        assert_eq!(stats.total_secs, 0.0);
        assert_eq!(stats.avg_secs, 0.0);
        assert_eq!(stats.active_days, 0);
        assert_eq!(stats.best, None);
        assert_eq!(stats.streak, 0);
    }

    #[test]
    fn density_span_renders_mid_dot_for_zero_max() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let span = density_span(&theme, 0.0, 0.0, false);
        assert_eq!(span.content.as_ref(), "\u{00b7}");
    }

    #[test]
    fn density_span_picks_full_block_at_max_intensity() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let span = density_span(&theme, 1.0, 1.0, false);
        assert_eq!(span.content.as_ref(), "\u{2588}");
    }

    /// Snapshot smoke test: render the Month tab end-to-end at full
    /// width and assert each of the five visual surfaces (heatmap, this-
    /// month card, three rollup panels) lands its title or a fixture
    /// data point. Buys cheap regression coverage against rename / mod-
    /// dispatch typos without locking in byte-exact layout.
    #[test]
    fn month_renders_full_layout() {
        use crate::app::{App, Tab};
        use crate::data::{TopAppRow, TopDomainRow};
        use daylog_core::aggregate::CategorySummary;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        use std::time::Instant;

        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let mut app = App::with_theme(theme);
        app.tab = Tab::Month;
        let now = Instant::now();

        // Trailing year: small ramp so the heatmap has visible variance
        // without needing 365 distinct values.
        let year: Vec<f64> = (0..365).map(|i| (i % 7) as f64 * 600.0).collect();
        app.data.month_trailing_year.apply_success(year, now);
        app.data.month_top_apps.apply_success(
            vec![TopAppRow {
                name: "kitty".into(),
                duration_secs: 100_000.0,
            }],
            now,
        );
        app.data.month_top_categories.apply_success(
            vec![CategorySummary {
                name: vec!["Work".into(), "Programming".into()],
                duration: 200_000.0,
            }],
            now,
        );
        app.data.month_top_domains.apply_success(
            vec![TopDomainRow {
                domain: "github.com".into(),
                duration_secs: 50_000.0,
            }],
            now,
        );

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let buf = terminal.backend().buffer().clone();
        let rendered = {
            let mut out = String::new();
            for y in 0..buf.area.height {
                for x in 0..buf.area.width {
                    out.push_str(buf[(x, y)].symbol());
                }
                out.push('\n');
            }
            out
        };

        // Tab strip is showing Month
        assert!(rendered.contains("Month"), "Month not in tab strip");
        // Heatmap title
        assert!(
            rendered.contains("Year heatmap"),
            "missing 'Year heatmap' title\n{rendered}"
        );
        // This-month card
        assert!(
            rendered.contains("This month"),
            "missing 'This month' title"
        );
        assert!(
            rendered.contains("Active days") && rendered.contains("Best day"),
            "expanded month stats missing"
        );
        // Rollup row titles
        assert!(
            rendered.contains("Top apps"),
            "missing 'Top apps' rollup title"
        );
        assert!(
            rendered.contains("Top categories"),
            "missing 'Top categories' rollup title"
        );
        assert!(
            rendered.contains("Top domains"),
            "missing 'Top domains' rollup title"
        );
        // Fixture data points landed
        assert!(rendered.contains("kitty"), "fixture top app missing");
        assert!(
            rendered.contains("github.com"),
            "fixture top domain missing"
        );
        // Heatmap painted at least one full-block char somewhere.
        assert!(
            rendered.contains('\u{2588}'),
            "heatmap painted no full-block cells"
        );
    }
}
