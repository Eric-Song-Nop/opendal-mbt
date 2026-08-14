# Using OpenDAL in a browser

The JavaScript target keeps the same `Eric-Song-Nop/opendal` import and the
same portable async data path as native. It replaces the native shared library
with a version-matched OpenDAL WebAssembly runtime embedded in the Moon
package. Normal applications do not import the compatibility `/browser`
package and do not manage JavaScript glue or a `.wasm` file.

## Select the target with Moon

This module prefers native when no target is given. Select the backend at the
Moon command boundary rather than changing imports:

```sh
# Native executable and native OpenDAL artifact.
moon run --target native cmd/app

# JavaScript executable and embedded browser OpenDAL runtime.
moon run --target js cmd/app
```

Use the same package declaration in source shared by both builds:

```moonbit nocheck
supported_targets = "+native+js"

import {
  "Eric-Song-Nop/opendal",
  "moonbitlang/async",
}
```

`--target` selects a compile-time public surface; it is not a runtime fallback.
Native-only blocking calls such as `Operator::new("fs")` do not become browser
calls, and JS-only async extensions do not become native methods. Put those
sections behind `#cfg(target="native")` or `#cfg(target="js")`. Code that uses
only the portable async contract below compiles unchanged for both targets.

This binding supports exactly Moon's `native` and `js` targets. Moon's `wasm`
and `wasm-gc` targets are not supported or treated as browser aliases. The
browser architecture is a JavaScript-target Moon application hosting the
embedded OpenDAL core Wasm runtime.

The portable API has the same async calling convention but different
target-native execution engines:

| Target | Async execution |
| --- | --- |
| Native | OpenDAL work runs as Rust Tokio tasks; a nonblocking completion pipe wakes the Moon async scheduler. It is not a synchronous-call wrapper. |
| JavaScript | OpenDAL work resolves through JavaScript Promises; Moon cancellation is carried into the bridge with `AbortSignal`. |

## The exact portable async contract

The shared root package deliberately guarantees this small resource-oriented
surface on native and JavaScript:

| Resource | Portable methods |
| --- | --- |
| `AsyncOperator` | `new`, `close`, `write`, `read`, `open_read_stream`, `open_writer` |
| `AsyncReadStream` | `next`, `close` |
| `AsyncWriter` | `write`, `finish`, `abort` |

`ByteRange`, `Metadata`, the `ErrorInfo` fields, the common error variants, and
the `OpenDalError` accessors have portable shapes. JavaScript additionally
defines `ErrorKind::ResourceBusy`, `ErrorKind::Cancelled`, and
`Operation::LoadRuntime`; exhaustive matches over those enums therefore need a
target-specific branch. Calls are written normally inside `async fn`; MoonBit
has no `await` keyword.

```mbt check
///|
async test "browser guide: portable async contract" {
  let storage = @opendal.AsyncOperator::new("memory")
  defer storage.close()

  storage.write("portable/whole.bin", b"one portable package") |> ignore
  assert_eq(
    storage.read("portable/whole.bin", range=Range(offset=4UL, length=8UL)),
    b"portable",
  )

  let writer = storage.open_writer("portable/chunks.bin")
  writer.write(b"Moon")
  writer.write(b"Bit")
  writer.finish() |> ignore

  let stream = storage.open_read_stream("portable/chunks.bin", chunk_size=3)
  let output = Buffer()
  while stream.next() is Some(chunk) {
    output.write_bytes(chunk)
  }
  stream.close()
  assert_eq(output.to_bytes(), b"MoonBit")

  let discarded = storage.open_writer("portable/discarded.bin")
  discarded.write(b"temporary")
  discarded.abort()
  discarded.abort()
}
```

The portable contract does not currently include async stat, list, delete,
copy, rename, presign, layers, or task handles. Native provides those through
its blocking `Operator`; JavaScript provides the capability-checked Promise
extensions described below.

## One-line embedded runtime

On JavaScript, this one constructor lazily decodes and initializes the runtime
embedded in the Moon package:

```mbt check
///|
#cfg(target="js")
async test "browser guide: embedded memory runtime" {
  let storage = @opendal.AsyncOperator::new("memory")
  defer storage.close()
  storage.write("hello.txt", b"hello from a browser") |> ignore
  assert_eq(storage.read("hello.txt"), b"hello from a browser")
}
```

There is no additional npm install, bundler plugin, CDN, Rust toolchain, or
sidecar Wasm fetch. Each `AsyncOperator::new` owns a private runtime; call
`close` when the operator is no longer needed.

## Services by target

The shipped artifacts compile a deliberately small service set:

| Service | Native | Browser JS | Configuration and persistence |
| --- | --- | --- | --- |
| `memory` | Yes | Yes | No configuration; process/runtime-local and ephemeral |
| `fs` | Yes | No | Native host directory selected by `root` |
| `opfs` | No | Yes | Origin Private File System, optionally scoped by `root` |
| `s3` | Yes | Yes | Native has typed credential constructors; browser uses a copied config map |

The service being compiled does not guarantee every operation. Inspect
`Operator::info().capability` when using the explicit runtime flow, or handle
`Unsupported` when starting from the portable `AsyncOperator::new` shortcut.

## Persist data with OPFS

OPFS stores private application data for the current web origin. The only
binding-specific option is `root`; every operation path is resolved beneath
it. Keep a distinct root per application or logical storage boundary:

```mbt check
///|
#cfg(target="js")
#warnings("-unused_value")
async fn open_browser_documents() -> @opendal.AsyncOperator {
  @opendal.AsyncOperator::new("opfs", config={ "root": "/my-app/documents/" })
}
```

OPFS requires a browser that implements `navigator.storage.getDirectory()` and
a secure context. Deploy over HTTPS; `http://localhost` is normally accepted
for development. Storage is origin-scoped, subject to browser quota and
eviction policy, and is not a user-selected host filesystem directory. OPFS
currently supports stat, read, write, create-directory, delete, and list, but
not native copy, rename, or presign; still consult the effective capability
snapshot rather than assuming a backend version's feature set.

## Connect to S3 from a browser

The browser surface accepts OpenDAL's string configuration keys. For example,
an application that has already obtained short-lived, scoped credentials can
construct an S3 operator like this:

```mbt check
///|
#cfg(target="js")
#warnings("-unused_value")
async fn open_browser_s3(
  endpoint : String,
  bucket : String,
  region : String,
  access_key_id : String,
  secret_access_key : String,
  session_token : String,
) -> @opendal.AsyncOperator {
  @opendal.AsyncOperator::new("s3", config={
    "endpoint": endpoint,
    "bucket": bucket,
    "region": region,
    "access_key_id": access_key_id,
    "secret_access_key": secret_access_key,
    "session_token": session_token,
    "root": "/browser-app/",
  })
}
```

Do not embed permanent AWS secret keys in MoonBit source, generated JavaScript,
HTML, or public environment variables. Browser-delivered credentials are
visible to the user. Prefer a backend that issues short-lived, least-privilege
session credentials, or use `"skip_signature": "true"` only for a deliberately
public bucket/endpoint.

The S3 endpoint must allow the page origin through CORS. Its policy must cover
the methods and request headers the application uses, including signed
`Authorization` and `x-amz-*` headers, and must expose response headers the app
needs such as `ETag`. If the endpoint also enables browser credentials such as
cookies, it cannot combine `Access-Control-Allow-Credentials: true` with a
wildcard origin. An HTTPS page should use an HTTPS endpoint to avoid
mixed-content blocking. These are endpoint/browser policies; the binding
cannot bypass them.

## JS-only storage operations

The JavaScript target extends `AsyncOperator` with `create_dir`, `stat`,
`exists`, `delete`, `list`, `open_lister`, `copy`, and `rename`. The following
memory recipe exercises the operations that backend supports:

```mbt check
///|
#cfg(target="js")
async test "browser guide: JS metadata and listing operations" {
  let storage = @opendal.AsyncOperator::new("memory")
  defer storage.close()

  storage.create_dir("browser/nested/")
  storage.write("browser/nested/value.bin", b"value") |> ignore
  assert_true(storage.exists("browser/nested/value.bin"))
  let metadata = storage.stat("browser/nested/value.bin")
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, 5UL)

  let entries = storage.list("browser/", recursive=true)
  assert_true(entries.length() > 0)

  let lister = storage.open_lister("browser/", recursive=true)
  let mut found = false
  while lister.next() is Some(entry) {
    if entry.path == "browser/nested/value.bin" {
      found = true
    }
  }
  lister.close()
  assert_true(found)

  storage.delete("browser/", recursive=true)
  assert_false(storage.exists("browser/nested/value.bin"))
}
```

`list` materializes its result. The bridge rejects a result above 65,536
entries or 16 MiB of encoded listing output (paths, names, and metadata
snapshots); `limit` remains a backend request hint, not a guaranteed total
bound. Use `open_lister` for constant binding memory and close it after an
early exit.

Copy and rename are direct backend operations and are never emulated. Obtain an
`Operator` from an explicit `Runtime` when capability-driven code needs to
inspect them before creating its async view:

```mbt check
///|
#cfg(target="js")
#warnings("-unused_value")
async fn copy_or_rename_when_supported(operator : @opendal.Operator) -> Unit {
  let capability = operator.info().capability
  let storage = operator.as_async()
  if capability.can_copy() {
    storage.copy("source.bin", "copied.bin") |> ignore
  }
  if capability.can_rename() {
    storage.rename("copied.bin", "renamed.bin")
  }
}
```

## Runtime and custom assets

Most applications should use `AsyncOperator::new`. Use `Runtime` directly to
share one Wasm instance across operators, inspect compiled schemes and
capabilities, or host the three version-matched runtime assets yourself:

```mbt check
///|
#cfg(target="js")
#warnings("-unused_value")
async fn use_external_browser_assets(
  runtime_module_url : String,
  bridge_module_url : String,
  bridge_wasm_url : String,
) -> Unit {
  let assets = @opendal.BrowserAssets::new(
    runtime_module_url, bridge_module_url, bridge_wasm_url,
  )
  let runtime = @opendal.Runtime::load(assets)
  let operator = runtime.operator("memory")
  let storage = operator.as_async()
  storage.write("shared-runtime.bin", b"value") |> ignore
  assert_eq(storage.read("shared-runtime.bin"), b"value")
  operator.close()
  runtime.close()
}
```

Do not mix files from different package releases: `Runtime::load` validates
the ABI and required feature flags and reports mismatches as `AbiMismatch`.
Close all child streams, writers, listers, and operators before their shared
runtime. Closing an `AsyncOperator` created by `AsyncOperator::new` also closes
its privately owned runtime; closing an operator obtained from `Runtime` does
not close that shared runtime. Wait for in-flight calls before closing a parent;
behavior of outstanding child work after parent close is deliberately not a
portable cascade or cancellation guarantee.

## Browser and hosting requirements

The embedded path requires browser implementations of `WebAssembly`,
`DecompressionStream("gzip")`, `Blob`, `Response`, and `atob`. Serve the
generated JavaScript with a JavaScript MIME type. The embedded path contains
the compressed Wasm bytes in that output, so it does not need a separately
served `.wasm` file and works with or without a bundler.

A host page's Content Security Policy must permit WebAssembly compilation. A
minimal policy normally includes:

```http
Content-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'
```

Add `connect-src` origins for S3 or other network endpoints. The explicit
`Runtime::load` path also needs `script-src` permission for both module URLs
and `connect-src` permission for the Wasm URL. If assets are cross-origin, the
asset host must return suitable CORS headers and the `.wasm` response should
use `application/wasm`.

## Lifecycle, cancellation, and errors

- `close` is synchronous and idempotent for operators and read streams. The JS
  writer/lister close methods can report cleanup errors; portable writer code
  should use async `finish` or `abort`.
- A stream, writer, or lister permits one in-flight state transition. A second
  concurrent call raises `ResourceBusy`; serialize `next`, `write`, `finish`,
  and `abort` calls for each individual resource.
- Moon async cancellation activates the JavaScript `AbortSignal`. Cancelling
  an ordinary operator call does not prove the backend stopped or rolled back
  remote effects.
- Cancelling a stream/lister `next` or a writer operation makes that resource
  terminal because cursor position or commit progress may be unknown. Close it
  and open a new resource; do not resume it.
- Whole-object reads and writes are capped at 64 MiB. Read-stream outputs and
  individual writer inputs are capped at 256 KiB. Browser operator config is
  capped at 1,024 entries and 1 MiB of UTF-8 key/value data.

Async catch clauses receive MoonBit's target-neutral `Error`. Convert it with
`OpenDalError::from_error` before inspecting structured fields:

```mbt check
///|
async test "browser guide: recover a structured async error" {
  let storage = @opendal.AsyncOperator::new("memory")
  defer storage.close()
  try storage.read("missing.bin") catch {
    error =>
      match @opendal.OpenDalError::from_error(error) {
        Some(storage_error) => {
          assert_true(storage_error.kind() is NotFound)
          assert_true(storage_error.status() is Permanent)
          let info = storage_error.info()
          assert_true(info.operation is Read)
          assert_eq(info.path, Some("missing.bin"))
        }
        None => fail("expected an OpenDAL storage error")
      }
  } noraise {
    _ => fail("expected a missing-object error")
  }
}
```

`Temporary` or `Persistent` is diagnostic classification, not permission to
replay a write. Treat `Cancelled` and `ResourceBusy` separately, and never log
S3 credentials or signed request material with an error.

Continue with [Common tasks](tasks.mbt.md) for the portable streaming recipes
and [Connecting](connecting.mbt.md) for the native typed S3 API.
