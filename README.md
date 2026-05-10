# Daylog

Terminal screen-time tracker for Linux.

<!-- TODO: asciinema cast -->

```bash
cargo install daylog-tui
daylog
```

Daylog runs entirely on your machine. No cloud, no sign-in, no telemetry.

## Features

- 24-hour timeline of every app + window you focused
- Top apps, top categories, and top web domains for today, this week, and this month
- KPI strip: today's active time, longest stretch, best window, pattern shift vs. typical day-of-week
- 7-day, weekly, and monthly rollups
- Category rules you can edit (`~/.config/daylog/categories.json`)

## Install

The binary is published to crates.io as `daylog-tui`; the executable it installs is named `daylog`:

```bash
cargo install daylog-tui
```

Or grab a prebuilt tarball from [Releases](https://github.com/Manas-Kenge/Daylog/releases):

```bash
curl -L https://github.com/Manas-Kenge/Daylog/releases/latest/download/daylog-x86_64-unknown-linux-gnu.tar.gz | tar -xz
./daylog
```

On first launch, daylog detects whether a local activity tracker is running. If not, it offers to install one (a single Y/N prompt). The installer is fully userspace — no sudo, no system packages.

## Quick start

```bash
daylog                       # open the dashboard (prompts for tracker install on first run)
daylog --setup               # re-run the tracker installer
daylog --uninstall-tracking  # stop and remove the bundled tracker (keeps your data)
daylog --help                # full usage
daylog --version             # print version
```

## Configuration

Custom category rules live at `~/.config/daylog/categories.json`. Daylog ships with sensible defaults — edit only if you want different buckets.

## Compatibility

x86_64 Linux. Tested on Ubuntu, Debian, Fedora, Arch, openSUSE, and derivatives. The tracker uses systemd-user units when available and falls back to XDG-autostart on non-systemd distros (Void, Artix, Devuan).

aarch64 and non-Linux platforms are not supported in v0.1.

## Build from source

```bash
git clone https://github.com/Manas-Kenge/Daylog
cd Daylog
cargo build --release -p daylog-tui
./target/release/daylog
```

The two crates in this workspace are:

- `daylog-core` — pure-Rust data layer (HTTP client, queries, aggregations, KPI math).
- `daylog-tui` (binary `daylog`) — the ratatui dashboard plus the first-launch tracker installer.

## License

MIT — see [LICENSE](./LICENSE).

Daylog bundles `aw-server-rust` and `aw-awatcher` from the [ActivityWatch](https://activitywatch.net) project (MPL-2.0). Full attribution lives in [THIRD-PARTY-NOTICES.md](./THIRD-PARTY-NOTICES.md).
