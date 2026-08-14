const ABI_VERSION = 0x0001_0006;
const REQUIRED_FEATURE_FLAGS = 0x0000_07fc;

const MAX_BUFFER_BYTES = 64 * 1024 * 1024;
const MAX_TRANSFER_CHUNK = 256 * 1024;
const MAX_CONFIG_ENTRIES = 1024;
const MAX_CONFIG_BYTES = 1024 * 1024;

const TASK_PENDING = 1;
const TASK_READY = 2;
const TASK_CANCELLED = 3;
const TASK_CONSUMED = 4;

const COMPLETION_WRITE = 1;
const COMPLETION_READ = 2;
const COMPLETION_STAT = 3;
const COMPLETION_CREATE_DIR = 4;
const COMPLETION_DELETE = 5;
const COMPLETION_LIST = 6;

const ERROR_PERMANENT = 1;
const ERROR_CANCELLED = 0x1005;
const UINT64_MAX = 0xffff_ffff_ffff_ffffn;

const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder("utf-8", { fatal: true });

const REQUIRED_EXPORTS = [
  "opendal_mbt_wasm_abi_version",
  "opendal_mbt_wasm_feature_flags",
  "opendal_mbt_wasm_live_handle_count",
  "opendal_mbt_wasm_teardown",
  "opendal_mbt_wasm_buffer_new_sized",
  "opendal_mbt_wasm_buffer_len",
  "opendal_mbt_wasm_buffer_data_ptr",
  "opendal_mbt_wasm_buffer_release",
  "opendal_mbt_wasm_registered_schemes",
  "opendal_mbt_wasm_operator_builder_new",
  "opendal_mbt_wasm_operator_builder_set",
  "opendal_mbt_wasm_operator_builder_build",
  "opendal_mbt_wasm_operator_builder_release",
  "opendal_mbt_wasm_operator_info_scheme",
  "opendal_mbt_wasm_operator_info_root",
  "opendal_mbt_wasm_operator_info_name",
  "opendal_mbt_wasm_operator_info_capability_word",
  "opendal_mbt_wasm_operator_write_options_start_v1",
  "opendal_mbt_wasm_operator_read_options_start_v1",
  "opendal_mbt_wasm_operator_stat_options_start_v1",
  "opendal_mbt_wasm_operator_create_dir_start",
  "opendal_mbt_wasm_operator_delete_start",
  "opendal_mbt_wasm_operator_list_start",
  "opendal_mbt_wasm_task_state",
  "opendal_mbt_wasm_task_take",
  "opendal_mbt_wasm_task_cancel",
  "opendal_mbt_wasm_task_release",
  "opendal_mbt_wasm_completion_kind",
  "opendal_mbt_wasm_completion_status",
  "opendal_mbt_wasm_completion_take_buffer",
  "opendal_mbt_wasm_completion_take_metadata_snapshot",
  "opendal_mbt_wasm_completion_take_entry_list",
  "opendal_mbt_wasm_completion_take_error",
  "opendal_mbt_wasm_completion_release",
  "opendal_mbt_wasm_entry_list_len",
  "opendal_mbt_wasm_entry_list_path",
  "opendal_mbt_wasm_entry_list_name",
  "opendal_mbt_wasm_entry_list_metadata_snapshot",
  "opendal_mbt_wasm_entry_list_release",
  "opendal_mbt_wasm_operator_release",
  "opendal_mbt_wasm_last_error_take",
  "opendal_mbt_wasm_last_error_clear",
  "opendal_mbt_wasm_error_snapshot_take",
  "opendal_mbt_wasm_error_release",
];

export class OpenDalBrowserContractError extends Error {
  constructor(message, options) {
    super(message, options);
    this.name = "OpenDalBrowserContractError";
  }
}

class OutcomeFailure extends Error {
  constructor(error) {
    super(error.message);
    this.error = error;
  }
}

function ok(value) {
  return { ok: true, value };
}

function failed(error) {
  return { ok: false, error };
}

function statusName(status) {
  switch (status) {
    case 1:
      return "Permanent";
    case 2:
      return "Temporary";
    case 3:
      return "Persistent";
    default:
      return "Unknown";
  }
}

function localError(kind, kindCode, message, context) {
  return {
    kind,
    kindCode,
    status: "Permanent",
    statusCode: ERROR_PERMANENT,
    message,
    operation: context.operation,
    path: context.path,
    destinationPath: context.destinationPath,
  };
}

function invalid(message, context) {
  throw new OutcomeFailure(localError("InvalidArgument", 0x1001, message, context));
}

function closedError(context) {
  return localError("ResourceClosed", 0x1002, "resource is closed", context);
}

function cancelledError(context) {
  return localError("Cancelled", ERROR_CANCELLED, "operation was cancelled", context);
}

function isWellFormed(value) {
  if (typeof value.isWellFormed === "function") {
    return value.isWellFormed();
  }
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) {
        return false;
      }
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function textValue(value, label, context) {
  if (typeof value !== "string") {
    invalid(`${label} must be a string`, context);
  }
  if (!isWellFormed(value)) {
    invalid(`${label} contains invalid UTF-16`, context);
  }
  return value;
}

