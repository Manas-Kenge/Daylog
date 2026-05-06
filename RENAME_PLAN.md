# Pulse → Daylog: Complete Rename Plan

## 0. Decisions required from the user before execution

These choices change the executable steps below — please confirm before any file is touched:

1. **New bundle identifier.** Current is `com.manas-kenge.pulse`. Default proposal: `com.manas-kenge.daylog` (preserves your namespace; is the lowest-risk reverse-DNS swap). If you've registered a different namespace for this product, name it now.
2. **User-data migration policy.** Changing identifier changes `app_config_dir()`, which means the wizard marker and category rules at `~/.config/com.manas-kenge.pulse/` are abandoned by the renamed build.
   - **Option A (clean break, recommended for v0.1 pre-1.0):** accept that any existing local installs will re-run the wizard and lose custom category rules. Tracking *data* (`~/.local/share/activitywatch/`) is unaffected — that dir is owned by aw-server-rust.
   - **Option B (migration shim):** add a one-shot Rust helper invoked at startup that, if the new `app_config_dir()` is empty *and* the legacy `~/.config/com.manas-kenge.pulse/` exists, moves it to the new location. Adds ~30 lines in `lib.rs`; has to keep working until you're confident no users are on pre-rename builds.
3. **Naming pattern.** Cargo currently uses `pulse` (binary) + `pulse_lib` (lib). Pick:
   - **Option A (preserve pattern):** `daylog` + `daylog_lib`. Matches the comment in `Cargo.toml` about Windows lib/bin name collision and is a true 1:1 rename. **Recommended.**
   - **Option B (simplify):** `daylog` + `daylog`. Requires removing the `[lib] name = …` line and adjusting the Windows-collision rationale. More work, no real benefit on Linux-only.
4. **Directory rename ordering.** The repo dir `/home/manas-kenge/dev/projects/pulse` → `daylog`:
   - **Option A (rename last):** do every in-repo edit first while still in `pulse/`, commit, then `mv` the directory. Safest because all relative tooling (Cargo, bun, IDE workspaces, CI run-from-root) is unaffected during edits.
   - **Option B (rename first):** rename the directory, reopen tools, then edit. Risks IDE/editor reload issues mid-rename.
   - **Recommended: Option A.** This plan assumes A.
5. **Runtime path migration on user machines.** Existing AppImage installs have binaries at `~/.local/share/pulse/bin/` and a `pulse-tracker.desktop` autostart entry — the renamed build looks at `~/.local/share/daylog/bin/` and `daylog-tracker.desktop`. The renamed build will silently re-extract binaries to the new path on first launch (idempotent), but the **old systemd units and autostart entry keep running** until manually stopped. Plan ships a `MIGRATION.md` step for end-users; do **not** auto-touch the old units (cross-package interference is bad form).

Assuming Option A throughout where applicable. The plan below is written for: identifier `com.manas-kenge.daylog`, naming pattern `daylog` + `daylog_lib`, directory rename last, no auto-migration shim.

---

## 1. Discovery step (run first; verify nothing is missing)

A single ripgrep invocation, scoped to tracked source and excluding generated/vendored content:

```bash
cd /home/manas-kenge/dev/projects/pulse && \
  rg -n --hidden --no-ignore-vcs \
     -g '!node_modules' -g '!dist' -g '!src-tauri/target' -g '!src-tauri/binaries' \
     -g '!src-tauri/extensions' -g '!.git' -g '!bun.lock' -g '!src-tauri/Cargo.lock' \
     -g '!*.png' -g '!*.ico' -g '!*.icns' \
     -e 'pulse' -e 'Pulse' -e 'PULSE' -e 'pulse_lib' \
     -e 'pulse-aw-server' -e 'pulse-awatcher' \
     -e 'pulse-tracker' -e 'pulse-supervisor'
```

Then a sanity check on the two excluded lockfiles (rename-relevant entries only — package contents pass through unchanged):

```bash
grep -n '"name": "pulse"\|^name = "pulse"' \
     /home/manas-kenge/dev/projects/pulse/bun.lock \
     /home/manas-kenge/dev/projects/pulse/src-tauri/Cargo.lock
```

