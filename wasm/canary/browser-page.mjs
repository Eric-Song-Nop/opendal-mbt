import bridgeInitializer from "/bridge.mjs";
import { loadOpenDalMoonBit } from "/loader.mjs";

const token = new URL(location.href).searchParams.get("token");
let reported = false;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function nextTurn() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

async function waitFor(read, expected, description) {
  const deadline = performance.now() + 10_000;
  while (performance.now() < deadline) {
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

async function report(payload) {
  if (reported) {
    return;
  }
  reported = true;
  await fetch(`/result?token=${encodeURIComponent(token ?? "")}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
}

window.addEventListener("error", (event) => {
  void report({ ok: false, error: event.error?.stack ?? event.message });
});
window.addEventListener("unhandledrejection", (event) => {
  void report({ ok: false, error: event.reason?.stack ?? String(event.reason) });
});

let runtime;
try {
  runtime = await loadOpenDalMoonBit({
    bridgeInitializer,
    bridge: new URL("/bridge.wasm", location.href),
    moonbit: new URL("/moonbit.wasm", location.href),
  });

  const exports = runtime.exports;
  const bulkTransferBytes = 16 * 1024 * 1024;
  const bulkTransferChunks = bulkTransferBytes / (256 * 1024);
  const transferBefore = runtime.transfer.stats();
  const moonMemoryBefore = runtime.moonbit.exports.memory.buffer;
  const bridgeMemoryBefore = runtime.bridge.exports.memory.buffer;
  assert(
    exports.opendal_mbt_wasm_canary_bulk_roundtrip() === 0,
    "16 MiB bulk transfer canary failed",
  );
  const transferAfter = runtime.transfer.stats();
  assert(
    transferAfter.moonToBridgeCalls - transferBefore.moonToBridgeCalls ===
      bulkTransferChunks + 3,
    "Moon-to-bridge calls did not scale with the 256 KiB chunk count",
  );
  assert(
    transferAfter.bridgeToMoonCalls - transferBefore.bridgeToMoonCalls ===
      bulkTransferChunks,
    "bridge-to-Moon calls did not scale with the 256 KiB chunk count",
  );
  assert(
    transferAfter.moonToBridgeBytes - transferBefore.moonToBridgeBytes ===
      bulkTransferBytes + 36,
    "Moon-to-bridge bulk byte count was not exact",
  );
  assert(
    transferAfter.bridgeToMoonBytes - transferBefore.bridgeToMoonBytes ===
      bulkTransferBytes,
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
  const liveBeforeOversize =
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count();
  runtime.bridge.exports.opendal_mbt_wasm_last_error_clear();
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_buffer_new_sized(
      64 * 1024 * 1024 + 1,
    ) === 0,
    "bridge accepted a buffer above the 64 MiB materialization limit",
  );
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_last_error_code() === 3,
    "oversized allocation did not report BufferTooLarge",
  );
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() ===
      liveBeforeOversize,
    "oversized allocation published a partial buffer handle",
  );
  runtime.bridge.exports.opendal_mbt_wasm_last_error_clear();

  const pendingBefore = exports.opendal_mbt_wasm_canary_pending_poll_count();
  let heartbeat = false;
  const heartbeatPromise = new Promise((resolve) => {
    setTimeout(() => {
      heartbeat = true;
      resolve();
    }, 0);
  });

  assert(
    exports.opendal_mbt_wasm_canary_async_start() === 1,
    "async canary did not start in the create-dir-pending stage",
  );
  assert(
    exports.opendal_mbt_wasm_canary_async_stage() === 1,
    "async canary completed synchronously",
  );

  await Promise.resolve();
  assert(
    exports.opendal_mbt_wasm_canary_pending_poll_count() === pendingBefore + 1,
    "the Rust future did not return Pending on its first poll",
  );
  assert(!heartbeat, "browser heartbeat ran before the first pending poll");
  assert(
    exports.opendal_mbt_wasm_canary_async_stage() === 1,
    "create_dir became ready in the first microtask",
  );

  await heartbeatPromise;
  assert(heartbeat, "browser heartbeat did not run");
  assert(
    exports.opendal_mbt_wasm_canary_async_stage() === 1,
    "create_dir completed before the previously queued browser heartbeat",
  );

  await waitFor(
    () => exports.opendal_mbt_wasm_canary_async_stage(),
    9,
    "asynchronous OpenDAL memory lifecycle",
  );
  assert(
    exports.opendal_mbt_wasm_canary_pending_poll_count() === pendingBefore + 8,
    "not every create/write/read/stat/list/delete/NotFound task observed Pending",
  );

  assert(
    exports.opendal_mbt_wasm_canary_cancel_start() === 0,
    "cancel-before-ready canary did not start",
  );
  const diagnosticBuffer =
    runtime.bridge.exports.opendal_mbt_wasm_buffer_new();
  assert(diagnosticBuffer !== 0, "could not allocate diagnostic buffer");
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_buffer_push(
      diagnosticBuffer,
      256,
    ) === 5,
    "could not seed the sticky diagnostic",
  );
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_buffer_release(
      diagnosticBuffer,
    ) === 0,
    "could not release diagnostic buffer",
  );
  await nextTurn();
  await nextTurn();
  await nextTurn();
  assert(
    exports.opendal_mbt_wasm_canary_cancel_callback_count() === 0,
    "a cancelled task delivered its user callback",
  );
  assert(
    exports.opendal_mbt_wasm_canary_cancel_is_clean() === 1,
    "cancel-before-ready left live resource handles",
  );
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_last_error_code() === 5,
    "an orphan scheduler poll overwrote the caller's sticky diagnostic",
  );
  runtime.bridge.exports.opendal_mbt_wasm_last_error_clear();

  const readyPendingBefore =
    exports.opendal_mbt_wasm_canary_pending_poll_count();
  const readyLiveBefore =
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count();
  assert(
    exports.opendal_mbt_wasm_canary_ready_start() === 1,
    "completion-wins canary did not start pending",
  );
  assert(
    exports.opendal_mbt_wasm_canary_ready_status() === 1,
    "completion-wins canary completed synchronously",
  );
  await Promise.resolve();
  assert(
    exports.opendal_mbt_wasm_canary_pending_poll_count() ===
      readyPendingBefore + 1,
    "completion-wins task did not observe Pending",
  );
  assert(
    exports.opendal_mbt_wasm_canary_ready_status() === 1,
    "completion-wins task became ready in the first microtask",
  );
  await waitFor(
    () => exports.opendal_mbt_wasm_canary_ready_status(),
    2,
    "completion-wins task",
  );
  assert(
    exports.opendal_mbt_wasm_canary_ready_cancel() === 2,
    "cancel rolled a completed operation back",
  );
  assert(
    exports.opendal_mbt_wasm_canary_ready_close() === 4,
    "close did not make a completed operation Closed",
  );
  assert(
    exports.opendal_mbt_wasm_canary_ready_cancel() === 4 &&
      exports.opendal_mbt_wasm_canary_ready_close() === 4,
    "repeated cancel/close changed a Closed operation",
  );
  await nextTurn();
  assert(
    exports.opendal_mbt_wasm_canary_ready_status() === 4,
    "a late callback changed a Closed operation",
  );
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() ===
      readyLiveBefore,
    "completion-wins lifecycle left live handles",
  );

  const multiPendingBefore =
    exports.opendal_mbt_wasm_canary_pending_poll_count();
  const multiLiveBefore =
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count();
  assert(
    exports.opendal_mbt_wasm_canary_multi_start() === 1,
    "multi-operator canary did not start pending",
  );
  assert(
    exports.opendal_mbt_wasm_canary_multi_stage() === 1,
    "multi-operator canary completed synchronously",
  );
  await Promise.resolve();
  assert(
    exports.opendal_mbt_wasm_canary_pending_poll_count() ===
      multiPendingBefore + 2,
    "two concurrent operators did not both observe Pending",
  );
  await waitFor(
    () => exports.opendal_mbt_wasm_canary_multi_stage(),
    2,
    "two concurrent operator reads",
  );
  await nextTurn();
  assert(
    exports.opendal_mbt_wasm_canary_multi_stage() === 2,
    "multi-operator callback was delivered more than once",
  );
  assert(
    exports.opendal_mbt_wasm_canary_pending_poll_count() ===
      multiPendingBefore + 2,
    "multi-operator tasks were polled as new work more than once",
  );
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() ===
      multiLiveBefore,
    "multi-operator lifecycle left live handles",
  );

  const disposePendingBefore =
    exports.opendal_mbt_wasm_canary_pending_poll_count();
  assert(
    exports.opendal_mbt_wasm_canary_dispose_start() === 1,
    "pending-dispose canary did not start",
  );
  await Promise.resolve();
  assert(
    exports.opendal_mbt_wasm_canary_pending_poll_count() ===
      disposePendingBefore + 1,
    "pending-dispose task did not observe Pending",
  );
  assert(
    exports.opendal_mbt_wasm_canary_dispose_callback_count() === 0,
    "pending-dispose callback ran before teardown",
  );
  runtime.dispose();
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() === 0,
    "bridge teardown left live resource handles",
  );
  await nextTurn();
  await nextTurn();
  await nextTurn();
  assert(
    exports.opendal_mbt_wasm_canary_dispose_callback_count() === 0,
    "late completion delivered after runtime disposal",
  );
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() === 0,
    "late completion revived bridge resources after teardown",
  );
  await report({
    ok: true,
    pendingTasks: 12,
    heartbeat: true,
    cancellation: "suppressed",
    diagnostics: "isolated",
    completionWins: true,
    concurrentOperators: 2,
    pendingDispose: "inert",
    bulkTransferBytes,
    bulkTransferChunks,
  });
} catch (error) {
  runtime?.dispose();
  await report({ ok: false, error: error?.stack ?? String(error) });
}
