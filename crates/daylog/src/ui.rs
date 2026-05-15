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
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};
use tachyonfx::EffectRenderer;

use crate::app::{App, Tab};
use crate::theme::Theme;

pub(crate) mod kpi_strip;
mod month;
pub(crate) mod overview;
pub(crate) mod stacked_bars;
pub(crate) mod timeline;
pub(crate) mod week;

pub type Backend = ratatui::backend::CrosstermBackend<Stdout>;

pub fn setup_terminal() -> io::Result<Terminal<Backend>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
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
            Constraint::Length(1), // top divider rule (also the visual margin)
            Constraint::Min(0),    // body — claims everything else
        ])
        .split(f.area());

    render_tabs(f, chunks[0], app);
    render_divider(f, chunks[1], &app.theme);
    render_body(f, chunks[2], app);
    render_offline_indicator(f, chunks[0], app);

    // Scope effects to body so the tab strip doesn't flicker mid-transition.
    if let Some(effect) = app.effect.borrow_mut().as_mut() {
        let last_tick = *app.last_tick.borrow();
        f.render_effect(effect, chunks[2], last_tick);
    }
}

/// Overlay on the right of the tab strip after 3+ consecutive fetch failures.
fn render_offline_indicator(f: &mut Frame, area: Rect, app: &App) {
    if !app.data.any_offline() {
        return;
    }
    if area.width < 20 {
        return;
    }
    let line = Line::from(Span::styled(
        "\u{25cb} tracker offline ",
        app.theme.error_style(),
    ));
    let p = Paragraph::new(line).alignment(Alignment::Right);
    let inset = Rect {
        x: area.x,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.render_widget(p, inset);
}

/// Tab strip. Active tab gets the ember pill; inactive tabs are dim.
fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let inset = Rect {
        x: area.x.saturating_add(2),
        y: area.y,
        width: area.width.saturating_sub(4),
        height: area.height,
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, t) in Tab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        if *t == app.tab {
            spans.push(Span::styled(
                format!("  {}  ", t.label()),
                theme.active_tab_style(),
            ));
        } else {
            spans.push(Span::styled(t.label().to_string(), theme.inactive_tab_style()));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), inset);
}

fn render_body(f: &mut Frame, area: Rect, app: &App) {
    match app.tab {
        Tab::Today => overview::render(f, area, app),
        Tab::Week => week::render(f, area, app),
        Tab::Month => month::render(f, area, app),
    }
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

/// BOLD UPPERCASE section header. `↻` suffix when the data feeding this section
/// is in-flight. Caller controls placement; renders on a single row at the top
/// of `area`.
pub fn render_section_header(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    label: &str,
    in_flight: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let title_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let mut spans: Vec<Span<'static>> = vec![Span::styled(label.to_uppercase(), title_style)];
    if in_flight {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("\u{21bb}", Style::default().fg(theme.dim)));
    }
    let row = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(Paragraph::new(Line::from(spans)), row);
}

/// Dim horizontal rule across the row. Replaces panel borders as the section
/// separator. Drawn at `area`'s top row.
pub fn render_divider(f: &mut Frame, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let rule: String = "\u{2500}".repeat(area.width as usize);
    let row = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(rule, theme.border_dim_style()))),
        row,
    );
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
            "tab strip should carry the three tab labels: {tabs_row}"
        );
        let active_row_idx = (2..=10).find(|y| row(*y).contains("Active"));
        assert!(
            active_row_idx.is_some(),
            "snapshot row should appear in the first several rows of the body"
        );
        assert!(
            !tabs_row.contains("Active"),
            "snapshot must not overwrite tabs: {tabs_row}"
        );

        // Active tab now wears the brand ember pill instead of REVERSED.
        let today_x = tabs_row.find("Today").expect("Today tab present") as u16;
        assert_eq!(
            buf[(today_x, 0)].style().bg,
            Some(theme.ember),
            "active tab background must be the ember accent"
        );
    }
}
