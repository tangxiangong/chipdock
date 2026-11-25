"""Read-only preflight for ChipDock's unified signed NSH 0.7.0 feed."""

import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

REMOTE = "https://github.com/tangxiangong/chipdock.git"
REF = "refs/heads/chipdock-ota"
PREFIX = "https://raw.githubusercontent.com/tangxiangong/chipdock/chipdock-ota/"
APP_ID = "com.tangxiangong.chipdock"
MAX_BUILD = 2**32 - 1
CONTEXT = b"nsh-updater-manifest\0"  # nsh-updater 0.7.0
PACKAGE_PATH = re.compile(r"(abi-[0-9]+)/([^/]+)/ChipDock-([0-9]+)\.nshpkg")
RUNTIME = (0, 7, 0)  # Cargo.lock at the audited commit; update with NSH.


def require(ok, message):
    if not ok:
        raise ValueError(message)


def build_number(value):
    require(type(value) is int and 0 <= value <= MAX_BUILD, "invalid u32 build")
    return value


def decode(value, size):
    raw = base64.b64decode(value.strip(), validate=True)
    require(len(raw) == size, "invalid key/signature length")
    return raw


def unique_object(pairs):
    result = {}
    for k, v in pairs:
        require(k not in result, f"duplicate JSON key: {k}")
        result[k] = v
    return result


def feed_folder(config):
    app_id = config["id"]
    require(app_id == APP_ID, f"unexpected OTA identity: {app_id}")
    url = config["updater"]["url"]
    match = re.fullmatch(re.escape(PREFIX) + r"(abi-[0-9]+)/" + re.escape(app_id) + r"/latest\.json", url)
    require(match is not None, f"unexpected OTA URL: {url}")
    decode(config["updater"]["publicKey"], 32)
    roots = config["updater"]["packages"]["roots"]
    require(isinstance(roots, list) and bool(roots), "missing package trust roots")
    for root in roots:
        decode(root, 32)
    build_number(config["build"])
    return f"{match[1]}/{app_id}"


def verify_manifest(config, body, signature, read_package, audit_package=None):
    # Do not parse or reserialize before verifying the exact signed bytes.
    key = Ed25519PublicKey.from_public_bytes(decode(config["updater"]["publicKey"], 32))
    key.verify(decode(signature, 64), CONTEXT + body)
    data = json.loads(body, object_pairs_hook=unique_object)
    require(data["id"] == config["id"], "manifest identity mismatch")
    require(data.get("platforms", {}) == {}, "not an OTA-only manifest")
    pkg = data["package"]
    number = build_number(pkg["build"])
    name = f"ChipDock-{number}.nshpkg"
    require(pkg["url"] == name, "package URL/build mismatch or cross-ABI path")
    require(isinstance(pkg["version"], str) and pkg["version"] == data["version"], "package version mismatch")
    runtime = pkg["minRuntime"]
    require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", runtime) is not None, "invalid minRuntime")
    require(tuple(map(int, runtime.split("."))) <= RUNTIME, "package requires newer NSH runtime")
    require(re.fullmatch(r"[0-9a-fA-F]{64}", pkg["sha256"]) is not None, "invalid SHA-256")
    payload = read_package(name)
    require(type(pkg["size"]) is int and pkg["size"] == len(payload) > 0, "package size mismatch")
    require(hashlib.sha256(payload).hexdigest() == pkg["sha256"].lower(), "package SHA-256 mismatch")
    if audit_package is not None:
        audit_package(config, pkg, payload)
    return number


def check_cli():
    version = subprocess.run(["nsh", "--version"], check=True, capture_output=True, text=True).stdout.strip()
    require(version == "nsh " + ".".join(map(str, RUNTIME)), f"use the pinned NSH CLI, got {version!r}")


def audit_package(config, package, payload):
    """Use the pinned CLI to verify the real package, without a signing key."""
    check_cli()
    with TemporaryDirectory() as temp:
        stage = Path(temp)
        # Verification-only entry: unsigned local manifest; original roots and
        # identity retained. Never publish these files or modify shell config.
        audit_config = {**config, "updater": {"github": "tangxiangong/chipdock",
                        "packages": config["updater"]["packages"]}}
        (stage / "nsh.json").write_text(json.dumps(audit_config))
        (stage / "App.tsx").write_text("// verification only\n")
        path = stage / package["url"]
        path.write_bytes(payload)
        subprocess.run(["nsh", "update-manifest", "--entry", stage, "--package", path,
                        "-o", stage], check=True)
        actual = json.loads((stage / "latest.json").read_text())["package"]
        require(actual == package, "signed feed differs from verified package metadata")


