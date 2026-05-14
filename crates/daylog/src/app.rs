//! Application state + main event loop.

use std::cell::RefCell;
use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use daylog_core::time::TimeRange;
use ratatui::style::Color;
use ratatui::Terminal;
use tachyonfx::fx::Direction;
use tachyonfx::{fx, Effect, Interpolation, Shader};
use throbber_widgets_tui::ThrobberState;
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
    pub quit: bool,
    pub dirty: bool,
    pub data: DataCache,
    /// Latched at startup from `$COLORTERM`/`$TERM`. Every widget pulls
    /// colours and modifiers from here so the spec's token table is the
    /// only source of truth.
    pub theme: Theme,
    /// Shared throbber animation state. One state for the whole app —
    /// every panel's spinner advances on the same 250ms tick so they look
    /// like one synchronised heartbeat instead of N drifting clocks.
    pub throbber: ThrobberState,
    /// Currently-running body-scoped effect, if any. RefCell because
    /// `render(f, &App)` is immutable but `Effect::process` needs `&mut`.
    /// On first launch we seed a fade-in; tab transitions overwrite with a
    /// sweep. Cleared once the effect completes.
    pub effect: RefCell<Option<Effect>>,
    /// Elapsed since last redraw — fed to `Effect::process` so animations
    /// advance in real time regardless of redraw cadence.
    pub last_tick: RefCell<tachyonfx::Duration>,
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
            quit: false,
            dirty: true,
            data: DataCache::new(),
            theme,
            throbber: ThrobberState::default(),
            effect: RefCell::new(None),
            last_tick: RefCell::new(tachyonfx::Duration::from_millis(0)),
        }
    }

    /// Seed a body-scoped fade-from-background effect. Called by the
    /// event loop just before the first draw so the cold-start paint
    /// dissolves in. Not invoked from `with_theme` because the test
    /// harness builds fresh `App`s and expects pixel-exact colours on the
    /// first frame — the fade would shift those during the transition.
    pub fn queue_fade_in(&mut self) {
        let bg = self.theme.bg;
        *self.effect.borrow_mut() = Some(fx::fade_from(
            bg,
            bg,
            (
                tachyonfx::Duration::from_millis(400),
                Interpolation::Linear,
            ),
        ));
    }

    /// Queue a left-to-right sweep across the body. Fired on tab change.
    pub fn queue_tab_sweep(&mut self) {
        let bg: Color = self.theme.bg;
        *self.effect.borrow_mut() = Some(fx::sweep_in(
            Direction::LeftToRight,
            10,
            0,
            bg,
            (
                tachyonfx::Duration::from_millis(220),
                Interpolation::Linear,
            ),
        ));
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
    dispatch_refetches(&mut app.data, range, app.tab, &result_tx, Instant::now());

    // Seed the cold-start fade-in so the first paint doesn't snap.
    app.queue_fade_in();

    let mut last_draw = Instant::now();

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
                dispatch_refetches(&mut app.data, range, app.tab, &result_tx, Instant::now());
                // Advance the shared throbber once per tick so the
                // skeleton spinner moves at a calm 4 fps.
                app.throbber.calc_next();
                // Always redraw on tick so transient indicators (offline
                // dot, "loading" tickers) animate without input.
                app.dirty = true;
            }
        }

        // Keep redrawing while an animation effect is in flight so tachyonfx
        // can advance state. Without this gate, the redraw stops as soon as
        // `dirty` clears and the effect freezes mid-frame.
        let has_effect = app.effect.borrow().is_some();
        if app.dirty || has_effect {
            let now = Instant::now();
            let elapsed = now.duration_since(last_draw);
            *app.last_tick.borrow_mut() = tachyonfx::Duration::from_millis(
                elapsed.as_millis().min(u32::MAX as u128) as u32,
            );
            last_draw = now;
            terminal.draw(|f| crate::ui::render(f, app))?;
            app.dirty = false;
            // Drop completed effects so the redraw loop can quiesce.
            let done = app
                .effect
                .borrow()
                .as_ref()
                .map(|e| e.done())
                .unwrap_or(false);
            if done {
                *app.effect.borrow_mut() = None;
            }
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
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.quit = true,
        KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => {
            app.tab = app.tab.next();
            app.queue_tab_sweep();
            app.dirty = true;
        }
        KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => {
            app.tab = app.tab.prev();
            app.queue_tab_sweep();
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
                let new = Tab::ALL[idx];
                if new != app.tab {
                    app.queue_tab_sweep();
                }
                app.tab = new;
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
    fn handle_key_2_lands_on_week() {
        // Pin numeric routing so reordering Tab::ALL doesn't silently
        // re-route the Week shortcut.
        let mut app = App::new();
        assert_eq!(app.tab, Tab::Today);
        handle_key(&mut app, KeyCode::Char('2'), KeyModifiers::NONE);
        assert_eq!(app.tab, Tab::Week);
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

}
