# OpenDAL MoonBit Wasm loader canary

`loadOpenDalMoonBit` instantiates the Rust OpenDAL bridge first and supplies
its scalar exports as the `opendal_mbt_bridge` import module when instantiating
a MoonBit Wasm consumer:

```js
import { loadOpenDalMoonBit } from "./index.mjs";

const runtime = await loadOpenDalMoonBit({
  bridge: new URL("../artifacts/opendal_mbt_wasm_bridge.wasm", import.meta.url),
  moonbit: new URL("../artifacts/opendal_wasm_canary.wasm", import.meta.url),
});

const status = runtime.exports.opendal_mbt_wasm_canary_roundtrip();
```

HTTP(S) URLs work in browsers and Node. The loader also reads `file:` URL
objects directly in Node; bare local path strings are intentionally not
interpreted as files.

This loader deliberately does not inspect either module's linear memory. The
MoonBit facade and Rust bridge exchange only scalar handles and byte values.
The loader remains repository-owned experimental glue until packaging chooses
between core-module composition and WIT components.