function optionalText(value, label, context) {
  if (value === undefined || value === null) {
    return undefined;
  }
  return textValue(value, label, context);
}

function boolValue(value, fallback, label, context) {
  if (value === undefined) {
    return fallback;
  }
  if (typeof value !== "boolean") {
    invalid(`${label} must be a boolean`, context);
  }
  return value;
}

function uint64Value(value, label, context) {
  let result;
  if (typeof value === "bigint") {
    result = value;
  } else if (typeof value === "number" && Number.isSafeInteger(value)) {
    result = BigInt(value);
  } else {
    invalid(`${label} must be a BigInt or safe integer`, context);
  }
  if (result < 0n || result > UINT64_MAX) {
    invalid(`${label} is outside UInt64`, context);
  }
  return result;
}

function byteView(value, context) {
  let bytes;
  if (value instanceof Uint8Array) {
    bytes = value;
  } else if (value instanceof ArrayBuffer) {
    bytes = new Uint8Array(value);
  } else if (ArrayBuffer.isView(value)) {
    bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  } else {
    invalid("data must be a Uint8Array, ArrayBuffer, or typed-array view", context);
  }
  if (bytes.byteLength > MAX_BUFFER_BYTES) {
    throw new OutcomeFailure(
      localError(
        "BufferTooLarge",
        0x1003,
        "buffer exceeds the 64 MiB browser bridge limit",
        context,
      ),
    );
  }
  return bytes;
}

function magicEquals(bytes, expected) {
  if (bytes.length < expected.length) {
    return false;
  }
  for (let index = 0; index < expected.length; index += 1) {
    if (bytes[index] !== expected.charCodeAt(index)) {
      return false;
    }
  }
  return true;
}

function contractFailure(message, cause) {
  throw new OpenDalBrowserContractError(message, cause ? { cause } : undefined);
}

function decodeErrorSnapshot(bytes, context) {
  try {
    if (bytes.length < 24 || !magicEquals(bytes, "ODE1")) {
      contractFailure("bridge returned a malformed ODE1 error snapshot");
    }
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (view.getUint32(4, true) !== 1) {
      contractFailure("bridge returned an unsupported ODE1 schema");
    }
    const kindCode = view.getUint32(8, true);
    const statusCode = view.getUint32(12, true);
    const nameLength = view.getUint32(16, true);
    const messageLength = view.getUint32(20, true);
    const nameEnd = 24 + nameLength;
    const messageEnd = nameEnd + messageLength;
    if (
      nameEnd < 24 ||
      messageEnd < nameEnd ||
      messageEnd !== bytes.length ||
      messageEnd > MAX_BUFFER_BYTES
    ) {
      contractFailure("bridge returned an invalid ODE1 payload length");
    }
    const kind = utf8Decoder.decode(bytes.subarray(24, nameEnd));
    const message = utf8Decoder.decode(bytes.subarray(nameEnd, messageEnd));
    return {
      kind,
      kindCode,
      status: statusName(statusCode),
      statusCode,
      message,
      operation: context.operation,
      path: context.path,
      destinationPath: context.destinationPath,
    };
  } catch (error) {
    if (error instanceof OpenDalBrowserContractError) {
      throw error;
    }
    contractFailure("bridge returned non-UTF-8 ODE1 text", error);
  }
}

function decodeMetadataSnapshot(bytes) {
  try {
    if (bytes.length < 84 || bytes.length > MAX_BUFFER_BYTES || !magicEquals(bytes, "ODM1")) {
      contractFailure("bridge returned a malformed ODM1 metadata snapshot");
    }
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (view.getUint32(4, true) !== 1) {
      contractFailure("bridge returned an unsupported ODM1 schema");
    }
    const present = view.getBigUint64(8, true);
    if ((present & ~0x1ffn) !== 0n) {
      contractFailure("bridge returned unknown ODM1 presence bits");
    }
    const modeCode = view.getUint32(16, true);
    const currentCode = view.getUint32(20, true);
    const deletedCode = view.getUint32(24, true);
    if (modeCode > 2 || currentCode > 1 || deletedCode > 1) {
      contractFailure("bridge returned a non-canonical ODM1 scalar");
    }
    if (view.getUint32(28, true) !== 0 || view.getUint32(52, true) !== 0) {
      contractFailure("bridge returned non-zero ODM1 reserved data");
    }
    const currentPresent = (present & 1n) !== 0n;
    if (!currentPresent && currentCode !== 0) {
      contractFailure("bridge returned data for an absent ODM1 current flag");
    }
    const modifiedSeconds = view.getBigInt64(40, true);
    const modifiedNanoseconds = view.getUint32(48, true);
    const modifiedPresent = (present & 2n) !== 0n;
    if (modifiedNanoseconds >= 1_000_000_000) {
      contractFailure("bridge returned invalid ODM1 nanoseconds");
    }
    if (!modifiedPresent && (modifiedSeconds !== 0n || modifiedNanoseconds !== 0)) {
      contractFailure("bridge returned data for an absent ODM1 timestamp");
    }
    const lengths = Array.from(
      { length: 7 },
      (_, index) => view.getUint32(56 + index * 4, true),
    );
    let cursor = 84;
    const strings = [];
    for (let index = 0; index < lengths.length; index += 1) {
      const isPresent = (present & (1n << BigInt(index + 2))) !== 0n;
      if (!isPresent && lengths[index] !== 0) {
        contractFailure("bridge returned bytes for an absent ODM1 string");
      }
      const end = cursor + lengths[index];
      if (end < cursor || end > bytes.length) {
        contractFailure("bridge returned a truncated ODM1 string");
      }
      strings.push(isPresent ? utf8Decoder.decode(bytes.subarray(cursor, end)) : undefined);
      cursor = end;
    }
    if (cursor !== bytes.length) {
      contractFailure("bridge returned trailing ODM1 bytes");
    }
    const modes = ["Unknown", "File", "Directory"];
    return {
      mode: modes[modeCode],
      contentLength: view.getBigUint64(32, true),
      isCurrent: currentPresent ? currentCode === 1 : undefined,
      isDeleted: deletedCode === 1,
      cacheControl: strings[0],
      contentDisposition: strings[1],
      contentEncoding: strings[2],
      contentMd5: strings[3],
      contentType: strings[4],
      etag: strings[5],
      lastModified: modifiedPresent
        ? { unixSeconds: modifiedSeconds, nanoseconds: modifiedNanoseconds }
        : undefined,
      version: strings[6],
    };
  } catch (error) {
    if (error instanceof OpenDalBrowserContractError) {
      throw error;
    }
    contractFailure("bridge returned non-UTF-8 ODM1 text", error);
  }
}

