# OpenDAL MoonBit WebAssembly canaries

Status: experimental, unpublished implementation candidate

This directory contains the repository proofs for the backend-neutral
`Eric-Song-Nop/opendal/wasm` binding. The binding constructs an OpenDAL
operator by scheme and configuration; `memory` is the deterministic artifact
fixture used by the current tests, not a special public backend or product
direction.

The architecture and delivery plan are documented in
[`docs/design/wasm-integration.md`](../docs/design/wasm-integration.md) and
[`docs/design/wasm-mooncake-delivery.md`](../docs/design/wasm-mooncake-delivery.md).

## What exists

The experimental public MoonBit package exposes:

- `available_schemes()` and `Operator::new(scheme, config)` for every service
  compiled into the selected bridge artifact;
- `Operator::info()`, including the effective scheme, root, name, and
  capabilities reported by OpenDAL;
- `Operator::as_async()` and `AsyncOperator` callback methods for create,
  write, read, stat, bounded list, and delete operations;
- native-shaped read ranges and operation options: read range/version/
  conditions, stat version/conditions, and write append/content headers/
  conditions, with write and stat both returning `Metadata`;
- a `Task` with `Pending`, `Completed`, `Cancelled`, and `Closed` states,
  plus idempotent logical cancellation and close;
- owned `Bytes`, `Metadata`, `Entry`, `Capability`, and native-shaped
  `OpenDalError`/`ErrorInfo` values;
- explicit, idempotent `Operator::close()` and `AsyncOperator::close()`.

The public MoonBit package intentionally does **not** expose synchronous
storage methods, `Operator::memory`, raw task/resource handles, ABI version or
feature queries, live-handle counters, or forced-pending diagnostics. Those
remaining raw bridge exports are low-level fixture and ABI oracles only.

The current bridge artifact compiles the OpenDAL memory service. A future
artifact may compile any set of browser-compatible OpenDAL services without
changing this MoonBit API. Runtime availability is the intersection of the
artifact's compiled schemes and the selected service's browser/host
requirements; `available_schemes()` reports the former, and capability values
come from the constructed operator.

## Module boundary

```text
MoonBit application
  -> Eric-Song-Nop/opendal/wasm
  -> scalar resource/task imports named opendal_mbt_bridge.*
  -> scheduler and bounded-copy imports named opendal_mbt_host.*
  -> repository loader
  -> Rust core-Wasm bridge
  -> OpenDAL service selected by Operator::new(scheme, config)
```

Rust and MoonBit define and export separate linear memories. Paths, payloads,
messages, metadata, errors, tasks, and completions are owned behind positive,
generation-checked integer handles. Cross-module calls use fixed-width Wasm
scalars. The loader instantiates Rust first, wires its exports and the host
scheduler/copy functions into MoonBit, and owns instance teardown.

OpenDAL failures cross that boundary as immutable, owned `ODE1` snapshots.
MoonBit validates the versioned little-endian envelope and strict UTF-8 before
constructing `OpenDalError`, then attaches the initiating operation, path, and
destination. Malformed snapshots are binding ABI mismatches rather than
partially decoded backend errors.

Stat and write results cross as owned `ODM1` metadata snapshots. The schema
carries the native-shaped mode, length, current/deleted state, timestamp, and
seven optional HTTP/version strings. MoonBit validates the complete envelope,
canonical absence, timestamp, lengths, and UTF-8 before publishing a value.
List entries carry the same schema using the metadata supplied by the lister;
optional fields can therefore be absent and the facade does not issue a stat
for each entry.

Whole-object materialization is capped at 64 MiB. The loader copies at most
256 KiB per call and recreates typed-array views for every window so
`memory.grow` cannot leave a stale view. The public facade does not use the
bridge's legacy per-byte buffer operations.

Operator configuration is capped at 1,024 entries and 1 MiB of combined key
and value UTF-8. ASCII-case-insensitive duplicate keys are rejected before an
operator is constructed.

Bounded list materialization fails atomically above 65,536 entries or 16 MiB
of combined path, name, and pre-encoded `ODM1` metadata bytes.

## Evidence

The three checks make different claims:

- `make wasm-static-contract` checks the committed browser-memory snapshot:
  exact Rust and MoonBit imports/exports, one defined and exported memory per
  module, no imported/shared/memory64 memory, no `env` or WASI imports, MoonBit
  bridge imports resolved by Rust exports, and raw/gzip/Brotli size ceilings.
- `make wasm-canary` runs in Node.js through the callback task path. It checks
  bootstrap, generic memory construction, bounded bulk transfer, write content
  options and returned metadata, ranged reads, stat/list metadata, two
  complete callback lifecycles, owned errors, repeated cleanup, and final
  teardown.
