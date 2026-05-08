//! Application state + main event loop.

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use daylog_core::time::TimeRange;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_stream::StreamExt;

use crate::data::{dispatch_refetches, DataCache, FetchResult};
use crate::theme::Theme;
use crate::ui::Backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Today,
    Week,
    Month,
}

impl Tab {
    // Settings was a stub ("content lands in a later phase") and has
    // been dropped until it has real content (read-only diagnostic
    // panel — server info, watcher list, cache health). Listing a tab
    // the user cycles into a placeholder reads as broken.
    pub const ALL: &'static [Tab] = &[Tab::Today, Tab::Week, Tab::Month];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Today => "Today",
            Tab::Week => "Week",
            Tab::Month => "Month",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn next(self) -> Tab {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }

    pub fn prev(self) -> Tab {
        let i = (self.index() + Self::ALL.len() - 1) % Self::ALL.len();
        Self::ALL[i]
    }
}

/// Time-range chips in the order shown under the tab strip. `r` cycles
/// forward, `Shift-R` reverses. We keep the set tight (4 chips) so the
/// chip row fits under any reasonable terminal width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeChip {
    Today,
    Yesterday,
    Last7,
    Last30,
}

impl RangeChip {
    pub const ALL: &'static [RangeChip] = &[
        RangeChip::Today,
        RangeChip::Yesterday,
        RangeChip::Last7,
        RangeChip::Last30,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RangeChip::Today => "Today",
            RangeChip::Yesterday => "Yesterday",
            RangeChip::Last7 => "Last 7",
            RangeChip::Last30 => "Last 30",
        }
    }

    pub fn to_range(self) -> TimeRange {
        match self {
            RangeChip::Today => TimeRange::Today,
            RangeChip::Yesterday => TimeRange::Yesterday,
            RangeChip::Last7 => TimeRange::LastNDays { days: 7 },
            RangeChip::Last30 => TimeRange::LastNDays { days: 30 },
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

pub struct App {
    pub tab: Tab,
    pub range_chip: RangeChip,
    pub help_visible: bool,
    pub quit: bool,
    pub dirty: bool,
    pub data: DataCache,
    /// Latched at startup from `$COLORTERM`/`$TERM`. Every widget pulls
    /// colours and modifiers from here so the spec's token table is the
    /// only source of truth.
    pub theme: Theme,
}

impl App {
    pub fn new() -> Self {
        Self::with_theme(Theme::detect())
    }

    /// Construct with an explicit theme. Tests pin a deterministic tier
    /// via `Theme::from_env_pair(Some("truecolor"), None)` so snapshot
    /// colour expectations don't drift with `$COLORTERM` on the host.
    pub fn with_theme(theme: Theme) -> Self {
        Self {
            tab: Tab::Today,
            range_chip: RangeChip::Today,
            help_visible: false,
            quit: false,
            dirty: true,
            data: DataCache::new(),
            theme,
        }
    }

    pub fn range(&self) -> TimeRange {
        self.range_chip.to_range()
    }

    /// Cycle the active range, resetting the data cache so the next
    /// dispatch fires fresh fetches for the new range.
    pub fn cycle_range(&mut self, forward: bool) {
        self.range_chip = if forward {
            self.range_chip.next()
        } else {
            self.range_chip.prev()
        };
        self.data = DataCache::new();
        self.dirty = true;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Main event loop. Selects between terminal input, periodic ticks (for
/// refetch staleness checks + dispatch), and incoming fetch results.
pub async fn event_loop(terminal: &mut Terminal<Backend>, app: &mut App) -> io::Result<()> {
    let mut events = EventStream::new();
    let mut tick = interval(Duration::from_millis(250));
    tick.tick().await; // first tick fires immediately; consume it.

    let (result_tx, mut result_rx) = mpsc::unbounded_channel::<FetchResult>();

    // Kick off the first set of fetches so the first frame doesn't wait
    // 250ms for the initial tick.
    let range = app.range();
    dispatch_refetches(&mut app.data, range, &result_tx, Instant::now());

    loop {
        tokio::select! {
            biased;
            maybe_evt = events.next() => {
                match maybe_evt {
                    Some(Ok(evt)) => handle_event(app, evt),
                    Some(Err(e)) => return Err(e),
                    None => return Ok(()),
                }
            }
            Some(msg) = result_rx.recv() => {
                app.data.apply(msg, Instant::now());
                app.dirty = true;
            }
            _ = tick.tick() => {
                let range = app.range();
                dispatch_refetches(&mut app.data, range, &result_tx, Instant::now());
                // Always redraw on tick so transient indicators (offline
                // dot, "loading" tickers) animate without input.
                app.dirty = true;
            }
        }

        if app.dirty {
            terminal.draw(|f| crate::ui::render(f, app))?;
            app.dirty = false;
        }

        if app.quit {
            return Ok(());
        }
    }
}

fn handle_event(app: &mut App, evt: Event) {
    match evt {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            handle_key(app, key.code, key.modifiers);
        }
        Event::Resize(_, _) => {
            app.dirty = true;
        }
        _ => {}
    }
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    if app.help_visible {
        // Help overlay swallows everything except dismiss keys.
        if matches!(code, KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')) {
            app.help_visible = false;
            app.dirty = true;
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.quit = true,
        KeyCode::Char('?') => {
            app.help_visible = true;
            app.dirty = true;
        }
        KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => {
            app.tab = app.tab.next();
            app.dirty = true;
        }
        KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => {
            app.tab = app.tab.prev();
            app.dirty = true;
        }
        KeyCode::Char('r') => {
            // lowercase 'r' = forward, Shift+R = reverse.
            let forward = !mods.contains(KeyModifiers::SHIFT);
            app.cycle_range(forward);
        }
        KeyCode::Char('R') => {
            // Some terminals emit Char('R') with NONE modifiers when shift+r
            // is pressed, others emit Char('r') + SHIFT. Cover both.
            app.cycle_range(false);
        }
        KeyCode::Char(d) if d.is_ascii_digit() && d != '0' => {
            // 1..4 jump to tab N (Today/Week/Month/Settings).
            let idx = (d as u8 - b'1') as usize;
            if idx < Tab::ALL.len() {
                app.tab = Tab::ALL[idx];
                app.dirty = true;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycle_forward_wraps() {
        let mut t = Tab::Today;
        for _ in 0..Tab::ALL.len() {
            t = t.next();
        }
        assert_eq!(t, Tab::Today);
    }

    #[test]
    fn tab_cycle_backward_wraps() {
        assert_eq!(Tab::Today.prev(), Tab::Month);
        assert_eq!(Tab::Month.next(), Tab::Today);
    }

    #[test]
    fn handle_key_quit_keys() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            let mut app = App::new();
            handle_key(&mut app, code, KeyModifiers::NONE);
            assert!(app.quit, "{code:?} should quit");
        }

        let mut app = App::new();
        handle_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(app.quit, "ctrl-c should quit");
    }

    #[test]
    fn handle_key_tab_cycle() {
        let mut app = App::new();
        handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Week);
        handle_key(&mut app, KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Today);
        handle_key(&mut app, KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Week);
        handle_key(&mut app, KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Today);
    }

    #[test]
    fn handle_key_arrow_keys_alias_tab_cycle() {
        // Most users try arrow keys before vim keys. Right/Left should
        // behave identically to l/h.
        let mut app = App::new();
        handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Week);
        handle_key(&mut app, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Today);
    }

    #[test]
    fn handle_key_numbers_jump_to_tab() {
        let mut app = App::new();
        handle_key(&mut app, KeyCode::Char('3'), KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Month);
        handle_key(&mut app, KeyCode::Char('1'), KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Today);
    }

    #[test]
    fn range_chip_cycle_wraps_both_directions() {
        let mut chip = RangeChip::Today;
        for _ in 0..RangeChip::ALL.len() {
            chip = chip.next();
        }
        assert_eq!(chip, RangeChip::Today);
        assert_eq!(RangeChip::Today.prev(), RangeChip::Last30);
    }

    #[test]
    fn cycle_range_resets_data_cache() {
        let mut app = App::new();
        let now = Instant::now();
        // Pretend a fetch already populated top_apps for Today.
        app.data
            .top_apps
            .apply_success(vec![], now);
        assert!(app.data.top_apps.value().is_some());

        app.cycle_range(true);

        assert_eq!(app.range_chip, RangeChip::Yesterday);
        assert!(
            app.data.top_apps.value().is_none(),
            "cycling range must drop the cached value so the next dispatch refetches"
        );
        assert!(app.dirty);
    }

    #[test]
    fn handle_key_r_cycles_range_forward_and_back() {
        let mut app = App::new();
        handle_key(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(app.range_chip, RangeChip::Yesterday);
        handle_key(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(app.range_chip, RangeChip::Last7);

        // Shift+r reverses.
        handle_key(&mut app, KeyCode::Char('r'), KeyModifiers::SHIFT);
        assert_eq!(app.range_chip, RangeChip::Yesterday);
    }

    #[test]
    fn help_overlay_swallows_navigation() {
        let mut app = App::new();
        app.help_visible = true;
        handle_key(&mut app, KeyCode::Char('l'), KeyModifiers::NONE);
        // Tab cycle was suppressed.
        assert_eq!(app.tab, Tab::Today);
        assert!(app.help_visible, "help still showing");

        // Esc closes help.
        handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.help_visible);
    }
}
