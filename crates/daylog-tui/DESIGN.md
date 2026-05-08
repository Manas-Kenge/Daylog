# Daylog TUI — Design Spec

The TUI's job is to feel like Daylog, not like a generic ratatui dashboard. This file is the source of truth for that translation: how the desktop's `:root` CSS variables (`src/index.css`), Activity Spectrum chart palette, and hierarchy from `src/pages/Overview.tsx` map onto a terminal surface.

Decisions D1–D6 below were locked in `/plan-design-review` on 2026-05-08.

## Visual identity — what "looks like Daylog" means

The desktop's identity rests on three things:

1. **Ember/orange signature accent.** Used for the live indicator dot, palette focus, primary CTA, and the top-app bars in the screenshot pinned to the README. Not red, not yellow — a specific warm orange at `oklch(0.70 0.16 45)`.
2. **Activity Spectrum** — a 5-step warm-to-cool gradient that maps morning → night across the hourly chart. Orange at the start of the day, violet at the end. This is the single most distinctive visual moment in the product.
3. **Panels float in negative space** — borders are a 12% white opacity, nearly invisible. Hierarchy comes from typography (bold headlines, dim labels) and whitespace, not from boxes.

The TUI translates each of these to terminal capabilities. Where the desktop uses opacity, the TUI uses `Modifier::DIM`. Where the desktop uses gradient, the TUI uses 5 bands. Where the desktop uses border-radius, the TUI uses spacing.

## Color tokens (D3)

A new module `crates/daylog-tui/src/theme.rs` is the single source for every color, modifier, and spacing constant in the TUI. **No widget reaches into `ratatui::style::Color::*` directly** — that's the equivalent of inline styles. Every widget pulls from `theme::Theme`.

`Theme::detect()` reads `$COLORTERM` and `$TERM` at startup and picks one of three palettes:

| Tier | Trigger | How colors are encoded |
|---|---|---|
| `Truecolor` | `COLORTERM=truecolor` or `COLORTERM=24bit` | `Color::Rgb(r,g,b)` from OKLCH conversions, exact desktop match |
| `Color256` | `TERM` contains `256color` and no `COLORTERM` | `Color::Indexed(n)` with hand-picked nearest-neighbours (table below) |
| `Ansi16` | Floor — anything else, including dumb SSH | `Color::Yellow` for ember, `Red/Yellow/Green/Cyan/Magenta` for spectrum |

The detection runs once at startup and the result is stashed on `App` (or passed via render args). No per-frame env reads.

### Token table

