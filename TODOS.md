# TODOS

Deferred work tracked outside the engineering plan. Each entry includes enough context that a future session (months later) can pick it up without reconstructing the reasoning.

## Asciinema cast for the README

**What:** Record a short asciinema cast (~30s) of the daylog dashboard and embed it in the repo-root `README.md` and `crates/daylog/README.md`. Both currently have a `<!-- TODO: asciinema cast -->` placeholder.

**Why:** Terminal product, terminal demo. asciinema casts are text-selectable, lightweight, and render natively on GitHub + crates.io. A still screenshot can't show the actual feel.

**How to apply:** Run a fresh session with realistic data (an hour or two of usage). Record `cargo run --release -p daylog-tui` covering tab cycling (Today → Week → Month) and range chip rotation. Upload to asciinema.org and replace the placeholder lines.

**Depends on / blocked by:** Nothing. Can ship anytime after the first crates.io publish.

## AUR PKGBUILD

**What:** Submit `daylog-tui` (or `daylog-tui-bin`) to the Arch User Repository so Arch / Manjaro / EndeavourOS users can install via `yay -S daylog-tui` instead of `cargo install`.

**Why:** Arch is the natural fit for a CLI screen-time tracker — the audience overlap is high, and AUR coverage is the standard discovery path for Rust CLI tools in that ecosystem. The TUI's whole pitch is "small binary, one command, no GUI" — AUR users already think that way.

**How to apply:** Source PKGBUILD pulls from `cargo install --root pkgdir/usr daylog-tui`; `-bin` PKGBUILD downloads the GitHub Release tarball directly. Do `-bin` first (faster install for users), source second.

**Depends on / blocked by:** First crates.io publish + first GitHub Release must be live.

## Cross-platform TUI builds (macOS, Windows)

**What:** Build daylog binaries for macOS (x86_64 + aarch64) and Windows. The dashboard itself uses ratatui + crossterm + reqwest, all of which are cross-platform. Only the wizard's tracker installer is Linux-specific.

**Why:** Mac developers running aw-server locally are a real audience. Some users run aw-server in a VM or on a remote Linux box and want to read it from their primary OS.

**How to apply:** Add target builds to release.yml. The wizard needs to gracefully degrade on non-Linux: if the tracker installer can't run (no systemd / no XDG-autostart), show a "please install ActivityWatch separately" message and skip the install step. The dashboard itself works against any aw-server instance.

**Depends on / blocked by:** Requires a real demand signal (someone asking) before doing the work. The wizard refactor is moderate effort (~1 day) for a question we don't yet have an answer for.

## TUI design polish (post-v0.1)

Surfaced in /plan-design-review on 2026-05-08 against `crates/daylog/DESIGN.md`. None of these block the v0.1 release; they are quality-of-life polish for v0.2.

### Fresh-install empty state

**What:** Replace the per-panel `no <foo> events yet` strings with a single Overview-wide `ActivityWatch is collecting data — check back in a few minutes` when total events across all panels is zero **and** uptime since first launch is under 5 minutes.

**Why:** A brand-new user opens the TUI seconds after the wizard completes. Currently they see three "no X yet" messages that read like bugs; the truth is the tracker is working, it just hasn't filled a window yet.

**Pros:** Honest framing of a 30-second loading-then-data flow.

**Cons:** Adds an "uptime since wizard-complete" lookup and a coordination hook between panels.

**Context:** Pass 2 of the design review. Not critical because the offline indicator already covers the case where the tracker fails to start.

**Depends on / blocked by:** None. Can land independently.

## Pin codegen from a lockfile (developer ergonomics)

**What:** Auto-generate `crates/daylog/src/tracking/pins.rs` from a checked-in lockfile (similar to the previous `scripts/binaries.lock`) via a `build.rs` script.

**Why:** Right now bumping a pinned upstream binary requires editing the URL + sha256 in `pins.rs` by hand. A lockfile + codegen would let `scripts/bump-binary.sh`-style automation come back without coupling the build to a shell script.

**How to apply:** New `crates/daylog/binaries.lock` (TSV: name, version, target, sha256, url-template). `build.rs` parses it, emits `OUT_DIR/pins.rs`, `tracking/pins.rs` re-exports via `include!`. Renovate config tracks the lockfile.

**Depends on / blocked by:** Not blocking anything. Worth doing once we have ≥3 pinned upstream binaries and the bump cadence becomes a maintenance burden.
