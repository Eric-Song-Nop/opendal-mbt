# OpenDAL for MoonBit on WebAssembly

Status: accepted direction; backend-neutral callback binding implemented as an
unpublished repository candidate

Last reviewed: 2026-08-14

Distribution and release plan:
[`wasm-mooncake-delivery.md`](wasm-mooncake-delivery.md).

## Decision

Proceed with a WebAssembly binding in which MoonBit application code constructs
OpenDAL operators by service scheme and configuration. The Rust/OpenDAL bridge
and the MoonBit application remain separate core WebAssembly modules for the
first product path. A repository-owned loader instantiates and connects them;
application code sees only the MoonBit facade.

The public binding is backend-neutral:

```moonbit
let operator = @opendal.Operator::new(
  "memory",
  config={ "root": "example" },
)
```

The same constructor is used for every service compiled into an artifact.
There is no service-specific public constructor. `available_schemes()` reports
the compiled registry, `Operator::info()` reports the configured operator, and
OpenDAL supplies the capability values.

Memory is the current deterministic test fixture. OPFS may later be an
optional browser-persistence fixture; it is neither an architectural
milestone, a privileged API, nor evidence for other services.

## Product boundary

The intended consumer experience is a normal MoonBit dependency. A user must
not need to know that OpenDAL is written in Rust, write an import object,
manage two memories, or install a Rust toolchain.

There are currently three distinct integration layers:

| Layer | Meaning | Current decision |
| --- | --- | --- |
| Moon source package | Moon compiles the `Eric-Song-Nop/opendal/wasm` facade with the application | Public application-code boundary |
| Core-Wasm dependency | Moon-produced Wasm imports functions from the Rust-produced OpenDAL bridge | Implemented runtime boundary |
| Wasm component | WIT and the canonical ABI describe composition and resources | Later evaluation, not the critical path |

Moon `0.1.20260807` and moonc `0.10.7+bc794d341` do not link an arbitrary
precompiled Rust `.wasm` as an ordinary Moon library. Moon supports Wasm
imports, exports, exported memory, and Component Model tooling, but the
repository must still deliver and compose the companion bridge. A working
checkout is therefore an engineering proof, not a Mooncake release.

## Public MoonBit API

The current experimental `/wasm` facade exposes:

- `available_schemes()`;
- `Operator::new(scheme, config)`, `Operator::info()`, and
  `Operator::as_async()`;
- `AsyncOperator::{check_callback, exists_callback, read_callback,
  stat_callback, write_callback, create_dir_callback, delete_callback,
  list_callback, copy_callback, rename_callback}` for the ten common
  whole-object operations, with bounded list materialization;
- native-shaped `ByteRange`, `Timestamp`, and complete `Metadata` value types;
- read range/version/conditions, stat version/conditions, and write append/
  content headers/conditions, with write and stat returning `Metadata`;
- `Task::state()`, logical `cancel()`, and `close()`;
- owned native-shaped `OpenDalError`/`ErrorInfo`, `Metadata`, `Entry`,
  `OperatorInfo`, and `Capability` values;
- explicit `Operator::close()` and the Wasm lifecycle extension
  `AsyncOperator::close()`.

The public package does not expose:

- synchronous storage methods;
- `Operator::memory` or another backend-specific constructor;
- Rust pointers, Wasm offsets, or either `WebAssembly.Memory`;
- loader callbacks, JavaScript promises, raw task/completion handles, or
  wasm-bindgen values;
- bridge ABI/feature functions, leak counters, forced-pending counters, or
  other canary diagnostics.

Those last diagnostics and the bridge's synchronous poll-once exports remain
available only as low-level ABI fixtures. Their presence in the Rust module
does not make them MoonBit API.

The callback shape is explicit:

```moonbit
let task = operator.as_async().read_callback(
  "notes/hello.txt",
  range=@opendal.Range(offset=0UL, length=4096UL),
  callback=result => {
    match result {
      Ok(bytes) => consume(bytes)
      Err(error) => report(error)
    }
  },
)

// Both are idempotent. Cancellation suppresses callback delivery but does not
// promise to abort the underlying browser operation.
task.cancel()
task.close()
```

