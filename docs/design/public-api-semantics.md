# Public API Semantics

Status: Phase 5A local-lifecycle contract frozen; Phase 5B-E API direction
reviewed but not implemented

This document defines the intended MoonBit-facing behavior of the OpenDAL
binding. It deliberately avoids fixing native ABI layouts; each implementation
phase must design an append-only ABI slice that satisfies this contract.

This document defines the implemented synchronous Reader, Writer, copy, and
rename semantics, then records forward contracts for Phase 5B-E. Forward
sections are design constraints, not claims that the APIs already ship. The
generated `src/pkg.generated.mbti` is the authoritative current public surface.

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
| Native resources | `Operator`, `Lister`, `Reader`, and `Writer` | Opaque and impossible for callers to fabricate |
| Read-only snapshots | `Metadata`, `Entry`, `ErrorInfo`, `OperatorInfo`, `Timestamp` | Fields are readable but values are produced by the binding |
| Read-only algebraic outputs | `EntryMode`, `ErrorKind`, `ErrorStatus`, `Operation` | Callers can inspect and pattern-match values but cannot fabricate them |
| Algebraic input | `ByteRange` | Uses `pub(all)` so callers can construct labelled ranges |
| Extensible query object | `Capability` | Opaque effective-capability snapshot with getter methods so new capabilities can be added compatibly |

Planned Phase 5 concrete types—including `S3Auth`, `PresignedRequest`,
`PresignedHeader`, `Copier`, `AsyncOperator`, `AsyncReadStream`, and
`AsyncWriter`—are owned by this same root facade package. They must not be
defined in or re-exported from `internal/*` packages merely because their
implementations are feature-specific.

`Operator` is logically immutable and shareable. `Lister`, `ReadStream`, and
`Writer` are stateful. `Reader` is a random-access reader without an implicit
cursor.

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

## Standard profile and typed S3 construction (planned Phase 5B)

The `v0.1.x` `local` artifact continues to mean exactly `memory` plus `fs`.
The next cloud-capable root-package release automatically resolves one
`standard` artifact containing `memory`, `fs`, and `s3`. Consumers still import
`Eric-Song-Nop/opendal`; they do not choose a profile through a MoonBit value,
a service subpackage, or an environment variable. Only the host variant is
selected. `local` and `standard` can coexist in the cache by version, profile,
target, and digest, but they are never linked into one process together.

S3 receives a typed root-facade constructor while the generic
`Operator::new("s3", config=...)` escape hatch remains available:

```moonbit nocheck
pub type S3Auth
pub type S3CredentialSource

pub fn S3Auth::default_chain() -> S3Auth
pub fn S3Auth::static_credentials(
  access_key_id~ : StringView,
  secret_access_key~ : StringView,
  session_token? : StringView,
) -> S3Auth
pub fn S3Auth::unsigned() -> S3Auth
pub fn S3Auth::assume_role(
  role_arn~ : StringView,
  source? : S3CredentialSource,
  external_id? : StringView,
  role_session_name? : StringView,
  duration_seconds? : UInt,
) -> S3Auth

pub fn S3CredentialSource::default_chain() -> S3CredentialSource
pub fn S3CredentialSource::static_credentials(
  access_key_id~ : StringView,
  secret_access_key~ : StringView,
  session_token? : StringView,
) -> S3CredentialSource

pub fn Operator::s3(
  bucket : StringView,
  region~ : StringView,
  root? : StringView,
  endpoint? : StringView,
  auth? : S3Auth,
  virtual_host_style? : Bool = false,
) -> Operator raise OpenDalError
```

Omitted `auth` means the default credential chain. The direct
`default_chain` and `static_credentials` helpers cover the common signed
modes; `S3CredentialSource` exists only to prevent unsigned or nested
assume-role modes from being supplied as role source credentials. Omitted
assume-role `source` also means the default chain. These types are opaque and
do not implement `Show` or `Debug`; each helper copies its views, and
`Operator::s3` performs the checked cross-field validation.

`bucket` and the labelled signing `region` are both required and non-empty,
including for a custom endpoint. Requiring the region makes signing behaviour
deterministic and avoids construction-time region discovery. `root` and
`endpoint` are optional owned copies; path-style addressing is the default and
`virtual_host_style=true` explicitly selects the upstream virtual-host mode.

The four supported authentication behaviours map directly to OpenDAL 0.58.1:

- default chain: OpenDAL loads its supported environment, shared-config, and
  instance-metadata sources;
