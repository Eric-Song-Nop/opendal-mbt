#!/bin/sh

set -eu

package_target=$(mktemp -d "${TMPDIR:-/tmp}/opendal-mbt-package-list.XXXXXX")
trap 'rm -rf "$package_target"' EXIT HUP INT TERM

package_files=$(moon package --list --frozen --target-dir "$package_target" 2>&1)
printf '%s\n' "$package_files"
package_list_file="$package_target/package-files.txt"
printf '%s\n' "$package_files" > "$package_list_file"

if printf '%s\n' "$package_files" | \
  grep -nE '^(trace\.json$|integration/|moon\.work$|Makefile$|Cargo\.(lock|toml)$|rust-toolchain\.toml$|native/rust/|wasm/|native/distribution-profile\.json$|native/distribution-profiles/|examples/c/|examples/browser/(\.mooncakes/|_build/|__pycache__/|.*pkg\.generated\.mbti$)|tests/|scripts/)'; then
  echo "maintainer-only files leaked into the published module" >&2
  exit 1
fi

for required_file in \
  LICENSE README.mbt.md getting-started.mbt.md connecting.mbt.md \
  tasks.mbt.md browser-guide.mbt.md build.js src/moon.pkg src/README.mbt.md \
  src/browser/moon.pkg src/browser/embedded_runtime.generated.mbt \
  src/browser/embedded.mbt src/browser_demo/moon.pkg \
  src/browser_demo/launcher.mbt src/browser_demo/main.mbt \
  src/getting-started.mbt.md src/connecting.mbt.md src/tasks.mbt.md \
  src/browser-guide.mbt.md \
  examples/browser/README.md examples/browser/launcher.mbt \
  examples/browser/main.js.mbt examples/browser/main.native.mbt \
  examples/browser/moon.mod examples/browser/moon.pkg \
  examples/browser/nonblocking.mbt examples/browser/run.mbt \
  examples/browser/verify_native_nonblocking.py \
  docs/README.md docs/design/browser-runtime.md \
  docs/design/c-abi.md docs/design/native-distribution.md \
  docs/design/public-api-semantics.md docs/reference/browser-api.md \
  docs/releasing.md docs/roadmap.md \
  native/artifact-selection.json native/artifacts.json \
  native/artifacts-standard.json \
  native/include/opendal_mbt.h; do
  if ! printf '%s\n' "$package_files" | grep -Fxq "$required_file"; then
    echo "required published file is missing: $required_file" >&2
    exit 1
  fi
done

node scripts/check-docs.mjs --package-list "$package_list_file"
