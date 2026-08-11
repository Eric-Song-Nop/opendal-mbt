#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("package-native-artifact.py")


class NativeArtifactTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "native/rust").mkdir(parents=True)
        (self.root / "native/include").mkdir(parents=True)
        (self.root / "native").mkdir(exist_ok=True)
        (self.root / "moon.mod").write_text(
            'name = "Eric-Song-Nop/opendal"\nversion = "0.1.0"\n',
            encoding="utf-8",
        )
        (self.root / "native/rust/Cargo.toml").write_text(
            """[package]
name = "opendal-mbt-native"
version = "0.1.0"
rust-version = "1.91"

[dependencies]
opendal = { version = "=0.58.1", default-features = false, features = ["blocking", "services-fs"] }
""",
            encoding="utf-8",
        )
        (self.root / "native/include/opendal_mbt.h").write_text(
            """#define OPENDAL_MBT_ABI_V1_MAJOR UINT32_C(1)
#define OPENDAL_MBT_ABI_V1_MINOR UINT32_C(2)
#define OPENDAL_MBT_ABI_V1_PATCH UINT32_C(3)
""",
            encoding="utf-8",
        )
        (self.root / "native/distribution-profile.json").write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "artifact_revision": "r1",
                    "service_profile": "local",
                    "services": ["memory", "fs"],
                    "rust_features": ["blocking", "services-fs"],
                    "targets": {
                        "aarch64-apple-darwin": {
                            "host_key": "darwin-arm64",
                            "minimum_macos_version": "11.0",
                        }
                    },
                }
            ),
            encoding="utf-8",
        )
        (self.root / "LICENSE").write_text("test license\n", encoding="utf-8")
        self.library = self.root / "libopendal_mbt_native.a"
        self.library.write_bytes(b"!<arch>\nfixture static library")
        self.native_libs = self.root / "native-static-libs.log"
        self.native_libs.write_text(
            "note: native-static-libs: -liconv -lSystem -lc -lm\n",
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def invoke(
        self,
        output: Path,
        target: str = "aarch64-apple-darwin",
        pinned: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        command = [
                sys.executable,
                str(SCRIPT),
                "--repo-root",
                str(self.root),
                "--library",
                str(self.library),
                "--native-static-libs-log",
                str(self.native_libs),
                "--rust-target",
                target,
                "--output-dir",
                str(output),
            ]
        if pinned is not None:
            command.extend(["--verify-pinned", str(pinned)])
        return subprocess.run(
            command,
            check=False,
            text=True,
            capture_output=True,
        )

    def test_archive_is_deterministic_and_self_describing(self) -> None:
        first = self.invoke(self.root / "first")
        second = self.invoke(self.root / "second")
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 0, second.stderr)
        first_result = json.loads(first.stdout)
        second_result = json.loads(second.stdout)
        self.assertEqual(first_result["archive_sha256"], second_result["archive_sha256"])

        archive = Path(first_result["archive"])
        self.assertEqual(hashlib.sha256(archive.read_bytes()).hexdigest(), first_result["archive_sha256"])
        with tarfile.open(archive, "r:gz") as contents:
            self.assertEqual(
                contents.getnames(),
                ["lib", "LICENSE", "lib/libopendal_mbt_native.a", "manifest.json"],
            )
            manifest = json.load(contents.extractfile("manifest.json"))

        self.assertEqual(manifest["binding_version"], "0.1.0")
        self.assertEqual(manifest["abi_version"], {"major": 1, "minor": 2, "patch": 3})
        self.assertEqual(manifest["opendal_version"], "0.58.1")
        self.assertEqual(manifest["services"], ["memory", "fs"])
        self.assertEqual(manifest["minimum_macos_version"], "11.0")
        self.assertEqual(
            manifest["system_link_flags"], ["-liconv", "-lSystem", "-lc", "-lm"]
        )
        self.assertEqual(
            manifest["static_library_sha256"],
            hashlib.sha256(self.library.read_bytes()).hexdigest(),
        )

    def test_unknown_target_is_rejected(self) -> None:
        result = self.invoke(self.root / "output", "x86_64-unknown-linux-gnu")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsupported Rust target", result.stderr)

    def test_version_mismatch_is_rejected(self) -> None:
        cargo = self.root / "native/rust/Cargo.toml"
        cargo.write_text(cargo.read_text().replace('version = "0.1.0"', 'version = "0.2.0"'), encoding="utf-8")
        result = self.invoke(self.root / "output")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Moon and Rust binding versions differ", result.stderr)

    def test_missing_native_link_report_is_rejected(self) -> None:
        self.native_libs.write_text("cargo finished\n", encoding="utf-8")
        result = self.invoke(self.root / "output")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not report native-static-libs", result.stderr)

    def test_published_digest_must_match_reproducible_build(self) -> None:
        first = self.invoke(self.root / "candidate")
        self.assertEqual(first.returncode, 0, first.stderr)
        result = json.loads(first.stdout)
        manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
        pinned_record = {
            **{key: value for key, value in manifest.items() if key != "schema_version"},
            "archive_name": result["archive_name"],
            "archive_size": result["archive_size"],
            "archive_sha256": result["archive_sha256"],
            "url": f"https://example.invalid/{result['archive_name']}",
        }
        table = self.root / "artifacts.json"
        table.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "artifacts": {"darwin-arm64": pinned_record},
                }
            ),
            encoding="utf-8",
        )
        verified = self.invoke(self.root / "verified", pinned=table)
        self.assertEqual(verified.returncode, 0, verified.stderr)

        pinned_record["archive_sha256"] = "0" * 64
        table.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "artifacts": {"darwin-arm64": pinned_record},
                }
            ),
            encoding="utf-8",
        )
        rejected = self.invoke(self.root / "rejected", pinned=table)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("archive_sha256 does not match", rejected.stderr)


if __name__ == "__main__":
    unittest.main()