class BridgeFacade {
  constructor(bridge) {
    this.bridge = bridge;
  }

  clearLastError() {
    this.bridge.opendal_mbt_wasm_last_error_clear();
  }

  takeLastError(context, fallbackMessage) {
    const errorHandle = this.bridge.opendal_mbt_wasm_last_error_take();
    if (errorHandle === 0) {
      return localError("Unexpected", 1, fallbackMessage, context);
    }
    return this.takeError(errorHandle, context);
  }

  takeError(errorHandle, context) {
    let ownedError = errorHandle;
    let snapshotHandle = 0;
    try {
      this.clearLastError();
      snapshotHandle = this.bridge.opendal_mbt_wasm_error_snapshot_take(ownedError);
      if (snapshotHandle === 0) {
        const detail = this.takeLastErrorWithoutSnapshot();
        contractFailure(`could not materialize ODE1 error snapshot: ${detail}`);
      }
      ownedError = 0;
      return decodeErrorSnapshot(this.takeBuffer(snapshotHandle), context);
    } finally {
      if (snapshotHandle !== 0) {
        snapshotHandle = 0;
      }
      if (ownedError !== 0) {
        this.bridge.opendal_mbt_wasm_error_release(ownedError);
      }
    }
  }

  takeLastErrorWithoutSnapshot() {
    const handle = this.bridge.opendal_mbt_wasm_last_error_take();
    if (handle === 0) {
      return "bridge did not publish an error";
    }
    this.bridge.opendal_mbt_wasm_error_release(handle);
    return "bridge rejected error snapshot ownership transfer";
  }

  expectHandle(handle, context, fallbackMessage) {
    if (handle === 0) {
      throw new OutcomeFailure(this.takeLastError(context, fallbackMessage));
    }
    return handle;
  }

  expectStatus(status, context, fallbackMessage) {
    if (status !== 0) {
      throw new OutcomeFailure(this.takeLastError(context, fallbackMessage));
    }
  }

  putBytes(bytes, context) {
    this.clearLastError();
    const handle = this.expectHandle(
      this.bridge.opendal_mbt_wasm_buffer_new_sized(bytes.byteLength),
      context,
      "could not allocate a bridge buffer",
    );
    try {
      for (let offset = 0; offset < bytes.byteLength; offset += MAX_TRANSFER_CHUNK) {
        const length = Math.min(MAX_TRANSFER_CHUNK, bytes.byteLength - offset);
        this.clearLastError();
        const pointer = this.bridge.opendal_mbt_wasm_buffer_data_ptr(handle, offset, length);
        if (pointer === 0) {
          throw new OutcomeFailure(
            this.takeLastError(context, "could not access a bridge buffer window"),
          );
        }
        new Uint8Array(this.bridge.memory.buffer, pointer >>> 0, length).set(
          bytes.subarray(offset, offset + length),
        );
      }
      return handle;
    } catch (error) {
      this.bridge.opendal_mbt_wasm_buffer_release(handle);
      throw error;
    }
  }

  putText(value, context) {
    return this.putBytes(utf8Encoder.encode(value), context);
  }

  putOptionalText(value, context) {
    return value === undefined ? 0 : this.putText(value, context);
  }

