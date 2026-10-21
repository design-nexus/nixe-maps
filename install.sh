#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if [[ ${NIXE_SKIP_BUILD:-0} != 1 ]]; then
  cargo build --release --manifest-path "$root/Cargo.toml"
fi

share="${XDG_DATA_HOME:-$HOME/.local/share}"
binary="${CARGO_TARGET_DIR:-$root/target}/release/nixe-maps"
[[ -f $binary ]] || { echo "Release binary missing: $binary" >&2; exit 1; }

install -Dm755 "$binary" "$HOME/.local/bin/nixe-maps"
install -Dm644 "$root/data/io.github.design_nexus.nixe.Maps.desktop" "$share/applications/io.github.design_nexus.nixe.Maps.desktop"
install -Dm644 "$root/data/icons/io.github.design_nexus.nixe.Maps.svg" "$share/icons/hicolor/scalable/apps/io.github.design_nexus.nixe.Maps.svg"

if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$share/applications" || true
fi

# Hyprland window rule
hypr="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
if [[ -d $hypr && -f $hypr/nixe.lua ]]; then
  if ! grep -q 'nixe\.Maps' "$hypr/nixe.lua"; then
    printf 'o.window("^io\\\\.github\\\\.design_nexus\\\\.nixe\\\\.Maps$", { float = true, center = true })\n' >> "$hypr/nixe.lua"
    if command -v hyprctl >/dev/null 2>&1; then
      hyprctl reload >/dev/null 2>&1 || true
    fi
  fi
fi

printf 'Installed Nixe Maps without opening it.\n'