- `make wasm-browser-canary` runs the same callback binding in real headless
  Chrome/Chromium. It explicitly enables the bridge's test-only
  forced-pending wrapper and checks twelve observed pending tasks, heartbeat
  ordering, cancellation suppression, completion-versus-cancel behavior,
  concurrent operators, diagnostic isolation, and teardown with work in
  flight.

Only the Chrome/Chromium check is evidence that the forced `Pending -> Ready`
sequence yields to a real browser event loop. The Node check exercises real
tasks and callbacks, but it neither enables the forced-pending hook nor makes
a browser-heartbeat claim.

The bridge ABI is version `0x0001_0006`. The bridge reports feature bitmap
`0x0000_07ff`; bits 0 and 1 are the memory and poll-once fixture capabilities,
bit 9 is structured error snapshots, and bit 10 is metadata snapshots and
operation options. The public facade requires only `0x0000_07fc` (generation
handles, binary buffers, task ABI, generic operator construction, common
mutations, bounded listing, bounded bulk transfer, structured errors, and
metadata/options).

The static snapshot is an implementation compatibility gate, not a release
manifest: it does not yet provide artifact provenance, immutable release
hashes, a service-profile distribution contract, or a clean Mooncake
consumer.

## Build and run

Prerequisites are the repository's pinned Rust and MoonBit toolchains, Node.js
18 or newer, the Rust `wasm32-unknown-unknown` target, and wasm-bindgen CLI
`0.2.127`. The browser canary additionally needs Chrome or Chromium; set
`OPENDAL_MBT_BROWSER_BIN` when its executable is not at a known path.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked

make wasm-static-contract
make wasm-canary
make wasm-browser-canary
```

`make wasm-rust` builds the raw Rust module and processes it with the exactly
pinned wasm-bindgen CLI. The raw Cargo output is wasm-bindgen input, not the
deployable bridge. `make wasm-moon` builds the MoonBit canary with the native
artifact resolver disabled for that Wasm build; native builds retain their
existing behavior.

After an intentional module-boundary change, regenerate the static snapshot
with the pinned Wasm toolchains and review the complete import/export diff:

```sh
make wasm-rust wasm-moon
node scripts/check-wasm-contract.mjs --print-snapshot \
  target/wasm-bindgen/release/opendal_mbt_wasm_bridge_bg.wasm \
  target/wasm-bindgen/release/opendal_mbt_wasm_bridge.mjs \
  _build/wasm/release/build/eric-song-nop/opendal-wasm-canary/opendal-wasm-canary.wasm \
  > wasm/contract/browser-memory.json
make wasm-static-contract
```

Snapshot regeneration is a compatibility review, not an automatic response to
a failing gate. Size baselines record the reviewed build; only the separately
reviewed ceilings fail ordinary build-size drift.

## Execution and cancellation

Rust schedules production tasks with `wasm_bindgen_futures::spawn_local` and
awaits the OpenDAL future directly. The forced-pending delay is disabled by
default and can be enabled only through a raw canary export; the browser test
does so before its deterministic scheduling matrix.

The loader starts task observation from a later microtask and polls pending
tasks on later zero-delay timer turns. A MoonBit callback therefore never runs
from the initiating MoonBit/Rust stack. Cancellation unregisters the loader
wait before cancelling and releasing the task handle. It suppresses callback
delivery and makes a late completion inert, but it does not claim to abort an
underlying browser API or OpenDAL future. `runtime.dispose()` cancels remaining
waits, permanently tears down the bridge instance, and makes subsequent late
completion inert.

This callback surface is explicit because ordinary-browser MoonBit `async fn`
suspension is not yet a stable, documented portability contract. The binding
does not disguise callbacks as synchronous storage operations.

## Remaining distribution work

M0 (two-module interoperability and ownership), M1 (callback task lifecycle),
and M2 (bounded bulk transfer) now have repository evidence. The static
browser-memory contract also has a committed gate. None of those facts makes
the package installable from Mooncakes.

Before a preview release, the project still needs:

- a versioned Wasm artifact manifest with provenance and exact hashes;
- a supported way to deliver the bridge, wasm-bindgen glue, loader, and
  manifest from the Moon package;
- a clean consumer with no Rust toolchain, npm dependency, source checkout, or
  handwritten import table;
- cold acquisition, verified cache, relocation, mismatch, and corruption
  tests;
- explicit browser/service profiles and service-specific acceptance fixtures.

OPFS may be one optional persistence fixture in that last category. It is not
a milestone by itself and does not stand in for the generic binding or for any
other OpenDAL backend.