`check_callback` returns `Unit`, while `exists_callback` returns `Bool`.
`read_callback` accepts an optional `ByteRange` (`Full`, `From`, `Range`, or
`Suffix`), version,
`if_match`, and `if_none_match`. `stat_callback` accepts the same version and
condition values. `write_callback` accepts append, four content headers, and
conditions; its success value is `Metadata`, not `Unit`. `copy_callback`
returns the metadata supplied by OpenDAL, while `rename_callback` returns
`Unit`; both preserve source and destination context on failure. Suffix
support is reported and accepted only when the base service implements native
suffix reads, not when a completion layer could simulate one with an extra
stat.

This surface stays experimental until MoonBit provides a documented
ordinary-browser continuation contract or the project explicitly chooses
callbacks as the stable browser API. The binding does not present blocking
methods as an async browser implementation.

## Runtime architecture

```text
MoonBit browser application
  -> Eric-Song-Nop/opendal/wasm facade
  -> versioned scalar resource/task imports
  -> repository-owned loader
       -> later-turn task observation and callback delivery
       -> bounded copies between independent memories
       -> instance teardown
  -> wasm-bindgen-processed Rust bridge
  -> wasm_bindgen_futures::spawn_local
  -> OpenDAL async core
  -> service selected by scheme/config from the compiled registry
```

JavaScript is an instantiation, scheduling, and copying adapter. Storage
semantics, configuration, capability inspection, task ownership, and errors
remain in the MoonBit facade and Rust OpenDAL bridge.

The native `Eric-Song-Nop/opendal` package remains separate and compatible.
Its C/static-library ABI, `moonbit.h` object conversion, POSIX pipe completion,
host archive resolver, and multithreaded Tokio runtime are not reused as Wasm
binary layouts or execution mechanisms.

## Boundary contract

The core-Wasm boundary follows these rules:

- direct calls carry fixed-width scalars only;
- operators, buffers, tasks, completions, metadata, entry lists, and errors are
  owned behind generation-checked integer handles;
- every owned handle has a matching release operation;
- strings and bytes are copied; neither language's object layout is ABI;
- operator configuration is capped at 1,024 entries and 1 MiB of combined
  key/value UTF-8, with ASCII-case-insensitive duplicate keys rejected;
- Rust and MoonBit define, export, and allocate from independent linear
  memories;
- whole-object materialization is limited to 64 MiB;
- host copy calls transfer at most 256 KiB and re-read both memory buffers for
  every window so `memory.grow` cannot leave stale typed-array views;
- bounded list materialization fails atomically above 65,536 entries or 16 MiB
  of combined path, name, and pre-encoded metadata snapshot bytes;
- asynchronous OpenDAL errors are owned by their completion and copied into
  immutable MoonBit errors;
- bridge teardown is permanent for one instance and late completion is inert.

Structured errors use an owned, versioned `ODE1` little-endian snapshot that
preserves the OpenDAL kind, status, kind name, and message atomically. MoonBit
validates the entire snapshot before constructing `OpenDalError` and attaches
the initiating operation, path, and destination from its owned callback
context. Unknown future kind/status codes remain representable; malformed
snapshots become `AbiMismatch` binding errors.

Metadata uses an owned, versioned `ODM1` little-endian snapshot. Its 84-byte
header carries schema and presence bits, mode/current/deleted scalars, content
length, signed Unix seconds plus canonical nanoseconds, two reserved words,
and seven string lengths. Strict UTF-8 payloads follow in cache-control,
content-disposition, content-encoding, content-MD5, content-type, ETag, and
version order. MoonBit validates the entire snapshot, including canonical
absence and the 64 MiB limit, before constructing `Metadata`. Completion takes
are failure-atomic: a successful take consumes the completion and returns one
owned snapshot buffer, which MoonBit releases after parsing. List entries use
the same schema with the potentially partial metadata supplied by the lister;
they do not cause per-entry stat calls.

The bridge ABI is 1.7 (`0x00010007`). It reports feature bitmap `0x00000fff`:
memory and poll-once fixture bits plus generation handles, binary buffers, task
ABI, generic operator construction, common mutations, bounded list, bulk
transfer, structured errors, metadata/options, and bit 11 for core async
parity. The public facade requires `0x00000ffc`; it does not require the memory
or poll-once fixtures.

## Task and callback semantics

