# Mooncake delivery plan for OpenDAL WebAssembly

Status: backend-neutral callback binding, M0–M2 repository evidence, and a
static browser-memory contract are implemented; Mooncake distribution is not
implemented and no Wasm release has been published

Last reviewed: 2026-08-13

Related design: [`wasm-integration.md`](wasm-integration.md)

## Product outcome

The WebAssembly binding becomes a product only when a MoonBit user can install
it from Mooncakes, import a MoonBit package, and produce a deployable browser
application without installing Rust, cloning this repository, writing an
import table, or locating a compiler cache by hand.

The preferred stable experience is:

```sh
moon add Eric-Song-Nop/opendal@<binding-version>
moon build --target wasm --release
```

Application code imports the explicit Wasm facade:

```moonbit
import {
  "Eric-Song-Nop/opendal/wasm" @opendal
}
```

The final output contains the MoonBit application, the matching Rust OpenDAL
bridge, wasm-bindgen glue, the official loader, and a machine-readable asset
manifest. Application code sees MoonBit operators, callbacks, values, and
errors. It does not see Rust, bridge handles, pointer offsets, an import table,
or a second `WebAssembly.Instance`.

Current Moon does not propagate arbitrary runtime assets from a library
dependency into the final application output. The first useful preview may
therefore use one additional repository-owned, versioned command:

```sh
moon add Eric-Song-Nop/opendal@<binding-version>

moonx Eric-Song-Nop/opendal/cmd/wasm-bundle@<binding-version> \
  --package cmd/app \
  --profile browser-memory-preview \
  --entry _start \
  --out dist
```

The bundler would be a precompiled executable from the same Mooncake version.
It would build the selected consumer target, acquire or materialize only the
manifest-pinned runtime files, verify them, and write a relocatable deployment
directory. It must not require Cargo, rustup, npm, this checkout, or
application-authored JavaScript.

This `moonx` step is a preview compatibility bridge. It is not implemented
today, and it is removed when Moon can propagate target-scoped dependency
runtime assets during an ordinary build.

## Release definitions

| Level | User experience | Allowed claim |
| --- | --- | --- |
| Source install | `moon add` resolves the module and `/wasm` type-checks | Not a Wasm release |
| Repository candidate | A checkout builds two modules, runs static/Node/Chrome checks, and wires them with the loader | Engineering evidence only; current state |
| Mooncake preview | `moon add` plus the exact-version official `moonx` bundler works without Rust/npm/checkout | First publishable preview |
| Native-like distribution | `moon add` plus ordinary `moon build --target wasm` materializes all runtime assets | Preferred stable build experience |
| Stable browser API | Backend-neutral public API, reviewed callback/continuation decision, lifecycle safety, and bounded transfer | First stable Wasm product claim |

A facade in a source archive is insufficient. The bridge, glue, loader, asset
manifest, and compatibility checks are part of the package contract.

## Scope and backend neutrality

The initial target is:

- MoonBit linear-memory `wasm`, not `wasm-gc`;
- a browser `Window` for the current acceptance profile, with secure-context
  requirements determined by each compiled service;
- an experimental callback-only MoonBit API;
- `Operator::new(scheme, config)` backed by OpenDAL's compiled service
  registry;
- service-minimal artifacts with exact compiled schemes;
- two independent core Wasm modules connected by the official loader;
- the existing native package remaining source- and ABI-compatible.

Node remains primarily a test host for this browser-oriented artifact; native
Node applications continue to have the native binding. Worker, WASI, and
`wasm-gc` support require separate runtime and artifact contracts.

No backend is the integration mechanism or a privileged product path.
`memory` is the deterministic common-contract fixture. OPFS, an HTTP object
service, or another browser-compatible OpenDAL service can be added to an
artifact and tested through the same constructor and callback operations.
Each fixture proves only its own host-specific behavior.

## Decisions already made

1. Keep module `Eric-Song-Nop/opendal` and add the `/wasm` package.
2. Keep the native root package unchanged; do not hide browser execution
   behind its blocking facade.
3. Keep Rust/OpenDAL and MoonBit as separate Wasm modules for the first
   delivery path, with separate memories and allocators.
