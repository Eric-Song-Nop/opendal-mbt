#!/usr/bin/env python3
"""Render a clean registry consumer for one exact published module version."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shutil
import sys


class ConsumerRenderError(Exception):
    """The registry consumer template cannot be rendered safely."""


def read_text(filename: Path) -> str:
    try:
        return filename.read_text(encoding="utf-8")
    except OSError as error:
        raise ConsumerRenderError(f"cannot read {filename}: {error}") from error


def module_name(repo_root: Path) -> str:
    source = read_text(repo_root / "moon.mod")
    match = re.search(r'^name\s*=\s*"([^"@]+)"', source, flags=re.MULTILINE)
    if match is None:
        raise ConsumerRenderError("cannot determine the published Moon module name")
    return match.group(1)


def validate_version(version: str) -> None:
    if not version or re.fullmatch(r"[0-9A-Za-z][0-9A-Za-z.+-]*", version) is None:
        raise ConsumerRenderError(f"invalid Moon module version: {version!r}")


def render_consumer(repo_root: Path, output_dir: Path, version: str) -> dict[str, str]:
    validate_version(version)
    template_dir = repo_root / "integration/consumer"
    template_module = template_dir / "moon.mod"
    if not template_dir.is_dir() or not template_module.is_file():
        raise ConsumerRenderError(f"registry consumer template is missing: {template_dir}")
    if output_dir.exists():
        raise ConsumerRenderError(f"output directory already exists: {output_dir}")

    published_module = module_name(repo_root)
    source = read_text(template_module)
    dependency = re.compile(
        rf'("{re.escape(published_module)})(?:@[^"\s]+)?(")'
    )
    rendered, count = dependency.subn(
        rf"\g<1>@{version}\g<2>",
        source,
    )
    if count != 1:
        raise ConsumerRenderError(
            "expected exactly one dependency on "
            f"{published_module} in {template_module}, found {count}"
        )

    try:
        shutil.copytree(template_dir, output_dir)
        (output_dir / "moon.mod").write_text(rendered, encoding="utf-8")
    except OSError as error:
        shutil.rmtree(output_dir, ignore_errors=True)
        raise ConsumerRenderError(f"cannot render registry consumer: {error}") from error
    return {
        "consumer": str(output_dir),
        "module": published_module,
        "version": version,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--version", required=True)
    return parser.parse_args()


def main() -> int:
    try:
        args = parse_args()
        result = render_consumer(
            args.repo_root.resolve(),
            args.output_dir.resolve(),
            args.version,
        )
    except ConsumerRenderError as error:
        print(f"render-registry-consumer: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