  takeBuffer(handle) {
    try {
      this.clearLastError();
      const length = this.bridge.opendal_mbt_wasm_buffer_len(handle);
      if (length < 0) {
        throw new OutcomeFailure(
          this.takeLastError(
            { operation: "Check", path: undefined, destinationPath: undefined },
            "could not query a bridge buffer length",
          ),
        );
      }
      if (length > MAX_BUFFER_BYTES) {
        contractFailure("bridge returned an oversized buffer");
      }
      const result = new Uint8Array(length);
      for (let offset = 0; offset < length; offset += MAX_TRANSFER_CHUNK) {
        const chunkLength = Math.min(MAX_TRANSFER_CHUNK, length - offset);
        this.clearLastError();
        const pointer = this.bridge.opendal_mbt_wasm_buffer_data_ptr(
          handle,
          offset,
          chunkLength,
        );
        if (pointer === 0) {
          throw new OutcomeFailure(
            this.takeLastError(
              { operation: "Check", path: undefined, destinationPath: undefined },
              "could not access a bridge buffer window",
            ),
          );
        }
        result.set(
          new Uint8Array(this.bridge.memory.buffer, pointer >>> 0, chunkLength),
          offset,
        );
      }
      return result;
    } finally {
      this.bridge.opendal_mbt_wasm_buffer_release(handle);
    }
  }

  takeText(handle) {
    try {
      return utf8Decoder.decode(this.takeBuffer(handle));
    } catch (error) {
      if (error instanceof OutcomeFailure || error instanceof OpenDalBrowserContractError) {
        throw error;
      }
      contractFailure("bridge returned non-UTF-8 text", error);
    }
  }

  takeMetadata(handle) {
    return decodeMetadataSnapshot(this.takeBuffer(handle));
  }

  capability(operatorHandle, context) {
    const word = (index) => {
      this.clearLastError();
      const value = this.bridge.opendal_mbt_wasm_operator_info_capability_word(
        operatorHandle,
        index,
      );
      const errorHandle = this.bridge.opendal_mbt_wasm_last_error_take();
      if (errorHandle !== 0) {
        throw new OutcomeFailure(this.takeError(errorHandle, context));
      }
      return value;
    };
    const word0 = word(0);
    const deleteMaxSizeValue = word(1);
    const has = (bit) => (word0 & (1n << BigInt(bit))) !== 0n;
    return {
      word0,
      deleteMaxSize: deleteMaxSizeValue === 0n ? undefined : deleteMaxSizeValue,
      canStat: has(0),
      canRead: has(1),
      canWrite: has(2),
      canCreateDir: has(3),
      canDelete: has(4),
      canList: has(5),
      canCopy: has(6),
      canRename: has(7),
      canReadSuffix: has(8),
      canWriteAppend: has(9),
      canListWithLimit: has(10),
      canListWithStartAfter: has(11),
      canListRecursive: has(12),
      canPresignStat: has(13),
      canPresignRead: has(14),
      canPresignWrite: has(15),
    };
  }

  operatorInfo(operatorHandle, context) {
    const takeInfoText = (name, fallback) => {
      this.clearLastError();
      return this.takeText(
        this.expectHandle(this.bridge[name](operatorHandle), context, fallback),
      );
    };
    return {
      scheme: takeInfoText(
        "opendal_mbt_wasm_operator_info_scheme",
        "could not query operator scheme",
      ),
      root: takeInfoText(
        "opendal_mbt_wasm_operator_info_root",
        "could not query operator root",
      ),
      name: takeInfoText(
        "opendal_mbt_wasm_operator_info_name",
        "could not query operator name",
      ),
      capability: this.capability(operatorHandle, context),
    };
  }

  listEntries(listHandle, context) {
    try {
      this.clearLastError();
      const length = this.bridge.opendal_mbt_wasm_entry_list_len(listHandle);
      if (length < 0) {
        throw new OutcomeFailure(
          this.takeLastError(context, "could not query a list result length"),
        );
      }
      const entries = [];
      for (let index = 0; index < length; index += 1) {
        this.clearLastError();
        const pathHandle = this.expectHandle(
          this.bridge.opendal_mbt_wasm_entry_list_path(listHandle, index),
          context,
          "could not take a listed path",
        );
        const path = this.takeText(pathHandle);
        this.clearLastError();
        const nameHandle = this.expectHandle(
          this.bridge.opendal_mbt_wasm_entry_list_name(listHandle, index),
          context,
          "could not take a listed name",
        );
        const name = this.takeText(nameHandle);
        this.clearLastError();
        const metadataHandle = this.expectHandle(
          this.bridge.opendal_mbt_wasm_entry_list_metadata_snapshot(listHandle, index),
          context,
          "could not take listed metadata",
        );
        entries.push({ path, name, metadata: this.takeMetadata(metadataHandle) });
      }
      return entries;
    } finally {
      this.bridge.opendal_mbt_wasm_entry_list_release(listHandle);
    }
  }

