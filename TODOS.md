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

## TUI design polish (post–v0.2 first cut)

Surfaced in /plan-design-review on 2026-05-08 against `crates/daylog-tui/DESIGN.md`. None of these block the v0.2 release; they are quality-of-life polish for the second cut.

### Fresh-install empty state

**What:** Replace the per-panel `no <foo> events yet` strings with a single Overview-wide `ActivityWatch is collecting data — check back in a few minutes` when total events across all panels is zero **and** uptime since first launch is under 5 minutes.

**Why:** A brand-new user opens the TUI seconds after the wizard completes. Currently they see three "no X yet" messages that read like bugs; the truth is the tracker is working, it just hasn't filled a window yet.

**Pros:** Honest framing of a 30-second loading-then-data flow. Same energy as the desktop wizard's last screen.

**Cons:** Adds an "uptime since wizard-complete" lookup and a coordination hook between panels.

**Context:** Pass 2 of the design review. Not critical because the offline indicator already covers the case where the tracker fails to start.

**Depends on / blocked by:** None. Can land independently after the first TUI cut.

### Distinguish fetch-error from no-data on TopCategories and Hourly

**What:** Match TopApps's pattern — when `last_error()` is `Some`, render `fetch error · check footer` instead of falling back to `loading…`.

**Why:** Today, a stuck fetch on TopCategories looks identical to a slow fetch. The user can't tell whether to wait or panic.

**Pros:** ~6 lines of code, brings the three Overview panels to parity.

**Cons:** None.

**Context:** Pass 2 of the design review. Found in `crates/daylog-tui/src/ui/overview.rs` — TopApps does it right, the other two don't.

**Depends on / blocked by:** None.

### Arrow-key aliases for tab cycling

**What:** Treat `KeyCode::Right` as `l` (next tab) and `KeyCode::Left` as `h` (prev tab) in `app::handle_key`.

**Why:** Most users try arrow keys before vim keys. The current bindings reward people who already know the convention; the alias rewards everyone else without taking anything away.

**Pros:** Two added match arms. No conflicts with existing keys.

**Cons:** None.

**Context:** Pass 6 of the design review.

**Depends on / blocked by:** None.
