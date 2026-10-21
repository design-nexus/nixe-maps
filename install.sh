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

# Install custom action icons
mkdir -p "$share/icons/hicolor/scalable/actions"
for icon in "$root/data/icons/"*-symbolic.svg; do
  if [[ -f "$icon" ]]; then
    install -m644 "$icon" "$share/icons/hicolor/scalable/actions/"
  fi
done

if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$share/applications" || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t "$share/icons/hicolor" || true
fi

printf 'Installed Nixe Maps without opening it.\n'
