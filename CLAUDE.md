# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Daylog is a Linux-only Tauri 2 desktop app — a single-window dashboard for [ActivityWatch](https://activitywatch.net). The window is a viewer; the tracker is a separate background daemon Daylog installs on first launch. See `PLAN.md` for the long-form engineering plan and the "Phase N" labels referenced in commit messages.

## Tooling

`bun` is the package manager (lockfile is `bun.lock`, and `tauri.conf.json` shells out to `bun run …`). Don't introduce npm/pnpm/yarn.

## Common commands

| Task | Command |
|---|---|
| Install JS deps | `bun install --frozen-lockfile` |
| Fetch bundled upstream binaries (required before any `tauri dev`/`build`) | `scripts/fetch-binaries.sh` |
| Run desktop app (frontend + Rust) | `bun run tauri dev` |
| Frontend-only dev server (rarely useful alone — no Tauri IPC) | `bun run dev` |
| Frontend build | `bun run build` (= `tsc && vite build`) |
| Frontend type-check only | `bunx tsc --noEmit` |
| Rust check | `cd src-tauri && cargo check --all-targets` |
| Rust tests | `cd src-tauri && cargo test` |
| Single Rust test | `cd src-tauri && cargo test <name>` |
| Full release bundle (.AppImage, .deb, .rpm) | `bun run tauri build` |
| Bump a pinned upstream binary | `scripts/bump-binary.sh <component> <version>` |

There is no JS test runner and no JS linter wired in — CI runs `tsc --noEmit`, `cargo check`, `cargo test`, `vite build`, then a full `tauri build`. Match those locally before claiming a change is green.

## Architecture

### Three separations to internalise

1. **Window vs tracker.** The Tauri app is a viewer. The tracker (`daylog-aw-server.service` + `daylog-awatcher.service`, or the XDG-autostart supervisor on non-systemd distros) runs at the user-systemd level and survives window close. Code that touches services lives in `src-tauri/src/tracking/` — never reach into systemd from anywhere else.
2. **Carrier vs runtime path.** AppImage / `.deb` / `.rpm` are carriers. Unit files always reference a stable `{BIN_DIR}` resolved at runtime: `~/.local/share/daylog/bin/` (AppImage; Daylog extracts on first launch and on version drift) or `/usr/lib/daylog/bin/` (deb/rpm; placed by the package manager). Templates in `src-tauri/services/*.tmpl` use `{BIN_DIR}` as a placeholder; substitution happens in `tracking::render_template`.
3. **Our stack vs existing AW.** First-launch probe (`tracking_detect`) hits `:5600`. If something answers, we treat it as `Supervisor::External` and never install our services. Don't add code paths that race against an existing aw-server.

### Frontend → Rust → aw-server

The WebView never talks to `localhost:5600` directly. All HTTP goes through Rust. The contract:

- Rust commands are registered in `src-tauri/src/lib.rs` (the big `invoke_handler!` list). Names are snake_case.
- Typed JS wrappers live in `src/lib/aw.ts` (data) and `src/lib/tracking.ts` (lifecycle + wizard). Use these — don't call `invoke()` directly from components.
- Tauri 2 converts JS camelCase keys → Rust snake_case params automatically. Wrappers pass camelCase; Rust signatures are snake_case. Don't fight this.
- Time ranges flow as `TimeRange` (see `src-tauri/src/time.rs` and `src/lib/aw-types.ts`); they serialize identically on both sides.

Adding a new aw query usually means: write Rust command in `lib.rs` → register in `invoke_handler!` → add a typed wrapper in `src/lib/aw.ts` → add a `useQuery` hook in `src/hooks/useAw.ts` keyed by `rangeKey(range)`.

### Frontend layout

- `src/main.tsx` mounts `<RangeProvider>` (active time range, palette-driven) and `<PageProvider>` (active page + filter). There is **no router** for v0.1 — `PageContext.push(pageId, filter?)` is the navigation primitive, and Escape calls `back()` to return to Overview.
- Pages in `src/pages/`. The command palette in `src/components/palette/` is the primary navigation surface; the topbar is secondary.
- shadcn-style components in `src/components/ui/`. `components.json` pins `style: radix-mira`, base color `neutral`, icon library `hugeicons`. The MCP server registered in `.mcp.json` is shadcn — use it when adding shadcn components.
- TanStack Query is the data layer. Per-query refetch intervals live in `src/hooks/useAw.ts`; widgets do not poll on their own.
- Dark theme is force-locked in `App.tsx`. Don't add a theme toggle without first checking `PLAN.md` (it's a deliberate later concern).

