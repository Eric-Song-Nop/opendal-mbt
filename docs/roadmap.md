# OpenDAL MoonBit Binding Roadmap

> **Before starting implementation:** Use the Skill tool to load the
> `moonbit-c-binding` skill, which provides comprehensive guidance on FFI
> declarations, ownership annotations, C stubs, and AddressSanitizer
> validation.

## Direction

Design the user-facing semantics top-down, implement each feature bottom-up,
and deliver the binding as end-to-end vertical slices:

```text
MoonBit public contract and acceptance examples
  -> versioned Rust C ABI and ownership contract
  -> C ABI tests
  -> MoonBit runtime stub and private FFI
  -> safe MoonBit implementation
  -> native debug/release tests and sanitizers
```

The intended architecture is:

```text
safe MoonBit API
  -> private native FFI
  -> thin MoonBit runtime C stub
  -> project-owned, versioned Rust C ABI
  -> pinned OpenDAL Rust crate
```

The upstream experimental C binding may be used as a behavioral reference or
test oracle, but is not the public or long-term ABI of this project.

## Baseline decisions

- Start with a synchronous, native-only binding.
- Pin the OpenDAL crate exactly and commit `Cargo.lock`.
- Start with the `memory` and `fs` services.
- Keep all raw FFI declarations private.
- Represent native resources as opaque MoonBit types backed by external
  objects with finalizers.
- Use typed MoonBit errors for operational failures; reserve `Option` for
  absence and end-of-stream.
- Use optional labelled parameters for operation options. Native code copies
  any data it retains.
- Grow the binding through optional, append-only ABI groups. Keep callbacks,
  broad cloud-service coverage, and observability exporters out of the public
  surface until their ownership and redaction contracts are designed.

## Status at a glance

| Area | Status | Available in |
| --- | --- | --- |
| Published compatibility baseline | Released | Moon package `0.1.0`, ABI `1.0`, `local` profile |
| Blocking local operations | Released | `memory` and `fs` in `0.1.0` |
| Bounded reads and explicit Writer abort | Pinned in `v0.2.0` candidate; publication pending | Phase 5A, ABI `1.1` |
| Typed S3 and the `standard` profile | Pinned in `v0.2.0` candidate; publication pending | Phase 5B, ABI `1.2` |
| Presign and explicit operational layers | Pinned in `v0.2.0` candidate; publication pending | Phase 5C, ABI `1.3`-`1.5` |
| Batch delete and managed Copier | Pinned in `v0.2.0` candidate; publication pending | Phase 5D, ABI `1.6` |
| Initial MoonBit async facade | Pinned in `v0.2.0` candidate; publication pending | Phase 5E, ABI `1.7` |
| Standard native host expansion | Pinned in `v0.2.0` candidate; publication pending | macOS arm64, Linux x86-64, Linux arm64 |

`v0.1.0` remains the published compatibility baseline. This tree is the fully
pinned `v0.2.0` standard release candidate across three target-native hosts.
Phase 5 is not released until the tag workflow reproduces the committed
digests, uploads those assets, publishes the same source to mooncakes.io, and
passes fresh registry consumers. Until then, `moon add
Eric-Song-Nop/opendal@0.1.0` still provides only the released local surface and
native archive.

The source implementation preserves the old local behavior while adding
public methods, optional ABI groups, the `standard` service profile, and one
new host. It does not silently install retry, timeout, or concurrency policy.

Roadmap status words have the following meaning:

- **planned**: scope and dependency order are known, but the public contract
  is not frozen;
- **designed**: public semantics, ABI ownership, state machines, and acceptance
  examples have been reviewed;
- **implemented**: all vertical-slice code and local validation are complete;
- **pinned candidate**: exact target-native artifact identities and digests are
  committed and clean packaged consumers have passed, but tag publication and
  fresh registry acceptance have not occurred;
- **released**: target artifacts, the Moon package, documentation, and a clean
  registry consumer have been verified from the same tag.

## Delivery order

### Phase 0: Public semantics (complete)

Define the stable MoonBit-facing contract before committing to an ABI:

- core public types and naming;
- error taxonomy and contextual information;
- path and configuration semantics;
- metadata, capability, and optional-field semantics;
- resource lifecycle rules for operator, lister, reader, and writer;
- signatures and behavior for the first synchronous operations;
- explicit non-goals for the first release.

Deliverables:

- `docs/design/public-api-semantics.md`;
- a compiler-checked MoonBit API proposal;
- type-checked usage tests kept outside the published package surface.

### Phase 1: ABI contract and memory-service spike

Status: complete for the first memory-service vertical slice.

The implemented safe MoonBit path is `Operator::new`/`info` plus default
whole-object `write` and `read`. Read/write options are intentionally absent
from the published MoonBit API until their complete Phase 3 slices land. The
Rust bridge fills the complete BASE and WHOLE_OBJECT ABI groups so the
advertised feature bit still guarantees every function pointer in its group.

