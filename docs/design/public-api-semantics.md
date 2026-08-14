# Public API Semantics

Status: Phase 5A-E source contract frozen for the pinned `v0.2.0` candidate

This document defines the intended MoonBit-facing behavior of the blocking core
and the initial asynchronous OpenDAL facade. It deliberately avoids fixing the
native ABI or implementation layout; those details must implement this
contract without leaking through the public package.

This document defines the implemented resource, S3, presign, layer, batch,
Copier, and initial async semantics. The generated
`src/pkg.generated.mbti` is the authoritative current public surface.

## Design stance

The API combines:

- the domain semantics and information preservation of OpenDAL's Rust API;
- the abstract resource boundary used by OCaml bindings;
- MoonBit's own algebraic data types, read-only public structs, optional
  labelled arguments, methods, and checked error effects.

It is not a transliteration of either Rust or OCaml:

- Rust ownership and lifetimes are not available in MoonBit, so native
  resources need runtime state plus GC finalizers.
- Returning `Result[T, String]` from every method, as the current OCaml binding
  does, would discard structured errors and duplicate MoonBit's typed
  `raise`/`catch` mechanism.
- Rust's parallel `read`, `read_with`, and `read_options` forms become one
  canonical method whose implemented options are optional labelled arguments.

## Public type categories

| Category | Types | Contract |
|---|---|---|
| Native resources | `Operator`, `Lister`, `Reader`, `ReadStream`, `Writer`, `Copier`, `AsyncReadStream`, and `AsyncWriter` | Opaque and impossible for callers to fabricate |
| Facade views | `AsyncOperator` | Opaque lightweight view retaining the configured Operator |
| Opaque configuration | `S3Auth`, `S3CredentialSource` | Constructed only through typed, secret-safe factory methods |
| Read-only snapshots | `Metadata`, `Entry`, `ErrorInfo`, `OperatorInfo`, `Timestamp`, `PresignedRequest`, `PresignedHeader` | Fields are readable but values are produced by the binding |
| Read-only algebraic outputs | `EntryMode`, `ErrorKind`, `ErrorStatus`, `Operation` | Callers can inspect and pattern-match values but cannot fabricate them |
| Algebraic input | `ByteRange` | Uses `pub(all)` so callers can construct labelled ranges |
| Extensible query object | `Capability` | Opaque effective-capability snapshot with getter methods so new capabilities can be added compatibly |

`Operator` is logically immutable and shareable; layer methods return new
Operators. `AsyncOperator` retains the same configured Operator. `Lister`,
`ReadStream`, `Writer`, `Copier`, `AsyncReadStream`, and `AsyncWriter` are
stateful. `Reader` is a random-access reader without an implicit cursor.

## Construction and configuration

```moonbit
Operator::new(
  scheme : StringView,
  config? : Map[String, String] = Map([]),
) -> Operator raise OpenDalError
```

- Schemes remain strings. OpenDAL's service set is large, versioned, and
  determined by Rust compile-time features; a closed MoonBit service enum would
  become stale quickly.
- The constructor copies the configuration map and never mutates it or retains
  references into it.
- Available schemes are the intersection of the Rust service features compiled
  into the native library and the binding's advertised service profile.
- Operator formatting must never expose configuration values or credentials.
- The binding must explicitly choose and document retry behavior; it must not
  inherit an implicit default from another language binding.

## Service profiles and typed S3

A service profile is a compile-time distribution identity, not a runtime
plugin choice. `local` names the memory/filesystem profile; `standard` names
its memory/filesystem/S3 successor. Native `library_info.service_profile` and
artifact `service_profile` fields contain that canonical identifier
(`local` or `standard`). Artifact manifests record the enabled scheme list
separately in `services`; the profile field is never a comma-separated service
list.

The generic `Operator::new` remains the escape hatch for compiled services.
S3 additionally has a typed constructor:

```moonbit nocheck
Operator::s3(
  bucket : StringView,
  region~ : StringView,
  root? : StringView,
  endpoint? : StringView,
  auth? : S3Auth,
  virtual_host_style? : Bool = false,
) -> Operator raise OpenDalError
```