4. Cross the boundary with versioned scalar/resource/task handles and explicit
   bounded copies.
5. Use a repository-owned loader for instantiation, scheduling, copying, and
   teardown. Application code never constructs the import object.
6. Keep WIT/component composition off the critical path until it improves a
   proven core-module product.
7. Give Wasm artifacts a trust chain independent from native static-library
   manifests.
8. Use one generic scheme/config constructor for all compiled services.
9. Keep raw synchronous poll-once and canary diagnostics out of the public
   MoonBit interface.

## Current implementation state

The repository candidate has this shape:

```text
MoonBit application Wasm
  -> Eric-Song-Nop/opendal/wasm callback facade
  -> scalar imports
  -> repository-owned loader
  -> Rust OpenDAL bridge Wasm
  -> OpenDAL service selected from the compiled registry
```

The current artifact compiles the memory fixture, but construction already
uses the generic registry. The public MoonBit facade exposes:

- `available_schemes()`;
- `Operator::new(scheme, config)`, `Operator::info()`, and
  `Operator::as_async()`;
- `AsyncOperator` callback methods for create, write, read, stat, bounded
  list, and delete operations;
- `Task` states `Pending`, `Completed`, `Cancelled`, and `Closed`;
- logical cancellation, explicit close, owned errors, metadata, entries, and
  capabilities.

It exposes no synchronous storage method, `Operator::memory`, raw bridge or
task handle, ABI/feature accessor, leak counter, or forced-pending diagnostic.

The bridge ABI is `0x0001_0005`. The bridge reports `0x0000_03ff`: bits 0 and
1 are memory and poll-once fixture capabilities, followed by generation
handles, binary buffers, task ABI, generic operator construction, common
mutations, bounded list, bulk transfer, and structured errors. The public
facade requires `0x0000_03fc`; it is not coupled to the memory or poll-once
fixture bits.

Production task starts use `wasm_bindgen_futures::spawn_local` and await the
OpenDAL future directly. A raw bridge-only test switch can wrap tasks started
afterward in a zero-delay future to force an observed first `Pending` poll. It
is disabled by default and enabled explicitly by the Chrome canary.

### Current acceptance commands

```sh
# Exact module boundary, memory shape, and size ceilings.
make wasm-static-contract

# Callback tasks, bulk transfer, and two lifecycle rounds in Node.js.
make wasm-canary

# Explicit forced-Pending lifecycle and race matrix in Chrome/Chromium.
make wasm-browser-canary
```

The static gate checks the committed browser-memory snapshot for:

- exact Rust and MoonBit imports and exports;
- exactly one defined and exported memory per module, with no memory import;
- no shared memory, memory64, `env`, or WASI dependency;
- every MoonBit `opendal_mbt_bridge` import resolving to a Rust export of the
  same kind;
- raw, gzip, and Brotli size ceilings for the bridge, glue, and MoonBit
  canary.

The Node canary constructs the operator through the public generic facade,
exercises bounded bulk transfer, completes the callback storage lifecycle
twice, checks owned success/error results and cleanup, and tears the instance
down. It uses real tasks and callbacks, not the public synchronous methods
that no longer exist.

The Chrome canary explicitly enables the forced-pending hook. It checks twelve
observed pending tasks across the main lifecycle, completion-wins case,
concurrent operators, and pending disposal. It also checks browser heartbeat
ordering, cancel-before-ready suppression, terminal close, scheduler
diagnostic isolation, and inert late completion after teardown.

Only Chrome is evidence for forced `Pending -> Ready` and browser event-loop
responsiveness. Node validates the task and callback path but does not enable
the forced-pending hook or make a browser-heartbeat claim.

### Honest boundary of the evidence

The repository has M0–M2 implementation evidence and a static browser-memory
contract. It does **not** yet have:

- a Mooncake preview or release;
- a versioned distribution manifest with immutable artifact provenance;
- automatic runtime-asset acquisition or embedding;
- a clean registry consumer;
- cold/hot cache, relocation, corruption, or mismatch evidence;
- startup budgets;
- a tested non-memory service profile;
- stable ordinary-browser MoonBit `async fn` suspension.