  settleTask(taskHandle, expectedKind, context, signal) {
    return new Promise((resolve, reject) => {
      let ownedTask = taskHandle;
      let timer;
      let settled = false;

      const cleanup = () => {
        if (timer !== undefined) {
          clearTimeout(timer);
        }
        signal?.removeEventListener("abort", onAbort);
      };
      const finish = (outcome) => {
        if (!settled) {
          settled = true;
          cleanup();
          resolve(outcome);
        }
      };
      const failContract = (error) => {
        if (!settled) {
          settled = true;
          cleanup();
          reject(error);
        }
      };
      const releaseTask = () => {
        if (ownedTask !== 0) {
          this.bridge.opendal_mbt_wasm_task_release(ownedTask);
          ownedTask = 0;
        }
      };
      const onAbort = () => {
        if (settled) {
          return;
        }
        if (ownedTask !== 0) {
          this.bridge.opendal_mbt_wasm_task_cancel(ownedTask);
          releaseTask();
        }
        finish(failed(cancelledError(context)));
      };
      const poll = () => {
        if (settled) {
          return;
        }
        try {
          this.clearLastError();
          const state = this.bridge.opendal_mbt_wasm_task_state(ownedTask);
          if (state === TASK_PENDING) {
            timer = setTimeout(poll, 0);
            return;
          }
          if (state === TASK_CANCELLED) {
            releaseTask();
            finish(failed(cancelledError(context)));
            return;
          }
          if (state === 0) {
            const error = this.takeLastError(context, "could not poll an OpenDAL task");
            releaseTask();
            finish(failed(error));
            return;
          }
          if (state === TASK_CONSUMED) {
            releaseTask();
            contractFailure("bridge exposed a consumed task to its Promise owner");
          }
          if (state !== TASK_READY) {
            releaseTask();
            contractFailure(`bridge returned unknown task state ${state}`);
          }
          this.clearLastError();
          const completionHandle = this.expectHandle(
            this.bridge.opendal_mbt_wasm_task_take(ownedTask),
            context,
            "could not take a ready OpenDAL task",
          );
          releaseTask();
          finish(this.settleCompletion(completionHandle, expectedKind, context));
        } catch (error) {
          releaseTask();
          if (error instanceof OutcomeFailure) {
            finish(failed(error.error));
          } else {
            failContract(error);
          }
        }
      };

      if (signal !== undefined && !(signal instanceof AbortSignal)) {
        releaseTask();
        finish(
          failed(localError("InvalidArgument", 0x1001, "signal must be an AbortSignal", context)),
        );
        return;
      }
      if (signal?.aborted) {
        onAbort();
        return;
      }
      signal?.addEventListener("abort", onAbort, { once: true });
      queueMicrotask(poll);
    });
  }

  settleCompletion(completionHandle, expectedKind, context) {
    let ownedCompletion = completionHandle;
    try {
      this.clearLastError();
      const actualKind = this.bridge.opendal_mbt_wasm_completion_kind(ownedCompletion);
      if (actualKind === 0) {
        throw new OutcomeFailure(
          this.takeLastError(context, "could not inspect an OpenDAL completion"),
        );
      }
      if (actualKind !== expectedKind) {
        contractFailure(
          `bridge returned completion kind ${actualKind}, expected ${expectedKind}`,
        );
      }
      this.clearLastError();
      const status = this.bridge.opendal_mbt_wasm_completion_status(ownedCompletion);
      const statusError = this.bridge.opendal_mbt_wasm_last_error_take();
      if (statusError !== 0) {
        throw new OutcomeFailure(this.takeError(statusError, context));
      }
      if (status !== 0) {
        this.clearLastError();
        const errorHandle = this.expectHandle(
          this.bridge.opendal_mbt_wasm_completion_take_error(ownedCompletion),
          context,
          "could not take an OpenDAL completion error",
        );
        ownedCompletion = 0;
        return failed(this.takeError(errorHandle, context));
      }

      if (expectedKind === COMPLETION_READ) {
        this.clearLastError();
        const buffer = this.expectHandle(
          this.bridge.opendal_mbt_wasm_completion_take_buffer(ownedCompletion),
          context,
          "could not take an OpenDAL read result",
        );
        ownedCompletion = 0;
        return ok(this.takeBuffer(buffer));
      }
      if (expectedKind === COMPLETION_WRITE || expectedKind === COMPLETION_STAT) {
        this.clearLastError();
        const metadata = this.expectHandle(
          this.bridge.opendal_mbt_wasm_completion_take_metadata_snapshot(ownedCompletion),
          context,
          "could not take OpenDAL metadata",
        );
        ownedCompletion = 0;
        return ok(this.takeMetadata(metadata));
      }
      if (expectedKind === COMPLETION_LIST) {
        this.clearLastError();
        const list = this.expectHandle(
          this.bridge.opendal_mbt_wasm_completion_take_entry_list(ownedCompletion),
          context,
          "could not take an OpenDAL list result",
        );
        ownedCompletion = 0;
        return ok(this.listEntries(list, context));
      }
      this.expectStatus(
        this.bridge.opendal_mbt_wasm_completion_release(ownedCompletion),
        context,
        "could not release an OpenDAL completion",
      );
      ownedCompletion = 0;
      return ok(undefined);
    } finally {
      if (ownedCompletion !== 0) {
        this.bridge.opendal_mbt_wasm_completion_release(ownedCompletion);
      }
    }
  }
}

class BrowserOperator {
  constructor(runtime, handle, info) {
    this.runtime = runtime;
    this.facade = runtime.facade;
    this.handle = handle;
    this.infoSnapshot = info;
  }

  context(operation, path, destinationPath) {
    return { operation, path, destinationPath };
  }

