#!/bin/sh
# Daylog package prerm hook (.deb / .rpm).
#
# Before removal, stop our user services for each logged-in user so the
# package manager can delete the binaries cleanly without exec'd processes
# holding them. We don't `disable` here — that's the user's choice via
# `daylog --uninstall-tracking` or the Settings UI. A package upgrade also
# triggers prerm, and we don't want to forget the user's enabled state.
#
# Best-effort: errors are swallowed.

set -u

loginctl list-users --no-legend 2>/dev/null \
  | awk '$1 >= 1000 {print $1}' \
  | while read -r uid; do
      user="$(getent passwd "$uid" | cut -d: -f1)" || continue
      [ -z "$user" ] && continue
      runuser -u "$user" -- systemctl --user stop \
          daylog-awatcher.service daylog-aw-server.service 2>/dev/null || true
    done

exit 0