`S3Auth` and `S3CredentialSource` are opaque values constructed through
default-chain, static/session, unsigned, and assume-role factory methods.
Required text cannot be empty; optional text distinguishes absence from an
invalid empty value; assume-role duration must fit the native 32-bit carrier.
All strings and credentials are copied during construction. Credential values
have no public fields, `Debug`, or `Show` representation and must not appear in
errors. Constructing the Operator performs no object I/O; `check` is the
explicit backend-I/O operation.

## Error model

All operational failures use a typed MoonBit error effect:

```moonbit
pub suberror OpenDalError {
  OpenDalError(ErrorInfo)
}
```

Callers normally propagate it naturally and can materialize it as a `Result`
only when an error must become data. Convenience accessors expose `info`,
`kind`, `status`, `is_temporary`, and `is_persistent`; the retry-status
helpers do not imply that a non-idempotent operation is safe to retry.

`ErrorInfo` contains:

- a stable `ErrorKind`;
- the mutually exclusive OpenDAL `ErrorStatus` (`Permanent`, `Temporary`, or
  `Persistent`);
- a human-readable message;
- the binding operation and optional path captured at the call site;
- an optional destination path, populated only for two-path operations such as
  copy and rename.

The path values are the caller's original OpenDAL-relative inputs. For copy and
rename, `path` is the source and `destination_path` is the destination. For
one-path operations the destination is absent; construction and check errors
have no path.

`ErrorKind` includes all current OpenDAL categories plus binding-local
`InvalidArgument`, `ResourceClosed`, `BufferTooLarge`, and `AbiMismatch`.
Because upstream `ErrorKind` is non-exhaustive, `UnknownKind(code, name)` is
mandatory. The numeric code belongs to this binding's versioned ABI; it is not
a cast of Rust's enum representation.

`Option` is reserved for actual absence:

- missing optional metadata;
- end of a lister, read stream, or Copier;
- unknown version-current status.

It does not hide operational failures. In particular, `stat` raises
`NotFound`; `exists` converts only `NotFound` to `false` and propagates every
other failure.

## Paths and directory semantics

Paths use OpenDAL semantics rather than host-OS path semantics:

- they are relative to the operator root;
- callers must not expect `.`/`..`, drive-letter, symlink, or platform path
  normalization;
- OpenDAL currently trims surrounding whitespace, removes leading `/`,
  collapses repeated `/`, and maps an empty input to root;
- a directory path and directory entry name end with `/`; a file path does not;
- `create_dir` requires a trailing `/`, is recursive, and is idempotent.

The MoonBit wrapper passes paths to the Rust boundary as UTF-8 and does not add
its own competing normalization layer. Invalid text or lengths raise
`InvalidArgument`.

## Whole-object operations

The core synchronous methods are:

```moonbit
op.exists(path) -> Bool raise OpenDalError
op.stat(path, version?, if_match?, if_none_match?) -> Metadata raise OpenDalError
op.read(path, range?, version?, if_match?, if_none_match?) -> Bytes raise OpenDalError
op.write(path, data : BytesView, append?, content_type?, content_disposition?, content_encoding?, cache_control?, if_match?, if_none_match?) -> Metadata raise OpenDalError
op.create_dir(path) -> Unit raise OpenDalError
op.delete(path, version?, recursive?) -> Unit raise OpenDalError
op.copy(source, destination) -> Metadata raise OpenDalError
op.rename(source, destination) -> Unit raise OpenDalError
```

Important guarantees and non-guarantees:

- `write` either writes all supplied bytes or raises and returns the resulting
  metadata on success.
- The binding does not promise that every backend write is atomic or creates
  missing parent directories.
- deleting a missing path succeeds.
- recursive deletion is expressed by `recursive=true`, not a separate
  `remove_all` convenience operation.
- `copy` and `rename` retain upstream semantics. The binding never implements
  rename as copy plus delete because that would change atomicity and failure
  behavior.
- `copy` returns the metadata supplied by the backend. It can be partial; use
  `stat(destination)` when complete destination metadata is required.
- unsupported operations raise `Unsupported`; they are not silently emulated.

