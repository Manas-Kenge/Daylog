# Daylog

Terminal screen-time tracker for Linux.

<!-- TODO: asciinema cast -->

```bash
cargo install daylog-tui --locked
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

### Prebuilt binary (no Rust toolchain needed)

```bash
curl -L https://github.com/Manas-Kenge/Daylog/releases/latest/download/daylog-x86_64-unknown-linux-gnu.tar.gz | tar -xz
./daylog
```

Move it onto `$PATH` (e.g. `~/.local/bin/daylog`) to keep it around.

### Via Cargo

Published to crates.io as `daylog-tui`; the executable is `daylog`:

```bash
cargo install daylog-tui --locked
```

Requires a Rust toolchain (`rustup default stable`) and a C toolchain — `rusqlite` builds SQLite from source. On Debian/Ubuntu: `apt install build-essential`. On Arch/Omarchy: `pacman -S base-devel`. On Fedora: `dnf install gcc`.

On first launch, daylog detects whether a local activity tracker is running. If not, it offers to install one (a single Y/N prompt). The installer is fully userspace — no sudo, no system packages.

## Quick start

```bash
daylog                       # open the dashboard (prompts for tracker install on first run)
daylog --setup               # re-run the tracker installer
daylog --uninstall-tracking  # stop and remove the bundled tracker (keeps your data)
daylog --help                # full usage
daylog --version             # print version
```

## Status bar integration

`daylog --json today` prints today's KPIs as a single JSON object to stdout. Safe to poll on a short interval; exits 0 even when there's no data yet.

```json
{
  "as_of": "2026-05-17T11:45:00+05:30",
  "today": {
    "total_active": "PT4H32M",
    "top_app":      { "name": "kitty", "duration": "PT2H10M" },
    "top_category": { "name": "Work > Programming", "duration": "PT3H5M" },
    "hours":        [0, 0, 0, 0, 0, 0, 0, 0, 12, 47, 60, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  }
}
```

`as_of` is RFC 3339 with local timezone. `total_active` is the sum of non-AFK time today, as an ISO-8601 duration. `top_app` and `top_category` are the highest-duration entries; both are `null` when there's no activity yet. `hours` is 24 ints — minutes of active time per hour, indexed `hours[0]` = 00:00–00:59 local time.

### Quickshell

```qml
import Quickshell
import Quickshell.Io
import QtQuick

Scope {
  property string totalActive: "—"

  Process {
    id: poller
    command: ["daylog", "--json", "today"]
    running: true
    stdout: SplitParser {
      onRead: data => {
        try { totalActive = JSON.parse(data).today.total_active } catch (e) {}
      }
    }
  }

  Timer {
    interval: 30000; repeat: true; running: true
    onTriggered: poller.running = true
  }

  Text { text: totalActive }
}
```

See the [`Process` docs](https://quickshell.org/docs/v0.1.0/types/Quickshell.Io/Process/) for the API.

### waybar

```json
"custom/daylog": {
  "exec": "daylog --json today | jq -r '.today.total_active'",
  "interval": 30
}
```

### i3blocks

```
[daylog]
command=daylog --json today | jq -r '.today.total_active'
interval=30
```

## Configuration

Custom category rules live at `~/.config/daylog/categories.json`. Daylog ships with sensible defaults — edit only if you want different buckets.

## Compatibility

x86_64 Linux. Tested on Ubuntu, Debian, Fedora, Arch (incl. Omarchy / EndeavourOS / CachyOS), openSUSE, and derivatives. The tracker uses systemd-user units when available and falls back to XDG-autostart on non-systemd distros (Void, Artix, Devuan).

Display servers: X11, GNOME-Wayland (auto-installs the `focused-window-dbus` shell extension; logout/login once after install), KDE-Wayland, and wlroots compositors (Hyprland, Sway, river, …).

aarch64 and non-Linux platforms are not supported in v0.1.

## Build from source

```bash
git clone https://github.com/Manas-Kenge/Daylog
cd Daylog
cargo build --release -p daylog-tui
./target/release/daylog
```

The two crates in this workspace are:

- `daylog-core` — pure-Rust data layer (reads aw-server-rust's SQLite directly, plus queries, aggregations, KPI math).
- `daylog-tui` (binary `daylog`) — the ratatui dashboard plus the first-launch tracker installer.

## License

MIT — see [LICENSE](./LICENSE).

Daylog downloads `aw-server-rust` and `aw-awatcher` from the [ActivityWatch](https://activitywatch.net) project (MPL-2.0) on first launch. Full attribution lives in [THIRD-PARTY-NOTICES.md](./THIRD-PARTY-NOTICES.md).
