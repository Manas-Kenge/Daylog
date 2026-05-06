#!/bin/sh
# Daylog package postinst hook (.deb / .rpm).
#
# After install or upgrade, reload each logged-in user's systemd manager and
# *try-restart* our units. try-restart is a no-op if the unit isn't running,
# so this is safe on a fresh install (where no Daylog user has run the wizard
# yet) and useful on upgrades (where users with already-running services pick
# up the new binaries without manual action).
#
# Best-effort: every operation is wrapped with `|| true`. We never want a
# transient systemd error to fail the package install.

set -u

# Iterate logged-in regular users (UID >= 1000 to skip system accounts).
loginctl list-users --no-legend 2>/dev/null \
  | awk '$1 >= 1000 {print $1}' \
  | while read -r uid; do
      user="$(getent passwd "$uid" | cut -d: -f1)" || continue
      [ -z "$user" ] && continue
      runuser -u "$user" -- systemctl --user daemon-reload 2>/dev/null || true
      runuser -u "$user" -- systemctl --user try-restart \
          daylog-aw-server.service daylog-awatcher.service 2>/dev/null || true
    done

exit 0
