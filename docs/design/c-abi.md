# OpenDAL MoonBit C ABI

Status: ABI v1.7 implemented; append-only v1 extensions frozen through the
initial async facade

The executable header is `native/include/opendal_mbt.h`. It is the canonical
source for exact field order, numeric constants, and function signatures. This
document defines the rules that are not expressible in C syntax.

## Boundary and scope

The stable native boundary is deliberately independent of the MoonBit runtime:

```text
safe MoonBit values and typed errors
  -> private MoonBit native declarations
  -> in-package C stub that includes moonbit.h
  -> this pure C ABI
  -> project-owned Rust static library
  -> OpenDAL 0.58.1
```

Only the C stub may see `moonbit.h`. The Rust library never receives or returns
a MoonBit string, `moonbit_bytes_t`, GC reference, external object, closure, or
reference-counted value. This is important because `moonbit.h` explicitly
describes itself as an unstable runtime API; it must not become the durable
Rust ABI.

ABI v1 covers the synchronous public contract, bounded read streams, Writer
abort, the standard S3 constructor, presigning, explicit timeout/retry/
concurrency-limit layers, batch delete, managed Copier operations, and the
initial asynchronous read/stream/writer facade. It intentionally excludes
foreign callbacks, public native task handles, full async parity, and raw
service-specific handles.

## Core choices

- The library exports one bootstrap symbol, `opendal_mbt_get_api`.
- All other entry points are obtained through an append-only v1 function
  table.
- The ABI uses fixed-width integer types, pointers, opaque typed handles, and
  `#[repr(C)]` data only.
- Rust owns every native handle and output snapshot. Only its paired API
  function may free one.
- Inputs are explicit-length borrowed views. No API accepts a NUL-terminated C
  string.
- Read results are immutable opaque buffers with a two-phase length/copy
  protocol; Rust allocation internals never cross the boundary.
- Errors, metadata, entries, and operator information are immutable owned
  snapshots with borrowed inspection views.
- Operation and path context belong to the safe MoonBit call site, not the
  OpenDAL error object.
- ABI v1 installs no layer implicitly. Retry, timeout, and concurrency limits
  are selected only through their feature-gated functions.

The upstream C binding remains useful as implementation evidence, but this
project does not depend on its symbols or ABI.

## Bootstrap and compatibility

The caller allocates and zeroes `opendal_mbt_api_v1_t`, then sets:

```c
api.struct_size = sizeof(api);
api.requested_major = OPENDAL_MBT_ABI_V1_MAJOR;
status = opendal_mbt_get_api(&api);
```

The bootstrap contract is:

1. `struct_size` and `requested_major` are a permanent common prefix. The
   caller always provides readable/writable storage through
   `OPENDAL_MBT_API_V1_INPUT_SIZE`, even when the declared `struct_size` value
   is invalidly smaller.
2. `inout_api` is required, and `struct_size` must cover at least
   `OPENDAL_MBT_API_V1_PREFIX_SIZE`; a smaller table or unsupported major gets
   `ABI_MISMATCH` without modifying the output payload.
3. After validating that input prefix, the library clears output bytes only
   from the end of the input prefix through
   `min(caller_struct_size, library_struct_size)`. It preserves the two caller
   inputs and never writes the caller tail or past `struct_size`.
4. The library fills an output scalar or function pointer only when the whole
   field fits in both tables, as defined by
   `OPENDAL_MBT_API_V1_FIELD_END(field)`. A field cut by either size stays all
   zero; a partial function pointer is never written.
5. On success it reports its full table size, minor, patch, feature bits, and
   output-size ceiling.
6. Before reading or calling a table field, the caller verifies that both
   `api.struct_size` and `api.library_struct_size` reach that field's end. For
   a function, it also verifies the required feature bit and a non-NULL
   pointer.
7. A newer library may fill an older, shorter caller table. A newer caller
   talking to an older library sees its zeroed tail remain `NULL`.
8. The table memory belongs to the caller. Function pointers remain valid only
   while the native library stays loaded.

The Rust bootstrap treats `inout_api` as raw, size-bounded storage. It reads
the fixed input prefix and writes each fully covered field with checked raw
offsets (or an equivalent bounded byte copy). When `struct_size <
sizeof(opendal_mbt_api_v1_t)`, it must not cast the pointer to
`&mut opendal_mbt_api_v1_t` or otherwise create a Rust reference claiming the
unprovided tail. A full-table reference is allowed only after both alignment
and full-size coverage have been established.

`opendal_mbt_get_api` is itself a panic boundary. After a valid input prefix is
available, it stages the complete library table locally inside `catch_unwind`
and installs fields only after staging succeeds. The final bounded raw writes
must be non-panicking. A contained bootstrap panic returns `PANIC` with the
caller output payload still zero and no partially installed field. NULL,
misalignment, unsupported major, or an undersized declared prefix returns
`ABI_MISMATCH`; invalid non-NULL backing storage remains caller UB under the
general pointer contract.

Feature groups are exact: `BASE` covers bootstrap diagnostics, immutable
snapshot helpers, and Operator construction/destruction; `WHOLE_OBJECT` covers
check/exists/stat/read/write/create-dir/delete/copy/rename; `LISTING` covers
Lister; `RANDOM_READER` covers Reader; `CHUNKED_WRITER` covers the v1.0 Writer
open/write/finish/free contract; `READ_STREAM` and `WRITER_ABORT` cover the two
v1.1 lifecycle additions; `S3` covers typed construction; `PRESIGN` covers the
three request constructors and owned request inspection; `LAYERS` covers
timeout and retry; `CONCURRENCY_LIMIT` covers its separately feature-gated
layer; `BATCH_DELETE` and `COPIER` cover the v1.6 operations; and `ASYNC` covers
the v1.7 start, resource, result-take, cancellation, and free functions. A set
feature bit guarantees that every pointer in that group is non-NULL.

