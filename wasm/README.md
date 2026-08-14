# Browser backend maintainer index

This directory owns the Rust WebAssembly and JavaScript Promise implementation
used when the root MoonBit package is compiled with `--target js`. The internal
Rust bridge being Wasm does not make MoonBit's `wasm` or `wasm-gc` targets
supported.

Applications should start with the
[Browser and JavaScript API reference](../docs/reference/browser-api.md).
Maintainers should read the complete
[Browser Runtime and Wasm ABI contract](../docs/design/browser-runtime.md)
before changing the bridge, task lifecycle, snapshots, bounds, or embedded
distribution.

```text
MoonBit async API -> JavaScript Promise/AbortSignal facade
                     -> scalar Browser Wasm ABI 1.7 -> OpenDAL
```

The current bridge registers the browser-compatible `memory`, `opfs`, and `s3`
services. The ABI, runtime, wasm-bindgen glue, and generated MoonBit payload are
one version-matched unit.

## Source map

| Path | Owner and purpose |
| --- | --- |
| [`browser-bridge/src/lib.rs`](browser-bridge/src/lib.rs) | Rust OpenDAL bridge, scalar exports, arena, tasks, snapshots, state machines, and adversarial unit tests |
| [`browser-bridge/Cargo.toml`](browser-bridge/Cargo.toml) | Exact bridge dependencies and browser service features |
| [`browser-runtime/index.mjs`](browser-runtime/index.mjs) | Promise facade, ABI validation, copying, structured outcomes, cancellation, and JavaScript resource wrappers |
| [`browser-runtime/contract.json`](browser-runtime/contract.json) | Machine-readable release/canary mirror of version, ABI, features, services, limits, snapshots, and local errors |
| [`browser-canary/`](browser-canary/) | Local server and real-Chrome acceptance page for the rebuilt module-form bridge |
| [`../scripts/generate-browser-embed.mjs`](../scripts/generate-browser-embed.mjs) | Deterministic generator and semantic freshness check for the embedded distribution |
| [`../src/browser/embedded_runtime.generated.mbt`](../src/browser/embedded_runtime.generated.mbt) | Checked-in generated Wasm, wasm-bindgen glue, and Promise runtime; never edit by hand |
| [`../src/browser_demo/`](../src/browser_demo/) | Packaged, one-command MoonBit consumer launched in real Chrome |
| [`../examples/browser/`](../examples/browser/) | Runnable portable async example with one native command and one real-browser command |

`contract.json` is not a substitute for the executable constants. Any contract
change must update the Rust bridge, Promise runtime, machine-readable mirror,
tests, and generated payload together.

## Required tools

The wasm-bindgen CLI must match the resolved Rust dependency exactly:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.127
wasm-bindgen --version
```

The final command must print `wasm-bindgen 0.2.127`. The Makefile rejects a
different version before building either distribution form.

A Chrome or Chromium executable is required for the canary, browser demo, and
packaged-browser proof. Set `CHROME_BIN` if it is not at a recognized platform
location.

## Maintainer workflow

Check MoonBit and Rust independently while editing:

```sh
make moon-deps
make moon-browser-check
make moon-browser-test
make browser-rust-check
make browser-rust-test
```

Then run the separately served bridge in real Chrome:

```sh
make browser-js-canary RUST_PROFILE=release
```

Node only launches Chrome and serves local files. OpenDAL operations execute in
the browser.

After changing the bridge, runtime, lockfile, generator, tool pin, Makefile
inputs, or binding version, regenerate and verify the embedded source:

```sh
make browser-embed-generate
git diff -- src/browser/embedded_runtime.generated.mbt
make browser-embed-check
```

Review the generated version/hash header and payload diff. The freshness check
allows host-specific Rust/LLVM or deflate bytes only after proving the source
fingerprint, generated layout, and public Wasm interface still match.

Finally verify both the repository demo and the clean published-package shape:

```sh
make portable-async-example-browser
make browser-demo
make packaged-browser
```

The portable example runs the same MoonBit function used by native in real
Chrome. `packaged-browser` hides Cargo, Rust, wasm-bindgen, npm, and common
bundlers and rejects separately shipped `.wasm`/`.mjs` assets. Passing it
proves that the published package can run the embedded demo with one Moon
command and a browser already installed.

## Invariants to preserve

- Browser ABI `0x0001_0007` and required feature mask `0x0000_fffc` match the
  Rust bridge, Promise runtime, generated header, and `contract.json`.
- Every owned scalar handle has one owner and matching release operation;
  stale generations and double consumption are rejected.
- Storage failures resolve as structured outcomes. Promise rejection is
  reserved for loading or contract corruption.
- Cancellation through `AbortSignal` makes late completion inert and
  terminalizes an affected stateful child, but cannot promise remote rollback.
- Whole buffers, transfer chunks, listings, and configuration are bounded
  before allocation or cross-memory copy.
- Configuration values and credentials never appear in construction errors.
- `Runtime::close` is idempotent, releases the arena, and prevents late tasks
  from reviving state.
- Embedded runtimes own fresh Wasm instances. Browser ESM caching means an
  explicit bridge module URL is loaded at most once per page and its returned
  runtime is shared between operators.

Release candidates must also follow the
[Browser release checklist](../docs/releasing.md#browser-release-checklist).