The static snapshot is a compatibility gate for the current artifacts, not a
release manifest or a claim that every OpenDAL backend is compiled or runnable
in browsers.

## Consumer contracts

### Application-code contract

Current application code uses the backend-neutral callback facade:

```moonbit
let operator = @opendal.Operator::new(
  "memory",
  config={ "root": "notes" },
)

let async_operator = operator.as_async()
let task = async_operator.read_callback(
  "hello.txt",
  callback=result => {
    match result {
      Ok(bytes) => consume(bytes)
      Err(error) => report(error)
    }
  },
)
```

`create_dir_callback`, `write_callback`, `stat_callback`, `list_callback`, and
`delete_callback` follow the same lifecycle. `Task::cancel()` and
`Task::close()` are idempotent. Cancellation suppresses callback delivery
and discards a late result; it does not claim to abort the underlying OpenDAL
future or browser API.

`AsyncOperator::close()` is a Wasm lifecycle extension that releases the
shared operator when only the async view is retained. Closing either view is
idempotent and makes both views unusable for new work.

The package does not offer synchronous write/read/stat fallbacks. A future
native-looking `async fn` facade requires a documented ordinary-browser
MoonBit continuation contract. Until that exists, the callback API is marked
experimental rather than renamed as async/await.

Operator configuration is bounded to 1,024 entries and 1 MiB of combined key
and value UTF-8. ASCII-case-insensitive duplicate keys are rejected before
construction, so configuration failure does not publish a partial operator.

### Preview distribution contract

A preview may use the official bundler only if all of these hold:

- one `moon add` declares the exact facade dependency;
- the exact-version bundler is acquired as a precompiled Mooncake executable,
  not compiled from this module's source by the consumer;
- bootstrap validates facade/bridge compatibility, ABI, required feature
  bits, OpenDAL version, profile, and manifest digest before construction;
- application code imports only `Eric-Song-Nop/opendal/wasm`;
- application code supplies no loader, promise adapter, pointer arithmetic, or
  import table;
- no Rust tool or source is downloaded;
- cold acquisition verifies sizes and hashes before publishing output;
- warm builds work offline from a verified content-addressed cache;
- corrupt or mismatched entries fail closed and are quarantined;
- output is relocatable and uses relative URLs.

An illustrative preview output is:

```text
dist/
  app.wasm
  bootstrap.mjs
  opendal-loader.mjs
  opendal-mbt.assets.json
  assets/
    Eric-Song-Nop/
      opendal/
        bridge.wasm
        bridge.mjs
```

`bootstrap.mjs` loads the manifest, verifies the runtime files, instantiates
the bridge and application through the official loader, and invokes the
configured application export. Consumers do not reconstruct the graph.

### Stable distribution contract

The preferred stable build removes the explicit bundler:

```sh
moon add Eric-Song-Nop/opendal@<binding-version>
moon build --target wasm --release
```

Moon or a Moon-supported bundling mode materializes the reachable dependency's
runtime assets beside the application. Browser startup never searches
`$HOME`, `$MOON_HOME`, `.mooncakes`, or another build-machine cache path.

## Runtime contract

### Task lifecycle

Each operation copies its input into Rust-owned storage and clones the
operator before task start returns. The loader starts observation in a
microtask and moves repeated pending polls to later zero-delay timer turns, so
the callback never re-enters the initiating MoonBit/Rust stack.

```text
task: Pending -> Ready(Completion) -> Consumed -> released
task: Pending or Ready -> Cancelled -> released
MoonBit Task: Pending -> Completed | Cancelled -> Closed
```

The current contract guarantees:

- at-most-once completion publication and result taking;
- per-completion OpenDAL errors;
- wait deregistration before task cancel/release;
- logical callback suppression after cancellation;
- an in-flight operator clone independent of the closed MoonBit wrapper;
- idempotent runtime disposal and inert late completion;
- generation checks and explicit release for every bridge resource.

It does not yet provide underlying host-promise abort, streaming readers or
writers, backpressure, or portable MoonBit `async fn` suspension.

### Bulk transfer

