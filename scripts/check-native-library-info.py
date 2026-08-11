#!/usr/bin/env python3
"""Link a release archive and verify its runtime library_info identity."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
from typing import Any


class LibraryInfoError(Exception):
    """The native release identity is missing or inconsistent."""


def read_manifest(filename: Path) -> dict[str, Any]:
    try:
        value = json.loads(filename.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise LibraryInfoError(f"cannot read artifact manifest {filename}: {error}")
    if not isinstance(value, dict) or value.get("schema_version") != 1:
        raise LibraryInfoError("unsupported artifact manifest")
    return value


def validate_manifest(
    manifest: dict[str, Any], expected_version: str, expected_profile: str
) -> list[str]:
    if manifest.get("binding_version") != expected_version:
        raise LibraryInfoError(
            "artifact binding version differs from the release: "
            f"{manifest.get('binding_version')!r} != {expected_version!r}"
        )
    if manifest.get("service_profile") != expected_profile:
        raise LibraryInfoError(
            "artifact service profile differs from the release: "
            f"{manifest.get('service_profile')!r} != {expected_profile!r}"
        )
    flags = manifest.get("system_link_flags")
    if not isinstance(flags, list) or not flags or any(
        not isinstance(flag, str) or not flag or "\x00" in flag or "\n" in flag
        for flag in flags
    ):
        raise LibraryInfoError("artifact system_link_flags are missing or invalid")
    return flags


def require_file(filename: Path, description: str) -> None:
    if not filename.is_file() or filename.is_symlink():
        raise LibraryInfoError(f"{description} must be a regular file: {filename}")


def run_probe(
    repo_root: Path,
    library: Path,
    manifest_file: Path,
    expected_version: str,
    expected_profile: str,
) -> None:
    require_file(library, "native static library")
    probe = repo_root / "tests/c/library_info_probe.c"
    require_file(probe, "library_info probe")
    flags = validate_manifest(
        read_manifest(manifest_file), expected_version, expected_profile
    )
    compiler = shlex.split(os.environ.get("CC", "cc"))
    if not compiler:
        raise LibraryInfoError("CC does not name a C compiler")

    with tempfile.TemporaryDirectory(prefix="opendal-mbt-library-info-") as raw:
        executable = Path(raw) / "library-info-probe"
        compile_result = subprocess.run(
            [
                *compiler,
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wpedantic",
                str(probe),
                str(library),
                *flags,
                "-o",
                str(executable),
            ],
            cwd=repo_root,
            check=False,
            text=True,
            capture_output=True,
        )
        if compile_result.returncode != 0:
            raise LibraryInfoError(
                "cannot link library_info probe:\n"
                + compile_result.stdout
                + compile_result.stderr
            )
        probe_result = subprocess.run(
            [str(executable), expected_version, expected_profile],
            cwd=repo_root,
            check=False,
            text=True,
            capture_output=True,
        )
        if probe_result.returncode != 0:
            raise LibraryInfoError(
                "native library_info does not match the release:\n"
                + probe_result.stdout
                + probe_result.stderr
            )
        print(probe_result.stdout, end="")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--expected-version", required=True)
    parser.add_argument("--expected-profile", required=True)
    return parser.parse_args()


def main() -> int:
    try:
        args = parse_args()
        run_probe(
            args.repo_root.resolve(),
            args.library.resolve(),
            args.manifest.resolve(),
            args.expected_version,
            args.expected_profile,
        )
    except LibraryInfoError as error:
        print(f"check-native-library-info: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
