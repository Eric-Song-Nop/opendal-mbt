# OpenDAL MoonBit Wasm loader canary

`loadOpenDalMoonBit` initializes the wasm-bindgen-processed Rust OpenDAL bridge
first and supplies its scalar exports as the `opendal_mbt_bridge` import module
when instantiating a MoonBit Wasm consumer:

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
```

The bridge initializer is the default export produced by the exact pinned
wasm-bindgen CLI. HTTP(S) URLs work in browsers. The loader also reads `file:`
URL objects directly in Node for the MoonBit module; the Node canary passes the
bridge bytes to its initializer because Node does not fetch `file:` URLs. Bare
local path strings are intentionally not interpreted as files.

This loader deliberately does not inspect either module's linear memory. The
MoonBit facade and Rust bridge exchange only scalar handles and byte values.
The loader remains repository-owned experimental glue until packaging chooses
between core-module composition and WIT components.