Every non-`BASE` group depends on `BASE`: a library must never advertise one
without also advertising `BASE`, and a caller requiring any operation group
must require and validate both bits. `WRITER_ABORT` additionally depends on
`CHUNKED_WRITER`. This guarantees construction plus the common buffer,
snapshot, error, and destruction functions needed to consume each group's
results. Feature availability remains separate from a configured backend's
capability; for example, a library can expose `PRESIGN` or `COPIER` while a
memory Operator reports the corresponding operation as unsupported.

The table ranges are normative: `BASE` is `library_info` through
`operator_free`; `WHOLE_OBJECT` is `operator_check` through `operator_rename`;
`LISTING` is `operator_lister` through `lister_free`; `RANDOM_READER` is
`operator_reader` through `reader_free`; `CHUNKED_WRITER` is
`operator_writer` through `writer_free`; `READ_STREAM` is
`operator_read_stream` through `read_stream_free`; `WRITER_ABORT` is the
appended `writer_abort` pointer; `S3` is `operator_s3`; `PRESIGN` is
`operator_presign_read` through `presigned_request_free`; `LAYERS` is
`operator_with_timeout` through `operator_with_retry`; `CONCURRENCY_LIMIT` is
`operator_with_concurrency_limit`; `BATCH_DELETE` is `operator_delete_many`;
`COPIER` is `operator_copier` through `copier_free`; and `ASYNC` is
`async_operator_read_start` through `async_task_free`.

### ABI v1.1 local-lifecycle extension

ABI v1.1 appends, in order:

1. a new `opendal_mbt_read_stream_options_v1_t` input with a complete
   `opendal_mbt_byte_range_v1_t`, `uint64_t chunk_size`, and the same optional
   version/condition views as read;
2. an opaque `opendal_mbt_read_stream_v1_t` handle;
3. `operator_read_stream`, `read_stream_next`, `read_stream_close`, and
   `read_stream_free`;
4. `writer_abort`.

`operator_read_stream` validates `chunk_size` as nonzero, representable as
Rust `usize`, no larger than `MAX_OUTPUT_BYTES`, and no larger than the
caller's negotiated output ceiling. The C stub passes the MoonBit `Int` only
after checking that it is positive and widening it to `uint64_t`. The Rust
reader uses concurrency one, prefetch zero, and no OpenDAL upstream chunk
setting. Leaving that setting absent prevents OpenDAL from resolving
`Full`/`From` through a preliminary `stat`. The bridge instead splits each raw
buffer locally, retains any remainder, and does not poll upstream again until
that remainder has been delivered.

`Full` and `From` are passed to the read request unchanged, so version and
conditions apply to the same request. `SUFFIX` is accepted only when the base
service reports native suffix-read capability. The binding clears
`OPENDAL_MBT_CAP_READ_SUFFIX` and returns `Unsupported` rather than using
OpenDAL's stat-based suffix simulation when that native capability is absent.

`read_stream_next` returns the existing transport statuses: `OK` with exactly
one non-NULL buffer, `END` with a NULL buffer and no error, or `ERROR` with a
NULL buffer and an owned error. `PANIC` also leaves both outputs NULL. Every OK
buffer is no larger than the stream's fixed chunk size and the per-call
`max_output_len`; a violation is treated as a terminal binding failure rather
than copied across the boundary.

The stream state is `Open(BufferIterator, pending Buffer?)`, `End`, `Failed`,
or `Closed`. `next` consumes a pending remainder before taking another
upstream iterator step, treats an upstream error as terminal, and returns
stable `END` after EOF. `read_stream_close` is a NULL-safe idempotent transition
to `Closed`; `read_stream_free` only drops state. Neither operation performs
MoonBit callbacks or commits remote effects.

The Writer implementation changes internally from OpenDAL's blocking Writer to
its async Writer so v1.1 can call `abort`. This does not reinterpret any v1.0
function pointer or behavior. The Writer state adds `Busy`, `Finished`, and
`Aborted` terminal distinctions. A call moves the inner Writer out while
holding the mutex, marks the handle Busy/terminal, releases the mutex, then
drives async work on the process runtime. This prevents a mutex guard from
being held across `Runtime::block_on`. A successful repeated abort from
`Aborted` returns `OK`; every other terminal operation returns a binding-owned
`ResourceClosed` error. Free drops the Writer without calling close or abort.

### ABI v1.2 typed-S3 extension

ABI v1.2 appends `operator_s3` under `OPENDAL_MBT_FEATURE_S3`. Its versioned
options record carries a required bucket and region, optional root and
endpoint, path-style/virtual-host selection, and exactly one typed
authentication policy: default chain, static/session credentials, unsigned,
or assume role with a typed credential source. All byte views are borrowed
only for the call and copied before return. Invalid combinations and
out-of-range values fail before an Operator is returned.

The constructor performs no object I/O. Credential material is never included
in immutable OperatorInfo, `library_info`, error diagnostics, or a public
debug representation. Only a profile that compiles and advertises `S3` may
install this function; passing the generic `"s3"` scheme to another profile
does not load the service dynamically.

### ABI v1.3 presign extension

ABI v1.3 appends the `PRESIGN` group: read, write, and stat constructors plus
owned request inspection/free functions. Expiry is explicit and positive.
Each successful call returns one immutable request handle owning its HTTP
method, URI, ordered header names, and arbitrary header bytes. Header order and
duplicate names are preserved. Request/header views borrow from that handle
until `presigned_request_free`; no presign call executes HTTP automatically.

Presigned URIs and headers can be bearer-equivalent credentials. The ABI does
not log them, attach them to errors, coerce header values to UTF-8, or provide a
`Debug`-style formatter.

### ABI v1.4 timeout/retry extension

