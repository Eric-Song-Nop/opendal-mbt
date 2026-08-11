#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
profile=${1:-debug}
cargo_profile_flag=

case "$profile" in
  debug) ;;
  release) cargo_profile_flag=--release ;;
  *)
    echo "profile must be debug or release" >&2
    exit 2
    ;;
esac

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/opendal-mbt-package.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT HUP INT TERM

package_target="$work_dir/package-build"
(
  cd "$repo_root"
  moon package --frozen --target-dir "$package_target"
)

set -- "$package_target"/publish/*.zip
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "expected exactly one packaged MoonBit module" >&2
  exit 1
fi
archive=$1

stage_dir="$work_dir/stage"
mkdir "$stage_dir" "$stage_dir/integration" "$stage_dir/integration/consumer"
unzip -q "$archive" -d "$stage_dir"
cp "$repo_root/integration/consumer/moon.mod" \
  "$stage_dir/integration/consumer/moon.mod"
cp "$repo_root/integration/consumer/moon.pkg" \
  "$stage_dir/integration/consumer/moon.pkg"
cp "$repo_root/integration/consumer/consumer_test.mbt" \
  "$stage_dir/integration/consumer/consumer_test.mbt"
cp "$repo_root/moon.work" "$stage_dir/moon.work"

(
  cd "$stage_dir"
  cargo build --workspace --locked $cargo_profile_flag
  OPENDAL_MBT_NATIVE_LIB="$stage_dir/target/$profile/libopendal_mbt_native.a" \
    moon test integration/consumer --target native --frozen \
      --warn-list '-68+73' --deny-warn \
      $(if [ "$profile" = release ]; then printf '%s' --release; fi)
)
