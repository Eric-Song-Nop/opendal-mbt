# OpenDAL for MoonBit on WebAssembly

Status: proposed

Last reviewed: 2026-08-12

## Decision

Proceed with a WebAssembly product line for this binding.

The product goal is that a MoonBit consumer can depend on OpenDAL through an
ordinary MoonBit-facing package and build for the `wasm` target without knowing
that OpenDAL is implemented in Rust. The first implementation should keep the
Rust/OpenDAL and MoonBit outputs as separate WebAssembly compilation units and
connect them through a small, versioned interface. Packaging and composition
must hide that detail from downstream application code.

This is feasible, but it is not currently the same operation as importing a
normal MoonBit source package. As of Moon `0.1.20260807` and moonc
`0.10.7+bc794d341`, MoonBit package dependencies are compiler inputs that are
linked before the final `.wasm` is emitted. The package and linker configuration
does not accept an arbitrary precompiled Rust `.wasm` as a MoonBit library
dependency. MoonBit does support Wasm imports, exported memory, custom exports,
and the Component Model toolchain, so the missing part is an explicit
cross-module interface and a packaging/composition step—not WebAssembly support
itself.

This distinction gives us two requirements:

1. **Implementation requirement:** define how MoonBit calls OpenDAL across a
   Wasm boundary, including data, resource, error, and asynchronous ownership.
2. **Product requirement:** make the resulting integration feel like a normal
   MoonBit dependency rather than asking every consumer to write a loader or
   understand two Wasm heaps.

OPFS is not part of this feasibility decision. It is one possible storage
service after the language and module boundary works. The first proof uses
OpenDAL's in-memory service specifically to avoid confusing storage-runtime
questions with the core interoperability question.

## Why the direction is sound

Both sides have real Wasm support:

- MoonBit treats `wasm` and `wasm-gc` as first-class backends, supports Wasm
  imports and exports, and documents a WIT/Component Model workflow.
- OpenDAL `0.58.1` deliberately supports `wasm32`; its own CI builds several
  services for `wasm32-unknown-unknown`, and its core changes `Send`, time, and
  HTTP behavior for that architecture.
- OpenDAL ships browser-oriented Wasm examples for OPFS and S3. These prove
  that the Rust core can run in a browser Wasm environment, although they do
  not provide a reusable MoonBit binding.

The useful conclusion is therefore not merely "both compile to Wasm." It is:

> We can expose OpenDAL as a MoonBit-facing Wasm package once we supply the
> interface and composition layer that the two language toolchains do not
> infer automatically.

The relevant upstream references are:

