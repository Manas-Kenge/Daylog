# TODOS

Deferred work tracked outside the engineering plan. Each entry includes enough context that a future session (months later) can pick it up without reconstructing the reasoning.

## Tray-resident desktop mode (post-v0.2)

**What:** Daylog desktop stays resident after window close — either via a tray icon or a hidden window with `tauri-plugin-single-instance` raising the existing window on next launch.

**Why:** Addresses the original "few seconds to load" complaint that motivated the v0.2 TUI conversation. The TUI is a different surface (terminal); this fixes the desktop itself. With single-instance, the second `daylog` invocation is instant — no WebView re-init.

**Pros:**
- Eliminates the cold-start latency complaint entirely for users who keep Daylog running.
- Composes naturally with future GNOME extension or systray integration: "click → raise existing window."
- Low-effort: `tauri-plugin-single-instance` is a small lift.

**Cons:**
- Resident memory cost (~150–250MB for WebKitGTK process). Some users will object.
- Tray-icon territory varies wildly across Linux DEs (XEmbed vs SNI vs AppIndicator) — same reason PLAN.md §6 deferred a tray icon for v0.1.
- Adds a settings choice ("close window = quit / stay resident").

**Context:** Came up in /office-hours 2026-05-07 when discussing what makes "click extension button → desktop open" feel instant. The honest fix is: keep the WebView alive after first open. Two paths — tray icon (cross-DE pain) or hidden-window + single-instance (cleaner). PLAN.md §14.1 already lists `tauri-plugin-single-instance` as v0.2 prep work for the parked GNOME extension.

**Depends on / blocked by:**
- TUI v0.2 ships first (current focus).
- `tauri-plugin-single-instance` integration.
- Decision on tray vs hidden-window + raise-on-next-launch.

**Not blocking the TUI plan.** Revisit after TUI v0.2 ships and you have actual usage data on which surface (TUI / desktop window / future extension) is the daily driver.
