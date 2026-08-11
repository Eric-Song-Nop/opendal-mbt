#!/usr/bin/env python3
"""Build one deterministic opendal-mbt native release archive."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shlex
import sys
import tarfile
import tempfile
from typing import Any


STATIC_LIBRARY = "libopendal_mbt_native.a"
STATIC_LIBRARY_PATH = f"lib/{STATIC_LIBRARY}"
CANDIDATE_URL_ORIGIN = "https://candidate.invalid"
PROFILE_FILES = {
    "local": Path("native/distribution-profile.json"),
    "standard": Path("native/distribution-profiles/standard.json"),
}


class ArtifactError(Exception):
    """A release input does not satisfy the distribution contract."""


def sha256_file(filename: Path) -> str:
    digest = hashlib.sha256()
    with filename.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_regular_file(filename: Path, description: str) -> None:
    try:
        stat = filename.lstat()
    except FileNotFoundError as error:
        raise ArtifactError(f"{description} does not exist: {filename}") from error
    if not filename.is_file() or filename.is_symlink():
        raise ArtifactError(f"{description} must be a regular file: {filename}")
    if stat.st_size == 0:
        raise ArtifactError(f"{description} must not be empty: {filename}")


def read_json(filename: Path) -> dict[str, Any]:
    try:
        value = json.loads(filename.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ArtifactError(f"cannot read JSON from {filename}: {error}") from error
    if not isinstance(value, dict):
        raise ArtifactError(f"expected a JSON object in {filename}")
    return value


def capture(pattern: str, source: str, description: str) -> str:
    match = re.search(pattern, source, flags=re.MULTILINE)
    if match is None:
        raise ArtifactError(f"cannot determine {description}")
    return match.group(1)


def source_versions(repo_root: Path) -> dict[str, Any]:
    moon_mod = (repo_root / "moon.mod").read_text(encoding="utf-8")
    cargo_toml = (repo_root / "native/rust/Cargo.toml").read_text(
        encoding="utf-8"
    )
    abi_header = (repo_root / "native/include/opendal_mbt.h").read_text(
        encoding="utf-8"
    )

    binding_version = capture(
        r'^version\s*=\s*"([^"]+)"', moon_mod, "Moon binding version"
    )
    rust_binding_version = capture(
        r'^version\s*=\s*"([^"]+)"', cargo_toml, "Rust binding version"
    )
    if binding_version != rust_binding_version:
        raise ArtifactError(
            "Moon and Rust binding versions differ: "
            f"{binding_version} != {rust_binding_version}"
        )

    return {
        "binding_version": binding_version,
        "opendal_version": capture(
            r'^opendal\s*=\s*\{[^\n]*version\s*=\s*"=([^"]+)"',
            cargo_toml,
            "pinned OpenDAL version",
        ),
        "rust_version": capture(
            r'^rust-version\s*=\s*"([^"]+)"', cargo_toml, "Rust version"
        ),
        "abi_version": {
            "major": int(
                capture(
                    r"^#define OPENDAL_MBT_ABI_V1_MAJOR UINT32_C\((\d+)\)",
                    abi_header,
                    "ABI major version",
                )
            ),
            "minor": int(
                capture(
                    r"^#define OPENDAL_MBT_ABI_V1_MINOR UINT32_C\((\d+)\)",
                    abi_header,
                    "ABI minor version",
                )
            ),
            "patch": int(
                capture(
                    r"^#define OPENDAL_MBT_ABI_V1_PATCH UINT32_C\((\d+)\)",
                    abi_header,
                    "ABI patch version",
                )
            ),
        },
    }


def native_static_libraries(filename: Path) -> list[str]:
    require_regular_file(filename, "rustc native-static-libs log")
    reported = []
    for line in filename.read_text(encoding="utf-8").splitlines():
        marker = "native-static-libs:"
        if marker in line:
            reported.append(line.split(marker, maxsplit=1)[1].strip())
    if not reported or not reported[-1]:
        raise ArtifactError("rustc did not report native-static-libs")

    try:
        flags = shlex.split(reported[-1], posix=True)
    except ValueError as error:
        raise ArtifactError(f"cannot parse native-static-libs: {error}") from error
    if not flags or any("\x00" in flag or "\n" in flag for flag in flags):
        raise ArtifactError("native-static-libs contains an invalid flag")
    return flags


def require_target_link_flags(target: dict[str, Any], flags: list[str]) -> None:
    required_frameworks = target.get("required_frameworks", [])
    if not isinstance(required_frameworks, list) or any(
        not isinstance(framework, str) or not framework
        for framework in required_frameworks
    ):
        raise ArtifactError("required_frameworks must contain framework names")
    if len(set(required_frameworks)) != len(required_frameworks):
        raise ArtifactError("required_frameworks must not contain duplicates")
    available_frameworks = {
        flags[index + 1]
        for index, flag in enumerate(flags[:-1])
        if flag == "-framework"
    }
    missing = [
        framework
        for framework in required_frameworks
        if framework not in available_frameworks
    ]
    if missing:
        raise ArtifactError(
            "rustc native-static-libs is missing required frameworks: "
            + ", ".join(missing)
        )


def load_profile(
    repo_root: Path,
    service_profile: str,
    rust_target: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    profile_file = PROFILE_FILES.get(service_profile)
    if profile_file is None:
        supported = ", ".join(sorted(PROFILE_FILES))
        raise ArtifactError(
            f"unknown service profile {service_profile}; supported profiles: {supported}"
        )
    profile = read_json(repo_root / profile_file)
    if profile.get("schema_version") != 1:
        raise ArtifactError("unsupported distribution profile schema")
    if profile.get("service_profile") != service_profile:
        raise ArtifactError(
            f"distribution profile identity does not match {service_profile}"
        )
    if service_profile == "local":
        if profile.get("services") != ["memory", "fs"]:
            raise ArtifactError("the immutable local profile must contain memory and fs")
        if profile.get("rust_features") != ["blocking", "services-fs"]:
            raise ArtifactError("the immutable local profile Rust features changed")
    elif service_profile == "standard":
        if profile.get("services") != ["memory", "fs", "s3"]:
            raise ArtifactError("the standard profile must contain memory, fs, and s3")
        expected_features = [
            "blocking",
            "services-fs",
            "services-s3",
            "http-transport-reqwest",
            "http-transport-reqwest-rustls",
            "executors-tokio",
            "layers-retry",
            "layers-timeout",
        ]
        if profile.get("rust_features") != expected_features:
            raise ArtifactError("the standard profile Rust features are inconsistent")
        if profile.get("cargo_features") != ["profile-standard"]:
            raise ArtifactError("the standard profile Cargo feature is inconsistent")
        if profile.get("runtime_initialization") != "install_default":
            raise ArtifactError("the standard profile must use install_default")
    revision = profile.get("artifact_revision")
    if not isinstance(revision, str) or re.fullmatch(r"r[1-9][0-9]*", revision) is None:
        raise ArtifactError("artifact_revision must have the form rN")

    targets = profile.get("targets")
    if not isinstance(targets, dict) or rust_target not in targets:
        supported = ", ".join(sorted(targets)) if isinstance(targets, dict) else ""
        raise ArtifactError(
            f"unsupported Rust target {rust_target}; supported targets: {supported}"
        )
    target = targets[rust_target]
    if not isinstance(target, dict):
        raise ArtifactError(f"invalid target profile for {rust_target}")
    floors = [
        key
        for key in ("minimum_macos_version", "minimum_glibc_version")
        if key in target
    ]
    if len(floors) != 1:
        raise ArtifactError(f"{rust_target} must declare one compatibility floor")
    if "required_frameworks" in target and "minimum_macos_version" not in target:
        raise ArtifactError("only a macOS target may require system frameworks")
    return profile, target


def manifest_for(
    repo_root: Path,
    library: Path,
    native_libs_log: Path,
    service_profile: str,
    rust_target: str,
) -> tuple[str, dict[str, Any]]:
    profile, target = load_profile(repo_root, service_profile, rust_target)
    versions = source_versions(repo_root)
    require_regular_file(library, "native static library")
    if library.name != STATIC_LIBRARY:
        raise ArtifactError(
            f"native static library must be named {STATIC_LIBRARY}: {library}"
        )

    artifact = "-".join(
        [
            "opendal-mbt",
            versions["binding_version"],
            profile["artifact_revision"],
            profile["service_profile"],
            rust_target,
        ]
    )
    system_link_flags = native_static_libraries(native_libs_log)
    require_target_link_flags(target, system_link_flags)
    manifest = {
        "schema_version": 1,
        "artifact": artifact,
        "artifact_revision": profile["artifact_revision"],
        "binding_version": versions["binding_version"],
        "abi_version": versions["abi_version"],
        "opendal_version": versions["opendal_version"],
        "rust_version": versions["rust_version"],
        "service_profile": profile["service_profile"],
        "services": profile["services"],
        "rust_features": profile["rust_features"],
        "rust_target": rust_target,
        "host_key": target["host_key"],
        "static_library": STATIC_LIBRARY_PATH,
        "static_library_size": library.stat().st_size,
        "static_library_sha256": sha256_file(library),
        "system_link_flags": system_link_flags,
    }
    for key in ("cargo_features", "runtime_initialization"):
        if key in profile:
            manifest[key] = profile[key]
    for key in ("minimum_macos_version", "minimum_glibc_version"):
        if key in target:
            manifest[key] = target[key]
    return artifact, manifest


def tar_info(name: str, size: int, mode: int) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.size = size
    info.mode = mode
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    return info


def add_directory(archive: tarfile.TarFile, name: str) -> None:
    info = tar_info(name, 0, 0o755)
    info.type = tarfile.DIRTYPE
    archive.addfile(info)


def add_bytes(archive: tarfile.TarFile, name: str, value: bytes) -> None:
    archive.addfile(tar_info(name, len(value), 0o644), io.BytesIO(value))


def add_file(archive: tarfile.TarFile, name: str, source: Path) -> None:
    with source.open("rb") as contents:
        archive.addfile(tar_info(name, source.stat().st_size, 0o644), contents)


def write_archive(
    destination: Path,
    license_file: Path,
    library: Path,
    manifest: dict[str, Any],
) -> None:
    manifest_bytes = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as raw:
        temporary = Path(raw.name)
        try:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(
                    mode="w", fileobj=compressed, format=tarfile.USTAR_FORMAT
                ) as archive:
                    add_directory(archive, "lib")
                    add_file(archive, "LICENSE", license_file)
                    add_file(archive, STATIC_LIBRARY_PATH, library)
                    add_bytes(archive, "manifest.json", manifest_bytes)
            os.replace(temporary, destination)
        except BaseException:
            temporary.unlink(missing_ok=True)
            raise


def load_artifact_table(table_file: Path) -> dict[str, Any]:
    table = read_json(table_file)
    if table.get("schema_version") != 1 or not isinstance(table.get("artifacts"), dict):
        raise ArtifactError("unsupported pinned artifact table")
    return table


def artifact_record(result: dict[str, Any], url: str) -> dict[str, Any]:
    manifest = read_json(Path(result["manifest"]))
    return {
        **{key: value for key, value in manifest.items() if key != "schema_version"},
        "archive_name": result["archive_name"],
        "archive_size": result["archive_size"],
        "archive_sha256": result["archive_sha256"],
        "url": url,
    }


def verify_pinned_artifact(table_file: Path, result: dict[str, Any]) -> None:
    table = load_artifact_table(table_file)
    manifest = read_json(Path(result["manifest"]))
    table_profile = table.get("service_profile")
    if table_profile is not None and table_profile != manifest.get("service_profile"):
        raise ArtifactError("artifact table service profile does not match the build")
    host_key = manifest.get("host_key")
    pinned = table["artifacts"].get(host_key)
    if not isinstance(pinned, dict):
        raise ArtifactError(f"no pinned artifact exists for {host_key}")
    for key, value in manifest.items():
        if key == "schema_version":
            continue
        if pinned.get(key) != value:
            raise ArtifactError(f"pinned artifact field {key} does not match the build")
    for key in ("archive_name", "archive_size", "archive_sha256"):
        if pinned.get(key) != result[key]:
            raise ArtifactError(f"pinned artifact field {key} does not match the build")
    expected_url_suffix = f"/{result['archive_name']}"
    if not isinstance(pinned.get("url"), str) or not pinned["url"].endswith(
        expected_url_suffix
    ):
        raise ArtifactError("pinned artifact URL does not match the archive name")
    if pinned["url"].startswith(f"{CANDIDATE_URL_ORIGIN}/"):
        raise ArtifactError("candidate artifact URL cannot be used for a release")


def write_candidate_artifact_table(
    table_file: Path,
    result: dict[str, Any],
) -> Path:
    table = load_artifact_table(table_file)
    manifest = read_json(Path(result["manifest"]))
    service_profile = manifest.get("service_profile")
    table_profile = table.get("service_profile")
    if table_profile is not None and table_profile != service_profile:
        raise ArtifactError("artifact table service profile does not match the build")
    table["service_profile"] = service_profile
    host_key = manifest.get("host_key")
    if not isinstance(host_key, str) or not host_key:
        raise ArtifactError("candidate artifact manifest has no host_key")
    candidate_url = f"{CANDIDATE_URL_ORIGIN}/{result['archive_name']}"
    table["artifacts"][host_key] = artifact_record(result, candidate_url)

    manifest_file = Path(result["manifest"])
    candidate_file = manifest_file.with_name(
        f"{manifest_file.name.removesuffix('.manifest.json')}"
        ".candidate-artifacts.json"
    )
    candidate_file.write_text(
        json.dumps(table, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return candidate_file


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--native-static-libs-log", type=Path, required=True)
    parser.add_argument(
        "--service-profile",
        choices=tuple(PROFILE_FILES),
        required=True,
    )
    parser.add_argument("--rust-target", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--mode",
        choices=("candidate", "release"),
        required=True,
        help="emit a staged candidate table or verify the committed release table",
    )
    parser.add_argument(
        "--artifact-table",
        type=Path,
        required=True,
        help="profile-specific artifact table used as the candidate base or release pin",
    )
    return parser.parse_args()


def run(args: argparse.Namespace) -> dict[str, Any]:
    repo_root = args.repo_root.resolve()
    license_file = repo_root / "LICENSE"
    require_regular_file(license_file, "license")
    artifact, manifest = manifest_for(
        repo_root,
        args.library.resolve(),
        args.native_static_libs_log.resolve(),
        args.service_profile,
        args.rust_target,
    )
    archive = args.output_dir.resolve() / f"{artifact}.tar.gz"
    write_archive(archive, license_file, args.library.resolve(), manifest)
    archive_sha256 = sha256_file(archive)
    checksum_file = archive.with_suffix(archive.suffix + ".sha256")
    checksum_file.write_text(
        f"{archive_sha256}  {archive.name}\n", encoding="utf-8"
    )
    manifest_file = args.output_dir.resolve() / f"{artifact}.manifest.json"
    manifest_file.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return {
        "archive": str(archive),
        "archive_name": archive.name,
        "archive_sha256": archive_sha256,
        "archive_size": archive.stat().st_size,
        "checksum": str(checksum_file),
        "manifest": str(manifest_file),
    }


def main() -> int:
    try:
        args = parse_args()
        result = run(args)
        if args.mode == "release":
            verify_pinned_artifact(args.artifact_table.resolve(), result)
        else:
            candidate_table = write_candidate_artifact_table(
                args.artifact_table.resolve(), result
            )
            result["candidate_artifact_table"] = str(candidate_table)
    except (ArtifactError, OSError) as error:
        print(f"package-native-artifact: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