Each public operation copies its inputs and clones its OpenDAL operator before
returning a `Task`. Rust schedules the future with `spawn_local` and, by
default, awaits it directly. A raw test-only switch can add a zero-delay wrapper
to tasks started afterward, guaranteeing an observed first `Pending` poll. The
Chrome canary enables this switch; production initialization does not.

The loader begins task observation from a microtask and moves repeated pending
polls to later timer turns. The MoonBit callback therefore cannot run from the
initiating MoonBit/Rust stack.

The lifecycle contract is:

```text
Pending -> Ready(Completion) -> Consumed -> released
Pending or Ready -> Cancelled -> released
MoonBit Task: Pending -> Completed | Cancelled -> Closed
```

- one task publishes at most one completion;
- one ready result can be taken once;
- cancellation unregisters the loader wait before task cancellation/release;
- cancellation is logical and makes late completion inert;
- an in-flight task owns its cloned operator even after the MoonBit wrapper is
  closed;
- `runtime.dispose()` stops all waits, clears bridge resources, and prevents
  late callbacks;
- task/completion errors do not depend on one shared sticky error slot.

True cancellation of a Fetch, OPFS, or another host promise and streaming
reader/writer backpressure are separate future contracts.

## Current evidence

The repository separates static, Node, and browser evidence:

| Check | Claim |
| --- | --- |
| `make wasm-static-contract` | Exact imports/exports, independent memory shape, forbidden-import policy, Moon-to-Rust import resolution, and raw/gzip/Brotli size ceilings match the committed browser-memory snapshot |
| `make wasm-canary` | Node can instantiate the modules, use the generic callback facade, transfer a 16 MiB value in bounded chunks, verify write headers and returned metadata, perform a ranged read, verify stat/list metadata, exercise check/exists, verify copy/rename `Unsupported` errors retain source/destination context, run the callback lifecycle twice, clean up, and tear down |
| `make wasm-browser-canary` | Real Chrome/Chromium runs the same core-operation lifecycle and observes 16 checked, non-cancelled tasks enter the explicitly forced `Pending` state, along with browser heartbeat ordering, cancellation/race behavior, concurrent operators, and inert pending teardown |

Only Chrome is evidence for forced `Pending -> Ready` behavior and a responsive
browser event loop. Node exercises real `spawn_local` tasks and callbacks but
does not enable the forced-pending hook or establish browser scheduling.

The committed static snapshot is an implementation contract, not a release
manifest. It pins the current browser-memory artifacts' imports, exports,
memory limits, and size ceilings. It does not yet establish artifact
provenance, immutable release hashes, registry delivery, startup budgets, or
compatibility with a different compiled-service profile.

## Engineering milestones

### M0 — Core-module interoperability: implemented

The MoonBit facade, loader, Rust bridge, version negotiation, generic memory
operator, binary/error/metadata ownership, generation handles, repeated
lifecycle, and teardown are exercised end to end. The static contract records
the current module boundary.

M0 is not a distribution release. The modules are still built from this
checkout.

### M1 — Callback task lifecycle: implementation complete; public API experimental

Check, exists, read, stat, write, create-dir, delete, list, copy, and rename use
owned tasks and per-completion results. Read/stat/write expose the native-shaped
option subset, write and stat return full-shaped metadata values, list returns
the lister's potentially partial metadata without per-entry stat, and
copy/rename failures retain source and destination context. Browser evidence
covers the forced-`Pending` state for 16 checked, non-cancelled tasks,
later-turn delivery, cancel-before-ready, completion-before-cancel, terminal
close, concurrent operators, close/teardown with work pending, and
late-completion suppression.

The stable MoonBit continuation decision is not complete; the public callback
surface remains experimental. This is an API-status gap, not an excuse to
route storage through synchronous poll-once calls.

### M2 — Bounded bulk transfer: implemented

Public paths copy in 256 KiB windows, cap materialized objects at 64 MiB, use
fresh views across memory growth, and avoid per-byte bridge calls. The 16 MiB
canary checks exact bytes, chunk-proportional calls, non-zero source slices,
both modules' memory growth, and allocation-limit failure without publishing a
partial handle.

### M3 — Mooncake delivery: pending

