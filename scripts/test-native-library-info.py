#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path
import runpy
import subprocess
import unittest


ROOT = Path(__file__).resolve().parent.parent
SCRIPT = Path(__file__).with_name("check-native-library-info.py")
SCRIPT_GLOBALS = runpy.run_path(str(SCRIPT))
LibraryInfoError = SCRIPT_GLOBALS["LibraryInfoError"]
validate_manifest = SCRIPT_GLOBALS["validate_manifest"]


class NativeLibraryInfoTest(unittest.TestCase):
    def test_release_manifest_identity_is_accepted(self) -> None:
        flags = validate_manifest(
            {
                "binding_version": "1.2.3",
                "service_profile": "standard",
                "system_link_flags": ["-lc", "-lm"],
            },
            "1.2.3",
            "standard",
        )
        self.assertEqual(flags, ["-lc", "-lm"])

    def test_stale_native_binding_version_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            LibraryInfoError, "binding version differs from the release"
        ):
            validate_manifest(
                {
                    "binding_version": "1.2.2",
                    "service_profile": "standard",
                    "system_link_flags": ["-lc"],
                },
                "1.2.3",
                "standard",
            )

    def test_wrong_native_profile_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            LibraryInfoError, "service profile differs from the release"
        ):
            validate_manifest(
                {
                    "binding_version": "1.2.3",
                    "service_profile": "local",
                    "system_link_flags": ["-lc"],
                },
                "1.2.3",
                "standard",
            )

    def test_probe_is_strict_c11(self) -> None:
        result = subprocess.run(
            [
                "cc",
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wpedantic",
                "-fsyntax-only",
                str(ROOT / "tests/c/library_info_probe.c"),
            ],
            cwd=ROOT,
            check=False,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