Rust and MoonBit export independent memories. The loader sees both and copies
at most 256 KiB per host call. It obtains a checked bridge window, recreates
both typed-array views, performs one immediate synchronous copy without an
intervening bridge call, and then discards the views.

Whole-object materialization is capped at 64 MiB. Offset-plus-length arithmetic
is checked on both sides, allocation failure cannot publish a partial handle,
and call count grows with chunks rather than bytes. The 16 MiB canary exercises
non-zero MoonBit slices and memory growth in both modules. Raw per-byte bridge
exports remain low-level fixture oracles and are not imported by the facade.

### Lists

`list_callback` streams the OpenDAL lister into owned snapshots before
publishing a completion. It fails atomically above 65,536 entries or 16 MiB of
combined path/name UTF-8. Optional `limit` and `start_after` values are sent to
the selected backend; they do not weaken the binding's total materialization
limits.

## Artifact and trust contract

### Profiles

Profiles are packaging choices, not distinct MoonBit APIs:

| Profile | Example compiled services | Purpose |
| --- | --- | --- |
| `browser-memory-canary` | memory | Current repository lifecycle, transfer, and static-contract fixture |
| `browser-memory-preview` | memory | First candidate for distribution work |
| `browser-local-fixtures` | memory plus selected local browser services | Optional persistence/permission fixtures |
| `browser-object-fixtures` | memory plus selected HTTP object services | CORS, credential, timeout, and redaction fixtures |
| `browser-standard` | measured, selected browser-compatible services | Consider only after size/startup and security review |

One all-services artifact is not assumed. OpenDAL service code, target
compatibility, browser APIs, size, startup, and attack surface differ. The
manifest declares exact compiled schemes and the public facade reports them.

### Manifest

Wasm receives an independent versioned manifest. It records at least:

- binding, bridge ABI, OpenDAL, Rust, Moon, wasm-bindgen, and optimization
  versions;
- runtime kind and service-profile identifier;
- exact registered schemes and required facade feature bits;
- bridge Wasm, glue, loader, license, and optional bootstrap logical paths;
- exact release URL or embedded package path, byte size, and SHA-256 for every
  runtime file;
- required WebAssembly proposals and minimum browser/Moon versions;
- the committed static-contract digest and measured size/startup budgets.

The existing static `browser-memory.json` snapshot is not this manifest. It is
source-controlled ABI/shape evidence used while the distribution schema is
still pending.

### Canonical build and release

Canonical artifacts use pinned toolchains, `cargo build --locked`,
`wasm32-unknown-unknown`, panic abort, exact wasm-bindgen processing,
feature-minimal service profiles, deterministic paths/timestamps, import/export
inspection, per-file hashes, and a dependency/SBOM record.

Tag release ordering is:

1. build candidate profile artifacts;
2. run the static contract and record candidate imports, exports, sizes, and
   startup measurements;
3. verify the exact candidate in a clean packaged consumer;
4. publish immutable GitHub Release assets;
5. publish the Mooncake containing the matching facade, loader, manifest, and
   embedded files or exact acquisition records;
6. install from the registry in a fresh consumer;
7. verify cold acquisition and offline hot-cache reuse.

The Moon package is never published before all externally referenced companion
files exist at their immutable URLs. There is no fallback from a pinned
version to `latest`.

### Cache and bundler authority

If a preview bundler fetches assets, it uses a content-addressed cache outside
Moon's opaque dependency-source cache. It verifies exact versioned URLs,
sizes, and SHA-256; stages unique downloads; publishes atomically under a
per-digest lock; reuses a verified cache offline; and rejects or quarantines
corruption.

The bundler can launch the Moon compiler and write deployment files, so it is
a trusted build-tool action, not a sandbox boundary. It must invoke fixed
child arguments without a shell, restrict writes to selected build/cache/output
roots, and fetch only manifest-pinned URLs. Documentation must state that
authority honestly.

## Service fixtures

The generic binding has three independent truths:

1. the facade accepts `scheme` and `config`;
2. the selected artifact compiled a scheme;
3. the host and service satisfy their runtime/security requirements.

Adding a service changes the artifact profile and its tests, not the public
constructor. Capability values are queried from the actual operator.

Useful fixture categories include:

