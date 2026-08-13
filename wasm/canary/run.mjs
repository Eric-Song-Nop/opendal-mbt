import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { loadOpenDalMoonBit } from "../loader/index.mjs";

const expectedAbiVersion = 0x0001_0004;
const bulkTransferBytes = 16 * 1024 * 1024;
const bulkTransferChunks = bulkTransferBytes / (256 * 1024);

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertBulkTransfer(runtime) {
  const before = runtime.transfer.stats();
  const moonMemoryBefore = runtime.moonbit.exports.memory.buffer;
  const bridgeMemoryBefore = runtime.bridge.exports.memory.buffer;
  const status = runtime.exports.opendal_mbt_wasm_canary_bulk_roundtrip();
  assert(status === 0, `16 MiB bulk canary failed with status ${status}`);
  const after = runtime.transfer.stats();
  assert(
    after.moonToBridgeCalls - before.moonToBridgeCalls ===
      bulkTransferChunks + 3,
    "Moon-to-bridge calls did not scale with the 256 KiB chunk count",
  );
  assert(
    after.bridgeToMoonCalls - before.bridgeToMoonCalls === bulkTransferChunks,
    "bridge-to-Moon calls did not scale with the 256 KiB chunk count",
  );
  assert(
    after.moonToBridgeBytes - before.moonToBridgeBytes ===
      bulkTransferBytes + 36,
    "Moon-to-bridge bulk byte count was not exact",
  );
  assert(
    after.bridgeToMoonBytes - before.bridgeToMoonBytes === bulkTransferBytes,
    "bridge-to-Moon bulk byte count was not exact",
  );
  assert(
    runtime.moonbit.exports.memory.buffer !== moonMemoryBefore,
    "16 MiB transfer did not exercise MoonBit memory.grow",
  );
  assert(
    runtime.bridge.exports.memory.buffer !== bridgeMemoryBefore,
    "16 MiB transfer did not exercise bridge memory.grow",
  );

  const bridge = runtime.bridge.exports;
  const liveBefore = bridge.opendal_mbt_wasm_live_handle_count();
  bridge.opendal_mbt_wasm_last_error_clear();
  assert(
    bridge.opendal_mbt_wasm_buffer_new_sized(64 * 1024 * 1024 + 1) === 0,
    "bridge accepted a buffer above the 64 MiB materialization limit",
  );
  assert(
    bridge.opendal_mbt_wasm_last_error_code() === 3,
    "oversized allocation did not report BufferTooLarge",
  );
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === liveBefore,
    "oversized allocation published a partial buffer handle",
  );
  bridge.opendal_mbt_wasm_last_error_clear();
}

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
  assertBulkTransfer(runtime);
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
