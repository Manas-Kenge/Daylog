//! First-launch tracker installer.
//!
//! Probes aw-server on `:5600`. If it's already running, daylog drops
//! straight into the dashboard. Otherwise the user gets a one-screen
//! ratatui prompt asking whether to install the embedded tracker.
//! Choice is persisted at `~/.config/daylog/.wizard-complete` so we
//! don't re-prompt on every launch.

use std::io;
use std::path::PathBuf;

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame, Terminal,
};
use tokio_stream::StreamExt;

use daylog_core::aw_client::AwClient;
use daylog_core::datastore;

use crate::theme::Theme;
use crate::tracking::{self, InstallError, LifecycleError};
use crate::ui::Backend;

pub const WIZARD_MARKER: &str = ".wizard-complete";

/// What the wizard decided after a single launch.
#[derive(Debug)]
pub enum WizardOutcome {
    /// Either the marker was already present, or aw-server was already
    /// up — nothing to do, drop into the dashboard.
    Skipped,
    /// User confirmed install + tracker is now live. Marker written.
    Installed,
    /// User declined the install. Marker written so we don't re-prompt.
    Declined,
    /// User pressed Q before answering. Caller should exit cleanly without
    /// rendering the dashboard.
    Quit,
}

#[derive(Debug, thiserror::Error)]
pub enum WizardError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("install: {0}")]
    Install(#[from] InstallError),
    #[error("lifecycle: {0}")]
    Lifecycle(#[from] LifecycleError),
}

/// Run the wizard against `terminal`. The terminal stays in raw-mode + alt-screen
/// for the whole flow; the dashboard takes over after `Skipped` / `Installed`.
pub async fn run_if_needed(
    terminal: &mut Terminal<Backend>,
    theme: &Theme,
) -> Result<WizardOutcome, WizardError> {
    let server_up = probe_aw_server().await;
    let db_present = datastore::db_path().map(|p| p.exists()).unwrap_or(false);

    // Surface the "non-aw-server-rust process is bound to :5600" case
    // before dropping into the dashboard. Most likely culprit is the
    // older aw-server (Python) from a pre-Rust ActivityWatch install.
    // Daylog no longer reads peewee SQLite, so the TUI would show
    // "tracker offline" without explanation. The screen waits for any
    // key, then falls through — the user still sees the dashboard,
    // they just know what's happening.
    if server_up && !db_present {
        render_wrong_server(terminal, theme)?;
        wait_for_any_key().await?;
    }

    if marker_exists() || server_up {
        // marker = user already chose; server_up without marker = some
        // other aw-server is already running. Either way, don't reprompt.
        // (The wrong-server warning above already covered the latter.)
        return Ok(WizardOutcome::Skipped);
    }

    render_prompt(terminal, theme)?;

    match wait_for_choice().await? {
        Choice::Quit => Ok(WizardOutcome::Quit),
        Choice::Decline => {
            write_marker()?;
            Ok(WizardOutcome::Declined)
        }
        Choice::Install => {
            install_tracker(terminal, theme).await?;
            write_marker()?;
            Ok(WizardOutcome::Installed)
        }
    }
}

/// Force the wizard regardless of marker / probe state. Wired to
/// `daylog --setup`. Always renders the prompt; user can still decline.
pub async fn run_forced(
    terminal: &mut Terminal<Backend>,
    theme: &Theme,
) -> Result<WizardOutcome, WizardError> {
    render_prompt(terminal, theme)?;
    match wait_for_choice().await? {
        Choice::Quit => Ok(WizardOutcome::Quit),
        Choice::Decline => Ok(WizardOutcome::Declined),
        Choice::Install => {
            install_tracker(terminal, theme).await?;
            write_marker()?;
            Ok(WizardOutcome::Installed)
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Choice {
    Install,
    Decline,
    Quit,
}

async fn install_tracker(
    terminal: &mut Terminal<Backend>,
    theme: &Theme,
) -> Result<(), WizardError> {
    render_progress(terminal, theme, "Downloading tracker (~44 MB)…")?;
    let bin_dir = tracking::place_binaries().await?;

    render_progress(terminal, theme, "Installing supervisor…")?;
    tracking::install_supervisor(&bin_dir).await?;

    render_progress(terminal, theme, "Waiting for the tracker to come up…")?;
    tracking::wait_until_live(15).await?;

    // GNOME-Wayland needs a shell extension for window titles; install if
    // applicable. Best-effort — failure here doesn't abort the install,
    // since the rest of the tracker still works (you just lose titles).
    if tracking::gnome::is_gnome_wayland() {
        render_progress(terminal, theme, "Installing GNOME shell extension…")?;
        let _ = tracking::gnome::setup().await;
    }

    render_progress(terminal, theme, "Done. Loading dashboard…")?;
    Ok(())
}

async fn probe_aw_server() -> bool {
    AwClient::new().info().await.is_ok()
}

async fn wait_for_any_key() -> Result<(), WizardError> {
    let mut events = EventStream::new();
    while let Some(event) = events.next().await {
        if let Event::Key(k) = event? {
            if k.kind == KeyEventKind::Press {
                return Ok(());
            }
        }
    }
    Ok(())
}

async fn wait_for_choice() -> Result<Choice, WizardError> {
    let mut events = EventStream::new();
    while let Some(event) = events.next().await {
        let event = event?;
        if let Event::Key(k) = event {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    return Ok(Choice::Install)
                }
                KeyCode::Char('n') | KeyCode::Char('N') => return Ok(Choice::Decline),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    return Ok(Choice::Quit)
                }
                _ => {}
            }
        }
    }
    // EventStream ended before we got a choice — treat as quit.
    Ok(Choice::Quit)
}

fn render_prompt(
    terminal: &mut Terminal<Backend>,
    theme: &Theme,
) -> io::Result<()> {
    terminal.draw(|f| {
        let area = f.area();
        let card = center_rect(area, 64, 11);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_dim_style())
            .title(Span::styled(
                " daylog · setup ",
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ));
        f.render_widget(Clear, card);

        let inner = block.inner(card);
        f.render_widget(block, card);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // headline
                Constraint::Length(3), // body
                Constraint::Length(1), // spacer
                Constraint::Length(1), // keys
            ])
            .split(inner);

        let headline = Paragraph::new(Line::from(Span::styled(
            "  Activity tracker not detected.",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )));
        f.render_widget(headline, chunks[0]);

        let body_lines = vec![
            Line::from(Span::styled(
                "  daylog needs a local tracker to record per-app and",
                Style::default().fg(theme.fg),
            )),
            Line::from(Span::styled(
                "  per-window time. Daylog can install one for you now",
                Style::default().fg(theme.fg),
            )),
            Line::from(Span::styled(
                "  (~30 MB, all local — no cloud, no sign-in).",
                Style::default().fg(theme.dim),
            )),
        ];
        f.render_widget(Paragraph::new(body_lines), chunks[1]);

        render_keys(f, chunks[3], theme);
    })?;
    Ok(())
}

