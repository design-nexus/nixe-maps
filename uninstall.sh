#!/usr/bin/env bash
set -euo pipefail

share="${XDG_DATA_HOME:-$HOME/.local/share}"
rm -f "$HOME/.local/bin/nyx-maps"
rm -f "$share/applications/io.github.design_nexus.nyx.Maps.desktop"
rm -f "$share/icons/hicolor/scalable/apps/io.github.design_nexus.nyx.Maps.svg"

if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$share/applications" || true
fi

printf 'Uninstalled Nyx Maps.\n'