- memory: deterministic task lifecycle, bulk bytes, error ownership,
  cancellation, teardown, and packaging;
- a browser-local persistence service such as OPFS: secure-context/window
  requirements, reload persistence, origin isolation, quota, and permissions;
- an HTTP object service such as S3: CORS/preflight, HTTPS/mixed content,
  browser-forbidden headers, redirects, temporary credentials or a broker,
  timeout, cancellation, and secret redaction.

OPFS is merely one possible optional fixture. It is not M4 by itself, does not
receive a special API, and does not prove network services, Workers, Node, or
WASI. Similarly, an S3 fixture proves only its selected transport and security
profile.

## Moon toolchain collaboration

Two upstream capabilities would turn the preview flow into the preferred
stable flow.

### Target-aware module prebuild

The module prebuild should receive documented target kind, profile, mode,
host/target triple, and a real isolated output directory. It should let the
native resolver return an empty link configuration for `wasm` without an
environment variable or native artifact lookup. Existing scripts that read
only current inputs must remain compatible.

### Declarative dependency runtime assets

Moon needs target-scoped runtime assets for reachable library dependencies:

- target/profile conditions;
- dependency-relative or generated inputs;
- collision-safe final paths;
- deterministic materialization in the build graph;
- an application asset manifest with relative URL, version, size, digest, and
  compatibility metadata;
- watch/cache inputs and conflict diagnostics;
- no native archives in Wasm builds and no Wasm bridge in native builds;
- no arbitrary transitive dependency shell hook.

Declarative copying and manifests should come before a generic dependency
post-link command. Component composition can be evaluated later as an explicit
root action.

## Milestones

### M0 — Core-module interoperability: implemented

Implemented evidence includes the MoonBit facade, Rust bridge, official loader,
generic memory operator, version negotiation, owned binary/error values,
generation handles, two callback lifecycle rounds in Node, teardown, and the
committed static module contract.

M0 proves the language/module boundary. It does not prove a registry consumer
or browser scheduling.

### M1 — Callback task engine: implementation complete; public API experimental

Implemented evidence includes task/completion ownership, later-turn callback
delivery, per-completion errors, logical cancellation, create-dir/write/read/
stat/list/delete, ready-versus-cancel, concurrent operators, close/teardown
while pending, double-take/release state tests, and inert late completion.

Chrome explicitly checks twelve forced-pending tasks and browser heartbeat
ordering. The remaining M1 product decision is whether a documented MoonBit
ordinary-browser continuation permits a stable `async fn` facade. Until then,
callbacks remain experimental.

### M2 — Bounded bulk binary ABI: implemented

Implemented evidence includes 256 KiB windows, a 64 MiB materialization cap,
fresh typed-array views across memory growth, exact 16 MiB round-trip,
chunk-proportional host calls, non-zero slices, allocation-limit failure, and
no per-byte imports in the public facade.

Streaming and backpressure are later work; M2 covers bounded whole objects.

### M3 — Mooncake preview distribution: pending

Deliverables:

- versioned artifact/profile manifest and canonical release archive;
- exact-version precompiled `moonx` bundler if ordinary runtime-asset
  propagation remains unavailable;
- verified content-addressed cache and relocatable output;
- clean consumer outside this checkout;
- mismatch, corruption, cold, hot-offline, and interrupted-acquisition tests;
- preview documentation that clearly labels the callback API experimental.

This is the first publishable preview. It may initially use only the memory
artifact because the product being tested is distribution, not a special
storage backend.

### M4 — Additional service profiles: pending

The generic constructor, available-scheme query, operator info, capabilities,
and common callback operations already exist. M4 packages selected additional
browser-compatible services and proves each service's relevant host behavior
without changing the public API.

Exit requires exact compiled schemes, correct capabilities, owned failures,
responsive browser execution, leak-free teardown, and the same clean Mooncake
consumer path. No one fixture stands in for all OpenDAL services.

### M5 — Stable distribution and API decision: pending

Deliverables include the callback-versus-`async fn` decision, target-aware
resolver behavior without a user environment variable, ordinary dependency
runtime assets or a formally retained bundler, compatibility policy, browser
support matrix, size/startup budgets, provenance, and fresh-registry
verification.

