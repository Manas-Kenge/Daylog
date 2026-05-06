#!/usr/bin/env bash
# Bump a single component to a new upstream version: download the archive,
# compute its SHA-256, and rewrite scripts/binaries.lock for that row.
#
# Usage: scripts/bump-binary.sh <component> <new-version>
# Example: scripts/bump-binary.sh aw-server-rust v0.13.3

set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 <component> <new-version>" >&2
  exit 2
fi

COMPONENT="$1"
NEW_VERSION="$2"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$REPO_ROOT/scripts/binaries.lock"

# Mirror the URL logic in fetch-binaries.sh. Kept inline to avoid a shared lib.
archive_url() {
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
      echo "https://github.com/ActivityWatch/activitywatch/releases/download/${version}/activitywatch-${version}-linux-${arch}.zip" ;;
    aw-awatcher)
      echo "https://github.com/2e3s/awatcher/releases/download/${version}/aw-awatcher.zip" ;;
    focused-window-dbus@flexagoon.com)
      echo "https://extensions.gnome.org/download-extension/focused-window-dbus@flexagoon.com.shell-extension.zip?version_tag=${version}" ;;
    *) echo "fatal: unknown component $component" >&2; exit 1 ;;
  esac
}

# Pull every (component, target) row, recompute sha for ${NEW_VERSION}, rewrite.
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
changed=0

while IFS= read -r line; do
  case "$line" in '#'*|'') echo "$line" >> "$TMP"; continue ;; esac
  IFS=$'\t' read -r row_component row_version row_target row_sha <<< "$line"

  if [ "$row_component" != "$COMPONENT" ]; then
    echo "$line" >> "$TMP"; continue
  fi

  url="$(archive_url "$row_component" "$NEW_VERSION" "$row_target")"
  echo "→ $row_component $row_target: $row_version → $NEW_VERSION"
  echo "  fetching $url"
  tmp_archive="$(mktemp)"
  curl -fL --progress-bar -o "$tmp_archive" "$url"
  new_sha="$(sha256sum "$tmp_archive" | awk '{print $1}')"
  rm -f "$tmp_archive"
  echo "  sha256: $new_sha"
  printf '%s\t%s\t%s\t%s\n' "$row_component" "$NEW_VERSION" "$row_target" "$new_sha" >> "$TMP"
  changed=$((changed + 1))
done < "$LOCK"

if [ "$changed" -eq 0 ]; then
  echo "fatal: component '$COMPONENT' not found in $LOCK" >&2
  exit 1
fi

mv "$TMP" "$LOCK"
trap - EXIT
echo "Updated $changed row(s) in $LOCK."