ABI v1.4 appends `operator_with_timeout` and `operator_with_retry` under
`OPENDAL_MBT_FEATURE_LAYERS`. Each function borrows its input Operator and
returns a separately owned derived Operator plus matching OperatorInfo. The
input and every resource already opened from it remain unchanged.

Timeout and delay values must be positive, the minimum retry delay must not
exceed the maximum, and `max_retries` counts attempts after the initial one.
The supported composition order is timeout followed by retry. Duplicate
timeout/retry layers and adding timeout outside retry are rejected. Retry is
never installed implicitly and does not promise exactly-once Writer or append
semantics: an uncertain remote commit can be replayed.

### ABI v1.5 concurrency-limit extension

ABI v1.5 appends `operator_with_concurrency_limit` under the independent
`OPENDAL_MBT_FEATURE_CONCURRENCY_LIMIT` group. It borrows an Operator and
returns a separately owned derived Operator plus the matching immutable
OperatorInfo snapshot. The operation limit and present HTTP limit must be
positive and representable as Rust `usize`; the presence flag is exactly zero
or one, and an absent HTTP limit has a canonical zero value.

The supported layer order is timeout, retry, then concurrency limit. The
concurrency layer may be installed once and is always outermost; subsequent
timeout or retry calls are rejected. Operation permits on body-style Reader
streams, Writers, Listers, Deleters, and Copiers are held until the body is
dropped. An optional HTTP permit is held until the HTTP response body is
dropped. The local profile neither compiles nor advertises this group.

### ABI v1.6 batch/Copier extension

ABI v1.6 appends independent `BATCH_DELETE` and `COPIER` groups.
`operator_delete_many` copies and validates the complete path array before it
starts OpenDAL work. Empty input succeeds. `OK` means the high-level deleter
closed successfully; an error can follow deletion of an unspecified subset.
The ABI exposes no ordered per-path result and promises neither atomicity,
input-order preservation, nor uniqueness.

`operator_copier` creates one same-Operator, one-object Copier. `copier_next`
returns `OK` with one progress delta or stable `END`; `copier_finish` drives
remaining work and returns destination metadata; `copier_abort` is explicit
and idempotent after successful abort. Failures are terminal, and abort cannot
roll back effects already visible at the backend. `copier_free` only releases
state; it never reports finish or abort success.

### ABI v1.7 async extension

ABI v1.7 appends `OPENDAL_MBT_FEATURE_ASYNC` on the advertised macOS and Linux
targets. A start function synchronously validates and copies all borrowed inputs and requires
the writable end of a fresh, empty pipe dedicated to that task and already
configured with `O_NONBLOCK`. A blocking or non-pipe descriptor is rejected.
A start attempt may duplicate and configure the descriptor before a later
validation step fails. After every attempt the caller must only close its
original descriptor; it must not write through it, reuse it for another task,
or change its shared file-status flags. On success the duplicate lives until
the task is terminal. A Tokio worker publishes exactly one owned result before
attempting one readiness-byte write. `EAGAIN` is ignored as a defensive
nonblocking fallback, while `EPIPE` is contained without changing the foreign
process's process-wide SIGPIPE disposition. It never calls MoonBit, retains a
MoonBit value, or puts result data in the pipe.

Completion, result-taking, and cancellation are linearized by the task state.
Cancellation that wins publishes the terminal cancellation state, requests
worker cancellation, and owns the one wake signal; a later worker completion
cannot publish or signal again. A result is taken exactly once with the
matching typed take function. Taking too early, after cancellation, twice, or
through the wrong typed function returns an error.

Async read streams and Writers allow one in-flight operation. Cancelling an
unclaimed stream/Writer result, or cancelling an in-flight stateful operation,
makes that resource terminal because cursor or commit progress may be unknown.
Async stream chunks retain the fixed returned-output/copy bound and pending-
remainder behavior of the synchronous stream. `async_read_stream_close` is
synchronous, idempotent, and performs no I/O. Async resource/task frees only
release owned state; they never invent successful finish or abort outcomes.

Version meaning:

- major: breaking layout, signature, ownership, or numeric-code change;
- minor: append-only table functions, outer option fields, feature bits, or
  error codes;
- patch: implementation fixes with no contract change.

Within major v1, fields and function pointers are never reordered, removed,
retyped, or reinterpreted. New table functions are appended. New input-option
fields may only be appended to the end of the outer option struct; an old
prefix keeps its old defaults. Each option type's permanent
`*_OPTIONS_V1_MIN_SIZE` macro names the v1.0 prefix: implementations require
at least that size, read an appended field only when the whole field is
covered, and default every missing tail field. A caller sets a newly added
field or presence bit only after negotiating the library minor/feature that
introduced it; an older library may reject unknown bits. The by-value leaf
records
`opendal_mbt_bytes_view_v1_t`, `opendal_mbt_kv_v1_t`,
`opendal_mbt_byte_range_v1_t`, `opendal_mbt_timestamp_v1_t`, and
`opendal_mbt_capability_v1_t` are frozen for ABI major v1. In particular, the
embedded range inside read options can never grow and shift later fields.

Every output `*_view_v1_t` layout is also frozen for ABI major v1. A future
output field requires a newly named view type and a newly appended inspector
function; it is never appended to an existing v1 view.

`OPENDAL_MBT_STATUS_ABI_MISMATCH` is returned for an unsupported major, a table
or view that is too small, a wrong struct version, an unavailable required
field, or malformed ABI-only input. The MoonBit layer maps this to
`ErrorKind::AbiMismatch`.

## Representation rules

The ABI permits:

- `uint8_t`, `int32_t`, `uint32_t`, `int64_t`, and `uint64_t`;
- pointers to explicitly declared C types;
- opaque incomplete handle types;
- function pointers with `OPENDAL_MBT_CALL`;
- Rust `Option<unsafe extern "C" fn(...)>` only for nullable function-table
  slots;
