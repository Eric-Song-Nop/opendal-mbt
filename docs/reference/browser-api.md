# Browser and JavaScript API Reference

Status: JavaScript target surface released in `0.2.0`

This page maps the public MoonBit API selected by `--target js`. It complements
the task-oriented guides: it identifies the exact portable slice, the
additional browser surface, and the target-specific reference commands without
duplicating implementation source.

For a runnable program shared by native and a real browser, including a
cross-origin S3 non-blocking proof, see the
[portable async example](../../examples/browser/README.md).

Applications import the normal root package:

```moon.pkg
import {
  "Eric-Song-Nop/opendal",
}
```

Native remains the module's preferred target. Browser builds and runs must
select JavaScript explicitly:

```sh
moon check --target js
moon run --target js --release <main-package>
```

The supported facade scope is native plus JavaScript. The Rust bridge embedded
by the JavaScript build is internally WebAssembly, but that does not make
MoonBit's `wasm`, `wasm-gc`, or other targets supported.

## Authoritative target views

Conditional compilation gives the root package a different resolved surface
on native and JavaScript. The committed `src/pkg.generated.mbti` records the
preferred native view only. Query the compiler for the JavaScript view:

```sh
moon ide doc --target js '@Eric-Song-Nop/opendal'
moon ide doc --target js '@Eric-Song-Nop/opendal/browser'
moon ide doc --target js \
  '@Eric-Song-Nop/opendal/browser.AsyncOperator::list'
```

The first command shows the types re-exported by the root package. Their
methods are owned by the public `browser` package, so the second and third
commands show the complete method documentation. Values remain used through
the application's ordinary `@opendal` import.

For comparison, inspect the native target with:

```sh
moon ide doc --target native '@Eric-Song-Nop/opendal'
```

## Exact portable async surface

The following 11 callables have the same names, argument shapes, value types,
async effects, and normal lifecycle rules on native and JavaScript:

```moonbit
async fn AsyncOperator::new(
  scheme : StringView,
  config? : Map[String, String] = Map([]),
) -> AsyncOperator

fn AsyncOperator::close(self : AsyncOperator) -> Unit

async fn AsyncOperator::write(
  self : AsyncOperator,
  path : StringView,
  data : BytesView,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Metadata

async fn AsyncOperator::read(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Bytes

async fn AsyncOperator::open_read_stream(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  chunk_size? : Int = 256 * 1024,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> AsyncReadStream

async fn AsyncReadStream::next(self : AsyncReadStream) -> Bytes?
fn AsyncReadStream::close(self : AsyncReadStream) -> Unit

async fn AsyncOperator::open_writer(
  self : AsyncOperator,
  path : StringView,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> AsyncWriter

async fn AsyncWriter::write(self : AsyncWriter, data : BytesView) -> Unit
async fn AsyncWriter::finish(self : AsyncWriter) -> Metadata
async fn AsyncWriter::abort(self : AsyncWriter) -> Unit
```

MoonBit async calls have no `await` keyword and carry a raising effect
implicitly. `ByteRange`, `Metadata`, `EntryMode`, `Timestamp`, `ErrorInfo`,
`ErrorKind`, `ErrorStatus`, `Operation`, and `OpenDalError` provide the common
value vocabulary. `OpenDalError::from_error(Error)` is the target-neutral way
to recover structured OpenDAL information inside an async catch clause.

The identical call shape has genuinely asynchronous target implementations.
Native launches OpenDAL async futures on Rust Tokio tasks and suspends the
MoonBit scheduler on a nonblocking completion pipe; it does not wrap the
blocking Operator. JavaScript waits on Promises and maps scheduler cancellation
through `AbortSignal` to the browser task bridge.

The portable contract intentionally does not specify whether an already-open
child completes after its parent is closed. Finish or close streams and
writers, and await in-flight calls, before `AsyncOperator::close()`.

## Shared values and capability inspection

The portable calls use the same data model on both targets:

