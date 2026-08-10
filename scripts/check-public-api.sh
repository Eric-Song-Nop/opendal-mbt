#!/bin/sh

set -eu

public_declarations=$(
  find . -type f -name '*.mbt' \
    ! -path './integration/*' ! -path './_build/*' \
    -exec grep -nH -E 'declare[[:space:]]+pub' {} + || true
)
if [ -n "$public_declarations" ]; then
  printf '%s\n' "$public_declarations"
  echo "public MoonBit files must not contain declaration-only APIs" >&2
  exit 1
fi

ffi_files=$(
  find . -type f -name '*.mbt' \
    ! -path './integration/*' ! -path './_build/*' \
    -exec grep -l -F 'extern "C" fn' {} + | sort || true
)
if [ "$ffi_files" != "./native_ffi.mbt" ]; then
  echo "native externs must live only in native_ffi.mbt" >&2
  printf '%s\n' "$ffi_files" >&2
  exit 1
fi

if grep -nE 'Native[A-Z]|native_' pkg.generated.mbti; then
  echo "native implementation types leaked into pkg.generated.mbti" >&2
  exit 1
fi
