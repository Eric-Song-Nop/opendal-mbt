#!/usr/bin/env python3
"""Check the semantic core shared by the native and browser Wasm facades."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


DECLARATION_RE = re.compile(
    r"^pub(?:\(all\))? (?:suberror|struct|enum|type) ([A-Za-z][A-Za-z0-9_]*)"
)


def split_top_level(value: str) -> list[str]:
    """Split a comma-separated MoonBit signature without splitting nested types."""
    parts: list[str] = []
    start = 0
    depths = {"(": 0, "[": 0, "{": 0}
    closing = {")": "(", "]": "[", "}": "{"}
    for index, char in enumerate(value):
        if char in depths:
            depths[char] += 1
        elif char in closing:
            opener = closing[char]
            depths[opener] -= 1
            if depths[opener] < 0:
                raise ValueError(f"unbalanced signature: {value}")
        elif char == "," and all(depth == 0 for depth in depths.values()):
            parts.append(value[start:index].strip())
            start = index + 1
    if any(depth != 0 for depth in depths.values()):
        raise ValueError(f"unbalanced signature: {value}")
    tail = value[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def public_lines(text: str) -> list[str]:
    return [line.strip() for line in text.splitlines() if line.startswith("pub ")]


def find_public_line(text: str, prefix: str) -> str | None:
    return next((line for line in public_lines(text) if line.startswith(prefix)), None)


def parse_native_operation(text: str, name: str) -> tuple[list[str], str] | None:
    prefix = f"pub async fn AsyncOperator::{name}("
    line = find_public_line(text, prefix)
    if line is None:
        return None
    match = re.fullmatch(
        rf"pub async fn AsyncOperator::{re.escape(name)}\((.*)\) -> (.+)", line
    )
    if match is None:
        raise ValueError(f"cannot parse native declaration: {line}")
    parameters = split_top_level(match.group(1))
    if not parameters or parameters[0] != "Self":
        raise ValueError(f"native declaration must start with Self: {line}")
    return parameters[1:], match.group(2)


def parse_wasm_operation(text: str, name: str) -> tuple[list[str], str] | None:
    prefix = f"pub fn AsyncOperator::{name}_callback("
    line = find_public_line(text, prefix)
    if line is None:
        return None
    match = re.fullmatch(
        rf"pub fn AsyncOperator::{re.escape(name)}_callback\((.*)\) -> Task raise OpenDalError",
        line,
    )
    if match is None:
        raise ValueError(f"cannot parse Wasm declaration: {line}")
    parameters = split_top_level(match.group(1))
    if not parameters or parameters[0] != "Self":
        raise ValueError(f"Wasm declaration must start with Self: {line}")
    callback_indexes = [
        index for index, parameter in enumerate(parameters) if parameter.startswith("callback~")
    ]
    if callback_indexes != [len(parameters) - 1]:
        raise ValueError(f"Wasm callback must be the final required label: {line}")
    callback = parameters[-1]
    callback_match = re.fullmatch(
        r"callback~ : \(Result\[(.+), OpenDalError\]\) -> Unit", callback
    )
    if callback_match is None:
        raise ValueError(f"cannot parse Wasm callback result: {line}")
    return parameters[1:-1], callback_match.group(1)


def declaration_shape(text: str, name: str) -> tuple[str, tuple[str, ...]] | None:
    lines = text.splitlines()
    for index, line in enumerate(lines):
        match = DECLARATION_RE.match(line)
        if match is None or match.group(1) != name:
            continue
        kind = " ".join(line.split()[1:2])
        if "{" not in line:
            return kind, ()
        members: list[str] = []
        for member in lines[index + 1 :]:
            stripped = member.strip()
            if stripped.startswith("}"):
                return kind, tuple(members)
            if stripped and not stripped.startswith("//"):
                members.append(stripped)
        raise ValueError(f"unterminated declaration for {name}")
    return None


def owned_methods(text: str, owner: str) -> tuple[str, ...]:
    prefixes = (f"pub fn {owner}::", f"pub impl Show for {owner}")
    return tuple(sorted(line for line in public_lines(text) if line.startswith(prefixes)))


def operation_variants(text: str) -> set[str]:
    shape = declaration_shape(text, "Operation")
    if shape is None:
        return set()
    return {member.split("(", 1)[0].strip() for member in shape[1]}


def verify(native_text: str, wasm_text: str, contract: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if contract.get("schema") != 1:
        return [f"unsupported cross-target contract schema: {contract.get('schema')!r}"]

    native_variants = operation_variants(native_text)
    wasm_variants = operation_variants(wasm_text)
    for name, expected in contract["operations"].items():
        try:
            native = parse_native_operation(native_text, name)
            wasm = parse_wasm_operation(wasm_text, name)
        except ValueError as error:
            errors.append(str(error))
            continue
        expected_shape = (expected["arguments"], expected["result"])
        if native is None:
            errors.append(f"native AsyncOperator::{name} is missing")
        elif native != (expected_shape[0], expected_shape[1]):
            errors.append(
                f"native AsyncOperator::{name} has {native!r}; expected {expected_shape!r}"
            )
        if wasm is None:
            errors.append(f"Wasm AsyncOperator::{name}_callback is missing")
        elif wasm != (expected_shape[0], expected_shape[1]):
            errors.append(
                f"Wasm AsyncOperator::{name}_callback has {wasm!r}; "
                f"expected {expected_shape!r}"
            )
        variant = expected["error_operation"]
        if variant not in native_variants:
            errors.append(f"native Operation is missing {variant}")
        if variant not in wasm_variants:
            errors.append(f"Wasm Operation is missing {variant}")

    for name in contract["shared_types"]:
        native_shape = declaration_shape(native_text, name)
        wasm_shape = declaration_shape(wasm_text, name)
        if native_shape is None:
            errors.append(f"native shared type {name} is missing")
        elif wasm_shape is None:
            errors.append(f"Wasm shared type {name} is missing")
        elif native_shape != wasm_shape:
            errors.append(
                f"shared type {name} drifted: native={native_shape!r}, "
                f"Wasm={wasm_shape!r}"
            )

    for owner in contract["shared_method_owners"]:
        native_methods = owned_methods(native_text, owner)
        wasm_methods = owned_methods(wasm_text, owner)
        if native_methods != wasm_methods:
            errors.append(
                f"shared {owner} methods drifted: native={native_methods!r}, "
                f"Wasm={wasm_methods!r}"
            )

    native_public = set(public_lines(native_text))
    wasm_public = set(public_lines(wasm_text))
    for declaration in contract["shared_operator_methods"]:
        if declaration not in native_public:
            errors.append(f"native common declaration is missing: {declaration}")
        if declaration not in wasm_public:
            errors.append(f"Wasm common declaration is missing: {declaration}")
    return errors


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", type=Path, default=Path("src/pkg.generated.mbti"))
    parser.add_argument("--wasm", type=Path, default=Path("src/wasm/pkg.generated.mbti"))
    parser.add_argument(
        "--contract",
        type=Path,
        default=Path("docs/design/cross-target-api.json"),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    contract = json.loads(args.contract.read_text(encoding="utf-8"))
    errors = verify(
        args.native.read_text(encoding="utf-8"),
        args.wasm.read_text(encoding="utf-8"),
        contract,
    )
    if errors:
        print("cross-target API contract failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print("cross-target API contract passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
