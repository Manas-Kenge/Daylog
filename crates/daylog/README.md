# daylog

Terminal screen-time and activity tracker for Linux.

```
cargo install daylog-tui --locked
daylog
```

Daylog runs as a single TUI. On first launch it sets up a local activity tracker for you (one Y/N prompt) and then drops into a dashboard with per-app, per-category, per-domain, and per-hour breakdowns of how you spent your time.

## Features

- 24h timeline of every app + window you focused
- Top apps, top categories, top web domains
- KPI strip: today's active time, longest stretch, best window, pattern shifts vs. typical day-of-week
- 7-day, weekly, and monthly rollups
- Local-only — no cloud, no sign-in, no telemetry

## Quick start

```
cargo install daylog-tui --locked
daylog
```

On first launch, daylog will prompt to install a local tracker (~30 MB, all userspace, no sudo). Decline if you already run ActivityWatch — daylog will use whatever's on `:5600`.

## Other commands

```
daylog --setup               # Re-run the tracker installer
daylog --uninstall-tracking  # Remove the tracker (keeps your recorded data)
daylog --json today          # Print today's KPIs as JSON (for status bars)
daylog --help                # Full usage
daylog --version             # Print version
```

The top-level [README](https://github.com/Manas-Kenge/Daylog#status-bar-integration) has recipes for piping `--json today` into Quickshell, waybar, and i3blocks.

## Configuration

Custom category rules live at `~/.config/daylog/categories.json`. Daylog ships with sensible defaults — edit only if you want different buckets.

## Requirements

- Linux x86_64
- A terminal with truecolor support (recommended)

## License

MIT — see [LICENSE](https://github.com/Manas-Kenge/Daylog/blob/master/LICENSE).

Daylog bundles `aw-server-rust` and `aw-awatcher` from the [ActivityWatch](https://activitywatch.net) project (MPL-2.0). See [THIRD-PARTY-NOTICES.md](https://github.com/Manas-Kenge/Daylog/blob/master/THIRD-PARTY-NOTICES.md) for full attribution.
