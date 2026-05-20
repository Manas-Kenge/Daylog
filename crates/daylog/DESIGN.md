# Daylog TUI — Design Spec

The TUI's job is to feel like a tool a terminal-native builder would keep open, not a generic ratatui demo. This file is the source of truth for that: how Daylog's three tabs translate the same dataset into a single coherent visual language.

The redesign locked in on 2026-05-15 deliberately broke from the previous spec's "translate the desktop CSS variables onto a terminal" framing. The desktop is gone (archived to `archive/desktop`), and TUIs that look polished do so by leaning into terminal idioms — typography hierarchy, restraint with chrome, selective color — not by emulating desktop polish.

The reference set for this redesign: lazygit, k9s, btop, atuin, htop, dust. None of them use bordered panels for every data region; all of them lean on bold/dim typography and section dividers to do the work that borders do badly.

## Visual identity — what "looks like Daylog" means

Three things carry the brand:

1. **Ember/orange signature accent.** Reserved for the *active range chip* and as the focus indicator on the eventually-shipping panel-focus signal. Used *less* than in the previous spec — when you see ember, it means "this is selected." `oklch(0.70 0.16 45)`.
2. **Activity Spectrum** — a 5-band warm-to-cool gradient mapping morning → night across the hourly chart. Orange at dawn, violet at midnight. Daylog's single most distinctive visual moment, kept exactly as before.
3. **Category color tokens** — Work/Comms/Media/Browsing/Documents/Other. These now do double duty: they paint the today-timeline barcode (Today tab), the horizontal weekday bars (Week tab), and the categories column's bar fills (every tab). The colors *are* the legend; the always-on legend row is gone.

The TUI translation:

- Where the desktop used opacity, the TUI uses `Modifier::DIM`.
- Where the desktop used gradient, the TUI uses 5 fixed bands (never interpolate).
- Where the desktop used border-radius and shadow, the TUI uses **spacing and dim horizontal rules** — not box borders.
- Where the desktop used hierarchical font-weight, the TUI uses `Modifier::BOLD` + uppercase section headers.

## Aesthetic — Dense Console

The design grammar belongs to the lazygit/k9s/htop family: information density is the design, not chrome. The discipline is restraint — every visual element pays its rent in conveyed information.

| Dimension | Decision |
|---|---|
| Decoration level | Minimal. Dim horizontal rules separate sections; single `│` characters separate columns. No decorative blobs, no gradients-as-decoration. |
| Layout approach | Grid-disciplined, borderless. Every tab follows the same 4-band rhythm (header → snapshot → hero → rollups → footer). |
| Color approach | Restrained. Ember is rare and means "selected." Spectrum colors carry hour-of-day. Category colors carry category identity. No decorative color. |
| Typography | Two-tone: `BOLD` for headers and values, `DIM` for labels and metadata. Uppercase for section headers. |
| Spacing density | Compact. 1-col left gutter, 1 blank row between snapshot and hero, dim rules between sections. No padding inside columns. |
| Motion | Minimal. Existing tachyonfx tab transitions stay. Throbber moves inline next to section header rather than overlaying a skeleton in a box. |

## The 4-band rhythm

Every tab body lays out vertically as:

```
┌─ Header band ─────────────────────────────────────────────────────────────┐
│ Numbered tabs (left). Range cycles via `r` / `Shift-R` keybind, no chip UI.│
├─ Snapshot band ───────────────────────────────────────────────────────────┤
│ 4 label-on-top / value-below pairs in a row, comma-separated style        │
├─ Hero band ───────────────────────────────────────────────────────────────┤
│ Per-tab visualization. Today → timeline barcode. Week → horizontal weekday │
│ bars. Month → year heatmap.                                                │
├─ Rollups band ────────────────────────────────────────────────────────────┤
│ 3 columns separated by single │ rules: apps │ categories │ domains         │
├─ Footer band ─────────────────────────────────────────────────────────────┤
│ Tab cycle · r range · ? help · q quit                                      │
└────────────────────────────────────────────────────────────────────────────┘
```

