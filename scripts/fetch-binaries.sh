#!/usr/bin/env bash
# Fetch pinned upstream binaries and place them in src-tauri/binaries/
# under Tauri's externalBin target-triple naming convention.
#
# Deps: curl, unzip, sha256sum, awk. No dpkg-deb, no jq.
# Idempotent: re-running with a satisfied lockfile is a no-op.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$REPO_ROOT/scripts/binaries.lock"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/pulse/binaries"

mkdir -p "$CACHE_DIR"

# Resolve (component, version, target) → archive URL + name + extract path +
# on-disk output path + mode. EXTRACT_PATH may be empty, in which case the
# whole archive is copied to OUT_PATH as-is (used for GNOME extension zips).
resolve() {
  local component="$1" version="$2" target="$3"
  local arch=""
  if [ "$target" != "noarch" ]; then
    case "$target" in
      x86_64-unknown-linux-gnu) arch="x86_64" ;;
      *) echo "fatal: unsupported target $target" >&2; exit 1 ;;
    esac
  fi
  case "$component" in
    aw-server-rust)
      ARCHIVE_URL="https://github.com/ActivityWatch/activitywatch/releases/download/${version}/activitywatch-${version}-linux-${arch}.zip"
      ARCHIVE_NAME="activitywatch-${version}-linux-${arch}.zip"
      EXTRACT_PATH="activitywatch/aw-server-rust/aw-server-rust"
      OUT_PATH="$REPO_ROOT/src-tauri/binaries/aw-server-rust"
      OUT_MODE="0755"
      ;;
    aw-awatcher)
      ARCHIVE_URL="https://github.com/2e3s/awatcher/releases/download/${version}/aw-awatcher.zip"
      ARCHIVE_NAME="aw-awatcher-${version}.zip"
      EXTRACT_PATH="aw-awatcher"
      OUT_PATH="$REPO_ROOT/src-tauri/binaries/aw-awatcher"
      OUT_MODE="0755"
      ;;
    focused-window-dbus@flexagoon.com)
      # version is the extensions.gnome.org "pk" (download tag), not a semver.
      ARCHIVE_URL="https://extensions.gnome.org/download-extension/focused-window-dbus@flexagoon.com.shell-extension.zip?version_tag=${version}"
      ARCHIVE_NAME="focused-window-dbus-${version}.zip"
      EXTRACT_PATH=""
      OUT_PATH="$REPO_ROOT/src-tauri/extensions/focused-window-dbus@flexagoon.com.zip"
      OUT_MODE="0644"
      ;;
    *) echo "fatal: unknown component $component" >&2; exit 1 ;;
  esac
}

verify_archive() {
  local file="$1" want_sha="$2"
  local got_sha
  got_sha="$(sha256sum "$file" | awk '{print $1}')"
  if [ "$got_sha" != "$want_sha" ]; then
    echo "  sha256 mismatch:" >&2
    echo "    expected: $want_sha" >&2
    echo "    got:      $got_sha" >&2
    return 1
  fi
}

fetch_one() {
  local component="$1" version="$2" target="$3" want_sha="$4"
  resolve "$component" "$version" "$target"

  local cached="$CACHE_DIR/$want_sha-$ARCHIVE_NAME"
  local stamp="$OUT_PATH.sha"

  mkdir -p "$(dirname "$OUT_PATH")"

  # Idempotency check — already placed with the right archive sha?
  if [ -f "$OUT_PATH" ] && [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$want_sha" ]; then
    echo "  ✓ $component $version ($target) — already up to date"
    return 0
  fi

  echo "  ↓ $component $version ($target)"

  if [ ! -f "$cached" ]; then
    echo "    fetching $ARCHIVE_URL"
    curl -fL --progress-bar -o "$cached.tmp" "$ARCHIVE_URL"
    mv "$cached.tmp" "$cached"
  else
    echo "    cache hit ($CACHE_DIR)"
  fi

  verify_archive "$cached" "$want_sha"

  if [ -z "$EXTRACT_PATH" ]; then
    # Ship the whole archive (used for GNOME extension zips).
    install -m "$OUT_MODE" "$cached" "$OUT_PATH"
  else
    # Extract a single file from the archive, place at OUT_PATH.
    local tmp
    tmp="$(mktemp -d)"
    ( cd "$tmp" && unzip -q "$cached" "$EXTRACT_PATH" )
    install -m "$OUT_MODE" "$tmp/$EXTRACT_PATH" "$OUT_PATH"
    rm -rf "$tmp"
  fi
  echo "$want_sha" > "$stamp"
  echo "    → $OUT_PATH"
}

main() {
  if [ ! -f "$LOCK" ]; then
    echo "fatal: $LOCK not found" >&2; exit 1
  fi
  for cmd in curl unzip sha256sum awk; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
      echo "fatal: required command not found: $cmd" >&2; exit 1
    fi
  done

  echo "Pulse binaries + extensions → src-tauri/{binaries,extensions}/"
  while IFS=$'\t' read -r component version target sha; do
    case "$component" in '#'*|'') continue ;; esac
    fetch_one "$component" "$version" "$target" "$sha"
  done < "$LOCK"
  echo "All binaries ready."
}

main "$@"
