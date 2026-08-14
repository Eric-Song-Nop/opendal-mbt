# Browser backend

This directory contains the browser implementation used by the MoonBit JS
backend. It deliberately has one WebAssembly instance and one linear memory:

```text
MoonBit async API -> JavaScript Promise facade -> scalar task ABI -> OpenDAL
```

The raw Rust ABI is private implementation detail. JavaScript owns its handles,
copies bytes directly to and from the OpenDAL bridge memory, and turns a task
into a `Promise`. Normal OpenDAL failures resolve to a structured outcome;
module-loading and ABI-corruption failures reject the promise.

The bridge currently compiles the browser-compatible `memory`, `opfs`, and `s3`
services. `memory` is used by the hermetic browser canary. OPFS and S3 require
the browser facilities and configuration documented by OpenDAL.

## Build and test

The generated glue must match the Rust `wasm-bindgen` dependency exactly:

```sh
cargo install --locked wasm-bindgen-cli --version 0.2.127
rustup target add wasm32-unknown-unknown
make browser-js-canary
```

`make browser-js-canary` builds the bridge, generates ES-module glue, serves the
canary from localhost, and runs it in headless Chrome. Node is only the test
launcher; OpenDAL operations run in the browser.

## Safety boundaries

- Every bridge resource has a generation-checked handle and an explicit owner.
- Cancellation is logical: it makes a late result inert, but cannot promise that
  an already-dispatched remote request was stopped.
- Buffer and listing sizes are bounded before allocation or cross-memory copy.
- Invalid UTF-8, malformed snapshots, stale handles, and double consumption are
  reported as structured errors.
- Operator construction errors do not echo backend configuration values.