The hero is where each tab gets its own identity. The chrome — header, snapshot, rollups, footer — stays identical across tabs so the product feels coherent. Section boundaries are dim horizontal rules drawn as a row of `─` characters in `border_dim_style()`, not box borders.

## Color tokens

`crates/daylog/src/theme.rs` is the single source for every color, modifier, and spacing constant. **No widget reaches into `ratatui::style::Color::*` directly.** Every widget pulls from `theme::Theme`.

`Theme::detect()` reads `$COLORTERM` and `$TERM` at startup and picks one of three palettes:

| Tier | Trigger | How colors are encoded |
|---|---|---|
| `Truecolor` | `COLORTERM=truecolor` or `COLORTERM=24bit` | `Color::Rgb(r,g,b)` from OKLCH conversions |
| `Color256` | `TERM` contains `256color` and no truecolor `COLORTERM` | `Color::Indexed(n)` with hand-picked nearest neighbours |
| `Ansi16` | Floor — anything else, including dumb SSH | `Color::Yellow` for ember, `Red/Yellow/Green/Cyan/Magenta` for spectrum |

Detection runs once at startup; the result is stashed on `App`. No per-frame env reads.

### Token table

| Token | OKLCH | Truecolor RGB | 256-color | ANSI-16 | Used for |
|---|---|---|---|---|---|
| `bg` | `oklch(0 0 0)` | `(0,0,0)` | `Indexed(0)` | `Black` | Background (don't paint — let terminal decide) |
| `fg` | `oklch(0.985 0 0)` | `(251,251,251)` | `Indexed(231)` | `White` | Body text, headers, values |
| `dim` | `oklch(0.708 0 0)` | `(176,176,176)` | `Indexed(244)` | `White` + `Modifier::DIM` | Labels, metadata, inactive tabs |
| `border_dim` | `oklch(1 0 0 / 12%)` | `(31,31,31)` | `Indexed(236)` | `Black` + `Modifier::DIM` | Section dividers, column separators |
| `ember` | `oklch(0.70 0.16 45)` | `(229,154,110)` | `Indexed(173)` | `Yellow` | Active range chip background only |
| `chart_1` | `oklch(0.72 0.17 50)` | `(238,159,99)` | `Indexed(215)` | `Yellow` | Spectrum hours 0–4 + Work category bars |
| `chart_2` | `oklch(0.78 0.14 80)` | `(218,191,108)` | `Indexed(186)` | `Yellow` + bold | Spectrum hours 5–9 + Comms category bars |
| `chart_3` | `oklch(0.72 0.10 145)` | `(141,189,142)` | `Indexed(108)` | `Green` | Spectrum hours 10–14 + Media category bars + top-app bars |
| `chart_4` | `oklch(0.70 0.10 200)` | `(115,180,202)` | `Indexed(110)` | `Cyan` | Spectrum hours 15–19 + Browsing category bars + top-domain bars |
| `chart_5` | `oklch(0.62 0.12 270)` | `(126,131,201)` | `Indexed(104)` | `Magenta` | Spectrum hours 20–23 + Documents category bars |
| `error` | `oklch(0.65 0.20 25)` | `(228,113,99)` | `Indexed(167)` | `Red` | Offline indicator, fetch errors |

RGB values are computed offline from OKLCH; do not recompute at runtime. 256-color indices were picked by visual inspection on a kitty + xterm baseline.

### Spectrum mapping (hour-of-day)

5-band fixed mapping for the hourly distribution chart:

```
band = match h {
    0..=4   => chart_1,   // 12am–4am: warm orange
    5..=9   => chart_2,   // 5am–9am:  yellow
    10..=14 => chart_3,   // 10am–2pm: green
    15..=19 => chart_4,   // 3pm–7pm:  cyan
    20..=23 => chart_5,   // 8pm–11pm: violet
};
```

**Do not interpolate** — the fallback chain only works if every band has a defined truecolor / 256 / ANSI-16 entry.

### Category color mapping

Used by the today-timeline barcode, stacked weekday bars, and the Top Categories column's bar fills:

```
"Work" | "Programming" => chart_1   (orange)
"Comms"                => chart_2   (yellow)
"Media"                => chart_3   (green)
"Browsing"             => chart_4   (cyan)
"Documents"            => chart_5   (violet)
_                      => dim       (Other / Uncategorized)
```

## Typography

Two-tone weight system. Use them like the desktop used font-weight + opacity.

| Element | Style |
|---|---|
| Section headers (`TOP APPS`, `ACTIVE MINUTES PER HOUR`) | `fg` + `BOLD`, **uppercase** |
| Snapshot label ("ACTIVE", "LONGEST") | `dim`, **uppercase**, no DIM modifier |
| Snapshot value ("5h 30m") | `fg` + `BOLD` |
| Snapshot sub-detail (" Work", " vs Tue") | `dim`, no DIM modifier |
| Hero numbers (peak day label, heatmap legend) | `fg` + `BOLD` |
| Top-N row: name column | `fg` + `BOLD` |
| Top-N row: duration column | `fg` |
| Top-N row: rank, bar | `dim` + per-column bar color |
| Section divider (`─`) | `border_dim` + `DIM` |
| Column separator (`│`) | `border_dim` + `DIM` |
| Active tab | `REVERSED`, never `DIM` |
| Inactive tab | `dim` + `DIM` |
| Throbber (inline next to section header) | `dim` |
| Footer hint keys (`Tab`, `r`, `?`, `q`) | `fg` + `BOLD` |
| Footer hint labels (`cycle`, `range`, `help`) | `dim` |
| Error message | `error` |

**Avoid the "double dim" trap.** `Modifier::DIM` composed with an already-dim color drops to invisible on linux console and several 256-color terminals. Labels that use `theme.dim` foreground do **not** also get `Modifier::DIM`. Tabs and chips that use `Modifier::DIM` rely on the modifier for the dimming, not a dim foreground.

## Glyphs

The eighth-block bar ladder is the workhorse:

```
▏  ▎  ▍  ▌  ▋  ▊  ▉  █
```

`proportional_bar(value, max, width)` returns a string of these glyphs at full sub-cell resolution. The bar fills the column's allocated slot rather than a fixed 8-cell width. Slot widths:
- Wide (≥100 cols): 4 cells per bar in the 3-column rollups.
- Narrow (80–99 cols): 6 cells in the 2-column rollups.
- Stacked (<80 cols): 10 cells in the single-column rollups.

Zero-value rows get a `·` (`U+00B7`) so the rank line still has a glyph at its end.

Other recurring glyphs:

| Glyph | Code | Use |
|---|---|---|
| `▌` | `U+258C` | Today-timeline barcode cell (half-block gives 2× horizontal resolution) |
| `▆` | `U+2586` | Weekday bar cell (horizontal, on the Week tab hero) — lower 3/4 block leaves a 1/4-cell gap above each row |
| `░ ▒ ▓ █` | `U+2591` `U+2592` `U+2593` `U+2588` | Year heatmap intensity ladder |
| `─` | `U+2500` | Section divider rule |
| `│` | `U+2502` | Column separator |
| `↻` | `U+21BB` | Inline throbber (idle / non-spinning fallback) |
| `↑` `↓` | `U+2191` `U+2193` | Pattern-shift direction in snapshot |

No box-drawing for panels — borders are deleted globally for data regions.

## Spacing

- **Left gutter:** 1 column inside every band.
- **Section divider:** 1 row of `─` characters at `border_dim` + `DIM`, drawn across the full width of the body.
- **Inter-band spacing:** the divider row *is* the spacing; no extra blank rows except between Snapshot and Hero on the Today tab (1 blank row gives the timeline barcode breathing room).
- **Column spacing in rollups:** 1 column of whitespace on each side of every `│` separator.
- **Stat-pair spacing in snapshot:** label and value sit on adjacent lines; pairs are separated by ≥4 columns of whitespace.

## Layout

### Width breakpoints

`Theme::layout_mode(width: u16)` returns one of:

| Mode | Trigger | Rollups | Snapshot | Hero |
|---|---|---|---|---|
| `Wide` | ≥100 cols | 3 columns: apps │ categories │ domains | 4 stat pairs | Full width |
| `Narrow` | 80–99 cols | 2 columns: apps │ categories | 3 stat pairs (drop pattern-shift suffix) | Full width |
| `Stacked` | <80 cols | 1 column at a time, stacked: apps then categories then domains | 2 stat pairs (drop best-window) | Full width, may abbreviate labels |

The hero band always takes full width — it's the focal element on every tab and shouldn't be the first thing to lose space.

### Today tab

| Band | Height | Content |
|---|---|---|
| Header | 1 | `1 Today  2 Week  3 Month` (left). Range cycles via `r` / `Shift-R`, no on-screen chips. |
| Divider | 1 | `─` rule |
| Snapshot | 2 | `ACTIVE / LONGEST / BEST WINDOW / PATTERN` label-value pairs |
| Blank | 1 | breathing room before hero |
| Hero | 3 | `TODAY · so far` header (1 row) + barcode (1 row of `▌` half-blocks colored per category) + hour ruler (1 row) |
| Divider | 1 | `─` rule |
| Rollups | 7 | 3 columns: top apps │ top categories │ top domains, header row + 5 data rows + 1 padding |
| Divider | 1 | `─` rule |
| Hourly | 10 | `ACTIVE MINUTES PER HOUR` header (1 row) + 1-row drop + spectrum chart (~7 rows incl. y-axis) + hour ruler (1 row) |
| Divider | 1 | `─` rule |
| Footer | 1 | `Tab: cycle  │  r: range  │  ?: help  │  q: quit` |

Total: 29 rows. The bottom flex band absorbs slack on taller terminals; on the 24-row floor the hourly chart still fits because the flex slot collapses to zero.

### Week tab

| Band | Height | Content |
|---|---|---|
| Header | 1 | tabs (same as Today) |
| Divider | 1 | `─` rule |
| Snapshot | 2 | `TOTAL ACTIVE / DAILY AVG / HIGHEST DAY / TOP CATEGORY` |
| Blank | 1 | breathing room |
| Hero | 9 | `THIS WEEK · Mon → Sun` header + inline category legend (1 row) + 7 horizontal weekday rows (one row per day: short weekday label, total duration, category-stacked horizontal bar, optional `← peak` annotation) + 1 padding |
| Divider | 1 | `─` rule |
| Rollups | 7 | same 3-column shape, `· 7d` suffix on each header |
| Divider | 1 | `─` rule |
| Footer | 1 | same as Today |

Total: 23 rows.

The weekday rows lay out left-to-right as `Day(4 cols) │ Duration(7 cols) │ gap(2) │ bar(remaining) │ peak annotation(8 cols)`. Bar cells are filled left-to-right with `█` glyphs coloured by category root via `category_root_style`, in the same `WEEK_ROOT_ORDER` used by the legend. Past days with zero activity get a single `·` glyph at the bar's left edge. Future days the user hasn't reached yet render the day label dim and skip duration + bar. The peak day (highest `total_active_secs`) gets an inline `← peak` annotation in `dim` in the reserved 8-col slot at the right; non-peak rows leave that slot blank so every bar shares the same maximum width.

### Month tab

| Band | Height | Content |
|---|---|---|
| Header | 1 | tabs (same) |
| Divider | 1 | `─` rule |
| Snapshot | 2 | `TOTAL ACTIVE / DAILY AVG / BEST DAY / TOP CATEGORY` |
| Blank | 1 | breathing room |
| Hero | 9 | `YEAR HEATMAP` header + month-abbrev row + 7 weekday rows + intensity-scale legend row |
| Divider | 1 | `─` rule |
| Rollups | 7 | same 3-column shape, `· 30d` suffix |
| Divider | 1 | `─` rule |
| Footer | 1 | same |

Total: 23 rows.

Heatmap retains the existing 53-week column structure (one column per ISO week, Mon top → Sun bottom). The intensity-scale legend `less ░ ▒ ▓ █ more` lives right-aligned on the row below the heatmap, replacing what used to be a panel border.

## Tabs

One row, left-aligned. No range-chip UI — the range cycles via keybind only.

```
1 Today  2 Week  3 Month
```

- **Tabs.** Numbered prefix is the keyboard hint (`1`/`2`/`3` jump directly). Active tab gets `REVERSED`. Inactive tabs are `dim` + `DIM`. No pill backgrounds, no spacing tricks — width stays uniform.
- **Range.** Cycles via `r` / `Shift-R`. No on-screen affordance. The active range scopes the data shown on Week and Month tabs; the current value is visible in the data itself (e.g. `· 7d` suffixes on rollup headers, day counts in snapshot values).

## Borders — none

There is exactly one bordered widget left in daylog: the help overlay (popover). Every other data region — KPI snapshot, today timeline, top apps/categories/domains, hourly chart, stacked weekday bars, year heatmap, week/month stats — is **borderless**.

Hierarchy comes from:
1. **Bold uppercase section headers.** Reads as "this region is X."
2. **Dim horizontal rules** between major sections.
3. **Single `│` characters** between columns within a section.
4. **Whitespace** for inter-band breathing room.

When panel-focus eventually ships (keyboard navigation between sections), the focused section gets a thin `▎` (`U+258E`) left-edge marker in `ember`, drawn into the gutter. This is the only post-redesign use of ember besides the active range chip.

## Motion

- **Tab transitions.** Existing tachyonfx `Effect::process` sweep stays. Scoped to body only — header and footer don't flicker.
- **Throbber.** Inline next to the section header (`TOP APPS ↻`) on slots that are in-flight. No skeleton-in-a-box overlay anymore. The `BRAILLE_SIX_DOUBLE` set stays; the placement changes.
- **Refresh interval.** Unchanged (live polling per the existing `REFRESH_LIVE` cadence).
- **No new motion.** No scroll-driven, no spring physics, no entrance animations. The terminal re-renders on dirty events; that's the entire motion budget.

## Interaction states

- **Loading.** Section header dim + inline throbber. Hero band shows a single dim `…` centered in its area.
- **Empty.** Section header dim + 1-line hint in dim below (`no app events yet`, `install browser extension to track domains`).
- **Error.** `theme.error` foreground on a 1-line message in the section's normal content area.
- **Offline.** Single 1-line message in the footer's left zone: `○ tracker offline` in `theme.error`. Pushes out the keybinds when present.
- **Fresh install.** When total events across all sections is zero and uptime < 5 minutes, replace per-section empty messages with one global hero-area message: `ActivityWatch is collecting data — check back in a few minutes`.

## Keyboard

- `1`–`3` — jump to Today / Week / Month directly (numbered tab prefix is the affordance).
- `Tab` / `Shift-Tab` — cycle tabs forward / back.
- `h` / `l` / `←` / `→` — same as Tab/Shift-Tab.
- `r` / `Shift-R` — cycle range chip forward / back.
- `?` — toggle help overlay.
- `q` / `Esc` / `Ctrl-C` — quit.

The `1`/`2`/`3` direct-jump is intentionally redundant with the numbered tab prefix — the number on screen *is* the binding. Don't change one without changing the other.

## Snapshot tests

`overview_renders_top_apps_categories_and_hourly` (and equivalents for Week/Month) is content-based, not byte-exact, so it survives layout changes. Extend it for the redesign:

- Assert section headers exist as bold uppercase strings (`TOP APPS`, `TOP CATEGORIES`, etc.).
- Assert no `Block::default().borders(Borders::ALL)` is constructed in the render path (except the help overlay).
- Assert eighth-block ladder glyphs appear in bar columns (presence of `▏`/`▎`/`▍`/`▌`/`▋`/`▊`/`▉`/`█` is enough — exact composition depends on widths).
- Assert spectrum colors at hours 0, 7, 12, 17, 22 (one per band).
- Assert category colors on the today-timeline barcode at known event times.
- Assert numbered tab prefix (`1 Today`, `2 Week`, `3 Month`) renders verbatim.
- Assert no always-on category legend row appears (it lives inline on Week's hero now, gone elsewhere).

## What this spec does not cover

- **Settings tab.** The current 3-tab structure has no Settings; the previous spec's row for it was speculative. When settings ships, it inherits the same 4-band rhythm.
- **Mouse interactions.** Explicitly disabled in `ui::setup_terminal()`.
- **Light-mode theme switching.** Daylog is dark-only. Revisit if there's demand.
- **Color customization.** Out of scope for now. The tier detection + OKLCH-grounded palette is the single source of truth.

## Cross-references

- Theme tokens: `crates/daylog/src/theme.rs`.
- Render entry: `crates/daylog/src/ui.rs`.
- Per-tab renderers: `crates/daylog/src/ui/{overview, week, month}.rs`.
- Hero widgets: `crates/daylog/src/ui/{timeline, stacked_bars}.rs` (the latter now hosts the horizontal `HorizontalBars` widget) + `month::render_heatmap`.
- KPI compute: `crates/daylog/src/data/kpi.rs`.
- Shared aggregations: `crates/daylog/src/data/aggregate.rs`.

## Decisions log

| Date | Decision | Rationale |
|---|---|---|
| 2026-05-08 | Initial spec (D1–D6) translating desktop CSS variables onto a terminal. | At the time, the goal was parity with the desktop app. |
| 2026-05-20 | Bumped Today tab's hourly chart from 6 rows to 10 (chart body grows from ~3 plotting rows to ~7) so the 5-band spectrum reads as distinct colour bands instead of a thin smear. Slack is absorbed by the bottom flex slot. Switched Week tab's `7-DAY ACTIVITY BREAKDOWN` from 7 vertical stacked columns to 7 horizontal rows; bar height became row height (1 per day), bar width became proportional to total activity. Reuses `allocate_segments` for the per-row category split so colours and rounding behaviour are identical to the previous vertical bars. | Vertical bars at 70%-wide card width gave each column only ~5 cells, which made small-category segments visually invisible. Rows give each day a full-width bar instead, fix the cramped column problem, and read more like the Top-N rollups (label + duration + bar) the user already scans elsewhere in the app. |
| 2026-05-15 | **Redesign locked.** Deleted panel borders globally. Adopted 4-band rhythm shared across all three tabs. Numbered tab prefix replaces pill-style active tab. Always-on category legend row deleted; legend lives inline on Week's hero where the colors carry the most signal. Eighth-block bar ladder replaces fixed 8-cell `█`/`░` bars. Ember scope reduced to future focus indicator only. Top-app bars use `chart_3` (green); top-domain bars `chart_4` (cyan); category bars per-category. **Range chips remain unrendered.** The chips exist in `app.rs` state and cycle via `r` / `Shift-R`, but there's no on-screen affordance — the active range is implicit in the data labels (`· 7d`, `· 30d` rollup suffixes). | Desktop is archived; previous spec's "translate the desktop" framing was obsolete. The reference set for modern TUI design (lazygit, k9s, btop, atuin) uses borderless typography-driven hierarchy and that's where polished TUIs land in 2026. Range-chip UI was considered and rejected as adding noise to a tracker the user opens once a day. |
