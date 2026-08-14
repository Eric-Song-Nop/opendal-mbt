///|
extern "js" fn fake_js_runtime() -> JsRuntime =
  #| () => {
  #|   const metadata = (length = 2n) => ({
  #|     mode: "File",
  #|     contentLength: length,
  #|     isCurrent: false,
  #|     isDeleted: false,
  #|     cacheControl: "no-store",
  #|     contentDisposition: undefined,
  #|     contentEncoding: "identity",
  #|     contentMd5: undefined,
  #|     contentType: "application/octet-stream",
  #|     etag: "fixture-etag",
  #|     lastModified: { unixSeconds: -5n, nanoseconds: 999999999 },
  #|     version: "fixture-version",
  #|   })
  #|   const error = (kind, kindCode, message = "fixture failure") => ({
  #|     ok: false,
  #|     error: {
  #|       kind,
  #|       kindCode,
  #|       status: "Permanent",
  #|       statusCode: 1,
  #|       message,
  #|     },
  #|   })
  #|   let operatorClosed = false
  #|   const operator = {
  #|     info() {
  #|       if (operatorClosed) return error("ResourceClosed", 0x1002)
  #|       return {
  #|         ok: true,
  #|         value: {
  #|           scheme: "memory",
  #|           root: "/fixture/",
  #|           name: "fixture",
  #|           capability: { word0: (1n << 13n) - 1n, deleteMaxSize: 100n },
  #|         },
  #|       }
  #|     },
  #|     close() { operatorClosed = true; return { ok: true } },
  #|     createDir(path, options) {
  #|       if (!path.endsWith("/") || !(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({ ok: true })
  #|     },
  #|     write(path, data, options) {
  #|       if (!(data instanceof Uint8Array) || data[1] !== 255 ||
  #|           options.contentType !== "" || !(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({ ok: true, value: metadata(BigInt(data.length)) })
  #|     },
  #|     read(path, options) {
  #|       if (path === "missing.bin")
  #|         return Promise.resolve(error("NotFound", 4, "object does not exist"))
  #|       if (path === "cancelled.bin")
  #|         return Promise.resolve(error("Cancelled", 0x1005, "operation was cancelled"))
  #|       if (path === "rejected.bin")
  #|         return Promise.reject(new Error("fixture contract rejection"))
  #|       if (options.range.kind !== "Range" || options.range.offset !== 1n ||
  #|           options.range.length !== 2n || options.version !== "v1")
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({ ok: true, value: Uint8Array.of(0, 255) })
  #|     },
  #|     stat(path, options) {
  #|       if (path === "missing.bin")
  #|         return Promise.resolve(error("NotFound", 4, "object does not exist"))
  #|       if (path === "denied.bin")
  #|         return Promise.resolve(error("PermissionDenied", 5, "access denied"))
  #|       if (!(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({ ok: true, value: metadata(2n) })
  #|     },
  #|     delete(path, options) {
  #|       if (options.recursive !== true)
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({ ok: true })
  #|     },
  #|     list(path, options) {
  #|       if (options.limit !== 2n || options.startAfter !== "a" ||
  #|           options.recursive !== true)
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({
  #|         ok: true,
  #|         value: [{ path: "fixture/value.bin", name: "value.bin", metadata: metadata(2n) }],
  #|       })
  #|     },
  #|   }
  #|   let runtimeClosed = false
  #|   return {
  #|     availableSchemes() {
  #|       return runtimeClosed ? error("ResourceClosed", 0x1002) :
  #|         { ok: true, value: ["memory", "opfs", "s3"] }
  #|     },
  #|     operator(scheme, config) {
  #|       if (runtimeClosed) return error("ResourceClosed", 0x1002)
  #|       if (scheme !== "memory" || config.root !== "fixture")
  #|         return error("ConfigInvalid", 3)
  #|       return { ok: true, value: operator }
  #|     },
  #|     close() { runtimeClosed = true; operatorClosed = true; return { ok: true } },
  #|   }
  #| }

///|
fn fake_runtime() -> Runtime {
  { js: fake_js_runtime(), closed: false }
}

///|
async test "browser facade preserves MoonBit-shaped values and options" {
  let runtime = fake_runtime()
  assert_eq(runtime.available_schemes(), ["memory", "opfs", "s3"])
  let operator = runtime.operator("memory", config=Map([("root", "fixture")]))
  let info = operator.info()
  assert_eq(info.scheme, "memory")
  assert_eq(info.root, "/fixture/")
  assert_true(info.capability.can_read())
  assert_true(info.capability.can_list_recursive())
  assert_eq(info.capability.delete_max_size(), Some(100))
  let asynchronous = operator.as_async()
  asynchronous.create_dir("fixture/")
  let written = asynchronous.write(
    "fixture/value.bin",
    b"\x00\xff",
    content_type="",
  )
  assert_eq(written.content_length, 2UL)
  assert_eq(written.is_current, Some(false))
  assert_eq(written.cache_control, Some("no-store"))
  assert_eq(
    written.last_modified,
    Some({ unix_seconds: -5L, nanoseconds: 999999999 }),
  )
  let bytes = asynchronous.read(
    "fixture/value.bin",
    range=Range(offset=1UL, length=2UL),
    version="v1",
  )
  assert_eq(bytes, b"\x00\xff")
  assert_true(asynchronous.exists("fixture/value.bin"))
  let entries = asynchronous.list(
    "fixture/",
    recursive=true,
    limit=2UL,
    start_after="a",
  )
  assert_eq(entries.length(), 1)
  assert_eq(entries[0].path, "fixture/value.bin")
  assert_true(entries[0].is_file())
  asynchronous.delete("fixture/value.bin", recursive=true)
  asynchronous.close()
  runtime.close()
}

///|
async test "browser facade preserves structured errors and rejection boundary" {
  let asynchronous = fake_runtime()
    .operator("memory", config=Map([("root", "fixture")]))
    .as_async()
  try asynchronous.read("missing.bin") catch {
    OpenDalError(info) => {
      assert_true(info.kind is NotFound)
      assert_true(info.status is Permanent)
      assert_true(info.operation is Read)
      assert_eq(info.path, Some("missing.bin"))
    }
    _ => fail("expected OpenDalError for missing read")
  } noraise {
    _ => fail("expected a structured NotFound error")
  }
  try asynchronous.read("cancelled.bin") catch {
    OpenDalError(_) as error => assert_true(error.is_cancelled())
    _ => fail("expected OpenDalError for cancelled read")
  } noraise {
    _ => fail("expected a structured cancellation error")
  }
  try asynchronous.read("rejected.bin") catch {
    OpenDalError(info) => {
      assert_true(info.kind is AbiMismatch)
      assert_true(info.operation is Read)
    }
    _ => fail("expected OpenDalError for rejected Promise")
  } noraise {
    _ => fail("expected a rejected Promise to become AbiMismatch")
  }
  try asynchronous.exists("missing.bin") catch {
    _ => fail("exists must convert NotFound to false")
  } noraise {
    exists => assert_false(exists)
  }
  try asynchronous.exists("denied.bin") catch {
    OpenDalError(info) => {
      assert_true(info.kind is PermissionDenied)
      assert_true(info.operation is Exists)
    }
    _ => fail("expected a structured PermissionDenied error")
  } noraise {
    _ => fail("exists must retain errors other than NotFound")
  }
}