Freeze a small, versioned C ABI and implement one complete slice:

- ABI/version query;
- memory operator construction;
- whole-object write and read;
- error copying and freeing;
- handle and buffer destruction.

Before implementation, document every function's pointer validity, ownership,
allocation/free pair, nullability, error behavior, panic boundary, and thread
contract.

ABI design deliverables:

- `docs/design/c-abi.md`;
- `native/include/opendal_mbt.h`;
- warning-clean C11 and C++17 header smoke tests.

Exit criteria:

- Rust ABI tests, C smoke tests, and MoonBit tests pass;
- native debug and release builds pass;
- AddressSanitizer/LeakSanitizer find no binding-owned leaks or invalid access.

All three criteria are satisfied for the memory slice. The macOS sanitizer
run uses a symbol-scoped suppression for dyld's per-worker TLS termination
records; project-owned Rust handles, C temporaries, and MoonBit external
objects remain unsuppressed.

### Phase 2: Blocking core MVP

Status: complete.

The phase was delivered as vertical slices in this order:

1. `stat`, `exists`, `delete`, and `create_dir`;
2. `Lister` and `Entry`;
3. filesystem service end-to-end tests;
4. basic capability inspection.

Both eager and streaming listing forms now share explicit native ownership and
terminal-state rules. Tests cover memory and filesystem services, recursive and
shallow listing, labelled list arguments, repeated end-of-stream, repeated close, native
errors, panics, and poisoned synchronization state.

### Phase 2.5: MoonBit facade stabilization

Status: complete.

- publish implemented operations only;
- use optional labelled arguments instead of public options records;
- accept `StringView` and `BytesView` inputs while returning owned values;
- name managed resource constructors `open_*`;
- expose structured error accessors and checked examples;
- verify the public contract from a real downstream consumer.

The generated interface now contains only implemented operations, all native
externs are isolated behind the safe facade, and an independent workspace
module executes the published API in debug and release builds. README examples
are compiler-checked and the publish manifest excludes development-only
workspace fixtures. A separate debug smoke test unpacks the source package,
builds its native archive in isolation, and runs the downstream consumer.

### Phase 3: Streaming and operation options

Status: complete.

Delivered:

- random-access `Reader` range reads without a hidden cursor;
- `Writer` with explicit `open/closed/failed` state;
- explicit, terminal `finish` semantics;
- incremental read and write options as optional labelled arguments;
- copy and rename without semantic emulation.

### Phase 4: Distribution and service profiles

Status: complete. `v0.1.0` is published as both a GitHub Release and the
`Eric-Song-Nop/opendal` module on mooncakes.io.

The distribution contract is now fixed: `moon add`, a normal package import,
and the native target must be sufficient for consumers. Rust, a source
checkout, `LIBRARY_PATH`, and consumer-owned linker flags are not part of the
installation contract.

The implementation order is:

1. build reproducible `local` profile artifacts for Apple silicon macOS and
   x86-64 glibc Linux;
2. install the selected artifact through Moon's prebuild configuration hook,
   with pinned digests and a content-addressed shared cache;
3. prove cold-cache, hot-cache/offline, corruption, and concurrent-install
   behavior;
4. run a packaged downstream consumer without Cargo, Rust, manual linker
   settings, or repository files;
5. publish the artifacts and package from the same version tag.

The `local` profile contains the memory and filesystem services. The initial
compatibility floors are macOS 11.0 and glibc 2.35. The minimum maintainer
toolchains are MoonBit `0.10.6+80dc50f24` and Rust `1.91.0`; the temporary
prebuild configuration mechanism requires Node.js 18 or newer at consumer
build time. See `docs/design/native-distribution.md` for the artifact, trust,
cache, and release contracts.

Delivered:

- deterministic target-native archives whose manifests record the exact ABI,
  dependency, compatibility, integrity, and system-link contracts;
- Apple silicon macOS and x86-64 glibc Linux release builders;
- a host-selecting Moon configuration script with pinned SHA-256 values,
  atomic installation, concurrent locking, corruption recovery, and offline
  hot-cache behavior;
- exact local-archive overrides for maintainer tests without consumer-facing
  linker configuration;
- a published package surface without Cargo manifests, Rust sources, or
  maintainer tooling;
- clean downstream debug and release gates without Cargo, Rust homes,
  `LIBRARY_PATH`, manual link flags, or repository source;
- one tag pipeline that publishes verified GitHub assets, publishes the same
  version to mooncakes.io, and executes the registry package afterward.

The `v0.1.0` tag workflow rebuilt and verified both native artifacts, published
the GitHub Release and Moon package, and passed a clean registry-consumer test.
An independent macOS consumer also passed native debug and release tests after
installing `Eric-Song-Nop/opendal@0.1.0` from the registry.

The initial platform target is macOS and Linux. Windows support requires a
separate linker and artifact-distribution decision.

