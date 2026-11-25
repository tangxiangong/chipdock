"""Offline policy tests; no production repository, signing key or NSH CLI."""

import base64
import contextlib
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

from preflight import CONTEXT, APP_ID, PREFIX, REF, MAX_BUILD, preflight, check_signing_key, audit_package


def b64(raw):
    return base64.b64encode(raw).decode()


class PreflightTests(unittest.TestCase):
    def setUp(self):
        self.tmp = TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.remote = self.root / "remote"
        self.remote.mkdir()
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "OTA fixture")
        self.git("config", "user.email", "ota@example.invalid")
        self.git("commit", "--allow-empty", "-qm", "initial")
        self.key = Ed25519PrivateKey.generate()
        public = b64(self.key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw))
        self.config = {"id": APP_ID, "build": 1, "version": "0.3.0", "rust": ".",
                         "theme": "theme.json", "icon": "icon.png", "updater": {
                             "url": PREFIX + "abi-1/" + APP_ID + "/latest.json", "publicKey": public,
                             "packages": {"roots": [public]}}}

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.remote, check=True, capture_output=True).stdout

    def commit(self):
        self.git("add", "-A")
        self.git("commit", "-qm", "fixture update")

    def branch(self):
        self.git("checkout", "-qb", "chipdock-ota")

    def feed(self, number, abi="abi-1", mutate=None):
        folder = self.remote / abi / APP_ID
        folder.mkdir(parents=True, exist_ok=True)
        name = f"ChipDock-{number}.nshpkg"
        payload = b"fixture only: not a real NSH package"
        (folder / name).write_bytes(payload)
        data = {"id": APP_ID, "version": "0.3.0", "platforms": {}, "package": {
            "version": "0.3.0", "build": number, "url": name, "sha256": hashlib.sha256(payload).hexdigest(),
            "size": len(payload), "minRuntime": "0.7.0"}}
        if mutate:
            mutate(data)
        body = json.dumps(data).encode()
        (folder / "latest.json").write_bytes(body)
        (folder / "latest.json.sig").write_text(b64(self.key.sign(CONTEXT + body)))
        return folder

    def existing(self, a=5):
        self.branch()
        self.feed(a)
        self.commit()

    def check(self, build, **kwargs):
        with TemporaryDirectory(dir=self.root) as scratch, contextlib.redirect_stdout(io.StringIO()):
            return preflight(self.config, build, Path(scratch), str(self.remote), verify_packages=False, **kwargs)

    def test_absent_requires_explicit_first_release(self):
        with self.assertRaisesRegex(ValueError, "first-release"):
            self.check(2)
        self.assertIsNone(self.check(2, first=True))

    def test_read_error_is_not_first_release(self):
        self.remote = self.root / "does-not-exist"
        with self.assertRaisesRegex(ValueError, "git ls-remote failed"):
            self.check(2, first=True)

    def test_build_and_u32(self):
        self.existing()
        for build in (1, 4, 5, MAX_BUILD + 1, True):
            with self.subTest(build=build), self.assertRaises(ValueError):
                self.check(build)
        self.assertEqual(self.check(6), self.git("rev-parse", REF).strip().decode())
        with self.assertRaisesRegex(ValueError, "first-release forbidden"):
            self.check(6, first=True)

    def test_incomplete_feed(self):
        self.branch()
        folder = self.feed(5)
        (folder / "latest.json").unlink()
        self.commit()
        with self.assertRaisesRegex(ValueError, "missing/nonregular"):
            self.check(6)

    def test_existing_empty_branch(self):
        self.branch()
        with self.assertRaisesRegex(ValueError, "feed missing"):
            self.check(2)

    def test_corrupt_signature_and_payload(self):
        self.existing()
        folder = self.remote / "abi-1" / APP_ID
        sig = folder / "latest.json.sig"
        original = sig.read_bytes()
        sig.write_text(b64(bytes(64)))
        self.commit()
        with self.assertRaises(InvalidSignature):
            self.check(6)
        sig.write_bytes(original)
        (folder / "ChipDock-5.nshpkg").write_bytes(b"x" * len((folder / "ChipDock-5.nshpkg").read_bytes()))
        self.commit()
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            self.check(6)

    def test_missing_signature(self):
        self.existing()
        (self.remote / "abi-1" / APP_ID / "latest.json.sig").unlink()
        self.commit()
        with self.assertRaisesRegex(ValueError, "missing/nonregular"):
            self.check(6)

    def test_signed_wrong_identity_and_cross_abi_url(self):
        self.existing()
        for change in (lambda d: d.update(id="other.app"),
                       lambda d: d["package"].update(url="../abi-2/ChipDock-3.nshpkg"),
                       lambda d: d["package"].update(minRuntime="0.8.0"),
                       lambda d: d["package"].update(build=True)):
            with self.subTest(change=change):
                self.feed(5, mutate=change)
                self.commit()
                with self.assertRaises(ValueError):
                    self.check(6)

    def test_mismatched_abi_configuration(self):
        self.config["updater"]["url"] = PREFIX + "abi-2/other.app/latest.json"
        with self.assertRaisesRegex(ValueError, "unexpected OTA URL"):
            self.check(2, first=True)

    def test_deleted_historical_package(self):
        self.existing()
        old = self.remote / "abi-1" / APP_ID / "ChipDock-9.nshpkg"
        old.write_bytes(b"old package")
        self.commit()
        old.unlink()
        self.commit()
        with self.assertRaisesRegex(ValueError, "immutable package already used"):
            self.check(9)
        with self.assertRaisesRegex(ValueError, "historical floor 9"):
            self.check(8)
        self.check(10)

    def test_new_abi_needs_shipped_floor(self):
        self.existing()
        self.config["updater"]["url"] = PREFIX + "abi-2/" + APP_ID + "/latest.json"
        with self.assertRaisesRegex(ValueError, "shipped build"):
            self.check(7, new_abi=True)
        self.config["build"] = 6
        with self.assertRaisesRegex(ValueError, "feed missing"):
            self.check(7)
        self.check(7, new_abi=True)

    def test_changed_remote_head_during_fetch(self):
        self.existing()
        with patch("preflight.remote_head", return_value="0" * 40):
            with self.assertRaisesRegex(ValueError, "remote moved"):
                self.check(6)

    def test_package_audit_metadata_mismatch(self):
        package = {"url": "ChipDock-5.nshpkg", "build": 3}
        def fake_run(command, **kwargs):
            self.assertNotIn("--key", command)
            stage = Path(command[command.index("--entry") + 1])
            config = json.loads((stage / "nsh.json").read_text())
            self.assertEqual(config["id"], APP_ID)
            self.assertEqual(config["updater"]["packages"], self.config["updater"]["packages"])
            self.assertNotIn("publicKey", config["updater"])
            (stage / "latest.json").write_text(json.dumps({"package": {**package, "build": 4}}))
        with patch("preflight.check_cli"), patch("preflight.subprocess.run", side_effect=fake_run):
            with self.assertRaisesRegex(ValueError, "differs from verified package metadata"):
                audit_package(self.config, package, b"fixture")

    def test_signing_key_must_match_shell_key(self):
        from cryptography.hazmat.primitives.serialization import PrivateFormat, NoEncryption
        secret = self.root / "fixture.secret"
        secret.write_text(b64(self.key.private_bytes(Encoding.Raw, PrivateFormat.Raw, NoEncryption())))
        check_signing_key(self.config, secret)
        self.config["updater"]["publicKey"] = b64(bytes(32))
        with self.assertRaisesRegex(ValueError, "does not match"):
            check_signing_key(self.config, secret)

    def test_desktop_uses_main_identity(self):
        app = self.root / "entry-app"
        app.mkdir()
        shutil.copytree(Path(__file__).parent, app / "ota", ignore=shutil.ignore_patterns("__pycache__"))
        (app / "nsh.json").write_text(json.dumps(self.config))
        dest = self.root / "desktop"
        subprocess.run([sys.executable, str(app / "ota/entry.py"), "desktop", str(dest)], check=True)
        result = json.loads((dest / "nsh.json").read_text())
        self.assertEqual(result["id"], APP_ID)
        self.assertEqual(result["updater"], self.config["updater"])
        self.assertEqual(result["build"], self.config["build"])

    def test_dry_run_cli_without_nsh_or_key(self):
        self.existing()
        app = self.root / "app"
        app.mkdir()
        shutil.copytree(Path(__file__).parent, app / "ota", ignore=shutil.ignore_patterns("__pycache__"))
        (app / "nsh.json").write_text(json.dumps(self.config))
        out = self.root / "does-not-exist" / "out"
        command = [sys.executable, str(app / "ota/build.py"), "6", str(out), "--dry-run",
                   "--offline-repo", str(self.remote), "--fixture-manifests-only", "--key", str(self.root / "missing.secret")]
        proc = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("DRY RUN", proc.stdout)
        self.assertFalse(out.parent.exists())
        command.remove("--dry-run")
        proc = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("requires --dry-run", proc.stderr)


if __name__ == "__main__":
    unittest.main()
