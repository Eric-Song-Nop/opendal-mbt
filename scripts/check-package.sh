#!/bin/sh

set -eu

package_target=$(mktemp -d "${TMPDIR:-/tmp}/opendal-mbt-package-list.XXXXXX")
trap 'rm -rf "$package_target"' EXIT HUP INT TERM

package_files=$(moon package --list --frozen --target-dir "$package_target" 2>&1)
printf '%s\n' "$package_files"

if printf '%s\n' "$package_files" | \
  grep -nE '^(integration/|moon\.work$|Makefile$|Cargo\.(lock|toml)$|rust-toolchain\.toml$|native/rust/|native/distribution-profile\.json$|native/distribution-profiles/|wasm/rust-bridge/|wasm/canary/|examples/|tests/|scripts/)'; then
  echo "maintainer-only files leaked into the published module" >&2
  exit 1
fi

for required_file in \
  LICENSE README.mbt.md getting-started.mbt.md connecting.mbt.md \
  tasks.mbt.md build.js src/moon.pkg src/README.mbt.md \
  src/getting-started.mbt.md src/connecting.mbt.md src/tasks.mbt.md \
  src/wasm/moon.pkg src/wasm/pkg.generated.mbti \
  src/wasm/bridge_ffi.mbt src/wasm/operator.mbt \
  src/wasm/transfer.mbt src/wasm/types.mbt \
  wasm/loader/index.mjs wasm/loader/package.json \
  wasm/README.md wasm/loader/README.md \
  native/artifact-selection.json native/artifacts.json \
  native/artifacts-standard.json \
  native/include/opendal_mbt.h; do
  if ! printf '%s\n' "$package_files" | grep -Fxq "$required_file"; then
    echo "required published file is missing: $required_file" >&2
    exit 1
  fi
done
