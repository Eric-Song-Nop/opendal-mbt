const DEFAULT_IMPORT_MODULE = "opendal_mbt_bridge";
const DEFAULT_HOST_MODULE = "opendal_mbt_host";
const MOONBIT_FFI_MODULE = "moonbit:ffi";
const TASK_PENDING = 1;
const TASK_READY = 2;
const MAX_TRANSFER_CHUNK = 256 * 1024;
const INVALID_ARGUMENT = 17;

function createTaskHost(bridgeExports) {
  let disposed = false;
  const waits = new Map();

  function schedule(registration, callback) {
    registration.timer = setTimeout(() => {
      registration.timer = undefined;
      callback();
    }, 0);
  }

  function cancelWait(handle) {
    const key = handle >>> 0;
    const registration = waits.get(key);
    if (registration === undefined) {
      return;
    }
    waits.delete(key);
    registration.cancelled = true;
    if (registration.timer !== undefined) {
      clearTimeout(registration.timer);
      registration.timer = undefined;
    }
  }

  function waitTask(handle, callback) {
    if (disposed) {
      return;
    }
    if (typeof callback !== "function") {
      throw new TypeError("task callback must be a function");
    }
    const key = handle >>> 0;
    cancelWait(key);
    const registration = { cancelled: false, timer: undefined };
    waits.set(key, registration);
    const poll = () => {
      if (
        disposed ||
        registration.cancelled ||
        waits.get(key) !== registration
      ) {
        return;
      }
      const state = bridgeExports.opendal_mbt_wasm_task_state(key);
      if (state === TASK_PENDING) {
        schedule(registration, poll);
        return;
      }
      waits.delete(key);
      registration.cancelled = true;
      if (state === TASK_READY) {
        callback(key);
      }
    };
    queueMicrotask(poll);
  }

  return {
    imports: { wait_task: waitTask, cancel_wait: cancelWait },
    dispose() {
      disposed = true;
      for (const handle of waits.keys()) {
        cancelWait(handle);
      }
    },
  };
}

function createTransferHost(bridgeExports) {
  const bridgeMemory = bridgeExports.memory;
  const bufferDataPointer =
    bridgeExports.opendal_mbt_wasm_buffer_data_ptr;
  const lastErrorCode = bridgeExports.opendal_mbt_wasm_last_error_code;
  if (!(bridgeMemory instanceof WebAssembly.Memory)) {
    throw new TypeError("Rust bridge does not export its WebAssembly.Memory");
  }
  if (
    typeof bufferDataPointer !== "function" ||
    typeof lastErrorCode !== "function"
  ) {
    throw new TypeError("Rust bridge does not export the bulk-transfer ABI");
  }
  let moonMemory;
  let moonToBridgeCalls = 0;
  let bridgeToMoonCalls = 0;
  let moonToBridgeBytes = 0;
  let bridgeToMoonBytes = 0;

  function bindMoonMemory(memory) {
    if (!(memory instanceof WebAssembly.Memory)) {
      throw new TypeError("MoonBit consumer does not export its WebAssembly.Memory");
    }
    moonMemory = memory;
  }

  function recordInvalidArgument(handle) {
    bufferDataPointer(handle >>> 0, 0, 0);
    return lastErrorCode() || INVALID_ARGUMENT;
  }

  function copyWindow(
    direction,
    handle,
    bridgeOffset,
    moonBase,
    moonDataLength,
    moonOffset,
    length,
  ) {
    const normalizedHandle = handle >>> 0;
    const normalizedBridgeOffset = bridgeOffset >>> 0;
    const normalizedMoonBase = moonBase >>> 0;
    const normalizedMoonDataLength = moonDataLength >>> 0;
    const normalizedMoonOffset = moonOffset >>> 0;
    const normalizedLength = length >>> 0;
    if (
      moonMemory === undefined ||
      normalizedLength === 0 ||
      normalizedLength > MAX_TRANSFER_CHUNK ||
      normalizedMoonOffset > normalizedMoonDataLength ||
      normalizedLength > normalizedMoonDataLength - normalizedMoonOffset
    ) {
      return recordInvalidArgument(normalizedHandle);
    }

    const moonBuffer = moonMemory.buffer;
    if (
      normalizedMoonBase > moonBuffer.byteLength ||
      normalizedMoonDataLength > moonBuffer.byteLength - normalizedMoonBase
    ) {
      return recordInvalidArgument(normalizedHandle);
    }

    const bridgePointer =
      bufferDataPointer(
        normalizedHandle,
        normalizedBridgeOffset,
        normalizedLength,
      ) >>> 0;
    if (bridgePointer === 0) {
      return lastErrorCode() || INVALID_ARGUMENT;
    }

    // A WebAssembly memory can replace its ArrayBuffer after memory.grow.
    // Re-read both buffers for every window and perform no bridge call between
    // obtaining the pointer and completing the synchronous copy.
    const currentBridgeBuffer = bridgeMemory.buffer;
    const currentMoonBuffer = moonMemory.buffer;
    if (
      bridgePointer > currentBridgeBuffer.byteLength ||
      normalizedLength > currentBridgeBuffer.byteLength - bridgePointer ||
      normalizedMoonBase > currentMoonBuffer.byteLength ||
      normalizedMoonDataLength >
        currentMoonBuffer.byteLength - normalizedMoonBase
    ) {
      return recordInvalidArgument(normalizedHandle);
    }
    const bridgeView = new Uint8Array(
      currentBridgeBuffer,
      bridgePointer,
      normalizedLength,
    );
    const moonView = new Uint8Array(
      currentMoonBuffer,
      normalizedMoonBase + normalizedMoonOffset,
      normalizedLength,
    );
    if (direction === "moon-to-bridge") {
      bridgeView.set(moonView);
      moonToBridgeCalls += 1;
      moonToBridgeBytes += normalizedLength;
    } else {
      moonView.set(bridgeView);
      bridgeToMoonCalls += 1;
      bridgeToMoonBytes += normalizedLength;
    }
    return 0;
  }

  function safelyCopy(direction, ...arguments_) {
    try {
      return copyWindow(direction, ...arguments_);
    } catch {
      return recordInvalidArgument(arguments_[0] ?? 0);
    }
  }

  return {
    imports: {
      copy_moon_to_bridge: (...arguments_) =>
        safelyCopy("moon-to-bridge", ...arguments_),
      copy_bridge_to_moon: (...arguments_) =>
        safelyCopy("bridge-to-moon", ...arguments_),
    },
    bindMoonMemory,
    stats() {
      return {
        moonToBridgeCalls,
        bridgeToMoonCalls,
        moonToBridgeBytes,
        bridgeToMoonBytes,
      };
    },
  };
}

