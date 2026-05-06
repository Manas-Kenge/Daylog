#!/usr/bin/env bash
# Fetch pinned upstream binaries and place them in src-tauri/binaries/
# under Tauri's externalBin target-triple naming convention.
#
# Deps: curl, unzip, sha256sum, awk. No dpkg-deb, no jq.
# Idempotent: re-running with a satisfied lockfile is a no-op.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$REPO_ROOT/scripts/binaries.lock"
OUT_DIR="$REPO_ROOT/src-tauri/binaries"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/pulse/binaries"

mkdir -p "$OUT_DIR" "$CACHE_DIR"

# Resolve (component, version, target) → (archive_url, extract_path)
# extract_path is the path *inside the archive* of the binary we want.
resolve() {
  local component="$1" version="$2" target="$3"
  local arch
  case "$target" in
    x86_64-unknown-linux-gnu) arch="x86_64" ;;
    *) echo "fatal: unsupported target $target" >&2; exit 1 ;;
  esac
  case "$component" in
    aw-server-rust)
      ARCHIVE_URL="https://github.com/ActivityWatch/activitywatch/releases/download/${version}/activitywatch-${version}-linux-${arch}.zip"
      ARCHIVE_NAME="activitywatch-${version}-linux-${arch}.zip"
      EXTRACT_PATH="activitywatch/aw-server-rust/aw-server-rust"
      ;;
    aw-awatcher)
      ARCHIVE_URL="https://github.com/2e3s/awatcher/releases/download/${version}/aw-awatcher.zip"
      ARCHIVE_NAME="aw-awatcher-${version}.zip"
      EXTRACT_PATH="aw-awatcher"
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
  local out="$OUT_DIR/${component}"
  local stamp="$out.sha"

  # Idempotency check — already extracted with the right archive sha?
  if [ -f "$out" ] && [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$want_sha" ]; then
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

  # Extract just the binary we want, regardless of zip layout.
  local tmp
  tmp="$(mktemp -d)"
  ( cd "$tmp" && unzip -q "$cached" "$EXTRACT_PATH" )
  install -m 0755 "$tmp/$EXTRACT_PATH" "$out"
  rm -rf "$tmp"
  echo "$want_sha" > "$stamp"
  echo "    → $out"
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

  echo "Pulse binaries → $OUT_DIR"
  while IFS=$'\t' read -r component version target sha; do
    case "$component" in '#'*|'') continue ;; esac
    fetch_one "$component" "$version" "$target" "$sha"
  done < "$LOCK"
  echo "All binaries ready."
}

main "$@"
