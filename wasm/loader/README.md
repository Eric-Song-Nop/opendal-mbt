# OpenDAL MoonBit Wasm loader canary

`loadOpenDalMoonBit` initializes the wasm-bindgen-processed Rust OpenDAL bridge
first, then instantiates the MoonBit Wasm consumer with three repository-owned
import groups:

- the Rust scalar exports as `opendal_mbt_bridge`;
- `opendal_mbt_host.wait_task` and `cancel_wait` for later-turn task polling,
  callback delivery, and explicit wait deregistration;
- `moonbit:ffi.make_closure` for MoonBit Wasm closure imports.

The application does not construct this import table:

```js
import { loadOpenDalMoonBit } from "./index.mjs";
import initializeBridge from "../artifacts/opendal_mbt_wasm_bridge.mjs";

const runtime = await loadOpenDalMoonBit({
  bridgeInitializer: initializeBridge,
  bridge: new URL(
    "../artifacts/opendal_mbt_wasm_bridge_bg.wasm",
    import.meta.url,
  ),
  moonbit: new URL("../artifacts/opendal_wasm_canary.wasm", import.meta.url),
});

const status = runtime.exports.opendal_mbt_wasm_canary_roundtrip();
runtime.dispose();
```

The bridge initializer is the default export produced by the exact pinned
wasm-bindgen CLI. HTTP(S) URLs work in browsers. The loader also reads `file:`
URL objects directly in Node for the MoonBit module; the Node canary passes the
bridge bytes to its initializer because Node does not fetch `file:` URLs. Bare
local path strings are intentionally not interpreted as files.

For callback operations, `wait_task` begins polling in a microtask. A pending
task is polled again from a later zero-delay timer turn; a ready task receives
its MoonBit callback. Cancelling an `Operation` deregisters its wait before the
task handle is released, so an already queued poll cannot touch a stale handle
or overwrite an unrelated bridge diagnostic. This keeps callback delivery out
of the initiating MoonBit/Rust stack. `dispose()` is idempotent: it cancels all
registered waits and permanently tears down the Rust instance so late task
completions are inert. Call it when the application instance is no longer used.

The Node runner currently invokes only the synchronous round-trip export, so
it is an ABI smoke test. The real Chrome/Chromium runner in
`../canary/run-browser.mjs` is the evidence that forced-pending callback tasks
reach ready state while a browser heartbeat remains responsive.

This loader deliberately does not inspect either module's linear memory. The
MoonBit facade and Rust bridge still transfer payload bytes through scalar
buffer operations, and only the separate host callback import carries a
MoonBit closure. Bounded cross-memory bulk transfer is a later milestone.

The loader enables the experimental public callback `Operation` API; it does
not make ordinary browser MoonBit `async fn` suspension portable. That remains
limited by the officially supported MoonBit runtime. The loader stays
repository-owned experimental glue until packaging and the long-term
core-module/WIT boundary are decided.
