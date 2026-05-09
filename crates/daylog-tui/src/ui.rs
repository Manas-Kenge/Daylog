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
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame, Terminal,
};

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
    // Mouse capture intentionally NOT enabled per design decision: keyboard-only
    // navigation, preserving native terminal scroll/select.
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
    let theme = &app.theme;

    // Outer frame. Just "daylog" (bold) in the top border; the active-tab
    // name lives in the tab strip below. Live indicator + clock used to
    // sit on the right of this border but were removed in the chrome
    // cleanup pass — offline state surfaces in the footer pill instead.
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_dim_style())
        .title(header_title(theme));
    let inner = outer.inner(f.area());
    f.render_widget(outer, f.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab strip
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer
        ])
        .split(inner);

    render_tabs(f, chunks[0], app);
    render_body(f, chunks[1], app);
    render_footer(f, chunks[2], app);

    if app.help_visible {
        render_help(f, app);
    }
}

fn header_title(theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "daylog",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
    .left_aligned()
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| Line::from(format!(" {} ", t.label())))
        .collect();
    let divider = Span::styled(
        symbols::line::VERTICAL,
        Style::default().fg(theme.border_dim),
    );
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        // Inactive tabs: theme.fg, no DIM modifier. The eye finds the
        // active tab by contrast change, not by un-greying every other
        // label. This is the single fix for "tabs invisible until pressed".
        .style(Style::default().fg(theme.fg))
        // Active: REVERSED + BOLD with ember-fg as a back-up signal on
        // tiers where REVERSED doesn't print background (linux fbcon).
        .highlight_style(
            Style::default()
                .fg(theme.ember)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
        )
        .divider(divider);
    f.render_widget(tabs, area);
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
    let mut spans = Vec::new();
    if app.data.any_offline() {
        spans.push(Span::styled(
            "\u{25cb} tracker offline",
            theme.error_style(),
        ));
        if area.width >= 60 {
            spans.push(Span::styled(
                "  \u{00b7}  ",
                Style::default().fg(theme.border_dim),
            ));
        }
    }
    // Hide key hints below 60 cols — same threshold the tab strip uses.
    // On narrow terminals the offline pill stays visible (it's the single
    // most actionable signal) and the hints drop out so nothing clips.
    if area.width >= 60 {
        let key = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
        let label = Style::default().fg(theme.dim);
        let sep = Style::default().fg(theme.border_dim);
        spans.extend(vec![
            Span::styled("Tab", key),
            Span::styled(" cycle  ", label),
            Span::styled("\u{00b7}", sep),
            Span::styled("  ?", key),
            Span::styled(" help  ", label),
            Span::styled("\u{00b7}", sep),
            Span::styled("  q", key),
            Span::styled(" quit ", label),
        ]);
    }
    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Right);
    f.render_widget(p, area);
}

fn render_help(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = center_rect(f.area(), 56, 18);
    f.render_widget(Clear, area);

    let key = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let body = Style::default().fg(theme.fg);
    let dim = Style::default().fg(theme.dim);
    let section = |s: &'static str| Line::from(Span::styled(s, key));

    let lines = vec![
        Line::from(Span::styled("Daylog TUI \u{2014} keys", key)),
        Line::from(""),
        section("  Tabs"),
        Line::from(Span::styled("    1 2 3 4         Jump", body)),
        Line::from(Span::styled("    Tab / \u{2192}         Next", body)),
        Line::from(Span::styled("    Shift-Tab / \u{2190}   Prev", body)),
        Line::from(Span::styled("    h / l           Vim cycle", body)),
        Line::from(""),
        section("  Range"),
        Line::from(Span::styled(
            "    r               Forward    Shift-R   Back",
            body,
        )),
        Line::from(""),
        section("  General"),
        Line::from(Span::styled("    ?               Toggle help", body)),
        Line::from(Span::styled(
            "    q / Esc         Quit       Ctrl-C    Quit",
            body,
        )),
        Line::from(""),
        Line::from(Span::styled("  Press ? or Esc to dismiss", dim)),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_dim_style())
        .title(Span::styled(" help ", key));
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

pub fn center_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// Format a duration in seconds as a short, dashboard-friendly label.
/// Matches the desktop's convention: "2h 14m" / "47m 12s" / "3s".
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

    #[test]
    fn format_duration_dashboard_shapes() {
        assert_eq!(format_duration(0.0), "0s");
        assert_eq!(format_duration(45.0), "45s");
        assert_eq!(format_duration(60.0), "1m 00s");
        assert_eq!(format_duration(2.0 * 3600.0 + 14.0 * 60.0 + 5.0), "2h 14m");
        assert_eq!(format_duration(-100.0), "0s");
    }
}