Expected hits, against which the executor verifies: `package.json`, `bun.lock`, `index.html` (no current pulse hit there but title is `Tauri + React + Typescript` — flag for separate fix at end), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/tracking/{install,lifecycle,systemd,xdg_autostart}.rs`, all four files in `src-tauri/services/`, `scripts/{fetch-binaries,postinst,prerm}.sh`, `.github/workflows/{ci,release}.yml`, `README.md`, `CLAUDE.md`, `PLAN.md`, `src/pages/{Wizard,WeekPage}.tsx`, `src/lib/productive.ts`. (The `animate-pulse` Tailwind class in `src/components/ui/skeleton.tsx` is NOT a rename target — it's a Tailwind utility, leave it alone.)

If discovery surfaces anything outside this list, stop and reconcile before proceeding.

---

## 2. Stage-by-stage rename

Each stage leaves the repo coherent (compiles, type-checks). Commit after each. Verification commands per stage are listed at the end.

### Stage 1 — Frontend strings, package.json, docs (no compile impact)

Pure text edits; touches no build identity yet.

| File | Change |
|---|---|
| `/home/manas-kenge/dev/projects/pulse/package.json` | `"name": "pulse"` → `"name": "daylog"` (line 2). |
| `/home/manas-kenge/dev/projects/pulse/index.html` | `<title>Tauri + React + Typescript</title>` → `<title>Daylog</title>` (incidental polish — flag separately if you want to defer). |
| `/home/manas-kenge/dev/projects/pulse/src/pages/Wizard.tsx` | All "Pulse" → "Daylog" in user-facing strings (lines 106, 118, 124, 135, 176, 179, 205, 216, 230, 233, 244). On line 164, `journalctl --user -u pulse-aw-server` → `daylog-aw-server` — but only after Stage 4, so leave the `journalctl` snippet for that stage. Update only the prose strings here. |
| `/home/manas-kenge/dev/projects/pulse/src/pages/WeekPage.tsx` | Lines 8, 149, 228: "Pulse" → "Daylog" (comment + UI string). |
| `/home/manas-kenge/dev/projects/pulse/src/lib/productive.ts` | Line 8: "Pulse" → "Daylog" (comment only). |
| `/home/manas-kenge/dev/projects/pulse/README.md` | Title `# Pulse` → `# Daylog`; every prose mention "Pulse" → "Daylog". |
| `/home/manas-kenge/dev/projects/pulse/PLAN.md` | Same global prose rewrite — careful: also contains hardcoded service/file names (`pulse-aw-server.service.tmpl`, `~/.local/share/pulse/bin/`, `apt remove pulse`, etc.). Rewrite all of them in this stage since this is a doc-only file. |
| `/home/manas-kenge/dev/projects/pulse/CLAUDE.md` | Same — prose + the `pulse-aw-server.service` / `pulse-awatcher.service` / `~/.local/share/pulse/bin/` / `/usr/lib/pulse/bin/` / `pulse_lib::run()` / `pulse_lib::uninstall_blocking` / `pulse --help` / `pulse --uninstall-tracking` / `pulse-tracker.desktop` references. |

Verify: `bunx tsc --noEmit` (covers Wizard/WeekPage/productive). `bun run build` (full Vite build). Cargo and Tauri are untouched at this stage.

Commit: `chore: rename Pulse → Daylog in docs and frontend strings`

### Stage 2 — Rust crate rename (binary, lib, version stamp)

This is the load-bearing change for everything else. Order within the stage matters because `main.rs` references `pulse_lib::*`.

1. **`/home/manas-kenge/dev/projects/pulse/src-tauri/Cargo.toml`**
   - Line 2: `name = "pulse"` → `name = "daylog"` (binary/package name).
   - Line 14: `name = "pulse_lib"` → `name = "daylog_lib"`.
2. **`/home/manas-kenge/dev/projects/pulse/src-tauri/src/main.rs`**
   - Line 9: `pulse_lib::uninstall_blocking()` → `daylog_lib::uninstall_blocking()`.
   - Line 37: `pulse_lib::run()` → `daylog_lib::run()`.
   - Lines 8, 18–20, 27, 30–33: prose "Pulse" → "Daylog", `pulse-*.service` → `daylog-*.service`, `pulse-tracker.desktop` → `daylog-tracker.desktop`, `pulse` (CLI name in Usage) → `daylog`, `~/.local/share/pulse/bin/` → `~/.local/share/daylog/bin/`.
3. **`/home/manas-kenge/dev/projects/pulse/src-tauri/src/tracking/install.rs`**
   - Line 127: `fn pulse_version()` → `fn daylog_version()`. All call sites in same file (lines 94, 154) update to `daylog_version()`.
   - Line 136: `Ok(base.join("pulse").join("bin"))` → `Ok(base.join("daylog").join("bin"))`. **This is the AppImage runtime data path.**
   - Lines 52, 84–85, 178, 190: prose comments "Pulse" → "Daylog".