- static credentials: both key fields are required and an optional session
  token is passed through;
- unsigned: credential loading and request signing are skipped; and
- assume role: OpenDAL uses the supplied default-chain or static source plus
  the supported role ARN, external ID, session name, and duration fields.

There is deliberately no `profile_name` parameter. The pinned OpenDAL S3
builder does not expose per-operator named-profile selection. This binding
will neither mutate `AWS_PROFILE` around construction nor parse credential
files in MoonBit to imitate it. The default chain may observe `AWS_PROFILE`
when it is already set before process startup, but that is ambient upstream
chain behaviour, not an `Operator::s3` named-profile argument. Typed
named-profile support waits for a real, concurrency-safe upstream capability.

No secret-bearing URI constructor is provided. Authentication values must be
redacted from `OpenDalError`, debug output, snapshots, native diagnostics,
artifact metadata, and cache paths. The generic map escape hatch follows the
same redaction contract even though it cannot provide the typed constructor's
conflict checks.

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
- end of a lister;
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
cursor, so the same Reader can serve independent ranges.

`close` is idempotent; later reads raise `ResourceClosed`. GC finalization is a
leak-safety backstop, not the preferred way to release a Reader promptly.

Every returned value must fit in one MoonBit `Bytes`. Requests or whole-object
reads exceeding that representable length raise `BufferTooLarge`; callers read
large objects with a `ReadStream` or in bounded independent ranges.

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
stream is opened so every `next` has the same memory and backpressure bound.
The default is 1 MiB. Values must be positive and no larger than the negotiated
native output ceiling or `Int::MAX`; invalid values raise `InvalidArgument`
before a native reader is retained.

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
- there is no implicit concurrency or prefetch, so opening a stream does not
  weaken its one-chunk bound.

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
- the first finish attempt is terminal: success produces `Closed`, failure
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
preserving the backend's answer for those features. OpenDAL 0.58 removed the
older native/full split from its public `OperatorInfo`; this binding does not
reconstruct it from raw internals.

Capability getters are introspection. They do not imply that the binding can
prevalidate every backend request, and OpenDAL may ignore, degrade, or reject
unsupported options according to the operation and service. The MoonBit layer
must not claim a stricter universal policy unless it implements and documents
one.

Reader, Writer, copy, rename, suffix-read, and append capability accessors are
available with their corresponding MoonBit operations.

## Retry policy

The first binding version does not install a retry layer. This keeps failures
and latency faithful to the configured service and avoids inheriting the
upstream experimental C binding's policy. A future retry API must be explicit
in the MoonBit surface and must document idempotency and Writer behavior.

## Presigned requests (planned Phase 5C.1)

Presigning returns an executable, fully owned request snapshot in the root
facade:

```moonbit nocheck
pub struct PresignedHeader {
  name : String
  value : Bytes
}

pub struct PresignedRequest {
  method : String
  uri : String
  headers : Array[PresignedHeader]
}

Operator::presign_read(
  path,
  expires_in_seconds~ : UInt64,
  range?=Full,
  version?,
  if_match?,
  if_none_match?,
) -> PresignedRequest raise OpenDalError
Operator::presign_write(
  path,
  expires_in_seconds~ : UInt64,
  append?=false,
  content_type?,
  content_disposition?,
  content_encoding?,
  cache_control?,
  if_match?,
  if_none_match?,
) -> PresignedRequest raise OpenDalError
Operator::presign_stat(
  path,
  expires_in_seconds~ : UInt64,
  version?,
  if_match?,
  if_none_match?,
) -> PresignedRequest raise OpenDalError
```

`PresignedRequest` and `PresignedHeader` are read-only snapshots. `method`,
`uri`, and header names are validated strings, but header values remain
`Bytes`: valid HTTP field values need not be UTF-8. `headers` is an array, not
a map, and preserves upstream iteration order and duplicate names. Neither the
MoonBit conversion nor the native ABI may combine, sort, or overwrite repeated
headers.

`expires_in_seconds` is required, positive, range-checked before conversion to
the native duration, and unit-bearing in the parameter name. The operation
options intentionally mirror the corresponding facade operation rather than
exporting an OpenDAL options record. A service that does not support the
specific presign operation raises `Unsupported`; capability getters distinguish
read, write, and stat presigning.

The returned URI and headers can contain bearer-equivalent signing material.
They are not included in `Show`, errors, ordinary debug logs, or snapshots by
default. Tests execute the request against the integration service and also
verify duplicate/binary header preservation; comparing only a URL string is
not sufficient acceptance evidence.

