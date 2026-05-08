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
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame, Terminal,
};

use crate::app::{App, RangeChip, Tab};

mod overview;

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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab strip
            Constraint::Length(1), // range chips
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer hints
        ])
        .split(f.area());

    render_tabs(f, chunks[0], app);
    render_range_chips(f, chunks[1], app);
    render_body(f, chunks[2], app);
    render_footer(f, chunks[3], app);

    if app.help_visible {
        render_help(f);
    }
}

fn render_range_chips(f: &mut Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, chip) in RangeChip::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  \u{00b7}  "));
        }
        let style = if *chip == app.range_chip {
            Style::default().add_modifier(Modifier::REVERSED).bold()
        } else {
            Style::default().dim()
        };
        spans.push(Span::styled(format!(" {} ", chip.label()), style));
    }
    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Left);
    f.render_widget(p, area);
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.label())).collect();
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .style(Style::default().dim())
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED).not_dim());
    f.render_widget(tabs, area);
}

fn render_body(f: &mut Frame, area: Rect, app: &App) {
    match app.tab {
        Tab::Today => overview::render(f, area, app),
        _ => render_placeholder(f, area, app.tab),
    }
}

fn render_placeholder(f: &mut Frame, area: Rect, tab: Tab) {
    let placeholder = Paragraph::new(format!(
        "{} — content lands in a later phase\n\nKeys: 1–4 jump tabs · Tab/Shift-Tab cycle · ? help · q quit",
        tab.label()
    ))
    .alignment(Alignment::Center)
    .style(Style::default().dim());

    let centered = center_rect(area, 60, 5);
    f.render_widget(placeholder, centered);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let mut spans = Vec::new();
    if app.data.any_offline() {
        spans.push(Span::styled(
            "\u{25cb} tracker offline  \u{00b7}  ",
            Style::default().fg(ratatui::style::Color::Red),
        ));
    }
    spans.extend(vec![
        Span::styled("Tab", Style::default().bold()),
        Span::raw(" cycle  \u{00b7}  "),
        Span::styled("?", Style::default().bold()),
        Span::raw(" help  \u{00b7}  "),
        Span::styled("q", Style::default().bold()),
        Span::raw(" quit"),
    ]);
    let p = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Right)
        .style(Style::default().dim());
    f.render_widget(p, area);
}

fn render_help(f: &mut Frame) {
    let area = center_rect(f.area(), 50, 14);
    f.render_widget(Clear, area);
    let lines = vec![
        Line::from(Span::styled("Daylog TUI \u{2014} keys", Style::default().bold())),
        Line::from(""),
        Line::from("  1\u{2013}4           Jump to tab N"),
        Line::from("  Tab / Shift-Tab  Cycle tabs"),
        Line::from("  h / l         Cycle tabs (vim)"),
        Line::from("  r             Cycle range forward"),
        Line::from("  Shift-R       Cycle range backward"),
        Line::from("  ?             Toggle this help"),
        Line::from("  q / Esc       Quit"),
        Line::from("  ctrl-c        Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? or Esc to dismiss",
            Style::default().dim(),
        )),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" help ")
        .style(Style::default());
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
