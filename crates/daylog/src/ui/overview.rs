//! Stable panel shapes so first-load skeletons don't reflow when data lands.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table},
    Frame,
};

use throbber_widgets_tui::ThrobberState;

use crate::app::App;
use crate::data::{Cached, TopAppRow, TopDomainRow};
use crate::theme::{self, LayoutMode, Theme};
use crate::ui::{
    format_duration, kpi_strip, render_divider, render_section_header, render_skeleton_body,
    timeline,
};
use daylog_core::aggregate::CategorySummary;

/// BOLD UPPERCASE title inside the panel border, with optional in-flight `↻`.
/// Leading/trailing spaces keep the title from touching border characters.
pub(super) fn panel_title(theme: &Theme, base: &str, in_flight: bool) -> Line<'static> {
    let title_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let body = format!(" {} ", base.trim().to_uppercase());
    if in_flight {
        Line::from(vec![
            Span::styled(body, title_style),
            Span::styled("\u{21bb} ", Style::default().fg(theme.dim)),
        ])
    } else {
        Line::from(Span::styled(body, title_style))
    }
}

/// Bordered panel: dim rounded border, 1-col h-padding, 1-row top inset, BOLD title.
pub(super) fn panel_block(theme: &Theme, title: &str, in_flight: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(ratatui::widgets::Padding::new(1, 1, 1, 0))
        .title(panel_title(theme, title, in_flight))
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let layout_mode = Theme::layout_mode(area.width);

    // Borderless 4-band rhythm per DESIGN.md: snapshot → hero → rollups → hourly.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // snapshot strip
            Constraint::Length(1),  // gap before hero
            Constraint::Length(5),  // today timeline: title row + gap + barcode + ruler
            Constraint::Length(1),  // divider
            Constraint::Length(9),  // bordered rollups: 5 rows + header + borders
            Constraint::Length(1),  // divider
            Constraint::Length(6),  // hourly: header + margin + chart + axis
            Constraint::Min(0),     // flex blank
        ])
        .split(area);

    render_snapshot(f, chunks[0], app, layout_mode);
    render_timeline_section(f, chunks[2], app);
    render_divider(f, chunks[3], &app.theme);
    render_rollups(f, chunks[4], app);
    render_divider(f, chunks[5], &app.theme);
    render_hourly_section(f, chunks[6], app);
}

fn render_snapshot(f: &mut Frame, area: Rect, app: &App, layout_mode: LayoutMode) {
    let theme: &Theme = &app.theme;
    let kpi = app.data.kpi.value();
    let kpi_err = app.data.kpi.last_error();
    let row = Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    kpi_strip::render(f, row, theme, layout_mode, kpi, kpi_err);
}

fn render_timeline_section(f: &mut Frame, area: Rect, app: &App) {
    if area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let in_flight = app.data.timeline_events.is_in_flight();
    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    render_section_header(f, inner, theme, "Today \u{00b7} so far", in_flight);
    render_category_legend(
        f,
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
        theme,
    );

    let drop = 2u16.min(inner.height);
    let body = Rect {
        x: inner.x,
        y: inner.y.saturating_add(drop),
        width: inner.width,
        height: inner.height.saturating_sub(drop),
    };
    timeline::render(
        f,
        body,
        theme,
        app.data.timeline_events.value(),
        in_flight,
        &app.throbber,
    );
}

/// Inline category legend used on Today's title row (right-aligned) and
/// Week's activity card. Static set: shows every canonical root so the
/// visual signature stays stable across days regardless of which categories
/// happen to be present today.
fn render_category_legend(f: &mut Frame, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let dim = theme.dim_style();
    let entries: &[(&str, ratatui::style::Color)] = &[
        ("Work", theme.chart_1),
        ("Comms", theme.chart_2),
        ("Media", theme.chart_3),
        ("Browsing", theme.chart_4),
        ("Documents", theme.chart_5),
    ];
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (name, color)) in entries.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled("\u{25a0}", Style::default().fg(*color)));
        spans.push(Span::styled(format!(" {}", name), dim));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
        area,
    );
}

