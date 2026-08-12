#!/usr/bin/env bash
set -euo pipefail

profile_name="${1:-release}"
case "$profile_name" in
  debug | release) ;;
  *)
    printf 'profile must be debug or release, got: %s\n' "$profile_name" >&2
    exit 2
    ;;
esac

if [[ "$profile_name" == release ]]; then
  rustc_output="$({
    CARGO_TERM_COLOR=never cargo rustc \
      -p opendal-mbt-native --locked --release \
      -- --print native-static-libs
  } 2>&1)"
else
  rustc_output="$({
    CARGO_TERM_COLOR=never cargo rustc \
      -p opendal-mbt-native --locked \
      -- --print native-static-libs
  } 2>&1)"
fi
printf '%s\n' "$rustc_output"

native_libraries="$({
  printf '%s\n' "$rustc_output" |
    sed -n 's/^note: native-static-libs: //p' |
    tail -n 1
} || true)"
if [[ -z "$native_libraries" ]]; then
  printf 'rustc did not report native-static-libs\n' >&2
  exit 1
fi

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
archive_path="$repository_root/target/$profile_name/libopendal_mbt_native.a"

make -C "$repository_root/examples/c" run \
  OPENDAL_MBT_LIB="$archive_path" \
  OPENDAL_MBT_NATIVE_LIBS="$native_libraries"

completion_probe="$(mktemp "${TMPDIR:-/tmp}/opendal-mbt-completion.XXXXXX")"
trap 'rm -f "$completion_probe"' EXIT
${CC:-cc} -std=c11 -O2 -Wall -Wextra -Werror -Wpedantic \
  "$repository_root/tests/c/async_completion_probe.c" \
  "$archive_path" $native_libraries -o "$completion_probe"
"$completion_probe"