## Batch delete and Copier

`Operator::delete_many(paths : ArrayView[String])` is an all-or-error facade
over OpenDAL's high-level deleter. The binding copies and validates every path
before backend work starts; empty input succeeds. A successful `Unit` means the
complete request finished. An error can follow partial remote effects and does
not contain per-path outcomes. The API promises neither atomicity, input-order
preservation, nor uniqueness; `Capability::delete_max_size()` is a backend
hint rather than a binding-wide batch limit.

Incremental copy is a stateful resource:

```moonbit
let copier = op.open_copier(source, destination)
while copier.next() is Some(delta) {
  consume_progress(delta)
}
let metadata = copier.finish()
```

The two paths always belong to the same Operator, and one Copier transfers one
object. It is not recursive, cross-Operator, or cross-service, and the binding
does not emulate those scopes with list/read/write loops. `next` returns a byte
delta or stable `None`; `finish` drives any remaining steps and consumes the
retained destination metadata. A failure is terminal. Successful abort is
idempotent but cannot roll back remote effects already visible. The Copier
owns all native state it needs, can outlive its originating Operator, and is
never implicitly finished or reported aborted by finalization.

## Listing

OpenDAL listing is prefix-based, not a traditional filesystem directory read:

- the parent directory need not exist;
- if the prefix itself exists it may be returned with its descendants;
- deeper matching objects can be returned even when the parent is absent;
- non-recursive listing means immediate children when the backend supports a
  delimiter;
- `limit` is a backend page/request hint, not a guaranteed total
  result bound.

Two forms are exposed:

```moonbit
op.list(path, recursive?, limit?, start_after?) -> Array[Entry] raise OpenDalError
op.open_lister(path, recursive?, limit?, start_after?) -> Lister raise OpenDalError
lister.next() -> Entry? raise OpenDalError
lister.close() -> Unit
```

`next` preserves the three native states: entry, end, and error. After an
error, the lister is exhausted. `close` is idempotent and releases native
resources early; subsequent `next` calls raise `ResourceClosed`.

The first API does not coerce `Lister` into `Iter[Entry]`. MoonBit's ordinary
iterator step does not naturally carry checked I/O errors, and hiding errors in
an iterator would weaken the contract.

## Reader

The Reader follows OpenDAL's random-access Reader rather than the cursor-bearing
C `StdReader` adapter:

```moonbit
let reader = op.open_reader(path, if_match="etag")
let bytes = reader.read(Range(offset=4096UL, length=1024UL))
reader.close()
```

`ByteRange` is an algebraic value:

```moonbit
Full
From(offset~ : UInt64)
Range(offset~ : UInt64, length~ : UInt64)
Suffix(length~ : UInt64)
```

The second `Range` value is a length, not an end offset. A short byte result
means the requested range reached object end. Calls do not mutate a hidden
cursor, so the same Reader can serve independent ranges. `Full` and `From`
are sent directly as read ranges without a preliminary `stat`; version and
conditions therefore apply to the same read request instead of a racy
stat-then-read pair. A backend can report `RangeNotSatisfied` for a `From`
offset beyond EOF rather than returning an empty result.

`Suffix` is available only when `Operator::info().capability.can_read_suffix()`
is true. That bit means the selected backend supports suffix ranges natively;
the binding deliberately does not use OpenDAL's stat-based suffix simulation.
Using `Suffix` when the bit is false raises `Unsupported` for whole reads,
Reader reads, and read streams.

`close` is idempotent; later reads raise `ResourceClosed`. GC finalization is a
leak-safety backstop, not the preferred way to release a Reader promptly.

Every returned value must fit in one MoonBit `Bytes`. Requests or whole-object
reads exceeding that representable length raise `BufferTooLarge`; the binding
checks each upstream buffer before extending its own output and never collects
the complete OpenDAL stream first. Callers read large objects with a
`ReadStream` or in bounded independent ranges.

This is a hard bound on binding-owned output and copy allocations, not on all
memory inside OpenDAL or a backend. OpenDAL 0.58 does not bound the size of one
raw streaming buffer, so at most one such buffer can be live outside the
binding's output limit.