4. **`/home/manas-kenge/dev/projects/pulse/src-tauri/src/tracking/lifecycle.rs`**
   - Line 158: `cfg.join("autostart").join("pulse-tracker.desktop")` → `daylog-tracker.desktop`.
   - Line 183: `.map(|d| d.join("pulse").join("bin"))` → `.join("daylog").join("bin")`. **Mirrors install.rs change — must stay in sync.**
   - Line 41, 144, 176: prose "Pulse" → "Daylog", `pulse --uninstall-tracking` → `daylog --uninstall-tracking`, `~/.local/share/pulse/bin/` → `~/.local/share/daylog/bin/`.

Verify: `cd /home/manas-kenge/dev/projects/pulse/src-tauri && cargo check --all-targets && cargo test`. The `cargo check` will rewrite `Cargo.lock` — that's expected (the package entry name changes). Tauri/Vite untouched.

Commit: `chore(rust): rename pulse crate → daylog (binary + lib + AppImage data path)`

### Stage 3 — Tauri config (productName, identifier, window title, resource list)

The identifier change is irreversible-without-migration — this is the user-data boundary.

**`/home/manas-kenge/dev/projects/pulse/src-tauri/tauri.conf.json`**
- Line 3: `"productName": "pulse"` → `"productName": "daylog"`. (Drives `.AppImage`/`.deb`/`.rpm` filenames and the system-package runtime path `/usr/lib/<productName>/bin/`.)
- Line 5: `"identifier": "com.manas-kenge.pulse"` → `"identifier": "com.manas-kenge.daylog"`. (Drives `app_config_dir()` → `~/.config/com.manas-kenge.daylog/`, where `.wizard-complete` and `categories.json` live. Confirms decision §0.1 and §0.2.)
- Line 15: `"title": "pulse"` → `"title": "Daylog"` (window title; capitalized for UI).
- Lines 37–40: `services/pulse-*.tmpl` → `services/daylog-*.tmpl`. **Coupled with Stage 4** — these resource paths must match the renamed files.

