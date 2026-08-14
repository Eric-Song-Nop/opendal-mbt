# OpenDAL MoonBit Wasm loader

`loadOpenDalMoonBit` initializes the wasm-bindgen-processed Rust OpenDAL bridge
and then instantiates the MoonBit Wasm application with repository-owned
imports:

- Rust scalar resource/task exports under `opendal_mbt_bridge`;
- `opendal_mbt_host.wait_task` and `cancel_wait` for later-turn callback
  delivery and explicit deregistration;
- `opendal_mbt_host.copy_moon_to_bridge` and `copy_bridge_to_moon` for bounded
  copies between the modules' separate memories;
- `moonbit:ffi.make_closure` for MoonBit Wasm closure imports.

Applications call the MoonBit `Eric-Song-Nop/opendal/wasm` facade. They do not
construct this import table, call raw bridge functions, or manage the second
module's memory.

```js
import { loadOpenDalMoonBit } from "./index.mjs";
import initializeBridge from "../artifacts/opendal_mbt_wasm_bridge.mjs";

const runtime = await loadOpenDalMoonBit({
  bridgeInitializer: initializeBridge,
  bridge: new URL(
    "../artifacts/opendal_mbt_wasm_bridge_bg.wasm",
    import.meta.url,
  ),
  moonbit: new URL("../artifacts/app.wasm", import.meta.url),
});

// Invoke an application export that uses Operator::new and AsyncOperator callbacks.
runtime.exports.start_application();

// Call after the application has closed its operators and tasks.
runtime.dispose();
```

The bridge initializer is the default export produced by the exactly pinned
wasm-bindgen CLI. HTTP(S) URLs work in browsers. In Node.js, `file:` URL
support and bridge-byte initialization are loader/canary conveniences; bare
local path strings are not interpreted as files.

## Scheduling contract

`wait_task` begins observation in a microtask. If the task remains pending, the
loader polls it again from a later zero-delay timer turn. A ready task receives
its MoonBit callback, so delivery cannot re-enter the initiating call stack.

Cancelling or closing a pending `Task` deregisters its wait before the
MoonBit facade cancels and releases the task. This prevents an already queued
poll from touching a stale handle. `dispose()` is idempotent: it cancels every
registered wait and permanently tears down the Rust instance, making late
task completion inert.

Production Rust tasks run directly on `spawn_local`. The loader never forces a
delay. The Chrome canary explicitly enables a raw bridge-only forced-pending
hook so it can prove browser heartbeat and event-loop ordering. Node also
exercises bulk transfer and the callback lifecycle twice, but only Chrome is
the forced-`Pending` browser scheduling oracle.

## Transfer contract

The loader validates that both modules define and export independent
`WebAssembly.Memory` objects. It copies at most 256 KiB per host call and
recreates both typed-array views for every window, because either module may
replace its `ArrayBuffer` after `memory.grow`. Whole-object materialization is
capped at 64 MiB.

The bridge pointer returned for a window is valid only for the immediate
synchronous copy; the loader performs no intervening bridge call. The public
MoonBit facade does not import the legacy per-byte bridge operations.

## Status

The loader supports the current experimental callback-only facade for any
service compiled into the chosen browser-compatible bridge artifact. It does
not provide a portable MoonBit `async fn` runtime and does not specialize for
memory, OPFS, or another backend.

The repository static contract pins the current browser-memory canary's exact
imports, exports, memory shape, and size ceilings. Packaging remains open: the
loader is repository-owned until the bridge/glue/manifest can be delivered to
a clean Mooncake consumer through a versioned, verified asset path.
