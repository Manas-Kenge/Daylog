//! Today tab — desktop-parity layout.
//!
//! Vertical bands inside the body, sized to their content (no panel
//! stretches to fill the terminal):
//!   1. KPI strip                            — 1 row, full-width
//!   2. Today's timeline (24h barcode)       — 6 rows, full-width
//!   3. Top apps  +  Top categories          — 11 rows, split 50 / 50
//!   4. Hourly distribution + Top domains    — 10 rows, hourly fixed 46 cols
//!   5. 7-day sparkline                      — 1 row, Wide only
//!   6. Flex blank                           — soaks up leftover terminal rows
//!
//! This mirrors `src/pages/Overview.tsx` minus the WeekHeatmap (which
//! lives on the Week tab). Every panel has a stable shape so first-load
//! skeletons don't reflow when data lands. Errors surface in the footer
//! pill, not as inline banners.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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
use crate::ui::{format_duration, kpi_strip, render_skeleton_body, sparkline, timeline};
use daylog_core::aggregate::CategorySummary;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let layout_mode = Theme::layout_mode(area.width);

    // Sparkline only renders Wide. On Narrow / Stacked the slot collapses
    // so the panels above absorb the room. Wide now reserves 3 rows so the
    // sparkline can sit in a bordered panel instead of as an orphan strip.
    let sparkline_height = match layout_mode {
        LayoutMode::Wide => 3,
        _ => 0,
    };

    // Each panel gets exactly the rows it needs; the trailing Min(0)
    // soaks up leftover terminal height so panels don't stretch. On
    // terminals shorter than the body's natural height (~30 inner rows
    // for the Today tab) the bottom panels clip — accepted trade-off
    // for the bare-minimum density the user asked for.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                // KPI strip (bordered)
            Constraint::Length(6),                // 24h timeline (borders + 3 stripes + axis + border)
            Constraint::Length(11),               // top apps + categories (8 rows + header + borders)
            Constraint::Length(10),               // hourly + domains
            Constraint::Length(sparkline_height), // 7-day sparkline panel (Wide only)
            Constraint::Min(0),                   // flex blank
        ])
        .split(area);

    render_kpi_strip(f, chunks[0], app, layout_mode);
    render_timeline(f, chunks[1], app);
    render_apps_categories_row(f, chunks[2], app);
    render_hourly_domains_row(f, chunks[3], app);
    if sparkline_height > 0 {
        render_sparkline(f, chunks[4], app, layout_mode);
    }
    // chunks[5] is the flex blank — left empty intentionally.
}