- structs mirrored in Rust with `#[repr(C)]`.

It does not permit C or Rust `bool`, C or Rust enums, `char` as text, `long`,
`size_t`, Rust `usize`/`isize`, Rust references, slices, `String`, `Vec`, trait
objects, or any other Rust `Option`. The function-pointer exception uses
Rust's documented nullable-pointer representation: `None` is a C NULL slot and
`Some(function)` is the pointer. It must never be generalized to data pointers,
handles, or value fields.

Enums and flags are fixed-width typedefs plus named integer constants. Rust
must map every OpenDAL enum explicitly; casting a Rust discriminant is
forbidden. Extensible kind/status codes stay at or below `INT32_MAX` so they
map losslessly into MoonBit's `Int` unknown cases. Booleans are `uint32_t` and
are exactly zero or one.

Structs are passed by pointer rather than returned by value. There is no
packing, bitfield, flexible member, or serialized padding. Every extensible
input/view struct starts with `struct_size` and `struct_version`; v1 requires
`struct_version == 1`. Reserved fields must be zero. Unknown input tags, flag
bits, or presence bits are rejected instead of silently ignored.

For an output view, the caller zeroes at least `sizeof(the_v1_view)`, sets
`struct_size` to the actual writable byte count and `struct_version` to 1, and
passes that storage to the matching inspector. The library requires
`struct_size >= sizeof(the_v1_view)`, writes exactly the current v1 prefix, and
never touches a caller tail. After the header is valid, the library clears the
known payload while preserving `struct_size` and `struct_version`; on any
later failure the known payload remains zero. A too-small or wrong-version
view returns `ABI_MISMATCH` without modifying its payload or accessing bytes
beyond the caller-declared size.

Lengths and offsets are `uint64_t`. Before constructing a Rust slice or
converting to an OpenDAL `usize`, the bridge checks the value against both
`SIZE_MAX` and `isize::MAX`, validates pointer arithmetic, and performs any
narrower conversion with an explicit checked conversion. For the config array
it also checks `config_len * sizeof(opendal_mbt_kv_v1_t)` for multiplication
overflow and against `isize::MAX` before forming a slice.

The C and Rust sides will both carry compile-time size, alignment, and offset
assertions for every concrete struct. A generated header is not allowed to
drift silently from the checked-in header.

## Bytes and UTF-8

`opendal_mbt_bytes_view_v1_t` is borrowed for the duration of one call:

- `len == 0` allows `data == NULL`;
- `len > 0` requires a non-NULL readable region of at least `len` bytes;
- the Rust bridge never retains the pointer;
- empty Rust slices are constructed without calling
  `slice::from_raw_parts(NULL, 0)`;
- binary uses allow every byte, including NUL;
- textual uses are strict UTF-8 with no terminator requirement, and embedded
  NUL is transported faithfully.

Scheme, config keys/values, paths, option strings, metadata strings, entry
paths/names, diagnostics, and version strings are textual uses. Invalid UTF-8
in caller-controlled text produces a typed `InvalidArgument` error. Invalid
UTF-8 in a library-owned output view indicates an ABI defect and becomes
`AbiMismatch` in MoonBit.

The constructor receives a borrowed array of key/value views. A NULL array is
valid only when `config_len == 0`. Duplicate keys are rejected because the
MoonBit source is a map and accepting duplicates would create an ABI-only
ordering policy. Rust copies all configuration data before returning.

Unless this document names an exception, every pointer argument is required
to be non-NULL. The complete exceptions are: a NULL options pointer selects
defaults; `out_error` may be NULL; the config array may be NULL when
`config_len == 0`; a byte-view `data` may be NULL when `len == 0`;
`buffer_copy` may receive a NULL destination only for its zero-capacity sizing
query; every `*_free`, `lister_close`, `reader_close`, `read_stream_close`, and
`async_read_stream_close` treats NULL as a no-op; and
`async_task_cancel(NULL)` is a no-op. The byte-view struct pointer itself,
required handles, ranges, success-output pointers, and `buffer_copy`'s
`out_required` are not optional.

The caller additionally guarantees that every non-NULL pointer is correctly
aligned for its declared C type, remains live for the whole call, and refers
to readable or writable storage as required by the signature. Each byte or
array region must lie within one allocated object and its total byte extent
must be no greater than `isize::MAX`. The config array must contain
`config_len` fully initialized `opendal_mbt_kv_v1_t` elements. Output storage
must permit all documented writes. `opendal_mbt_get_api` additionally requires
`inout_api` to have `opendal_mbt_api_v1_t` alignment.

The bridge checks detectable contract errors—NULL rules, numeric limits,
integer overflow, known struct sizes/versions, and pointer alignment—before it
forms a Rust reference or slice; these return `ABI_MISMATCH`. C cannot prove
that an arbitrary non-NULL address is live or covers the claimed region, so a
dangling, forged, out-of-bounds pointer, a region spanning allocations, or any
otherwise invalid non-NULL data/output pointer is caller undefined behavior,
just like a forged or stale handle.

## Options

A NULL options pointer means the public default. Otherwise the caller supplies
a zero-initialized struct whose size covers that option's
`*_OPTIONS_V1_MIN_SIZE`, sets `struct_version` to 1, and sets only negotiated
presence/flag bits.

Presence bits distinguish `None` from `Some("")`. Boolean flags represent
true; an absent bit represents false. Every carrier whose presence bit is
clear must be canonical: a byte carrier is `{ data = NULL, len = 0 }` and a
numeric carrier is zero. In v1.0 the numeric case is `ListOptions.limit`, so
`LIMIT_PRESENT` clear with `limit != 0` is `ABI_MISMATCH`. A general
zero-length byte view may still have a non-NULL `data`; only an absent option
field requires the canonical form. When `LIMIT_PRESENT` is set, zero is a
transportable backend hint like any other value, subject to the checked
`usize` conversion. All option input is borrowed for the call, and Rust copies
anything retained by an OpenDAL reader, writer, or lister.