  info() {
    if (this.handle === 0 || this.runtime.closed) {
      return failed(closedError(this.context("Check")));
    }
    return ok(this.infoSnapshot);
  }

  close() {
    if (this.handle === 0) {
      return ok(undefined);
    }
    const context = this.context("Check");
    const handle = this.handle;
    this.handle = 0;
    this.runtime.operators.delete(this);
    try {
      this.facade.clearLastError();
      this.facade.expectStatus(
        this.facade.bridge.opendal_mbt_wasm_operator_release(handle),
        context,
        "could not release an OpenDAL operator",
      );
      return ok(undefined);
    } catch (error) {
      if (error instanceof OutcomeFailure) {
        return failed(error.error);
      }
      throw error;
    }
  }

  async runTask(operation, pathValue, expectedKind, options, start) {
    const path = typeof pathValue === "string" ? pathValue : undefined;
    const context = this.context(operation, path);
    try {
      if (this.handle === 0 || this.runtime.closed) {
        return failed(closedError(context));
      }
      const normalizedPath = textValue(pathValue, "path", context);
      context.path = normalizedPath;
      const task = start(context, normalizedPath);
      return await this.facade.settleTask(task, expectedKind, context, options?.signal);
    } catch (error) {
      if (error instanceof OutcomeFailure) {
        return failed(error.error);
      }
      throw error;
    }
  }

  createDir(path, options = {}) {
    return this.runTask("CreateDir", path, COMPLETION_CREATE_DIR, options, (context, value) => {
      const pathHandle = this.facade.putText(value, context);
      try {
        this.facade.clearLastError();
        return this.facade.expectHandle(
          this.facade.bridge.opendal_mbt_wasm_operator_create_dir_start(
            this.handle,
            pathHandle,
          ),
          context,
          "could not start create-dir",
        );
      } finally {
        this.facade.bridge.opendal_mbt_wasm_buffer_release(pathHandle);
      }
    });
  }

  write(path, data, options = {}) {
    return this.runTask("Write", path, COMPLETION_WRITE, options, (context, value) => {
      const handles = [];
      try {
        const pathHandle = this.facade.putText(value, context);
        handles.push(pathHandle);
        const dataHandle = this.facade.putBytes(byteView(data, context), context);
        handles.push(dataHandle);
        const append = boolValue(options.append, false, "append", context);
        const optional = [
          optionalText(options.contentType, "contentType", context),
          optionalText(options.contentDisposition, "contentDisposition", context),
          optionalText(options.contentEncoding, "contentEncoding", context),
          optionalText(options.cacheControl, "cacheControl", context),
          optionalText(options.ifMatch, "ifMatch", context),
          optionalText(options.ifNoneMatch, "ifNoneMatch", context),
        ].map((text) => {
          const handle = this.facade.putOptionalText(text, context);
          if (handle !== 0) {
            handles.push(handle);
          }
          return handle;
        });
        this.facade.clearLastError();
        return this.facade.expectHandle(
          this.facade.bridge.opendal_mbt_wasm_operator_write_options_start_v1(
            this.handle,
            pathHandle,
            dataHandle,
            append ? 1 : 0,
            ...optional,
          ),
          context,
          "could not start write",
        );
      } finally {
        for (const handle of handles) {
          this.facade.bridge.opendal_mbt_wasm_buffer_release(handle);
        }
      }
    });
  }

  read(path, options = {}) {
    return this.runTask("Read", path, COMPLETION_READ, options, (context, value) => {
      const handles = [];
      try {
        const pathHandle = this.facade.putText(value, context);
        handles.push(pathHandle);
        const range = normalizeRange(options.range, context);
        const optional = [
          optionalText(options.version, "version", context),
          optionalText(options.ifMatch, "ifMatch", context),
          optionalText(options.ifNoneMatch, "ifNoneMatch", context),
        ].map((text) => {
          const handle = this.facade.putOptionalText(text, context);
          if (handle !== 0) {
            handles.push(handle);
          }
          return handle;
        });
        this.facade.clearLastError();
        return this.facade.expectHandle(
          this.facade.bridge.opendal_mbt_wasm_operator_read_options_start_v1(
            this.handle,
            pathHandle,
            range.kind,
            range.offset,
            range.length,
            ...optional,
          ),
          context,
          "could not start read",
        );
      } finally {
        for (const handle of handles) {
          this.facade.bridge.opendal_mbt_wasm_buffer_release(handle);
        }
      }
    });
  }

  stat(path, options = {}) {
    return this.runTask("Stat", path, COMPLETION_STAT, options, (context, value) => {
      const handles = [];
      try {
        const pathHandle = this.facade.putText(value, context);
        handles.push(pathHandle);
        const optional = [
          optionalText(options.version, "version", context),
          optionalText(options.ifMatch, "ifMatch", context),
          optionalText(options.ifNoneMatch, "ifNoneMatch", context),
        ].map((text) => {
          const handle = this.facade.putOptionalText(text, context);
          if (handle !== 0) {
            handles.push(handle);
          }
          return handle;
        });
        this.facade.clearLastError();
        return this.facade.expectHandle(
          this.facade.bridge.opendal_mbt_wasm_operator_stat_options_start_v1(
            this.handle,
            pathHandle,
            ...optional,
          ),
          context,
          "could not start stat",
        );
      } finally {
        for (const handle of handles) {
          this.facade.bridge.opendal_mbt_wasm_buffer_release(handle);
        }
      }
    });
  }

