import initializeBridge from "/opendal_mbt_browser_bridge.mjs";
import { loadOpenDalBrowser } from "/browser-runtime.mjs";

const token = new URL(location.href).searchParams.get("token");
const statusElement = document.querySelector("#status");
let reported = false;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function nextTurn() {
  return new Promise((resolveTurn) => setTimeout(resolveTurn, 0));
}

function expectOk(outcome, description) {
  assert(
    outcome !== null && typeof outcome === "object",
    `${description} did not return an Outcome object`,
  );
  if (outcome.ok !== true) {
    const detail = outcome.error?.message ?? JSON.stringify(outcome.error);
    throw new Error(`${description} failed: ${detail}`);
  }
  return outcome.value;
}

function expectError(outcome, description) {
  assert(
    outcome !== null && typeof outcome === "object" && outcome.ok === false,
    `${description} did not return a failed Outcome`,
  );
  assert(
    outcome.error !== null && typeof outcome.error === "object",
    `${description} did not return a structured error`,
  );
  return outcome.error;
}

function assertBytesEqual(actual, expected, description) {
  assert(actual instanceof Uint8Array, `${description} was not a Uint8Array`);
  assert(
    actual.length === expected.length,
    `${description} length was ${actual.length}, expected ${expected.length}`,
  );
  for (let index = 0; index < expected.length; index += 1) {
    assert(
      actual[index] === expected[index],
      `${description} differed at byte ${index}: ` +
        `${actual[index]} != ${expected[index]}`,
    );
  }
}

async function observePending(promise, bridge, pendingBefore, description) {
  assert(
    promise !== null && typeof promise.then === "function",
    `${description} did not return a Promise`,
  );
  let settled = false;
  promise.then(
    () => {
      settled = true;
    },
    () => {
      settled = true;
    },
  );
  assert(!settled, `${description} settled in its initiating stack`);
  await Promise.resolve();
  assert(
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count() ===
      pendingBefore + 1,
    `${description} did not observe the forced Pending poll`,
  );
  assert(!settled, `${description} settled in the first microtask`);
}

async function pendingOk(start, bridge, description) {
  const pendingBefore =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const promise = start();
  await observePending(promise, bridge, pendingBefore, description);
  return expectOk(await promise, description);
}

async function pendingError(start, bridge, description) {
  const pendingBefore =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const promise = start();
  await observePending(promise, bridge, pendingBefore, description);
  return expectError(await promise, description);
}