Verify: do **not** run `bun run tauri build` until Stage 4 finishes (the resource list now references files that don't exist yet). `bunx tsc --noEmit` and `cargo check` are still safe and should pass. Defer the Tauri build verification.

Commit: `chore(tauri): rename productName + identifier to daylog`

### Stage 4 — Service file renames + service-name constants + scripts

Four template files renamed; constants in two Rust files updated to match; package scripts updated. This stage and Stage 3 together define the service layer.

**File renames** (use `git mv` to preserve history):
- `/home/manas-kenge/dev/projects/pulse/src-tauri/services/pulse-aw-server.service.tmpl` → `daylog-aw-server.service.tmpl`
- `/home/manas-kenge/dev/projects/pulse/src-tauri/services/pulse-awatcher.service.tmpl` → `daylog-awatcher.service.tmpl`
- `/home/manas-kenge/dev/projects/pulse/src-tauri/services/pulse-supervisor.sh.tmpl` → `daylog-supervisor.sh.tmpl`
- `/home/manas-kenge/dev/projects/pulse/src-tauri/services/pulse-tracker.desktop.tmpl` → `daylog-tracker.desktop.tmpl`

**Edits inside the renamed templates:**
- `daylog-aw-server.service.tmpl` line 2: `Description=Pulse activity tracking server (aw-server-rust)` → `Description=Daylog activity tracking server (aw-server-rust)`.
- `daylog-awatcher.service.tmpl` lines 2, 4, 5: `Pulse` → `Daylog`; `pulse-aw-server.service` → `daylog-aw-server.service` (in `After=` and `Requires=`).
- `daylog-supervisor.sh.tmpl` lines 2, 12: comment "Pulse" → "Daylog"; `LOG_DIR=...pulse` → `daylog`.
- `daylog-tracker.desktop.tmpl` lines 3–5: `Name=Pulse Tracker` → `Name=Daylog Tracker`; `Comment=Background activity tracker for Pulse` → `…for Daylog`; `Exec={BIN_DIR}/pulse-supervisor.sh` → `Exec={BIN_DIR}/daylog-supervisor.sh`.

**Rust source** — service-name constants and template lookups:
- `/home/manas-kenge/dev/projects/pulse/src-tauri/src/tracking/systemd.rs`
  - Line 9: `pub const SERVER_UNIT: &str = "pulse-aw-server.service"` → `"daylog-aw-server.service"`.
  - Line 10: `WATCHER_UNIT` → `"daylog-awatcher.service"`.
  - Lines 19, 25: `render_template(app, "pulse-aw-server.service.tmpl", …)` and `"pulse-awatcher.service.tmpl"` → matching `daylog-*` template names.
- `/home/manas-kenge/dev/projects/pulse/src-tauri/src/tracking/xdg_autostart.rs`
  - Line 9: `AUTOSTART_FILE: &str = "pulse-tracker.desktop"` → `"daylog-tracker.desktop"`.
  - Line 10: `SUPERVISOR_FILE: &str = "pulse-supervisor.sh"` → `"daylog-supervisor.sh"`.
  - Line 15: `render_template(app, "pulse-supervisor.sh.tmpl", …)` → `"daylog-supervisor.sh.tmpl"`.
  - Line 26: `render_template(app, "pulse-tracker.desktop.tmpl", …)` → `"daylog-tracker.desktop.tmpl"`.
  - Line 73: comment `pulse --uninstall-tracking` → `daylog --uninstall-tracking`.

**Package scripts** (`.deb`/`.rpm` hooks):
- `/home/manas-kenge/dev/projects/pulse/scripts/postinst.sh` lines 2, 6, 23: comments + `pulse-aw-server.service pulse-awatcher.service` → `daylog-aw-server.service daylog-awatcher.service`.
- `/home/manas-kenge/dev/projects/pulse/scripts/prerm.sh` lines 2, 7, 20: same — comments + the two service names in the `systemctl --user stop` invocation. Also line 7's `pulse --uninstall-tracking` → `daylog --uninstall-tracking`.
- `/home/manas-kenge/dev/projects/pulse/scripts/fetch-binaries.sh` lines 12, 119: `CACHE_DIR=…/pulse/binaries` → `daylog/binaries`; echo "Pulse binaries…" → "Daylog binaries…".

Verify: `cd /home/manas-kenge/dev/projects/pulse/src-tauri && cargo check --all-targets && cargo test`. Then `cd /home/manas-kenge/dev/projects/pulse && bun run tauri build` — this is the first end-to-end build with the full rename in effect, and confirms `tauri.conf.json` resource paths resolve. Bundle filenames will now contain "daylog".

Commit: `chore(tracking): rename systemd + XDG service files and constants to daylog`

### Stage 5 — CI cache keys + smoke commands

CI changes are isolated to two YAML files. Order: do this after Stage 4 so the bundle filenames the smoke jobs install actually contain "daylog".

**`/home/manas-kenge/dev/projects/pulse/.github/workflows/ci.yml`**
- Line 33: `path: ~/.cache/pulse/binaries` → `~/.cache/daylog/binaries` (matches Stage 4's `fetch-binaries.sh` change).
- Line 34: `key: pulse-binaries-${{ hashFiles(...) }}` → `daylog-binaries-…`.

**`/home/manas-kenge/dev/projects/pulse/.github/workflows/release.yml`**
- Line 41: cache path → `~/.cache/daylog/binaries`.
- Line 42: cache key prefix `pulse-binaries-` → `daylog-binaries-`.
- Line 116: step name "Install Pulse .deb…" → "Install Daylog .deb…".
- Lines 124, 127: step name "Smoke test — pulse --help" / "pulse --uninstall-tracking" → "daylog …".
- Lines 125, 128, 148, 149, 165, 166: command body `pulse --help` / `pulse --uninstall-tracking` → `daylog --help` / `daylog --uninstall-tracking`. (The renamed `productName: daylog` makes the installed binary `/usr/bin/daylog` on `.deb`/`.rpm`; the `.AppImage` argv[0] follows productName too.)
- Lines 145, 162: step name "Install Pulse .rpm" → "Install Daylog .rpm".
- Line 208: "Smoke test — pulse --help via AppImage" → "daylog --help via AppImage".

Verify: this is GitHub-only — no local verification possible beyond `actionlint .github/workflows/*.yml` if available, otherwise visual review. The next CI run on push proves it.

Commit: `chore(ci): rename pulse → daylog in workflows (cache keys, smoke commands)`

### Stage 6 — Repo directory rename (last)

After Stages 1–5 are committed and verified, rename the working directory itself.

```bash
cd /home/manas-kenge/dev/projects/ && mv pulse daylog
```

Then in any open IDE/terminal, reopen at `/home/manas-kenge/dev/projects/daylog/`. Re-run all verification one last time from the new path:

```bash
cd /home/manas-kenge/dev/projects/daylog && \
  bunx tsc --noEmit && \
  bun run build && \
  cd src-tauri && cargo check --all-targets && cargo test && \
  cd .. && bun run tauri build
```

No commit (directory location isn't tracked by git).

---

## 3. Per-stage verification matrix

| After stage | Run |
|---|---|
| 1 | `bunx tsc --noEmit` &nbsp;·&nbsp; `bun run build` |
| 2 | `cd src-tauri && cargo check --all-targets && cargo test` |
| 3 | `cd src-tauri && cargo check --all-targets` (Tauri build deferred — resources missing until Stage 4) |
| 4 | `cd src-tauri && cargo check --all-targets && cargo test` &nbsp;·&nbsp; `bun run tauri build` (full bundle — confirms filenames carry `daylog`) |
| 5 | Visual review of YAML; optional `actionlint` |
| 6 | All four checks again from the new directory path |

---

## 4. Out of scope (do NOT do automatically — flag back to user)

| Item | Why deferred |
|---|---|
| GitHub repo rename (`gh repo rename`) | User decision; affects external links, badges, anyone with a clone. |
| `git remote set-url origin …` | Coupled with the above; do only after the GitHub-side rename. |
| Deletion of user's existing `~/.local/share/pulse/bin/` on the dev machine | Contains binaries the *running* old app may still reference; user should remove only after stopping old systemd units manually. |
| Deletion of `~/.config/com.manas-kenge.pulse/` | Holds the user's wizard-complete marker and category rules. Decision §0.2 governs this; the rename plan itself never touches it. |
| `systemctl --user disable --now pulse-aw-server pulse-awatcher` on the user's dev machine | Old units keep running until the user (or `daylog --uninstall-tracking` *of the old binary*) stops them. The renamed app starts new daylog-* units in parallel — both stacks would race for port 5600. **The executor must instruct the user to run `systemctl --user disable --now pulse-aw-server pulse-awatcher && rm ~/.config/systemd/user/pulse-*.service ~/.config/autostart/pulse-tracker.desktop && systemctl --user daemon-reload` before launching the renamed dev build.** This is a runbook step, not a code edit. |
| Renaming existing git branches | Cosmetic; no functional impact. |
| Touching `bun.lock` and `Cargo.lock` by hand | Both regenerate from `package.json` and `Cargo.toml` respectively when their tooling runs. The "name" field updates ride along automatically; never edit these by hand. |
| The `animate-pulse` Tailwind class in `src/components/ui/skeleton.tsx` | Tailwind utility class — unrelated to product name. Leave it. |

---

## 5. Notes / subtleties the executor must respect

- **Rust crate-name-to-snake-case convention.** `daylog` (Cargo package name) implies `daylog_lib` (lib name) per the existing comment in `Cargo.toml` about the Windows lib/bin collision. Don't try to use a hyphen in the lib name; Rust crate identifiers can't contain `-` when used as a path.
- **`Cargo.lock` and `bun.lock` will mutate when their toolchains run** after the manifest edits. That's normal — commit the resulting deltas in the same stage as the manifest change.
- **Tauri resource paths are relative to `src-tauri/`** in `tauri.conf.json` — `services/daylog-aw-server.service.tmpl` is correct, not `src-tauri/services/…`.
- **`render_template` does substring substitution**, so the literal template *content* references `{BIN_DIR}` placeholders that are fine as-is — only the file *names* and the prose `Description=` / `Name=` lines change.
- **`pulse-tracker.desktop` cleanup in `lifecycle.rs::uninstall`** (line 158) intentionally targets the *new* name `daylog-tracker.desktop` after rename. Old installs' `pulse-tracker.desktop` is now orphaned; that's the cost of the identifier change and is documented in the user-runbook out-of-scope item above.
- **`pulse_version()` rename in `install.rs`** is purely an internal function name. Its return value (`env!("CARGO_PKG_VERSION")`) is unchanged — it reads from `Cargo.toml`'s `version` field, which we are not touching.
- **GitHub Actions cache keys** mutate when the prefix changes — the first CI run after Stage 5 will repopulate `~/.cache/daylog/binaries` from scratch (one slow run, then back to cached). Acceptable.
- **`CARGO_PKG_NAME`** is consumed transitively but not by anything in this codebase that I found — no code reads `env!("CARGO_PKG_NAME")` to construct paths. (Verified: grep for `CARGO_PKG_` in `src-tauri/src/` returns only the version usage in `install.rs`.)

---

### Critical Files for Implementation

- `/home/manas-kenge/dev/projects/pulse/src-tauri/Cargo.toml`
- `/home/manas-kenge/dev/projects/pulse/src-tauri/tauri.conf.json`
- `/home/manas-kenge/dev/projects/pulse/src-tauri/src/tracking/systemd.rs`
- `/home/manas-kenge/dev/projects/pulse/src-tauri/src/tracking/xdg_autostart.rs`
- `/home/manas-kenge/dev/projects/pulse/.github/workflows/release.yml`
