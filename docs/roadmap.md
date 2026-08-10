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
- Model operation options as immutable MoonBit values. Native code copies any
  data it retains.
- Treat async, callbacks, presigning, layers, batch operations, and the broad
  cloud-service matrix as later work.

## Delivery order

### Phase 0: Public semantics (complete, awaiting API review)

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
- a declaration-only MoonBit API contract;
- type-checked public API usage tests.

### Phase 1: ABI contract and memory-service spike

Freeze a small, versioned C ABI and implement one complete slice:

- ABI/version query;
- memory operator construction;
- whole-object write and read;
- error copying and freeing;
- handle and buffer destruction.

Before implementation, document every function's pointer validity, ownership,
allocation/free pair, nullability, error behavior, panic boundary, and thread
contract.

Exit criteria:

- Rust ABI tests, C smoke tests, and MoonBit tests pass;
- native debug and release builds pass;
- AddressSanitizer/LeakSanitizer find no binding-owned leaks or invalid access.

### Phase 2: Blocking core MVP

Add vertical slices in this order:

1. `stat`, `exists`, `delete`, and `create_dir`;
2. `Lister` and `Entry`;
3. filesystem service end-to-end tests;
4. basic capability inspection.

### Phase 3: Streaming and operation options

Add:

- random-access `Reader` range reads without a hidden cursor;
- `Writer` with explicit `open/closed/failed` state;
- explicit, terminal `close` semantics;
- immutable read, write, stat, and list options;
- copy and rename without semantic emulation.

### Phase 4: Distribution and service profiles

- Decide and document source-build versus prebuilt-artifact installation.
- Publish checksummed native artifacts for selected OS/architecture pairs.
- Keep service features explicit and grouped into supported profiles.
- Establish the minimum MoonBit and Rust toolchains.

The initial platform target is macOS and Linux. Windows support requires a
separate linker and artifact-distribution decision.

### Phase 5: Deferred capabilities

Consider cloud service profiles, presigning, layers, batch operations, and
async only after the blocking API and native lifecycle model are stable.
Async requires a separate design for runtime ownership, cancellation,
cross-thread callbacks, and exactly-once completion.

## Cross-cutting acceptance requirements

- Every non-primitive FFI parameter has an explicit ownership annotation.
- Rust, C, and MoonBit allocations are freed only by their owning runtime.
- Whole-object reads reject values that cannot fit in a MoonBit `Bytes`;
  streaming remains available for larger objects.
- Tests cover empty and binary content, embedded NUL bytes, Unicode paths,
  invalid UTF-8 at native boundaries, missing objects, unsupported operations,
  overflow, early garbage collection, repeated close/abort, and failure paths.
- Public API changes are reviewed through generated interface diffs.
- Each feature lands as a complete vertical slice rather than as a large batch
  of unwrapped native functions.

## Open design decisions

- Exact public type and method names.
- A reliable public Writer abort operation, which requires an async-writer
  implementation in the Rust shim rather than OpenDAL's blocking Writer.
- Default retry policy; it must not be inherited accidentally from another
  binding.
- Source-build and prebuilt-library installation contract.