fn render_rollups(f: &mut Frame, area: Rect, app: &App) {
    let layout_mode = Theme::layout_mode(area.width);
    let cols = match layout_mode {
        LayoutMode::Wide => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(area),
        LayoutMode::Narrow => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area),
        LayoutMode::Stacked => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(area),
    };

    render_top_apps_panel(
        f,
        cols[0],
        &app.theme,
        &app.data.top_apps,
        "Top apps",
        &app.throbber,
    );
    if cols.len() >= 2 {
        render_top_categories_panel(
            f,
            cols[1],
            &app.theme,
            &app.data.top_categories,
            "Top categories",
            &app.throbber,
        );
    }
    if cols.len() >= 3 {
        render_top_domains_panel(
            f,
            cols[2],
            &app.theme,
            &app.data.top_domains,
            "Top domains",
            &app.throbber,
        );
    }
}

pub(super) fn render_top_apps_panel(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cache: &Cached<Vec<TopAppRow>>,
    title: &str,
    throbber: &ThrobberState,
) {
    if area.height == 0 {
        return;
    }
    let in_flight = cache.is_in_flight();
    let block = panel_block(theme, title, in_flight);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(rows) = cache.value() else {
        render_skeleton_body(f, inner, theme, throbber, in_flight);
        return;
    };

    if rows.is_empty() {
        let p = Paragraph::new("no app events yet").style(Style::default().fg(theme.dim));
        f.render_widget(p, inner);
        return;
    }

    let max_secs = rows.iter().map(|r| r.duration_secs).fold(0.0_f64, f64::max);

    let header = Row::new(vec![
        Cell::from("#").style(Style::default().fg(theme.dim)),
        Cell::from("App").style(Style::default().fg(theme.dim)),
        Cell::from("Active").style(Style::default().fg(theme.dim)),
        Cell::from(""),
    ])
    .height(1);

    let max_rows = inner.height.saturating_sub(1) as usize;
    let body_rows: Vec<Row> = rows
        .iter()
        .take(max_rows)
        .enumerate()
        .map(|(i, r)| top_app_row(i + 1, r, max_secs, theme))
        .collect();

    let widths = [
        Constraint::Length(3), // rank
        Constraint::Min(8),    // app name
        Constraint::Length(8), // active duration
        Constraint::Length(8), // proportional bar (matches categories + domains)
    ];
    let table = Table::new(body_rows, widths).header(header);
    f.render_widget(table, inner);
}

pub(super) fn top_app_row(
    rank: usize,
    row: &TopAppRow,
    max_secs: f64,
    theme: &Theme,
) -> Row<'static> {
    let bar = proportional_bar(row.duration_secs, max_secs, 8);
    Row::new(vec![
        Cell::from(format!("{}", rank)).style(Style::default().fg(theme.dim)),
        Cell::from(row.name.clone())
            .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Cell::from(format_duration(row.duration_secs)).style(Style::default().fg(theme.fg)),
        Cell::from(bar).style(Style::default().fg(theme.chart_3)),
    ])
}

pub(super) fn render_top_categories_panel(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cache: &Cached<Vec<CategorySummary>>,
    title: &str,
    throbber: &ThrobberState,
) {
    if area.height == 0 {
        return;
    }
    let in_flight = cache.is_in_flight();
    let block = panel_block(theme, title, in_flight);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(rows) = cache.value() else {
        render_skeleton_body(f, inner, theme, throbber, in_flight);
        return;
    };

    if rows.is_empty() {
        let p = Paragraph::new("no categorized events yet")
            .style(Style::default().fg(theme.dim));
        f.render_widget(p, inner);
        return;
    }

    let max_secs = rows.iter().map(|r| r.duration).fold(0.0_f64, f64::max);

    let header = Row::new(vec![
        Cell::from("#").style(Style::default().fg(theme.dim)),
        Cell::from("Category").style(Style::default().fg(theme.dim)),
        Cell::from("Active").style(Style::default().fg(theme.dim)),
        Cell::from(""),
    ])
    .height(1);

    let max_rows = inner.height.saturating_sub(1) as usize;
    let body_rows: Vec<Row> = rows
        .iter()
        .take(max_rows)
        .enumerate()
        .map(|(i, r)| category_row(i + 1, r, max_secs, theme))
        .collect();

    let widths = [
        Constraint::Length(3),  // rank
        Constraint::Min(18),    // category name — must fit "Work / Programming"
        Constraint::Length(8),  // active duration
        Constraint::Length(8),  // proportional bar (matches apps + domains)
    ];
    let table = Table::new(body_rows, widths).header(header);
    f.render_widget(table, inner);
}

