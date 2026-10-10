"""Validate opt-in example preflight without dispatching provider requests."""
import pathlib
import subprocess
import tempfile
import unittest


class ExampleSetup(unittest.TestCase):
    def run_missing_setting(self, missing):
        target = pathlib.Path("target/example-setup-tests")
        target.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=target) as parent:
            root = pathlib.Path(parent).resolve() / "unused-run"
            env = {
                "PATH": "/usr/bin:/bin",
                "TINYRSI_LIVE_OPENROUTER": "1",
                "TINYRSI_COMPLETE_DIR": str(root),
                "TINYRSI_LEAN": "/usr/bin/true",
                "OPENROUTER_API_KEY": "placeholder-unused",
            }
            del env[missing]
            result = subprocess.run(
                [str(pathlib.Path("target/debug/examples/complete_openrouter").resolve())],
                env=env, capture_output=True, timeout=10,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(root.exists(), "invalid setup must not consume a fresh run path")

    def test_missing_lean_does_not_create_output_directory(self):
        self.run_missing_setting("TINYRSI_LEAN")

    def test_missing_key_does_not_create_output_directory(self):
        self.run_missing_setting("OPENROUTER_API_KEY")


if __name__ == "__main__":
    unittest.main()