A non-NULL read-options value always contains a complete canonical range. An
omitted public range is encoded as a v1 `FULL` range with its size/version set,
reserved fields zero, and both numeric fields zero; an all-zero nested range
header is not valid.

Range tags are:

- `FULL`: `offset == 0`, `length == 0`;
- `FROM`: `offset` is the first byte, `length == 0`;
- `OFFSET_LENGTH`: `offset` and byte count; checked addition must not overflow;
- `SUFFIX`: `offset == 0`, `length` is the suffix byte count.

Non-canonical unused range fields are rejected. `ListOptions.limit` is checked
before conversion to `usize`; it remains a backend page/request hint rather
than a total-result guarantee.

## Transport status and output atomicity

Transport status is separate from `ErrorKind`:

| Status | Meaning |
|---|---|
| `OK` | All success outputs are complete |
| `END` | Lister reached EOF; valid only from `lister_next` |
| `ERROR` | Normal binding/OpenDAL failure; optional owned error returned |
| `BUFFER_TOO_SMALL` | Buffer-copy destination untouched; required length returned |
| `ABI_MISMATCH` | Caller/library contract mismatch; no normal error required |
| `PANIC` | A contained Rust panic; no panic payload crosses the ABI |

Every fallible operation initializes its scalar, handle, and snapshot success
out-parameters to NULL/zero before validation. A bulk byte destination such as
`buffer_copy.destination` is not an out-parameter to be pre-cleared. The
observable combinations are:

- `OK`: success outputs are populated, `*out_error == NULL`;
- `ERROR`: success outputs remain empty, and a non-NULL `out_error` receives
  exactly one owned immutable error;
- `END`: entry and error outputs are both NULL;
- `BUFFER_TOO_SMALL`: valid only from `buffer_copy`; `out_required` contains
  the exact buffer length while the destination remains untouched;
- all remaining statuses: success outputs remain empty.

`out_error` may be NULL when the caller deliberately discards detail. All
other output pointers are required unless explicitly covered by the
buffer-copy sizing rule, and output storage may not alias other inputs or
outputs. A function never returns partially initialized success data, uses
`errno`, or stores thread-local “last error” state.

## Ownership and views

| Object | Created by | Borrowed inspection | Destruction |
|---|---|---|---|
| API table | caller | function pointers | caller storage |
| LibraryInfo | library static data | `library_info` view | valid while loaded |
| Operator | `operator_new`, `operator_s3`, or a layer function | operation calls | `operator_free` |
| OperatorInfo | an Operator constructor or layer function | `operator_info_view` | `operator_info_free` |
| Buffer | read calls | `buffer_len`/`buffer_copy` | `buffer_free` |
| Metadata | stat/write/copy/finish calls | `metadata_view` | `metadata_free` |
| Entry | `lister_next` | entry + metadata views | `entry_free` |
| Error | failed operation | `error_view` | `error_free` |
| Lister | `operator_lister` | `lister_next` | `lister_free` |
| Reader | `operator_reader` | `reader_read` | `reader_free` |
| ReadStream | `operator_read_stream` | `read_stream_next` | `read_stream_free` |
| Writer | `operator_writer` | write/close calls | `writer_free` |
| PresignedRequest | a presign call | request + header views | `presigned_request_free` |
| Copier | `operator_copier` | next/finish/abort calls | `copier_free` |
| AsyncTask | an async start function | matching typed take | `async_task_free` |
| AsyncReadStream | async read-stream result take | async next/close calls | `async_read_stream_free` |
| AsyncWriter | async writer result take | async write/finish/abort calls | `async_writer_free` |

Every successful constructor transfers one owning pointer to the caller.
`*_free(NULL)`, `lister_close(NULL)`, `reader_close(NULL)`,
`read_stream_close(NULL)`, `async_read_stream_close(NULL)`, and
`async_task_cancel(NULL)` are no-ops. A non-NULL owning pointer is freed exactly
once and is then invalid. Copying an opaque pointer does not retain or clone
it. Destroying or freeing a handle while another call uses the same handle is
caller undefined behavior; the safe wrapper prevents it.

Except for the null-safe free/close functions above, an operation given a NULL
handle returns `ABI_MISMATCH`. A forged, wrong-type, stale, or already-freed
non-NULL pointer is caller undefined behavior; no C ABI can safely inspect
arbitrary addresses to recover from that misuse.

Child resources clone/own all native state they need and remain valid after the
originating Operator is freed. Borrowed output views are valid until their
owning immutable snapshot is freed. The C stub copies every view into a
MoonBit-owned value before calling the paired free.

Rust memory is freed only by Rust functions. C temporary memory is freed only
by C. MoonBit memory is freed only by the MoonBit runtime. In particular, C
must never call `free()` on Rust or MoonBit memory, and Rust must never call
`moonbit_incref`/`moonbit_decref`.

## Buffer copy protocol

Read calls materialize one immutable Rust-owned buffer. They do not expose a
`Vec` pointer, length, or capacity.

`buffer_copy` supports an atomic two-phase protocol:

1. `destination == NULL`, `capacity == 0` queries the exact required length.
   It returns `OK` for an empty buffer and `BUFFER_TOO_SMALL` for a non-empty
   buffer; both statuses write the exact length to `out_required`.
2. `destination == NULL` with nonzero capacity is `ABI_MISMATCH`; this check
   takes precedence over the capacity comparison.
3. For a non-NULL destination, if `capacity < required`, it returns
   `BUFFER_TOO_SMALL`, writes the required length, and copies no bytes.
