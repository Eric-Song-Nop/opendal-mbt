import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { loadOpenDalMoonBit } from "../loader/index.mjs";

const expectedAbiVersion = 0x0001_0000;

async function readProcessedBridge(path) {
  const bytes = await readFile(resolve(path));
  const module = await WebAssembly.compile(bytes);
  const rawImports = WebAssembly.Module.imports(module).filter(({ module }) =>
    module.startsWith("__wbindgen_"),
  );
  if (rawImports.length !== 0) {
    throw new Error("Rust bridge was not processed by wasm-bindgen");
  }
  return bytes;
}

async function main() {
  const [bridgeGluePath, bridgePath, moonbitPath] = process.argv.slice(2);
  if (!bridgeGluePath || !bridgePath || !moonbitPath) {
    throw new Error(
      "usage: node wasm/canary/run.mjs " +
        "<rust-bridge.mjs> <rust-bridge.wasm> <moonbit-canary.wasm>",
    );
  }

  const bridgeGlue = await import(
    pathToFileURL(resolve(bridgeGluePath)).href,
  );
  const runtime = await loadOpenDalMoonBit({
    bridgeInitializer: bridgeGlue.default,
    bridge: await readProcessedBridge(bridgePath),
    moonbit: await readFile(resolve(moonbitPath)),
  });
  const version = runtime.exports.opendal_mbt_wasm_canary_bridge_version();
  if (version !== expectedAbiVersion) {
    throw new Error(
      `bridge ABI mismatch: expected 0x${expectedAbiVersion.toString(16)}, ` +
        `found 0x${version.toString(16)}`,
    );
  }
  const staleRejected =
    runtime.bridge.exports.opendal_mbt_wasm_canary_stale_handle_rejected();
  if (staleRejected !== 1) {
    throw new Error("bridge accepted a released handle after slot reuse");
  }
  const status = runtime.exports.opendal_mbt_wasm_canary_roundtrip();
  if (status !== 0) {
    throw new Error(`OpenDAL MoonBit Wasm canary failed with status ${status}`);
  }
  runtime.dispose();
  if (runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() !== 0) {
    throw new Error("bridge teardown left live resource handles");
  }
}

await main();
