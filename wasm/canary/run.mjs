import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { loadOpenDalMoonBit } from "../loader/index.mjs";

const expectedAbiVersion = 0x0001_0000;

async function main() {
  const [bridgePath, moonbitPath] = process.argv.slice(2);
  if (!bridgePath || !moonbitPath) {
    throw new Error(
      "usage: node wasm/canary/run.mjs <rust-bridge.wasm> <moonbit-canary.wasm>",
    );
  }

  const runtime = await loadOpenDalMoonBit({
    bridge: await readFile(resolve(bridgePath)),
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
}

await main();
