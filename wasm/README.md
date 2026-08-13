# OpenDAL MoonBit WebAssembly canaries

Status: experimental, unpublished implementation slice

This directory contains the repository end-to-end proofs described in
[`docs/design/wasm-integration.md`](../docs/design/wasm-integration.md) and the
current delivery plan in
[`docs/design/wasm-mooncake-delivery.md`](../docs/design/wasm-mooncake-delivery.md).
A MoonBit `wasm` package invokes the real Rust OpenDAL `memory` service without
sharing either language runtime's object layout or linear-memory allocator.

## What the canaries prove

There are two deliberately different runners:

- `make wasm-canary` is a Node.js smoke test for the synchronous scalar ABI. It
  checks ABI version and features, binary-safe write/read, `stat`, owned
  `NotFound`, generation-checked stale-handle rejection, explicit release, and
  teardown with zero live handles.
- `make wasm-browser-canary` launches a real headless Chrome/Chromium page. It
  drives the callback task ABI through create-dir, write, read, `stat`,
  idempotent recursive delete, and `NotFound`; forces all seven Rust futures
  to return `Pending` on their first poll; proves
  that a previously queued browser heartbeat runs before completion; suppresses
  a cancel-before-ready callback; and verifies handle cleanup and instance
  teardown.

Only the browser canary is evidence for `Pending -> Ready` and event-loop
responsiveness. A passing Node canary is not asynchronous-browser evidence.

Both calls originate in
[`integration/wasm/canary.mbt`](../integration/wasm/canary.mbt) through the
public `Eric-Song-Nop/opendal/wasm` package. The Rust implementation is not
reproduced in MoonBit or JavaScript.

## Module boundary

```text
MoonBit canary
  -> Eric-Song-Nop/opendal/wasm
  -> scalar resource/task imports named opendal_mbt_bridge.*
  -> callback scheduling import named opendal_mbt_host.wait_task
  -> Rust core-Wasm module
  -> OpenDAL memory service
```

Rust and MoonBit keep separate memories. Paths, payloads, messages, metadata,
errors, tasks, and completions are owned through positive, generation-checked
integer handles. The Rust bridge ABI uses only fixed-width Wasm scalars; no
pointer or language object enters Rust. The separate scheduler import carries a task
handle and a MoonBit closure through MoonBit's Wasm closure FFI.
[`loader/index.mjs`](loader/index.mjs) instantiates Rust first, supplies the
bridge exports and callback host imports to MoonBit, and owns teardown.

## Build and run

Prerequisites are the repository's pinned Rust and MoonBit toolchains, Node.js
18 or newer, the Rust `wasm32-unknown-unknown` target, and the wasm-bindgen CLI
version locked by the bridge dependency graph. The browser canary additionally
needs Chrome or Chromium; set `OPENDAL_MBT_BROWSER_BIN` if it is not in a known
location.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked

# Synchronous ABI smoke in Node.js.
make wasm-canary

# Forced-Pending callback lifecycle in a real browser.
make wasm-browser-canary
```

The equivalent steps are:

```sh
CARGO_PROFILE_RELEASE_PANIC=abort \
  cargo build --locked --package opendal-mbt-wasm-bridge \
  --target wasm32-unknown-unknown --release

mkdir -p target/wasm-bindgen/release
wasm-bindgen --target web --no-typescript \
  --out-dir target/wasm-bindgen/release \
  --out-name opendal_mbt_wasm_bridge \
  target/wasm32-unknown-unknown/release/opendal_mbt_wasm_bridge.wasm
mv target/wasm-bindgen/release/opendal_mbt_wasm_bridge.js \
  target/wasm-bindgen/release/opendal_mbt_wasm_bridge.mjs

OPENDAL_MBT_SKIP_NATIVE=1 \
  moon build --target wasm --release integration/wasm

node wasm/canary/run.mjs \
  target/wasm-bindgen/release/opendal_mbt_wasm_bridge.mjs \
  target/wasm-bindgen/release/opendal_mbt_wasm_bridge_bg.wasm \
  _build/wasm/release/build/eric-song-nop/opendal-wasm-canary/opendal-wasm-canary.wasm

node wasm/canary/run-browser.mjs \
  target/wasm-bindgen/release/opendal_mbt_wasm_bridge.mjs \
  target/wasm-bindgen/release/opendal_mbt_wasm_bridge_bg.wasm \
  _build/wasm/release/build/eric-song-nop/opendal-wasm-canary/opendal-wasm-canary.wasm
