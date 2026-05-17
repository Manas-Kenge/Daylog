# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Daylog is a Linux-only terminal screen-time tracker. Single binary, single purpose: a ratatui dashboard that shows how you spent your time on your computer, broken down by app, category, hour-of-day, and web domain. The tracker (aw-server-rust + aw-awatcher) is downloaded and supervised by daylog itself on first launch — there is no separate desktop app, no GUI, and no system packages.

The previous Tauri desktop app is preserved on the `archive/desktop` branch and no longer ships from `master`. See `PLAN.md`'s 2026-05-10 addendum for the pivot rationale.

## Tooling

`cargo` is the only build tool. There is no `bun`, `node`, or `npm` here anymore. Don't reintroduce them.

## Common commands

| Task | Command |
|---|---|
| `cargo check` workspace | `cargo check --workspace` |
| Run tests | `cargo test --workspace` |
| Run a single test | `cargo test -p daylog-tui <name>` |
| Build the binary | `cargo build --release -p daylog-tui` |
| Run from source | `cargo run -p daylog-tui` |
| Install locally for dogfood | `cargo install --path crates/daylog --locked` |
| Dry-run a publish | `cargo publish --dry-run -p daylog-tui` |

CI runs `cargo check --workspace`, `cargo test --workspace`, `cargo build --release -p daylog-tui`, and a `daylog --help / --version` smoke. Match those locally before claiming a change is green.

## Architecture

### One crate, library + binary

`crates/daylog` is the only crate. It publishes to crates.io as **`daylog-tui`** (the bare `daylog` is taken by an unrelated project) and the executable it produces is named `daylog`. Both invariants live in `crates/daylog/Cargo.toml`'s `[package] name` and `[[bin]] name`.

The crate has both a `src/lib.rs` (library) and `src/main.rs` (binary entry) — the same ripgrep/fd/bat/tokei/hyperfine layout that's standard for Rust "library + CLI" projects. Internally:

- `src/data/` — the pure-Rust data layer. Reads aw-server-rust's SQLite file directly via `data/datastore.rs` + `data/transforms.rs` (ports of upstream's `aw-transform/` crate). `data/aw_client.rs` is HTTP-only for metadata (server info, category settings). Plus aggregations, KPI math, category rules + matcher, `TimeRange` enum, the `--json today` snapshot builder.
- `src/cache.rs` — `Cached<T>` wrappers + `dispatch_refetches` for live polling. Sits on top of `data/`.
- `src/tracking/` — the first-launch tracker installer (see below).
- `src/ui/` + `src/wizard.rs` + `src/app.rs` — the ratatui surface.

This was two crates (`daylog-core` + `daylog-tui`) through v0.1.x; they were collapsed at v0.2.0 after the Tauri desktop consumer was archived. The path-dep version stamp that the split required is now gone.

### Tracker bootstrap

Lives in `crates/daylog/src/tracking/`. On first launch the wizard probes `:5600`. If aw-server-rust is up (verified by `datastore::db_path().exists()`), it skips. If a *different* aw-server is running — most commonly the older aw-server (Python) from a pre-Rust ActivityWatch desktop install — the wizard renders a "wrong tracker" warning explaining how to migrate: daylog only reads aw-server-rust's SQLite schema. Otherwise it downloads the pinned upstream binaries (aw-server-rust + aw-awatcher) into `~/.cache/daylog/binaries/`, sha256-verifies, extracts to `~/.local/share/daylog/bin/`, then writes either systemd-user units or an XDG-autostart supervisor depending on what `lifecycle::detect()` finds, and starts both. On GNOME-Wayland it also offers to install the upstream `focused-window-dbus` shell extension.

Why download instead of bundle? Embedding the ~44 MB of upstream binaries via `include_bytes!` blew past crates.io's 10 MB tarball limit. The download path keeps the published crate small.

Module breakdown:

- `tracking/pins.rs` — pinned URLs + sha256 sums for upstream artifacts. Hand-maintained; bump the version + sha together.
- `tracking/download.rs` — reqwest streaming download, sha256 verify, zip extraction. Cache layout: `~/.cache/daylog/binaries/<sha-prefix>-<name>.zip`.
- `tracking/install.rs` — `place_binaries()` (async). Idempotent — re-extracts only when the daylog version stamp changes.
- `tracking/lifecycle.rs` — supervisor abstraction (`Systemd` | `XdgAutostart` | `External`). `install_supervisor`, `status`, `pause`, `resume`, `stop`, `uninstall`, `wait_until_live`. `pause` semantics differ per supervisor (documented in source).
- `tracking/systemd.rs`, `tracking/xdg_autostart.rs` — concrete supervisors. `detect()` picks one based on `/run/systemd/system`.
- `tracking/gnome.rs` — install + enable the `focused-window-dbus@flexagoon.com` extension. `applicable: false` outside GNOME-Wayland.
- `tracking/mod.rs` — `config_dir()` (via the `dirs` crate, no Tauri), service templates embedded via `include_str!`, `render_template()` does the `{BIN_DIR}` substitution.

Service templates live at `crates/daylog/services/*.tmpl` and are compiled into the binary via `include_str!` in `tracking/mod.rs`. They're tiny so embedding is fine.

### TUI

`crates/daylog/src/`:

- `lib.rs` — CLI entrypoint. Parses flags (`--setup`, `--uninstall-tracking`, `--json today`, `--help`, `--version`); without flags, runs the wizard (if needed) then drops into the dashboard.
- `main.rs` — 4-line bin entry calling `daylog_tui::run`.
- `wizard.rs` — first-launch ratatui flow. One Y/N/Q prompt, then progress lines while the install runs.
- `app.rs` — application state + main event loop (tab cycle, range chip, refetch dispatch).
- `cache.rs` — `Cached<T>` wrappers + `dispatch_refetches` for live polling.
- `data/{aggregate,aw_client,categories,datastore,kpi,paths,queries,snapshot,time,transforms}.rs` — data layer. `data/mod.rs` re-exports everything at the `data::` namespace so call sites write `use crate::data::{TimeRange, AwClient, ...}`.
- `theme.rs` — single source for every color and style modifier. No widget reaches into `ratatui::style::Color::*` directly.
- `ui.rs` + `ui/{overview, week, month, timeline, sparkline, stacked_bars, kpi_strip}.rs` — render tree. Each tab gets its own module.

Before changing anything visual in the TUI (layout, color, typography, borders, spacing, glyphs), read [`crates/daylog/DESIGN.md`](./crates/daylog/DESIGN.md). It is the source of truth for the design system — the 4-band rhythm shared across tabs, the borderless data-region rule, the eighth-block bar ladder, color token scoping, and the keyboard-driven tab/chip vocabulary. Don't deviate without updating DESIGN.md in the same change.

### First launch

The wizard-complete marker is `~/.config/daylog/.wizard-complete` (constant in `wizard.rs`). The wizard writes it after the user confirms install OR explicit decline. To re-prompt: delete the marker, or run `daylog --setup`.

## CI / release

- `.github/workflows/ci.yml` — every push/PR. cargo check + test + release build + a `daylog --help / --version` smoke. Runs in ~3 min.
- `.github/workflows/release.yml` — `v*.*.*` tag push. Builds the Linux x86_64 tarball, publishes `daylog-tui` to crates.io (gated by `CARGO_REGISTRY_TOKEN`), then cuts the GitHub Release attaching the tarball + checksums. The release job depends on the publish job, so a crates.io failure aborts the GitHub Release.

If you add a CLI flag in `lib.rs`, extend the smoke step in `ci.yml`.

## License

Daylog's own source is MIT-licensed (see [`LICENSE`](./LICENSE)). The `aw-server-rust` and `aw-awatcher` upstream binaries that daylog downloads on first launch stay under MPL-2.0; full attribution lives in [`THIRD-PARTY-NOTICES.md`](./THIRD-PARTY-NOTICES.md).

## Conventions

- Comments lean toward *why*, not *what*. Short and load-bearing; no tutorial blocks. See `tracking/lifecycle.rs` and `tracking/install.rs` for the established voice.
- Errors are typed (`thiserror`). The Tauri-IPC `serde::Serialize` impls have been removed since there's no IPC layer anymore.
- `#[allow(dead_code)]` is used deliberately in a few spots (e.g. `Supervisor::External`) — don't strip without reading the comment above it.
- When bumping a pinned upstream binary: update `crates/daylog/src/tracking/pins.rs` (URL + sha256) and `THIRD-PARTY-NOTICES.md` (pinned version line) in the same commit.