### Rust crate layout (`src-tauri/src/`)

- `lib.rs` — Tauri commands + handler registration. The single source of truth for the IPC surface.
- `aw_client.rs` — HTTP client + `queries` module (string AQL queries the frontend never sees).
- `aggregate.rs` — server-side reducers (top apps, hourly buckets, AFK summary, categorized events).
- `categories.rs` — user-editable category rules, persisted to `<app_config_dir>/categories.json`. `Matcher::new` validates regexes; commands that mutate config must call it before saving.
- `time.rs` — `TimeRange` enum used across the IPC boundary.
- `tracking/` — everything that touches the user's machine state:
  - `install.rs` — placing binaries into `{BIN_DIR}` and version-stamping.
  - `lifecycle.rs` — supervisor abstraction (`Systemd` | `XdgAutostart` | `External`); `install_supervisor`, `status`, `pause`, `resume`, `stop`, `uninstall`. `pause` semantics differ per supervisor (documented in the source).
  - `systemd.rs`, `xdg_autostart.rs` — concrete supervisors. `detect()` picks one based on `/run/systemd/system`.
  - `gnome.rs` — install + enable the `focused-window-dbus@flexagoon.com` shell extension. Only relevant on GNOME-Wayland; `applicable: false` everywhere else.
- `main.rs` — handles CLI flags (`--help`, `--uninstall-tracking`) before delegating to `daylog_lib::run()`. The uninstall path uses `daylog_lib::uninstall_blocking` so AppImage users have an escape hatch without a running app.

### First launch

Wizard completion is gated by an empty marker file at `<app_config_dir>/.wizard-complete` (`WIZARD_MARKER` in `lib.rs`). `useFirstLaunch` reads it; `Wizard.tsx` writes it via `wizard_complete_set`. Resetting first launch = delete that file.

## Bundled binaries

`src-tauri/binaries/aw-server-rust`, `src-tauri/binaries/aw-awatcher`, and `src-tauri/extensions/focused-window-dbus@flexagoon.com.zip` are **not committed manually** — they're produced by `scripts/fetch-binaries.sh` from the pins in `scripts/binaries.lock` (tab-separated: `component	version	target	sha256`). Both CI and local builds run this script.

To upgrade an upstream component, run `scripts/bump-binary.sh <component> <version>` — it downloads, hashes, and rewrites the lockfile. Never edit `binaries.lock` by hand. Renovate manages `aw-server-rust` and `aw-awatcher`; the GNOME extension version is a `pk` download tag and must be bumped manually.

`v0.1` is x86_64-only — neither upstream publishes aarch64 artifacts.

## CI / release

- `.github/workflows/ci.yml` runs on every push/PR: cargo check + test, `tsc --noEmit`, vite build, and a full `tauri build` (slow but catches packaging regressions). All on `ubuntu-22.04` so the artifact glibc baseline stays at 2.35.
- `.github/workflows/release.yml` triggers on `v*.*.*` tags. It builds artifacts once and smoke-tests them in containers across the deb family (Ubuntu/Debian), rpm family (Fedora/openSUSE), and AppImage across deb/rpm/Arch/openSUSE — plus informational runs on Void (non-systemd) and Alpine (musl, expected to fail). The `release` job that publishes the GitHub Release only runs on actual tag pushes.
- The smoke matrix runs `daylog --help` and `daylog --uninstall-tracking` against the installed binary. If you add a CLI flag in `main.rs`, extend the smoke commands.

## Conventions worth knowing

- Comments lean toward *why*, not *what* — see existing modules (`tracking/lifecycle.rs`, `scripts/fetch-binaries.sh`) for the established voice. Short and load-bearing; no tutorial blocks.
- Errors are typed (`thiserror`) and serialize via `impl serde::Serialize` writing `to_string()`. Frontend receives a string. Match this when adding new error enums.
- `#[allow(dead_code)]` is used deliberately in a few spots (e.g. `Supervisor::External`) — don't strip without checking the comment above it.