4. With sufficient capacity it copies exactly the required bytes and returns
   `OK`, writes the same exact length to `out_required`, and adds no
   terminator.

The destination is atomic: every status other than `OK` leaves all destination
bytes unchanged; `OK` writes only `[0, required)` and leaves any capacity tail
unchanged. Validation and all work that can fail or panic complete before the
copy begins, and the final bounded copy path is non-panicking. `out_required`
is still an ordinary scalar output: it is cleared before validation and is set
only for `OK` or `BUFFER_TOO_SMALL` as defined above.

Sizing never repeats a storage operation; both calls inspect the same immutable
buffer. OpenDAL's potentially segmented `Buffer` is flattened once inside the
owned snapshot before it crosses the ABI.

The MoonBit C stub passes its maximum representable `Bytes` length as
`max_output_len` to read calls. The current native runtime uses `int32_t` array
lengths and `moonbit_make_bytes(int32_t, ...)`, so v1 advertises and enforces no
more than `INT32_MAX` output bytes. The stub checks the returned length again
before allocation. Whole reads stream through OpenDAL and reject an oversized
upstream buffer before extending the binding-owned result; read streams return
and copy at most the configured `chunk_size` per call.

Those limits are hard bounds on binding-owned native outputs and ABI/MoonBit
copies, not on all storage-runtime memory. OpenDAL 0.58 does not bound one raw
streaming `Buffer`; a cursor can temporarily retain one larger shared backing
buffer while returning locally split chunks. During handoff, the bounded
native snapshot and bounded MoonBit `Bytes` can also coexist.

The same representability rule applies to output strings, entry arrays, and
materialized lists. If an individual output or collection cannot be allocated
without narrowing or overflow, the wrapper releases all native snapshots and
raises `BufferTooLarge`. A conversion error while streaming a lister exhausts
that lister after reporting the error once.

## Error contract

The Rust error snapshot contains only:

- stable binding-owned kind code;
- permanent/temporary/persistent status code;
- stable kind name for forward-compatible diagnostics;
- sanitized human-readable message.

Numeric codes are never Rust discriminants. A MoonBit wrapper that does not
recognize a newer kind constructs `UnknownKind(code, name)`. It likewise maps
an unfamiliar status to `UnknownStatus(code)`.

The safe MoonBit call site attaches `Operation`, `path`, and
`destination_path`. This keeps Rust/OpenDAL errors independent of the MoonBit
ADT and accurately represents both sides of copy/rename. Private MoonBit
Reader/Lister/Writer wrappers retain their original path for later errors.

Messages use OpenDAL's message text, not `Debug`, its source chain, or a
backtrace. The bridge never appends or logs the config map. During operator
construction, values of keys whose names indicate passwords, secrets, tokens,
credentials, or private/access keys, plus URI user-info, are redacted before a
message is returned. Tests use sentinel secrets and require that no error text
contains them.

Binding-local validation errors use `Permanent` status. `PANIC` maps in the C
stub to a fixed `Unexpected` message without the panic payload or backtrace.
OOM abort, `panic=abort`, signals, undefined behavior, and foreign C++
exceptions cannot be converted into normal errors.

## Public API mapping

| MoonBit surface | ABI operation |
|---|---|
| `Operator::new` | `operator_new`, returning Operator + cached OperatorInfo |
| `Operator::s3` | `operator_s3`, returning Operator + cached OperatorInfo |
| `Operator::info` | pure cached MoonBit snapshot; no later native call |
| `check` | `operator_check` |
| `exists` | `operator_exists` |
| `stat` | `operator_stat` + metadata view/free |
| `read` | `operator_read` + buffer length/copy/free |
| `write` | `operator_write` + metadata view/free |
| `create_dir` | `operator_create_dir` |
| `delete` | `operator_delete` |
| `copy` | `operator_copy` + metadata view/free |
| `rename` | `operator_rename` |
| `open_lister` | `operator_lister` |
| `list` | materialize `operator_lister` + repeated `lister_next` |
| `Lister::next` | `lister_next` + entry views/free |
| `Lister::close` | `lister_close` |
| `open_reader` | `operator_reader` |
| `Reader::read` | `reader_read` + buffer length/copy/free |
| `Reader::close` | `reader_close` |
| `open_read_stream` | `operator_read_stream` |
| `ReadStream::next` | `read_stream_next` + buffer length/copy/free |
| `ReadStream::close` | `read_stream_close` |
| `open_writer` | `operator_writer` |
| `Writer::write` | `writer_write` |
| `Writer::finish` | `writer_close` + metadata view/free |
| `Writer::abort` | `writer_abort` |
| `presign_read`/`presign_write`/`presign_stat` | matching presign call + request/header views/free |
| `with_timeout` | `operator_with_timeout`, returning Operator + OperatorInfo |
| `with_retry` | `operator_with_retry`, returning Operator + OperatorInfo |
| `with_concurrency_limit` | `operator_with_concurrency_limit`, returning Operator + OperatorInfo |
| `delete_many` | `operator_delete_many` |
| `open_copier` | `operator_copier` |
| `Copier::next` | `copier_next` |
| `Copier::finish` | `copier_finish` + metadata view/free |
| `Copier::abort` | `copier_abort` |
| `Operator::as_async` | pure MoonBit wrapper retaining the Operator |
| `AsyncOperator::read` | `async_operator_read_start` + pipe readiness + buffer task take/free |
| `AsyncOperator::open_read_stream` | `async_operator_read_stream_start` + read-stream task take/free |
| `AsyncReadStream::next` | `async_read_stream_next_start` + buffer task take/free |
| `AsyncReadStream::close` | `async_read_stream_close` |
| `AsyncOperator::open_writer` | `async_operator_writer_start` + writer task take/free |
| `AsyncWriter::write`/`abort` | matching async start + unit task take/free |
| `AsyncWriter::finish` | `async_writer_finish_start` + metadata task take/free |