The same checked-allocation rule applies to native output strings and
materialized entry arrays. The wrapper releases any partially converted native
snapshots before raising `BufferTooLarge`.

## Read streams

Large sequential reads use a distinct cursor-bearing resource. The existing
`Reader` remains random access and never acquires a hidden position:

```moonbit
let stream = op.open_read_stream(
  "large.bin",
  range=From(offset=4096UL),
  chunk_size=1024 * 1024,
)
while stream.next() is Some(chunk) {
  consume(chunk)
}
stream.close()
```

The public surface is:

```moonbit
Operator::open_read_stream(
  path,
  range?=Full,
  chunk_size?=1024 * 1024,
  version?,
  if_match?,
  if_none_match?,
) -> ReadStream raise OpenDalError
ReadStream::next() -> Bytes? raise OpenDalError
ReadStream::close() -> Unit
```

`ReadStream` is deliberately not named `SequentialReader`: it is a stateful
byte stream, while `Reader` already names the reusable random-access resource.
It is not coerced to `Iter[Bytes]`, because an ordinary iterator step cannot
preserve checked I/O errors.

`chunk_size` is an `Int`, matching the length and allocation domain of MoonBit
`Bytes`; remote object offsets and ranges remain `UInt64`. It is fixed when the
stream is opened, and every returned native buffer, ABI copy, and MoonBit
`Bytes` is no larger than that bound. The default is 1 MiB. Values must be
positive and no larger than the negotiated native output ceiling or
`Int::MAX`; invalid values raise `InvalidArgument` before a native reader is
retained.

The binding leaves OpenDAL's upstream chunk option unset to avoid an implicit
`stat` for open-ended ranges. It splits each raw buffer locally and does not
poll upstream again while a remainder is pending. The pending buffer uses
shared backing storage, so one backend-provided buffer can be larger than
`chunk_size`; `chunk_size` is therefore a returned-output/copy bound, not a
hard bound on total native or backend memory.

The state machine is:

- `Open -> End` when upstream reaches EOF; `next` then returns `None` forever;
- `Open -> Failed` on an OpenDAL error or contained native panic; the failing
  call reports that failure and later `next` calls raise `ResourceClosed`;
- `Open -> Closed` on explicit `close`; `close` is idempotent and later `next`
  raises `ResourceClosed`;
- the handle owns everything it needs and can outlive its originating
  `Operator`;
- calls on one handle serialize; distinct handles and ordinary `Reader` calls
  can proceed concurrently;
- there is no implicit concurrency or prefetch, and the cursor retains at most
  one upstream buffer while returning bounded chunks from it.

Opening a stream constructs the cursor without a preliminary storage `stat`.
For backends that open lazily, missing-object, condition, or range failures can
therefore be reported by the first `next` rather than by
`open_read_stream`.

`Operator::open_read_stream` and `ReadStream::next` use `Operation::Read` for
error context. The new lifecycle API does not extend the already public,
exhaustively matchable `Operation` enum.

## Writer

```moonbit
let writer = op.open_writer(path, content_type="application/octet-stream")
writer.write(chunk1)
writer.write(chunk2)
let metadata = writer.finish()

let speculative = op.open_writer("scratch.bin")
speculative.write(chunk1)
speculative.abort()
speculative.abort() // successful abort is idempotent
```

- `write` writes an entire supplied chunk or raises.
- any `write` failure is terminal and moves the Writer to `Failed`;
- only a successful explicit `finish` lets the binding report the Writer as
  successfully completed;
- the first finish attempt is terminal: success produces `Finished`, failure
  produces `Failed`;
- the first abort attempt is terminal: success produces `Aborted`, failure
  produces `Failed`;
- repeating a successful `abort` is harmless, which makes explicit cleanup
  paths composable; aborting a finished or failed Writer raises
  `ResourceClosed`;
- an abort/finish or abort/write race has one winner; the losing call observes
  `ResourceClosed` and never starts a second upstream operation;
- later `write` or `finish` calls raise `ResourceClosed`;
- dropping/finalizing an open or failed Writer never calls `finish` and reports
  neither finish nor abort success.

