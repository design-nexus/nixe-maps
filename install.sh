#!/usr/bin/env bash
set -euo pipefail
# Piped from curl (`curl -fsSL …/install.sh | bash`): no checkout here, so fetch
# the source and run its installer from there.
if [[ -z ${BASH_SOURCE[0]:-} ]]; then
  command -v git >/dev/null || { echo "Git is needed to install nixe-maps." >&2; exit 1; }
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  git clone -q --depth 1 https://github.com/design-nexus/nixe-maps.git "$work/src"
  bash "$work/src/install.sh" "$@"
  exit
fi
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# A release archive (.prebuilt) brings its binaries.
if [[ ${NIXE_SKIP_BUILD:-0} != 1 && ! -f $root/.prebuilt ]]; then
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
