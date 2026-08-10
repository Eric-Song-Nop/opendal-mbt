#!/usr/bin/env bash
set -euo pipefail

profile_name="${1:-release}"
case "$profile_name" in
  debug)
    cargo_profile_args=()
    ;;
  release)
    cargo_profile_args=(--release)
    ;;
  *)
    printf 'profile must be debug or release, got: %s\n' "$profile_name" >&2
    exit 2
    ;;
esac

rustc_output="$({
  CARGO_TERM_COLOR=never cargo rustc \
    -p opendal-mbt-native --locked "${cargo_profile_args[@]}" \
    -- --print native-static-libs
} 2>&1)"
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