### Phase 5: Standard profile and complete first-generation facade

Status: pinned `v0.2.0` release candidate through Phase 5E; tag publication and
fresh registry acceptance remain.

The user guides were checked against OpenDAL's binding overview, connecting,
getting-started, and common-task documentation. Phase 5 was delivered as
ordered vertical slices so each public contract remained reviewable even
though the final source stack now contains all five slices:

```text
5A local lifecycle completion
  -> 5B first cloud profile
       -> 5C presign and explicit operational layers
            -> 5D batch and copier tasks
                 -> 5E public async API

Linux arm64 artifacts and S3 integration run in parallel
```

This order remains useful for history, review, and bisecting:

- large-object reads and reliable writer cleanup must work before remote
  services make memory use and unfinished multipart uploads expensive;
- presigning needs a service that can exercise signed HTTP requests;
- retry and timeout policies need stateful stream/write rules first;
- copier cancellation and public async operations need a reviewed cross-runtime
  task model rather than ad-hoc callbacks;
- enabling a scheme in documentation never makes it available in an already
  compiled static library.

#### Phase 5A: Complete the local blocking lifecycle

Status: included in the pinned but unpublished `v0.2.0` standard candidate.

Delivered in the current tree:

- `ReadStream` and `Operator::open_read_stream`, with owned chunks, a hard
  per-call returned-output/copy bound, stable EOF, idempotent close, ranges,
  versions, and read conditions;
- output-bounded whole-object and ranged reads that stream through OpenDAL and
  reject oversized output before extending the binding result or copying it
  into MoonBit;
- `Writer::abort` with an explicit terminal state and idempotent successful
  abort;
- append-only ABI v1.1 groups, C consumers, MoonBit runtime tests, and the
  original cursorless/concurrent random-access `Reader` lifecycle; suffix
  reads are capability-gated to native backend support.

##### Goals

- read arbitrarily large objects without aggregating the whole object into one
  binding-owned Rust output or one MoonBit `Bytes` value;
- make the per-call output limit a hard binding-owned allocation/copy bound;
- provide an explicit and reliable Writer abort path;
- use OpenDAL's async reader/writer internally where necessary while preserving
  the current synchronous MoonBit facade;
- preserve released handle/lifecycle behavior while refusing stat-simulated
  suffix ranges that cannot meet the same-request read contract.

##### 5A.1 Public-semantics design

The sequential API must use a new public resource type. The existing `Reader`
remains a reusable, concurrent, random-access reader whose calls name an
independent `ByteRange`; it must not acquire a hidden cursor.

A working API shape, to be finalized by compiler-checked acceptance examples,
is:

```moonbit nocheck
let stream = operator.open_read_stream(
  "large.bin",
  range=Full,
  chunk_size=1024 * 1024,
)
for ;; {
  match stream.next() {
    Some(chunk) => consume(chunk)
    None => break
  }
}
stream.close()

let writer = operator.open_writer("output.bin")
writer.write(first_chunk)
writer.abort()
```

The design freezes these read-stream rules before ABI work starts:

- the names `ReadStream` and `Operator::open_read_stream`;
- `chunk_size : Int`, fixed at open with a 1 MiB default;
- valid minimum and maximum chunk sizes and their relationship to
  `max_output_bytes`, MoonBit array limits, `usize`, and `isize`;
- preservation of `Full`, `From`, and `Range`, plus native-backend `Suffix`
  when `can_read_suffix()` is true, and preservation of `version`, `if_match`,
  and `if_none_match` on the same read request;
- error-context mapping to the released `Operation` enum; adding enum variants
  can break exhaustive downstream matches, so reuse `Read`/`Write` unless a
  deliberate public compatibility change is reviewed;
- `Open -> End`, `Open -> Closed`, and `Open -> Failed` transitions;
- stable repeated end-of-stream and idempotent explicit close;
- terminal treatment of stream errors when the upstream cursor cannot be
  resumed safely;
- same-handle serialization and whether cross-handle calls may proceed
  concurrently;
- the guarantee that the resource outlives the `Operator` from which it was
  opened;
- no prefetch or concurrency default that can violate the configured returned-
  output/copy bound.

The Writer design must add `abort` without weakening the existing contract:

| Initial state | Operation | Resulting state |
| --- | --- | --- |
| Open | successful `write` | Open |
| Open | failed `write` or panic | Failed |
| Open | first `finish` attempt | Finished or Failed; always terminal |
| Open | first `abort` attempt | Aborted or Failed; always terminal |
| Finished, Aborted, or Failed | `write`/`finish` | `ResourceClosed` |

The design must also decide and test whether a repeated successful `abort` is
idempotent or returns `ResourceClosed`. Native free and MoonBit finalization
must never commit data. They must not start an unobservable background finish
or silently report abort success; callers use explicit `finish` or `abort`
when completion or cleanup matters.

##### 5A.2 ABI and Rust implementation

