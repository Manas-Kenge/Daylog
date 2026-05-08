//! Today tab — three widgets in a vertical stack:
//!   1. Top apps table (left half)  +  Top categories list (right half)
//!   2. Hourly distribution bar chart (full width)

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::App;
use crate::data::TopAppRow;
use crate::ui::format_duration;
use daylog_core::aggregate::{CategorySummary, HourBucket};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // top apps + categories row
            Constraint::Min(0),     // hourly chart
        ])
        .split(area);

    render_top_row(f, chunks[0], app);
    render_hourly(f, chunks[1], app);
}

fn render_top_row(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_top_apps(f, cols[0], app);
    render_top_categories(f, cols[1], app);
}

fn render_top_apps(f: &mut Frame, area: Rect, app: &App) {
    let title = title_with_status(" Top apps ", app.data.top_apps.is_in_flight());
    let block = Block::default().borders(Borders::ALL).title(title);

    let Some(rows) = app.data.top_apps.value() else {
        let inner_msg = if app.data.top_apps.last_error().is_some() {
            "fetch error · check footer"
        } else {
            "loading…"
        };
        let p = Paragraph::new(inner_msg)
            .block(block)
            .style(Style::default().dim());
        f.render_widget(p, area);
        return;
    };

    if rows.is_empty() {
        let p = Paragraph::new("no app events yet")
            .block(block)
            .style(Style::default().dim());
        f.render_widget(p, area);
        return;
    }

    let max_secs = rows.iter().map(|r| r.duration_secs).fold(0.0_f64, f64::max);
    let ember = app.theme.ember;
    let table_rows: Vec<Row> = rows
        .iter()
        .take(area.height.saturating_sub(2) as usize)
        .map(|r| top_app_row(r, max_secs, ember))
        .collect();

    let widths = [
        Constraint::Min(12),
        Constraint::Length(16),
        Constraint::Length(10),
    ];
    let table = Table::new(table_rows, widths).block(block);
    f.render_widget(table, area);
}

fn top_app_row(row: &TopAppRow, max_secs: f64, bar_color: ratatui::style::Color) -> Row<'static> {
    let bar_width: usize = 14;
    let filled = if max_secs > 0.0 {
        ((row.duration_secs / max_secs) * bar_width as f64).round() as usize
    } else {
        0
    };
    let bar = format!(
        "{}{}",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(bar_width.saturating_sub(filled))
    );
    Row::new(vec![
        Cell::from(row.name.clone()).style(Style::default().bold()),
        Cell::from(bar).style(Style::default().fg(bar_color)),
        Cell::from(format_duration(row.duration_secs)),
    ])
}

fn render_top_categories(f: &mut Frame, area: Rect, app: &App) {
    let title = title_with_status(" Top categories ", app.data.top_categories.is_in_flight());
    let block = Block::default().borders(Borders::ALL).title(title);

    let Some(rows) = app.data.top_categories.value() else {
        let p = Paragraph::new("loading…")
            .block(block)
            .style(Style::default().dim());
        f.render_widget(p, area);
        return;
    };

    if rows.is_empty() {
        let p = Paragraph::new("no categorized events yet")
            .block(block)
            .style(Style::default().dim());
        f.render_widget(p, area);
        return;
    }

    let table_rows: Vec<Row> = rows
        .iter()
        .take(area.height.saturating_sub(2) as usize)
        .map(category_row)
        .collect();

    let widths = [Constraint::Min(10), Constraint::Length(10)];
    let table = Table::new(table_rows, widths).block(block);
    f.render_widget(table, area);
}

fn category_row(row: &CategorySummary) -> Row<'static> {
    let name = row.name.join(" / ");
    Row::new(vec![
        Cell::from(name).style(Style::default().bold()),
        Cell::from(format_duration(row.duration)),
    ])
}

fn render_hourly(f: &mut Frame, area: Rect, app: &App) {
    let title = title_with_status(" Hourly (today) ", app.data.hourly.is_in_flight());
    let block = Block::default().borders(Borders::ALL).title(title);

    let Some(buckets) = app.data.hourly.value() else {
        let p = Paragraph::new("loading…")
            .block(block)
            .style(Style::default().dim());
        f.render_widget(p, area);
        return;
    };

    if buckets.is_empty() {
        let p = Paragraph::new("no hourly data")
            .block(block)
            .style(Style::default().dim());
        f.render_widget(p, area);
        return;
    }

    // BarChart wants &[(&str, u64)]. Convert seconds → minutes for readable
    // bar heights at typical terminal sizes.
    let labels: Vec<String> = (0..24).map(|h| format!("{:02}", h)).collect();
    let data: Vec<(&str, u64)> = buckets
        .iter()
        .filter_map(|b: &HourBucket| {
            let label = labels.get(b.hour as usize)?;
            Some((label.as_str(), (b.duration / 60.0).round() as u64))
        })
        .collect();

    // Phase 8 will replace this single-color fill with per-bar spectrum
    // colours via BarGroup. For now we paint with chart_3 (afternoon green)
    // as a neutral placeholder so the cyan-everywhere look is gone.
    let placeholder = app.theme.chart_3;
    let chart = BarChart::default()
        .block(block)
        .data(&data)
        .bar_width(2)
        .bar_gap(1)
        .bar_style(Style::default().fg(placeholder))
        .value_style(Style::default().fg(app.theme.fg).bg(placeholder));
    f.render_widget(chart, area);
}