def check_signing_key(config, path):
    private = Ed25519PrivateKey.from_private_bytes(decode(path.read_bytes(), 32))
    public = private.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    require(public == decode(config["updater"]["publicKey"], 32),
            "release key does not match shell feed public key")


def git(cwd, *args, allow_absent=False):
    result = subprocess.run(["git", *args], cwd=cwd, capture_output=True, timeout=180,
                            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    if allow_absent and result.returncode == 2 and not result.stdout:
        return None
    require(result.returncode == 0, f"git {args[0]} failed ({result.returncode}): "
            + result.stderr.decode(errors="replace").strip())
    return result.stdout


def remote_head(cwd, remote):
    result = git(cwd, "ls-remote", "--exit-code", "--refs", remote, REF, allow_absent=True)
    if result is None:
        return None
    match = re.fullmatch(rb"([0-9a-f]{40})\t" + REF.encode() + rb"\n", result)
    require(match is not None, "unexpected ls-remote response")
    return match[1].decode()


def preflight(config, candidate, scratch, remote=REMOTE, first=False, new_abi=False,
              verify_packages=True):
    folder = feed_folder(config)
    abi = folder.split("/")[0]
    require(config["build"] < build_number(candidate), "build must exceed bundled build")
    head = remote_head(scratch, remote)
    if head is None:
        require(first and not new_abi, "branch absent; explicit --first-release required")
        print(f"FIRST_RELEASE: no advertised {REF}; build={candidate}")
        return None
    require(not first, "branch exists; --first-release forbidden")
    snapshot = scratch / "snapshot.git"
    git(scratch, "init", "--bare", str(snapshot))
    # Full reachable history detects even removed numbered packages.
    git(snapshot, "fetch", "--no-tags", "--no-recurse-submodules", remote, REF)
    require(git(snapshot, "rev-parse", "FETCH_HEAD").strip().decode() == head,
            "remote moved during read; retry")
    entries = {}
    for row in git(snapshot, "ls-tree", "-r", "-t", "-z", head).split(b"\0"):
        if row:
            meta, name = row.split(b"\t", 1)
            entries[name.decode()] = meta.split()[0]
    history = set(git(snapshot, "log", "--format=", "--name-only", "--no-renames", "-m", "-z", head).decode().split("\0"))
    history.update(entries)
    historical = [(m[1], build_number(int(m[3]))) for p in history
                  if (m := PACKAGE_PATH.fullmatch(p)) and m[2] == APP_ID]
    for part in (abi, folder):
        require(part not in entries or entries[part] == b"040000", f"not a directory: {part}")
    path = f"{folder}/ChipDock-{candidate}.nshpkg"
    require(path not in history, f"immutable package already used: {path}")
    older = [n for a, n in historical if a != abi]
    require(not older or config["build"] > max(older), "shipped build must exceed previous ABI packages")
    if not any(p == folder or p.startswith(folder + "/") for p in entries):
        require(new_abi and bool(older) and not any(p.startswith(folder + "/") for p in history),
                "feed missing on existing branch; not a verified new ABI")
        published = []
        state = "NEW_ABI"
    else:
        require(not new_abi, "feed exists; unexpected --new-abi")
        def read(name):
            path = f"{folder}/{name}"
            require(entries.get(path) in (b"100644", b"100755"), f"missing/nonregular file: {path}")
            return git(snapshot, "show", f"{head}:{path}")
        number = verify_manifest(config, read("latest.json"), read("latest.json.sig"), read,
                                 audit_package if verify_packages else None)
        published = [number]
        state = "EXISTING_FEED"
        print(f"{folder}: verified build={number}")
    floor = max([config["build"]] + published + [n for _, n in historical])
    require(candidate > floor, f"build must exceed published/historical floor {floor}")
    print(f"{state}: base={head} floor={floor} build={candidate}")
    return head
