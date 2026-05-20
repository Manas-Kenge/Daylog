use std::io::{self, Stdout};
use std::time::{Duration, Instant};

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

/// How long the update banner stays visible before the next tick drops it.
const UPDATE_BANNER_VISIBLE_FOR: Duration = Duration::from_secs(6);

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

/// Best-effort restore; ignores errors. Panic-safe.
pub fn restore_terminal_raw() -> io::Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    Ok(())
}

pub fn render(f: &mut Frame, app: &App) {
    let show_banner = should_show_update_banner(app);
    let constraints: &[Constraint] = if show_banner {
        &[
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    } else {
        &[
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    render_tabs(f, chunks[0], app);
    render_divider(f, chunks[1], &app.theme);
    render_body(f, chunks[2], app);
    render_offline_indicator(f, chunks[0], app);
    if show_banner {
        render_update_banner(f, chunks[3], app);
    }

    if let Some(effect) = app.effect.borrow_mut().as_mut() {
        let last_tick = *app.last_tick.borrow();
        f.render_effect(effect, chunks[2], last_tick);
    }
}

fn should_show_update_banner(app: &App) -> bool {
    if app.tab != Tab::Today {
        return false;
    }
    if app.data.update_info.is_none() {
        return false;
    }
    let mut shown = app.update_banner_shown_at.borrow_mut();
    let now = Instant::now();
    let started = *shown.get_or_insert(now);
    now.duration_since(started) < UPDATE_BANNER_VISIBLE_FOR
}

fn render_update_banner(f: &mut Frame, area: Rect, app: &App) {
    let Some(info) = &app.data.update_info else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let line = Line::from(vec![
        Span::styled("\u{2191} ", theme.ember_style()),
        Span::styled(
            format!("daylog v{} available", info.latest),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(info.release_url.clone(), Style::default().fg(theme.dim)),
    ]);
    f.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

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

        let today_x = tabs_row.find("Today").expect("Today tab present") as u16;
        assert_eq!(
            buf[(today_x, 0)].style().bg,
            Some(theme.ember),
            "active tab background must be the ember accent"
        );
    }
}