fn render_wrong_server(
    terminal: &mut Terminal<Backend>,
    theme: &Theme,
) -> io::Result<()> {
    terminal.draw(|f| {
        let area = f.area();
        let card = center_rect(area, 70, 13);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_dim_style())
            .title(Span::styled(
                " daylog · wrong tracker on :5600 ",
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ));
        f.render_widget(Clear, card);
        let inner = block.inner(card);
        f.render_widget(block, card);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // headline
                Constraint::Length(6), // body
                Constraint::Length(1), // spacer
                Constraint::Length(1), // keys
            ])
            .split(inner);

        let headline = Paragraph::new(Line::from(Span::styled(
            "  Detected an aw-server on :5600, but not aw-server-rust.",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        )));
        f.render_widget(headline, chunks[0]);

        let body_lines = vec![
            Line::from(Span::styled(
                "  Daylog reads aw-server-rust's SQLite file directly. The",
                Style::default().fg(theme.fg),
            )),
            Line::from(Span::styled(
                "  older aw-server (Python) uses a different schema and",
                Style::default().fg(theme.fg),
            )),
            Line::from(Span::styled(
                "  isn't supported. Stop the other server, then run:",
                Style::default().fg(theme.fg),
            )),
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                "    daylog --setup",
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "  to install aw-server-rust. Your existing data is preserved.",
                Style::default().fg(theme.dim),
            )),
        ];
        f.render_widget(Paragraph::new(body_lines), chunks[1]);

        let keys = Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("Any key", Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(" to continue (dashboard will show offline)", Style::default().fg(theme.dim)),
        ]));
        f.render_widget(keys, chunks[3]);
    })?;
    Ok(())
}

fn render_progress(
    terminal: &mut Terminal<Backend>,
    theme: &Theme,
    message: &str,
) -> io::Result<()> {
    let owned = message.to_string();
    terminal.draw(|f| {
        let area = f.area();
        let card = center_rect(area, 64, 7);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_dim_style())
            .title(Span::styled(
                " daylog · setup ",
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ));
        f.render_widget(Clear, card);
        let inner = block.inner(card);
        f.render_widget(block, card);

        let p = Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                owned.clone(),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Left);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(inner);
        f.render_widget(p, chunks[0]);
    })?;
    Ok(())
}

fn render_keys(f: &mut Frame, area: Rect, theme: &Theme) {
    let key = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let label = Style::default().fg(theme.dim);
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled("Y", key),
        Span::styled("/", label),
        Span::styled("Enter", key),
        Span::styled(" install   ", label),
        Span::styled("N", key),
        Span::styled(" skip   ", label),
        Span::styled("Q", key),
        Span::styled("/", label),
        Span::styled("Esc", key),
        Span::styled(" quit", label),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn center_rect(area: Rect, width: u16, height: u16) -> Rect {
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

fn marker_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("daylog").join(WIZARD_MARKER))
}

fn marker_exists() -> bool {
    marker_path().map(|p| p.exists()).unwrap_or(false)
}

fn write_marker() -> io::Result<()> {
    let Some(p) = marker_path() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "could not resolve config dir",
        ));
    };
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, b"")?;
    Ok(())
}