Materializing `list` over the native lister is not an emulation of a storage
operation; it is the eager consumption form of the same listing primitive.
Optional-argument normalization, `Metadata::is_file`/`is_dir`, and
Capability getters are pure MoonBit operations. The C ABI retains OpenDAL
0.58.1's effective composed capability snapshot except for suffix reads: it
clears that bit unless the base service supports suffix ranges natively. The
public MoonBit surface exposes getters only for operations and options
currently callable through the facade, so a true public capability always has
a corresponding MoonBit operation.

## Resource states

The Rust bridge owns state and synchronization; a C stub does not infer state
from a NULL raw pointer.

```text
Lister: Open --EOF/error--> Exhausted --close--> Closed
             \--------------------close-------> Closed

Reader: Open ---------------------close-------> Closed

ReadStream: Open --EOF-------------------------> End
            Open --error/panic----------------> Failed
            Open/End/Failed --close-----------> Closed

Writer: Open --write success------------------> Open
        Open --finish attempt-----------------> Finished or Failed
        Open --abort attempt------------------> Aborted or Failed

Copier: Open --next progress------------------> Open
        Open --EOF----------------------------> End
        Open/End --finish attempt-------------> Finished or Failed
        Open --abort attempt------------------> Aborted or Failed

AsyncTask: Running --worker-------------------> Completed --take--> Taken
           Running/Completed --cancel---------> Cancelled

AsyncReadStream: Open --next------------------> Busy --> Open/End/Failed
                 Open/Busy/End/Failed --close-> Closed

AsyncWriter: Open --write---------------------> Busy --> Open/Failed
             Open --finish--------------------> Busy --> Finished/Failed
             Open --abort---------------------> Busy --> Aborted/Failed
```

- Lister EOF returns `END`. An ordinary error is returned once, then the
  lister is Exhausted and later `next` returns `END`. Explicit close is
  idempotent; `next` after close returns `ResourceClosed`.
- Reader reads do not move a cursor. Ordinary read errors do not close it.
  Close is idempotent; later reads return `ResourceClosed`. A contained panic
  closes it because internal state is uncertain.
- ReadStream EOF returns stable `END`. An error or panic is terminal and later
  `next` calls return `ResourceClosed`. Close is idempotent and performs no
  I/O.
- Writer calls are serialized. Any `write` error is terminal. The first
  finish/close or abort attempt is terminal whether it succeeds or fails.
  Repeating a successful abort returns `OK`; every other later operation
  returns `ResourceClosed`.
- Copier calls are serialized. `END` from `next` is stable until `finish`
  consumes the retained Copier. Finish drives any remaining work. Failure is
  terminal; repeating a successful abort returns `OK`.
- Async tasks publish at most one completion and allow exactly one matching
  result take. Cancellation before a take is terminal. Cancelling an in-flight
  stateful operation marks its AsyncReadStream or AsyncWriter failed.
- AsyncReadStream and AsyncWriter reject a second in-flight operation. Stream
  EOF is stable; explicit stream close is idempotent. Async Writer finish and
  abort have the same terminal/repeated-abort rules as the synchronous Writer.
- Freeing/finalizing any open Writer or Copier only drops it. It never calls
  finish or abort and reports neither outcome. A successful abort does not
  promise rollback of effects already visible; failure, cancellation, or a
  drop can leave partial data or orphan multipart state.

The close functions keep their outer ABI handles alive so later calls can
deterministically report state; the paired free still occurs once. The same is
true of terminal Writer, Copier, and async resource handles.

## Thread contract

- Bootstrap, distinct handles, and immutable output snapshots are safe to use
  concurrently.
- Operator operations on one handle are concurrent.
- Reader range reads on one handle are concurrent. Close linearizes through an
  internal state lock: admitted reads finish before close, later reads fail as
  closed.
- Lister, ReadStream, Writer, and Copier calls on one handle are internally
  serialized. Racing synchronous calls are memory-safe, but their acquisition
  order is unspecified.
- Async task completion/cancel/take transitions are serialized. AsyncReadStream
  and AsyncWriter permit one operation at a time and reject overlap without
  running a second upstream operation.
- No handle has thread affinity. A finalizer may run on a different thread.
- Free must not race any call on the same handle.

The Rust implementation must carry compile-time `Send + Sync` assertions for
every type whose ABI contract promises them. Poisoned synchronization state is
converted into a binding error/state transition rather than causing another
panic.

## Rust implementation baseline

The bridge pins OpenDAL exactly and commits `Cargo.lock`:

```toml
[package]
edition = "2024"
rust-version = "1.91"

[lib]
crate-type = ["staticlib"]

[features]
profile-local = ["opendal/blocking", "opendal/services-fs"]
profile-standard = [
  "profile-local",
  "layers-concurrent-limit",
  "layers-timeout-retry",
  "opendal/services-s3",
  "opendal/http-transport-reqwest",
  "opendal/http-transport-reqwest-rustls",
  "opendal/executors-tokio",
]
layers-concurrent-limit = ["opendal/layers-concurrent-limit"]
layers-timeout-retry = ["opendal/layers-retry", "opendal/layers-timeout"]

[dependencies]
opendal = { version = "=0.58.1", default-features = false }
tokio = { version = "=1.52.0", features = ["rt-multi-thread", "fs"] }

[profile.release]
panic = "unwind"
```

Memory is always enabled in OpenDAL 0.58.1; the deprecated
`services-memory` feature is unnecessary. `profile-local` adds explicit
filesystem support. `profile-standard` adds S3, its HTTP/TLS/runtime features,
and only the layer implementations that its public methods can install.
Disabling facade defaults ensures that no retry, timeout, logging, or unrelated
layer is pulled in or applied accidentally.