Extend ABI major v1 append-only. Do not repurpose existing function pointers,
feature bits, opaque handles, option flags, or output layouts.

Expected ABI work:

- add an independent sequential-reader feature group and opaque handle;
- append open/next/close/free functions with explicit ownership and terminal
  state contracts;
- add an independent Writer-abort feature group rather than redefining the
  released CHUNKED_WRITER guarantee;
- bump the ABI minor version when the v1 table grows;
- keep every feature group all-or-nothing and dependent on BASE;
- return one Rust-owned output-bounded chunk per `next` call using the existing
  two-phase buffer-copy contract;
- reuse the fallible process-wide Tokio runtime, but do not hold a Rust mutex
  across arbitrary MoonBit work;
- contain every panic at the C boundary and transition stateful handles to the
  documented terminal state;
- keep finalizers free of blocking network I/O and unable to surface false
  completion.

The bounded read path must consume at most one OpenDAL buffer at a time. It
must not collect an OpenDAL stream, flatten multiple buffers, or perform a
whole-object blocking read before checking the limit. `Operator::read` and
Reader open-ended reads now follow that strategy, so `max_output_len` is a
hard bound on binding-owned output and copy allocations rather than only a
post-read rejection.

OpenDAL 0.58 does not expose a maximum size for one raw streaming `Buffer`.
ReadStream therefore leaves upstream chunking unset to avoid OpenDAL's hidden
stat for `Full`/`From`, locally splits one raw buffer, and retains its shared
remainder before polling again. The configured chunk size does not claim a
hard bound on total backend/native memory. For the same no-stat reason, suffix
ranges are exposed only when the base service supports them natively; the
filesystem backend no longer inherits OpenDAL's stat-based suffix simulation.

##### 5A.3 Delivery slices

Land Phase 5A as reviewable vertical slices:

1. public semantics, state diagrams, and compiler-checked API examples;
2. sequential-reader C ABI, Rust implementation, ABI tests, and C consumer;
3. private MoonBit FFI, C stub, safe facade, and checked documentation;
4. async Writer internals plus Writer-abort ABI and native tests;
5. MoonBit `Writer::abort`, lifecycle tests, and downstream consumer updates;
6. distribution-manifest, release-note, and registry-consumer verification.

##### 5A.4 Required tests

- empty, one-chunk, exact-boundary, multi-chunk, and final-short-chunk reads;
- binary data, embedded NUL, Unicode paths, every `ByteRange`, and conditional
  read options;
- logical objects larger than a configured test output cap, exercised with a
  small injectable cap or synthetic chunk source instead of allocating
  multi-gigabyte CI fixtures;
- platform-limit length/overflow paths tested without requiring a real object
  larger than the machine can allocate;
- invalid zero/overflowing/over-limit chunk sizes;
- repeated EOF, repeated close, early close, read-after-close, and finalization;
- same-handle serialization, concurrent independent handles, close/read races,
  panic containment, and poisoned-state behavior;
- abort before write, after one or many writes, after write failure, concurrent
  abort/write, abort/finish races, repeated abort, and GC without finish;
- Rust unit/ABI tests, C11 consumer tests, MoonBit black-box tests, debug and
  release profiles, coverage, ASan/LSan, and clean packaged consumers.

##### 5A exit criteria

- [x] The public API and generated `.mbti` match the reviewed acceptance
  examples.
- [x] Tests demonstrate bounded incremental reads and enforce the native
  output ceiling without allocating a multi-gigabyte fixture.
- [x] Existing `v0.1.0` APIs retain their behavior, and ABI v1.0 callers still
  negotiate the prefix they understand.
- [x] Rust, C, MoonBit, debug/release, lifecycle, and sanitizer gates cover the
  source slice.
- [x] Source/native tests cover the Phase 5A semantics, while every pinned
  candidate passes release-build, identity, and clean packaged-consumer gates.
- [ ] The `v0.2.0` tag workflow and fresh registry consumers must repeat the
  release gates after publication.

#### Phase 5B: Ship the first cloud service profile

Status: pinned in the unpublished `v0.2.0` standard candidate; tag publication
and fresh registry acceptance remain.

S3 is the first and only Phase 5 cloud service. The public path is
`Operator::s3`, backed by opaque `S3Auth` and `S3CredentialSource` values for
the default chain, static/session credentials, unsigned access, and assume
role. The source `standard` profile contains `memory`, `fs`, and `s3`; it is a
successor profile, not a runtime-selectable plugin beside `local`.

##### 5B.1 Profile-selection and distribution contract

The implemented choice keeps `local` small and unchanged in meaning. A package
release names exactly one pinned table in `native/artifact-selection.json`;
there is no runtime profile selector and therefore no duplicate native archive
inside one Moon process. The `v0.2.0` release candidate selects `standard`, a
superset of `local`. Maintainer source builds choose a profile explicitly
without changing the committed package selection.

The distribution contract records:

- stable profile identifiers and their compiled OpenDAL features;
- the single profile selected by package metadata;
- prevention of duplicate native symbols when packages are composed;
- cache keys that include package version, profile, host, and archive digest;
- coexistence of local and cloud artifacts in the shared Moon cache;
- per-profile archive size budgets and dependency/license inventory;
- exact system libraries, platform floors, and supported host matrix;
- version and ABI compatibility between the facade and every profile;
- failure messages for unsupported profile/host combinations before download;
- cold install, offline hot-cache, corruption quarantine, and concurrent
  install behavior for each profile.

##### 5B.2 Service configuration and credentials

`Operator::new(scheme, config=...)` remains the generic escape hatch, while S3
uses the typed `Operator::s3` constructor. The binding does not generate public
configuration records for every upstream service.

The S3 contract provides:

- required bucket and region plus optional root and endpoint;
- explicit and session credentials, default-chain credentials, unsigned
  access, and assume-role with either default-chain or static source;
- which configuration values are copied, normalized, or passed through;
- validation of empty, conflicting, malformed, and out-of-range typed values;
- secret redaction from `OpenDalError`, debug output, logs, snapshots, and CI;
- environment and shared-profile precedence without reading credentials in the
  MoonBit layer unnecessarily;
- path-style versus virtual-hosted endpoints where supported;
- timeouts and proxy/TLS behavior that are native-profile policy rather than
  implicit MoonBit defaults;
- no connection-URI parser and no named-profile string in this slice.

##### 5B.3 Integration and release tests

- A pinned MinIO server with ephemeral, masked credentials runs in CI on native
  Linux x86-64 and arm64.
- The suite covers construction/check, stat, whole/ranged/sequential read,
  whole/chunked write, Writer abort, recursive list, batch delete, one-shot
  copy, managed Copier, and presigned write/read/stat execution.
- Public and native tests cover typed credential modes, validation, capability
  mapping, and secret-free public representations.
- Candidate standard artifacts are built and consumed on every advertised
  host, and their exact identities and digests are committed as the `v0.2.0`
  pins. Tag-mode reproduction and registry execution remain release gates.
  Multipart thresholds, versioned buckets, and injected network faults remain
  continuous-hardening coverage rather than extra public API.

##### 5B exit criteria

- [x] The typed constructor copies inputs, rejects conflicting or malformed
  values, and keeps credentials out of public `Debug`/`Show` surfaces.
- [x] Capabilities reflect backend reality and unsupported operations are not
  emulated.
- [x] Pinned MinIO integration runs the standard source profile on native
  Linux x86-64 and arm64 with ephemeral, masked credentials.
- [x] Local and standard profile metadata and caches have independent
  identities; the immutable local pins are unchanged.
- [x] Pin the exact standard candidate artifacts and pass clean packaged
  consumers on every advertised host.
- [ ] Publish those exact assets and prove fresh registry consumers from the
  `v0.2.0` tag.

#### Phase 5C: Presigned operations and explicit layers

Status: included in the pinned but unpublished `v0.2.0` candidate as separate
presign and layer ABI groups; tag publication remains.

Delivered in the current tree:

- owned `PresignedRequest` and `PresignedHeader` snapshots plus capability
  checks for read, write, and stat;
- immutable `Operator::with_timeout`, `Operator::with_retry`, and
  `Operator::with_concurrency_limit` methods;
- explicit composition order of timeout, retry, then outermost concurrency
  limit, with duplicate and reverse-order configurations rejected;
- end-to-end presigned PUT, GET, and HEAD execution against pinned MinIO.

##### Phase 5C.1: Presigned requests

The binding-owned result contains an owned HTTP method, URI, and array of owned
header names with binary values. Inputs use an explicit expiry in seconds and
the existing read/write/stat options where meaningful.

Implemented work:

- presigned read, write, and stat methods as separate optional ABI functions;
- capability reporting for each presign operation;
- strict UTF-8 and header validation, owned snapshots, and paired native frees;
- expiry validation and stable behavior for unsupported services;
- tests that execute generated requests against the CI service rather than
  checking only their string shape;
- documentation warning that URLs and headers may contain credentials and must
  not be logged casually.

Presigned delete and other methods remain separate decisions; they should not
be included merely because upstream exposes them.

##### Phase 5C.2: Retry, timeout, and concurrency policies

The public surface contains explicit retry and timeout configuration plus one
concurrency limit layer whose operation and HTTP permit lifetimes are part of
the public contract.

Rules to preserve:

- no retry layer is installed by default;
- retry applies only to errors classified as temporary by the configured
  policy and changes exhausted errors to the documented persistent status;
- layer ordering is explicit and tested because timeout/retry order changes
  stateful operation behavior;
- composition order is timeout, then retry, then concurrency limit; the
  concurrency limit is outermost, is installed at most once, and prevents
  later timeout/retry additions;