- [MoonBit WebAssembly integration](https://docs.moonbitlang.com/en/latest/toolchain/wasm/index.html)
- [MoonBit Wasm FFI](https://docs.moonbitlang.com/en/latest/language/ffi.html)
- [MoonBit package Wasm link options](https://docs.moonbitlang.com/en/latest/toolchain/moon/package.html#wasm-backend-link-options)
- [MoonBit Component Model tutorial](https://docs.moonbitlang.com/en/latest/toolchain/wasm/component-model-tutorial.html)
- [OpenDAL 0.58.1 Wasm CI coverage](https://github.com/apache/opendal/blob/v0.58.1/.github/workflows/ci_core.yml#L262-L285)
- [OpenDAL OPFS Wasm example](https://github.com/apache/opendal/tree/v0.58.1/core/edge/opfs_wasm32)
- [OpenDAL S3 Wasm example](https://github.com/apache/opendal/tree/v0.58.1/core/edge/s3_read_on_wasm)

## What "a MoonBit package" should mean

There are three different integration levels that should not be conflated.

| Level | Meaning | Decision |
| --- | --- | --- |
| Moon source dependency | `moon add` imports MoonBit packages that the Moon compiler links into its final output | Keep this user experience for the MoonBit facade |
| Core Wasm module dependency | A Moon-produced module imports functions implemented by a Rust-produced module | Use this for the first interoperability proof |
| Wasm component dependency | WIT describes typed imports, exports, resources, and canonical data conversion | Evaluate as the preferred long-term packaging boundary |

The first level does not currently consume the second level automatically.
The repository must bridge that gap. A successful release may internally ship
two modules plus a loader, or one composed component; application code should
only see the safe MoonBit API.

Object-level static linking is not the initial path. The installed
`moonc link-core -help` accepts Moon compiler inputs and emits Wasm, but exposes
no Wasm object, archive, or precompiled-module input. Even if both compilers
could emit relocatable Wasm objects, their allocators, object layouts, runtime
initialization, and asynchronous executors would still need an agreed ABI.

## Current repository constraints

The existing implementation is intentionally native and cannot be retargeted
by changing one build flag:

- [`src/moon.pkg`](../../src/moon.pkg) declares only `native` support and
  unconditionally compiles `opendal_stub.c`.
- [`native/rust/Cargo.toml`](../../native/rust/Cargo.toml) emits a Rust
  `staticlib`; the profiles enable blocking/filesystem behavior and a Tokio
  multithreaded runtime.
- [`src/native_ffi.mbt`](../../src/native_ffi.mbt) defines public resource
  types alongside `extern "C"` declarations, so the public facade and native
  representation are not yet separable by target-gating one file.
- [`src/async.mbt`](../../src/async.mbt) and
  [`src/async_native_ffi.mbt`](../../src/async_native_ffi.mbt) use a POSIX
  file-descriptor completion protocol. A browser has neither that protocol nor
  the native Tokio runtime that drives it.
- [`build.js`](../../build.js) selects a host-native archive without first
  treating the requested Moon target as a build-graph decision. A Wasm package
  must not download or link a host static library.
- The current C stub translates between the stable project C ABI and MoonBit C
  runtime objects. Neither `moonbit.h` object layouts nor those native external
  objects are a Wasm interoperability ABI.

These constraints argue for an additive Wasm package and bridge first. They do
not argue against Wasm.

## Proposed architecture

### Consumer surface

Start with an experimental, async-first package such as
`Eric-Song-Nop/opendal/wasm`. Keep the existing
`Eric-Song-Nop/opendal` native package compatible while the Wasm contract is
being proven. After the Wasm surface is stable, decide whether target-specific
implementations can safely share one import path.

The Wasm facade should reuse backend-independent value semantics where they
are actually portable: errors, metadata snapshots, byte ranges, path
validation, and service configuration values. Resource handles and execution
must be backend-specific.

The public API must not expose:

- Rust pointers or Wasm offsets;
- `WebAssembly.Memory` management;
- loader callbacks;
- raw task handles;
- `wasm-bindgen` values;
- a requirement to understand which module owns an allocation.

### Initial module graph

```text
MoonBit application
  -> MoonBit OpenDAL Wasm facade
  -> versioned Wasm imports
  -> small host/composition adapter
  -> Rust OpenDAL Wasm bridge
  -> OpenDAL async core
```

The first implementation may produce two core Wasm modules and a small
JavaScript host adapter. This matches OpenDAL's currently demonstrated browser
target and lets each compiler retain its own runtime and memory. It also makes
data copies explicit, which is safer for a first ABI than pretending two
allocators share one heap.

The long-term shape to investigate is:

```text
MoonBit component
  -> WIT OpenDAL interface
  -> Rust OpenDAL component
  -> composed WebAssembly component
```

WIT is attractive because its canonical ABI defines string/list transfer and
resource ownership instead of making this repository invent those layouts.
However, MoonBit's documented component workflow and OpenDAL's demonstrated
browser `wasm32-unknown-unknown` workflow are not yet proof that the complete
OpenDAL async browser stack composes as a component. That is a milestone gate,
not an assumption.

### Boundary contract

Until a WIT component proves viable, the core-Wasm boundary should follow
these rules:

- Pass only fixed-width scalar values across direct Wasm calls.
- Represent operators, readers, writers, tasks, errors, and returned buffers as
  generation-checked integer handles owned by the Rust bridge.
- Copy strings and bytes through explicit length/query/copy operations. Do not
  depend on MoonBit `String`, `Bytes`, Rust `Vec`, or `wasm-bindgen` object
  layouts.
- Give every owned handle one explicit release operation. Release is
  idempotent at the safe MoonBit facade and checked at the bridge.
- Put a version and feature bitmap at the bootstrap boundary before adding
  optional operations.
- Bound every cross-boundary allocation and copy before materializing it in
  MoonBit.
- Translate errors into owned, immutable snapshots; never retain a borrowed
  pointer into another module's heap.
- Keep each module's allocator private. Shared linear memory is not the first
  milestone because `import-memory` does not make two allocators or object
  layouts compatible.

The existing native C ABI is useful as a semantic reference for handles,
version negotiation, bounded buffers, and explicit frees. Its C layouts and
MoonBit runtime adapter must not be reused as the Wasm ABI by accident.

### Asynchronous execution

Wasm support should be async-first. Blocking browser I/O would either freeze
the event loop or require thread and shared-memory assumptions that are not
part of the baseline browser platform.

The first async protocol must define all of the following before exposing a
public operation:

- how an OpenDAL future is scheduled on the host event loop;
- how completion wakes MoonBit without POSIX pipes;
- whether completion is delivered by polling, a host callback, or a Promise
  adapter;
- exactly-once result consumption;
- cancellation races and late completion;
- dropping a MoonBit resource while work is pending;
- re-entrancy and callback ordering;
- the behavior when the host tears down the Wasm instance.

MoonBit's experimental Wasm1 async support is relevant research, but it is not
yet a portability assumption for browser, Node, and arbitrary Wasm hosts. The
first proof may use a small explicit host scheduler while keeping task details
private behind the MoonBit facade.

## The first decisive proof

The first proof should answer one question:

> Can a downstream MoonBit `wasm` application call the Rust OpenDAL core
> through a packaged facade and complete a binary-safe in-memory round trip
> without application-owned glue?

Use the OpenDAL memory service and only these operations:

1. initialize/version-check the bridge;
2. create an operator;
3. write bytes containing NUL and non-UTF-8 values;
4. read the same path;
5. return an expected `NotFound` error for a missing path;
6. release all resources;
7. repeat the lifecycle to detect stale-handle reuse.

This proof deliberately excludes OPFS, S3, filesystem access, credentials,
CORS, and streaming. Passing it establishes the language boundary, byte ABI,
ownership, error mapping, and package/loader shape. Failing it tells us exactly
which toolchain or runtime contract is missing without service-specific noise.

Acceptance criteria:

- `moon build --target wasm` builds the MoonBit consumer facade.
- OpenDAL `0.58.1` is executed in Rust-produced Wasm rather than reimplemented
  in JavaScript or MoonBit.
- The consumer test imports only the public MoonBit package API; it contains no
  Rust module instantiation, pointer arithmetic, or handwritten import table.
- Binary data is round-tripped exactly and all ownership transitions are
  observable in bridge-level counters or a comparable leak oracle.
- A clean packaged-consumer fixture works outside the repository checkout.
- The artifact layout and composition command are deterministic and recorded.

## Delivery plan

### Milestone 0: Toolchain canary

Goal: prove the smallest cross-language call before introducing OpenDAL.

Deliverables:

- a tiny Rust Wasm module exporting a version function and one fallible
  handle-based byte echo operation;
- a MoonBit `wasm` package importing that interface;
- a repository-owned host adapter that instantiates and wires both modules;
- a downstream fixture that calls only the MoonBit facade;
- recorded sizes, imports, exports, memories, and required Wasm proposals.

Exit decision:

- continue with core-module composition if the loader can be packaged without
  consumer-written glue;
- prefer the WIT/component route if it already provides equivalent browser
  execution and simpler ownership;
- stop and raise a Moon toolchain requirement if neither route can deliver a
  dependency-quality consumer experience.

### Milestone 1: OpenDAL memory vertical slice

Goal: replace the byte-echo implementation with the actual OpenDAL memory
service.

Deliverables:

- a separate Rust Wasm bridge crate, built as the Wasm-appropriate library
  type with native-only dependencies disabled;
- versioned bootstrap, operator construction, `read`, `write`, `stat`, and
  release operations;
- a private one-shot host completion adapter sufficient to drive the OpenDAL
  futures used by this canary, without treating that adapter as the final
  public async contract;
- bounded byte transfer and immutable error/metadata snapshots;
- MoonBit value conversion and safe resource wrappers;
- the binary-safe packaged-consumer acceptance case above.

The milestone is not complete if the Rust bridge only compiles. The call must
originate in MoonBit and return through the same public facade. Milestone 1
proves the end-to-end path; Milestone 2 replaces the canary completion adapter
with a fully specified lifecycle that is safe under cancellation and drop.

### Milestone 2: Async lifecycle

Goal: make execution semantics suitable for real browser-backed services.

Deliverables:

- a host-event-loop scheduling protocol;
- pending, ready, cancelled, consumed, and released state definitions;
- exactly-once completion and result-take rules;
- cancellation and late-completion tests;
- async read/write through the safe MoonBit facade;
- no file descriptors, signals, blocking waits, or multithreaded Tokio runtime.

Do not add streaming until one-shot async lifecycle and cancellation are
settled.

### Milestone 3: Dependency and artifact experience

Goal: turn the prototype into something that deserves to be called a MoonBit
package.

Deliverables:

- a normal Moon dependency for the facade;
- a version-pinned Rust Wasm artifact with digest verification;
- one documented composition/bundle command owned by this repository;
- a clean consumer that does not require Rust or this source checkout;
- offline reuse after the artifact has been acquired once;
- explicit browser/runtime compatibility metadata;
- an independent Wasm artifact manifest rather than reuse of the native
  `artifacts-standard.json` trust chain.

The ideal endpoint is `moon add` plus the consumer's normal application build.
If current Moon package hooks cannot safely deliver the companion Wasm module,
the interim contract may include one project-owned bundling command, but not
consumer-authored JavaScript glue.

### Milestone 4: First real storage service

Goal: validate host I/O after the package boundary is stable.

Service order:

1. `memory`, already used by the interoperability proof;
2. OPFS for browser-local persistence;
3. S3-compatible storage using browser Fetch;
4. other services only after their target and credential contracts are
   demonstrated;
5. WASI as a separate runtime investigation, not an alias for browser Wasm.

OPFS is valuable here because it tests real asynchronous browser storage. It
is not evidence for or against the underlying MoonBit/OpenDAL package design.

S3 acceptance must cover CORS, browser-forbidden headers, mixed-content rules,
credential exposure, and cancellation. Long-lived cloud credentials must not
become the default browser configuration; temporary credentials, presigned
requests, or a same-origin broker need an explicit security contract.

### Milestone 5: API convergence and release

Goal: decide how the Wasm and native products converge.

Deliverables:

- comparison of generated public interfaces for shared value semantics;
- an explicit support matrix for native, browser Wasm, Node, and WASI;
- a decision on one import path versus an explicit `/wasm` package;
- size and startup budgets for the standard Wasm artifact;
- release provenance, checksums, compatibility floors, and a fresh packaged
  consumer from the released Moon package;
- documentation that labels unsupported native-only operations rather than
  silently emulating them.

## Main difficulties and how to retire them

| Difficulty | Why it matters | Evidence that retires the risk |
| --- | --- | --- |
| Moon package versus precompiled Wasm module | `moon add` does not itself link an arbitrary Rust `.wasm` | Clean downstream fixture with repository-owned automatic wiring |
| Two runtime memories and allocators | Matching `wasm32` targets do not imply matching heaps or object layouts | Binary and error round trips under explicit copy/ownership instrumentation |
| Async host integration | Current native completion depends on POSIX and Tokio threads | Browser event-loop tests for success, cancellation, drop, and late completion |
| Component Model maturity for this stack | WIT is promising, but OpenDAL's proven browser target is core Wasm | A composed MoonBit/Rust OpenDAL component running the same memory acceptance case |
| API shape | Existing synchronous native facade has no honest browser equivalent | Async-first Wasm API reviewed independently, followed by an explicit convergence decision |
| Distribution | A working checkout is not a package | Fresh consumer with no Rust toolchain, source checkout, or handwritten loader |
| Service portability | OpenDAL service support varies by target and host APIs | Per-service browser/WASI build and runtime evidence; no blanket support claim |
| Binary size and startup | OpenDAL features and Rust glue can make browser artifacts expensive | Recorded size/startup budgets and feature-minimal builds before release |
| Security | Browser credentials and cross-origin requests change the threat model | Service-specific credential and CORS contract with redaction tests |

## Work that can be reused

The native implementation already contains reusable semantics, even though its
FFI is not reusable as Wasm binary layout:

- typed errors and owned metadata values;
- byte-range validation and bounded output rules;
- S3 configuration validation;
- explicit Writer/Reader/resource lifecycle ideas;
- ABI version negotiation and feature groups;
- immutable snapshots and paired release operations;
- compile-contract fixtures and the generated public interface as comparison
  inputs.

Before adding a Wasm backend to the existing facade, move portable values and
validation away from files that also define native resource representations.
The native adapter should continue to pass its existing contract while the
Wasm adapter evolves independently.

## Explicit non-goals for the first release

- Reusing the current C static library or `moonbit.h` stub inside Wasm.
- Making the existing blocking API appear to work in a browser.
- Supporting `wasm-gc` before the linear-memory integration works.
- Claiming all OpenDAL services support browsers or WASI.
- Sharing one linear-memory allocator between Rust and MoonBit as an
  optimization before the copied ABI is correct.
- Requiring downstream users to hand-maintain a JavaScript import table.
- Shipping OPFS or S3 before the memory-service package proof succeeds.
- Folding Wasm artifacts into the native artifact manifest or release gates.

## Immediate implementation backlog

The first implementation PR after this design should remain intentionally
small:

1. make `build.js` target-aware so non-native targets never resolve a host
   native archive;
2. add an unpublished `wasm` facade package and downstream fixture;
3. add the Rust byte-echo canary and repository-owned loader;
4. prove binary transfer, handle release, stale-handle rejection, and error
   transfer;
5. replace the canary internals with OpenDAL memory `read`/`write` only;
6. record whether core-module composition or WIT gives the better package
   boundary before expanding the public API.

That sequence finds the decisive failures early. It also preserves the native
package while making measurable progress toward the intended result: OpenDAL
as a usable MoonBit package for the `wasm` target.
