# Pulse

A native Linux desktop dashboard for ActivityWatch.

> **Status:** pre-scaffold. See [PLAN.md](./PLAN.md) for the engineering plan.

## What it is

A single-window native app that shows a beautiful real-time pulse of your day, sourced from a local [ActivityWatch](https://activitywatch.net) server. No browser tab, no cloud, no sign-in.

## What it isn't

- A replacement for `aw-server` or `aw-awatcher`. Pulse only renders the UI.
- Cross-platform (Linux-first; macOS/Windows are not on the roadmap for v0.1).
- A behavioral nudging tool. Pulse is observational.

## Prerequisites

- Linux with WebKitGTK 4.1 (Ubuntu 22.04+, Fedora 39+, Debian 12+, or any rolling-release distro).
- That's it. Pulse bundles its own copy of [`aw-server-rust`](https://github.com/ActivityWatch/aw-server-rust) and [`awatcher`](https://github.com/2e3s/awatcher) — install one artifact and you have everything. If you already have ActivityWatch installed, Pulse detects it and uses your existing setup instead.

## Supported Linux distros

Pulse ships three artifacts that together cover virtually every active desktop Linux distro:

| Format | Tested in CI on | Covers (by inheritance) |
|---|---|---|
| **`.AppImage`** | Ubuntu, Debian, Fedora, Arch, openSUSE | Anything with glibc ≥ 2.35 — including Manjaro, EndeavourOS, **Omarchy**, Garuda, ArcoLinux, Solus, and the niche derivatives we don't list explicitly |
| **`.deb`** | Ubuntu 22.04, Ubuntu 24.04, Debian 12 | Linux Mint, Pop!_OS, Zorin, elementary OS, Kubuntu/Xubuntu/Lubuntu, KDE Neon, MX Linux, Tuxedo OS, Kali, Raspberry Pi OS, Deepin |
| **`.rpm`** | Fedora 41, openSUSE Tumbleweed | Rocky Linux, AlmaLinux, RHEL, CentOS Stream, Mageia, Nobara, openSUSE Leap |

You don't need a Mint-specific or Pop-specific package — they install the Ubuntu `.deb` verbatim. Same for Manjaro / EndeavourOS / Omarchy + the AppImage.

**Known gaps:**

| Distro | Status | Workaround |
|---|---|---|
| Alpine, Chimera Linux | Won't run the AppImage (musl libc) | Use [distrobox](https://github.com/89luca89/distrobox) with a glibc container |
| NixOS | No first-party package | Community can derive a `default.nix` from the AppImage |
| Gentoo | No first-party package | Community can write an ebuild |
| Void / Artix / Devuan | AppImage runs; tracker uses XDG-autostart fallback (no systemd) | Tracking starts at login like normal |

## Build

See [PLAN.md → Phase 0 / Phase 1](./PLAN.md#7-implementation-phases).