- an operation permit remains held for the lifetime of body-style readers,
  writers, listers, deleters, and copiers, while an optional HTTP permit
  remains held until its response body is dropped;
- retry is opt-in and explicitly does **not** promise exactly-once Writer or
  append behavior: an uncertain remote commit may be replayed, so callers must
  choose the policy for stateful writes deliberately;
- timeouts and cancellation leave readers/writers in a documented state;
- each layer is represented by an optional artifact/feature dependency and
  does not enlarge unrelated profiles accidentally.

Logging, tracing, metrics exporters, custom retry observers, and other
callback-oriented layers remain deferred until their callback, thread, and
data-redaction contracts are reviewed. They must not be smuggled into the
first retry/timeout slice.

##### 5C exit criteria

- [x] Presigned requests execute end-to-end against the cloud integration
  service; snapshots own method, URI, header names, and binary header values.
- [x] Argument validation, layer immutability, duplicate rejection, and the
  timeout/retry/concurrency order are exercised from MoonBit and the native
  ABI.
- [x] Concurrency permits follow the documented body and HTTP response-body
  lifetimes.
- [x] Stateful resources become terminal after unsafe failures or
  cancellation; retry makes no exactly-once write promise.
- [x] Default operators retain `v0.1.0` no-implicit-retry behavior.
- [x] Source/native end-to-end tests cover the Phase 5C semantics, while every
  pinned candidate passes release identity and clean packaged-consumer gates.
- [ ] Reproduce those pins and run fresh registry consumers from the `v0.2.0`
  tag; broader deterministic network fault injection remains a hardening item
  rather than a release-surface expansion.

#### Phase 5D: Batch deletion and copier tasks

Status: included in the pinned but unpublished `v0.2.0` candidate with
deliberately smaller, honest contracts; tag publication remains.

##### Phase 5D.1: Batch deletion

The implemented `Operator::delete_many(ArrayView[String])` copies and validates
all paths before starting, then delegates the set to OpenDAL's high-level
deleter. Empty input succeeds. `Capability::delete_max_size()` exposes the
backend hint when present; the binding's own representability ceiling remains
separate.

The public result is intentionally `Unit`, not a fabricated ordered result
array. Success means all requested deletes completed. An error is
all-or-error at the MoonBit boundary and may follow partial remote effects;
it does not report per-path outcomes, promise atomicity, preserve input order,
or promise uniqueness. Duplicate and missing paths are accepted according to
the selected backend.

##### Phase 5D.2: Copier/task API

`Operator::open_copier(source, destination)` returns a managed, synchronous
`Copier` for one object on the same Operator. `next()` returns one byte-count
delta or stable `None`; `finish()` drives remaining steps and returns
destination metadata; `abort()` is explicit and idempotent after successful
abort. A failed operation is terminal, and abort cannot roll back effects the
backend already made visible.

This first Copier is intentionally not recursive, cross-Operator, or
cross-service. The binding does not simulate those claims with list/copy loops.
The resource owns the native operator state it needs and may outlive the
MoonBit Operator that opened it; finalization only frees state and never
reports finish or abort success.

##### 5D exit criteria

- [x] Batch input and Copier progress values are copied, bounded, and owned.
- [x] Batch all-or-error behavior and possible partial remote effects are
  documented without inventing per-path results.
- [x] Copier completion, failure, abort, stable end, and finalizer behavior are
  deterministic; the resource can outlive its originating Operator.
- [x] Memory reports unsupported Copier capability honestly, while S3 runs
  batch delete and Copier against MinIO.
- [x] Source/native and MinIO tests cover the Phase 5D lifecycle, while every
  pinned candidate passes release identity and clean packaged-consumer gates.
- [ ] Reproduce those pins and run fresh registry consumers from the `v0.2.0`
  tag. Recursive and cross-backend copying remain out of scope.

#### Phase 5E: Public MoonBit async API

Status: the first portable public async slice is included in the pinned but
unpublished `v0.2.0` candidate; full native blocking-API parity is
intentionally deferred.

The root package is selected at compile time for native or JavaScript, with
native as the default and explicit `--target js` for browsers.
`AsyncOperator::new` is the shared constructor; `AsyncOperator::close` is
idempotent and non-raising. `Operator::as_async()` remains a lightweight native
view. The portable surface contains whole `write`, whole/ranged `read`, bounded
`open_read_stream`/`next`/`close`, and
`open_writer`/`write`/`finish`/`abort`. It uses ordinary MoonBit `async fn`
methods, not public callbacks or native task handles. Native whole writes are
composed from the true async Writer and never run the synchronous API on an
executor thread.

Each native operation owns copied inputs and its result. A worker publishes
the result, then writes one byte to a private pipe watched by MoonBit's async
runtime; it never invokes MoonBit or retains a MoonBit reference from a foreign
thread. Cancelling the MoonBit operation requests native cancellation through
the task finalizer. Completion and cancellation contend under one native task
state so only one terminal result and one wake signal can win.