fn title_with_status<'a>(base: &'a str, in_flight: bool) -> Line<'a> {
    if in_flight {
        Line::from(vec![
            Span::raw(base),
            Span::styled("\u{21bb}", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(" "),
        ])
    } else {
        Line::from(base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::time::Instant;

    /// Builds an App pre-populated with fixture data for snapshot tests.
    /// Bypasses the network — applies success directly to each Cached<T>.
    fn app_with_fixture() -> App {
        let mut app = App::new();
        let now = Instant::now();
        app.data.top_apps.apply_success(
            vec![
                TopAppRow {
                    name: "kitty".into(),
                    duration_secs: 14487.0,
                },
                TopAppRow {
                    name: "brave".into(),
                    duration_secs: 11561.0,
                },
                TopAppRow {
                    name: "code".into(),
                    duration_secs: 1400.0,
                },
            ],
            now,
        );
        app.data.top_categories.apply_success(
            vec![
                CategorySummary {
                    name: vec!["Work".into(), "Programming".into()],
                    duration: 16000.0,
                },
                CategorySummary {
                    name: vec!["Browsing".into()],
                    duration: 11500.0,
                },
                CategorySummary {
                    name: vec!["Uncategorized".into()],
                    duration: 800.0,
                },
            ],
            now,
        );
        app.data.hourly.apply_success(
            (0..24)
                .map(|h| HourBucket {
                    hour: h,
                    duration: if (9..18).contains(&h) { 1800.0 } else { 0.0 },
                })
                .collect(),
            now,
        );
        app
    }

    /// Snapshot test per decision 3D: render the Today tab to a TestBackend
    /// and assert presence of the key visual elements. We assert by
    /// content rather than byte-exact match so trivial layout tweaks
    /// don't cascade into a thousand expected-string updates.
    #[test]
    fn overview_renders_top_apps_categories_and_hourly() {
        let app = app_with_fixture();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);

        // Tab strip
        assert!(
            rendered.contains("Today"),
            "tab strip missing Today\n{rendered}"
        );

        // Top apps panel: title + at least one app name + a duration label
        assert!(rendered.contains("Top apps"), "missing Top apps title");
        assert!(rendered.contains("kitty"), "missing kitty in top apps");
        assert!(rendered.contains("brave"), "missing brave in top apps");
        assert!(
            rendered.contains("4h 01m") || rendered.contains("4h 02m"),
            "expected duration label for kitty (~14487s = 4h 01m)\n{rendered}"
        );

        // Top categories panel
        assert!(
            rendered.contains("Top categories"),
            "missing Top categories title"
        );
        assert!(
            rendered.contains("Work / Programming"),
            "missing nested category label"
        );

        // Hourly chart
        assert!(rendered.contains("Hourly"), "missing Hourly title");

        // Footer hints
        assert!(
            rendered.contains("Tab cycle") && rendered.contains("q quit"),
            "footer hints missing"
        );

        // Offline indicator must NOT show in fixture (everything succeeded)
        assert!(
            !rendered.contains("tracker offline"),
            "fixture cache is healthy; offline indicator should be hidden"
        );
    }

    #[test]
    fn overview_renders_loading_when_caches_empty() {
        // Fresh App: no data has resolved yet. Each panel should show
        // its loading skeleton instead of crashing or rendering blank.
        let app = App::new();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(rendered.contains("loading"), "expected at least one loading skeleton");
    }

    #[test]
    fn overview_shows_offline_indicator_when_caches_offline() {
        let mut app = App::new();
        let now = Instant::now();
        // 3 failures crosses OFFLINE_THRESHOLD per decision 1C.
        for _ in 0..3 {
            app.data.top_apps.apply_failure("conn refused".into(), now);
        }
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(
            rendered.contains("tracker offline"),
            "footer should surface tracker-offline indicator after 3 failures\n{rendered}"
        );
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
}