Static libraries cannot safely depend on constructor registration being run.
The bridge explicitly invokes `opendal::install_default()` through an
idempotent runtime initialization path; with the standard profile this installs
the compiled S3 HTTP transport. It initializes its Tokio runtime with a
fallible `OnceLock` result and never uses `unwrap`/`expect` at the ABI boundary.
Blocking Operator construction occurs inside the runtime's entered context.

Every C-callable entry point—the direct bootstrap and every function pointer
installed into the table—is an `extern "C"` panic boundary. Installed
functions clear outputs, validate ABI inputs, and run implementation work
inside `catch_unwind(AssertUnwindSafe(...))`; bootstrap uses the staged-table
rule above. No unwind crosses C. Stateful panic transitions are Lister ->
Exhausted, Reader -> Closed, and ReadStream/Writer/Copier/async resources ->
their documented terminal failure state. Destructors also contain panics; they
never call finish/abort or report completion.

The final linker requirements of a Rust `staticlib` are recorded from
`rustc --print native-static-libs` rather than hard-coded from one machine.
Every target-native artifact manifest and pinned table carries those exact
flags; a release probe links the archive and verifies its runtime identity.

## MoonBit C stub contract

The package-local C stub is intentionally mechanical:

- it includes both `moonbit.h` and this header;
- it negotiates and validates the API table once before the first operation;
- it converts MoonBit UTF-16 text to owned UTF-8 Bytes before entering C;
- it uses `Moonbit_array_length` only inside the stub and widens lengths before
  constructing ABI views;
- it marks every non-primitive private extern parameter `#borrow` unless an
  explicit ownership transfer is required;
- it never stores a borrowed MoonBit pointer in Rust;
- it represents each native handle with a MoonBit external object whose
  finalizer calls exactly one Rust `*_free` and never frees the external-object
  container;
- it copies Rust snapshots/buffers into MoonBit-owned values before freeing
  them;
- it converts transport errors into the typed MoonBit error model, attaching
  operation and path context at the safe wrapper call site.

Private MoonBit wrappers retain cached OperatorInfo and the original path(s)
for Reader, ReadStream, Lister, Writer, Copier, AsyncReadStream, and
AsyncWriter. They can be private structs while their public types stay opaque.

## Validation gates

The implemented header has warning-clean C11 and C++17 syntax smoke tests.
ABI v1.7 remains complete only while all of the following gates pass:

1. Real C consumers negotiate the table, exercise memory lifecycle operations,
   and cover standard-profile typed S3 construction and optional groups.
2. C and Rust layout assertions match on every supported target.
3. Negotiation tests cover shorter old callers, sizes cut inside every scalar
   and function-pointer field, longer new callers with tail canaries,
   unsupported majors, absent features, non-BASE feature dependency
   invariants, and NULL optional pointers. No test ever observes a partially
   written field.
   Unsafe-code review and Miri additionally verify that short callers never
   produce a full-table Rust reference. Panic injection covers bootstrap
   staging and proves that no partial field is installed.
4. Input tests cover NULL+zero, NULL+nonzero, empty/binary data, embedded NUL,
   Unicode, invalid UTF-8, duplicate config keys, unknown flags/tags, overflow,
   values larger than `SIZE_MAX`/`isize::MAX`/MoonBit limits, every v1.0 option
   minimum, and truncated future option fields defaulted without being read.
5. Output tests prove atomic clearing; lister, read-stream, and Copier
   entry/chunk/progress/end/error states; async typed-result matching; and
   two-phase buffer-copy destination canaries for every non-OK status and the
   untouched tail after OK.
6. Ownership tests cover `free(NULL)`, NULL close/cancel no-ops, every error
   branch, early Operator free while children remain live, explicit and
   repeated close/abort, open Writer/Copier finalization without finish, async
   result cancellation/take races, and snapshot lifetime independence.
   Writer/Copier tests allow backend effects to be indeterminate unless their
   explicit terminal operation succeeds.
7. Panic injection exercises every entry point and verifies that the process
   survives, output is cleared, no payload leaks, no foreign-runtime callback
   occurs, and state transitions are terminal where required.
8. ASan/LSan/UBSan cover the complete Rust-C-MoonBit slice; Miri covers unsafe
   Rust input helpers; TSAN stresses same-handle concurrency and close races.
9. Debug and release run on macOS and Linux for the promised architectures.
   Windows and 32-bit claims require their own ABI/link/conversion lanes.
10. Any shared/prebuilt ABI artifact exports only
    `opendal_mbt_get_api`; static-archive internals are not treated as the
    public symbol surface.

## Primary implementation references

- [OpenDAL 0.58.1 facade features and Rust baseline](https://github.com/apache/opendal/blob/v0.58.1/core/Cargo.toml)
- [OpenDAL blocking Operator](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/blocking/operator.rs)
- [OpenDAL random-access blocking Reader](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/blocking/read/reader.rs)
- [OpenDAL blocking Writer and Drop behavior](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/blocking/write/writer.rs)
- [OpenDAL Capability fields](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/types/capability.rs)
- [OpenDAL effective OperatorInfo capability](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/types/operator/info.rs)
- [OpenDAL BytesRange](https://opendal.apache.org/docs/rust/opendal/enum.BytesRange.html)
- [OpenDAL explicit default installation](https://github.com/apache/opendal/blob/v0.58.1/core/core/src/lib.rs)
- [OpenDAL experimental C header](https://github.com/apache/opendal/blob/v0.58.1/bindings/c/include/opendal.h)
- [Rust `catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)
- [Rust nullable pointer optimization for FFI](https://doc.rust-lang.org/nomicon/ffi.html#the-nullable-pointer-optimization)
- [Rust static-library linkage](https://doc.rust-lang.org/reference/linkage.html)