Streams and writers admit one in-flight operation. Cancelling `next`, `write`,
`finish`, or `abort` makes that stateful resource terminal when progress or
commit status may be unknown. Bounded stream calls return one output-bounded
owned chunk per operation and do not poll upstream while a locally split
remainder is pending. Whole-object async `read` still materializes one
output-bounded `Bytes` value. Neither guarantee bounds one raw buffer allocated
inside OpenDAL or the backend.

Async stat, list/lister, delete, copy/Copier, presign, and separate public task
handles are not part of the portable native slice. The JavaScript target has
additional capability-checked Promise operations and `AsyncLister`; portable
parity for those operations requires corresponding non-blocking native ABI
operations in a later slice.

##### 5E exit criteria

- [x] Supported operations wait through MoonBit's async pipe and do not block
  an async executor thread on a Rust runtime call.
- [x] Native task state enforces exactly-once completion/cancellation signaling
  and never calls back into MoonBit from a worker thread.
- [x] Bounded streaming preserves one-operation-at-a-time delivery, does not
  poll upstream while a remainder is pending, and preserves copied-input
  ownership.
- [x] Memory integration covers async range reads, stable stream EOF, writer
  finish, and idempotent successful abort.
- [x] The synchronous facade and ABI prefix remain backward compatible.
- [x] Source/native tests cover async runtime and cancellation semantics, while
  every pinned candidate passes release identity and clean packaged-consumer
  gates.
- [ ] Reproduce those pins and run fresh registry consumers from the `v0.2.0`
  tag; expand the async surface only in later independent slices.

### Parallel track A: Platform and artifact expansion

Status: Linux arm64 is pinned and clean-consumer tested in the unpublished
`v0.2.0` standard candidate; tag publication remains.

Additional targets are independent release lanes, not checkboxes added to a
generic support claim. The Phase 5 `standard` profile adds a target-native
Linux arm64 builder alongside the original two hosts. Intel macOS remains
blocked on an installable MoonBit CLI even though a native Rust runner exists;
Rust-only output is not accepted as a MoonBit release artifact. Musl Linux and
Windows remain later candidates and require their own ABI and packaging work.

Every new target requires:

- a documented compiler target, CPU baseline, OS/libc floor, and calling
  convention;
- exact native static libraries captured on that target;
- deterministic packaging and pinned archive/library digests;
- C11/C++17 header compilation plus a real linked C consumer;
- debug/release MoonBit consumers using the published artifact path;
- sanitizer or the closest platform-equivalent memory checks;
- cold/hot/offline/corruption/concurrency resolver tests;
- a release-builder strategy that does not misrepresent cross-compiled output
  as target-native validation.

Windows additionally needs an explicit MSVC/GNU choice, `__cdecl` and symbol
visibility verification, `.lib`/archive naming, system-library capture, path
and cache locking behavior, and a replacement or validation path for Unix
`tar` assumptions.

The Node.js 18 and `tar` requirement belongs to the current experimental Moon
prebuild mechanism, not to the desired permanent user experience. Track stable
Moon native-artifact support and remove these tools from the consumer contract
when a verified declarative replacement exists.

### Parallel track B: Continuous compatibility and hardening

These are ongoing requirements, not a final cleanup phase:

- keep release builds exactly pinned to OpenDAL and commit `Cargo.lock`;
- run a scheduled compatibility canary against candidate OpenDAL upgrades,
  but never update a published profile without ABI, behavior, artifact, and
  release-note review;
- fuzz ABI bootstrap sizes, alignments, flags, option prefixes, UTF-8 views,
  output clearing, and paired-free paths;
- stress handle state machines and close/read, abort/write, abort/finish, and
  cancellation/completion races;
- add large-object allocation and throughput benchmarks with recorded chunk
  and resident-memory ceilings;
- retain C11 and C++17 header consumers so the project-owned ABI remains usable
  independently of MoonBit;
- keep all guide examples compiler-checked and ensure unsupported capabilities
  remain stated explicitly;
- generate and review `pkg.generated.mbti` for every public change;
- inventory licenses and add SBOM/provenance or artifact attestations before
  expanding the cloud dependency graph;
- document security reporting and test credential/token redaction at every
  public and diagnostic boundary.

### Candidate release trains

Version numbers below are planning labels, not promises. A release may be split
if a milestone cannot satisfy its exit criteria independently.

| Release line | Intended content |
| --- | --- |
| Published `0.1.x` | Immutable `local` profile: memory/fs, original blocking facade, macOS arm64 and Linux x86-64 |
| Pinned, unpublished `0.2.0` release candidate | Phase 5A-E, `standard` memory/fs/S3 profile, and Linux arm64 |
| Later releases | Additional async operations, services, targets, recursive/cross-service transfer only after separate contracts |