The native shim owns OpenDAL's async Writer and synchronously drives its
explicit `abort`. A successful abort means OpenDAL reported that cleanup
succeeded; it does not promise rollback of effects a backend had already made
visible. If a write, finish, abort, panic, or finalizer ends the resource without
a successful abort, partial data or orphan multipart state can remain. Both
`finish` and `abort` use `Operation::Write` for error context.

## Presigned requests

`presign_read`, `presign_write`, and `presign_stat` return a fully owned
`PresignedRequest`; they do not execute HTTP. Expiry is a required positive
number of seconds, and the ordinary read/write/stat options remain explicit.
The result exposes `http_method`, `uri`, and an ordered array of
`PresignedHeader`. Header values are `Bytes`, because valid HTTP field values
need not be UTF-8, and duplicate names are preserved rather than collapsed.

The URI and headers can contain bearer-equivalent signing material. Neither
`PresignedRequest` nor `PresignedHeader` derives `Debug` or `Show`, and callers
must not log or attach them to ordinary error reports. The application owns the
choice of HTTP client, request execution, body, and response handling.

## Metadata and entries

`Metadata` and `Entry` are immutable snapshots rather than native builders.
List metadata can be partial and is tied to the listing operation's context.

- `Entry.path` is relative to the operator root.
- directory `path` and `name` end with `/`.
- timestamp values retain Unix seconds and nanoseconds instead of adopting the
  OCaml binding's second-only representation.
- optional HTTP metadata, ETag, version, and last-modified values remain
  optional.
- OpenDAL's public `content_length` getter returns zero both for an empty
  object and when a service did not provide the field. The first binding
  version preserves that `UInt64` behavior rather than inventing a false
  `Option` distinction that the public Rust API cannot supply.

## Optional arguments and capabilities

Each operation has one canonical method with optional labelled arguments. The
binding does not expose Rust-shaped options records or native options handles.
Native code copies every view or collection that survives a call. Omitted
booleans default to `false`; omitted strings and limits are absent.

Capability values are read-only snapshots. `OperatorInfo.capability` exposes
only operations and options callable through the current MoonBit facade, while
preserving the backend's answer for those features. The suffix-read bit is the
intentional exception to OpenDAL's composed snapshot: it is true only when the
base service supports suffix ranges natively, because the binding does not
expose OpenDAL's stat-based simulation as a callable suffix feature.

Capability getters are introspection. They do not imply that the binding can
prevalidate every backend request, and OpenDAL may ignore, degrade, or reject
unsupported options according to the operation and service. The MoonBit layer
must not claim a stricter universal policy unless it implements and documents
one.

Reader, Writer, copy, rename, suffix-read, append, and presign capability
accessors are available with their corresponding MoonBit operations. The
optional batch-delete size hint is exposed separately from boolean support.

## Operational layer policy

No operational layer is installed by default. `with_timeout`, `with_retry`,
and `with_concurrency_limit` each return a separately owned derived Operator;
the input Operator and resources already opened from it do not change.

The only supported composition order is timeout, then retry, then concurrency
limit. `max_retries` counts attempts after the initial request. Retry is
opt-in, acts only on errors selected by its native policy, and does not promise
exactly-once Writer or append semantics: an uncertain remote commit can be
replayed. The concurrency limit is installed at most once, is always the
outermost layer, and prevents a later timeout or retry layer from changing what
one operation permit represents.

`operation_limit` applies to OpenDAL operations. For a body-style resource,
including a Reader-created stream, Writer, Lister, Deleter, or Copier, its
permit is retained for the body lifetime. The optional
`http_request_limit` is a distinct transport limit whose permit is retained
until the HTTP response body is dropped. Both limits must be positive; there
is no implicit or environment-selected limit.

## Portable async facade

The root `Eric-Song-Nop/opendal` package supports native and JavaScript. Target
selection happens at compile time: native is the module default, while browser
builds pass `--target js`. Both targets expose the same portable entry point;
the JavaScript implementation lazily initializes its embedded OpenDAL Wasm
runtime and the native implementation constructs the native operator directly.

`Operator::as_async()` remains available when native callers first construct
or layer a synchronous Operator. The portable async surface is:

