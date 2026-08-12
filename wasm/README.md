# OpenDAL MoonBit WebAssembly canary

Status: experimental, unpublished implementation slice

This directory implements the first end-to-end proof described in
[`docs/design/wasm-integration.md`](../docs/design/wasm-integration.md): a
MoonBit `wasm` package invokes the real Rust OpenDAL `memory` service without
sharing either language runtime's object layout or linear-memory allocator.

## What this slice proves

The canary crosses the complete boundary twice and checks:

- ABI version and feature discovery;
- a binary-safe write/read containing NUL and non-UTF-8 bytes;
- `stat` metadata copied into a MoonBit value;
- OpenDAL `NotFound` translated into an owned MoonBit error;
- explicit release, a bridge-level stale-handle rejection probe, generation
  reuse, and a live-handle leak oracle;
- repository-owned loading, so the fixture supplies no handwritten Wasm import
  table.

The OpenDAL call originates in [`integration/wasm/canary.mbt`](../integration/wasm/canary.mbt)
through the public `Eric-Song-Nop/opendal/wasm` package. The Rust implementation
is not reproduced in MoonBit or JavaScript.

## Module boundary

```text
MoonBit canary
  -> Eric-Song-Nop/opendal/wasm
  -> scalar imports named opendal_mbt_bridge.*
  -> Rust core-Wasm module
  -> OpenDAL memory service
```

Rust and MoonBit keep separate memories. Paths, payloads, messages, metadata,
and errors are copied through positive, generation-checked integer handles.
Every direct import uses only Wasm `i32`; no pointer or language object crosses
the module boundary. [`loader/index.mjs`](loader/index.mjs) instantiates the
Rust module first and supplies its exports to the MoonBit module.

## Build and run

Prerequisites are the repository's pinned Rust and MoonBit toolchains, Node.js
18 or newer, and the Rust `wasm32-unknown-unknown` target:

```sh
rustup target add wasm32-unknown-unknown
make wasm-canary
```

The equivalent steps are:

```sh
CARGO_PROFILE_RELEASE_PANIC=abort \
  cargo build --locked --package opendal-mbt-wasm-bridge \
  --target wasm32-unknown-unknown --release

OPENDAL_MBT_SKIP_NATIVE=1 \
  moon build --target wasm --release integration/wasm

node wasm/canary/run.mjs \
  target/wasm32-unknown-unknown/release/opendal_mbt_wasm_bridge.wasm \
  _build/wasm/release/build/eric-song-nop/opendal-wasm-canary/opendal-wasm-canary.wasm
```

`OPENDAL_MBT_SKIP_NATIVE=1` is currently explicit because the installed Moon
prebuild protocol supplies environment and module paths, but not the selected
backend. The opt-out returns an empty link configuration before reading or
downloading a native artifact. Native builds retain their existing default.

The runner exits successfully only when the MoonBit-exported round-trip returns
status `0`. Other status values identify the failed acceptance check; they are
defined next to the fixture in `integration/wasm/canary.mbt`.

## Package surface

The experimental `Eric-Song-Nop/opendal/wasm` package currently exposes:

- `Operator::memory`, `write`, `read`, `stat`, and idempotent `close`;
- `Metadata` with `content_length` and `is_file`;
- an owned `WasmError` snapshot with code, message, and `is_not_found`;
- bridge version/features and a canary-only live-handle diagnostic.

The native `Eric-Song-Nop/opendal` package and its C/static-library ABI are not
changed or reused by this backend.

## Deliberate limit: poll once

The Rust canary polls each OpenDAL future exactly once with a no-op waker. The
in-memory service is expected to complete without host I/O. If a future becomes
pending, the bridge returns error code `9`; it never spins or blocks the browser
thread.

This adapter is therefore **not valid for OPFS, S3, or any operation that can
suspend**. The next milestone is a host-event-loop scheduler with explicit
pending, ready, cancelled, consumed, and released states. OPFS belongs after
that lifecycle is proven; it is a storage backend test, not the reason this
language boundary is possible.

## Still required before release

- exercise this canary in a supported browser runtime and record import/export
  and artifact-size evidence;
- replace poll-once with the async lifecycle above;
- pin, checksum, and distribute the Rust Wasm companion artifact;
- make a clean registry consumer acquire the companion module without a Rust
  checkout;
- decide whether core-module loading remains the product boundary or is
  replaced by a WIT component;
- add OPFS, then browser-safe S3, only after async cancellation and teardown are
  specified.

The existing native artifact manifest is intentionally not used for these Wasm
outputs. Wasm distribution will receive its own compatibility and provenance
contract.
