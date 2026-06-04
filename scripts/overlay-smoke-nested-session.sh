#!/usr/bin/env bash
set -euo pipefail

cd "${SIGNAL_AURAS_REPO:?}"
export WAYLAND_DISPLAY="${SIGNAL_AURAS_NESTED_WAYLAND_DISPLAY:?}"
export QT_QPA_PLATFORM=wayland
export XDG_CURRENT_DESKTOP=KDE
export XDG_SESSION_TYPE=wayland
export KDE_FULL_SESSION=true
unset DISPLAY

cargo run -p signal-auras-cli -- doctor overlay-nested
