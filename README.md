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

- Linux with WebKitGTK 4.1 (Ubuntu 22.04+, Fedora 39+, Debian 12+).
- That's it. Pulse bundles its own copy of [`aw-server-rust`](https://github.com/ActivityWatch/aw-server-rust) and [`awatcher`](https://github.com/2e3s/awatcher) — install one `.deb` and you have everything. If you already have ActivityWatch installed, Pulse detects it and uses your existing setup instead.

## Build

See [PLAN.md → Phase 0 / Phase 1](./PLAN.md#7-implementation-phases).
