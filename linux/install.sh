#!/usr/bin/env bash
set -euo pipefail
source_dir="$(cd "$(dirname "$0")" && pwd)"
app_id=com.xiaoyu.chipdock.gpui
binary="$HOME/.local/bin/chipdock-gpui"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
[[ "$data_dir" == /* ]] || { echo 'XDG_DATA_HOME must be an absolute path' >&2; exit 1; }
[[ "$binary$data_dir" != *$'\n'* ]] || { echo 'Installation paths must not contain newlines' >&2; exit 1; }
mkdir -p "$(dirname "$binary")" "$data_dir/applications" "$data_dir/icons" "$data_dir/doc"
pending="$(mktemp "$(dirname "$binary")/.chipdock-install.XXXXXX")"
trap 'rm -f "$pending"' EXIT
install -m 755 "$source_dir/bin/chipdock-gpui" "$pending"
mv -f "$pending" "$binary"
cp -R "$source_dir/share/icons/." "$data_dir/icons/"
cp -R "$source_dir/share/doc/." "$data_dir/doc/"
# Desktop Entry string escaping follows Exec argument quoting; %% is a literal percent.
exec_path="$(printf '%s' "$binary" | sed 's/\\/\\\\/g; s/["$`]/\\&/g; s/%/%%/g; s/\\/\\\\/g')"
while IFS= read -r line; do
  case "$line" in
    Exec=*) printf 'Exec="%s" %%f\n' "$exec_path" ;;
    *) printf '%s\n' "$line" ;;
  esac
done < "$source_dir/share/applications/$app_id.desktop" > "$data_dir/applications/$app_id.desktop"
if command -v gtk-update-icon-cache >/dev/null; then
  gtk-update-icon-cache -f -t "$data_dir/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$data_dir/applications" >/dev/null 2>&1 || true
fi
printf 'Installed ChipDock: %s\nOpen ChipDock from your applications menu.\n' "$binary"
