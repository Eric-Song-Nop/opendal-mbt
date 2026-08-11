#!/usr/bin/env python3

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import unittest


ROOT = Path(__file__).resolve().parent.parent
LOCAL_FEATURES = ["blocking", "services-fs"]
STANDARD_FEATURES = [
    "blocking",
    "services-fs",
    "services-s3",
    "http-transport-reqwest",
    "http-transport-reqwest-rustls",
    "executors-tokio",
]
STANDARD_TARGETS = {
    "aarch64-apple-darwin": {
        "host_key": "darwin-arm64",
        "minimum_macos_version": "11.0",
        "required_frameworks": ["Security", "CoreFoundation"],
    },
    "aarch64-unknown-linux-gnu": {
        "host_key": "linux-arm64",
        "minimum_glibc_version": "2.35",
    },
    "x86_64-unknown-linux-gnu": {
        "host_key": "linux-x64",
        "minimum_glibc_version": "2.35",
    },
}


def read_json(relative: str) -> dict:
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


class DistributionProfilesTest(unittest.TestCase):
    def test_local_profile_remains_the_immutable_v0_1_contract(self) -> None:
        profile = read_json("native/distribution-profile.json")
        self.assertEqual(profile["schema_version"], 1)
        self.assertEqual(profile["artifact_revision"], "r1")
        self.assertEqual(profile["service_profile"], "local")
        self.assertEqual(profile["services"], ["memory", "fs"])
        self.assertEqual(profile["rust_features"], LOCAL_FEATURES)

        table = read_json("native/artifacts.json")
        self.assertEqual(table["schema_version"], 1)
        self.assertEqual(table["service_profile"], "local")
        self.assertEqual(sorted(table["artifacts"]), ["darwin-arm64", "linux-x64"])
        for artifact in table["artifacts"].values():
            self.assertEqual(artifact["binding_version"], "0.1.0")
            self.assertEqual(artifact["artifact_revision"], "r1")
            self.assertEqual(artifact["service_profile"], "local")
            self.assertEqual(artifact["services"], ["memory", "fs"])
            self.assertEqual(artifact["rust_features"], LOCAL_FEATURES)

    def test_standard_profile_is_one_memory_fs_s3_artifact(self) -> None:
        profile = read_json("native/distribution-profiles/standard.json")
        self.assertEqual(profile["schema_version"], 1)
        self.assertEqual(profile["service_profile"], "standard")
        self.assertEqual(profile["services"], ["memory", "fs", "s3"])
        self.assertEqual(profile["rust_features"], STANDARD_FEATURES)
        self.assertEqual(profile["cargo_features"], ["profile-standard"])
        self.assertEqual(profile["runtime_initialization"], "install_default")
        self.assertEqual(profile["targets"], STANDARD_TARGETS)
        self.assertEqual(
            {target["host_key"] for target in profile["targets"].values()},
            {"darwin-arm64", "linux-arm64", "linux-x64"},
        )

        table = read_json("native/artifacts-standard.json")
        self.assertEqual(table["schema_version"], 1)
        self.assertEqual(table["service_profile"], "standard")
        for artifact in table["artifacts"].values():
            self.assertEqual(artifact["service_profile"], "standard")
            self.assertEqual(artifact["services"], ["memory", "fs", "s3"])
            self.assertEqual(artifact["rust_features"], STANDARD_FEATURES)

    def test_cargo_profiles_are_explicit_and_standard_is_the_source_default(self) -> None:
        result = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
            cwd=ROOT,
            check=True,
            text=True,
            capture_output=True,
        )
        package = json.loads(result.stdout)["packages"][0]
        dependency = next(
            value for value in package["dependencies"] if value["name"] == "opendal"
        )
        self.assertEqual(dependency["req"], "=0.58.1")
        self.assertFalse(dependency["uses_default_features"])
        self.assertEqual(dependency["features"], [])

        features = package["features"]
        self.assertEqual(features["default"], ["profile-standard"])
        self.assertEqual(
            features["profile-local"],
            ["opendal/blocking", "opendal/services-fs"],
        )
        self.assertEqual(
            features["profile-standard"],
            [
                "profile-local",
                "opendal/services-s3",
                "opendal/http-transport-reqwest",
                "opendal/http-transport-reqwest-rustls",
                "opendal/executors-tokio",
            ],
        )

        lockfile = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
        self.assertIn('name = "opendal-service-s3"', lockfile)
        self.assertIn('name = "opendal-http-transport-reqwest"', lockfile)

    def test_published_selection_is_internal_and_stays_local_until_api_activation(self) -> None:
        selection = read_json("native/artifact-selection.json")
        self.assertEqual(
            selection,
            {
                "schema_version": 1,
                "service_profile": "local",
                "artifact_table": "artifacts.json",
            },
        )
        resolver = (ROOT / "build.js").read_text(encoding="utf-8")
        self.assertNotIn("OPENDAL_MBT_PROFILE", resolver)


if __name__ == "__main__":
    unittest.main()