| Token | OKLCH (desktop) | Truecolor RGB | 256-color | ANSI-16 | Used for |
|---|---|---|---|---|---|
| `bg` | `oklch(0 0 0)` | `(0,0,0)` | `Indexed(0)` | `Black` | Background (don't paint — let terminal decide) |
| `fg` | `oklch(0.985 0 0)` | `(251,251,251)` | `Indexed(231)` | `White` | Body text |
| `dim` | `oklch(0.708 0 0)` | `(176,176,176)` | `Indexed(244)` | `White` + `Modifier::DIM` | Labels, metadata, inactive tabs |
| `border_dim` | `oklch(1 0 0 / 12%)` | `(31,31,31)` | `Indexed(236)` | `Black` + `Modifier::DIM` | Data-panel borders |
| `ember` | `oklch(0.70 0.16 45)` | `(229,154,110)` | `Indexed(173)` | `Yellow` | Live dot, primary accent, top-app bars |
| `chart_1` | `oklch(0.72 0.17 50)` | `(238,159,99)` | `Indexed(215)` | `Yellow` | Spectrum hours 0–4 (early morning) |
| `chart_2` | `oklch(0.78 0.14 80)` | `(218,191,108)` | `Indexed(186)` | `Yellow` + bold | Spectrum hours 5–9 (morning) |
| `chart_3` | `oklch(0.72 0.10 145)` | `(141,189,142)` | `Indexed(108)` | `Green` | Spectrum hours 10–14 (afternoon) |
| `chart_4` | `oklch(0.70 0.10 200)` | `(115,180,202)` | `Indexed(110)` | `Cyan` | Spectrum hours 15–19 (evening) |
| `chart_5` | `oklch(0.62 0.12 270)` | `(126,131,201)` | `Indexed(104)` | `Magenta` | Spectrum hours 20–23 (night) |
| `error` | `oklch(0.65 0.20 25)` | `(228,113,99)` | `Indexed(167)` | `Red` | Offline indicator, fetch errors |

RGB values are computed offline from OKLCH; do not recompute at runtime. 256-color indices were picked by visual inspection on a kitty + xterm baseline.

## Spectrum mapping (D6)

5-band fixed mapping. Hour `h` selects:

```
band = match h {
    0..=4   => chart_1,   // 12am–4am: warm orange
    5..=9   => chart_2,   // 5am–9am:  yellow
    10..=14 => chart_3,   // 10am–2pm: green
    15..=19 => chart_4,   // 3pm–7pm:  cyan
    20..=23 => chart_5,   // 8pm–11pm: violet
};
```

Same banding logic on desktop and TUI. Screenshots at the same time of day on either surface should agree on color. **Do not interpolate** — the fallback chain only works if every band has a defined truecolor / 256 / ANSI-16 entry.

## Tab structure

The TUI ships with **4 tabs**, mirroring the desktop app's surfaces — no invented surfaces:

| Tab | Desktop counterpart | Content source |
|---|---|---|
| Today | `src/pages/Overview.tsx` | This spec (Overview layout below) |
| Week | `src/pages/WeekPage.tsx` | Port from desktop, later review |
| Month | `src/pages/MonthPage.tsx` | Port from desktop, later review |
| Settings | Desktop's settings dialog (Phase 4 in PLAN.md) | Later review |

The Apps and Categories tabs that appeared in the earlier TUI skeleton are removed — the desktop has no equivalent surfaces; they would have been TUI-only inventions and are gone.

## Today layout (D1 + D2 + D4)

Vertical stack, top to bottom:

| Row(s) | Content | Border |
|---|---|---|
| 1 | Tab strip | none |
| 1 | Range chips | none |
| 1 | KPI strip + 7-day sparkline (shared row) | none |
| 12 | TopApps │ TopCategories | dim, on each |
| Min | Hourly (24-hour spectrum bar chart) | dim |
| 1 | Footer hints | none |

The KPI strip and sparkline share a single line:

```
Active 5h 30m  Longest 1h 12m  +2h Browsing vs typical Tue   ▁▂▅█▆▃▂  Mon-Sun
└────────── KPI strip (left, ~70%) ──────────┘              └ sparkline (right) ┘
```

### Narrow-width fallback

At terminal width `< 100`, drop the suffix prose:

```
Active 5h 30m · Longest 1h 12m · +2h Browsing             ▁▂▅█▆▃▂
```

At width `< 80`, stack TopApps and TopCategories vertically instead of side-by-side and drop the sparkline label:

```
Active 5h 30m
┌─ Top apps ────────────────────┐
│ kitty       ████████░  4h     │
…
```

Width-based branching lives in `theme::Theme::layout_mode(width: u16)` returning `Wide | Narrow | Stacked`.

### Borders

- **Borderless:** tab strip, range chips, KPI strip, sparkline, footer.
- **Dim borders:** TopApps, TopCategories, Hourly, Help overlay.

Implementation note: `Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.border_dim))`. The border colour is one rung above pure background — visible but quiet.

## Typography

The TUI has two type weights: `Modifier::BOLD` and `Modifier::DIM`. Use them like the desktop uses font-weight + opacity.

| Element | Style |
|---|---|
| KPI headline number ("5h 30m") | `BOLD` |
| KPI label ("Longest", "Active") | `DIM` |
| Active tab | `REVERSED` + not `DIM` |
| Inactive tabs | `DIM` |
| Active range chip | `REVERSED` + `BOLD` |
| Inactive range chips | `DIM` |
| App name in TopApps | `BOLD` |
| Duration column | default |
| Bar fill in TopApps | `theme.ember` |
| Loading message | `DIM` |
| Error message | `theme.error` |
| "Active" label before headline | `DIM` (no ember accent — text label, not a status dot) |
| Footer hint keys ("Tab", "?", "q") | `BOLD` |
| Footer hint connectives | `DIM` |

## KPI compute path (D5)

KPI math (`Active`, `Longest stretch`, `Pattern shift`) is promoted to `daylog-core` so the desktop and TUI compute it identically.

- New module `daylog-core::kpi` with `today(events: &[AwEvent], baseline: Option<&BaselineCache>) -> KpiSummary`.
- `KpiSummary { active_secs, afk_secs, longest_stretch_secs, pattern_shift: Option<PatternShift> }`.
- Desktop's existing TS hooks become thin wrappers over a new IPC command `kpi_today(range: TimeRange) -> KpiSummary`.
- TUI's `data::DataCache` gains a `kpi: Cached<KpiSummary>` and `dispatch_refetches` includes it.
- Sparkline uses the same `daylog-core::aggregate::trailing_days(...)` already cached for the desktop's WeekHeatmap.

This refactor touches `src-tauri/src/lib.rs`, `src/lib/aw.ts`, and one or two desktop hooks. It is on the v0.2 critical path because the TUI cannot ship its KPI strip without it.

## Interaction states (Pass 2 deferrals)

Already implemented per `crates/daylog-tui/src/ui/overview.rs`: per-panel loading skeleton, empty state, fetch-error message on TopApps, global offline indicator after 3 failures. Two gaps deferred to TODOS:

- TopCategories and Hourly should distinguish "fetch failed" from "no data yet" the way TopApps does.
- Fresh-install empty state: replace `no app events yet` with `ActivityWatch is collecting data — check back in a few minutes` when total events across all panels is zero **and** uptime since first launch is < 5 minutes.

## Keyboard (Pass 6 deferral)

Current bindings: `1`–`6`, `Tab`/`Shift-Tab`, `h`/`l`, `r`/`Shift-R`, `?`, `q`/`Esc`/`ctrl-c`. Add `Right`/`Left` aliases for `l`/`h` (most users try arrow keys before vim keys).

## Snapshot tests

The existing `overview_renders_top_apps_categories_and_hourly` snapshot test in `src/ui/overview.rs` is content-based, not byte-exact, which means it survives layout changes. Extend it:

- Assert ember-coloured top-app bar fills (look for the colour, not just the bar character).
- Assert spectrum colours on the hourly chart at hours 0, 7, 12, 17, 22 (one per band).
- Assert KPI strip presence on Overview at width 120 and Narrow shorthand at width 80.

## What this spec does not cover

- Week / Month / Settings tab bodies — out of Today-first scope. They will inherit the same theme tokens and layout primitives; their specific layouts are a later review (port from `WeekPage.tsx` / `MonthPage.tsx` / desktop settings dialog respectively).
- Mouse interactions — explicitly disabled in `ui::setup_terminal()` per the keyboard-only design decision.
- Light-mode / theme switching — desktop is dark-locked in `App.tsx`; TUI follows. Revisit when desktop revisits.
- Animations / transitions — TUI re-renders on dirty events; transient indicators (`↻` while in-flight) animate via the 250ms tick. No motion design beyond that.

## Cross-references

- Desktop tokens: `src/index.css` (`:root` and `.dark`).
- Desktop Overview hierarchy (the basis for the TUI's Today tab): `src/pages/Overview.tsx`.
- Desktop screenshot: `public/demo.png` (also pinned in README).
- TUI implementation: `crates/daylog-tui/src/{ui.rs, ui/overview.rs, app.rs}`.
- Shared aggregations: `crates/daylog-core/src/{aggregate.rs, kpi.rs (new)}`.
    