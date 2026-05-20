use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, RangeChip};
use crate::theme::Theme;
use crate::ui::{
    format_duration,
    overview::{
        panel_block, render_top_apps_panel, render_top_categories_panel,
        render_top_domains_panel,
    },
    render_divider, render_skeleton_body,
};

const HEATMAP_LABEL_GUTTER: usize = 3;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(1),
            Constraint::Min(8),
        ])
        .split(area);

    render_top_row(f, chunks[0], app);
    render_divider(f, chunks[1], &app.theme);
    render_rollup_row(f, chunks[2], app);
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
    if area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let in_flight = app.data.month_trailing_year.is_in_flight();
    let block = panel_block(theme, "Year heatmap", in_flight);
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
    month_label: Option<&'static str>,
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

    let mut label_chars = vec![' '; width.max(HEATMAP_LABEL_GUTTER)];
    let mut last_end = 0usize;
    for (c, col) in columns.iter().enumerate() {
        if let Some(label) = col.month_label {
            let natural_start = HEATMAP_LABEL_GUTTER + c;
            let start = natural_start.max(last_end + 1);
            let label_len = label.chars().count();
            if start + label_len > label_chars.len() {
                break;
            }
            for (i, ch) in label.chars().enumerate() {
                if let Some(cell) = label_chars.get_mut(start + i) {
                    *cell = ch;
                }
            }
            last_end = start + label_len;
        }
    }
    let label_str: String = label_chars.into_iter().collect();
    lines.push(Line::from(Span::styled(
        label_str,
        Style::default().fg(theme.dim),
    )));

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
    if area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let in_flight = app.data.month_trailing_year.is_in_flight();
    let block = panel_block(theme, "This month", in_flight);
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

    let lines = vec![
        row("Total active", format_duration(stats.total_secs), None),
        Line::from(""),
        row("Daily avg", format_duration(stats.avg_secs), avg_hint),
        Line::from(""),
        row("Best day", best_value, best_hint),
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

/// Index 0 = today_active; trailing[i] = days_ago i+1. Average is over
/// active days only.
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

        // Future cells reserved through month-end, so today may not be in the final column.
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
        assert!(
            labels.len() >= 12,
            "expected ≥12 month labels, got {:?}",
            labels
        );
    }

    #[test]
    fn month_stats_averages_only_over_active_days() {
        let mut trailing = vec![0.0; 29];
        trailing[0] = 4.0 * 3600.0;
        let stats = month_stats(2.0 * 3600.0, &trailing);
        assert_eq!(stats.total_secs, 6.0 * 3600.0);
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

    #[test]
    fn month_renders_full_layout() {
        use crate::app::{App, Tab};
        use crate::cache::{TopAppRow, TopDomainRow};
        use crate::data::aggregate::CategorySummary;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        use std::time::Instant;

        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let mut app = App::with_theme(theme);
        app.tab = Tab::Month;
        let now = Instant::now();

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

        assert!(rendered.contains("Month"), "Month not in tab strip");
        assert!(
            rendered.contains("YEAR HEATMAP"),
            "missing YEAR HEATMAP section header\n{rendered}"
        );
        assert!(
            rendered.contains("THIS MONTH"),
            "missing THIS MONTH section header"
        );
        assert!(
            rendered.contains("Best day") && rendered.contains("Daily avg"),
            "this-month stats missing\n{rendered}"
        );
        assert!(
            rendered.contains("TOP APPS"),
            "missing TOP APPS section header"
        );
        assert!(
            rendered.contains("TOP CATEGORIES"),
            "missing TOP CATEGORIES section header"
        );
        assert!(
            rendered.contains("TOP DOMAINS"),
            "missing TOP DOMAINS section header"
        );
        assert!(rendered.contains("kitty"), "fixture top app missing");
        assert!(
            rendered.contains("github.com"),
            "fixture top domain missing"
        );
        assert!(
            rendered.contains('\u{2588}'),
            "heatmap painted no full-block cells"
        );
    }
}