The package still needs a versioned manifest, immutable bridge/glue/loader
artifacts, verified acquisition or embedded runtime assets, relocatable output,
and a clean consumer without Cargo, npm, this checkout, or handwritten glue.
Until that exists, the binding is an unpublished repository candidate.

### M4 — Service-profile fixtures: pending

The generic constructor and capability surface already exist. M4 is not “add
OPFS support to the API”; it is to package additional browser-compatible
OpenDAL services and validate each one's actual host, security, persistence,
and cancellation behavior through the same public API.

## Service model

OpenDAL may support a backend while a particular Wasm artifact does not
compile it, or while a browser host cannot satisfy its requirements. Claims
therefore have three layers:

1. the binding API accepts a scheme and configuration;
2. the artifact declares which schemes it compiled;
3. a service fixture proves the required browser host behavior.

No single service fixture proves another. Examples include:

- memory for deterministic lifecycle, error, transfer, and packaging tests;
- OPFS for optional secure-context persistence, reload, quota, and permission
  tests;
- an HTTP object service for CORS, forbidden headers, redirects, temporary
  credentials, timeout, and redaction tests.

Adding any of these is a profile and test change. Application code continues
to use `Operator::new(scheme, config)` and capability checks.

## Distribution requirements

A Mooncake preview must deliver the facade, loader, bridge Wasm, wasm-bindgen
glue, and a machine-readable manifest as one version-compatible set. A clean
consumer must be able to build and deploy without:

- Rust, Cargo, rustup, or a source checkout;
- npm or an application-authored JavaScript import table;
- knowledge of Moon's dependency cache layout;
- an unverified “latest” artifact fallback;
- a manual native-resolver environment variable.

If current Moon cannot propagate dependency runtime assets, a versioned
repository-owned `moonx` bundler is an acceptable preview bridge. The preferred
stable result remains ordinary `moon add` plus `moon build --target wasm`.
WIT/component composition should replace the core-module loader only if it
improves this application and distribution contract with browser evidence.

## Main open risks

| Risk | Required evidence |
| --- | --- |
| MoonBit browser continuations are not yet a stable host contract | Documented ordinary-browser suspension/resumption or an explicit callback-only release decision |
| Dependency runtime assets are not propagated | Clean Mooncake consumer using a verified official bundler or a Moon-supported asset graph |
| Generic constructor may be read as universal runtime support | Exact compiled-scheme manifest and per-service host acceptance |
| Logical cancellation may waste underlying I/O | Service-specific abort probes; retain the weaker claim until proven |
| Two memories increase transfer cost | Current bounded M2 evidence, followed by streaming/backpressure measurements |
| Service profiles increase download/startup size | Static size ceilings per profile plus measured startup budgets |
| Browser credentials can leak | Temporary-credential/presign/broker policy and redaction tests |

## Explicit non-goals for the first preview

- Reusing the native C ABI or static archive inside Wasm.
- Exposing blocking storage methods in a browser facade.
- Making memory, OPFS, S3, or another backend a privileged constructor.
- Claiming every OpenDAL service works in browsers, Workers, Node, or WASI.
- Sharing one allocator merely because both modules use linear memory.
- Requiring application authors to maintain loader glue.
- Treating the static canary snapshot as release provenance.
- Blocking useful core-module delivery on a WIT redesign.

## References

- [MoonBit WebAssembly integration](https://docs.moonbitlang.com/en/latest/toolchain/wasm/index.html)
- [MoonBit Wasm FFI](https://docs.moonbitlang.com/en/latest/language/ffi.html)
- [MoonBit package Wasm link options](https://docs.moonbitlang.com/en/latest/toolchain/moon/package.html#wasm-backend-link-options)
- [MoonBit Component Model tutorial](https://docs.moonbitlang.com/en/latest/toolchain/wasm/component-model-tutorial.html)
- [OpenDAL 0.58.1 Wasm CI coverage](https://github.com/apache/opendal/blob/v0.58.1/.github/workflows/ci_core.yml#L262-L285)
- [OpenDAL OPFS Wasm example](https://github.com/apache/opendal/tree/v0.58.1/core/edge/opfs_wasm32)
- [OpenDAL S3 Wasm example](https://github.com/apache/opendal/tree/v0.58.1/core/edge/s3_read_on_wasm)