| Type | Public shape |
| --- | --- |
| `ByteRange` | `Full`, `From(offset)`, `Range(offset, length)`, or `Suffix(length)`; length is a count, not an inclusive end |
| `Timestamp` | signed Unix seconds plus normalized nanoseconds |
| `Metadata` | mode, unsigned content length, current/deleted flags, cache/content headers, MD5, ETag, last-modified timestamp, and version |
| `Entry` | path, basename, and metadata snapshot |
| `OperatorInfo` | scheme, root, backend name, and effective `Capability` snapshot |
| `ErrorInfo` | kind, retry status, message, operation, source path, and destination path |

`EntryMode` is `File`, `Directory`, or `Unknown`. `ErrorStatus` is
`Permanent`, `Temporary`, `Persistent`, or `UnknownStatus(code)`. The common
`ErrorKind` and `Operation` variants are portable, while JavaScript adds
`ResourceBusy`, `Cancelled`, and `LoadRuntime`; exhaustive enum matches must
account for the selected target.

Inspect `OperatorInfo.capability` through these methods rather than depending
on internal bit positions:

```text
can_stat              can_read                can_write
can_create_dir        can_delete              can_list
can_copy              can_rename              can_read_suffix
can_write_append      can_list_with_limit     can_list_with_start_after
can_list_recursive    can_presign_stat        can_presign_read
can_presign_write     delete_max_size
```

The snapshot describes the configured backend, including operations that are
only callable through a target-specific extension. It does not promise that an
operation will continue to succeed, so callers still handle `Unsupported` and
ordinary storage errors.

## Browser-only operations

The JavaScript target adds eight `AsyncOperator` methods. They are not
available on the native `AsyncOperator` yet, even when a similarly named
blocking native operation exists.

| Method | Result and notable options |
| --- | --- |
| `create_dir(path)` | `Unit` |
| `stat(path, version?, if_match?, if_none_match?)` | `Metadata` |
| `exists(path)` | `Bool` |
| `delete(path, version?, recursive?)` | `Unit` |
| `list(path, recursive?, limit?, start_after?)` | bounded `Array[Entry]` |
| `open_lister(path, recursive?, limit?, start_after?)` | `AsyncLister` |
| `copy(source, destination)` | destination `Metadata` |
| `rename(source, destination)` | `Unit` |

`AsyncLister` is browser-only:

```moonbit
async fn AsyncLister::next(self : AsyncLister) -> Entry?
fn AsyncLister::close(self : AsyncLister) -> Unit raise OpenDalError
```

JavaScript also exposes explicit early-release methods
`AsyncWriter::close() -> Unit raise OpenDalError` and
`OpenDalError::is_cancelled() -> Bool`. These are useful browser extensions but
are not part of the portable 11-callable slice.

All optional operation features remain capability dependent. Inspect
`Operator::info().capability` before relying on suffix reads, append, recursive
listing, list limits, start-after, copy, or rename. A capability is
introspection rather than a substitute for handling `Unsupported` at the
operation boundary.

## Browser-only runtime types

The JavaScript target owns three additional public types:

| Type | Purpose |
| --- | --- |
| `Runtime` | One validated OpenDAL Wasm facade and its resource arena; embedded construction owns a fresh instance |
| `BrowserAssets` | Version-matched runtime, wasm-bindgen module, and Wasm URLs |
| `AsyncLister` | Bounded, stateful asynchronous listing cursor |

The JavaScript `Operator` is a configured handle owned by a `Runtime`. Its
public methods are `info`, `as_async`, and `close`; synchronous storage I/O is
not exposed in browsers.

The runtime control surface is:

```moonbit
async fn Runtime::new() -> Runtime
async fn Runtime::load(assets : BrowserAssets) -> Runtime
fn Runtime::available_schemes(self : Runtime) -> Array[String] raise OpenDalError
fn Runtime::operator(
  self : Runtime,
  scheme : StringView,
  config? : Map[String, String] = Map([]),
) -> Operator raise OpenDalError
fn Runtime::close(self : Runtime) -> Unit raise OpenDalError

fn BrowserAssets::new(
  runtime_module_url : String,
  bridge_module_url : String,
  bridge_wasm_url : String,
) -> BrowserAssets

fn Operator::info(self : Operator) -> OperatorInfo
fn Operator::as_async(self : Operator) -> AsyncOperator
fn Operator::close(self : Operator) -> Unit
```