fn render_kpi_strip(f: &mut Frame, area: Rect, app: &App, layout_mode: LayoutMode) {
    let theme: &Theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING_TIGHT)
        .title(panel_title(
            theme,
            " Snapshot ",
            app.data.kpi.is_in_flight(),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let kpi = app.data.kpi.value();
    let kpi_err = app.data.kpi.last_error();
    kpi_strip::render(f, inner, theme, layout_mode, kpi, kpi_err);
}

fn render_timeline(f: &mut Frame, area: Rect, app: &App) {
    timeline::render(
        f,
        area,
        &app.theme,
        app.data.timeline_events.value(),
        app.data.timeline_events.is_in_flight(),
        &app.throbber,
    );
}

fn render_sparkline(f: &mut Frame, area: Rect, app: &App, layout_mode: LayoutMode) {
    let theme: &Theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING_TIGHT)
        .title(panel_title(
            theme,
            " 7-day rhythm ",
            app.data.trailing_active.is_in_flight(),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let kpi = app.data.kpi.value();
    let trailing = app.data.trailing_active.value();
    let today_active = kpi.map(|k| k.active_secs);
    sparkline::render(f, inner, theme, layout_mode, today_active, trailing);
}

fn render_apps_categories_row(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    render_top_apps_panel(
        f,
        cols[0],
        &app.theme,
        &app.data.top_apps,
        " Top apps ",
        &app.throbber,
    );
    render_top_categories_panel(
        f,
        cols[1],
        &app.theme,
        &app.data.top_categories,
        " Top categories ",
        &app.throbber,
    );
}

fn render_hourly_domains_row(f: &mut Frame, area: Rect, app: &App) {
    // Fixed-width hourly column keeps its 24 bars dense regardless of
    // terminal width. With ~46 cols (− 2 borders − ~5 Y-axis-label cols)
    // each hour gets ~1.6 columns — bars sit close instead of sparse.
    // Top domains absorbs the rest.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(46), Constraint::Min(20)])
        .split(area);
    render_hourly(f, cols[0], app);
    render_top_domains_panel(
        f,
        cols[1],
        &app.theme,
        &app.data.top_domains,
        " Top domains ",
        &app.throbber,
    );
}

/// Bold + theme.fg panel title. Lives in the panel's top border but
/// styled with stronger contrast than the surrounding dim border so the
/// section header reads as a header, not part of the frame chrome.
pub(super) fn panel_title(theme: &Theme, base: &'static str, in_flight: bool) -> Line<'static> {
    let title_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    if in_flight {
        Line::from(vec![
            Span::styled(base, title_style),
            Span::styled("\u{21bb}", Style::default().fg(theme.dim)),
            Span::raw(" "),
        ])
    } else {
        Line::from(Span::styled(base, title_style))
    }
}

pub(super) fn render_top_apps_panel(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cache: &Cached<Vec<TopAppRow>>,
    title: &'static str,
    throbber: &ThrobberState,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING)
        .title(panel_title(theme, title, cache.is_in_flight()));

    let Some(rows) = cache.value() else {
        let inner = block.inner(area);
        f.render_widget(block, area);
        render_skeleton_body(f, inner, theme, throbber, cache.is_in_flight());
        return;
    };

    if rows.is_empty() {
        let p = Paragraph::new("no app events yet")
            .block(block)
            .style(Style::default().fg(theme.dim));
        f.render_widget(p, area);
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

    // -2 borders, -1 header line.
    let max_rows = area.height.saturating_sub(3) as usize;
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
    let table = Table::new(body_rows, widths).header(header).block(block);
    f.render_widget(table, area);
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
    title: &'static str,
    throbber: &ThrobberState,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING)
        .title(panel_title(theme, title, cache.is_in_flight()));

    let Some(rows) = cache.value() else {
        let inner = block.inner(area);
        f.render_widget(block, area);
        render_skeleton_body(f, inner, theme, throbber, cache.is_in_flight());
        return;
    };

    if rows.is_empty() {
        let p = Paragraph::new("no categorized events yet")
            .block(block)
            .style(Style::default().fg(theme.dim));
        f.render_widget(p, area);
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

    let max_rows = area.height.saturating_sub(3) as usize;
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
    let table = Table::new(body_rows, widths).header(header).block(block);
    f.render_widget(table, area);
}

pub(super) fn category_row(
    rank: usize,
    row: &CategorySummary,
    max_secs: f64,
    theme: &Theme,
) -> Row<'static> {
    let name = row.name.join(" / ");
    let bar = proportional_bar(row.duration, max_secs, 8);
    // Single flat bar colour across all rows — matches Top apps (chart_3)
    // and Top domains (chart_4). The category root is already carried by
    // the name column AND by the timeline above; per-row bar colouring
    // here was redundant and made the panel feel louder than its siblings.
    Row::new(vec![
        Cell::from(format!("{}", rank)).style(Style::default().fg(theme.dim)),
        Cell::from(name).style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Cell::from(format_duration(row.duration)).style(Style::default().fg(theme.fg)),
        Cell::from(bar).style(Style::default().fg(theme.chart_5)),
    ])
}

pub(super) fn render_top_domains_panel(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cache: &Cached<Vec<TopDomainRow>>,
    title: &'static str,
    throbber: &ThrobberState,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING)
        .title(panel_title(theme, title, cache.is_in_flight()));

    let Some(rows) = cache.value() else {
        let inner = block.inner(area);
        f.render_widget(block, area);
        render_skeleton_body(f, inner, theme, throbber, cache.is_in_flight());
        return;
    };

    if rows.is_empty() {
        // Per `daylog_core::queries::top_domains`: an empty Ok-result is
        // the "no aw-watcher-web bucket" signal. Mirror the desktop's
        // WebPanel install hint instead of "no data" — it's an actionable
        // message, not a transient empty state.
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
        let p = Paragraph::new(lines).block(block);
        f.render_widget(p, area);
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

    let max_rows = area.height.saturating_sub(3) as usize;
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
    let table = Table::new(body_rows, widths).header(header).block(block);
    f.render_widget(table, area);
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

/// Proportional fill bar — full block + light shade for the unfilled
/// remainder. Width is fixed so rows stay column-aligned.
pub(super) fn proportional_bar(value: f64, max: f64, width: usize) -> String {
    let filled = if max > 0.0 {
        ((value / max) * width as f64).round() as usize
    } else {
        0
    };
    let filled = filled.min(width);
    format!(
        "{}{}",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(width.saturating_sub(filled))
    )
}

fn render_hourly(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(theme::PANEL_BORDER)
        .border_style(theme.border_dim_style())
        .padding(theme::PANEL_PADDING_TIGHT)
        .title(panel_title(
            theme,
            " Hourly distribution ",
            app.data.hourly.is_in_flight(),
        ));

    let Some(buckets) = app.data.hourly.value() else {
        let inner = block.inner(area);
        f.render_widget(block, area);
        render_skeleton_body(f, inner, theme, &app.throbber, app.data.hourly.is_in_flight());
        return;
    };

    if buckets.is_empty() {
        let p = Paragraph::new("no hourly data")
            .block(block)
            .style(Style::default().fg(theme.dim));
        f.render_widget(p, area);
        return;
    }

    // Bucket each hour into one of five spectrum bands so the per-bar
    // colour signal survives. ratatui's Chart paints one colour per
    // Dataset, so we emit five datasets — one per band — each carrying
    // only the hours that fall in that band.
    let mut band_data: [Vec<(f64, f64)>; 5] =
        [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let mut max_min = 0.0_f64;
    for b in buckets {
        let h = b.hour as usize;
        let band_idx = match b.hour {
            0..=4 => 0,
            5..=9 => 1,
            10..=14 => 2,
            15..=19 => 3,
            _ => 4,
        };
        let minutes = (b.duration / 60.0).max(0.0);
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

    // Y-ceiling rounds to the next 30-min so axis labels stay
    // round-numbered. Floor at 60m so an empty-ish day doesn't squish
    // bars to nothing.
    let y_ceiling = ((max_min / 30.0).ceil() * 30.0).max(60.0);
    let y_mid = (y_ceiling / 2.0).round() as u64;
    let y_top = y_ceiling.round() as u64;
    let axis_style = Style::default().fg(theme.dim);

    let chart = Chart::new(datasets)
        .block(block)
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
    f.render_widget(chart, area);
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

    /// Snapshot test: render the Today tab to a TestBackend and assert
    /// presence of the key visual elements. Asserts by content rather
    /// than byte-exact match so trivial layout tweaks don't cascade
    /// into a thousand expected-string updates.
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

        // Tab strip
        assert!(
            rendered.contains("Today"),
            "tab strip missing Today\n{rendered}"
        );

        // Timeline panel
        assert!(
            rendered.contains("Today's timeline"),
            "missing Today's timeline title\n{rendered}"
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

        // Top domains panel — title shows even with no web watcher
        assert!(rendered.contains("Top domains"), "missing Top domains title");
        assert!(
            rendered.contains("no web watcher"),
            "no-web-watcher hint missing in domains panel\n{rendered}"
        );

        // Hourly chart
        assert!(
            rendered.contains("Hourly distribution"),
            "missing Hourly title"
        );

        // Footer: fixture sets top_domains to an empty Ok-result, which
        // the footer interprets as "extension not installed" and surfaces
        // the install tip in place of the normal key hints.
        assert!(
            rendered.contains("tip:") && rendered.contains("Top domains"),
            "domains-empty tip missing from footer\n{rendered}"
        );

        // Offline indicator must NOT show in fixture (everything succeeded)
        assert!(
            !rendered.contains("tracker offline"),
            "fixture cache is healthy; offline indicator should be hidden"
        );
    }

    /// First-load skeleton: panel structure renders even before any
    /// data lands. No "loading" banners, no "kpi unavailable" text —
    /// those were Phase 1 / Phase 2 banners that the redesign replaced
    /// with shape-stable skeletons.
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
            rendered.contains("Top apps"),
            "Top apps panel chrome should render even with no data\n{rendered}"
        );
        assert!(
            rendered.contains("Today's timeline"),
            "Timeline panel chrome should render even with no data\n{rendered}"
        );
        assert!(
            rendered.contains("Top domains"),
            "Top domains panel chrome should render even with no data\n{rendered}"
        );
        assert!(
            !rendered.contains("kpi unavailable"),
            "Phase 2 removes the red 'kpi unavailable' banner\n{rendered}"
        );
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

    /// Per DESIGN.md D6: hourly chart paints multiple spectrum bands.
    /// Phase 2 switched BarChart → Chart with per-band Datasets; this
    /// test guards the per-band colouring.
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
