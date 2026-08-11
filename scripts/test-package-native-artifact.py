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
        (self.root / "native/distribution-profiles").mkdir(parents=True)
        (self.root / "moon.mod").write_text(
            'name = "Eric-Song-Nop/opendal"\nversion = "0.1.0"\n',
            encoding="utf-8",
        )
        (self.root / "native/rust/Cargo.toml").write_text(
            """[package]
name = "opendal-mbt-native"
version = "0.1.0"
rust-version = "1.91"

[features]
default = ["profile-standard"]
profile-local = ["opendal/blocking", "opendal/services-fs"]
profile-standard = [
  "profile-local",
  "layers-concurrent-limit",
  "layers-timeout-retry",
  "opendal/services-s3",
  "opendal/http-transport-reqwest",
  "opendal/http-transport-reqwest-rustls",
  "opendal/executors-tokio",
]
layers-concurrent-limit = ["opendal/layers-concurrent-limit"]
layers-timeout-retry = ["opendal/layers-retry", "opendal/layers-timeout"]

[dependencies]
opendal = { version = "=0.58.1", default-features = false }
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
        targets = {
            "aarch64-apple-darwin": {
                "host_key": "darwin-arm64",
                "minimum_macos_version": "11.0",
                "required_frameworks": ["Security", "CoreFoundation"],
            }
        }
        (self.root / "native/distribution-profile.json").write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "artifact_revision": "r1",
                    "service_profile": "local",
                    "services": ["memory", "fs"],
                    "rust_features": ["blocking", "services-fs"],
                    "targets": targets,
                }
            ),
            encoding="utf-8",
        )
        (self.root / "native/distribution-profiles/standard.json").write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "artifact_revision": "r1",
                    "service_profile": "standard",
                    "services": ["memory", "fs", "s3"],
                    "rust_features": [
                        "blocking",
                        "services-fs",
                        "services-s3",
                        "http-transport-reqwest",
                        "http-transport-reqwest-rustls",
                        "executors-tokio",
                        "layers-retry",
                        "layers-timeout",
                        "layers-concurrent-limit",
                    ],
                    "cargo_features": ["profile-standard"],
                    "runtime_initialization": "install_default",
                    "targets": targets,
                }
            ),
            encoding="utf-8",
        )
        self.artifact_tables = {
            "local": self.root / "native/artifacts.json",
            "standard": self.root / "native/artifacts-standard.json",
        }
        for profile, table in self.artifact_tables.items():
            table.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "service_profile": profile,
                        "artifacts": {
                            "linux-x64": {
                                "artifact": f"published-{profile}-linux",
                                "service_profile": profile,
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
            "note: native-static-libs: -liconv -framework Security "
            "-framework CoreFoundation -lSystem -lc -lm\n",
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def invoke(
        self,
        output: Path,
        target: str = "aarch64-apple-darwin",
        service_profile: str = "standard",
        mode: str = "candidate",
        artifact_table: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--repo-root",
                str(self.root),
                "--library",
                str(self.library),
                "--native-static-libs-log",
                str(self.native_libs),
                "--service-profile",
                service_profile,
                "--rust-target",
                target,
                "--output-dir",
                str(output),
                "--mode",
                mode,
                "--artifact-table",
                str(artifact_table or self.artifact_tables[service_profile]),
            ],
            check=False,
            text=True,
            capture_output=True,
        )

    def test_standard_archive_is_deterministic_and_self_describing(self) -> None:
        first = self.invoke(self.root / "first")
        second = self.invoke(self.root / "second")
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 0, second.stderr)
        first_result = json.loads(first.stdout)
        second_result = json.loads(second.stdout)
        self.assertEqual(
            first_result["archive_sha256"], second_result["archive_sha256"]
        )

        archive = Path(first_result["archive"])
        self.assertEqual(
            hashlib.sha256(archive.read_bytes()).hexdigest(),
            first_result["archive_sha256"],
        )
        with tarfile.open(archive, "r:gz") as contents:
            self.assertEqual(
                contents.getnames(),
                ["lib", "LICENSE", "lib/libopendal_mbt_native.a", "manifest.json"],
            )
            manifest = json.load(contents.extractfile("manifest.json"))

        self.assertEqual(manifest["binding_version"], "0.1.0")
        self.assertEqual(manifest["abi_version"], {"major": 1, "minor": 2, "patch": 3})
        self.assertEqual(manifest["opendal_version"], "0.58.1")
        self.assertEqual(manifest["service_profile"], "standard")
        self.assertEqual(manifest["services"], ["memory", "fs", "s3"])
        self.assertEqual(manifest["cargo_features"], ["profile-standard"])
        self.assertEqual(manifest["runtime_initialization"], "install_default")
        self.assertEqual(manifest["minimum_macos_version"], "11.0")
        self.assertEqual(
            manifest["system_link_flags"],
            [
                "-liconv",
                "-framework",
                "Security",
                "-framework",
                "CoreFoundation",
                "-lSystem",
                "-lc",
                "-lm",
            ],
        )

    def test_immutable_local_profile_remains_packageable(self) -> None:
        packaged = self.invoke(self.root / "local", service_profile="local")
        self.assertEqual(packaged.returncode, 0, packaged.stderr)
        result = json.loads(packaged.stdout)
        manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
        self.assertIn("-local-aarch64-apple-darwin", result["archive_name"])
        self.assertEqual(manifest["services"], ["memory", "fs"])
        self.assertEqual(manifest["rust_features"], ["blocking", "services-fs"])
        self.assertNotIn("cargo_features", manifest)

    def test_candidate_mode_updates_only_the_built_standard_host(self) -> None:
        packaged = self.invoke(self.root / "candidate")
        self.assertEqual(packaged.returncode, 0, packaged.stderr)
        result = json.loads(packaged.stdout)
        table = json.loads(
            Path(result["candidate_artifact_table"]).read_text(encoding="utf-8")
        )
        candidate = table["artifacts"]["darwin-arm64"]
        self.assertEqual(table["service_profile"], "standard")
        self.assertEqual(candidate["archive_name"], result["archive_name"])
        self.assertEqual(candidate["service_profile"], "standard")
        self.assertEqual(
            candidate["url"],
            f"https://candidate.invalid/{result['archive_name']}",
        )
        self.assertEqual(
            table["artifacts"]["linux-x64"]["artifact"],
            "published-standard-linux",
        )

    def test_unknown_target_is_rejected(self) -> None:
        result = self.invoke(self.root / "output", "x86_64-unknown-linux-gnu")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsupported Rust target", result.stderr)

    def test_version_mismatch_is_rejected(self) -> None:
        cargo = self.root / "native/rust/Cargo.toml"
        cargo.write_text(
            cargo.read_text().replace('version = "0.1.0"', 'version = "0.2.0"'),
            encoding="utf-8",
        )
        result = self.invoke(self.root / "output")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Moon and Rust binding versions differ", result.stderr)

    def test_missing_native_link_report_is_rejected(self) -> None:
        self.native_libs.write_text("cargo finished\n", encoding="utf-8")
        result = self.invoke(self.root / "output")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not report native-static-libs", result.stderr)

    def test_standard_macos_artifact_requires_declared_frameworks(self) -> None:
        self.native_libs.write_text(
            "note: native-static-libs: -liconv -framework Security -lSystem\n",
            encoding="utf-8",
        )
        result = self.invoke(self.root / "missing-framework")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "missing required frameworks: CoreFoundation",
            result.stderr,
        )

    def test_release_pin_must_match_and_cannot_use_candidate_url(self) -> None:
        first = self.invoke(self.root / "candidate-release")
        self.assertEqual(first.returncode, 0, first.stderr)
        result = json.loads(first.stdout)
        table = Path(result["candidate_artifact_table"])

        rejected = self.invoke(
            self.root / "candidate-url-release",
            mode="release",
            artifact_table=table,
        )
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("candidate artifact URL cannot be used", rejected.stderr)

        document = json.loads(table.read_text(encoding="utf-8"))
        document["artifacts"]["darwin-arm64"]["url"] = (
            f"https://example.invalid/{result['archive_name']}"
        )
        table.write_text(json.dumps(document), encoding="utf-8")
        verified = self.invoke(
            self.root / "verified",
            mode="release",
            artifact_table=table,
        )
        self.assertEqual(verified.returncode, 0, verified.stderr)

        document["artifacts"]["darwin-arm64"]["archive_sha256"] = "0" * 64
        table.write_text(json.dumps(document), encoding="utf-8")
        mismatched = self.invoke(
            self.root / "mismatched",
            mode="release",
            artifact_table=table,
        )
        self.assertNotEqual(mismatched.returncode, 0)
        self.assertIn("archive_sha256 does not match", mismatched.stderr)

    def test_profile_table_mismatch_is_rejected(self) -> None:
        result = self.invoke(
            self.root / "wrong-table",
            artifact_table=self.artifact_tables["local"],
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("service profile does not match", result.stderr)


if __name__ == "__main__":
    unittest.main()
