"""Prepare the signed OTA feed tree for one atomic publication on chipdock-ota."""

import argparse
import json
import re
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from urllib.parse import urlsplit

app = Path(__file__).resolve().parents[1]
repo = app.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("build", type=int)
parser.add_argument("out", type=Path, help="new directory for the complete feed tree")
parser.add_argument("--key", type=Path, default=repo / ".chipdock-ota-keys/release.secret")
args = parser.parse_args()
output = args.out.resolve()
if output.exists():
    parser.error("choose a new output directory")
output.parent.mkdir(parents=True, exist_ok=True)

with TemporaryDirectory(dir=output.parent) as temporary:
    stage = Path(temporary)
    desktop = stage / "desktop"
    subprocess.run([sys.executable, app / "ota/entry.py", "desktop", desktop], check=True)
    tree = stage / "feed"
    for entry in (app, desktop):
        config = json.loads((entry / "nsh.json").read_text())
        if not config["build"] < args.build <= 4294967295:
            parser.error("build must exceed the bundled build and fit in a u32")
        url = urlsplit(config["updater"]["url"])
        prefix = "/tangxiangong/chipdock/chipdock-ota/"
        if url.hostname != "raw.githubusercontent.com" or not url.path.startswith(prefix):
            parser.error("unexpected OTA branch URL")
        parts = url.path[len(prefix):].split("/")
        if (len(parts) != 3 or not re.fullmatch(r"abi-[0-9]+", parts[0])
                or parts[1:] != [config["id"], "latest.json"]
                or not re.fullmatch(r"[a-zA-Z0-9]+(?:\.[a-zA-Z0-9]+)+", config["id"])):
            parser.error("expected an ABI directory, application ID and latest.json")
        folder = tree / parts[0] / parts[1]
        folder.mkdir(parents=True)
        package = folder / f'ChipDock-{args.build}.nshpkg'
        subprocess.run(["nsh", "publish", app, "--id", config["id"], "--build", str(args.build),
                        "--key", args.key, "--cert", app / "ota/release.cert",
                        "--source-map", "none", "-o", package], check=True)
        subprocess.run(["nsh", "update-manifest", "--entry", entry, "--package", package,
                        "--key", args.key, "-o", folder], check=True)
    tree.rename(output)
