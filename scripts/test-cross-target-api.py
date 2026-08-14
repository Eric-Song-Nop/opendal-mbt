#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name("check-cross-target-api.py")
SPEC = importlib.util.spec_from_file_location("cross_target_api", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def fixture(native_result: str = "Bytes", wasm_result: str = "Bytes") -> tuple[str, str]:
    shared = """\
pub enum Operation {
  Read
}
pub struct Metadata {
  content_length : UInt64
}
pub fn Metadata::is_file(Self) -> Bool
pub fn Operator::as_async(Self) -> AsyncOperator
"""
    native = shared + f"pub async fn AsyncOperator::read(Self, StringView, range? : ByteRange) -> {native_result}\n"
    wasm = shared + (
        "pub fn AsyncOperator::read_callback(Self, StringView, range? : ByteRange, "
        f"callback~ : (Result[{wasm_result}, OpenDalError]) -> Unit) -> Task raise OpenDalError\n"
    )
    return native, wasm


CONTRACT = {
    "schema": 1,
    "operations": {
        "read": {
            "arguments": ["StringView", "range? : ByteRange"],
            "result": "Bytes",
            "error_operation": "Read",
        }
    },
    "shared_types": ["Metadata", "Operation"],
    "shared_method_owners": ["Metadata"],
    "shared_operator_methods": ["pub fn Operator::as_async(Self) -> AsyncOperator"],
}


class CrossTargetApiTest(unittest.TestCase):
    def test_matching_semantic_signatures_pass(self) -> None:
        native, wasm = fixture()
        self.assertEqual(MODULE.verify(native, wasm, CONTRACT), [])

    def test_native_result_drift_is_rejected(self) -> None:
        native, wasm = fixture(native_result="Metadata")
        self.assertTrue(
            any("native AsyncOperator::read" in error for error in MODULE.verify(native, wasm, CONTRACT))
        )

    def test_wasm_result_drift_is_rejected(self) -> None:
        native, wasm = fixture(wasm_result="Metadata")
        self.assertTrue(
            any("Wasm AsyncOperator::read_callback" in error for error in MODULE.verify(native, wasm, CONTRACT))
        )

    def test_callback_must_be_last(self) -> None:
        native, wasm = fixture()
        wasm = wasm.replace(
            "range? : ByteRange, callback~ :",
            "callback~ : (Result[Bytes, OpenDalError]) -> Unit, range? : ByteRange, ignored~ :",
        )
        errors = MODULE.verify(native, wasm, CONTRACT)
        self.assertTrue(any("callback" in error for error in errors))

    def test_shared_type_drift_is_rejected(self) -> None:
        native, wasm = fixture()
        wasm = wasm.replace("content_length : UInt64", "content_length : Int64")
        self.assertTrue(
            any("shared type Metadata drifted" in error for error in MODULE.verify(native, wasm, CONTRACT))
        )

    def test_missing_operation_variant_is_rejected(self) -> None:
        native, wasm = fixture()
        wasm = wasm.replace("  Read\n", "  Write\n")
        self.assertIn("Wasm Operation is missing Read", MODULE.verify(native, wasm, CONTRACT))


if __name__ == "__main__":
    unittest.main()
