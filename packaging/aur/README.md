# AUR — `daylog-bin`

This directory holds a reference `PKGBUILD` for the [Arch User Repository](https://aur.archlinux.org/) package `daylog-bin`. It downloads the pre-built tarball from the matching GitHub Release; no Rust toolchain on the user's machine, no build time.

## Publishing a new version

1. Bump `pkgver` and `pkgrel=1` (reset rel on every version bump).
2. Update the tarball `sha256sums` line — run `sha256sum` against the matching `daylog-<ver>-x86_64-unknown-linux-gnu.tar.gz` from the GitHub Release.
3. Regenerate `.SRCINFO` next to the PKGBUILD:
   ```bash
   makepkg --printsrcinfo > .SRCINFO
   ```
4. Push to the AUR git remote (one-time setup needed; see https://wiki.archlinux.org/title/AUR_submission_guidelines).
   ```bash
   git clone ssh://aur@aur.archlinux.org/daylog-bin.git aur-daylog-bin
   cp PKGBUILD .SRCINFO aur-daylog-bin/
   cd aur-daylog-bin && git add -A && git commit -m "v$pkgver" && git push
   ```

## Local smoke test

```bash
makepkg -si           # build + install into the local system
which daylog          # /usr/bin/daylog
daylog --version
```

## Why `-bin` and not a from-source package?

Building from source pulls a Rust toolchain (~500 MB) and compiles `rusqlite`'s bundled SQLite — slow and toolchain-heavy for what is otherwise a single static binary. The `-bin` variant matches user expectations on Arch (cf. `bat-bin`, `fd-bin`, `ripgrep-bin`).

A from-source `daylog` package can be added later if the community asks for one.