## Retry and timeout layers (planned Phase 5C.2)

Layers produce a new immutable `Operator`; they never mutate an existing
operator or resources already opened from it. The first facade uses methods
with labelled scalar arguments rather than Rust-shaped layer builders:

```moonbit nocheck
let resilient = op
  .with_timeout(
    operation_timeout_millis=10_000UL,
    io_timeout_millis=3_000UL,
  )
  .with_retry(
    max_retries=3U,
    min_delay_millis=50UL,
    max_delay_millis=2_000UL,
    jitter=true,
  )
```

`with_timeout` takes required positive control-operation and I/O-body budgets.
`with_retry` takes a required finite retry count and optional validated
backoff bounds/jitter. Exact signatures and numeric ceilings are
compiler-checked before the slice is frozen, but no public Rust builder or
callback interceptor is exposed.

The call order above is the supported composition: the timeout layer is added
first and retry wraps it, so each retry attempt completes or times out before
the retry wrapper must restore its stateful body. Reversing that known-unsafe
order or adding duplicate retry/timeout layers is rejected at construction
rather than accepted with undefined state restoration.

Retry is wholly opt-in and responds only to errors OpenDAL marks temporary;
exhaustion produces the documented persistent status. It is not an
exactly-once facility. The pinned upstream retry layer can repeat a Writer
chunk, finish, or abort call after the remote service performed the action but
the response was lost. Therefore:

- append and other non-idempotent writes can duplicate data;
- a conditional overwrite may reduce risk but does not create a universal
  transaction;
- finish or abort can be attempted more than once below the public Writer
  state machine; and
- a final `OpenDalError` can coexist with partial or complete remote effects.

The public Writer still becomes terminal after its method returns an error,
but that terminal transition says nothing about how many hidden attempts ran.
Documentation and tests use response-loss/uncertain-commit failures and never
claim that replay is safe merely because the final error is `Persistent`.

## High-level batch delete and Copier (planned Phase 5D)

The first batch surface binds OpenDAL's high-level deleter contract:

```moonbit nocheck
Operator::delete_many(paths : ArrayView[String]) -> Unit raise OpenDalError
```

The binding copies and bounds the input before starting. An empty input
succeeds. Equal submitted path/options pairs can be deduplicated by OpenDAL's
unordered batch buffer, and backend batches can be issued in an order unrelated
to the input. Success means all retained entries were flushed and the deleter
closed. An error is all-or-error API data, not proof that nothing happened: an
unspecified subset may already be gone.

Consequently `delete_many` does not return an ordered per-input array, a
partial-success array, or an atomicity flag. The binding cannot reconstruct
those values after upstream deduplication and unordered flushing. A future
best-effort convenience loop, if wanted, has a different name and explicitly
non-native semantics. `Capability::delete_max_size()` exposes the backend's
per-request batch hint; it is distinct from the binding's total input ceiling.

OpenDAL Copier is exposed as a same-Operator, single-object lifecycle:

```moonbit nocheck
Operator::open_copier(
  source : StringView,
  destination : StringView,
) -> Copier raise OpenDalError
Copier::next() -> UInt64? raise OpenDalError
Copier::finish() -> Metadata raise OpenDalError
Copier::abort() -> Unit raise OpenDalError
```

`next` advances one upstream step and returns that step's byte-count delta.
`None` means upstream completed and its destination metadata is retained until
`finish`; repeated `next` before `finish` remains `None`. `finish` can be called
earlier to drive all remaining steps, then returns the destination metadata.
Progress conversion to `UInt64` is checked.

A `next` or `finish` error is terminal. A successful `abort` is explicit and
idempotent; aborting a completed, finished, or failed Copier raises
`ResourceClosed`. An abort/next or abort/finish race has one winner. The handle
owns the operator state it needs and can outlive the `Operator`, while its
finalizer only frees memory: it neither finishes nor reports a successful
abort, and backend intermediate state can remain. All errors use
`Operation::Copy` with source and destination context, avoiding a new
exhaustively matchable enum case.

Both Copier paths are relative to one `Operator`. It is not a recursive tree
copy and cannot accept a second service/operator. Recursive and cross-service
work belongs to a future `Transfer` resource with separate traversal,
Reader/Writer, progress, and partial-failure semantics. It is not an overload
or hidden mode of `Copier`.

## Public async facade (planned Phase 5E)