/// Colour each bar by its category root so the column doubles as the legend.
pub(super) fn category_row(
    rank: usize,
    row: &CategorySummary,
    max_secs: f64,
    theme: &Theme,
) -> Row<'static> {
    let name = row.name.join(" / ");
    let root = row.name.first().map(String::as_str).unwrap_or("");
    let bar = proportional_bar(row.duration, max_secs, 8);
    let bar_color = theme.category_color(root);
    Row::new(vec![
        Cell::from(format!("{}", rank)).style(Style::default().fg(theme.dim)),
        Cell::from(name).style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Cell::from(format_duration(row.duration)).style(Style::default().fg(theme.fg)),
        Cell::from(bar).style(Style::default().fg(bar_color)),
    ])
}

pub(super) fn render_top_domains_panel(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cache: &Cached<Vec<TopDomainRow>>,
    title: &str,
    throbber: &ThrobberState,
) {
    if area.height == 0 {
        return;
    }
    let in_flight = cache.is_in_flight();
    let block = panel_block(theme, title, in_flight);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(rows) = cache.value() else {
        render_skeleton_body(f, inner, theme, throbber, in_flight);
        return;
    };

    if rows.is_empty() {
        // Empty Ok = "no aw-watcher-web bucket"; show the install hint.
        let lines = vec![
            Line::from(Span::styled(
                "no web watcher detected",
                Style::default().fg(theme.dim),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "install the firefox or chrome extension",
                Style::default().fg(theme.dim),
            )),
            Line::from(Span::styled(
                "to track domains and URLs",
                Style::default().fg(theme.dim),
            )),
        ];
        let p = Paragraph::new(lines);
        f.render_widget(p, inner);
        return;
    }

    let max_secs = rows.iter().map(|r| r.duration_secs).fold(0.0_f64, f64::max);

    let header = Row::new(vec![
        Cell::from("#").style(Style::default().fg(theme.dim)),
        Cell::from("Domain").style(Style::default().fg(theme.dim)),
        Cell::from("Active").style(Style::default().fg(theme.dim)),
        Cell::from(""),
    ])
    .height(1);

    let max_rows = inner.height.saturating_sub(1) as usize;
    let body_rows: Vec<Row> = rows
        .iter()
        .take(max_rows)
        .enumerate()
        .map(|(i, r)| top_domain_row(i + 1, r, max_secs, theme))
        .collect();

    let widths = [
        Constraint::Length(3),  // rank
        Constraint::Min(10),    // domain
        Constraint::Length(8),  // active
        Constraint::Length(8),  // bar (matches apps + categories)
    ];
    let table = Table::new(body_rows, widths).header(header);
    f.render_widget(table, inner);
}

pub(super) fn top_domain_row(
    rank: usize,
    row: &TopDomainRow,
    max_secs: f64,
    theme: &Theme,
) -> Row<'static> {
    let bar = proportional_bar(row.duration_secs, max_secs, 8);
    Row::new(vec![
        Cell::from(format!("{}", rank)).style(Style::default().fg(theme.dim)),
        Cell::from(row.domain.clone())
            .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Cell::from(format_duration(row.duration_secs)).style(Style::default().fg(theme.fg)),
        Cell::from(bar).style(Style::default().fg(theme.chart_4)),
    ])
}