async function report(payload) {
  if (reported) {
    return;
  }
  reported = true;
  statusElement.textContent = payload.ok ? "passed" : `failed\n${payload.error}`;
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

let bridge;
let runtime;
let operator;
let operatorClosed = false;
let runtimeClosed = false;
try {
  assert(token !== null && token.length !== 0, "runner token is missing");
  const bridgeWasm = new URL(
    "/opendal_mbt_browser_bridge_bg.wasm",
    location.href,
  );
  bridge = await initializeBridge({ module_or_path: bridgeWasm });
  runtime = await loadOpenDalBrowser({ bridge });

  assert(
    typeof bridge.opendal_mbt_wasm_live_handle_count === "function" &&
      typeof bridge.opendal_mbt_wasm_canary_set_force_pending === "function" &&
      typeof bridge.opendal_mbt_wasm_canary_forced_pending_poll_count ===
        "function",
    "bridge does not expose the browser acceptance hooks",
  );
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 0,
    "runtime initialization left live bridge handles",
  );

  const schemes = expectOk(runtime.availableSchemes(), "availableSchemes");
  assert(Array.isArray(schemes), "availableSchemes value was not an array");
  assert(schemes.includes("memory"), "browser artifact did not register memory");

  operator = expectOk(
    runtime.operator("memory", { root: "browser-canary" }),
    "memory operator construction",
  );
  const info = expectOk(operator.info(), "operator info");
  assert(info.scheme === "memory", `operator scheme was ${info.scheme}`);
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 1,
    "memory operator did not own exactly one bridge handle",
  );

  assert(
    bridge.opendal_mbt_wasm_canary_set_force_pending(1) === 0,
    "could not enable the forced-Pending browser hook",
  );
  const pendingAtStart =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();

  const directory = "canary/";
  const path = `${directory}binary.bin`;
  const missingPath = `${directory}missing.bin`;
  const payload = Uint8Array.of(
    0x00,
    0xff,
    0xfe,
    0x80,
    0x41,
    0x00,
    0xc3,
    0x28,
    0x7f,
    0x01,
    0x00,
  );

  await pendingOk(
    () => operator.createDir(directory, {}),
    bridge,
    "createDir",
  );
  const writeMetadata = await pendingOk(
    () => operator.write(path, payload, {}),
    bridge,
    "write",
  );
  assert(
    typeof writeMetadata.contentLength === "bigint" &&
      writeMetadata.contentLength === BigInt(payload.length),
    "write metadata did not preserve UInt64 content length as BigInt",
  );

  const readBytes = await pendingOk(
    () => operator.read(path, {}),
    bridge,
    "read",
  );
  assertBytesEqual(readBytes, payload, "binary read");

  const statMetadata = await pendingOk(
    () => operator.stat(path, {}),
    bridge,
    "stat",
  );
  assert(
    statMetadata.mode === "File",
    `stat metadata mode was ${statMetadata.mode}`,
  );
  assert(
    typeof statMetadata.contentLength === "bigint" &&
      statMetadata.contentLength === BigInt(payload.length),
    "stat metadata did not preserve UInt64 content length as BigInt",
  );

  const entries = await pendingOk(
    () =>
      operator.list(directory, {
        recursive: false,
        limit: 16n,
        startAfter: undefined,
      }),
    bridge,
    "list",
  );
  assert(Array.isArray(entries), "list value was not an array");
  const fileEntry = entries.find((entry) => entry.path === path);
  assert(fileEntry !== undefined, `list did not contain ${path}`);
  assert(fileEntry.name === "binary.bin", `listed name was ${fileEntry.name}`);
  assert(
    fileEntry.metadata?.contentLength === BigInt(payload.length),
    "listed metadata content length was not exact",
  );

  const notFound = await pendingError(
    () => operator.read(missingPath, {}),
    bridge,
    "missing read",
  );
  assert(notFound.kind === "NotFound", `error kind was ${notFound.kind}`);
  assert(notFound.kindCode === 4, `NotFound kindCode was ${notFound.kindCode}`);
  assert(
    notFound.status === "Permanent",
    `NotFound status was ${notFound.status}`,
  );
  assert(notFound.operation === "Read", `error operation was ${notFound.operation}`);
  assert(notFound.path === missingPath, `error path was ${notFound.path}`);
  assert(
    typeof notFound.message === "string",
    "NotFound error message was not an owned string",
  );

  const abortController = new AbortController();
  const pendingBeforeCancel =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const cancelledPromise = operator.read(path, {
    signal: abortController.signal,
  });
  await observePending(
    cancelledPromise,
    bridge,
    pendingBeforeCancel,
    "cancelled read",
  );
  abortController.abort();
  const cancelled = expectError(await cancelledPromise, "cancelled read");
  assert(cancelled.kind === "Cancelled", `cancel kind was ${cancelled.kind}`);
  assert(
    cancelled.kindCode === 0x1005,
    `cancel kindCode was ${cancelled.kindCode}`,
  );
  assert(
    cancelled.status === "Permanent",
    `cancel status was ${cancelled.status}`,
  );
  assert(cancelled.operation === "Read", `cancel operation was ${cancelled.operation}`);
  assert(cancelled.path === path, `cancel path was ${cancelled.path}`);
  assert(
    typeof cancelled.message === "string" &&
      !cancelled.message.includes("browser-canary") &&
      !cancelled.message.includes("binary.bin"),
    "cancel error leaked operator configuration or object data",
  );
  await nextTurn();
  await nextTurn();
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 1,
    "cancelled operation left a live task, completion, or buffer handle",
  );

  await pendingOk(
    () => operator.delete(path, { recursive: false, version: undefined }),
    bridge,
    "delete",
  );
  const afterDelete = await pendingError(
    () => operator.read(path, {}),
    bridge,
    "read after delete",
  );
  assert(afterDelete.kind === "NotFound", "delete did not remove the object");

  const pendingTasks =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count() - pendingAtStart;
  assert(
    pendingTasks === 9,
    `expected 9 forced-Pending operations, observed ${pendingTasks}`,
  );

  expectOk(operator.close(), "operator close");
  operatorClosed = true;
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 0,
    "operator close left live bridge handles",
  );
  expectOk(runtime.close(), "runtime close");
  runtimeClosed = true;
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 0,
    "runtime close left live bridge handles",
  );

  await nextTurn();
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 0,
    "late completion revived bridge resources",
  );
  await report({
    ok: true,
    service: "memory",
    pendingTasks,
    binaryBytes: payload.length,
    structuredNotFound: true,
    abortSignal: "cancelled",
    liveHandles: 0,
  });
} catch (error) {
  if (operator && !operatorClosed) {
    try {
      operator.close();
    } catch {
      // Preserve the primary acceptance failure.
    }
  }
  if (runtime && !runtimeClosed) {
    try {
      runtime.close();
    } catch {
      // Preserve the primary acceptance failure.
    }
  }
  const liveHandles = (() => {
    try {
      return bridge?.opendal_mbt_wasm_live_handle_count?.();
    } catch {
      return undefined;
    }
  })();
  await report({
    ok: false,
    error: error?.stack ?? String(error),
    liveHandles,
  });
}
