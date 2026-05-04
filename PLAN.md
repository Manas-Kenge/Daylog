# Pulse — Engineering Plan

A native Linux desktop dashboard for ActivityWatch.

> **Working name.** Rename freely; nothing in this plan depends on it.

---

## 1. Vision

A single-window native desktop app that shows a beautiful, dense, real-time pulse of your day, sourced from the local ActivityWatch server you already have running. No browser tab. No sign-in. No cloud. The whole thing fits in one window with poster-quality information density.

**v0.1 hero scenario:** double-click the Pulse icon → a window opens showing today's timeline as a horizontal heatmap, top apps + categories with sparklines, and a live focus-session timer. The screenshot of that window is what the project ships on.

**Constraints locked in office-hours:**
- Linux-first, single-user, local-only.
- aw-server (HTTP daemon, port 5600) and aw-awatcher (already installed at `/usr/bin/aw-awatcher`) are external dependencies — we don't fork or modify them.
- We own only the UI and the local API client.
- Visual density beats feature density. Distribution (.deb / .AppImage / GitHub Release) is a Phase-1 concern, not a Phase-N afterthought.

---

## 2. Architecture

```
┌────────────────────────────────────────────────────────────┐
│  Pulse (this project) — single Tauri app                   │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  WebView (WebKitGTK)                                 │  │
│  │  React 19 + Vite + Tailwind 4 + shadcn/ui            │  │
│  │  TanStack Query + ECharts + date-fns                 │  │
│  │  Renders dashboard in our own native window          │  │
│  └────────────┬─────────────────────────────────────────┘  │
│               │ Tauri IPC (invoke + events)                │
│  ┌────────────┴─────────────────────────────────────────┐  │
│  │  Rust core (src-tauri)                               │  │
│  │  - HTTP client (reqwest) → aw-server :5600           │  │
│  │  - Polling loop, emits events to WebView             │  │
│  │  - Settings persistence (tauri-plugin-store)         │  │
│  └──────────────────┬───────────────────────────────────┘  │
└─────────────────────┼──────────────────────────────────────┘
                      │ HTTP/JSON
                      ▼
        ┌──────────────────────────┐
        │  aw-server (background)  │  ← already running, we don't ship it (yet)
        │  http://localhost:5600   │
        │  ~/.local/share/...db    │
        └──────────▲───────────────┘
                   │ events
        ┌──────────┴───────────────┐
        │  aw-awatcher (background)│  ← already installed, we don't ship it (yet)
        │  + GNOME extension       │
        └──────────────────────────┘
```

**Why HTTP through Rust core, not directly from the WebView?** Three reasons:
1. CORS — aw-server may not allow webview origins. Rust-side calls are origin-free.
2. Polling logic, retries, and reconnection live in one stable place.
3. Future sidecar work (Phase 5) needs Rust to spawn the binaries anyway.

The frontend never sees `localhost:5600`. It only knows about Tauri commands like `get_today_events()` and event channels like `pulse:bucket-updated`.

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

The dashboard window contains exactly these widgets, in this order:

```
┌────────────────────────────────────────────────────────────────┐
│ [Pulse]                              today · 14:23 · 5h 18m ▾  │
├────────────────────────────────────────────────────────────────┤
│  Today's timeline                                              │
│  [▓▓░░▓▓▓▓░░▓▓▓▓▓▓░░░░▓▓▓▓ ... ]  <- horizontal heatmap, 24h   │
│  hover: app + title at that instant                            │
├────────────────────────────────────────────────────────────────┤
│  Top apps              │  Top categories     │  Current focus  │
│  ────────────────      │  ────────────────   │  ─────────────  │
│  kitty       2h 14m ▁▃▅│  dev work    3h 02m │  ◐ 47 min       │
│  firefox     1h 03m ▂▄▆│  comms       0h 41m │  ↳ kitty        │
│  Code        0h 42m ▁▂▃│  reading     0h 33m │     "PLAN.md"   │
│  Slack       0h 12m ▁▁▂│  uncategor.  1h 02m │                 │
│  ...                   │                     │                 │
└────────────────────────────────────────────────────────────────┘
```

**Functional requirements:**
- Auto-refresh every 5 seconds.
- Connection-state indicator (top right): green = aw-server reachable, amber = retrying, red = down.
- Window remembers its size and position between launches.
- Light/dark mode follows GNOME accent color (or manual override in settings).
- One settings panel: timezone, refresh interval, category rules (initial UI: list editor).

