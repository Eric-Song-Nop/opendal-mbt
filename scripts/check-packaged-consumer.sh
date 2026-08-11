#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_table=
artifact_profile=
if [ "$#" -eq 3 ] && [ "$1" = "--artifact-table" ]; then
  if [ ! -f "$2" ]; then
    echo "candidate artifact table does not exist: $2" >&2
    exit 2
  fi
  artifact_table=$(CDPATH= cd -- "$(dirname -- "$2")" && pwd)/$(basename -- "$2")
  artifact_profile=$(node -e '
    const fs = require("node:fs");
    const value = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    if (typeof value.service_profile !== "string" ||
        !/^[a-z][a-z0-9-]*$/.test(value.service_profile)) process.exit(2);
    process.stdout.write(value.service_profile);
  ' "$artifact_table")
  shift 2
fi
if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
  echo "usage: check-packaged-consumer.sh [--artifact-table <json>] <native-artifact.tar.gz>" >&2
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
if [ -n "$artifact_table" ]; then
  if [ "$artifact_profile" = "local" ]; then
    staged_table="$stage_dir/native/artifacts.json"
  else
    staged_table="$stage_dir/native/artifacts-$artifact_profile.json"
  fi
  cp "$artifact_table" "$staged_table"
  printf '{\n  "schema_version": 1,\n  "service_profile": "%s",\n  "artifact_table": "%s"\n}\n' \
    "$artifact_profile" "$(basename -- "$staged_table")" > \
    "$stage_dir/native/artifact-selection.json"
fi
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
    # Materialize exact registry dependencies in this fresh consumer tree.
    # The caller has already refreshed the shared registry through moon-deps.
    moon check --target wasm
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
