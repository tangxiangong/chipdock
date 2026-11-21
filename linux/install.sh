#!/usr/bin/env bash
set -euo pipefail
source_dir="$(cd "$(dirname "$0")" && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
[[ "$data" == /* ]] || { echo "XDG_DATA_HOME must be absolute" >&2; exit 1; }
[[ "$HOME$data" != *$'\n'* ]] || { echo "Installation paths must not contain newlines" >&2; exit 1; }
test -x "$source_dir/ChipDock"
test -s "$source_dir/nsh/bundle.nshc"
parent="$HOME/.local/lib"
app="$parent/chipdock"
id=com.xiaoyu.chipdock
test -f "$source_dir/$id.desktop"
test -s "$source_dir/$id.png"
test -s "$source_dir/$id.svg"
if [[ -e "$app" || -L "$app" ]]; then
  [[ -d "$app" && ! -L "$app" && ! -e "$app/package-managed" ]] || {
    echo "Cannot replace this installation: $app" >&2; exit 1
  }
  python3 -c 'import json,sys; sys.exit(json.load(open(sys.argv[1]))["id"] != sys.argv[2])' "$app/nsh/app.json" "$id"
fi
mkdir -p "$parent" "$HOME/.local/bin" "$data/applications" \
  "$data/icons/hicolor/256x256/apps" "$data/icons/hicolor/scalable/apps"
stage="$(mktemp -d "$parent/.chipdock.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
cp -R "$source_dir" "$stage/new"
if [[ -e "$app" ]]; then mv "$app" "$stage/previous"; fi
if ! mv "$stage/new" "$app"; then
  if [[ -e "$stage/previous" ]]; then mv "$stage/previous" "$app"; fi
  exit 1
fi
python3 - "$app/ChipDock" "$HOME/.local/bin/ChipDock" <<'PYLINK'
import os, sys, tempfile
from pathlib import Path
executable, link = map(Path, sys.argv[1:])
with tempfile.TemporaryDirectory(dir=link.parent) as temporary:
    pending = Path(temporary) / "ChipDock"
    pending.symlink_to(executable)
    os.replace(pending, link)
PYLINK
install -m 644 "$app/$id.png" "$data/icons/hicolor/256x256/apps/$id.png"
install -m 644 "$app/$id.svg" "$data/icons/hicolor/scalable/apps/$id.svg"
python3 - "$app/ChipDock" "$app/$id.desktop" "$data/applications/$id.desktop" <<'PY'
from pathlib import Path
import sys
exe, template, dest = sys.argv[1:]
if '\n' in exe: raise SystemExit("Invalid binary path")
exe = ''.join(('\\' if c in '\\"$`' else '') + c for c in exe).replace('\\','\\\\').replace('%','%%')
lines = Path(template).read_text().splitlines()
Path(dest).write_text('\n'.join(f'Exec="{exe}" %f' if line.startswith('Exec=') else line for line in lines) + '\n')
PY