Normal applications use the embedded runtime through
`AsyncOperator::new(...)`. Advanced applications that need to share one runtime
or serve assets from a controlled origin can use this flow:

```moonbit
let assets = @opendal.BrowserAssets::new(
  "https://static.example/opendal/browser-runtime.mjs",
  "https://static.example/opendal/opendal_mbt_browser_bridge.mjs",
  "https://static.example/opendal/opendal_mbt_browser_bridge_bg.wasm",
)
let runtime = @opendal.Runtime::load(assets)
let schemes = runtime.available_schemes()
let operator = runtime.operator("memory")
let asynchronous = operator.as_async()

// Use asynchronous, then release children before their owners.
asynchronous.close()
runtime.close()
```

All three assets must come from the same build and must match browser ABI 1.7
exactly. `Runtime::load` validates the Wasm exports, version, required feature
mask, memory, and initial ownership state. Prefer the embedded constructor
unless custom hosting is an actual requirement.

`Runtime::load` may be called only once for a particular
`bridge_module_url` during a page's lifetime. Browser module imports and the
wasm-bindgen initializer cache the initialized exports by module URL, while
`Runtime::close` permanently tears down that instance. Share the one returned
runtime between operators; do not load the same bridge module URL concurrently
or again after close. `Runtime::new` does not have this custom-asset cache
restriction and is the correct path for independently owned runtimes.

`Runtime::available_schemes()` reports the exact artifact's services. The
current embedded artifact contains:

- `memory`, for ephemeral in-page storage and deterministic tests;
- `opfs`, which requires the browser Origin Private File System facilities;
- `s3`, which additionally depends on browser networking, correct endpoint and
  credentials, and server CORS policy.

## Limits

The browser boundary enforces these limits before allocation or cross-memory
copy:

| Value | Hard limit |
| --- | ---: |
| Whole read/write buffer | 64 MiB |
| Read-stream output chunk | 256 KiB |
| Individual Writer input | 256 KiB |
| Materialized listing | 65,536 entries and 16 MiB encoded output |
| Operator configuration | 1,024 entries and 1 MiB combined UTF-8 |

Use `AsyncReadStream` for larger reads and split Writer input into chunks.
`limit` on `list` and `open_lister` is a backend request hint; it does not
replace the binding's hard listing bounds.

The embedded loader requires `WebAssembly` and `DecompressionStream`. The host
Content Security Policy must permit Wasm compilation, normally with
`script-src 'wasm-unsafe-eval'`. Custom asset origins also need the applicable
script/fetch and CORS permissions.

## Errors and cancellation

Storage, validation, bounds, closed-resource, and busy-resource failures become
typed `OpenDalError` values with operation and path context. Browser-specific
kinds are:

- `Cancelled`, when the bridge reports an operation-level `AbortSignal`
  cancellation outcome;
- `ResourceBusy`, when a stream, writer, or lister already has an operation in
  flight.

Module loading, an incompatible ABI, a missing export, or a malformed bridge
snapshot becomes `AbiMismatch`. Unknown numeric kinds and statuses are
preserved rather than collapsed.

Cancellation makes a stateful child terminal because remote cursor or commit
progress may be unknown. It does not guarantee that an already-dispatched S3
request stopped and does not roll back visible effects.

MoonBit scheduler cancellation activates the operation's `AbortSignal` but
propagates as the scheduler cancellation error rather than being relabelled as
`OpenDalError`. A structured `Cancelled` result supplied by the bridge remains
available to `OpenDalError::is_cancelled()`.

For the private protocol and ownership rationale, see
[Browser Runtime and Wasm ABI](../design/browser-runtime.md). Maintainers should
start from the [Wasm index](https://github.com/Eric-Song-Nop/opendal-mbt/blob/v0.2.0/wasm/README.md).
