#!/bin/sh

set -eu

package_target=$(mktemp -d "${TMPDIR:-/tmp}/opendal-mbt-package-list.XXXXXX")
trap 'rm -rf "$package_target"' EXIT HUP INT TERM

package_files=$(moon package --list --frozen --target-dir "$package_target" 2>&1)
printf '%s\n' "$package_files"

if printf '%s\n' "$package_files" | \
  grep -nE '^(integration/|moon\.work$|Makefile$|scripts/check-)'; then
  echo "development workspace fixtures leaked into the published module" >&2
  exit 1
fi

for required_file in \
  LICENSE README.mbt.md build.js native/artifacts.json \
  native/distribution-profile.json Cargo.toml Cargo.lock examples/c/Makefile \
  native/include/opendal_mbt.h; do
  if ! printf '%s\n' "$package_files" | grep -Fxq "$required_file"; then
    echo "required published file is missing: $required_file" >&2
    exit 1
  fi
done