/// Eighth-block proportional bar; zero rows get `·`, trailing cells get `░` so the track stays visible.
pub(super) fn proportional_bar(value: f64, max: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if max <= 0.0 || value <= 0.0 {
        let mut s = String::with_capacity(width);
        s.push('\u{00b7}'); // ·
        for _ in 1..width {
            s.push(' ');
        }
        return s;
    }

    const PARTIALS: [&str; 8] = [
        "",         // 0 folds into FULL track
        "\u{258f}", // ▏ 1/8
        "\u{258e}", // ▎ 2/8
        "\u{258d}", // ▍ 3/8
        "\u{258c}", // ▌ 4/8
        "\u{258b}", // ▋ 5/8
        "\u{258a}", // ▊ 6/8
        "\u{2589}", // ▉ 7/8
    ];
    const FULL: &str = "\u{2588}"; // █
    const EMPTY: &str = "\u{2591}"; // ░

    let frac = (value / max).clamp(0.0, 1.0);
    let mut total_eighths = (frac * (width as f64 * 8.0)).round() as usize;
    // Sub-eighth values still earn one visible 1/8 sliver — otherwise tiny
    // non-zero rows look identical to zero rows.
    if total_eighths == 0 {
        total_eighths = 1;
    }
    let total_eighths = total_eighths.min(width * 8);

    let full_cells = total_eighths / 8;
    let remainder_idx = total_eighths % 8;

    let mut out = String::with_capacity(width * 3);
    for _ in 0..full_cells {
        out.push_str(FULL);
    }
    let mut cells_drawn = full_cells;
    if remainder_idx > 0 && cells_drawn < width {
        out.push_str(PARTIALS[remainder_idx]);
        cells_drawn += 1;
    }
    for _ in cells_drawn..width {
        out.push_str(EMPTY);
    }
    out
}

