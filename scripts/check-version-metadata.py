#!/usr/bin/env python3
"""Verify that source, native, lockfile, and consumer versions stay aligned."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys


class VersionContractError(Exception):
    """Repository version metadata is missing or inconsistent."""


def read_text(filename: Path) -> str:
    try:
        return filename.read_text(encoding="utf-8")
    except OSError as error:
        raise VersionContractError(f"cannot read {filename}: {error}") from error


def capture(pattern: str, source: str, description: str) -> str:
    match = re.search(pattern, source, flags=re.MULTILINE)
    if match is None:
        raise VersionContractError(f"cannot determine {description}")
    return match.group(1)


def cargo_package_version(filename: Path) -> str:
    source = read_text(filename)
    package = re.search(
        r"^\[package\]\s*$\n(?P<body>.*?)(?=^\[|\Z)",
        source,
        flags=re.MULTILINE | re.DOTALL,
    )
    if package is None:
        raise VersionContractError(f"cannot find [package] in {filename}")
    return capture(
        r'^version\s*=\s*"([^"]+)"',
        package.group("body"),
        f"package version in {filename}",
    )


def locked_package_versions(filename: Path, package_name: str) -> list[str]:
    source = read_text(filename)
    versions = []
    for package in re.finditer(
        r"^\[\[package\]\]\s*$\n(?P<body>.*?)(?=^\[\[package\]\]|\Z)",
        source,
        flags=re.MULTILINE | re.DOTALL,
    ):
        body = package.group("body")
        name = re.search(r'^name\s*=\s*"([^"]+)"', body, flags=re.MULTILINE)
        version = re.search(r'^version\s*=\s*"([^"]+)"', body, flags=re.MULTILINE)
        if name is not None and name.group(1) == package_name:
            if version is None:
                raise VersionContractError(
                    f"{package_name} has no version in {filename}"
                )
            versions.append(version.group(1))
    return versions


def consumer_dependency_version(filename: Path, module_name: str) -> str:
    source = read_text(filename)
    versions = [
        version
        for name, version in re.findall(r'"([^"@]+)@([^"]+)"', source)
        if name == module_name
    ]
    if len(versions) != 1:
        raise VersionContractError(
            f"expected exactly one versioned {module_name} dependency in {filename}"
        )
    return versions[0]


def check_repository(repo_root: Path) -> tuple[str, str]:
    moon_mod = read_text(repo_root / "moon.mod")
    module_name = capture(
        r'^name\s*=\s*"([^"]+)"', moon_mod, "Moon module name"
    )
    binding_version = capture(
        r'^version\s*=\s*"([^"]+)"', moon_mod, "Moon module version"
    )
    rust_version = cargo_package_version(repo_root / "native/rust/Cargo.toml")
    if rust_version != binding_version:
        raise VersionContractError(
            "Moon and Rust binding versions differ: "
            f"{binding_version} != {rust_version}"
        )

    locked_versions = locked_package_versions(
        repo_root / "Cargo.lock", "opendal-mbt-native"
    )
    if locked_versions != [binding_version]:
        rendered = ", ".join(locked_versions) if locked_versions else "missing"
        raise VersionContractError(
            "Cargo.lock opendal-mbt-native version differs from moon.mod: "
            f"{rendered} != {binding_version}"
        )

    consumer_version = consumer_dependency_version(
        repo_root / "integration/consumer/moon.mod", module_name
    )
    if consumer_version != binding_version:
        raise VersionContractError(
            "integration consumer dependency differs from moon.mod: "
            f"{consumer_version} != {binding_version}"
        )
    return module_name, binding_version


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    return parser.parse_args()


def main() -> int:
    try:
        args = parse_args()
        module_name, binding_version = check_repository(args.repo_root.resolve())
    except VersionContractError as error:
        print(f"check-version-metadata: {error}", file=sys.stderr)
        return 1
    print(f"version metadata is consistent: {module_name}@{binding_version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