Async remains in the root facade with distinct resource types and natural
method names:

```moonbit nocheck
pub async fn AsyncOperator::new(
  scheme : StringView,
  config? : Map[String, String] = Map([]),
) -> AsyncOperator
pub async fn AsyncOperator::read(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
) -> Bytes
pub async fn AsyncOperator::write(
  self : AsyncOperator,
  path : StringView,
  data : BytesView,
) -> Metadata
pub async fn AsyncOperator::open_read_stream(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  chunk_size? : Int = 1024 * 1024,
) -> AsyncReadStream
pub async fn AsyncReadStream::next(self : AsyncReadStream) -> Bytes?
pub fn AsyncReadStream::close(self : AsyncReadStream) -> Unit
pub async fn AsyncOperator::open_writer(
  self : AsyncOperator,
  path : StringView,
) -> AsyncWriter
pub async fn AsyncWriter::write(
  self : AsyncWriter,
  chunk : BytesView,
) -> Unit
pub async fn AsyncWriter::finish(self : AsyncWriter) -> Metadata
pub async fn AsyncWriter::abort(self : AsyncWriter) -> Unit
```

The abbreviated signatures above inherit the synchronous methods' labelled
operation options; they are not a reduced semantic fork. MoonBit async
functions raise by default, so source declarations do not add an explicit
`raise`. There are no `_async` suffixes, public callback-taking variants,
public native task handles on the ordinary path, or public types owned by an
`async/internal` package. Typed `AsyncOperator::s3` mirrors `Operator::s3` once
the first generic slice proves the bridge.

The bridge never calls MoonBit from a Tokio worker. It uses the supported
public `moonbitlang/async/pipe` package and its `PipeRead`/`PipeWrite::fd`
surface; it does not import `moonbitlang/async/internal/*`. Each call creates a
Rust-owned task/result cell. Native code duplicates and owns the write
descriptor, while MoonBit awaits the private facade-owned `PipeRead`. A Tokio
worker stores exactly one terminal result and writes a readiness token to its
OS descriptor; MoonBit drains the token, then takes the result through FFI on
the MoonBit thread. No pipe type appears in the OpenDAL public API, but pipe
ownership, platform support, wake coalescing, error, and shutdown rules are
part of the public design contract.

Native task state contains no MoonBit closure or object, so it needs no
foreign-thread GC rooting. Cancellation of the calling MoonBit task requests
native cancellation and uses the same wake path; exactly one of success,
OpenDAL error, contained panic, or cancellation becomes takeable. Remote side
effects that happened before cancellation are not rolled back. Late native
completion is safely discarded after ownership has been resolved, and
finalizers never block waiting for Tokio.

An `AsyncReadStream` has at most one in-flight `next` and retains the same
one-chunk backpressure bound as `ReadStream`. An `AsyncWriter` has at most one
in-flight `write`, `finish`, or `abort` and follows the same terminal lifecycle
as `Writer`. The async executor thread never performs a blocking `block_on`,
and synchronous resources continue using their existing facade without nested
runtime waits.

## Check semantics

`Operator::check` means the OpenDAL check operation, currently implemented via
a root listing and treating `NotFound` as success. It is not documented as a
comprehensive health, credential, read/write, or consistency check.

## Current-release non-goals

- the planned Phase 5B-E APIs above until their complete vertical slices land;
- callback APIs, even after the pipe-based async facade lands;
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
- [OpenDAL S3 builder](https://github.com/apache/opendal/blob/v0.58.1/services/s3/src/backend.rs)
- [OpenDAL presign operations](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/types/operator/operator.rs)
- [OpenDAL retry layer](https://github.com/apache/opendal/blob/v0.58.1/layers/retry/src/lib.rs)
- [OpenDAL timeout layer](https://github.com/apache/opendal/blob/v0.58.1/layers/timeout/src/lib.rs)
- [OpenDAL batch deleter](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/raw/oio/delete/batch_delete.rs)
- [OpenDAL Copier](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/types/copy.rs)
- [OpenDAL OCaml public interface](https://github.com/apache/opendal/blob/v0.58.1/bindings/ocaml/lib/operator.mli)
- [MoonBit error handling](https://docs.moonbitlang.com/en/stable/language/error-handling.html)
- [MoonBit package access control](https://docs.moonbitlang.com/en/latest/language/packages.html#access-control)
- [MoonBit optional arguments](https://docs.moonbitlang.com/en/latest/language/fundamentals.html#optional-arguments)