  delete(path, options = {}) {
    return this.runTask("Delete", path, COMPLETION_DELETE, options, (context, value) => {
      const handles = [];
      try {
        const pathHandle = this.facade.putText(value, context);
        handles.push(pathHandle);
        const version = optionalText(options.version, "version", context);
        const versionHandle = this.facade.putOptionalText(version, context);
        if (versionHandle !== 0) {
          handles.push(versionHandle);
        }
        const recursive = boolValue(options.recursive, false, "recursive", context);
        this.facade.clearLastError();
        return this.facade.expectHandle(
          this.facade.bridge.opendal_mbt_wasm_operator_delete_start(
            this.handle,
            pathHandle,
            versionHandle,
            recursive ? 1 : 0,
          ),
          context,
          "could not start delete",
        );
      } finally {
        for (const handle of handles) {
          this.facade.bridge.opendal_mbt_wasm_buffer_release(handle);
        }
      }
    });
  }

  list(path, options = {}) {
    return this.runTask("List", path, COMPLETION_LIST, options, (context, value) => {
      const handles = [];
      try {
        const pathHandle = this.facade.putText(value, context);
        handles.push(pathHandle);
        const recursive = boolValue(options.recursive, false, "recursive", context);
        const hasLimit = options.limit !== undefined && options.limit !== null;
        const limit = hasLimit ? uint64Value(options.limit, "limit", context) : 0n;
        const startAfter = optionalText(options.startAfter, "startAfter", context);
        const startAfterHandle = this.facade.putOptionalText(startAfter, context);
        if (startAfterHandle !== 0) {
          handles.push(startAfterHandle);
        }
        this.facade.clearLastError();
        return this.facade.expectHandle(
          this.facade.bridge.opendal_mbt_wasm_operator_list_start(
            this.handle,
            pathHandle,
            recursive ? 1 : 0,
            hasLimit ? 1 : 0,
            limit,
            startAfterHandle,
          ),
          context,
          "could not start list",
        );
      } finally {
        for (const handle of handles) {
          this.facade.bridge.opendal_mbt_wasm_buffer_release(handle);
        }
      }
    });
  }
}

function normalizeRange(value, context) {
  if (value === undefined || value === null || value === "Full") {
    return { kind: 0, offset: 0n, length: 0n };
  }
  if (typeof value !== "object") {
    invalid("range must be a range object", context);
  }
  const kind = value.kind ?? "Full";
  if (kind === "Full") {
    return { kind: 0, offset: 0n, length: 0n };
  }
  if (kind === "From") {
    return { kind: 1, offset: uint64Value(value.offset, "range.offset", context), length: 0n };
  }
  if (kind === "Range") {
    const offset = uint64Value(value.offset, "range.offset", context);
    const length = uint64Value(value.length, "range.length", context);
    if (offset + length > UINT64_MAX) {
      invalid("range offset plus length exceeds UInt64", context);
    }
    return { kind: 2, offset, length };
  }
  if (kind === "Suffix") {
    return { kind: 3, offset: 0n, length: uint64Value(value.length, "range.length", context) };
  }
  invalid("range.kind must be Full, From, Range, or Suffix", context);
}

export class OpenDalBrowserRuntime {
  constructor(bridge) {
    this.facade = new BridgeFacade(bridge);
    this.operators = new Set();
    this.closed = false;
  }

  availableSchemes() {
    const context = { operation: "Check", path: undefined, destinationPath: undefined };
    if (this.closed) {
      return failed(closedError(context));
    }
    try {
      this.facade.clearLastError();
      const handle = this.facade.expectHandle(
        this.facade.bridge.opendal_mbt_wasm_registered_schemes(),
        context,
        "could not query registered OpenDAL schemes",
      );
      const encoded = this.facade.takeText(handle);
      return ok(encoded === "" ? [] : encoded.split("\n"));
    } catch (error) {
      if (error instanceof OutcomeFailure) {
        return failed(error.error);
      }
      throw error;
    }
  }

