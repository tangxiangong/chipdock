"""Generate NSH entry metadata for ChipDock's secondary desktop ID or shell feed."""

import json
import os
from pathlib import Path
import sys

app = Path(__file__).resolve().parents[1]
kind, destination = sys.argv[1:]
dest = Path(destination).resolve()
dest.mkdir(parents=True, exist_ok=True)
config = json.loads((app / "nsh.json").read_text())
if kind == "desktop":
    app_id = "com.xiaoyu.chipdock.gpui"
    url = config["updater"]["url"]
    suffix = f'/{config["id"]}/latest.json'
    if not url.endswith(suffix):
        raise SystemExit("OTA feed path must end with the application ID and latest.json")
    config["updater"]["url"] = url[:-len(suffix)] + f"/{app_id}/latest.json"
    config["id"] = app_id
elif kind == "shell":
    config["updater"] = json.loads((app / "shell-updater.json").read_text())
else:
    raise SystemExit("expected desktop or shell")

for field in ("rust", "theme", "icon"):
    config[field] = os.path.relpath(app / config[field], dest).replace(os.sep, "/")
source = os.path.relpath(app / "App", dest).replace(os.sep, "/")
(dest / "App.tsx").write_text(f'import "./{source}";\n')
(dest / "nsh.json").write_text(json.dumps(config, indent=2) + "\n")
