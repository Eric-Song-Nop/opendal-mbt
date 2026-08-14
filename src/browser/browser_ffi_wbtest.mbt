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
  #|     copy(source, destination, options) {
  #|       if (source === "missing.bin")
  #|         return Promise.resolve(error("NotFound", 4, "copy source does not exist"))
  #|       if (destination !== "fixture/copied.bin" || !(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({ ok: true, value: metadata(2n) })
  #|     },
  #|     rename(source, destination, options) {
  #|       if (source !== "fixture/copied.bin" || destination !== "fixture/moved.bin" ||
  #|           !(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       return Promise.resolve({ ok: true })
  #|     },
  #|     openReadStream(path, options) {
  #|       if (options.chunkSize !== 2 || options.range.kind !== "Full" ||
  #|           !(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       let index = 0
  #|       let closed = false
  #|       const cancelNext = path === "fixture/cancel.bin"
  #|       const chunks = [Uint8Array.of(0, 255), Uint8Array.of(7)]
  #|       return Promise.resolve({ ok: true, value: {
  #|         next({ signal }) {
  #|           if (closed || !(signal instanceof AbortSignal))
  #|             return Promise.resolve(error("ResourceClosed", 0x1002))
  #|           if (cancelNext) return Promise.resolve(
  #|             error("Cancelled", 0x1005, "stream was cancelled"),
  #|           )
  #|           if (index === chunks.length) return Promise.resolve({ ok: true })
  #|           return Promise.resolve({ ok: true, value: chunks[index++] })
  #|         },
  #|         close() { closed = true; return { ok: true } },
  #|       } })
  #|     },
  #|     openWriter(path, options) {
  #|       const validFixture =
  #|         (path === "fixture/chunks.bin" && options.contentType === "application/data") ||
  #|         (path === "fixture/oversized.bin" && options.contentType === undefined)
  #|       if (!validFixture || !(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       let length = 0n
  #|       let closed = false
  #|       return Promise.resolve({ ok: true, value: {
  #|         write(chunk, { signal }) {
  #|           if (closed || !(chunk instanceof Uint8Array) ||
  #|               !(signal instanceof AbortSignal))
  #|             return Promise.resolve(error("ResourceClosed", 0x1002))
  #|           if (chunk[0] === 9)
  #|             return Promise.resolve(error("Cancelled", 0x1005, "writer was cancelled"))
  #|           length += BigInt(chunk.length)
  #|           return Promise.resolve({ ok: true })
  #|         },
  #|         finish({ signal }) {
  #|           if (closed || !(signal instanceof AbortSignal))
  #|             return Promise.resolve(error("ResourceClosed", 0x1002))
  #|           closed = true
  #|           return Promise.resolve({ ok: true, value: metadata(length) })
  #|         },
  #|         abort({ signal }) {
  #|           if (!(signal instanceof AbortSignal))
  #|             return Promise.resolve(error("InvalidArgument", 0x1001))
  #|           closed = true
  #|           return Promise.resolve({ ok: true })
  #|         },
  #|         close() { closed = true; return { ok: true } },
  #|       } })
  #|     },
  #|     openLister(path, options) {
  #|       if (path !== "fixture/" || options.limit !== 1n ||
  #|           !(options.signal instanceof AbortSignal))
  #|         return Promise.resolve(error("InvalidArgument", 0x1001))
  #|       let yielded = false
  #|       let closed = false
  #|       return Promise.resolve({ ok: true, value: {
  #|         next({ signal }) {
  #|           if (closed || !(signal instanceof AbortSignal))
  #|             return Promise.resolve(error("ResourceClosed", 0x1002))
  #|           if (yielded) return Promise.resolve({ ok: true })
  #|           yielded = true
  #|           return Promise.resolve({ ok: true, value: {
  #|             path: "fixture/value.bin", name: "value.bin", metadata: metadata(2n),
  #|           } })
  #|         },
  #|         close() { closed = true; return { ok: true } },
  #|       } })
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
test "browser bridge resource-busy kind is preserved" {
  let error = error_from_bridge(
    0x1006,
    1,
    "ResourceBusy",
    "operation already in flight",
    error_context(Read, path="fixture/value.bin"),
  )
  assert_true(error.kind() is ResourceBusy)
  assert_true(error.status() is Permanent)
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

///|
async test "browser facade exposes bounded stateful resources and native moves" {
  let asynchronous = fake_runtime()
    .operator("memory", config=Map([("root", "fixture")]))
    .as_async()
  try
    asynchronous.open_read_stream(
      "fixture/value.bin",
      chunk_size=256 * 1024 + 1,
    )
  catch {
    OpenDalError(info) => assert_true(info.kind is InvalidArgument)
    _ => fail("expected bounded read-stream validation")
  } noraise {
    _ => fail("oversized read-stream chunks must fail locally")
  }
  let copied = asynchronous.copy("fixture/value.bin", "fixture/copied.bin")
  assert_eq(copied.content_length, 2UL)
  asynchronous.rename("fixture/copied.bin", "fixture/moved.bin")
  let stream = asynchronous.open_read_stream("fixture/value.bin", chunk_size=2)
  assert_eq(stream.next(), Some(b"\x00\xff"))
  assert_eq(stream.next(), Some(b"\x07"))
  assert_eq(stream.next(), None)
  assert_eq(stream.next(), None)
  stream.close()
  let writer = asynchronous.open_writer(
    "fixture/chunks.bin",
    content_type="application/data",
  )
  writer.write(b"ab")
  writer.write(b"\x00\xffz")
  let written = writer.finish()
  assert_eq(written.content_length, 5UL)
  try writer.abort() catch {
    OpenDalError(info) => assert_true(info.kind is ResourceClosed)
    _ => fail("expected a terminal finished writer")
  } noraise {
    _ => fail("abort after finish must not report success")
  }
  writer.close()
  let bounded_writer = asynchronous.open_writer("fixture/oversized.bin")
  try bounded_writer.write(Bytes::make(256 * 1024 + 1, 0)) catch {
    OpenDalError(info) => assert_true(info.kind is BufferTooLarge)
    _ => fail("expected bounded writer validation")
  } noraise {
    _ => fail("oversized writer chunks must fail locally")
  }
  bounded_writer.write(Bytes::make(256 * 1024, 0))
  assert_eq(bounded_writer.finish().content_length, 256UL * 1024UL)
  let aborted_writer = asynchronous.open_writer(
    "fixture/chunks.bin",
    content_type="application/data",
  )
  aborted_writer.abort()
  aborted_writer.abort()
  let lister = asynchronous.open_lister("fixture/", limit=1UL)
  guard lister.next() is Some(entry) else {
    fail("expected one streamed listing entry")
  }
  assert_eq(entry.path, "fixture/value.bin")
  assert_eq(lister.next(), None)
  assert_eq(lister.next(), None)
  lister.close()
  try asynchronous.copy("missing.bin", "fixture/copied.bin") catch {
    OpenDalError(info) => {
      assert_true(info.kind is NotFound)
      assert_true(info.operation is Copy)
      assert_eq(info.path, Some("missing.bin"))
      assert_eq(info.destination_path, Some("fixture/copied.bin"))
    }
    _ => fail("expected structured copy failure")
  } noraise {
    _ => fail("missing copy source must fail")
  }
  let cancelled_stream = asynchronous.open_read_stream(
    "fixture/cancel.bin",
    chunk_size=2,
  )
  try cancelled_stream.next() catch {
    OpenDalError(_) as error => assert_true(error.is_cancelled())
    _ => fail("expected stream cancellation")
  } noraise {
    _ => fail("cancelled stream next must fail")
  }
  try cancelled_stream.next() catch {
    OpenDalError(info) => assert_true(info.kind is ResourceClosed)
    _ => fail("expected terminal stream")
  } noraise {
    _ => fail("cancelled stream must become terminal")
  }
  let cancelled_writer = asynchronous.open_writer(
    "fixture/chunks.bin",
    content_type="application/data",
  )
  try cancelled_writer.write(b"\x09") catch {
    OpenDalError(_) as error => assert_true(error.is_cancelled())
    _ => fail("expected writer cancellation")
  } noraise {
    _ => fail("cancelled writer write must fail")
  }
  try cancelled_writer.write(b"late") catch {
    OpenDalError(info) => assert_true(info.kind is ResourceClosed)
    _ => fail("expected terminal writer")
  } noraise {
    _ => fail("cancelled writer must become terminal")
  }
}
