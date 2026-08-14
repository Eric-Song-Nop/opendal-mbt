#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/opendal-mbt-browser-package.XXXXXX")
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
mkdir "$stage_dir"
unzip -q "$package_archive" -d "$stage_dir"

for required in \
  src/browser/embedded_runtime.generated.mbt \
  src/browser/embedded.mbt \
  src/browser_demo/moon.pkg \
  src/browser_demo/launcher.mbt \
  src/browser_demo/main.mbt; do
  if [ ! -f "$stage_dir/$required" ]; then
    echo "packaged browser consumer is missing $required" >&2
    exit 1
  fi
done

for forbidden in \
  .mooncakes trace.json Cargo.toml Cargo.lock rust-toolchain.toml \
  node_modules package.json wasm scripts target; do
  if [ -e "$stage_dir/$forbidden" ]; then
    echo "packaged browser consumer contains maintainer asset $forbidden" >&2
    exit 1
  fi
done

external_asset=$(find "$stage_dir" -type f \( \
  -name '*.wasm' -o -name '*.mjs' -o -name '*.rs' -o -name '*.tgz' \
\) -print -quit)
if [ -n "$external_asset" ]; then
  echo "packaged browser consumer contains an external runtime asset: $external_asset" >&2
  exit 1
fi

moon_path=$(command -v moon)
moon_install=$(CDPATH= cd -- "$(dirname -- "$moon_path")/.." && pwd)
test_moon_home="$work_dir/moon-home"
mkdir "$test_moon_home"
for entry in bin include lib registry; do
  if [ -e "$moon_install/$entry" ]; then
    ln -s "$moon_install/$entry" "$test_moon_home/$entry"
  fi
done

clean_bin="$work_dir/clean-bin"
mkdir "$clean_bin"
link_tool() {
  tool_name=$1
  tool_path=$(command -v "$tool_name" 2>/dev/null || true)
  if [ -n "$tool_path" ] && [ ! -e "$clean_bin/$tool_name" ]; then
    ln -s "$tool_path" "$clean_bin/$tool_name"
  fi
}
for tool in moon moonc moonrun node sh ps \
  google-chrome-stable google-chrome chromium chromium-browser; do
  link_tool "$tool"
done

if output=$(
  cd "$stage_dir"
  PATH="$clean_bin"
  MOON_HOME="$test_moon_home"
  export PATH MOON_HOME
  unset CARGO_HOME RUSTUP_HOME FORCE_COLOR OPENDAL_MBT_NATIVE_LIB
  for forbidden_tool in cargo rustc wasm-bindgen npm npx esbuild webpack rollup vite; do
    if command -v "$forbidden_tool" >/dev/null 2>&1; then
      echo "$forbidden_tool is visible during the packaged browser test" >&2
      exit 1
    fi
  done
  test ! -e .mooncakes
  moon run --target js --release src/browser_demo 2>&1
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
  *"opendal browser demo ok: 23 bytes, 1 entry"*) ;;
  *)
    echo "packaged browser demo did not complete the Chrome round trip" >&2
    exit 1
    ;;
esac
