//! Terminal lifecycle + top-level frame rendering.

use std::io::{self, Stdout};

use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
    Frame, Terminal,
};
use tachyonfx::EffectRenderer;

use crate::app::{App, Tab};
use crate::theme::Theme;

pub(crate) mod kpi_strip;
mod month;
pub(crate) mod overview;
pub(crate) mod sparkline;
pub(crate) mod stacked_bars;
pub(crate) mod timeline;
pub(crate) mod week;

pub type Backend = ratatui::backend::CrosstermBackend<Stdout>;

pub fn setup_terminal() -> io::Result<Terminal<Backend>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // Mouse capture intentionally NOT enabled: preserves native terminal scroll/select.
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

pub fn restore_terminal(terminal: &mut Terminal<Backend>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Best-effort terminal restore from inside a panic hook. Stdout may be in
/// an unknown state; we ignore errors and try every undo step independently.
pub fn restore_terminal_raw() -> io::Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    Ok(())
}

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab strip
            Constraint::Length(1), // legend row (also acts as the visual margin)
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    render_body(f, chunks[2], app);
    render_footer(f, chunks[3], app);
    render_tabs(f, chunks[0], app);
    render_color_legend(f, chunks[1], &app.theme);

    // Scope effects to body so tabs/footer don't flicker mid-transition.
    if let Some(effect) = app.effect.borrow_mut().as_mut() {
        let last_tick = *app.last_tick.borrow();
        f.render_effect(effect, chunks[2], last_tick);
    }
}

/// Right-aligned category legend. Drops labels at 80/50/30-col breakpoints.
fn render_color_legend(f: &mut Frame, area: Rect, theme: &Theme) {
    if area.width < 30 {
        return;
    }
    let labelled = area.width >= 80;
    let abbreviated = area.width < 80;
    let dot = "\u{25CF}";
    let entries: &[(&str, &str, ratatui::style::Color)] = &[
        ("Work", "Work", theme.chart_1),
        ("Comms", "Comms", theme.chart_2),
        ("Media", "Media", theme.chart_3),
        ("Browsing", "Web", theme.chart_4),
        ("Documents", "Docs", theme.chart_5),
        ("Other", "Other", theme.dim),
    ];

    let label_style = Style::default().fg(theme.dim);
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (full, short, color)) in entries.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(dot.to_string(), Style::default().fg(*color)));
        if labelled || area.width >= 50 {
            let name = if abbreviated { *short } else { *full };
            spans.push(Span::styled(format!(" {}", name), label_style));
        }
    }

    let inset = Rect {
        x: area.x,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Right);
    f.render_widget(p, inset);
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let inset = Rect {
        x: area.x.saturating_add(2),
        y: area.y,
        width: area.width.saturating_sub(4),
        height: area.height,
    };
    // Tabs::padding() is uniform; wrap the active label in spaces so only it gets the wider pill background.
    let titles: Vec<Line<'static>> = Tab::ALL
        .iter()
        .map(|t| {
            if *t == app.tab {
                Line::from(format!("  {}  ", t.label()))
            } else {
                Line::from(t.label())
            }
        })
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .style(
            Style::default()
                .fg(theme.fg)
                .bg(theme.bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_style(
            Style::default()
                .fg(theme.bg)
                .bg(theme.ember)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled(
            symbols::DOT,
            Style::default().fg(theme.border_dim).bg(theme.bg),
        ))
        .padding("  ", "  ");
    f.render_widget(tabs, inset);
}

fn render_body(f: &mut Frame, area: Rect, app: &App) {
    match app.tab {
        Tab::Today => overview::render(f, area, app),
        Tab::Week => week::render(f, area, app),
        Tab::Month => month::render(f, area, app),
    }
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let dim = Style::default().fg(theme.dim);
    let sep = Style::default().fg(theme.border_dim);
    let key = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);

    let mut spans = Vec::new();
    if app.data.any_offline() {
        spans.push(Span::styled(
            "\u{25cb} tracker offline",
            theme.error_style(),
        ));
        if area.width >= 60 {
            spans.push(Span::styled("  \u{00b7}  ", sep));
        }
    }

    // Empty-Ok top_domains = "no aw-watcher-web bucket" signal. Pending = don't speculate.
    let domains_missing = app
        .data
        .top_domains
        .value()
        .map(|rows| rows.is_empty())
        .unwrap_or(false);

    if area.width >= 60 {
        if domains_missing {
            spans.push(Span::styled("tip: ", key));
            spans.push(Span::styled(
                "install the browser extension to populate ",
                dim,
            ));
            spans.push(Span::styled("Top domains", key));
        } else {
            spans.extend(vec![
                Span::styled("Tab", key),
                Span::styled(" cycle  ", dim),
                Span::styled("\u{00b7}", sep),
                Span::styled("  q", key),
                Span::styled(" quit ", dim),
            ]);
        }
    }
    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Right);
    f.render_widget(p, area);
}