**Non-functional:**
- Cold start to first paint: ≤500ms after window creation.
- Idle CPU: <1% (the WebView is a chunk of memory but not CPU when nothing's animating).
- Binary size: <15MB compressed `.deb`, <25MB `.AppImage`.

---

## 6. NOT in v0.1 scope (deferred, with rationale)

| Item | Reason |
|---|---|
| Cross-platform builds (macOS/Windows) | Linux-first per office-hours premise P2. Multi-OS CI doubles the build matrix. Re-evaluate after v0.1 stars >100. |
| Tray icon / menubar widget | Linux tray support varies wildly across DEs (XEmbed vs SNI vs AppIndicator). Worth its own design pass. |
| Notifications / focus alerts | Outside the "passive observer" identity of v0.1. |
| Bundled `aw-server-rust` sidecar | First-launch UX work. v0.1 assumes user already has AW running (they do — we set it up). |
| Multi-day / weekly / monthly views | v0.2. Today first; hard to get density right even for one day. |
| Categorization rule editor (visual) | v0.1 ships with a JSON-edit list. Visual rule builder is its own feature. |
| AW bucket creation / event editing | We are read-only against aw-server in v0.1. Period. |
| GNOME Shell extension companion | Lateral path discussed in office-hours; revisit in v0.2 once the dashboard is done. |
| Cloud sync / multi-device | Violates premise P2 (local-only). |
| Goals / limits / "you've used Twitter for 2h" | Behavioral nudging is a separate product. Pulse v0.1 is observational. |
| Browser activity (aw-watcher-web) | Requires user to install Firefox/Chrome extension separately. Add a "you can also install this" hint in settings, but don't ship the extension. |

---

## 7. Implementation phases

Each phase is sized to fit one weekend (your time, not CC time). Phases are sequential — no parallel lanes for v0.1.

### Phase 0 — System prerequisites (1 evening)

You run these. They need sudo and one logout:

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
# Create the Tauri app inside the existing pulse/ directory.
# create-tauri-app is interactive; pick:
#   - app name: pulse
#   - identifier: com.pulse.app  (or your domain)
#   - frontend: TypeScript / JavaScript
#   - flavor: React
#   - package manager: bun
bun create tauri-app

# Then add Tailwind 4 + shadcn:
cd pulse
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

### Phase 3 — Dashboard widgets (2-3 weekends)

This is the design-heavy phase. Build the four widgets in the v0.1 mockup:

1. **Today's timeline (horizontal heatmap)** — hardest. Build it first, alone, until it's beautiful. ECharts `heatmap` series with custom tooltip. Color encodes category, height encodes nothing (it's a bar). Hover scrubs to that instant.
2. **Top apps** with sparklines — `bar` + `line` series.
3. **Top categories** — bars with category colors.
4. **Current focus** — text + a circular progress indicator (custom SVG, not ECharts).

Polling: a single `useQuery` per widget, all using the same query key prefix, refetchInterval 5s. TanStack Query dedupes fan-out.

**Exit criteria:** all four widgets render real data, refresh on a 5s tick, tolerate aw-server going down (show amber state, don't crash).

### Phase 4 — Settings + categorization (1 weekend)

- Settings dialog (shadcn `Dialog`) with: refresh interval, theme, category rules.
- Category rules: list of `{ pattern: regex, category: string, color: hex }`. Persisted via `tauri-plugin-store`.
- Apply rules client-side when displaying — don't push back to aw-server.

**Exit criteria:** rules persist across app restarts; categorization in widgets reflects rules within one refresh tick.

### Phase 5 — Distribution (1 weekend)

The plan that takes most v0.1 attempts to a graveyard repo if skipped.

- `tauri.conf.json`: targets `deb`, `appimage`. Set `productName`, `version`, `identifier`, icons (use https://tauri.app/v2/guide/features/icons/).
- GitHub Actions workflow `.github/workflows/release.yml`:
  - Trigger: push of `v*.*.*` tag.
  - Matrix: ubuntu-22.04 (LTS, broadest libwebkit2gtk-4.1 compat).
  - Steps: install deps (same list as Phase 0), `bun install`, `bun run tauri build`, upload `*.deb` and `*.AppImage` to a GH Release.
- README.md: install one-liner (curl | sudo dpkg -i), screenshot of the dashboard, GIF of polling, one-paragraph explanation of "you must have ActivityWatch already installed".
- Tag `v0.1.0`, watch the workflow, smoke-test the artifacts on a clean Ubuntu VM (or a Distrobox container) before announcing.

**Exit criteria:** a fresh Ubuntu user can `wget` your `.deb`, `sudo dpkg -i` it, double-click the icon, and see the dashboard with their data.

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
| aw-server not running | High (user reboots, service didn't start) | Connection indicator goes red after 3 failed polls. Empty state with "Is ActivityWatch running? `systemctl --user status aw-server`" hint. |
| aw-server returns 500 / malformed JSON | Low | Toast error with raw response; widget shows last-known-good data. |
| Categorization regex is invalid (user-edited) | Medium | Validate on save; refuse to persist invalid pattern. Show inline error. |
| WebKitGTK rendering bug (Tailwind 4 / ECharts) | Medium | Test in `bun run tauri dev` from day one. Don't trust browser-only testing. |
| Clock skew between local and aw-server timestamps | Low | aw-server uses local clock too; mostly safe. Display all times in the user's local TZ. |
| Tauri dev/build version mismatch | Low | Pin `@tauri-apps/cli` to exact version in `package.json`; pin `tauri` crate version in `Cargo.toml`. |
| User doesn't have aw-awatcher running, only aw-server | Medium | Detect empty buckets list and show explicit setup hint linking to AW docs. |

---

## 10. Project layout (after Phase 1)

```
pulse/
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
│   │   ├── ui/                   ← shadcn components live here
│   │   └── widgets/
│   │       ├── Timeline.tsx
│   │       ├── TopApps.tsx
│   │       ├── TopCategories.tsx
│   │       └── CurrentFocus.tsx
│   ├── lib/
│   │   ├── aw.ts                 ← Tauri invoke wrappers
│   │   ├── aw-types.ts
│   │   ├── categories.ts         ← rule engine
│   │   └── utils.ts              ← shadcn cn() lives here
│   └── hooks/
│       └── useAw.ts              ← TanStack Query hooks
├── src-tauri/                    ← Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── icons/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs                ← Tauri commands
│   │   └── aw/
│   │       ├── mod.rs            ← HTTP client
│   │       ├── queries.rs        ← AQL templates
│   │       └── types.rs          ← Rust mirror of TS types
│   └── tests/
│       └── fixtures/
│           ├── window_events.json
│           ├── afk_events.json
│           └── query_response.json
└── .github/
    └── workflows/
        └── release.yml           ← Phase 5
```

---

## 11. Open questions (decide before / during implementation)

1. **Polling vs event stream.** aw-server doesn't expose websockets. Polling at 5s is fine for v0.1, but if we later want sub-second focus tracking, we'd need to long-poll or watch sqlite directly. *Decision deferred.*
2. **Category storage location.** App-local store (current plan) or aw-server bucket? Storing in aw-server would make rules portable to the official WebUI. Cost: harder to test, more network calls. *Decision: app-local for v0.1, revisit.*
3. **Light mode default.** The aesthetic of dense data dashboards leans dark. shadcn default is system-follow. *Decision: follow GNOME for now; verify dark mode renders the heatmap acceptably.*
4. **Hostname assumption.** Bucket IDs include hostname (`aw-watcher-window_manas`). What if the user changes hostname or has multiple machines? *Decision for v0.1: query `/api/0/buckets/` at startup, pick the first `aw-watcher-window_*` and `aw-watcher-afk_*`. Document the assumption. Multi-host is a v0.2 problem.*

---

## 12. What this plan does NOT do (and you might think it should)

- **Does not vendor any ActivityWatch code.** Not the WebUI, not the server, not the watchers. The HTTP API on `localhost:5600` is the entire interface. No fork, no clone, no `git submodule`.
- **Does not promise cross-platform.** macOS/Windows are explicit non-goals for v0.1.
- **Does not include a tray icon, notifications, or background daemon.** Pulse is a foreground app. Close the window, it goes away. aw-server keeps tracking regardless.
- **Does not generate insights or summaries.** v0.1 is observational. No "you spent too much time on Reddit" — that's a different product.

---

## 13. Definition of done for v0.1

- [ ] `bun run tauri dev` starts cleanly on a fresh clone after Phase 0 prereqs.
- [ ] All four widgets render real data from a running aw-server.
- [ ] Connection state indicator works (green / amber / red).
- [ ] Settings persist across restarts.
- [ ] Category rules apply within one refresh tick.
- [ ] All Rust tests pass; all Vitest tests pass.
- [ ] `bun run tauri build` produces working `.deb` and `.AppImage`.
- [ ] GitHub Actions release workflow produces both artifacts on tag push.
- [ ] README has a screenshot, install one-liner, and "you need ActivityWatch" note.
- [ ] Smoke-tested on a clean Ubuntu 24.04 install (VM or Distrobox).

When all 10 boxes are checked, tag `v0.1.0` and post to r/ActivityWatch + HN Show.