  operator(schemeValue, config = {}) {
    const context = { operation: "NewOperator", path: undefined, destinationPath: undefined };
    if (this.closed) {
      return failed(closedError(context));
    }
    let builder = 0;
    let operatorHandle = 0;
    try {
      const scheme = textValue(schemeValue, "scheme", context);
      const configEntries = normalizeConfig(config, context);
      const schemeHandle = this.facade.putText(scheme, context);
      try {
        this.facade.clearLastError();
        builder = this.facade.expectHandle(
          this.facade.bridge.opendal_mbt_wasm_operator_builder_new(schemeHandle),
          context,
          "could not create an OpenDAL operator builder",
        );
      } finally {
        this.facade.bridge.opendal_mbt_wasm_buffer_release(schemeHandle);
      }
      for (const [key, value] of configEntries) {
        const keyHandle = this.facade.putText(key, context);
        const valueHandle = this.facade.putText(value, context);
        try {
          this.facade.clearLastError();
          this.facade.expectStatus(
            this.facade.bridge.opendal_mbt_wasm_operator_builder_set(
              builder,
              keyHandle,
              valueHandle,
            ),
            context,
            "could not set OpenDAL operator configuration",
          );
        } finally {
          this.facade.bridge.opendal_mbt_wasm_buffer_release(keyHandle);
          this.facade.bridge.opendal_mbt_wasm_buffer_release(valueHandle);
        }
      }
      this.facade.clearLastError();
      operatorHandle = this.facade.expectHandle(
        this.facade.bridge.opendal_mbt_wasm_operator_builder_build(builder),
        context,
        "could not build an OpenDAL operator",
      );
      const info = this.facade.operatorInfo(operatorHandle, context);
      const operator = new BrowserOperator(this, operatorHandle, info);
      operatorHandle = 0;
      this.operators.add(operator);
      return ok(operator);
    } catch (error) {
      if (error instanceof OutcomeFailure) {
        return failed(error.error);
      }
      throw error;
    } finally {
      if (operatorHandle !== 0) {
        this.facade.bridge.opendal_mbt_wasm_operator_release(operatorHandle);
      }
      if (builder !== 0) {
        this.facade.bridge.opendal_mbt_wasm_operator_builder_release(builder);
      }
    }
  }

  close() {
    if (this.closed) {
      return ok(undefined);
    }
    let firstError;
    for (const operator of [...this.operators]) {
      const outcome = operator.close();
      if (!outcome.ok && firstError === undefined) {
        firstError = outcome.error;
      }
    }
    this.closed = true;
    this.operators.clear();
    const status = this.facade.bridge.opendal_mbt_wasm_teardown();
    if (status !== 0 && firstError === undefined) {
      firstError = localError(
        "Unexpected",
        1,
        "could not tear down the OpenDAL browser bridge",
        { operation: "Check", path: undefined, destinationPath: undefined },
      );
    }
    return firstError === undefined ? ok(undefined) : failed(firstError);
  }
}

function normalizeConfig(config, context) {
  let entries;
  if (config instanceof Map) {
    entries = [...config.entries()];
  } else if (
    config !== null &&
    typeof config === "object" &&
    (Object.getPrototypeOf(config) === Object.prototype || Object.getPrototypeOf(config) === null)
  ) {
    entries = Object.entries(config);
  } else {
    invalid("config must be a Map or plain object", context);
  }
  if (entries.length > MAX_CONFIG_ENTRIES) {
    invalid("config exceeds the 1024-entry browser bridge limit", context);
  }
  let encodedBytes = 0;
  const normalized = entries.map(([key, value]) => {
    const normalizedKey = textValue(key, "config key", context);
    const normalizedValue = textValue(value, `config value for ${normalizedKey}`, context);
    encodedBytes += utf8Encoder.encode(normalizedKey).length;
    encodedBytes += utf8Encoder.encode(normalizedValue).length;
    if (encodedBytes > MAX_CONFIG_BYTES) {
      invalid("config exceeds the 1 MiB browser bridge limit", context);
    }
    return [normalizedKey, normalizedValue];
  });
  return normalized;
}

function validateBridge(bridge) {
  if (bridge === null || typeof bridge !== "object") {
    contractFailure("loadOpenDalBrowser requires initialized wasm-bindgen exports");
  }
  if (!(bridge.memory instanceof WebAssembly.Memory)) {
    contractFailure("OpenDAL bridge does not export its WebAssembly memory");
  }
  for (const name of REQUIRED_EXPORTS) {
    if (typeof bridge[name] !== "function") {
      contractFailure(`OpenDAL bridge is missing required export ${name}`);
    }
  }
  const actualAbi = bridge.opendal_mbt_wasm_abi_version();
  if (actualAbi !== ABI_VERSION) {
    contractFailure(
      `OpenDAL bridge ABI mismatch: expected 0x${ABI_VERSION.toString(16)}, ` +
        `got 0x${actualAbi.toString(16)}`,
    );
  }
  const features = bridge.opendal_mbt_wasm_feature_flags();
  if ((features & REQUIRED_FEATURE_FLAGS) !== REQUIRED_FEATURE_FLAGS) {
    contractFailure(
      `OpenDAL bridge is missing required feature flags 0x${(
        REQUIRED_FEATURE_FLAGS & ~features
      ).toString(16)}`,
    );
  }
  const liveHandles = bridge.opendal_mbt_wasm_live_handle_count();
  if (liveHandles !== 0) {
    contractFailure(
      `OpenDAL bridge must be unowned at load time; found ${liveHandles} live handles`,
    );
  }
}

export async function loadOpenDalBrowser(input) {
  let bridge = input?.bridge;
  if (bridge === undefined) {
    if (typeof input?.init !== "function") {
      contractFailure("loadOpenDalBrowser requires bridge exports or an init function");
    }
    bridge =
      input.module === undefined
        ? await input.init()
        : await input.init({ module_or_path: input.module });
  }
  validateBridge(bridge);
  return new OpenDalBrowserRuntime(bridge);
}