/// Skeleton body: animated throbber if `fetching`, static `…` otherwise.
/// Centred vertically in the panel inner area.
pub fn render_skeleton_body(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    throbber: &throbber_widgets_tui::ThrobberState,
    fetching: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = if fetching {
        // Non-mutating render path; app.rs already advanced `throbber` this frame.
        let widget = throbber_widgets_tui::Throbber::default()
            .style(Style::default().fg(theme.dim))
            .throbber_set(throbber_widgets_tui::BRAILLE_SIX_DOUBLE)
            .use_type(throbber_widgets_tui::WhichUse::Spin);
        Line::from(widget.to_symbol_span(throbber))
    } else {
        Line::from(Span::styled("\u{2026}", Style::default().fg(theme.dim)))
    };
    let y_offset = area.height.saturating_sub(1) / 2;
    let row = Rect {
        x: area.x,
        y: area.y + y_offset,
        width: area.width,
        height: 1,
    };
    let p = Paragraph::new(line).alignment(Alignment::Center);
    f.render_widget(p, row);
}

/// "2h 14m" / "47m 12s" / "3s".
pub fn format_duration(secs: f64) -> String {
    let total = secs.max(0.0) as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}h {:02}m", h, m)
    } else if m > 0 {
        format!("{}m {:02}s", m, s)
    } else {
        format!("{}s", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn format_duration_dashboard_shapes() {
        assert_eq!(format_duration(0.0), "0s");
        assert_eq!(format_duration(45.0), "45s");
        assert_eq!(format_duration(60.0), "1m 00s");
        assert_eq!(format_duration(2.0 * 3600.0 + 14.0 * 60.0 + 5.0), "2h 14m");
        assert_eq!(format_duration(-100.0), "0s");
    }

    #[test]
    fn tabs_render_above_body_and_are_visible_on_first_frame() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let app = App::with_theme(theme);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, &app)).expect("render frame");
        let buf = terminal.backend().buffer().clone();

        let row = |y: u16| {
            let mut out = String::new();
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out
        };

        let tabs_row = row(0);
        assert!(
            tabs_row.contains("Today")
                && tabs_row.contains("Week")
                && tabs_row.contains("Month"),
            "tab strip should be the first row: {tabs_row}"
        );
        let active_row_idx = (2..=7).find(|y| row(*y).contains("Active"));
        assert!(
            active_row_idx.is_some(),
            "KPI strip should appear in the first few rows of the body"
        );
        assert!(
            !tabs_row.contains("Active"),
            "KPI strip must not overwrite tabs: {tabs_row}"
        );

        let today_x = tabs_row.find("Today").expect("Today tab present") as u16;
        assert_eq!(
            buf[(today_x, 0)].style().bg,
            Some(theme.ember),
            "active tab should have an explicit visible background"
        );
    }
}
