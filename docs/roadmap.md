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
- Treat async, callbacks, presigning, layers, batch operations, and the broad
  cloud-service matrix as later work.

## Status at a glance

| Area | Status | Released baseline |
| --- | --- | --- |
| Public semantics and ABI foundation | Complete | ABI `1.0.0` |
| Blocking local operations | Complete | `memory` and `fs` |
| Random reader, chunked writer, copy, and rename | Complete | Moon package `0.1.0` |
| Native package distribution | Complete | macOS arm64 and glibc Linux x86-64 |
| Local large-object and writer lifecycle completion | Planned next | Phase 5A |
| Cloud/network profiles | Planned | Phase 5B onward |
| Presign, layers, batch, copier, and async | Deferred and ordered below | Phase 5C onward |

`v0.1.0` is the compatibility baseline for all future work. A later feature
may add public methods, optional ABI groups, service profiles, or artifacts,
but it must not silently change the existing local profile's operation,
resource-lifecycle, retry, or error semantics.

Roadmap status words have the following meaning:

- **planned**: scope and dependency order are known, but the public contract
  is not frozen;
- **designed**: public semantics, ABI ownership, state machines, and acceptance
  examples have been reviewed;
- **implemented**: all vertical-slice code and local validation are complete;
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

### Phase 5: Deferred capabilities

Status: planned. No Phase 5 public API is frozen yet.

The user guides were checked against OpenDAL's binding overview, connecting,
getting-started, and common-task documentation. The missing capabilities are
real product gaps, but they do not form one safe implementation batch. Phase 5
therefore consists of separately releasable milestones with this dependency
order:

```text
5A local lifecycle completion
  -> 5B first cloud profile
       -> 5C presign and explicit operational layers
            -> 5D batch and copier tasks
                 -> 5E public async API

platform/artifact expansion and continuous hardening run in parallel
```

This order is intentional:

- large-object reads and reliable writer cleanup must work before remote
  services make memory use and unfinished multipart uploads expensive;
- presigning needs a service that can exercise signed HTTP requests;
- retry and timeout policies need stateful stream/write rules first;
- copier cancellation and public async operations need a reviewed cross-runtime
  task model rather than ad-hoc callbacks;
- enabling a scheme in documentation never makes it available in an already
  compiled static library.

#### Phase 5A: Complete the local blocking lifecycle

Status: recommended next milestone.

##### Goals

- read arbitrarily large objects without materializing the whole object in one
  Rust buffer or one MoonBit `Bytes` value;
- make the per-call output limit a hard native allocation/copy bound;
- provide an explicit and reliable Writer abort path;
- use OpenDAL's async reader/writer internally where necessary while preserving
  the current synchronous MoonBit facade;
- preserve every released `Reader`, `Writer`, `Operator::read`, and
  `Operator::write` behavior.

##### 5A.1 Public-semantics design

The sequential API must use a new public resource type. The existing `Reader`
remains a reusable, concurrent, random-access reader whose calls name an
independent `ByteRange`; it must not acquire a hidden cursor.

A working API shape, to be finalized by compiler-checked acceptance examples,
is:

```moonbit nocheck
let stream = operator.open_read_stream(
  "large.bin",
  range=ByteRange::Full,
  chunk_size=1024 * 1024,
)
while stream.next() is Some(chunk) {
  consume(chunk)
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
- preservation of `Full`, `From`, `Range`, and `Suffix`, plus `version`,
  `if_match`, and `if_none_match` options;
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
- no prefetch or concurrency default that can violate the configured memory
  bound.

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
- return one Rust-owned bounded chunk per `next` call using the existing
  two-phase buffer-copy contract;
- reuse the fallible process-wide Tokio runtime, but do not hold a Rust mutex
  across arbitrary MoonBit work;
- contain every panic at the C boundary and transition stateful handles to the
  documented terminal state;
- keep finalizers free of blocking network I/O and unable to surface false
  completion.

The bounded read path must consume at most one configured chunk at a time. It
must not collect an OpenDAL stream, flatten multiple buffers, or perform a
whole-object blocking read before checking the limit. The same implementation
strategy should be evaluated for `Operator::read` so that its documented
`max_output_len` becomes a hard bound rather than only a post-read rejection.

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

- the public API and generated `.mbti` diff match the reviewed acceptance
  examples;
- a large-object test demonstrates bounded incremental reads and records the
  maximum native chunk allocation;
- no existing `v0.1.0` API or behavior changes unintentionally;
- older ABI v1.0 callers still negotiate and run successfully;
- all local, CI, sanitizer, artifact, and registry-consumer gates pass.

#### Phase 5B: Ship the first cloud service profile

Status: planned after Phase 5A. S3 is the initial candidate because it can be
tested against local S3-compatible implementations and exercises the common
credential, multipart, conditional-operation, and presign requirements. The
choice is not frozen until the artifact and configuration design is reviewed.

##### 5B.1 Profile-selection and distribution contract

The current `local` artifact must remain small and unchanged in meaning. Before
adding a service feature, decide how an ordinary Moon consumer selects exactly
one compatible native profile without cloning this repository, installing
Rust, setting linker flags, or depending on ambient environment variables.

The design must specify:

- stable profile identifiers and their compiled OpenDAL features;
- whether profile selection is expressed by a distinct Moon package/module or
  another declarative package mechanism;
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

Keep `Operator::new(scheme, config=...)` as the generic escape hatch, but add a
typed or builder-style configuration path for each deliberately supported
cloud service. Do not attempt to generate and freeze types for every upstream
service at once.

For the first S3 profile, freeze and document:

- required bucket/root fields and optional endpoint/region behavior;
- explicit credentials, session credentials, role/profile/default-chain
  behavior, and anonymous or unsigned access;
- which configuration values are copied, normalized, or passed through;
- strict rejection of unknown, conflicting, and malformed options;
- secret redaction from `OpenDalError`, debug output, logs, snapshots, and CI;
- environment and shared-profile precedence without reading credentials in the
  MoonBit layer unnecessarily;
- path-style versus virtual-hosted endpoints where supported;
- timeouts and proxy/TLS behavior that are native-profile policy rather than
  implicit MoonBit defaults;
- connection-URI parsing only if it can avoid credential leakage and ambiguous
  percent-decoding; otherwise leave URI construction deferred.

##### 5B.3 Integration and release tests

- run an isolated S3-compatible server in CI with ephemeral credentials;
- cover construction, check, stat, read, sequential read, whole/chunked write,
  abort, list, delete, copy, rename-capability reporting, and error context;
- exercise multipart thresholds, empty objects, Unicode paths, large objects,
  conditions, versions where available, and temporary network failures;
- prove credentials never appear in errors, snapshots, command output, release
  manifests, or cache paths;
- build and test every advertised cloud artifact on every advertised host;
- verify a clean registry consumer selects, downloads, verifies, links, and
  runs the cloud profile without a Rust toolchain.

##### 5B exit criteria

- one cloud profile is released without changing the installation contract or
  inflating the `local` profile unexpectedly;
- service capabilities reflect backend reality and unsupported operations are
  never emulated silently;
- credential-chain and redaction behavior are documented and tested;
- local and cloud profiles can be cached and maintained independently.

#### Phase 5C: Presigned operations and explicit layers

Status: planned after a cloud profile exists. Presigning and layers are
separate ABI/public-API slices even if they share a release.

##### Phase 5C.1: Presigned requests

Define a binding-owned, fully owned result type containing the HTTP method,
URI, and headers. Inputs use an explicit expiry/duration representation and
the existing read/write/stat options where meaningful.

Required work:

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

Start with explicit retry and timeout configuration. Add concurrency limiting
only after its interaction with sequential readers and writers is specified.

Rules to preserve:

- no retry layer is installed by default;
- retry applies only to errors classified as temporary by the configured
  policy and changes exhausted errors to the documented persistent status;
- layer ordering is explicit and tested because timeout/retry order changes
  stateful operation behavior;
- retry never replays a non-replayable write body after an uncertain commit;
- timeouts and cancellation leave readers/writers in a documented state;
- each layer is represented by an optional artifact/feature dependency and
  does not enlarge unrelated profiles accidentally.

Logging, tracing, metrics exporters, custom retry observers, and other
callback-oriented layers remain deferred until their callback, thread, and
data-redaction contracts are reviewed. They must not be smuggled into the
first retry/timeout slice.

##### 5C exit criteria

- presigned requests work end-to-end against the cloud integration service;
- retry/timeout behavior is deterministic under injected failures and virtual
  or bounded test time;
- stateful bodies are never resumed or replayed after an unsafe terminal
  transition;
- default operators retain the `v0.1.0` no-implicit-retry behavior.

#### Phase 5D: Batch deletion and copier tasks

Status: planned after remote-service lifecycle and policy behavior are stable.

##### Phase 5D.1: Batch deletion

- expose backend-native batch deletion with a bounded input batch size;
- define ordered per-path success/failure results and preserve path context;
- report partial failure without discarding successful outcomes;
- expose batch-size/capability limits rather than assuming one universal size;
- do not claim backend batch semantics for a hidden serial loop; a separate
  convenience helper may be documented later if desired;
- test duplicate paths, missing paths, versions, Unicode, large batches,
  partial failures, and cancellation/timeout interactions.

##### Phase 5D.2: Copier/task API

Design a managed task handle rather than a single long blocking call. Freeze:

- source and destination operator ownership and lifetime;
- recursive traversal, overwrite, metadata, concurrency, and failure policy;
- progress snapshots with bounded path/error payloads;
- explicit completion, cancellation, and partial-effect semantics;
- whether a task can be polled synchronously before the public async API;
- finalizer behavior that never reports cancellation or completion falsely;
- cross-service capability checks and non-emulation rules.

##### 5D exit criteria

- batch results and copier progress remain bounded and own all returned data;
- cancellation and partial failures have deterministic, documented outcomes;
- resources can outlive their originating operators without use-after-free;
- stress tests cover large trees, races, and early finalization.

#### Phase 5E: Public MoonBit async API

Status: deliberately last. Internal use of Tokio in earlier phases does not
constitute or promise a public MoonBit async surface.

Before exposing one async operation, write and review a dedicated design that
answers:

- whether the public surface uses MoonBit `async fn`, explicit task handles, or
  both;
- ownership and shutdown of the process-wide Rust runtime;
- how MoonBit continuations/callbacks are retained, rooted, invoked, and
  released across GC and foreign threads;
- thread-affinity and reentrancy rules for every callback;
- exactly-once completion across success, native error, panic, cancellation,
  and handle destruction races;
- cancellation token ownership and the point after which remote side effects
  may already have occurred;
- backpressure for streamed reads/writes without unbounded native queues;
- error propagation into MoonBit's checked async error model;
- behavior during process shutdown and late finalization;
- coexistence of sync and async operators without nested-runtime deadlocks.

Implementation begins with one small end-to-end async slice and its
cancellation tests. Do not mechanically duplicate the full blocking interface
or expose callbacks for every native function. Expand only after the first
slice proves ownership, backpressure, cancellation, and completion invariants.

##### 5E exit criteria

- no blocking wait occurs on a MoonBit async executor thread in the supported
  path;
- completion is exactly once under adversarial cancellation and GC races;
- callback-held MoonBit objects neither leak nor become dangling references;
- bounded streaming preserves backpressure;
- sync API behavior and artifact installation remain backwards compatible.

### Parallel track A: Platform and artifact expansion

Additional targets are independent release lanes, not checkboxes added to a
generic support claim. Candidate targets include Linux arm64, macOS x86-64,
musl Linux, and Windows. Prioritize them from user demand, CI availability,
artifact size, and maintenance cost.

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

| Candidate release | Intended content |
| --- | --- |
| `0.2.x` | Phase 5A bounded sequential reader and Writer abort |
| `0.3.x` | Phase 5B first cloud profile and typed configuration path |
| `0.4.x` | Phase 5C presign plus explicit retry/timeout policies |
| later `0.x` | Phase 5D batch/copier and Phase 5E async, each independently releasable |

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
  `Bytes`. Until bounded sequential reads land, callers must choose independent
  ranges that each fit this limit.
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

## Open design decisions

| Decision | Blocks | Required outcome |
| --- | --- | --- |
| Read-stream name, chunk-size API, and terminal error behavior | 5A | Resolved in `docs/design/public-api-semantics.md` |
| Hard native allocation bound for whole and sequential reads | 5A | No whole-object materialization before the limit |
| Repeated abort and finalizer behavior | 5A | Explicit Writer state machine |
| ABI v1 minor-extension layout and feature grouping | 5A | Old caller compatibility test |
| Error `Operation` mapping for new read/write lifecycle calls | 5A | No accidental exhaustive-enum source break |
| Consumer-facing native profile selection | 5B | Ordinary Moon dependency with exactly one compatible archive |
| First cloud service and typed configuration ownership | 5B | One reviewed, testable profile rather than a broad matrix promise |
| Credential-chain precedence and connection-URI security | 5B | No secret leakage or ambiguous configuration |
| Presigned request types and expiry representation | 5C | Owned, bounded, executable request snapshot |
| Retry/timeout ordering and stateful body policy | 5C | No unsafe replay or hidden default policy |
| Batch partial-result and copier cancellation semantics | 5D | Bounded deterministic task results |
| MoonBit async shape, callback rooting, cancellation, and runtime shutdown | 5E | Exactly-once completion design |
| Next CPU/OS/libc targets | Parallel A | Demand-backed compatibility and CI plan |
| Replacement for experimental prebuild tooling | Parallel A | Stable artifact installation without unnecessary consumer tools |
