"""Prepare the signed OTA feed tree for one atomic publication on chipdock-ota."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory

from cryptography.exceptions import InvalidSignature
from preflight import (REMOTE, check_cli, check_signing_key, feed_folder, preflight,
                       remote_head, require, verify_manifest)

app = Path(__file__).resolve().parents[1]
repo = app.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("build", type=int)
parser.add_argument("out", type=Path, help="new directory for the complete feed tree")
parser.add_argument("--key", type=Path, default=repo / ".chipdock-ota-keys/release.secret")
mode = parser.add_mutually_exclusive_group()
mode.add_argument("--first-release", action="store_true", help="acknowledge absent branch, after checking release history")
mode.add_argument("--new-abi", action="store_true", help="acknowledge a fresh ABI on an existing branch")
parser.add_argument("--dry-run", action="store_true", help="verify feed without signing or writing output")
parser.add_argument("--offline-repo", type=Path, help="local Git fixture; dry-run only")
parser.add_argument("--fixture-manifests-only", action="store_true",
                    help="skip NSH package audit for synthetic offline fixtures only")
parser.add_argument("--abi-reviewed", action="store_true", help="confirm native exports/runtime compatible with target shells")
args = parser.parse_args()
output = args.out.resolve()
if output.exists():
    parser.error("choose a new output directory")
if args.offline_repo and not args.dry_run:
    parser.error("--offline-repo requires --dry-run")
if args.fixture_manifests_only and not args.offline_repo:
    parser.error("--fixture-manifests-only requires --offline-repo and --dry-run")
remote = str(args.offline_repo.resolve()) if args.offline_repo else REMOTE

try:
    with TemporaryDirectory() as temporary:
        stage = Path(temporary)
        config = json.loads((app / "nsh.json").read_text())
        base = preflight(config, args.build, stage, remote, args.first_release, args.new_abi,
                         verify_packages=not args.fixture_manifests_only)
        if args.dry_run:
            if args.fixture_manifests_only:
                print("FIXTURE ONLY: package signature audit skipped")
            print("DRY RUN: no signing, no output; native ABI and new release certificate not tested")
            sys.exit(0)
        require(args.abi_reviewed, "native ABI review required (--abi-reviewed)")
        check_cli()
        check_signing_key(config, args.key)
        output.parent.mkdir(parents=True, exist_ok=True)
        with TemporaryDirectory(dir=output.parent) as build_temp:
            tree = Path(build_temp) / "feed"
            folder = tree / feed_folder(config)
            folder.mkdir(parents=True)
            package = folder / f'ChipDock-{args.build}.nshpkg'
            subprocess.run(["nsh", "publish", app, "--id", config["id"], "--build", str(args.build),
                            "--key", args.key, "--cert", app / "ota/release.cert",
                            "--source-map", "none", "-o", package], check=True)
            subprocess.run(["nsh", "update-manifest", "--entry", app, "--package", package,
                            "--key", args.key, "-o", folder], check=True)
            number = verify_manifest(config, (folder / "latest.json").read_bytes(),
                                     (folder / "latest.json.sig").read_bytes(),
                                     lambda name: (folder / name).read_bytes())
            require(number == args.build, "generated manifest build mismatch")
            require(remote_head(stage, remote) == base, "remote changed during build; retry")
            require(not output.exists(), "output appeared during build")
            tree.rename(output)
except InvalidSignature:
    parser.error("OTA manifest signature verification failed")
except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as error:
    parser.error(str(error))
