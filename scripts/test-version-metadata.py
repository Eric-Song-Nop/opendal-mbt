#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("check-version-metadata.py")


class VersionMetadataTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "native/rust").mkdir(parents=True)
        (self.root / "integration/consumer").mkdir(parents=True)
        (self.root / "moon.mod").write_text(
            'name = "example/opendal"\nversion = "1.2.3"\n',
            encoding="utf-8",
        )
        (self.root / "native/rust/Cargo.toml").write_text(
            '[package]\nname = "opendal-mbt-native"\nversion = "1.2.3"\n',
            encoding="utf-8",
        )
        (self.root / "Cargo.lock").write_text(
            'version = 4\n\n[[package]]\n'
            'name = "opendal-mbt-native"\nversion = "1.2.3"\n',
            encoding="utf-8",
        )
        (self.root / "integration/consumer/moon.mod").write_text(
            'name = "example/consumer"\n\n'
            'import {\n  "example/opendal@1.2.3",\n}\n',
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def invoke(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--repo-root", str(self.root)],
            check=False,
            text=True,
            capture_output=True,
        )

    def test_matching_versions_are_accepted(self) -> None:
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("example/opendal@1.2.3", result.stdout)

    def test_stale_registry_consumer_dependency_is_rejected(self) -> None:
        consumer = self.root / "integration/consumer/moon.mod"
        consumer.write_text(
            consumer.read_text(encoding="utf-8").replace("@1.2.3", "@1.1.0"),
            encoding="utf-8",
        )
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("integration consumer dependency differs", result.stderr)

    def test_stale_native_lockfile_version_is_rejected(self) -> None:
        lockfile = self.root / "Cargo.lock"
        lockfile.write_text(
            lockfile.read_text(encoding="utf-8").replace('"1.2.3"', '"1.2.2"'),
            encoding="utf-8",
        )
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Cargo.lock opendal-mbt-native version differs", result.stderr)

    def test_unversioned_registry_consumer_dependency_is_rejected(self) -> None:
        consumer = self.root / "integration/consumer/moon.mod"
        consumer.write_text(
            'name = "example/consumer"\n\nimport {\n  "example/opendal",\n}\n',
            encoding="utf-8",
        )
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("expected exactly one versioned", result.stderr)


if __name__ == "__main__":
    unittest.main()
