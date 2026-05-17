# Third-Party Notices

Daylog downloads upstream binaries and a GNOME Shell extension on first launch. Each is used as-is, without modification, and remains under its original license. This file is the canonical attribution record; the same versions are pinned in [`crates/daylog/src/tracking/pins.rs`](./crates/daylog/src/tracking/pins.rs).

---

## `aw-server-rust` — Mozilla Public License 2.0

- **Project:** [ActivityWatch / aw-server-rust](https://github.com/ActivityWatch/aw-server-rust)
- **Pinned version:** `v0.13.2`
- **License:** MPL-2.0 — <https://mozilla.org/MPL/2.0/>
- **Role in Daylog:** the local HTTP server on `:5600` that stores and queries activity events. Daylog reads from it via the `daylog_tui::data::aw_client` module and never modifies it.

## `awatcher` (`aw-awatcher`) — Mozilla Public License 2.0

- **Project:** [2e3s / awatcher](https://github.com/2e3s/awatcher)
- **Pinned version:** `v0.3.3`
- **License:** MPL-2.0 — <https://mozilla.org/MPL/2.0/>
- **Role in Daylog:** the cross-DE Linux watcher that reports the focused window and AFK state to `aw-server-rust`.

## `focused-window-dbus@flexagoon.com` — see upstream

- **Project:** [flexagoon / focused-window-dbus](https://github.com/flexagoon/focused-window-dbus)
- **Pinned download tag:** `62865` (extensions.gnome.org)
- **License:** as published on the upstream repository.
- **Role in Daylog:** an optional GNOME Shell extension installed only on GNOME-Wayland sessions. It exposes the focused-window over D-Bus so `awatcher` can read it on Wayland (where the standard X11 path is unavailable).

---

## Source code for downloaded binaries

MPL-2.0 requires that the source for any distributed binary remains accessible. Each upstream repository above is the canonical source; `crates/daylog/src/tracking/pins.rs` pins the exact version + sha256 of each archive. To reproduce a downloaded binary, check out the matching tag in the upstream repository and follow its build instructions. To upgrade Daylog to a newer upstream version, edit `pins.rs` (URL + sha256) and update the pinned-version line in this file in the same commit.

## License of Daylog itself

Daylog's own source code is licensed under the MIT License. See [LICENSE](./LICENSE).
