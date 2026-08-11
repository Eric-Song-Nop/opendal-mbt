# Public API Semantics

Status: Phase 3 complete

This document defines the intended MoonBit-facing behavior of the synchronous
OpenDAL binding. It deliberately avoids fixing the native ABI or implementation
layout; those are Phase 1 concerns and must implement this contract.

This document defines the implemented synchronous Reader, Writer, copy, and
rename semantics. The generated `src/pkg.generated.mbti` is the authoritative
current public surface.

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

`Operator` is logically immutable and shareable. `Lister` and `Writer` are
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
large objects in bounded ranges.

The same checked-allocation rule applies to native output strings and
materialized entry arrays. The wrapper releases any partially converted native
snapshots before raising `BufferTooLarge`.

## Writer

```moonbit
let writer = op.open_writer(path, content_type="application/octet-stream")
writer.write(chunk1)
writer.write(chunk2)
let metadata = writer.finish()
```

- `write` writes an entire supplied chunk or raises.
- any `write` failure is terminal and moves the Writer to `Failed`;
- only a successful explicit `finish` lets the binding report the Writer as
  successfully completed;
- the first finish attempt is terminal: success produces `Closed`, failure
  produces `Failed`;
- later `write` or `finish` calls raise `ResourceClosed`;
- dropping/finalizing an open or failed Writer never calls `finish` and reports
  neither success nor an error.

No public `abort` is frozen yet. OpenDAL's blocking Writer does not expose a
reliable abort operation. Before finish succeeds, a write/finish failure or drop
can therefore leave visible partial data or orphan multipart state depending
on the backend. The binding promises neither rollback nor “no partial
effects”; callers treat only a successful `finish` as completed. An abort API
can be added only if the Rust shim owns an async Writer and can define its
synchronous failure behavior precisely.

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

## Check semantics

`Operator::check` means the OpenDAL check operation, currently implemented via
a root listing and treating `NotFound` as success. It is not documented as a
comprehensive health, credential, read/write, or consistency check.

## Initial non-goals

- async or callback APIs;
- presigned requests and middleware/layers;
- batch deletion and long-running copier APIs;
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