```moonbit nocheck
async fn AsyncOperator::new(...) -> AsyncOperator
fn AsyncOperator::close() -> Unit
async fn AsyncOperator::write(...) -> Metadata
async fn AsyncOperator::read(...) -> Bytes
async fn AsyncOperator::open_read_stream(...) -> AsyncReadStream
async fn AsyncReadStream::next() -> Bytes?
fn AsyncReadStream::close() -> Unit
async fn AsyncOperator::open_writer(...) -> AsyncWriter
async fn AsyncWriter::write(BytesView) -> Unit
async fn AsyncWriter::finish() -> Metadata
async fn AsyncWriter::abort() -> Unit
```

Every operation except resource `close` is an ordinary MoonBit `async fn`;
async functions carry their raising effect implicitly, and MoonBit has no
`await` keyword. Close is idempotent, does not raise, and releases the selected
operator early. `AsyncOperator::write` uses the stateful async Writer rather
than calling the blocking native whole-write operation. No native task handle
or callback enters the public API. Native workers own copied inputs and
results, signal readiness through a private pipe, and never call MoonBit from a
foreign thread. JavaScript operations wait on Promises and connect MoonBit
cancellation to the browser bridge.

An AsyncReadStream and AsyncWriter admit one in-flight operation. Cancelling
`next`, `write`, `finish`, or `abort` makes that resource terminal because
cursor or commit progress may be unknown; already-visible remote effects are
not rolled back. Async stream calls return one output-bounded owned chunk at a
time and do not poll upstream while a locally split remainder is pending. The
same one-raw-buffer caveat as synchronous streams applies. Whole/ranged async
`read` still materializes one output-bounded `Bytes` value. Async stat,
list/lister, delete, copy/Copier, presign, and public task handles remain
outside the portable native slice. The JavaScript target exposes additional
capability-checked async operations and `AsyncLister` as target extensions.

Whole-object values in the portable facade are limited to 64 MiB. Read-stream
outputs and individual Writer inputs are limited to 256 KiB, and native whole
writes split larger accepted inputs into bounded Writer calls. Async catch
clauses receive MoonBit's general `Error`; `OpenDalError::from_error` recovers
the structured OpenDAL value with the same API on either target.

Callers close or finish child streams and writers and await in-flight calls
before closing their AsyncOperator. Whether already-open children complete or
become terminal after a parent is closed is deliberately outside the portable
contract because the native handle and browser runtime have different
ownership trees.

## Check semantics

`Operator::check` means the OpenDAL check operation, currently implemented via
a root listing and treating `NotFound` as success. It is not documented as a
comprehensive health, credential, read/write, or consistency check.

## Current non-goals

- portable native async parity beyond whole writes/reads, bounded streams, and
  chunked Writers;
- callback adapters and public native task handles;
- presigned delete or methods beyond read/write/stat;
- ordered per-path batch results or transactional rollback;
- recursive, cross-Operator, or cross-service Copier transfers;
- logging, tracing, metrics, and callback-oriented custom layers;
- exhaustive exposure of every service-specific configuration;
- public raw handles or raw C/Rust error objects;
- automatic operation emulation;
- a guarantee that every enabled service behaves like a filesystem.

## Primary references

- [OpenDAL v0.58.1 blocking Operator](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/blocking/operator.rs)
- [OpenDAL error model](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/types/error.rs)
- [OpenDAL metadata](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/types/metadata.rs)
- [OpenDAL operation options](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/types/options.rs)
- [OpenDAL blocking Reader](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/blocking/read/reader.rs)
- [OpenDAL blocking Writer](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/blocking/write/writer.rs)
- [OpenDAL OCaml public interface](https://github.com/apache/opendal/blob/v0.58.1/bindings/ocaml/lib/operator.mli)
- [MoonBit error handling](https://docs.moonbitlang.com/en/stable/language/error-handling.html)
- [MoonBit package access control](https://docs.moonbitlang.com/en/latest/language/packages.html#access-control)
- [MoonBit optional arguments](https://docs.moonbitlang.com/en/latest/language/fundamentals.html#optional-arguments)
