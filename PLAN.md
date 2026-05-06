# Daylog — Engineering Plan

A native Linux desktop dashboard for ActivityWatch.

> **Working name.** Rename freely; nothing in this plan depends on it.

---

## 1.0 Post-CEO-review addendum (2026-05-06)

After a `/plan-ceo-review` session, the dashboard scope was deliberately expanded under a re-affirmed observational thesis. The *thesis* didn't change — Daylog remains observational, not nudging — but the *KPI surface* was reshaped to lean harder into pattern-discovery, the unique-to-Daylog wedge that cloud trackers can't match (no round-trip latency) and ActivityWatch can't deliver (no UI).

Locked decisions:

1. **KPI strip rebuilt around discovery, not scoring.** Replace `Productive` with `Best Window`; replace `Peak hour` with `Pattern shift`; replace `Started` with `Cadence`. New 5-up: **Active · Best Window · Longest stretch · Cadence · Pattern shift**. Every card carries a one-line "vs trailing-7-day median" sub-text — the wedge.
2. **"Notable today"** card replaces `CurrentFocus` on row 3 — surfaces 1–2 daily anomalies vs rolling baseline. CurrentFocus moves to the mini-window (where ambient widgets belong).
3. **Timeline becomes the visual hero**, not row 2 of 3 equal rows. Yesterday-ghost rendered underneath at low opacity for at-a-glance comparison.
4. **AFK is visible everywhere** — Timeline shows idle stripes, `Cadence` lists idle gaps, `Active` subtracts AFK with graceful degrade when no AFK bucket is present.
5. **Click-to-filter palette wiring** — donut segments, app rows, and category badges open the palette with the filter typed in.
6. **Week (stacked bars) + Month (calendar heatmap) pages** added as palette-reachable detail views.
7. **Mini-window pulled forward from v0.2 → v0.1.** `--mini` CLI flag and palette command spawn a frameless 320×120 always-on-top secondary `WebviewWindow`. Cross-DE always-on-top is acknowledged-fiddly; ship without always-on-top first if the per-DE work overruns.
8. **"Productive" terminology renamed to "Time in Work"** wherever it persists. The judgment frame ("productive" implies the rest is unproductive) is replaced with descriptive language. `productive_roots` → `work_roots` rename in `lib/productive.ts` follows from this.

Out-of-scope, re-confirmed: Focus Score gauges, productive-time goals, app-usage limits, weekly streaks, Pomodoro timers. These are nudging mechanics. Daylog measures; it does not intervene. If anyone asks for these post-launch the answer is "RescueTime is over there."

The sections below remain the implementation source of truth. §5 (Overview composition), §6 (deferred items), §13 (definition of done), and §14 (v0.2 roadmap) have been updated to reflect this addendum.

---

## 1. Vision

A single-window native desktop app that shows a beautiful, dense, real-time view of your day, sourced from the local ActivityWatch server you already have running. No browser tab. No sign-in. No cloud. The whole thing fits in one window with poster-quality information density.

**v0.1 hero scenario:** double-click the Daylog icon → a window opens showing today's timeline as a horizontal heatmap, top apps + categories with sparklines, and a live focus-session timer. Hit `⌘K` (or `Ctrl+K`) and a Raycast-style command palette appears: type `yesterday`, the dashboard reflects yesterday's data; type an app name, jump straight to its detail. The screenshot we ship on is the dashboard with the palette open mid-typing — keyboard-driven activity awareness, not another point-and-click tracker.

**Why palette-primary, not a sidebar:** AW's existing WebUI is already a dashboard. The reason a power user installs a desktop client instead of bookmarking `localhost:5600` is **surface availability**, not visual density. Visual density is table stakes. The keyboard-summonable palette is the v0.1 differentiation hook; ambient surfaces (topbar applet, pinned mini-window) follow in v0.2.

**Constraints locked in office-hours:**
- Linux-first, single-user, local-only. **Universal across distros** — Ubuntu, Debian, Fedora, Arch, openSUSE, Pop, Mint, Manjaro, Void, Alpine, etc. — not Debian-only.
- `aw-server-rust` and `aw-awatcher` binaries are **bundled inside Daylog's AppImage, `.deb`, and `.rpm` artifacts** and managed at runtime by user-scope systemd services (or an XDG-autostart supervisor on non-systemd distros), installed on first launch. We don't fork or modify their source; we ship their binaries (both MPL-2.0).
- **Daylog the tracker is a background daemon, not a Tauri sidecar.** It starts when the user logs in and stops when the user logs out — the macOS Screen Time model. Closing the Daylog window does not stop tracking; the window is just a viewer that connects to `localhost:5600` when opened.
- If the user already has ActivityWatch running on `:5600`, Daylog detects it and uses it instead of starting our bundled stack — never two servers fighting for the same port.
- We own the UI, the local API client, the first-launch setup flow, and the unit/autostart templates.
- Visual density beats feature density. Distribution is a Phase-0 concern: a single AppImage download must put a working dashboard in front of a fresh user within 60 seconds, on any modern Linux distro.

---

## 2. Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│  Daylog AppImage / .deb / .rpm    (one of three carrier formats)      │
│  ──────────────────────────────────────────────────────────────      │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ Daylog app (foreground; runs only when the user opens it)       │  │
│  │                                                                │  │
│  │  ┌──────────────────────────────────────────────────────────┐  │  │
│  │  │ WebView (WebKitGTK)                                      │  │  │
│  │  │ React 19 + Tailwind 4 + shadcn/ui                        │  │  │
│  │  │ TanStack Query + ECharts                                 │  │  │
│  │  └────────────┬─────────────────────────────────────────────┘  │  │
│  │               │ Tauri IPC                                      │  │
│  │  ┌────────────┴─────────────────────────────────────────────┐  │  │
│  │  │ Rust core                                                │  │  │
│  │  │  - HTTP client → :5600                                   │  │  │
│  │  │  - First-launch wizard                                   │  │  │
│  │  │  - Tracker install/control (systemd or XDG fallback)     │  │  │
│  │  └──────────────────┬───────────────────────────────────────┘  │  │
│  └─────────────────────┼──────────────────────────────────────────┘  │
│                        │ HTTP/JSON                                   │
│                        ▼                                             │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ Background tracker (always running while the user is logged in)│  │
│  │ ────────────────────────────────────────────────────────────   │  │
│  │  daylog-aw-server.service   →  {BIN_DIR}/aw-server-rust         │  │
│  │     listens on localhost:5600                                  │  │
│  │     stores SQLite at ~/.local/share/activitywatch/             │  │
│  │                                                                │  │
│  │  daylog-awatcher.service    →  {BIN_DIR}/aw-awatcher            │  │
│  │     pushes window/AFK events to localhost:5600                 │  │
│  │     (depends on focused-window-dbus on GNOME-Wayland only)     │  │
│  │                                                                │  │
│  │  Unit files:    ~/.config/systemd/user/   (always user-level)  │  │
│  │  Fallback:      ~/.config/autostart/daylog-tracker.desktop      │  │
│  │                 + supervisor.sh loop  (non-systemd distros)    │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  {BIN_DIR} resolves at runtime:                                      │
│   - AppImage:       ~/.local/share/daylog/bin/                        │
│                     (Daylog extracts on first launch + version drift) │
│   - .deb / .rpm:    /usr/lib/daylog/bin/                              │
│                     (placed by the package manager)                  │
│                                                                      │
│  GNOME-Wayland only (installed user-level on first launch):          │
│   ~/.local/share/gnome-shell/extensions/                             │
│      focused-window-dbus@flexagoon.com/                              │
└──────────────────────────────────────────────────────────────────────┘
```

**Three key separations to keep in mind:**

1. **Daylog the window vs Daylog the tracker.** The foreground app is "the dashboard you open." The two background services (or the supervisor process on non-systemd distros) are "the tracker that runs always." Closing the window does not stop tracking. Uninstalling Daylog stops both — `.deb` and `.rpm` pre-remove hooks call `systemctl --user stop` for each logged-in user; AppImage users get a `daylog --uninstall-tracking` CLI flag.
2. **Carrier vs runtime path.** The package format (AppImage / .deb / .rpm) is just a carrier for the binaries. The *runtime path* the services point at is always stable and never the AppImage mount — we resolve `{BIN_DIR}` per the table above and the unit files always reference that resolved path. Re-extraction happens automatically on AppImage version change.
3. **Our stack vs an existing AW install.** On first launch, we probe `:5600`. If something answers (the user has their own AW), we use it and never install our services. If nothing answers, we install and enable our bundled tracker. This is the "don't fight an existing setup" rule — without it, two servers race for the port and one silently loses.

**Why HTTP through the Rust core, not directly from the WebView?** Three reasons:
1. CORS — `aw-server` doesn't allow arbitrary webview origins. Rust-side calls are origin-free.
2. Polling logic, retries, reconnection, and the systemd-control commands live in one stable place.
3. The Rust side already needs to install/control the tracker (systemd or XDG-autostart) and probe `:5600` on launch — keeping all server-touching code in one place avoids divergence.

The frontend never sees `localhost:5600`. It only calls Tauri commands like `aw_today_window()` and listens to events like `daylog:bucket-updated`.

---

## 3. Tech stack (locked)

| Layer | Choice | Reason |
|---|---|---|
| Shell | **Tauri 2.x** | Tiny binary, sidecar support, native window, Linux-first packaging. |
| Frontend framework | **React 19 + TypeScript + Vite** | shadcn/ui is React; widest chart-library coverage. |
| Styling | **Tailwind CSS 4** | shadcn/ui is built on it; fastest path to design density. |
| Components | **shadcn/ui** | Owned source, not a dep. Easy to customize for visual density. |
| State / data | **TanStack Query v5** | Caches AW responses, dedupes polling, handles loading/error states declaratively. |
| Charts | **Apache ECharts** (via `echarts-for-react`) | Best-in-class for timelines, heatmaps, sparklines. License: Apache 2.0. |
| Date handling | **date-fns** | Tree-shakable; no Moment baggage. |
| Rust HTTP | **reqwest** + **tokio** | Tauri's stack. |
| Settings | **tauri-plugin-store** | Persistent JSON in app config dir. |
| Package manager | **bun** | Already installed; faster than npm/pnpm for fresh installs. |
| Linter | **Biome** | Replaces ESLint+Prettier in one binary. |
| Tests (frontend) | **Vitest** | Fast, Vite-native. |
| Tests (Rust) | **cargo test** | Built-in. |
| E2E | **Playwright** *(deferred to v0.2)* | Tauri WebDriver story is still rough; not worth the time in v0.1. |

**Locked.** Don't relitigate any of these without a concrete failure to point at.

---

## 4. Data model — what aw-server actually gives us

Confirmed against your live server (May 4, 2026):

```
GET /api/0/info
→ { hostname, version, testing, device_id }