The preferred exit is `moon add` plus ordinary `moon build --target wasm`
producing a complete relocatable deployment. A high-quality `moonx` preview
must not be called identical to native-like distribution.

### M6 — Network service profile: optional later work

A selected HTTP object-service artifact needs a typed and reviewed browser
configuration story, CORS/security requirements, temporary-credential or
broker guidance, timeout/cancellation cases, and secret redaction. Success in
one browser profile does not imply Node, Worker, or WASI support.

### M7 — Streaming and component evaluation: optional later work

Bounded Reader/Writer resources require backpressure, abort, close, and partial
transfer semantics. WIT replaces the core-module loader only if it simplifies
application and packaging behavior while retaining the proven lifecycle,
transfer, and service behavior.

## Validation matrix

| Layer | Current or required evidence |
| --- | --- |
| Static module contract | Current: exact imports/exports, independent memories, safe imports, import resolution, and size ceilings |
| Rust state model | Current: task/cancel/late-completion/resource unit cases |
| Moon facade | Current: generic constructor, callback operations, value/error conversion, generated public interface |
| Node runtime | Current: callback tasks, bulk transfer, two lifecycle rounds, cleanup, teardown |
| Browser runtime | Current: twelve explicitly forced pending tasks, heartbeat, races/concurrency, pending teardown |
| Binary transfer | Current: NUL/non-UTF-8, 16 MiB chunks, limits, non-zero slice, memory growth |
| Service profiles | Pending beyond memory: exact schemes plus service-specific runtime/security cases |
| Packaging | Pending: clean Mooncake consumer with no Rust/npm/checkout and relocatable output |
| Integrity | Pending: wrong hash/size/version/ABI, corrupt cache, interrupted acquisition |
| Release | Pending: canonical candidate, exact pins, registry install, offline hot cache |

## Risk register

| Risk | Impact | Retirement evidence |
| --- | --- | --- |
| Moon browser async remains host-specific | Cannot offer native-looking stable `async fn` | Documented browser continuation or explicit stable callback decision |
| Runtime assets are not propagated | Ordinary build omits the bridge | M3 official bundler, then declarative asset support or formally retained bundler |
| Bundler has build-machine authority | Compromised helper could exceed intended work | Exact precompiled version, fixed argv without shell, path containment, pinned URLs, disclosed trust boundary |
| wasm-bindgen glue drifts | Loader/bridge mismatch | Exact tool version and one hashed artifact set |
| Two memories add copy cost | Poor large-object throughput | M2 limits and measurements, then streaming/backpressure |
| Cancellation cannot abort host work | Wasted I/O after logical cancel | Service-specific abort probe; retain logical-only claim |
| Generic constructor is mistaken for universal support | Unsupported service/runtime combinations | Exact compiled-scheme manifest and per-service host matrix |
| Browser credentials leak | Security incident | Temporary credentials/presign/broker and redaction tests |
| Service profiles grow artifact size | Slow download/startup | Per-profile static ceilings and runtime startup budgets |
| Static snapshot is mistaken for release trust | False provenance claim | Separate signed or digest-pinned release manifest and clean registry verification |
| WIT expands scope prematurely | Delays usable delivery | Keep component work after M3–M5 unless it removes a measured blocker |

## Immediate next actions

1. Treat the frozen callback-only `.mbti` and committed static contract as
   review gates; update either only with an intentional compatibility review.
2. Define the independent Wasm artifact/profile manifest, including exact
   registered schemes, compatibility, provenance, and startup evidence.
3. Implement the versioned official bundler or the smallest supported runtime-
   asset path, then test a clean consumer without Rust/npm/checkout.
4. Add cold, hot-offline, corruption, mismatch, relocation, and interrupted-
   acquisition tests.
5. Continue the Moon continuation investigation while leaving callbacks
   explicitly experimental.
6. Add optional non-memory service fixtures only through the same generic
   constructor and only after the common distribution gate is usable.

This ordering keeps the actual product criterion in view: a Mooncake-installed,
backend-neutral OpenDAL binding for browser Wasm. It does not elevate one
storage fixture into the architecture.
