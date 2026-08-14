import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { loadOpenDalMoonBit } from "../loader/index.mjs";

const expectedAbiVersion = 0x0001_0007;
const expectedFeatureFlags = 0x0000_0fff;
const bulkTransferBytes = 16 * 1024 * 1024;
const bulkTransferChunks = bulkTransferBytes / (256 * 1024);
const metadataSnapshotBytes = 84;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function nextTurn() {
  return new Promise((resolveNextTurn) => setTimeout(resolveNextTurn, 0));
}

async function waitFor(read, expected, description) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const actual = read();
    if (actual === expected) {
      return;
    }
    if (actual < 0) {
      throw new Error(`${description} failed with state ${actual}`);
    }
    await nextTurn();
  }
  throw new Error(`timed out waiting for ${description}; found ${read()}`);
}

async function assertBulkTransfer(runtime) {
  const bridge = runtime.bridge.exports;
  const liveBefore = bridge.opendal_mbt_wasm_live_handle_count();
  const before = runtime.transfer.stats();
  const moonMemoryBefore = runtime.moonbit.exports.memory.buffer;
  const bridgeMemoryBefore = runtime.bridge.exports.memory.buffer;
  const status = runtime.exports.opendal_mbt_wasm_canary_bulk_start();
  assert(status === 1, `16 MiB bulk canary failed to start: ${status}`);
  assert(
    runtime.exports.opendal_mbt_wasm_canary_bulk_stage() === 1,
    "16 MiB bulk canary completed synchronously",
  );
  await waitFor(
    () => runtime.exports.opendal_mbt_wasm_canary_bulk_stage(),
    3,
    "16 MiB asynchronous bulk transfer",
  );
  const infoTransferBytes =
    runtime.exports.opendal_mbt_wasm_canary_bulk_info_transfer_bytes();
  assert(infoTransferBytes > 0, "operator info transfer byte count was empty");
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === liveBefore,
    "16 MiB bulk canary left live resource handles",
  );
  const after = runtime.transfer.stats();
  assert(
    after.moonToBridgeCalls - before.moonToBridgeCalls ===
      bulkTransferChunks + 3,
    "Moon-to-bridge calls did not scale with the 256 KiB chunk count",
  );
  assert(
    after.bridgeToMoonCalls - before.bridgeToMoonCalls ===
      bulkTransferChunks + 4,
    "bridge-to-Moon calls did not scale with the 256 KiB chunk count",
  );
  assert(
    after.moonToBridgeBytes - before.moonToBridgeBytes ===
      bulkTransferBytes + 36,
    "Moon-to-bridge bulk byte count was not exact",
  );
  assert(
    after.bridgeToMoonBytes - before.bridgeToMoonBytes ===
      bulkTransferBytes + infoTransferBytes + metadataSnapshotBytes,
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

async function assertAsyncLifecycle(runtime, iteration) {
  const bridge = runtime.bridge.exports;
  const liveBefore = bridge.opendal_mbt_wasm_live_handle_count();
  const status = runtime.exports.opendal_mbt_wasm_canary_async_start();
  assert(
    status === 1,
    `asynchronous lifecycle ${iteration} failed to start: ${status}`,
  );
  assert(
    runtime.exports.opendal_mbt_wasm_canary_async_stage() === 1,
    `asynchronous lifecycle ${iteration} completed synchronously`,
  );
  await waitFor(
    () => runtime.exports.opendal_mbt_wasm_canary_async_stage(),
    13,
    `asynchronous OpenDAL lifecycle ${iteration}`,
  );
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === liveBefore,
    `asynchronous lifecycle ${iteration} left live resource handles`,
  );
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
  const version = runtime.bridge.exports.opendal_mbt_wasm_abi_version();
  if (version !== expectedAbiVersion) {
    throw new Error(
      `bridge ABI mismatch: expected 0x${expectedAbiVersion.toString(16)}, ` +
        `found 0x${version.toString(16)}`,
    );
  }
  const features = runtime.bridge.exports.opendal_mbt_wasm_feature_flags();
  if (features !== expectedFeatureFlags) {
    throw new Error(
      `bridge feature mismatch: expected 0x${expectedFeatureFlags.toString(16)}, ` +
        `found 0x${features.toString(16)}`,
    );
  }
  const staleRejected =
    runtime.bridge.exports.opendal_mbt_wasm_canary_stale_handle_rejected();
  if (staleRejected !== 1) {
    throw new Error("bridge accepted a released handle after slot reuse");
  }
  await assertBulkTransfer(runtime);
  for (let iteration = 1; iteration <= 2; iteration += 1) {
    await assertAsyncLifecycle(runtime, iteration);
  }
  runtime.dispose();
  if (runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() !== 0) {
    throw new Error("bridge teardown left live resource handles");
  }
}

await main();