fn render_hourly_section(f: &mut Frame, area: Rect, app: &App) {
    if area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let in_flight = app.data.hourly.is_in_flight();
    render_section_header(f, area, theme, "Active minutes per hour", in_flight);
    let drop = 2u16.min(area.height);
    let inner = Rect {
        x: area.x,
        y: area.y.saturating_add(drop),
        width: area.width,
        height: area.height.saturating_sub(drop),
    };

    let Some(buckets) = app.data.hourly.value() else {
        render_skeleton_body(f, inner, theme, &app.throbber, in_flight);
        return;
    };

    if buckets.is_empty() {
        let p = Paragraph::new("no hourly data").style(Style::default().fg(theme.dim));
        f.render_widget(p, inner);
        return;
    }

    // One Dataset per spectrum band; skip zero-minute hours so the marker doesn't paint a baseline sliver.
    let mut band_data: [Vec<(f64, f64)>; 5] =
        [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let mut max_min = 0.0_f64;
    for b in buckets {
        let minutes = (b.duration / 60.0).max(0.0);
        if minutes <= 0.0 {
            continue;
        }
        let h = b.hour as usize;
        let band_idx = match b.hour {
            0..=4 => 0,
            5..=9 => 1,
            10..=14 => 2,
            15..=19 => 3,
            _ => 4,
        };
        if minutes > max_min {
            max_min = minutes;
        }
        band_data[band_idx].push((h as f64, minutes));
    }

    let band_colors = [
        theme.chart_1,
        theme.chart_2,
        theme.chart_3,
        theme.chart_4,
        theme.chart_5,
    ];

    let datasets: Vec<Dataset> = band_data
        .iter()
        .enumerate()
        .map(|(i, data)| {
            Dataset::default()
                .data(data)
                .graph_type(GraphType::Bar)
                .marker(Marker::Block)
                .style(Style::default().fg(band_colors[i]))
        })
        .collect();

    // Round ceiling up to 30-min and floor at 60m so labels stay round and bars don't squish on empty days.
    let y_ceiling = ((max_min / 30.0).ceil() * 30.0).max(60.0);
    let y_mid = (y_ceiling / 2.0).round() as u64;
    let y_top = y_ceiling.round() as u64;
    let axis_style = Style::default().fg(theme.dim);

    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([0.0, 23.0])
                .labels(vec![
                    Span::styled("00", axis_style),
                    Span::styled("06", axis_style),
                    Span::styled("12", axis_style),
                    Span::styled("18", axis_style),
                    Span::styled("23", axis_style),
                ])
                .style(axis_style),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, y_ceiling])
                .labels(vec![
                    Span::styled("0", axis_style),
                    Span::styled(y_mid.to_string(), axis_style),
                    Span::styled(format!("{}m", y_top), axis_style),
                ])
                .style(axis_style),
        );
    f.render_widget(chart, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use daylog_core::aggregate::HourBucket;
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
        // top_domains: empty Ok = "no web watcher" hint. Tests don't
        // exercise the populated path here; that's covered by a unit
        // test inside the rendering module.
        app.data.top_domains.apply_success(vec![], now);
        // timeline_events: empty so the timeline renders the dim "·"
        // placeholder strip instead of the skeleton ellipsis.
        app.data.timeline_events.apply_success(vec![], now);
        app
    }

    /// Asserts by content, not byte-exact match, so layout tweaks don't cascade.
    #[test]
    fn overview_renders_full_layout() {
        let app = app_with_fixture();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let buf = terminal.backend().buffer().clone();
        let rendered = buffer_to_string(&buf);

        assert!(
            rendered.contains("Today"),
            "tab strip missing Today\n{rendered}"
        );

        assert!(
            rendered.contains("TODAY \u{00b7} SO FAR"),
            "missing today-timeline section header\n{rendered}"
        );

        assert!(rendered.contains("TOP APPS"), "missing TOP APPS section header");
        assert!(rendered.contains("kitty"), "missing kitty in top apps");
        assert!(rendered.contains("brave"), "missing brave in top apps");
        assert!(
            rendered.contains("4h 01m") || rendered.contains("4h 02m"),
            "expected duration label for kitty (~14487s = 4h 01m)\n{rendered}"
        );

        assert!(
            rendered.contains("TOP CATEGORIES"),
            "missing TOP CATEGORIES section header"
        );
        assert!(
            rendered.contains("Work / Programming"),
            "missing nested category label"
        );

        assert!(rendered.contains("TOP DOMAINS"), "missing TOP DOMAINS section header");
        assert!(
            rendered.contains("no web watcher"),
            "no-web-watcher hint missing in domains panel\n{rendered}"
        );

        assert!(
            rendered.contains("ACTIVE MINUTES PER HOUR"),
            "missing hourly-chart section header\n{rendered}"
        );

        assert!(
            !rendered.contains("tracker offline"),
            "fixture cache is healthy; offline indicator should be hidden"
        );
    }

    /// First-load skeleton: panel chrome renders before any data lands.
    #[test]
    fn overview_renders_skeleton_when_caches_empty() {
        let app = App::new();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(
            rendered.contains("TOP APPS"),
            "Top apps section header should render even with no data\n{rendered}"
        );
        assert!(
            rendered.contains("TODAY \u{00b7} SO FAR"),
            "Today-timeline header should render even with no data\n{rendered}"
        );
        assert!(
            rendered.contains("TOP DOMAINS"),
            "Top domains section header should render even with no data\n{rendered}"
        );
        assert!(
            !rendered.contains("kpi unavailable"),
            "first-load skeleton must not surface a 'kpi unavailable' banner\n{rendered}"
        );
    }

    #[test]
    fn overview_shows_offline_indicator_when_caches_offline() {
        let mut app = App::new();
        let now = Instant::now();
        // 3 = OFFLINE_THRESHOLD.
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

    /// Guards per-band colouring in the hourly chart.
    #[test]
    fn hourly_chart_paints_multiple_spectrum_bands() {
        use crate::theme::Theme;
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let mut app = App::with_theme(theme);
        let now = Instant::now();
        app.data.hourly.apply_success(
            (0..24)
                .map(|h| HourBucket {
                    hour: h,
                    duration: 1800.0,
                })
                .collect(),
            now,
        );

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::ui::render(f, &app))
            .expect("render frame");
        let buf = terminal.backend().buffer().clone();

        let mut saw_chart_1 = false;
        let mut saw_chart_5 = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let fg = buf[(x, y)].style().fg;
                if fg == Some(theme.chart_1) {
                    saw_chart_1 = true;
                }
                if fg == Some(theme.chart_5) {
                    saw_chart_5 = true;
                }
            }
        }
        assert!(
            saw_chart_1,
            "hourly chart should paint chart_1 (orange) for hours 0-4"
        );
        assert!(
            saw_chart_5,
            "hourly chart should paint chart_5 (violet) for hours 20-23"
        );
    }
}
