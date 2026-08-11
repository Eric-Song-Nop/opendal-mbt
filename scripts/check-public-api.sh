#!/bin/sh

set -eu

public_declarations=$(
  find ./src -type f -name '*.mbt' \
    -exec grep -nH -E 'declare[[:space:]]+pub' {} + || true
)
if [ -n "$public_declarations" ]; then
  printf '%s\n' "$public_declarations"
  echo "public MoonBit files must not contain declaration-only APIs" >&2
  exit 1
fi

ffi_files=$(
  find ./src -type f -name '*.mbt' \
    -exec grep -l -F 'extern "C" fn' {} + | sort || true
)
expected_ffi_files=$(printf '%s\n' \
  './src/async_native_ffi.mbt' \
  './src/native_ffi.mbt')
if [ "$ffi_files" != "$expected_ffi_files" ]; then
  echo "native externs must live only in the private FFI modules" >&2
  printf '%s\n' "$ffi_files" >&2
  exit 1
fi

if grep -nE 'Native[A-Z]|native_' src/pkg.generated.mbti; then
  echo "native implementation types leaked into src/pkg.generated.mbti" >&2
  exit 1
fi
