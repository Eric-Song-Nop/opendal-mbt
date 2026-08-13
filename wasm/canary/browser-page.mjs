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

  runtime.dispose();
  assert(
    runtime.bridge.exports.opendal_mbt_wasm_live_handle_count() === 0,
    "bridge teardown left live resource handles",
  );
  await report({
    ok: true,
    pendingTasks: 8,
    heartbeat: true,
    cancellation: "suppressed",
    diagnostics: "isolated",
  });
} catch (error) {
  runtime?.dispose();
  await report({ ok: false, error: error?.stack ?? String(error) });
}
