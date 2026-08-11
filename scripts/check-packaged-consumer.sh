#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "usage: check-packaged-consumer.sh <native-artifact.tar.gz>" >&2
  exit 2
fi
artifact=$(CDPATH= cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")

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
package_archive=$1

stage_dir="$work_dir/stage"
mkdir "$stage_dir" "$stage_dir/integration" "$stage_dir/integration/consumer"
unzip -q "$package_archive" -d "$stage_dir"
cp "$repo_root/integration/consumer/moon.mod" \
  "$stage_dir/integration/consumer/moon.mod"
cp "$repo_root/integration/consumer/moon.pkg" \
  "$stage_dir/integration/consumer/moon.pkg"
cp "$repo_root/integration/consumer/consumer_test.mbt" \
  "$stage_dir/integration/consumer/consumer_test.mbt"
cp "$repo_root/moon.work" "$stage_dir/moon.work"

for forbidden in Cargo.toml Cargo.lock rust-toolchain.toml native/rust \
  examples tests scripts; do
  if [ -e "$stage_dir/$forbidden" ]; then
    echo "maintainer-only source leaked into the Moon package: $forbidden" >&2
    exit 1
  fi
done

moon_path=$(command -v moon)
moon_install=$(CDPATH= cd -- "$(dirname -- "$moon_path")/.." && pwd)
test_moon_home="$work_dir/moon-home"
mkdir "$test_moon_home"
for entry in bin include lib registry; do
  if [ -e "$moon_install/$entry" ]; then
    ln -s "$moon_install/$entry" "$test_moon_home/$entry"
  fi
done

node "$repo_root/scripts/prepare-test-native-cache.js" \
  "$stage_dir" "$artifact" "$test_moon_home"

clean_bin="$work_dir/clean-bin"
mkdir "$clean_bin"
link_tool() {
  tool_name=$1
  tool_path=$(command -v "$tool_name" 2>/dev/null || true)
  if [ -n "$tool_path" ] && [ ! -e "$clean_bin/$tool_name" ]; then
    ln -s "$tool_path" "$clean_bin/$tool_name"
  fi
}
for tool in moon moonc moonrun node cc clang gcc ld as ar ranlib nm strip \
  dsymutil objcopy llvm-objcopy xcrun sh; do
  link_tool "$tool"
done

if output=$(
  cd "$stage_dir"
  PATH="$clean_bin"
  MOON_HOME="$test_moon_home"
  export PATH MOON_HOME
  unset CARGO_HOME RUSTUP_HOME LIBRARY_PATH LD_LIBRARY_PATH \
    DYLD_LIBRARY_PATH OPENDAL_MBT_NATIVE_LIB
  if command -v cargo >/dev/null 2>&1; then
    echo "Cargo is visible during the packaged consumer test" >&2
    exit 1
  fi
  {
    moon test integration/consumer --target native --frozen \
      --warn-list '-68+73' --deny-warn
    moon test integration/consumer --target native --frozen --release \
      --warn-list '-68+73' --deny-warn
  } 2>&1
); then
  status=0
else
  status=$?
fi
printf '%s\n' "$output"
if [ "$status" -ne 0 ]; then
  exit "$status"
fi
case "$output" in
  *"[opendal.mbt] Using cached"*) ;;
  *)
    echo "packaged consumer did not use the verified native cache" >&2
    exit 1
    ;;
esac
