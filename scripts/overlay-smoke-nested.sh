#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
socket="signal-auras-overlay-smoke-$$"

export SIGNAL_AURAS_REPO="$repo"
export SIGNAL_AURAS_NESTED_WAYLAND_DISPLAY="$socket"

exec dbus-run-session -- kwin_wayland \
  --virtual \
  --width 3840 \
  --height 2160 \
  --output-count 1 \
  --socket "$socket" \
  --no-lockscreen \
  --no-global-shortcuts \
  --exit-with-session "$repo/scripts/overlay-smoke-nested-session.sh"