GET /api/0/buckets/
→ {
    "aw-watcher-window_<host>": { id, type: "currentwindow", ... },
    "aw-watcher-afk_<host>":    { id, type: "afkstatus",     ... }
  }

GET /api/0/buckets/<id>/events?limit=N&start=ISO&end=ISO
→ [
    { id, timestamp, duration, data: { app, title } },              // window bucket
    { id, timestamp, duration, data: { status: "afk" | "not-afk" }} // afk bucket
  ]

POST /api/0/query   ← this is the workhorse for aggregations
→ runs an AQL query, returns aggregated buckets per day
```

`/api/0/query` is how the official WebUI computes "time per app today." We do the same. AQL is small and well-documented; we will likely use 4–6 query templates total.

**TypeScript types** (to write in Phase 2):

```ts
// src/lib/aw-types.ts
export interface AwEvent<T = unknown> {
  id: number;
  timestamp: string;     // ISO8601 with timezone
  duration: number;      // seconds
  data: T;
}
export interface WindowEventData { app: string; title: string; }
export interface AfkEventData    { status: "afk" | "not-afk"; }
export interface Bucket { id: string; type: string; client: string; hostname: string; }
```

---

## 5. v0.1 scope — what ships

A single-window app with one poster-quality dashboard view and a global command palette. **No sidebar.** Detail views (apps list, hourly patterns, activity log) are reachable via the palette, not via persistent navigation.

```
┌────────────────────────────────────────────────────────────────────┐
│ Daylog              today · 14:23 · 5h 18m              [⌘K]        │
├────────────────────────────────────────────────────────────────────┤
│ ┌────────┬──────────┬──────────┬──────────┬───────────────────┐    │
│ │ Active │ Best win │ Longest  │ Cadence  │ Pattern shift     │    │
│ │ 5h 18m │ 14-17    │ 47m Work │ 09:14 →  │ +2h Browsing      │    │
│ │ +18m   │ ▂▂▆▇▇▄▂  │ ▂▆█▇▃    │ now      │ vs typical Tue    │    │
│ │ vs typ │ +10% vs  │ +12m vs  │ 2 idle   │ (-31m Work)       │    │
│ │        │ typical  │ typical  │ gaps     │                   │    │
│ └────────┴──────────┴──────────┴──────────┴───────────────────┘    │
├────────────────────────────────────────────────────────────────────┤
│ Today's timeline (HERO ROW — ~50% of vertical space)               │
│ [▓▓░░▓▓▓▓░░░▓▓▓▓▓▓░░░░▒▒▓▓▓▓▓▓▓▓░░░░▒░  ▓▓▓░│ ◀ NOW              │
│  ░░░░▒▒▒░░░▒▒▒▒▒▒░░░░░░░▒▒▒▒▒▒▒▒░░░░░░  ▒▒▒░│  ← yesterday-ghost │
│ legend: Work · Comms · Browsing · Media · Other · │ idle (AFK)     │
│ hover: app + title at that instant                                 │
├────────────────────────────────────────────────────────────────────┤
│ Top apps              │ Top categories      │ Notable today        │
│ ────────────────      │ ────────────────    │ ─────────────────    │
│ kitty       2h 14m ▁▃▅│ Work        3h 02m  │ • 3 stretches of     │
│ firefox     1h 03m ▂▄▆│ Comms       0h 41m  │   Code 30+ min       │
│ Code        0h 42m ▁▂▃│ Browsing    0h 33m  │   (typical: 1)       │
│ Slack       0h 12m ▁▁▂│ (donut, click =     │ • Late start vs avg  │
│ click → palette filter│  filter to Apps)    │   (09:14 vs 08:42)   │
└────────────────────────────────────────────────────────────────────┘

   ↓  user hits ⌘K (or Ctrl+K)

┌────────────────────────────────────┐
│ ▍ Search Daylog...                  │
│  • Today                           │
│  • Yesterday                       │
│  • This week                       │
│  • Hourly patterns                 │
│  • Apps → kitty (2h 14m)           │
│  • Categories → dev work           │
│  • Activity log                    │
│  • Settings → Tracking             │
│  • ? Show shortcuts                │
└────────────────────────────────────┘
```

**Dashboard composition (`pages/Overview.tsx`, three rows, fits 1280×800 without scroll):**

| Row | Widget(s) | What it shows |
|---|---|---|
| 1 | `KpiStrip` (5-up) | Active · Best Window · Longest stretch · Cadence · Pattern shift. Each card carries a one-line "vs trailing-7-day median" sub-text. |
| 2 (HERO) | `Timeline` | 24h heatmap, color-encoded by category, AFK as low-opacity stripes, NOW indicator, yesterday-ghost rendered underneath at low opacity. Takes ~50% of vertical space. |
| 3 | `TopApps` \| `TopCategories` \| `NotableToday` | Three columns: top apps with sparklines (click → palette filter), top categories donut (click segment → palette filter), notable-today anomaly card. |

**KPI definitions (which questions each card answers):**

| Card | Question | Source | Notes |
|---|---|---|---|
| **Active** | "How long was I at the keyboard today?" | `aw_afk_summary` | Total active seconds (AFK subtracted). "vs typical" sub-text against trailing-7-day median. Degrades to "—" when no AFK bucket present. |
| **Best Window** | "When did I focus best today?" | `useCategorizedEvents` + focused-stretches reducer (`focusByHour`) | Hour-range with the highest concentration of qualifying focus runs (≥120s on a single category root). E.g., `14-17`. Inline sparkline of focus-by-hour with the window highlighted. |
| **Longest stretch** | "What was my deepest stretch today?" | `useCategorizedEvents` | Biggest uninterrupted run on a single category root, ≥120s floor. Sub-label shows the root. |
| **Cadence** | "What did my day actually look like?" | first/last event in range + AFK intervals | Start time, end time (or `now` if active), count of idle gaps ≥10min. Replaces the v0.1-original `Started` and partly `Peak hour`. |
| **Pattern shift** | "What's notable about today vs my typical day?" | trailing-7-day median per category, computed locally | The wedge metric. Surfaces the largest absolute delta against trailing-7-day median for the same weekday-class (workday vs weekend). E.g., `+2h Browsing vs typical Tue`. Suppressed until ≥7 days of history exist; placeholder reads `building baseline (N/7 days)`. |

The "Productive" concept (Work-rooted time) is no longer a dedicated KPI card — it's served by `TopCategories` (Work appears as a category row) and indirectly by `Pattern shift` (which surfaces deltas in Work specifically when they're the largest of the day). The `productive.ts` module is renamed to use `work_roots` terminology to drop the implicit judgment frame; Phase 4 settings UI lets the user edit this allowlist.

**Dropped from earlier KPI design** (and where the info now lives):

| Old card | Why dropped | Where it lives now |
|---|---|---|
| `Productive` | Single-number judgment metric. Implies the rest of the day was unproductive. Conflicts with the observational thesis. | Time in Work surfaces in `TopCategories`; deltas surface via `Pattern shift`. |
| `Started` | One number doesn't tell the day's story; idle gaps and end-of-day are equally informative. | Subsumed by `Cadence`. |
| `Peak hour` | Raw "most active hour" is less useful than "best *focus* window." Active ≠ focused. | Subsumed by `Best Window`. |
| `Activity %` | Just AFK ratio. 100% on a Twitter-all-day session. | AFK now visible directly in Timeline as low-opacity stripes. |
| `Switches` | No threshold for good/bad. Number with no signal. | Visible as color flicker in Timeline. |
| `Apps unique` | "12 apps" not actionable. | Header chip on `TopApps`. |
| `Top category` | Duplicates `TopCategories` donut directly below. | `TopCategories` widget. |

**Five widgets, exactly.** No scroll wall. The dashboard is range-aware: every widget consumes `RangeContext`, so palette commands like `Yesterday` or `This week` re-render the same layout against a different range — there is no separate "week dashboard" or "yesterday dashboard."

**Moved off the dashboard (reachable via palette, not Overview):**

| Widget / page | Detail destination | Palette command |
|---|---|---|
| `HourlyDistribution` | `pages/HourlyPatternsPage.tsx` (existing) | `Hourly patterns` |
| `WebPanel` | `pages/WebPage.tsx` (existing) | `Web` |
| `ActivityLog` | `pages/ActivityLogPage.tsx` (existing) | `Activity log` |
| `WeekStacked` (new) | `pages/WeekPage.tsx` — 7-day stacked bars by category root, with weekday-typical overlay | `Week` |
| `MonthHeatmap` (new) | `pages/MonthPage.tsx` — calendar heatmap of total active per day, GitHub-contributions style | `Month` |

`Week` and `Month` are new palette destinations added in this scope expansion. They share a query-batch pattern with `HourlyPatternsPage` (parallel `aw_hourly` / `aw_categorized_events` per day). Widgets live under `components/widgets/` so the palette can preview a thumbnail for each.

**`CurrentFocus` is no longer on Overview.** It moves to the v0.1 mini-window (see §14, pulled forward). Ambient widgets belong on ambient surfaces, not in the dashboard. Row 3's third column is now `NotableToday`.

**`NotableToday` widget:** surfaces 1–2 anomalies per day computed against trailing-14-day rolling stats. Examples:
- "3 stretches of Code 30+ min today (typical: 1)"
- "Late start vs typical Tue (09:14 vs 08:42)"
- "Unusually quiet morning — no Comms before 11:00"

Anomaly thresholds need empirical tuning. Start dumb: Z-score against trailing 14 days, suppress when absolute delta < 15min OR daily total < 30min. Empty state: "No notable patterns today." Card never claims certainty it doesn't have.

**Click-to-filter palette wiring:** clicking any tracked-app row in `TopApps`, any donut segment in `TopCategories`, or any category badge anywhere on the dashboard opens the command palette pre-populated with that filter (e.g., `> kitty` typed into the search). Pure wiring against the existing `PageContext.push()` — no new navigation surface, just makes the existing widgets *talk to each other* via the keyboard-summonable surface.

**Command palette (primary navigation):**
- Built on `cmdk` (via shadcn's `Command` component).
- Hotkeys: `⌘K` and `Ctrl+K` both bind globally inside the window. `Esc` dismisses. `?` opens shortcut help.
- Static commands: `Today`, `Yesterday`, `This week`, `This month` (range switch); `Apps`, `Categories`, `Hourly patterns`, `Week`, `Month`, `Activity log` (detail views); `Mini` (spawns the v0.1 mini-window — see §14); `Settings → Tracking | Categories | General`.
- Dynamic commands: each tracked app and category becomes a typeable result (`kitty 2h 14m → jump to detail`).
- Click-driven entry: clicking any donut segment, app row, or category badge on the dashboard opens the palette pre-populated with that term — palette remains the single navigation surface.
- Detail views render in the main pane. Approach undecided between sliding overlay (dashboard stays behind) and route swap; pick during impl.
- Affordance: a small `⌘K` chip in the topbar. First-launch toast hints at the shortcut. Not a hamburger menu.

**Functional requirements:**
- **First-launch wizard** (shown only on first run, dismissible after success):
  - Detects whether `:5600` is already answering. If yes → "We found an existing ActivityWatch install. Use it." If no → "Set up tracking" installs our bundled tracker (systemd user services on systemd distros, XDG-autostart supervisor otherwise).
  - On GNOME Wayland: detects whether `focused-window-dbus@flexagoon.com` is enabled. If not, runs `gnome-extensions enable` for it and prompts a logout/login.
  - Polls buckets after enabling services until first event arrives (max 15s timeout, with a clear error path).
  - Skipping the wizard puts the dashboard into a degraded state with a banner explaining what's missing.
- Auto-refresh every 5 seconds.
- Connection-state indicator (top right): green = aw-server reachable, amber = retrying, red = down.
- Window remembers its size and position between launches.
- Light/dark mode follows GNOME accent color (or manual override in settings).
- Settings panel with two sections:
  - **General:** timezone, refresh interval, category rules (initial UI: list editor).
  - **Tracking:** shows which mode is active (bundled / external), service status (`active`/`inactive`/`failed`), `Restart services` button, `Switch to bundled` / `Switch to external` toggle.

**Non-functional:**
- Cold start to first paint: ≤500ms after window creation.
- Idle CPU: <1% (the WebView is a chunk of memory but not CPU when nothing's animating).
- Binary size: <25MB AppImage, <15MB compressed `.deb`, <15MB `.rpm`.

---

## 6. NOT in v0.1 scope (deferred, with rationale)

| Item | Reason |
|---|---|
| Cross-platform builds (macOS/Windows) | Linux-first per office-hours premise P2. Multi-OS CI doubles the build matrix. Re-evaluate after v0.1 stars >100. |
| Sidebar navigation | Reverted to palette-primary in office-hours after a sidebar scaffold landed in `phase-3-dashboard`. Filling 8+ pages contradicts §1's "visual density beats feature density" rule. Sidebar code removed in Phase 3 rewrite. |
| Topbar applet (GNOME shell extension showing current focus + today total) | **Explicit v0.2 roadmap.** This is the ambient-surface companion to the palette. Deferred only because cross-DE work (GNOME / KDE / Sway) is its own engineering project and would push v0.1 past the weekend budget. |
| ~~Pinned mini-window~~ | **MOVED INTO v0.1 SCOPE** post-CEO-review (§1.0). Frameless 320×120 secondary `WebviewWindow` spawned via `--mini` CLI flag or `Mini` palette command. Always-on-top behavior is best-effort per-DE; ship without it first if cross-DE work overruns. |
| Tray icon / menubar widget | Linux tray support varies wildly across DEs (XEmbed vs SNI vs AppIndicator). The pinned mini-window above is the cross-DE alternative. |
| Notifications / focus alerts | Outside the "passive observer" identity of v0.1. |
| ~~Bundled `aw-server-rust` sidecar~~ | **MOVED INTO v0.1 SCOPE** — see Phase 5. Reframed: not a Tauri sidecar, but a session-scoped background daemon (systemd user unit, with XDG-autostart fallback). |
| Flatpak / Snap | **Deferred to v0.2.** Sandboxing fights our use case (D-Bus access, Wayland foreign-toplevel, talking to localhost:5600, GNOME extension install). Each portal is its own rabbit hole. AppImage covers the universal-Linux requirement without the sandbox tax. |
| AUR / nix / pacman / ebuild packages | **Community.** We don't ship distro-native packages beyond `.deb` and `.rpm`. We make the build reproducible (`bun run tauri build` after `scripts/fetch-binaries.sh`); packagers do the rest. |
| ~~Multi-day / weekly / monthly views~~ | **MOVED INTO v0.1 SCOPE** post-CEO-review (§1.0). `Week` (7-day stacked bars) and `Month` (calendar heatmap) palette destinations. |
| Goal targets / app-usage limits / streaks / Pomodoro | **Re-confirmed NOT in scope** post-CEO-review. Behavioral nudging is a separate product. Daylog measures; it does not intervene. If users ask, the answer is "RescueTime is over there." |
| Categorization rule editor (visual) | v0.1 ships with a JSON-edit list. Visual rule builder is its own feature. |
| AW bucket creation / event editing | We are read-only against aw-server in v0.1. Period. |
| GNOME Shell extension companion | Lateral path discussed in office-hours; revisit in v0.2 once the dashboard is done. |
| Cloud sync / multi-device | Violates premise P2 (local-only). |
| Goals / limits / "you've used Twitter for 2h" | Behavioral nudging is a separate product. Daylog v0.1 is observational. |
| Browser activity (aw-watcher-web) | Requires user to install Firefox/Chrome extension separately. Add a "you can also install this" hint in settings, but don't ship the extension. |

---

## 7. Implementation phases

Each phase is sized to fit one weekend (your time, not CC time). Phases are sequential — no parallel lanes for v0.1. Total estimate: **8–10 weekends to v0.1.0 release**.

### Phase 0 — Developer prerequisites (1 evening) — **for you, the developer**

These are the toolchain you need to *build* Daylog. **End users do not run any of these** — they install one `.deb`. You run these once on your dev machine:

```bash
# 1. Install rustup (gives you a current cargo + toolchain).
#    The Ubuntu rust package is incomplete and outdated — uninstall it first
#    or rustup will fight it.
sudo apt remove -y rustc cargo  # safe; nothing on your system depends on it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# accept defaults; restart shell so ~/.cargo/bin is on PATH

# 2. Install Tauri's Linux system deps
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf

# 3. Verify
rustc --version    # should be 1.80+
cargo --version
node --version     # already 24.x
bun --version      # already 1.3.x
```

**Exit criteria:** all four commands print versions, no errors.

### Phase 1 — Scaffold (2-3 hours)

```bash
cd ~/dev/projects
# Create the Tauri app inside the existing daylog/ directory.
# create-tauri-app is interactive; pick:
#   - app name: daylog
#   - identifier: com.manas-kenge.daylog  (or your domain)
#   - frontend: TypeScript / JavaScript
#   - flavor: React
#   - package manager: bun
bun create tauri-app

# Then add Tailwind 4 + shadcn:
cd daylog
bun add -d tailwindcss @tailwindcss/vite
bunx shadcn@latest init   # pick: TypeScript, default style, slate, CSS variables
bunx shadcn@latest add button card badge separator dialog input label tabs

# Add runtime deps
bun add @tanstack/react-query echarts echarts-for-react date-fns clsx tailwind-merge

# Rust side
cd src-tauri
cargo add reqwest --features json
cargo add serde --features derive
cargo add tokio --features full
cargo add tauri-plugin-store
cd ..

# Sanity check
bun run tauri dev
```

A blank Tauri window with a Vite React page should open. **If it doesn't, stop and debug — every later phase depends on this working.**

**Exit criteria:** `bun run tauri dev` opens a window, hot reload works on a `console.log` change.

### Phase 2 — AW client + types (1 weekend)

Goal: a typed, well-tested HTTP client that the frontend talks to via Tauri commands.

Deliverables:
- `src-tauri/src/aw/mod.rs` — Rust client over reqwest, returns typed structs.
- `src-tauri/src/aw/queries.rs` — AQL query templates (today's window time-per-app, today's AFK summary, current event).
- `src-tauri/src/lib.rs` — Tauri commands: `aw_info`, `aw_today_window`, `aw_today_afk`, `aw_current`.
- `src/lib/aw.ts` — thin TS wrapper around `invoke()` calls.
- `src/lib/aw-types.ts` — shared types (manually mirrored from Rust; consider `ts-rs` if it gets painful).
- Tests: `cargo test` covers query shape and event parsing against fixture JSON committed to `src-tauri/tests/fixtures/`.

**Exit criteria:** in dev tools console, `await window.__TAURI__.core.invoke('aw_today_window')` returns a sorted list of `{ app, duration }` from your real machine.

### Phase 3 — Dashboard widgets + command palette (3 weekends)

Two parallel tracks: the four hero widgets, and the palette that replaces sidebar navigation.

**Track A — widgets (build first):**

1. **Today's timeline (horizontal heatmap)** — hardest. Build it first, alone, until it's beautiful. ECharts `heatmap` series with custom tooltip. Color encodes category, height encodes nothing (it's a bar). Hover scrubs to that instant.
2. **Top apps** with sparklines — `bar` + `line` series.
3. **Top categories** — bars with category colors.
4. **Current focus** — text + a circular progress indicator (custom SVG, not ECharts).

Polling: a single `useQuery` per widget, all using the same query key prefix, refetchInterval 5s. TanStack Query dedupes fan-out.

**Track B — command palette:**

1. `bunx shadcn@latest add command` to bring in the `cmdk`-backed `Command` primitive.
2. `src/components/palette/CommandPalette.tsx` — modal overlay, search + result list, keyboard navigation.
3. `src/components/palette/commands.ts` — command registry. Static commands (ranges, detail views, settings) + dynamic providers (apps, categories pulled from current data).
4. `src/hooks/useHotkey.ts` — global `⌘K` / `Ctrl+K` binding. `Esc` to dismiss. `?` to show shortcut help.
5. Wire `RangeContext` to range commands. Wire detail commands to swap main pane content (try slide-over first; route-swap fallback).
6. **Delete** the sidebar scaffold landed in `phase-3-dashboard`: `components/layout/Sidebar.tsx`, `pages/Placeholder.tsx`, `lib/nav.ts`'s `NavId` and `PAGE_TITLES`. Rewrite `App.tsx` to drop the `232px_1fr` grid; main pane is full width.

**Exit criteria:**
- All four widgets render real data, refresh on a 5s tick, tolerate aw-server going down (show amber state, don't crash).
- `⌘K` opens the palette anywhere in the window. Typing `yesterday` switches the dashboard to yesterday's data within one frame.
- Typing an app name shows it as a result with its today-total; selecting it lands on a detail view.
- No sidebar exists in the rendered DOM.

### Phase 4 — Settings + categorization (1 weekend)

- Settings dialog (shadcn `Dialog`) with: refresh interval, theme, category rules.
- Category rules: list of `{ pattern: regex, category: string, color: hex }`. Persisted via `tauri-plugin-store`.
- Apply rules client-side when displaying — don't push back to aw-server.

**Exit criteria:** rules persist across app restarts; categorization in widgets reflects rules within one refresh tick.

### Phase 5 — Always-on tracking + universal-Linux bundling (2 weekends)

This phase converts Daylog from "works on your machine" to "works on any Linux distro a stranger is running, in the Screen Time model — track always, view on demand." It delivers four things:

1. A way to *carry* the binaries inside any package format (AppImage, `.deb`, `.rpm`).
2. A first-launch wizard that *installs* the tracker as a session-scoped background daemon on **any** Linux distro (systemd or not).
3. The optional GNOME-Wayland extension, installed user-level on detection.
4. Predictable update + uninstall paths across all three carrier formats.

The Daylog window is just an HTTP client of `localhost:5600` — closing it does not stop tracking. The two binaries (`aw-server-rust` + `aw-awatcher`) run for the duration of the user's login session and stop at logout.

**5a. Vendor the binaries.**

Add `scripts/binaries.lock` — a single source of truth for upstream versions and SHA-256 checksums, one tab-separated row per `(component, version, target, sha256)`. Auto-bumped by Renovate.

```
aw-server-rust  v0.13.2  x86_64-unknown-linux-gnu  8f62b10b…
aw-awatcher     v0.3.3   x86_64-unknown-linux-gnu  30b51a94…
```

**v0.1 is x86_64-Linux only.** Neither upstream publishes aarch64 release artifacts: `aw-server-rust` ships only inside `ActivityWatch/activitywatch`'s linux-x86_64 bundle zip (the `aw-server-rust` repo itself has no GitHub releases at all), and `2e3s/awatcher` ships only `x86_64.zip`. aarch64 (Asahi / Pi / arm laptops) is a v0.2 goal that requires building both from source.

Add `scripts/fetch-binaries.sh` — POSIX bash; deps `curl` + `unzip` + `sha256sum` + `awk`. Reads the lock, downloads the upstream zips, verifies SHA-256 against the lock, extracts the binary we want from each archive, and places it at `src-tauri/binaries/<binary>` for Tauri to bundle as a resource. (No target-triple suffix in the on-disk filename: v0.1 is x86_64-only, so the suffix would just be noise.) Cached by archive sha at `~/.cache/daylog/binaries/`; idempotent — re-running with a satisfied lock is a no-op. **No `dpkg-deb`, no `jq`** — script runs on macOS dev machines and any CI runner. Used as a `prebuild` step. Source mapping per component is inline in the script:
- `aw-server-rust` → extracted from `activitywatch-<version>-linux-x86_64.zip` (parent bundle).
- `aw-awatcher` → extracted from `aw-awatcher.zip` (own release).

Add `scripts/bump-binary.sh <component> <version>` — fetches the new tarball across all targets, computes their SHA-256, and rewrites `binaries.lock`. One-line PR diff per upgrade.

Add `.github/renovate.json` — `customManagers` regex over `binaries.lock`, `datasourceTemplate: github-releases` per component. Weekly schedule, grouped PRs. CI runs `fetch-binaries.sh && tauri build`; if green, merge.

In `tauri.conf.json`:
```json
{
  "bundle": {
    "active": true,
    "targets": ["appimage", "deb", "rpm"],
    "resources": [
      "binaries/aw-server-rust",
      "binaries/aw-awatcher",
      "services/daylog-aw-server.service.tmpl",
      "services/daylog-awatcher.service.tmpl",
      "services/daylog-supervisor.sh.tmpl",
      "services/daylog-tracker.desktop.tmpl",
      "extensions/focused-window-dbus@flexagoon.com.zip"
    ]
  }
}
```

The binaries are bundled as `resources` (not `externalBin`) because Daylog never spawns them via `Command::new_sidecar()` — they're owned by systemd / the supervisor, not by Tauri. Resources are accessed at runtime via `app.path().resolve("binaries/aw-server-rust", BaseDirectory::Resource)`, which returns the correct on-disk path inside whatever carrier Daylog is running from (AppImage mount, `/usr/lib/<id>/` for `.deb`/`.rpm`, or `src-tauri/target/...` in dev).

**5b. {BIN_DIR}: where the binaries actually live at runtime.**

Format-dependent, because AppImage mounts ephemerally:

| Carrier | `{BIN_DIR}` (resolved at first launch) | Owner |
|---|---|---|
| AppImage | `~/.local/share/daylog/bin/` — Daylog extracts the bundled binaries here on first launch and on every version change | User |
| `.deb`, `.rpm` | `/usr/lib/daylog/bin/` — the package manager places them at install time | System |

Resolution logic (`src-tauri/src/tracking/install.rs`):
1. Detect the install method: if `/usr/lib/daylog/bin/aw-server-rust` exists, that's our `{BIN_DIR}`. Else assume AppImage.
2. AppImage path: read the embedded binary version (compile-time constant), compare against `~/.local/share/daylog/bin/.version`. If missing or stale, copy the binaries from `tauri::path::resolve_resource("binaries/aw-server-rust")` etc. into the user dir, write the new `.version`, `chmod +x`.
3. Always render unit/desktop templates with the resolved `{BIN_DIR}`.

This decouples the *carrier* (the package format) from the *runtime path* (always stable, never the AppImage mount). AppImage updates work cleanly: new AppImage launches → detects version drift → re-extracts → restarts services.

**5c. Tracker lifecycle: systemd primary, XDG-autostart fallback.**

The two binaries run as **user-scope** services that start at login and stop at logout. We never use system-scope services — that requires root and contradicts Daylog's per-user model. We do **not** call `loginctl enable-linger`: tracking is tied to the active user session, matching Screen Time's semantics. A tracker that runs while you're logged out is wrong.

Detection (`src-tauri/src/tracking/lifecycle.rs`):
```rust
// systemd if /run/systemd/system exists; else XDG-autostart fallback.
fn supervisor() -> Supervisor {
    if std::path::Path::new("/run/systemd/system").exists() {
        Supervisor::Systemd
    } else {
        Supervisor::XdgAutostart
    }
}
```

**On systemd distros** (~98% of Linux desktops — Ubuntu/Debian/Fedora/Arch/openSUSE/Pop/Mint/Manjaro), Daylog renders these templates to `~/.config/systemd/user/`:

```ini
# daylog-aw-server.service.tmpl  ({BIN_DIR} interpolated at install time)
[Unit]
Description=Daylog activity tracking server
After=graphical-session.target

[Service]
ExecStart={BIN_DIR}/aw-server-rust --port 5600 --testing false
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
```

```ini
# daylog-awatcher.service.tmpl
[Unit]
Description=Daylog activity watcher
After=daylog-aw-server.service
Requires=daylog-aw-server.service

[Service]
ExecStart={BIN_DIR}/aw-awatcher
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
```

Install steps:
1. Resolve `{BIN_DIR}` per 5b.
2. Render templates to `~/.config/systemd/user/daylog-{aw-server,awatcher}.service`.
3. `systemctl --user daemon-reload`.
4. `systemctl --user enable --now daylog-aw-server daylog-awatcher`.
5. Wait until `127.0.0.1:5600/api/0/info` answers (max 15s; clear error path on timeout).

**On non-systemd distros** (Void, Alpine, Artix, Devuan), Daylog installs an XDG autostart entry and a small supervisor script:

```desktop
# ~/.config/autostart/daylog-tracker.desktop
[Desktop Entry]
Type=Application
Name=Daylog Tracker
Exec={BIN_DIR}/daylog-supervisor.sh
X-GNOME-Autostart-enabled=true
NoDisplay=true
```

```bash
# daylog-supervisor.sh.tmpl  (rendered into {BIN_DIR}/daylog-supervisor.sh)
#!/usr/bin/env bash
# Run aw-server + awatcher; restart either if it dies; exit on session end.
set -u
BIN_DIR="{BIN_DIR}"
trap 'kill 0' EXIT
while true; do "$BIN_DIR/aw-server-rust" --port 5600 --testing false; sleep 2; done &
while true; do "$BIN_DIR/aw-awatcher"; sleep 2; done &
wait
```

XDG autostart is honored by every Linux desktop environment (GNOME, KDE, XFCE, Cinnamon, MATE, Sway, Hyprland), so this fallback works everywhere systemd isn't.

Both paths reach the same end state: aw-server + awatcher running for the duration of the user's session. The Daylog UI doesn't know or care which is in use; it queries a Tauri command `tracking_status()` returning `{ supervisor: "systemd" | "xdg-autostart" | "external", state: "active" | "inactive" | "failed", since: ISO8601 }`.

**5d. GNOME-Wayland extension (optional, detected at first run).**

`aw-awatcher` handles X11 and wlroots-Wayland (Sway/Hyprland) and KDE-Wayland natively. **GNOME-Wayland is the only case** that needs `focused-window-dbus@flexagoon.com`. The bundled extension zip ships inside Daylog; no internet access required at first launch.

Detection: `XDG_CURRENT_DESKTOP=*GNOME*` **and** `XDG_SESSION_TYPE=wayland`. Otherwise skip everything in this section.

If matched:
1. Extract the bundled zip to `~/.local/share/gnome-shell/extensions/focused-window-dbus@flexagoon.com/` (user-level — no root).
2. `gnome-extensions enable focused-window-dbus@flexagoon.com`.
3. Show "Please log out and back in for tracking to start." (Wayland cannot live-reload extensions.)

If `gnome-extensions` is not on `PATH` (extension support disabled on the system), drop a "Tracking on this GNOME-Wayland session may be incomplete — see docs" banner and continue in degraded mode.

**5e. First-launch wizard (universal).**

A new React route mounted on first run (detect via missing settings file). Five steps, each a Tauri command:

1. `tracking_detect()` — probes `GET http://127.0.0.1:5600/api/0/info` (1s timeout). Returns `Existing { hostname, version }` on 200, `None` otherwise.
2. If `Existing`: skip install. Show "Using your existing ActivityWatch install" and proceed to dashboard.
3. If `None`: `tracking_install_bundled()`:
   - Resolve `{BIN_DIR}` per 5b; place binaries.
   - Pick supervisor per 5c (`systemd` vs `xdg-autostart`).
   - Render templates, install, start. Capture stderr; surface failures with the actual error and a "Copy" + "View logs" affordance (`journalctl --user -u daylog-aw-server` for systemd, `~/.local/share/daylog/supervisor.log` for XDG).
4. `tracking_wait_until_live(15s)` — polls until ready or timeout.
5. `tracking_setup_gnome_extension()` per 5d (no-op outside GNOME-Wayland).

Skipping or failing the wizard puts the dashboard into a degraded state with a banner; "Open Settings → Tracking" retries.

**5f. Update + uninstall hygiene.**

| Action | What happens |
|---|---|
| AppImage replaced with a newer version | On next launch, Daylog compares the embedded binary version against `~/.local/share/daylog/bin/.version`. If they differ, re-extracts binaries and runs `systemctl --user restart daylog-*` (or kills + relaunches the supervisor on the XDG path). |
| `.deb` upgraded via `apt` / `.rpm` upgraded via `dnf` | `postinst` runs `systemctl --user --machine=$USER@.host daemon-reload && systemctl --user --machine=$USER@.host restart daylog-*` for each logged-in user. |
| Settings → "Pause tracking" | `systemctl --user stop daylog-awatcher` (or kills awatcher in supervisor). aw-server stays running so historical queries still work. |
| Settings → "Stop background tracking" | Stops + disables both services / removes the autostart entry. Leaves binaries in place so a re-enable is a single click. |
| AppImage trashed | Services keep running until the next logout, then never start again (binaries still in `~/.local/share/daylog/bin/`). `daylog --uninstall-tracking` cleans up fully. |
| `apt remove daylog` / `dnf remove daylog` | Pre-remove hook runs `systemctl --user --machine=$USER@.host stop daylog-*` and `disable` for each logged-in user. |

We never delete `~/.local/share/activitywatch/` automatically — that's the user's tracking history, not ours to remove. Documented in README and shown as a toast on uninstall confirmation.

**5g. CI build matrix + container-based distro smoke tests.**

Two GitHub Actions workflows. Both target `ubuntu-22.04` for the build host (oldest libwebkit2gtk-4.1; binaries with glibc 2.35 run on every supported desktop distro).

`.github/workflows/ci.yml` (push to master + every PR): one job, ~10 min with caching. Runs `cargo check` + `cargo test`, `bunx tsc --noEmit`, `bun run build`, and a full release `tauri build`. The release build catches packaging regressions before merge so we never discover them on tag push.

`.github/workflows/release.yml` (push of `v*.*.*` tag; also `workflow_dispatch` for dry-runs). Three stages:

1. **Build** (one job on `ubuntu-22.04`) — produces `.AppImage`, `.deb`, `.rpm` (all x86_64). Caches `~/.cache/daylog/binaries/` keyed by `hashFiles('scripts/binaries.lock')` so unchanged upstream versions don't re-download.

2. **Smoke matrix** (9 parallel container jobs):

   | Tier | Jobs | Containers | Verifies |
   |---|---|---|---|
   | **Hard-fail** | `smoke-deb` × 3 | `ubuntu:22.04`, `ubuntu:24.04`, `debian:12` | `apt install ./*.deb` resolves deps cleanly + `daylog --help` runs |
   | **Hard-fail** | `smoke-rpm-fedora` | `fedora:41` | `dnf install ./*.rpm` + `daylog --help` |
   | **Hard-fail** | `smoke-rpm-opensuse` | `opensuse/tumbleweed` | `zypper install` + `daylog --help` |
   | **Hard-fail** | `smoke-appimage` × 5 | `ubuntu:24.04`, `debian:12`, `fedora:41`, `archlinux:latest`, `opensuse/tumbleweed` | `APPIMAGE_EXTRACT_AND_RUN=1 ./*.AppImage --help` |
   | **Informational** (`continue-on-error`) | `smoke-appimage-void` | `voidlinux/void-glibc-full` | Confirms AppImage runs on a non-systemd distro |
   | **Informational** (expected fail) | `smoke-appimage-alpine` | `alpine:latest` | Catches the day Alpine becomes glibc-compatible — emits `::warning::` if it ever passes |

3. **Release job** (gated on all hard-fail smoke jobs; only runs on tag refs) — downloads artifacts, creates GitHub Release via `softprops/action-gh-release@v2` with auto-generated notes.

**Distro coverage by inheritance** — explicit container coverage covers ~35+ derivatives without separate jobs:

| Tested in CI | Covers (via shared base) |
|---|---|
| `ubuntu:22.04` / `24.04` | Linux Mint, Pop!_OS, Zorin, elementary OS, KDE Neon, Kubuntu/Xubuntu/Lubuntu, Tuxedo OS, Deepin, Raspberry Pi OS |
| `debian:12` | Devuan, Kali, MX Linux |
| `fedora:41` | Rocky Linux, AlmaLinux, RHEL, CentOS Stream, Mageia, Nobara |
| `opensuse/tumbleweed` | openSUSE Leap |
| `archlinux:latest` | Manjaro, EndeavourOS, **Omarchy**, Garuda, ArcoLinux |
| `voidlinux` | Void, Artix |

What CI **cannot** test (deferred to manual VM smoke per Phase 6 exit criteria):
- The Tauri WebKit window opening — needs a display server.
- The wizard flow end-to-end.
- systemd user services actually starting (most containers strip systemd).
- `gnome-extensions install` running against real GNOME Shell.

aarch64 builds are deferred to v0.2 since both upstream binaries are x86_64-only today; adding aarch64 will require source builds in CI.

**Exit criteria:**
- AppImage launched on a fresh **Ubuntu 24.04**, **Fedora 41**, and **Arch (current)** VM: wizard succeeds, dashboard shows real data within 30s, tracking continues after closing the Daylog window, and is running again automatically after a logout/login cycle.
- Same test on a fresh **Void Linux** VM (non-systemd): XDG-autostart fallback engages, supervisor script keeps both binaries alive across induced kills, dashboard shows real data within 30s.
- `.deb` smoke-tested via `dpkg -i` on Ubuntu; `.rpm` via `dnf install` on Fedora — same end state as the AppImage path.
- AppImage replacement: install vN, log a few events, replace with vN+1, relaunch — old data still queryable, services running the new binaries, no manual steps.
- "Pause tracking" toggle in Settings stops awatcher within 1s; "Resume" restarts it; aw-server stays up either way.
- `apt remove daylog` / `dnf remove daylog` cleanly stop and disable services; `~/.local/share/activitywatch/` is preserved.

### Phase 6 — Release polish + manual VM smoke tests (1 weekend)

The CI/release pipeline itself shipped in Phase 5g; what's left is the polish that turns "the pipeline works" into "we can tag v0.1.0 with confidence":

- **README polish.** Animated GIF of the wizard + dashboard. Screenshot of the dashboard with the palette open mid-typing. "Daylog bundles ActivityWatch — no other install required" callout. "Existing AW user? Daylog detects and uses your install" note. "Tracking runs in the background like Screen Time — close Daylog anytime" note. The `Supported Linux distros` table is already in the README from 5g.
- **License attribution.** `THIRD-PARTY-NOTICES.md` crediting ActivityWatch (MPL-2.0) and awatcher (MPL-2.0). About dialog references it.
- **Manual VM smoke tests.** Container CI proves the artifact installs and the binary runs; it can't exercise the full UI flow because containers don't have a display server. Before tagging v0.1.0, smoke-test on real VMs (Distrobox or full VM):
  - **Ubuntu 24.04** — AppImage and `.deb` paths. Wizard → dashboard within 60s. Close window, wait 2 min, reopen — new events show up.
  - **Fedora 41** — AppImage and `.rpm` paths. Same flow.
  - **Arch (current)** — AppImage. Same flow.
  - **Void Linux** — AppImage. Confirms XDG-autostart fallback installs and the supervisor stays alive.
- **Tag v0.1.0** only after every VM smoke test passes. The release.yml workflow does the rest (matrix → GitHub Release).
- **Announce** on r/ActivityWatch, r/linux, HN Show.

**Exit criteria:** a stranger on any of the four test distros can download one AppImage, double-click it, and get a working dashboard with their data in under 60 seconds — and tracking keeps running after they close the window.

---

## 8. Test strategy

100% coverage for the AW client is the goal — it's the load-bearing surface. UI components get smoke + interaction tests; visual regression is deferred.

```
COVERAGE PLAN

[+] src-tauri/src/aw/mod.rs (Rust client)
  ├── parse_window_event           [★★★] fixture-driven, all field combos
  ├── parse_afk_event              [★★★] both statuses, edge timestamps
  ├── http_get_buckets             [★★ ] mocked reqwest, success path
  ├── http_get_buckets             [GAP] 404, timeout, malformed JSON  ← add
  └── http_post_query              [GAP] AQL error response             ← add

[+] src-tauri/src/aw/queries.rs
  ├── today_window_query string    [★★★] snapshot test
  └── today_afk_query string       [★★★] snapshot test

[+] src/lib/aw.ts
  └── thin invoke wrapper          [★  ] type-only, no logic — smoke test

[+] src/components/widgets/*
  ├── Timeline                     [★★ ] renders with fixture, tooltip works
  ├── TopApps                      [★★ ] renders with fixture, sort order correct
  ├── TopCategories                [★★ ] renders with fixture, rules applied
  └── CurrentFocus                 [★★ ] renders idle / active states

[+] src/components/widgets/Timeline (USER FLOW)
  ├── [GAP] [→E2E v0.2] hover scrubs across 24h band

CONNECTION-STATE STATES (USER FLOW)
  ├── [★★ ] amber on retry              ← unit test the hook
  ├── [GAP] [→E2E v0.2] red after 3 failed polls
  └── [GAP] [→E2E v0.2] recovers to green when server returns

COVERAGE: ~70% planned for v0.1. The 4 [GAP] items annotated [→E2E v0.2] are
deliberately deferred — they require a running aw-server and a Tauri WebDriver
setup that's not worth the time in v0.1.

CRITICAL TESTS (do not skip):
  - http_get_buckets timeout/error paths — silent failure here means dashboard
    shows zeros and user thinks they had no activity. Real production failure.
  - parse_*_event with malformed JSON — aw-server schema can drift; we want a
    loud error, not silent nulls.
```

---

## 9. Failure modes (each requires a plan)

| Failure | Likelihood | Plan |
|---|---|---|
| Bundled `daylog-aw-server.service` fails to start | Medium | Wizard captures stderr from `systemctl --user start`. Surface in UI with copy-button for the error and a "View systemd logs" button that opens `journalctl --user -u daylog-aw-server`. On the XDG fallback path, show `~/.local/share/daylog/supervisor.log` instead. |
| Port 5600 already in use by something other than AW | Low | Probe response: if `:5600` answers but `/api/0/info` returns non-AW JSON, treat as conflict. Wizard offers to use port 5601 for our bundled stack and stores the chosen port. |
| User has existing AW install with stale `aw-server` (older API) | Medium | On detect, log the version returned by `/api/0/info`. If `< 0.13.0`, show banner "Your ActivityWatch is older than Daylog expects; consider updating." Don't fail — try anyway. |
| GNOME Shell extension installed but disabled, user clicks Skip in wizard | Medium | Daylog runs in degraded state. Empty buckets after 30s → banner "No window data is being tracked. Open Settings → Tracking to enable the GNOME extension." |
| User on KDE / Sway / wlroots — no GNOME extension applicable | Low | Detect compositor via `XDG_CURRENT_DESKTOP` + `XDG_SESSION_TYPE`. Skip GNOME steps entirely. awatcher uses wlr-foreign-toplevel (Sway/Hyprland), KWin protocol (KDE), or X11 directly. |
| Non-systemd distro (Void, Alpine, Artix, Devuan) | Low | `/run/systemd/system` absent → wizard takes the XDG-autostart fallback per Phase 5c. Same end state, different supervisor. |
| User uninstalls Daylog but expects AW to keep running | Low | `.deb` / `.rpm` pre-remove hook disables our services for each logged-in user. AppImage users get `daylog --uninstall-tracking`. README documents the difference + toast in app on uninstall confirmation: "This will stop tracking. Your data is preserved at `~/.local/share/activitywatch/`." |
| AppImage extraction to `~/.local/share/daylog/bin/` fails (disk full / permissions) | Low | Wizard surfaces the actual `io::Error` with path. Retry button and "Open file manager at this path" affordance. Without the extraction, the unit files would point at the AppImage mount and break on next run, so we fail loud rather than fall back. |
| User manually deletes `~/.local/share/daylog/bin/` while services are running | Very Low | Services fail on next restart. Daylog detects missing binaries on next launch and re-extracts (idempotent path). Logged to telemetry-free local log. |
| Bundled binary version drifts from upstream (security fix not picked up) | Medium | `scripts/binaries.lock` pins versions. Renovate config auto-PRs upstream releases weekly with updated SHA-256. |
| `~/.local/share/activitywatch/` permissions broken (user `sudo`-ed something they shouldn't have) | Low | Server fails to start with permission error. Wizard surfaces the fix command: `sudo chown -R $USER ~/.local/share/activitywatch`. |
| aw-server returns 500 / malformed JSON during normal use | Low | Toast error with raw response; widget shows last-known-good data. |
| Categorization regex is invalid (user-edited) | Medium | Validate on save; refuse to persist invalid pattern. Show inline error. |
| WebKitGTK rendering bug (Tailwind 4 / ECharts) | Medium | Test in `bun run tauri dev` from day one. Don't trust browser-only testing. |
| Clock skew between local and aw-server timestamps | Low | aw-server uses local clock too; mostly safe. Display all times in the user's local TZ. |
| Tauri dev/build version mismatch | Low | Pin `@tauri-apps/cli` to exact version in `package.json`; pin `tauri` crate version in `Cargo.toml`. |

---

## 10. Project layout (after Phase 1)

```
daylog/
├── PLAN.md                       ← this file
├── README.md                     ← Phase 5
├── package.json
├── bun.lockb
├── biome.json
├── vite.config.ts
├── tailwind.config.ts
├── components.json               ← shadcn config
├── index.html
├── src/                          ← React frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   │   ├── ui/                   ← shadcn components live here (incl. Command)
│   │   ├── layout/
│   │   │   └── Topbar.tsx        ← thin; shows range, totals, ⌘K chip
│   │   ├── palette/
│   │   │   ├── CommandPalette.tsx
│   │   │   └── commands.ts       ← static + dynamic command registry
│   │   └── widgets/
│   │       ├── Timeline.tsx
│   │       ├── TopApps.tsx
│   │       ├── TopCategories.tsx
│   │       └── CurrentFocus.tsx
│   ├── pages/                    ← detail views reachable via palette
│   │   ├── Overview.tsx          ← default view
│   │   ├── AppsPage.tsx
│   │   ├── ActivityLogPage.tsx
│   │   └── HourlyPatternsPage.tsx
│   ├── context/
│   │   └── RangeContext.tsx      ← active time range, palette mutates this
│   ├── lib/
│   │   ├── aw.ts                 ← Tauri invoke wrappers
│   │   ├── aw-types.ts
│   │   ├── categories.ts         ← rule engine
│   │   └── utils.ts              ← shadcn cn() lives here
│   └── hooks/
│       ├── useAw.ts              ← TanStack Query hooks
│       └── useHotkey.ts          ← global ⌘K / Ctrl+K binding
├── src-tauri/                    ← Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── icons/
│   ├── binaries/                 ← bundled at build time, gitignored
│   │   ├── aw-server-rust-x86_64-unknown-linux-gnu
│   │   └── aw-awatcher-x86_64-unknown-linux-gnu
│   ├── services/                 ← templates rendered at install time
│   │   ├── daylog-aw-server.service.tmpl
│   │   ├── daylog-awatcher.service.tmpl
│   │   ├── daylog-supervisor.sh.tmpl       ← XDG-autostart fallback
│   │   └── daylog-tracker.desktop.tmpl     ← XDG-autostart fallback
│   ├── extensions/               ← bundled GNOME Shell extension (Wayland only)
│   │   └── focused-window-dbus@flexagoon.com.zip
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs                ← Tauri commands
│   │   ├── tracking/
│   │   │   ├── mod.rs            ← detect / install / status
│   │   │   ├── install.rs        ← {BIN_DIR} resolution + binary placement
│   │   │   ├── lifecycle.rs      ← supervisor dispatch (systemd vs XDG)
│   │   │   ├── systemd.rs        ← systemctl --user wrappers
│   │   │   ├── xdg_autostart.rs  ← non-systemd supervisor wrappers
│   │   │   └── gnome.rs          ← gnome-extensions wrappers
│   │   └── aw/
│   │       ├── mod.rs            ← HTTP client
│   │       ├── queries.rs        ← AQL templates
│   │       └── types.rs          ← Rust mirror of TS types
│   └── tests/
│       └── fixtures/
│           ├── window_events.json
│           ├── afk_events.json
│           └── query_response.json
├── scripts/
│   ├── fetch-binaries.sh         ← download AW binaries at build time (POSIX bash)
│   ├── bump-binary.sh            ← one-liner to bump a version + recompute sha256
│   ├── binaries.lock             ← pinned versions + sha256, all targets
│   ├── postinst.sh               ← .deb/.rpm post-install hook (try-restart per user)
│   └── prerm.sh                  ← .deb/.rpm pre-remove hook (stop services per user)
└── .github/
    ├── renovate.json             ← auto-PR upstream binary releases
    └── workflows/
        ├── ci.yml                ← Phase 5g (push/PR: cargo + tsc + tauri build)
        └── release.yml           ← Phase 5g (tag: build + 9-distro smoke matrix
                                                    + GitHub Release upload)
```

---

## 11. Open questions (decide before / during implementation)

1. **Polling vs event stream.** aw-server doesn't expose websockets. Polling at 5s is fine for v0.1, but if we later want sub-second focus tracking, we'd need to long-poll or watch sqlite directly. *Decision deferred.*
2. **Category storage location.** App-local store (current plan) or aw-server bucket? Storing in aw-server would make rules portable to the official WebUI. Cost: harder to test, more network calls. *Decision: app-local for v0.1, revisit.*
3. **Light mode default.** The aesthetic of dense data dashboards leans dark. shadcn default is system-follow. *Decision: follow GNOME for now; verify dark mode renders the heatmap acceptably.*
4. **Hostname assumption.** Bucket IDs include hostname (`aw-watcher-window_manas`). What if the user changes hostname or has multiple machines? *Decision for v0.1: query `/api/0/buckets/` at startup, pick the first `aw-watcher-window_*` and `aw-watcher-afk_*`. Document the assumption. Multi-host is a v0.2 problem.*
5. **Detail view: slide-over or route swap?** Slide-over keeps the dashboard visually present; route swap is simpler. Try slide-over first. If transitions feel like blinking, fall back to route swap. *Decide during Phase 3 Track B.*
6. **Cmd+K vs Ctrl+K on Linux.** Bind both. Cheap.
7. **Palette dynamic commands at scale.** What does the result list show when a user has 200+ tracked apps? *Decision for v0.1: top 20 by current-range duration; "Show all apps" expands inline. Not a v0.1 blocker.*

---

## 12. What this plan does NOT do (and you might think it should)

- **Does not vendor any ActivityWatch code.** Not the WebUI, not the server, not the watchers. The HTTP API on `localhost:5600` is the entire interface. No fork, no clone, no `git submodule`.
- **Does not promise cross-platform.** macOS/Windows are explicit non-goals for v0.1.
- **Does not include a tray icon or notifications.** Daylog the *window* is a foreground app — close it and it goes away. Daylog the *tracker*, however, is explicitly a background daemon (user-scope systemd, with XDG-autostart fallback on non-systemd distros) — it runs whenever you're logged in, and is what makes the Screen Time model possible.
- **Does not generate insights or summaries.** v0.1 is observational. No "you spent too much time on Reddit" — that's a different product.

---

## 13. Definition of done for v0.1

- [ ] `bun run tauri dev` starts cleanly on a fresh clone after Phase 0 prereqs.
- [ ] `scripts/fetch-binaries.sh` fetches and verifies aw-server-rust + aw-awatcher for `x86_64-unknown-linux-gnu` with pinned SHA-256 checksums; idempotent re-runs are no-ops.
- [ ] `scripts/bump-binary.sh aw-server-rust v0.13.3` produces a one-line `binaries.lock` diff with refreshed checksums.
- [ ] First-launch wizard succeeds end-to-end on a fresh Ubuntu 24.04, Fedora 41, and Arch VM with no prior AW: detect → place binaries at `{BIN_DIR}` → install systemd unit → enable extension (GNOME-Wayland only) → first event arrives.
- [ ] First-launch wizard succeeds end-to-end on a fresh Void Linux VM (non-systemd): XDG-autostart fallback engages, supervisor script keeps both binaries alive, first event arrives.
- [ ] First-launch wizard correctly detects an existing AW install on `:5600` and skips bundled-stack install.
- [ ] Tracking continues running after the Daylog window is closed; reopening Daylog shows stats covering the entire login session.
- [ ] After a logout/login cycle, services restart automatically; no manual intervention required.
- [ ] All five Overview widgets render real data from the running aw-server: `KpiStrip` (Active · Best Window · Longest stretch · Cadence · Pattern shift), `Timeline` (hero row, with AFK stripes + yesterday-ghost), `TopApps`, `TopCategories`, `NotableToday`.
- [ ] Each KPI card carries a "vs trailing-7-day median" sub-text (suppressed with `building baseline (N/7 days)` placeholder when <7 days of history).
- [ ] `NotableToday` surfaces 0–2 anomaly cards per day; empty state reads `No notable patterns today` (never claims false positives).
- [ ] `Overview.tsx` composes exactly those five widgets. `HourlyDistribution`, `WebPanel`, `ActivityLog`, `WeekStacked`, `MonthHeatmap` are not imported by `Overview.tsx` — they live on their own palette-reachable pages.
- [ ] `Week` and `Month` palette destinations render correctly: 7-day stacked bars by category root with weekday-typical overlay; calendar heatmap of daily active total.
- [ ] All five Overview widgets re-render correctly when `RangeContext` switches (Today / Yesterday / This week / This month).
- [ ] Click-to-filter wiring: clicking a donut segment, app row, or category badge opens the palette with the term pre-typed.
- [ ] AFK is visible everywhere: low-opacity stripes in Timeline, idle-gap count in Cadence, AFK subtracted from Active. Active KPI degrades to "—" with a tooltip when no AFK bucket exists.
- [ ] Mini-window: `daylog --mini` (and the `Mini` palette command) spawns a frameless 320×120 secondary window showing current focus + a one-line category bar. Always-on-top is best-effort per-DE; documented gap if a DE doesn't honor the hint.
- [ ] **No sidebar exists in the rendered DOM.** Single-page dashboard is the default view.
- [ ] **Command palette opens on `⌘K` and `Ctrl+K`** anywhere in the window. `Esc` dismisses. `?` opens shortcut help.
- [ ] **Range commands work:** `Today` / `Yesterday` / `This week` / `This month` switch the dashboard within one frame.
- [ ] **Detail commands work:** `Apps`, `Categories`, `Hourly patterns`, `Activity log` reach the right view via the palette only (no nav UI).
- [ ] **Dynamic commands work:** typing an app name shows it as a result with its current-range total; selecting it lands on a filtered detail view.
- [ ] Connection state indicator works (green / amber / red).
- [ ] Settings persist across restarts; "Tracking" panel shows correct service status and lets you switch modes.
- [ ] Category rules apply within one refresh tick.
- [ ] `apt remove daylog` and `dnf remove daylog` cleanly disable and stop both services for each logged-in user.
- [ ] `daylog --uninstall-tracking` (AppImage path) cleanly disables and stops both services.
- [ ] All Rust tests pass; all Vitest tests pass.
- [ ] `bun run tauri build` produces working `.AppImage`, `.deb`, and `.rpm` for `x86_64`.
- [ ] `.github/workflows/ci.yml` is green on master and on every PR (cargo check, tests, frontend build, full release `tauri build`).
- [ ] `.github/workflows/release.yml` produces all three artifacts on tag push and uploads them to a GitHub Release.
- [ ] All hard-fail smoke jobs in the release matrix pass: `smoke-deb` (ubuntu:22.04 / 24.04 / debian:12), `smoke-rpm-fedora` (fedora:41), `smoke-rpm-opensuse` (opensuse/tumbleweed), `smoke-appimage` (ubuntu:24.04 / debian:12 / fedora:41 / archlinux:latest / opensuse/tumbleweed).
- [ ] README: screenshot of the dashboard with the palette open mid-typing, AppImage install one-liner (with `.deb` / `.rpm` alternatives), GIF of wizard, "Daylog bundles ActivityWatch" callout, "Tracking runs in the background like Screen Time" note, "Press ⌘K" hint.
- [ ] Smoke-tested on clean Ubuntu 24.04, Fedora 41, Arch, and Void VMs: download AppImage → `chmod +x` → double-click → dashboard within 60s; `⌘K` → `yesterday` → reflects yesterday's data within 10s; close window, wait 2 minutes, reopen — new events show up.
- [ ] License attribution: ActivityWatch (MPL-2.0) and awatcher (MPL-2.0) credited in About dialog and `THIRD-PARTY-NOTICES.md`.

When all boxes are checked, tag `v0.1.0` and post to r/ActivityWatch, r/linux, and HN Show.

---

## 14. v0.2 roadmap (ambient surfaces + extensions)

The mini-window has been pulled into v0.1 (§1.0 addendum). What remains for v0.2:

1. **GNOME shell topbar applet.** Companion extension that shows current focus + today total in the top bar. Hover reveals a small popover with top 3 apps. Talks to `aw-server` directly over HTTP (same `:5600`). Distributed via extensions.gnome.org and bundled in the `.deb` similar to `focused-window-dbus`. Cross-DE work is its own engineering project; staying in v0.2.
2. **KDE / Sway / wlroots ambient surfaces.** The mini-window is the cross-DE fallback; per-DE widgets (KDE Plasmoid, Sway tray) are post-v0.1 polish.
3. **Per-rule `productive: boolean` flag** in category settings, with `work_roots`-aware Pattern Shift weighting (e.g., work-Slack contributes to Work, personal-Discord doesn't).
4. **Configurable productive allowlist UI** — currently `work_roots` is hardcoded to `["Work"]`; v0.2 exposes it in Settings → General.
5. **Browser activity (aw-watcher-web)** integration polish — first-class Web page already exists at v0.1; v0.2 adds a "you can install the browser extension" hint flow and a richer per-domain panel.
6. **aarch64 builds** — neither aw-server-rust nor aw-awatcher publishes aarch64 release artifacts; v0.2 builds them from source as part of the release pipeline.
