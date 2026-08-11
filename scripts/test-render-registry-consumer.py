#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("render-registry-consumer.py")


class RegistryConsumerRenderTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        template = self.root / "integration/consumer"
        template.mkdir(parents=True)
        (self.root / "moon.mod").write_text(
            'name = "example/opendal"\nversion = "7.8.9"\n',
            encoding="utf-8",
        )
        (template / "moon.mod").write_text(
            'name = "example/consumer"\n\n'
            'import {\n  "example/opendal@0.1.0",\n}\n',
            encoding="utf-8",
        )
        (template / "moon.pkg").write_text("supported_targets = \"native\"\n")
        (template / "consumer_test.mbt").write_text(
            'test { inspect("registry", content="registry") }\n',
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def invoke(
        self, version: str = "7.8.9", output_name: str = "rendered"
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--repo-root",
                str(self.root),
                "--output-dir",
                str(self.root / output_name),
                "--version",
                version,
            ],
            check=False,
            text=True,
            capture_output=True,
        )

    def test_tag_version_replaces_stale_template_dependency(self) -> None:
        result = self.invoke("7.8.9-rc.2")
        self.assertEqual(result.returncode, 0, result.stderr)
        rendered = (self.root / "rendered/moon.mod").read_text(encoding="utf-8")
        self.assertIn('"example/opendal@7.8.9-rc.2"', rendered)
        self.assertNotIn("@0.1.0", rendered)
        self.assertTrue((self.root / "rendered/consumer_test.mbt").is_file())

    def test_unversioned_template_dependency_is_rendered(self) -> None:
        template = self.root / "integration/consumer/moon.mod"
        template.write_text(
            template.read_text(encoding="utf-8").replace("@0.1.0", ""),
            encoding="utf-8",
        )
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        rendered = (self.root / "rendered/moon.mod").read_text(encoding="utf-8")
        self.assertIn('"example/opendal@7.8.9"', rendered)

    def test_missing_dependency_is_rejected_without_partial_output(self) -> None:
        template = self.root / "integration/consumer/moon.mod"
        template.write_text(
            template.read_text(encoding="utf-8").replace(
                "example/opendal", "example/other"
            ),
            encoding="utf-8",
        )
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("expected exactly one dependency", result.stderr)
        self.assertFalse((self.root / "rendered").exists())

    def test_invalid_version_is_rejected(self) -> None:
        result = self.invoke('7.8.9"')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid Moon module version", result.stderr)


if __name__ == "__main__":
    unittest.main()
