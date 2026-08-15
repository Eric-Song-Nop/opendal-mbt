import initializeBridge from "/opendal_mbt_browser_bridge.mjs";
import { loadOpenDalBrowser } from "/browser-runtime.mjs";

const token = new URL(location.href).searchParams.get("token");
const statusElement = document.querySelector("#status");
const SCALAR_CHUNK_BYTES = 256 * 1024;
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

function concatenateBytes(chunks) {
  const length = chunks.reduce((total, chunk) => total + chunk.length, 0);
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

function assertCancelled(error, operation, path, description) {
  assert(error.kind === "Cancelled", `${description} kind was ${error.kind}`);
  assert(error.kindCode === 0x1005, `${description} kindCode was ${error.kindCode}`);
  assert(error.status === "Permanent", `${description} status was ${error.status}`);
  assert(error.operation === operation, `${description} operation was ${error.operation}`);
  assert(error.path === path, `${description} path was ${error.path}`);
  assert(
    typeof error.message === "string" &&
      !error.message.includes("browser-canary") &&
      !error.message.includes(path),
    `${description} leaked operator configuration or object data`,
  );
}

function assertResourceClosed(outcome, description) {
  const error = expectError(outcome, description);
  assert(error.kind === "ResourceClosed", `${description} kind was ${error.kind}`);
  assert(error.kindCode === 0x1002, `${description} kindCode was ${error.kindCode}`);
  return error;
}

function assertResourceBusy(outcome, description) {
  const error = expectError(outcome, description);
  assert(error.kind === "ResourceBusy", `${description} kind was ${error.kind}`);
  assert(error.kindCode === 0x1006, `${description} kindCode was ${error.kindCode}`);
  return error;
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
  const contractResponse = await fetch("/browser-contract.json");
  assert(contractResponse.ok, "browser contract could not be loaded");
  const contract = await contractResponse.json();
  bridge = await initializeBridge({ module_or_path: bridgeWasm });
  assert(
    bridge.opendal_mbt_wasm_abi_version() === contract.abiVersion,
    "browser contract ABI version drifted from the bridge",
  );
  assert(
    (bridge.opendal_mbt_wasm_feature_flags() & contract.requiredFeatureFlags) ===
      contract.requiredFeatureFlags,
    "browser contract feature flags drifted from the bridge",
  );
  assert(
    contract.limits?.transferChunkBytes === SCALAR_CHUNK_BYTES &&
      contract.localErrorKinds?.resourceBusy === 0x1006,
    "browser contract scalar limits or local error kinds drifted",
  );
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
    info.capability.canCopy === false && info.capability.canRename === false,
    "memory unexpectedly advertised native copy or rename support",
  );
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
  assertCancelled(cancelled, "Read", path, "cancelled read");
  await nextTurn();
  await nextTurn();
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 1,
    "cancelled operation left a live task, completion, or buffer handle",
  );

  const copiedPath = `${directory}copied.bin`;
  const renamedPath = `${directory}renamed.bin`;
  const unsupportedCopy = await pendingError(
    () => operator.copy(path, copiedPath, {}),
    bridge,
    "copy",
  );
  assert(
    unsupportedCopy.kind === "Unsupported" && unsupportedCopy.kindCode === 2,
    "memory copy did not return structured Unsupported",
  );
  assert(
    unsupportedCopy.operation === "Copy" &&
      unsupportedCopy.path === path &&
      unsupportedCopy.destinationPath === copiedPath,
    "copy error lost source or destination context",
  );
  const unsupportedRename = await pendingError(
    () => operator.rename(path, renamedPath, {}),
    bridge,
    "rename",
  );
  assert(
    unsupportedRename.kind === "Unsupported" && unsupportedRename.kindCode === 2,
    "memory rename did not return structured Unsupported",
  );
  assert(
    unsupportedRename.operation === "Rename" &&
      unsupportedRename.path === path &&
      unsupportedRename.destinationPath === renamedPath,
    "rename error lost source or destination context",
  );

  const streamPath = `${directory}stream.bin`;
  const streamPayload = Uint8Array.of(
    0x00,
    0xff,
    0x80,
    0xfe,
    0x41,
    0x00,
    0xc3,
    0x28,
    0x7f,
    0x01,
    0x02,
    0x03,
    0xf5,
    0x00,
    0x10,
    0x11,
    0x12,
    0xed,
    0xa0,
    0x80,
    0x42,
    0x43,
    0x44,
    0x00,
    0x90,
    0x91,
    0x92,
    0x93,
    0x94,
    0x95,
    0x96,
    0x97,
    0x98,
    0x99,
    0xaa,
    0xbb,
    0xcc,
  );
  const writer = await pendingOk(
    () =>
      operator.openWriter(streamPath, {
        contentType: "application/octet-stream",
      }),
    bridge,
    "open writer",
  );
  const firstWritePending =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const firstWrite = writer.write(streamPayload.subarray(0, 9), {});
  await observePending(firstWrite, bridge, firstWritePending, "first writer chunk");
  assertResourceBusy(
    await writer.write(Uint8Array.of(0x55), {}),
    "concurrent writer write",
  );
  expectOk(await firstWrite, "first writer chunk");
  await pendingOk(
    () => writer.write(streamPayload.subarray(9, 23), {}),
    bridge,
    "second writer chunk",
  );
  await pendingOk(
    () => writer.write(streamPayload.subarray(23), {}),
    bridge,
    "third writer chunk",
  );
  const finishedMetadata = await pendingOk(
    () => writer.finish({}),
    bridge,
    "writer finish",
  );
  assert(
    finishedMetadata.contentLength === BigInt(streamPayload.length),
    "writer finish metadata content length was not exact",
  );
  assertResourceClosed(await writer.finish({}), "repeated writer finish");
  expectOk(writer.close(), "finished writer close");

  const chunkSize = 5;
  const reader = await pendingOk(
    () => operator.openReadStream(streamPath, { chunkSize }),
    bridge,
    "open read stream",
  );
  const firstReadPending =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const firstRead = reader.next({});
  await observePending(firstRead, bridge, firstReadPending, "first stream chunk");
  assertResourceBusy(await reader.next({}), "concurrent stream next");
  const readChunks = [expectOk(await firstRead, "first stream chunk")];
  while (true) {
    const chunk = await pendingOk(
      () => reader.next({}),
      bridge,
      "next stream chunk",
    );
    if (chunk === undefined) {
      break;
    }
    readChunks.push(chunk);
  }
  for (const chunk of readChunks) {
    assert(chunk.length > 0, "read stream returned an empty non-EOF chunk");
    assert(
      chunk.length <= chunkSize,
      `read stream chunk exceeded ${chunkSize} bytes`,
    );
  }
  assertBytesEqual(
    concatenateBytes(readChunks),
    streamPayload,
    "multi-chunk stream read",
  );
  assert(
    expectOk(await reader.next({}), "stable read EOF") === undefined,
    "read EOF was not explicit undefined",
  );
  expectOk(reader.close(), "reader close after EOF");

  const boundaryPath = `${directory}boundary.bin`;
  const boundaryPayload = new Uint8Array(SCALAR_CHUNK_BYTES);
  for (let index = 0; index < boundaryPayload.length; index += 1) {
    boundaryPayload[index] = index % 251;
  }
  boundaryPayload[0] = 0x00;
  boundaryPayload[1] = 0xff;
  const boundaryWriter = await pendingOk(
    () => operator.openWriter(boundaryPath, {}),
    bridge,
    "open boundary writer",
  );
  await pendingOk(
    () => boundaryWriter.write(boundaryPayload, {}),
    bridge,
    "write 256 KiB boundary chunk",
  );
  const boundaryMetadata = await pendingOk(
    () => boundaryWriter.finish({}),
    bridge,
    "finish boundary writer",
  );
  assert(
    boundaryMetadata.contentLength === BigInt(SCALAR_CHUNK_BYTES),
    "boundary writer metadata length was not exact",
  );
  const boundaryReader = await pendingOk(
    () =>
      operator.openReadStream(boundaryPath, {
        chunkSize: SCALAR_CHUNK_BYTES,
      }),
    bridge,
    "open boundary reader",
  );
  assertBytesEqual(
    await pendingOk(
      () => boundaryReader.next({}),
      bridge,
      "read 256 KiB boundary chunk",
    ),
    boundaryPayload,
    "256 KiB boundary stream chunk",
  );
  assert(
    (await pendingOk(
      () => boundaryReader.next({}),
      bridge,
      "boundary reader EOF",
    )) === undefined,
    "boundary reader did not return explicit EOF",
  );
  expectOk(boundaryReader.close(), "boundary reader close");
  const oversizedReadOpen = expectError(
    await operator.openReadStream(boundaryPath, {
      chunkSize: SCALAR_CHUNK_BYTES + 1,
    }),
    "oversized read chunk",
  );
  assert(
    oversizedReadOpen.kind === "InvalidArgument" &&
      oversizedReadOpen.kindCode === 0x1001,
    "oversized read chunk did not fail local validation",
  );
  const oversizedWriter = await pendingOk(
    () => operator.openWriter(`${directory}oversized.bin`, {}),
    bridge,
    "open oversized writer",
  );
  const oversizedWrite = expectError(
    await oversizedWriter.write(new Uint8Array(SCALAR_CHUNK_BYTES + 1), {}),
    "oversized writer chunk",
  );
  assert(
    oversizedWrite.kind === "BufferTooLarge" &&
      oversizedWrite.kindCode === 0x1003,
    "oversized writer chunk did not fail local validation",
  );
  await pendingOk(
    () => oversizedWriter.abort({}),
    bridge,
    "abort writer after local validation failure",
  );
  expectOk(oversizedWriter.close(), "oversized writer close");

  const abortedPath = `${directory}aborted.bin`;
  const abortedWriter = await pendingOk(
    () => operator.openWriter(abortedPath, {}),
    bridge,
    "open abort writer",
  );
  await pendingOk(
    () => abortedWriter.write(Uint8Array.of(0x00, 0xff, 0x41), {}),
    bridge,
    "write before abort",
  );
  await pendingOk(
    () => abortedWriter.abort({}),
    bridge,
    "writer abort",
  );
  expectOk(await abortedWriter.abort({}), "idempotent writer abort");
  assertResourceClosed(await abortedWriter.finish({}), "finish aborted writer");
  expectOk(abortedWriter.close(), "aborted writer close");
  const abortedStat = await pendingError(
    () => operator.stat(abortedPath, {}),
    bridge,
    "stat aborted object",
  );
  assert(abortedStat.kind === "NotFound", "writer abort committed partial data");

  const lister = await pendingOk(
    () =>
      operator.openLister(directory, {
        recursive: false,
        limit: 64n,
      }),
    bridge,
    "open lister",
  );
  const streamedEntries = [];
  while (true) {
    const entry = await pendingOk(
      () => lister.next({}),
      bridge,
      "lister next",
    );
    if (entry === undefined) {
      break;
    }
    assert(typeof entry.path === "string", "lister entry path was not text");
    assert(typeof entry.name === "string", "lister entry name was not text");
    assert(entry.metadata?.mode !== undefined, "lister entry omitted metadata");
    streamedEntries.push(entry);
  }
  const streamedPaths = new Set(streamedEntries.map((entry) => entry.path));
  for (const expectedPath of [path, streamPath, boundaryPath]) {
    assert(streamedPaths.has(expectedPath), `streaming lister omitted ${expectedPath}`);
  }
  assert(
    expectOk(await lister.next({}), "stable lister EOF") === undefined,
    "lister EOF was not explicit undefined",
  );
  expectOk(lister.close(), "lister close after EOF");

  const cancelledReader = await pendingOk(
    () => operator.openReadStream(streamPath, { chunkSize: 4 }),
    bridge,
    "open cancellable reader",
  );
  const streamAbortController = new AbortController();
  const pendingBeforeStreamCancel =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const cancelledNext = cancelledReader.next({
    signal: streamAbortController.signal,
  });
  await observePending(
    cancelledNext,
    bridge,
    pendingBeforeStreamCancel,
    "cancelled stream next",
  );
  streamAbortController.abort();
  assertCancelled(
    expectError(await cancelledNext, "cancelled stream next"),
    "ReadStreamNext",
    streamPath,
    "cancelled stream next",
  );
  assertResourceClosed(
    await cancelledReader.next({}),
    "next after stream cancellation",
  );
  expectOk(cancelledReader.close(), "cancelled reader close");

  const cancelledWriterPath = `${directory}cancelled-writer.bin`;
  const cancelledWriter = await pendingOk(
    () => operator.openWriter(cancelledWriterPath, {}),
    bridge,
    "open cancellable writer",
  );
  const writerAbortController = new AbortController();
  const pendingBeforeWriterCancel =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const cancelledWrite = cancelledWriter.write(Uint8Array.of(0x00, 0xff), {
    signal: writerAbortController.signal,
  });
  await observePending(
    cancelledWrite,
    bridge,
    pendingBeforeWriterCancel,
    "cancelled writer write",
  );
  writerAbortController.abort();
  assertCancelled(
    expectError(await cancelledWrite, "cancelled writer write"),
    "WriterWrite",
    cancelledWriterPath,
    "cancelled writer write",
  );
  assertResourceClosed(
    await cancelledWriter.write(Uint8Array.of(1), {}),
    "write after writer cancellation",
  );
  expectOk(cancelledWriter.close(), "cancelled writer close");

  const cancelledLister = await pendingOk(
    () => operator.openLister(directory, {}),
    bridge,
    "open cancellable lister",
  );
  const listerAbortController = new AbortController();
  const pendingBeforeListerCancel =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const cancelledListerNext = cancelledLister.next({
    signal: listerAbortController.signal,
  });
  await observePending(
    cancelledListerNext,
    bridge,
    pendingBeforeListerCancel,
    "cancelled lister next",
  );
  listerAbortController.abort();
  assertCancelled(
    expectError(await cancelledListerNext, "cancelled lister next"),
    "ListerNext",
    directory,
    "cancelled lister next",
  );
  assertResourceClosed(
    await cancelledLister.next({}),
    "next after lister cancellation",
  );
  expectOk(cancelledLister.close(), "cancelled lister close");
  await nextTurn();
  await nextTurn();
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 1,
    "cancelled streaming resources or late completions retained bridge handles",
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

  const operatorOwnedReader = await pendingOk(
    () => operator.openReadStream(streamPath, { chunkSize: 8 }),
    bridge,
    "open operator-owned reader",
  );
  const operatorOwnedWriter = await pendingOk(
    () => operator.openWriter(`${directory}operator-close.bin`, {}),
    bridge,
    "open operator-owned writer",
  );
  const operatorOwnedLister = await pendingOk(
    () => operator.openLister(directory, {}),
    bridge,
    "open operator-owned lister",
  );
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 4,
    "operator did not own exactly its three open child resources",
  );
  const pendingBeforeOperatorClose =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count();
  const cancelledByOperatorClose = operatorOwnedReader.next({});
  await observePending(
    cancelledByOperatorClose,
    bridge,
    pendingBeforeOperatorClose,
    "stream next interrupted by operator close",
  );

  expectOk(operator.close(), "operator close");
  operatorClosed = true;
  assertCancelled(
    expectError(
      await cancelledByOperatorClose,
      "stream next interrupted by operator close",
    ),
    "ReadStreamNext",
    streamPath,
    "stream next interrupted by operator close",
  );
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 0,
    "operator close left live bridge handles",
  );
  assertResourceClosed(
    await operatorOwnedReader.next({}),
    "reader after operator close",
  );
  assertResourceClosed(
    await operatorOwnedWriter.write(Uint8Array.of(1), {}),
    "writer after operator close",
  );
  assertResourceClosed(
    await operatorOwnedLister.next({}),
    "lister after operator close",
  );

  const runtimeOwnedOperator = expectOk(
    runtime.operator("memory", { root: "runtime-close" }),
    "runtime-owned operator construction",
  );
  const runtimeOwnedWriter = await pendingOk(
    () => runtimeOwnedOperator.openWriter("runtime-close.bin", {}),
    bridge,
    "open runtime-owned writer",
  );
  expectOk(runtime.close(), "runtime close");
  runtimeClosed = true;
  assert(
    bridge.opendal_mbt_wasm_live_handle_count() === 0,
    "runtime close left live bridge handles",
  );
  assertResourceClosed(
    await runtimeOwnedWriter.write(Uint8Array.of(2), {}),
    "writer after runtime close",
  );

  const pendingTasks =
    bridge.opendal_mbt_wasm_canary_forced_pending_poll_count() - pendingAtStart;
  assert(
    pendingTasks >= 30,
    `expected at least 30 forced-Pending operations, observed ${pendingTasks}`,
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
    streamingBinaryBytes: streamPayload.length,
    maxStreamChunkBytes: chunkSize,
    scalarBoundaryBytes: SCALAR_CHUNK_BYTES,
    streamedEntries: streamedEntries.length,
    copyRename: "structured-unsupported-on-memory",
    writerLifecycle: "finish-and-abort",
    structuredNotFound: true,
    abortSignal: "reader-writer-lister-cancelled",
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