```

The raw Cargo output is wasm-bindgen input, not a deployable bridge. OpenDAL's
Wasm dependencies use JavaScript-backed time, UUID, and related support; the
generated `.mjs` supplies those host imports and initializes the processed
`_bg.wasm` module. The CLI version must match the Rust `wasm-bindgen` crate.

`OPENDAL_MBT_SKIP_NATIVE=1` is currently explicit because the installed Moon
prebuild protocol supplies environment and module paths, but not the selected
backend. The opt-out returns an empty link configuration before reading or
downloading a native artifact. Native builds retain their existing default.

The Node runner exits successfully only when the MoonBit-exported synchronous
round trip returns status `0`. Other status values identify the failed
acceptance check; they are defined next to the fixture in
`integration/wasm/canary.mbt`. The browser runner serves the artifacts on
loopback HTTP, waits for the page to report, and prints a successful result of
the following shape:

```json
{"ok":true,"pendingTasks":7,"heartbeat":true,"cancellation":"suppressed"}
```

## Package surface

The experimental `Eric-Song-Nop/opendal/wasm` package currently exposes:

- backend-neutral `Operator::new(scheme, config)`, `available_schemes`,
  `OperatorInfo`, and `Capability`; `Operator::memory` remains a thin test
  convenience;
- synchronous `write`, `read`, and `stat`, and idempotent `close`;
- `write_callback`, `read_callback`, `stat_callback`,
  `create_dir_callback`, and `delete_callback`, each returning an `Operation`
  with `Pending`, `Completed`, `Cancelled`, and `Closed` states;
- idempotent `Operation::cancel` and `Operation::close`;
- `Metadata` with `content_length` and `is_file`;
- an owned `WasmError` snapshot with code, message, and `is_not_found`;
- bridge version/features plus canary-only live-handle and forced-pending
  diagnostics.

The bridge ABI is version `0x0001_0002`. Its current feature bitmap is
`0x0000_007f`: memory service, poll-once synchronous canary, generation
handles, binary buffers, task ABI, generic operator construction through the
compiled OpenDAL service registry, and common create-dir/delete mutations.

The native `Eric-Song-Nop/opendal` package and its C/static-library ABI are not
changed or reused by this backend.

## Execution and cancellation limits

The callback path owns Rust tasks and completions. Rust uses
`wasm_bindgen_futures::spawn_local`; the canary wraps each operation in a
zero-delay browser timer that guarantees at least one real `Pending` poll. The
loader observes task state from later microtasks/timer turns and delivers the
MoonBit callback only after the task is ready. Inputs are copied and the
OpenDAL operator is cloned before a task starts.

This is a real browser scheduling proof, but it is not yet the stable MoonBit
`async fn` API. Ordinary browser-hosted MoonBit async/await remains limited by
the officially documented runtime support. The currently usable public Wasm
path is the explicitly experimental callback `Operation` API.

Cancellation is **logical cancellation**. It suppresses the user callback and
makes a late completion inert, but it does not claim to abort the underlying
OpenDAL future or browser I/O. `runtime.dispose()` stops loader polling, clears
the bridge arena, and makes later task completion inert.

The synchronous methods remain only for ABI smoke coverage. They still poll
once with a no-op waker and report code `9` if an OpenDAL future suspends; they
must not be used for OPFS, S3, or other genuinely asynchronous services.

## Still required before release

- obtain a documented, portable ordinary-browser MoonBit async continuation
  contract, or keep the callback surface explicitly preview-only;
- complete the task race/concurrency matrix beyond the current success,
  `NotFound`, cancel-before-ready, operator-close, and teardown cases;
- replace per-byte public transfer with bounded bulk copy and verify large
  binary values and `memory.grow` handling;
- record exact imports/exports, artifact sizes, and startup evidence;
- pin, checksum, and distribute the Rust Wasm companion artifact;
- make a clean registry consumer acquire the companion module without a Rust
  checkout;
- decide whether core-module loading remains the product boundary or is
  replaced by a WIT component;
- exercise additional browser-compatible service profiles through the same
  generic constructor after the common lifecycle, transfer, packaging, and
  service-specific cancellation contracts are proven; OPFS is only one
  optional persistence fixture.

The existing native artifact manifest is intentionally not used for these Wasm
outputs. Wasm distribution will receive its own compatibility and provenance
contract.
