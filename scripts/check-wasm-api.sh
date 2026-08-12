#!/bin/sh

set -eu

target_dir=$(mktemp -d "${TMPDIR:-/tmp}/opendal-mbt-wasm-api.XXXXXX")
trap 'rm -rf "$target_dir"' EXIT HUP INT TERM

OPENDAL_MBT_SKIP_NATIVE=1 moon info --target wasm --frozen \
  --target-dir "$target_dir" src/wasm/operator.mbt >/dev/null

generated="$target_dir/wasm/debug/check/Eric-Song-Nop/opendal/wasm/wasm.mbti"
diff -u src/wasm/pkg.generated.mbti "$generated"
