"""Tests for scripts/install.sh against a fake release on the local disk.

The installer downloads with curl, which reads file:// URLs, so each test lays
out a releases folder shaped like GitHub's (latest/download/... and
download/<tag>/...) and points RSI_RALPH_DOWNLOAD_BASE at it. A fake `uname`
on PATH selects the platform, so detection is tested on any host.

Run with: python3 -m unittest discover -s scripts/install_tests -p 'test_*.py'
"""

import hashlib
import io
import os
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

INSTALL = Path(__file__).resolve().parents[1] / "install.sh"
SHELL = shutil.which("dash") or "/bin/sh"


def fake_binary(version: str) -> bytes:
    return f"#!/bin/sh\necho 'rsi-ralph {version}'\n".encode()


def publish(releases: Path, where: str, target: str, version: str, checksum=None):
    """Write rsi-ralph-<target>.tar.gz and its .sha256 under releases/where."""
    folder = releases / where
    folder.mkdir(parents=True, exist_ok=True)
    data = fake_binary(version)
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as tar:
        info = tarfile.TarInfo(f"rsi-ralph-{target}/rsi-ralph")
        info.size = len(data)
        info.mode = 0o755
        tar.addfile(info, io.BytesIO(data))
    archive = folder / f"rsi-ralph-{target}.tar.gz"
    archive.write_bytes(buffer.getvalue())
    digest = checksum or hashlib.sha256(archive.read_bytes()).hexdigest()
    (folder / f"{archive.name}.sha256").write_text(f"{digest}  {archive.name}\n")


class InstallShTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root)
        self.releases = self.root / "releases"
        self.bin = self.root / "fake-bin"
        self.bin.mkdir()
        self.dest = self.root / "dest"
        self.uname("Linux", "x86_64")

    def uname(self, system: str, machine: str):
        script = self.bin / "uname"
        script.write_text(
            f'#!/bin/sh\ncase "$1" in -s) echo {system} ;; -m) echo {machine} ;; esac\n'
        )
        script.chmod(0o755)

    def install(self, *args, env=None):
        environ = {
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "HOME": str(self.root / "home"),
            "RSI_RALPH_DOWNLOAD_BASE": self.releases.as_uri(),
            "RSI_RALPH_INSTALL_DIR": str(self.dest),
        }
        environ.update(env or {})
        return subprocess.run(
            [SHELL, str(INSTALL), *args],
            env=environ,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_installs_the_latest_release_for_the_detected_target(self):
        publish(self.releases, "latest/download", "x86_64-unknown-linux-gnu", "9.9.9")
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        installed = self.dest / "rsi-ralph"
        self.assertTrue(os.access(installed, os.X_OK))
        self.assertIn("rsi-ralph 9.9.9", result.stdout)
        self.assertIn("is not on PATH", result.stdout)

    def test_installs_a_pinned_version_from_its_tag(self):
        publish(self.releases, "download/v1.2.3", "aarch64-apple-darwin", "1.2.3")
        self.uname("Darwin", "arm64")
        result = self.install("--version", "v1.2.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("rsi-ralph 1.2.3", result.stdout)

    def test_maps_each_platform_to_its_target(self):
        cases = {
            ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
            ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
            ("Darwin", "arm64"): "aarch64-apple-darwin",
            ("Linux", "amd64"): "x86_64-unknown-linux-gnu",
        }
        for (system, machine), target in cases.items():
            with self.subTest(system=system, machine=machine):
                self.uname(system, machine)
                result = self.install("--dry-run")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(f"target  {target}", result.stdout)
                self.assertIn(f"rsi-ralph-{target}.tar.gz", result.stdout)

    def test_refuses_an_archive_whose_checksum_does_not_match(self):
        publish(
            self.releases,
            "latest/download",
            "x86_64-unknown-linux-gnu",
            "9.9.9",
            checksum="0" * 64,
        )
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum mismatch", result.stderr)
        self.assertFalse((self.dest / "rsi-ralph").exists())

    def test_reports_a_release_without_this_target(self):
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("could not download", result.stderr)

    def test_rejects_unsupported_platforms_and_points_windows_at_powershell(self):
        self.uname("Linux", "riscv64")
        self.assertIn("no prebuilt binary for riscv64", self.install("--dry-run").stderr)
        self.uname("FreeBSD", "x86_64")
        self.assertIn("no prebuilt binary for FreeBSD", self.install("--dry-run").stderr)
        self.uname("MINGW64_NT-10.0", "x86_64")
        self.assertIn("install.ps1", self.install("--dry-run").stderr)

    def test_target_and_dir_flags_override_detection_and_environment(self):
        other = self.root / "elsewhere"
        result = self.install(
            "--dry-run", "--target", "x86_64-unknown-linux-musl", "--dir", str(other)
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("target  x86_64-unknown-linux-musl", result.stdout)
        self.assertIn(f"into    {other}", result.stdout)

    def test_rejects_unknown_options_and_missing_values(self):
        unknown = self.install("--nope")
        self.assertNotEqual(unknown.returncode, 0)
        self.assertIn("unknown option --nope", unknown.stderr)
        missing = self.install("--version")
        self.assertNotEqual(missing.returncode, 0)
        self.assertIn("--version needs a value", missing.stderr)
        self.assertEqual(self.install("--help").returncode, 0)


if __name__ == "__main__":
    unittest.main()