function isResponse(value) {
  return typeof Response !== "undefined" && value instanceof Response;
}

async function toModule(source) {
  if (source instanceof WebAssembly.Module) {
    return source;
  }
  if (isResponse(source)) {
    if (!source.ok) {
      throw new Error(`could not fetch Wasm module: HTTP ${source.status}`);
    }
    const fallback = source.clone();
    try {
      return await WebAssembly.compileStreaming(Promise.resolve(source));
    } catch {
      return WebAssembly.compile(await fallback.arrayBuffer());
    }
  }
  if (source instanceof URL && source.protocol === "file:") {
    if (typeof process === "undefined" || !process.versions?.node) {
      throw new TypeError("file: Wasm URLs are only supported by the Node loader");
    }
    const { readFile } = await import("node:fs/promises");
    return WebAssembly.compile(await readFile(source));
  }
  if (source instanceof URL || typeof source === "string") {
    return toModule(await fetch(source));
  }
  if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
    return WebAssembly.compile(source);
  }
  throw new TypeError(
    "Wasm source must be a URL, Response, WebAssembly.Module, ArrayBuffer, or typed array",
  );
}

async function instantiateBridge(source, imports, initializer) {
  if (initializer === undefined) {
    return WebAssembly.instantiate(await toModule(source), imports);
  }
  if (typeof initializer !== "function") {
    throw new TypeError("bridgeInitializer must be a function");
  }
  const exports = await initializer({ module_or_path: source });
  if (exports === null || typeof exports !== "object") {
    throw new TypeError("bridgeInitializer did not return Wasm exports");
  }
  return { exports };
}

/**
 * Load the Rust OpenDAL bridge and a MoonBit Wasm consumer wired to it.
 *
 * The bridge initializer is the default export generated by wasm-bindgen. The
 * consumer only imports scalar bridge functions. Rust and MoonBit retain
 * separate memories and allocators; the MoonBit facade performs explicit byte
 * copies through generation-checked bridge handles.
 */
export async function loadOpenDalMoonBit({
  bridge,
  bridgeInitializer,
  moonbit,
  imports = {},
  bridgeImports = {},
  importModule = DEFAULT_IMPORT_MODULE,
  hostModule = DEFAULT_HOST_MODULE,
}) {
  const bridgeInstance = await instantiateBridge(
    bridge,
    bridgeImports,
    bridgeInitializer,
  );
  let taskHost;
  let transferHost;
  try {
    const moonbitModule = await toModule(moonbit);
    const existing = imports[importModule] ?? {};
    const existingHost = imports[hostModule] ?? {};
    const existingMoonBitFfi = imports[MOONBIT_FFI_MODULE] ?? {};
    taskHost = createTaskHost(bridgeInstance.exports);
    transferHost = createTransferHost(bridgeInstance.exports);
    const moonbitImports = {
      ...imports,
      [importModule]: {
        ...existing,
        ...bridgeInstance.exports,
      },
      [hostModule]: {
        ...existingHost,
        ...taskHost.imports,
        ...transferHost.imports,
      },
      [MOONBIT_FFI_MODULE]: {
        make_closure: (funcref, closure) => funcref.bind(null, closure),
        ...existingMoonBitFfi,
      },
    };
    const moonbitInstance = await WebAssembly.instantiate(
      moonbitModule,
      moonbitImports,
    );
    transferHost.bindMoonMemory(moonbitInstance.exports.memory);
    let disposed = false;
    return {
      exports: moonbitInstance.exports,
      bridge: bridgeInstance,
      moonbit: moonbitInstance,
      transfer: {
        stats: transferHost.stats,
      },
      dispose() {
        if (disposed) {
          return;
        }
        disposed = true;
        taskHost.dispose();
        bridgeInstance.exports.opendal_mbt_wasm_teardown();
      },
    };
  } catch (error) {
    taskHost?.dispose();
    bridgeInstance.exports.opendal_mbt_wasm_teardown?.();
    throw error;
  }
}

export default loadOpenDalMoonBit;