Each release tag must build its native artifacts and Moon package from the same
commit, upload and verify every pinned artifact, publish the matching package,
and execute a clean registry consumer. An artifact-only rebuild uses a new
artifact revision and pinned digests; it must never replace different bytes at
an existing immutable identity without an explicit recovery procedure.

## Cross-cutting acceptance requirements

- Every non-primitive FFI parameter has an explicit ownership annotation.
- Rust, C, and MoonBit allocations are freed only by their owning runtime.
- Native handles have documented open, terminal, close/free, panic, poison,
  concurrency, and finalizer states before their ABI is implemented.
- No unwind, Rust reference, MoonBit pointer, borrowed UTF-8 view, or borrowed
  byte slice crosses the durable C ABI beyond its documented call lifetime.
- Whole-object and ranged reads reject values that cannot fit in one MoonBit
  `Bytes`; callers use `ReadStream` or `AsyncReadStream` when they need a hard
  per-returned-chunk output/copy bound.
- Each new optional ABI group is append-only, all-or-nothing, and dependent on
  BASE; unsupported groups leave their function pointers unset.
- ABI feature availability and backend operation capability remain distinct:
  the table can implement a function whose selected service reports
  `Unsupported`.
- Tests cover empty and binary content, embedded NUL bytes, Unicode paths,
  invalid UTF-8 at native boundaries, missing objects, unsupported operations,
  overflow, early garbage collection, repeated close/free, and failure paths.
- Stateful tests cover normal errors, panics, lock poisoning, concurrent calls,
  finalization, and every legal or rejected terminal-state transition.
- Public API changes are reviewed through generated interface diffs.
- Public concrete result/configuration types live in the facade or an
  intentionally public package, never behind an `internal/*` re-export whose
  constructors or methods downstream users cannot resolve reliably.
- Published documentation and capability claims describe only functionality
  present in the selected native service profile.
- New profiles and targets pass deterministic packaging, cache-integrity,
  offline reuse, and clean-consumer tests before release.
- Each feature lands as a complete vertical slice rather than as a large batch
  of unwrapped native functions.

## Definition of done for a vertical slice

A feature is not complete when the Rust method exists. Every slice includes:

1. reviewed MoonBit semantics, non-goals, and compiler-checked acceptance
   examples;
2. capability and error behavior, including unsupported-backend behavior;
3. versioned C ABI declarations with pointer, size, ownership, thread, panic,
   output-clearing, and allocation/free contracts;
4. Rust ABI implementation and adversarial ABI tests;
5. a mechanical package-local C stub with explicit MoonBit ownership
   annotations;
6. a safe MoonBit facade with no public raw handles or extern declarations;
7. black-box downstream tests and white-box tests only where internal state is
   the subject;
8. debug/release, C/C++, coverage, sanitizer, packaged-consumer, and artifact
   validation proportional to the feature's risk;
9. regenerated and reviewed `pkg.generated.mbti`;
10. checked guides, support matrix, limitations, migration/release notes, and a
    clean registry consumer when the slice is released.

## Resolved Phase 5 decisions and remaining follow-up

| Decision | State | Outcome |
| --- | --- | --- |
| Sequential read API and bounds | Resolved | `ReadStream`, fixed positive `chunk_size`, one owned output-bounded chunk, one possible larger upstream buffer, stable EOF |
| Writer cleanup | Resolved | Explicit terminal `finish`/`abort`; successful repeated abort is idempotent; finalizers never commit |
| ABI extension strategy | Resolved | Append-only v1 optional groups through ABI `1.7`, with older-prefix negotiation tests |
| Standard profile selection | Resolved for `v0.2.0` | One pinned successor profile containing memory/fs/S3; no public runtime selector |
| S3 configuration ownership | Resolved | Typed `Operator::s3`, opaque auth/source values, copied input, generic constructor retained as escape hatch |
| Presigned result | Resolved | Owned method/URI/binary-header snapshot with explicit expiry and no automatic HTTP execution |
| Layer order and replay policy | Resolved | Immutable timeout -> retry -> concurrency; opt-in retry makes no exactly-once stateful-write promise |
| Batch result shape | Resolved | All-or-error `Unit`, possible partial remote effects, no fabricated ordered per-path result |
| Copier scope | Resolved | Managed same-Operator one-object copy; not recursive or cross-service |
| Initial async shape | Resolved | MoonBit `async fn` facade using owned native tasks and a pipe wakeup; no public callbacks/task handles |
| Standard release trust roots | Pinned candidate; tag verification pending | `artifacts-standard.json` is populated and selected atomically; publish the exact bytes and verify fresh registry consumers from `v0.2.0` |
| Additional services and async parity | Deferred | Add only as independent, capability-honest vertical slices |
| Intel macOS, musl, and Windows | Deferred | Require installable MoonBit toolchains plus target-native build/link/consumer evidence |
| Replacement for experimental prebuild tooling | Open toolchain follow-up | Adopt stable native-artifact support when it can preserve pinned, ordinary-package installation |
