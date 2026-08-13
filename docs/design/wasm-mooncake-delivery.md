# Mooncake delivery plan for OpenDAL WebAssembly

Status: proposed execution plan

Last reviewed: 2026-08-13

Related design: [`wasm-integration.md`](wasm-integration.md)

Implementation canary: [PR #59](https://github.com/Eric-Song-Nop/opendal-mbt/pull/59)

## Product outcome

The WebAssembly binding is a product only when a MoonBit user can install it
from Mooncakes, import a MoonBit package, and produce a deployable browser
application without installing Rust, cloning this repository, writing an
import table, or locating a compiler cache by hand.

The final user experience is:

```sh
moon add Eric-Song-Nop/opendal@<binding-version>
moon build --target wasm --release
```

The application imports the explicit Wasm facade:

```moonbit
import {
  "Eric-Song-Nop/opendal/wasm" @opendal
}
```

The build output contains the MoonBit application module, the matching
OpenDAL bridge, the official loader, and a machine-readable asset manifest.
Application code sees MoonBit operators, values, errors, and async operations;
it does not see Rust, `wasm-bindgen`, JavaScript promises, pointers, or a
second WebAssembly instance.

Current Moon does not yet propagate runtime assets from a library dependency
to the final application output. The first useful preview therefore has one
additional repository-owned command:

```sh
moon add Eric-Song-Nop/opendal@<binding-version>

moonx Eric-Song-Nop/opendal/cmd/wasm-bundle@<binding-version> \
  --package cmd/app \
  --profile browser-memory-preview \
  --entry _start \
  --out dist
```

This remains a Mooncake-only installation flow. The bundler is an executable
package from the same versioned Moon module. `moonx` is supplied by the Moon
toolchain, not installed by `moon add`. For the preview, Mooncakes must publish
the bundler itself as a precompiled linear-Wasm executable asset. `moonx`
downloads and verifies that exact executable instead of compiling its source,
which avoids trying to bootstrap through this module's unconditional native
prebuild.

The bundler builds the consumer with a fixed child command equivalent to
`moon build --target wasm --release --frozen --target-dir <isolated-dir>` for
the selected package, sets the internal native-resolver opt-out for that child
only, obtains the exact companion bridge, verifies it, and writes a deployable
directory. It requires the Moon toolchain, a compatible `moonrun`, and the
package's existing Node.js prebuild prerequisite, but not Cargo, rustup, npm,
a source checkout, or user-authored JavaScript glue.

Running a bundler that can start the Moon compiler and write deployment files
is a trusted build-tool action, not a sandbox boundary. The implementation
must pass child arguments without a shell, restrict writes to the selected
target/cache/output roots, and fetch only URLs pinned by the versioned release
manifest. If release-manifest signing is not yet available, the exact version,
size, and SHA-256 pins are the minimum trust boundary. If `moonx` later exposes
a usable least-privilege `moonrun` policy,
the release should publish one; until then the documentation must not claim
that the bundler is sandboxed.

The extra `moonx` step is an explicit compatibility bridge, not the desired
permanent UX. It is removed when Moon can propagate target-scoped runtime
assets during an ordinary build.

## Release definitions

The words "installable", "preview", and "stable" have precise meanings in
this plan.

| Level | User experience | Release claim |
| --- | --- | --- |
| Source install | `moon add` resolves the module and `/wasm` type-checks | Not sufficient for a Wasm release |
| Repository canary | A checkout builds two modules and the loader wires them | Engineering proof only |
| Mooncake preview | `moon add` plus the precompiled official `moonx` bundler works without Rust/npm/source checkout | First publishable preview |
| Native-like build | `moon add` plus ordinary `moon build --target wasm` materializes all runtime assets | Stable distribution target |
| Stable browser API | The facade is async, cancellation-safe, bulk-transfer capable, and supports browser-local persistence | First stable Wasm product |

A release must not be described as Mooncake-installable merely because the
MoonBit facade is present in the source archive. The companion Rust module and
the runtime adapter are part of the package contract.

## Scope decision

The first stable target is:

- MoonBit linear-memory `wasm`, not `wasm-gc`;
- a secure-context browser `Window`;
- an async-first MoonBit API;
- OpenDAL `memory` and OPFS through a `browser-local` artifact profile;
- two core Wasm modules connected by an official loader;
- the existing native package remaining source- and ABI-compatible.

The following are separate product tracks, not aliases for this target:

- Node continues to use the native binding;
- Browser Worker support waits for an OpenDAL OPFS path that does not require
  `window()`;
- WASI needs its own target, network/filesystem contract, and artifact;
- `wasm-gc` needs a separate ABI and runtime investigation;
- browser S3 is an opt-in profile after the async and security contracts are
  proven.

OPFS is the first useful storage backend, but it is not the interoperability
mechanism. The implementation must first make an actually pending OpenDAL
future complete safely through the browser event loop.

## Decisions already made

1. Keep the module name `Eric-Song-Nop/opendal` and add the `/wasm` package.
   Do not create a second public module unless the current module-level
   prebuild limitation remains unresolved for an extended period.
2. Keep the native root package unchanged. Browser semantics are async and
   must not be hidden behind the existing blocking facade.
3. Keep Rust/OpenDAL and MoonBit as separate Wasm compilation units for the
   first release. Each runtime owns its own memory and allocator.
4. Cross the direct module boundary with versioned scalar/resource/task
   handles. Copy values explicitly; never share MoonBit or Rust object layouts.
5. Use an official host adapter for instantiation, scheduling, and bulk copy.
   Downstream application code never constructs the import object.
6. Treat WIT and the Component Model as a later simplification experiment,
   not as a prerequisite for OPFS.
7. Publish Wasm artifacts through a trust chain independent of the native
   static-library manifests.
8. Make `memory` the async and distribution canary, OPFS the first real
   browser service, and S3 the first browser network service.

## Current implementation state

The implementation in PR #59 already establishes the correct basic module
shape:

```text
MoonBit application Wasm
  -> Eric-Song-Nop/opendal/wasm facade
  -> scalar imports
  -> repository-owned loader
  -> Rust OpenDAL bridge Wasm
  -> OpenDAL memory service
```

It includes ABI/feature negotiation, generation-checked handles, binary-safe
memory read/write, metadata, owned errors, explicit release, and a leak oracle.
That is enough to prove that MoonBit and Rust do not need a shared allocator
or a single compiler.

The current code remains a canary for four reasons:

- OpenDAL futures are polled once with a no-op waker; `Pending` is an error.
- MoonBit read/write/stat are synchronous, so real browser I/O cannot suspend.
- bytes cross the boundary one byte per Wasm call.
- the Mooncake archive does not yet contain or automatically acquire the
  version-matched Rust bridge.

The canary code is present, but runtime execution evidence remains a milestone
gate. A compiler check or a successful Rust target build alone does not close
that gate.

## Why Mooncake installation needs an asset plan

Moon package imports and precompiled Wasm modules are different kinds of
dependencies.

Mooncake can include arbitrary non-ignored files such as `.wasm`, `.mjs`, and
JSON in a published source archive. The official module configuration
documents publishing filters for the whole archive, and `moon package --list`
shows the result:

- [Moon module publishing files](https://docs.moonbitlang.com/en/latest/toolchain/moon/module.html#include-and-exclude)
- [Moon package management](https://docs.moonbitlang.com/en/latest/toolchain/moon/package-manage-tour.html)

However, merely installing those files does not make them final application
outputs:

- a dependency source directory is an implementation detail of Moon's cache;
- the cache location can change or be disabled;
- a browser cannot fetch an absolute build-machine cache path;
- the Wasm linker has no precompiled-module input for an arbitrary Rust
  `.wasm`;
- package `dev_build` commands do not run for downstream dependencies for
  security reasons;
- the experimental module prebuild output supports variables and native-style
  link configuration, not runtime assets.

The pinned Moon `0.1.20260807` implementation exposes only `env` and `paths` to
the module prebuild. The target field is still disconnected and `out_dir` is
the literal `TODO`:

- [Build-script input and output types](https://github.com/moonbitlang/moon/blob/4da23f805e562bcdb20a45764f1ab12cb892bf1d/crates/moonutil/src/build_script.rs#L25-L92)
- [Actual prebuild input construction](https://github.com/moonbitlang/moon/blob/4da23f805e562bcdb20a45764f1ab12cb892bf1d/crates/moonbuild/src/build_script.rs#L122-L133)
- [Dependency `dev_build` security behavior](https://docs.moonbitlang.com/en/latest/toolchain/moon/package.html#rule-and-dev-build)

This explains the current explicit `OPENDAL_MBT_SKIP_NATIVE=1`. The preview
bundler sets it internally. The user-facing stable command must not require it.

## Consumer contracts

### Preview contract

The preview is allowed to use the official bundler, but it must meet all of
these conditions:

- one `moon add` installs the public facade and declares the exact dependency;
- the bundler is a precompiled Wasm executable from the same Mooncake module
  and requested version, so the consumer does not compile bundler source;
- bootstrap compares a release-specific facade/bridge compatibility
  fingerprint, ABI version, feature bitmap, OpenDAL version, and artifact
  manifest digest before constructing an operator;
- the bundler invokes Moon with the native resolver disabled internally;
- the bundler downloads no source code and invokes no Rust tool;
- application MoonBit code imports only `Eric-Song-Nop/opendal/wasm`;
- application code supplies no loader, promise adapter, or import table;
- a cold build verifies hashes before publishing output;
- a warm build can complete offline from a verified cache;
- corrupted cache entries are rejected and quarantined;
- the resulting `dist` directory is relocatable and contains relative URLs.

The preview output layout is:

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

The exact consumer application filename can vary. The names under the OpenDAL
asset namespace are stable within one artifact-manifest schema.
`bootstrap.mjs` is the deployable entry point: it loads the manifest, verifies
the declared runtime files, instantiates the bridge and application through
the official loader, then invokes the configured Moon entry export (initially
`_start`). Applications may import that entry point directly or use it from a
small HTML shell; they do not reconstruct the two-module import graph.

The compatibility fingerprint, rather than inferred semantic-version text,
is the authoritative mismatch check. The current Moon CLI does not expose a
documented machine-readable query for the exact resolved version of one
dependency, and parsing private build files would create another unstable
contract. A resolved-version check can be added when Moon provides that query;
until then the facade compiles the expected fingerprint and the bridge exports
the actual one, so minimal-version selection cannot silently pair incompatible
code and artifacts.

### Stable contract

The stable build removes the explicit bundler command:

```sh
moon add Eric-Song-Nop/opendal@<binding-version>
moon build --target wasm --release
```

Moon or a Moon-supported bundling mode must copy the reachable dependency's
runtime assets beside the application and emit their relative locations.
There is no fallback to searching `$HOME`, `$MOON_HOME`, `.mooncakes`, or an
opaque global dependency cache at browser runtime.

### Application-code contract

The intended stable API is async-first and MoonBit-native. The exact spelling
will be frozen only after the browser async spike, but its semantics are:

```moonbit
async fn use_local_storage() -> Unit raise @opendal.OpenDalError {
  let op = @opendal.Operator::opfs()
  defer op.close()

  op.write("notes/hello.txt", b"hello from MoonBit")
  let bytes = op.read("notes/hello.txt")
  let metadata = op.stat("notes/hello.txt")
  // Use MoonBit values; no Promise, pointer, or JS loader value is visible.
}
```

If current Moon cannot expose a portable browser continuation API, an alpha
package may temporarily expose an explicitly experimental operation/callback
surface. The stable `/wasm` API is not declared complete until async functions
can suspend and resume through a documented runtime interface.

## Runtime architecture

### Module graph

```text
MoonBit browser application
  -> async OpenDAL MoonBit facade
  -> task/resource ABI imports
  -> official JS host adapter
       -> schedules callbacks/microtasks
       -> performs bounded bulk copies
       -> owns module instantiation order
  -> wasm-bindgen-compatible Rust bridge
  -> wasm_bindgen_futures browser executor
  -> OpenDAL async core
  -> memory / OPFS / Fetch-compatible services
```

JavaScript is a runtime adapter, not a storage implementation. Paths,
capabilities, error mapping, service configuration, and storage operations
remain implemented by MoonBit facade code and Rust OpenDAL.

### Bootstrap

Before creating an operator, the facade and loader validate:

- bridge ABI major/minor;
- artifact-manifest schema;
- OpenDAL version;
- service-profile identifier;
- required feature bitmap;
- supported runtime kind;
- optional bulk-transfer and cancellation protocol versions.

An ABI-major mismatch fails before any resource is created. Optional operations
are capability-gated. The loader never silently selects a different artifact
or the latest release.

### Async task ABI

The poll-once adapter is replaced with an owned task arena. A representative
operation is:

```text
operator_read_start(operator, path) -> task_handle
host_wait(task_handle) -> Promise/completion callback
task_state(task_handle) -> state
task_take(task_handle) -> completion_handle
task_cancel(task_handle) -> status
task_release(task_handle) -> status
```

Every operation copies its inputs into Rust-owned storage before `start`
returns. Rust drives the OpenDAL future with
`wasm_bindgen_futures::spawn_local` or an equivalent browser-local executor.

The state model is:

```text
Pending
  -> Ready(Completion)
  -> Consumed
  -> Released

Pending
  -> CancelRequested
  -> Cancelled
  -> Released
```

Required semantics:

- each completion is delivered at most once;
- each result is taken at most once;
- callback delivery always occurs in a later microtask to avoid re-entering a
  borrowed Rust or MoonBit frame;
- cancelling a task prevents later delivery to MoonBit;
- the first cancellation contract is logical cancellation; true cancellation
  of an underlying Fetch or OPFS promise is claimed only when proven;
- an in-flight task owns an OpenDAL operator clone, so closing the MoonBit
  wrapper cannot cause use-after-close;
- dropping an instance unregisters callbacks and makes late completions inert;
- task, completion, error, buffer, reader, and writer handles use generation
  checks and explicit release;
- errors belong to the completion, not to one global sticky error slot.

### MoonBit continuation gate

Rust can produce a browser Promise today. The open question is how a MoonBit
`async fn` suspends on that Promise in an ordinary browser runtime.

MoonBit documents Wasm closure imports through `externref` and the
`moonbit:ffi.make_closure` host import, which is useful for callback delivery:

- [MoonBit Wasm FFI and closures](https://docs.moonbitlang.com/en/latest/language/ffi.html)

MoonBit also documents Wasm1 async as experimental and currently tied to the
latest `moonrun` rather than arbitrary Wasm hosts:

- [MoonBit experimental async support](https://docs.moonbitlang.com/en/latest/language/async-experimental.html)

The next implementation spike must compare:

1. a documented browser-compatible Moon async suspension interface, if one is
   available in the selected Moon version;
2. a host callback that resumes a repository-owned Moon continuation;
3. an explicit experimental `Operation[T]` callback API as a preview-only
   fallback.

Undocumented compiler intrinsics may be used for a short-lived experiment but
not as the stable package contract. A synchronous busy loop, `Atomics.wait`,
POSIX pipe, or UI-thread blocking facade is rejected.

### Bulk data transfer

The current per-byte `buffer_push` and `buffer_get` calls remain useful as a
canary oracle but are removed from production read/write paths.

The host adapter can see both module memories and performs bounded copies:

1. the Moon facade creates a contiguous ABI scratch slice;
2. the host import receives an offset and length;
3. the loader takes a fresh `Uint8Array` view of Moon memory;
4. the Rust bridge allocates a destination or supplies a bridge buffer view;
5. the loader copies one bounded chunk;
6. ownership is committed only after the full chunk succeeds.

The reverse path follows the same rule. Typed-array views are recreated after
`memory.grow` because previous views may be detached.

Initial safety limits are deliberately conservative:

- 256 KiB transfer chunks;
- a configurable 64 MiB whole-object materialization limit;
- checked offset-plus-length arithmetic on both sides;
- no unbounded allocation based solely on a foreign length;
- cross-boundary call count grows with chunk count, not byte count.

Streaming readers and writers are added only after one-shot task cancellation,
bulk copy, and backpressure are stable.

## Public API plan

The unpublished canary API may change. The public browser API is developed in
this order:

| Stage | Constructors | Operations | Notes |
| --- | --- | --- | --- |
| Async memory | `Operator::memory` | write, read, stat, delete | proves real `Pending` completion |
| Browser local | `Operator::opfs` | create_dir, write, read, stat, list, delete | first useful product |
| Browser S3 | typed S3 builder | write, read, stat, delete | temporary credentials and CORS contract |
| Streaming | same builders | Reader, Writer, abort, bounded chunks | requires backpressure |
| Convergence | target-specific constructors | portable common subset | compare native and Wasm interfaces |

Portable values should converge with the native binding where their semantics
are truly shared:

- owned errors;
- metadata snapshots;
- byte ranges;
- presigned request values;
- typed S3 configuration;
- path and size validation.

Resource representations and execution remain target-specific. A browser API
does not claim blocking methods, local filesystem access, native file
descriptors, multithreaded Tokio, or implicit finalization.

Explicit `close`/`abort` stays part of the contract. Automatic finalization is
only an additional safety net if MoonBit later provides a portable finalizer
whose timing and re-entrancy rules are suitable.

## Artifact and trust contract

### Profiles

Use service-minimal artifacts:

| Profile | OpenDAL features | Purpose |
| --- | --- | --- |
| `memory-canary` | memory only | repository interoperability checks; never the stable default |
| `browser-memory-preview` | memory only | async, bulk-copy, and Mooncake distribution preview |
| `browser-local` | memory + OPFS | first public browser artifact |
| `browser-s3` | memory + S3 + Fetch transport | later opt-in network artifact |
| `browser-standard` | selected local + cloud services | considered only after size measurements |

Memory is built into OpenDAL core. OPFS and S3 features are explicit. One
"all services" Wasm is not the default because service code, browser glue, and
attack surface affect size and startup.

### Manifest

Wasm uses a separate manifest, for example
`wasm/artifacts-browser.json`:

```json
{
  "schema_version": 1,
  "binding_version": "<semver>",
  "bridge_abi": "<major.minor>",
  "opendal_version": "0.58.1",
  "rust_toolchain": "1.91",
  "target": "wasm32-unknown-unknown",
  "runtime": "browser-window",
  "service_profile": "browser-local",
  "features": ["task-v1", "bulk-copy-v1", "memory", "opfs"],
  "artifacts": {
    "bridge_wasm": {
      "packaged_path": "wasm/artifacts/browser-local/bridge.wasm",
      "release_url": "<exact tag URL>",
      "size": 0,
      "sha256": "<64 lowercase hex>"
    },
    "bridge_js": {
      "packaged_path": "wasm/artifacts/browser-local/bridge.mjs",
      "release_url": "<exact tag URL>",
      "size": 0,
      "sha256": "<64 lowercase hex>"
    }
  }
}
```

The real schema also records the loader digest, wasm-bindgen version, wasm-opt
version, minimum Moon version, required WebAssembly proposals, and license
files.

### Build and release

Canonical artifacts are produced in a pinned Linux release environment with:

- `cargo build --locked` for `wasm32-unknown-unknown`;
- release panic strategy `abort`;
- exact Rust, wasm-bindgen, wasm-opt, OpenDAL, and lockfile versions;
- feature-minimal Cargo profiles;
- deterministic archive paths and timestamps;
- imports/exports recorded in the manifest;
- an SBOM or dependency inventory;
- archive and every runtime file hashed independently.

Tag release ordering is:

1. build the candidate bridge/profile artifacts;
2. record candidate hashes and inspect imports/exports/size;
3. verify a clean packaged consumer against those candidate hashes;
4. publish GitHub Release assets;
5. publish the Mooncake containing the exact manifest, facade, loader, and the
   same canonical bridge bytes;
6. install from the registry in a fresh consumer;
7. verify cold acquisition and offline hot-cache use.

The Moon package is never published before the exact companion URLs exist.
The preview bundler may acquire the canonical files from those URLs. The
stable ordinary-build path instead materializes the package-embedded
`packaged_path` files through Moon's dependency runtime-asset graph. Both
paths verify the same size and digest and must produce byte-identical runtime
files. Embedding the stable assets is conditional on measuring and accepting
the registry archive size; it avoids making application builds depend on
GitHub availability.

### Cache

The bundler owns a documented content-addressed cache namespace; it does not
read Moon's opaque dependency-source cache. A suitable default is under
`$MOON_HOME/cache/lib/opendal-mbt/<binding-version>/<profile>/wasm32-unknown-unknown/sha256-<digest>/`
with an explicit override for controlled environments. This follows the
native resolver's cache convention while keeping every Wasm profile and
digest isolated.

The resolver follows the native artifact discipline:

- exact versioned URLs only;
- TLS plus SHA-256 and size verification;
- per-digest lock with stale-lock handling;
- download to a unique staging path;
- atomic publication after complete validation;
- quarantine or removal of corrupt entries;
- hot-cache reuse with the network unavailable;
- no fallback from a pinned release to `latest`;
- local artifact override limited to maintainer workflows.

## Browser service plan

### Memory

Memory is not the product storage backend; it is the deterministic test of
task lifecycle, bulk bytes, cancellation, error ownership, and packaging.

The decisive memory test must force at least one operation through
`Pending -> Ready`. An immediately ready future does not validate browser
scheduling.

### OPFS

OPFS is the first public service after the async and distribution gates.
OpenDAL `0.58.1` includes a browser-oriented OPFS Wasm example:

- [OpenDAL OPFS Wasm example](https://github.com/apache/opendal/tree/v0.58.1/core/edge/opfs_wasm32)

The first support claim is restricted to a secure-context browser `Window`.
Acceptance covers:

- operator construction through `navigator.storage.getDirectory()`;
- create directory, write, read, stat, list, and delete;
- persistence after page reload;
- isolation between origins;
- missing-path mapping;
- quota and permission failures;
- cancellation during pending I/O;
- instance teardown with work in flight;
- zero live bridge resources after cleanup.

Copy, rename, presign, Node, Worker, and WASI are not inferred from the OPFS
service name. Capability values report only operations proven in this target.

### S3

S3 follows OPFS and uses OpenDAL's browser Fetch-compatible transport:

- [OpenDAL S3 Wasm example](https://github.com/apache/opendal/tree/v0.58.1/core/edge/s3_read_on_wasm)

The S3 profile must define:

- CORS and preflight requirements;
- HTTPS and mixed-content behavior;
- redirect and browser-forbidden header handling;
- timeout and logical cancellation semantics;
- temporary credentials, presigned requests, or a same-origin broker;
- secret redaction in errors, traces, and configuration snapshots.

Long-lived access keys are not the default browser quickstart. First release
scope is one-shot write/read/stat/delete. Streaming and multipart behavior
follow the common streaming milestone.

## Toolchain collaboration

Two upstream Moon capabilities turn the preview flow into the stable flow.

### 1. Target-aware prebuild

The implementation already has `BuildInfo`/`TargetInfo` skeletons and the
planning call site already knows the concrete backend. The upstream request
should have these acceptance criteria:

- input activates the existing `build.target.kind` skeleton and exposes the
  final target kind, profile, mode, host, and target triple;
- target-kind names are stable, documented values matching concrete CLI
  targets such as `wasm` and `native`;
- `--target all` invokes the script once per concrete backend;
- the default reports the resolved module `preferred_target`;
- `paths.out_dir` is absolute, created, writable, and isolated by
  module/target-kind/profile;
- old scripts that only read `env` and `paths` remain compatible;
- behavior is explicit for check, info, build, run, test, and bundle;
- an OpenDAL Wasm build returns an empty native link configuration without
  touching a native artifact table, cache, or network.

This is a small and independently useful toolchain change.

### 2. Declarative runtime assets

Moon needs a target-scoped runtime-asset concept for library dependencies.
The exact manifest syntax is an upstream design question, but its semantics
must cover:

- only assets from reachable packages;
- target/profile conditions;
- dependency-relative static inputs and generated `out_dir` inputs;
- namespaced final paths to prevent collisions;
- deterministic copy/materialize nodes in the build graph;
- a final application asset manifest with logical name, relative URL, version,
  size, digest, and ABI metadata;
- watch/cache inputs and clear conflict diagnostics;
- no native archive propagation into Wasm and no Wasm bridge propagation into
  native builds;
- no arbitrary transitive post-link command execution.

An illustrative declaration is:

```text
runtime_assets = [
  {
    target: "wasm",
    source: "wasm/artifacts/opendal_mbt_bridge.wasm",
    output: "assets/Eric-Song-Nop/opendal/bridge.wasm"
  }
]
```

The first solution should be declarative copying and manifests, not a generic
dependency-controlled shell hook. WIT composition may justify an explicit
root opt-in post-link action later.

## Workstreams

The work proceeds in parallel where dependencies allow.

### A. Rust bridge

- replace poll-once with a task arena and browser executor;
- remove global sticky errors from concurrent paths;
- add completion, cancellation, and teardown states;
- add bulk buffer primitives;
- add feature-minimal memory/OPFS/S3 profiles;
- generate wasm-bindgen browser glue;
- preserve generation-checked resource ownership;
- record imports, exports, sizes, and features.

### B. MoonBit facade

- make the unpublished `/wasm` API async-first;
- prototype stable browser continuation integration;
- own all error/value conversion;
- hide task, completion, callback, and loader handles;
- add explicit close/abort and capability queries;
- share pure value/validation semantics with native without sharing handles;
- pin a generated public-interface contract for every preview.

### C. Loader and bundler

- instantiate the Rust module before the Moon module;
- validate manifest/ABI/features before exposing the application;
- implement microtask callback delivery and teardown;
- implement bounded bulk copies across the two memories;
- provide the Mooncake executable bundler;
- implement verified cold/hot artifact acquisition;
- emit a relocatable deployment manifest;
- prove that the emitted directory can be consumed by common static web
  bundlers without requiring an npm package.

### D. Distribution and release

- define the Wasm artifact schema and service profiles;
- create deterministic candidate/release workflows;
- add a clean registry-consumer fixture;
- verify no Rust/Cargo/npm/source checkout is visible to the consumer;
- keep native and Wasm release trust chains independent but version-aligned;
- publish pre-release versions until browser async and OPFS gates close.

### E. Upstream Moon work

- propose target-aware prebuild;
- propose declarative runtime assets;
- track a stable browser async/Promise continuation interface;
- evaluate WIT component composition after the core-module product works.

## Milestones and exit criteria

Effort ranges are order-of-magnitude estimates for one engineer, not release
date promises. Toolchain collaboration can proceed concurrently. Recalibrate
M1–M4 after the task-runner, precompiled-`moonx`, and OPFS skeleton spikes;
those three probes contain most of the schedule uncertainty.

### M0 — Core-module memory canary

Status: implementation present in draft PR #59; acceptance evidence pending.

Deliverables:

- MoonBit facade, Rust bridge, and official loader;
- scalar-only bootstrap;
- binary write/read/stat/NotFound;
- generation handles and leak oracle;
- explicit native-prebuild opt-out.

Exit:

- the round trip runs through the MoonBit public facade and real OpenDAL;
- repeated lifecycle returns the live-handle count to baseline;
- the exact module imports/exports and artifact sizes are recorded.

M0 is not a release.

### M1 — Actually asynchronous memory engine

Estimated size: large, roughly 5–8 engineering days.

Deliverables:

- task ABI v1;
- forced-delayed memory operation that reaches `Pending`;
- Promise/event-loop driver;
- per-completion errors;
- logical cancellation and instance teardown;
- concurrent operations on multiple operators.

Exit:

- success, failure, cancel-before-ready, ready-versus-cancel race,
  close-while-pending, double-take, double-release, and late completion all
  have deterministic results;
- callbacks are not delivered synchronously;
- there are no pipes, signals, blocking waits, threads, or SharedArrayBuffer
  requirements;
- all handle classes return to baseline.

Decision gate:

- if a documented Moon browser continuation works, proceed with public
  `async fn`;
- otherwise publish only an explicitly experimental callback/operation API
  while pursuing the Moon runtime capability.

### M2 — Bulk binary ABI

Estimated size: medium, roughly 3–5 engineering days.

Deliverables:

- bounded cross-memory chunk transfer;
- checked pointer/range protocol internal to the loader;
- OOM and memory-growth handling;
- removal of per-byte calls from public operations.

Exit:

- a binary 16 MiB value round-trips exactly;
- boundary call count is proportional to chunks;
- memory growth does not reuse stale typed-array views;
- cancellation and allocation failure leave no partial owned result;
- the configured whole-object limit fails before excessive allocation.

### M3 — Mooncake preview distribution

Estimated size: large, roughly 5–8 engineering days.

Deliverables:

- `cmd/wasm-bundle` Mooncake executable and its precompiled linear-Wasm
  registry asset, built against a recorded minimum compatible `moonrun`;
- Wasm artifact manifest and versioned GitHub Release archive;
- content-addressed verified cache;
- relocatable `dist` output;
- clean consumer outside this checkout;
- Mooncake pre-release documentation.

Exit:

- a fresh consumer runs only `moon add` and the exact-version `moonx` command;
- `moonx` downloads the bundler executable rather than compiling its source,
  and the bundler starts only the fixed Wasm child build in its own target
  directory;
- Cargo, rustup, npm, this repository, local artifact overrides, and manual
  JavaScript are absent;
- cold acquisition verifies every artifact;
- offline hot-cache rebuild succeeds;
- corruption and version mismatch fail before application output is published;
- user code imports only the MoonBit `/wasm` package.

This is the first publishable preview, initially with memory only.

### M4 — OPFS browser-local profile

Estimated size: large, roughly 5–8 engineering days after M1–M3.

Deliverables:

- `services-opfs` bridge profile;
- typed `Operator::opfs` constructor;
- create_dir/read/write/stat/list/delete;
- Browser Window integration fixture;
- reload, quota, permission, and cancellation cases.

Exit:

- data persists across reload under HTTPS or localhost;
- capability values match the implemented OpenDAL backend;
- all expected failures become owned MoonBit errors;
- the UI event loop remains responsive while storage is pending;
- teardown leaves no tasks, callbacks, buffers, or operators;
- the clean Mooncake consumer deploys without Rust/npm.

M4 is the first useful browser-local preview.

### M5 — Stable distribution and API gate

Estimated size: dependent on the Moon async and runtime-asset decisions.

Deliverables:

- stable async facade or an explicit decision that stable release must wait;
- target-aware native resolver behavior with no user environment variable;
- ordinary-build runtime asset propagation or a formally retained bundler;
- API/version compatibility policy;
- browser and Moon version support matrix;
- size/startup budgets;
- release provenance and fresh-registry verification.

Exit for the preferred stable claim:

- `moon add` plus `moon build --target wasm --release` produces a complete,
  relocatable deployment;
- application code uses public MoonBit async APIs only;
- no experimental compiler intrinsic is part of the ABI;
- native package contracts and artifacts remain unchanged;
- the registry consumer proves exact facade/bridge version matching.

If Moon runtime assets are not available, the project may ship a high-quality
preview with `moonx`, but it must not call that build path identical to native.

### M6 — Browser S3 profile

Estimated size: large, roughly 5–8 engineering days after the common async
runtime is stable.

Deliverables:

- Fetch-compatible OpenDAL S3 artifact;
- typed S3 configuration;
- temporary-credential/presign/broker guidance;
- CORS and security contract;
- failure, timeout, and cancellation coverage.

Exit:

- a CORS-enabled HTTPS endpoint passes write/read/stat/delete;
- 403, 404, preflight failure, redirect, timeout, and cancellation map
  deterministically;
- errors and logs do not expose credentials;
- Node and WASI are not implied by browser success.

### M7 — Streaming and component evaluation

Deliverables:

- bounded Reader and Writer resources;
- backpressure, abort, close, and partial-transfer semantics;
- comparison of core-module loader versus WIT/component composition.

Exit:

- streaming never materializes an unbounded whole object;
- cancellation releases producer and consumer resources;
- WIT replaces the existing ABI only if it reduces application packaging and
  retains browser async/OPFS behavior with measurable evidence.

## Validation matrix

Future validation is evidence for milestone gates; it is not a reason to make
an interactive development session wait idly.

| Layer | Evidence |
| --- | --- |
| Static ABI | exact Moon imports versus Rust exports, manifest schema, API snapshot |
| Rust state model | task/cancel/late-completion/resource property cases |
| Moon facade | type/error conversion and compile-contract fixtures |
| Core Wasm | import/export inspection, no WASI/POSIX/thread surprises |
| Browser runtime | real browser Window, delayed completion, responsive event loop |
| Binary transfer | NUL/non-UTF-8, 16 MiB chunks, limits, memory growth |
| OPFS | persistence, origin isolation, quota/permission, reload |
| Packaging | fresh Mooncake consumer, no Rust/npm/checkout, relocatable output |
| Integrity | wrong hash/size/version/ABI, corrupt cache, interrupted download |
| Release | candidate artifacts, exact pins, registry install, offline hot cache |

CI remains a merge and release gate, but implementation work should move to
independent tasks after a workflow is triggered rather than waiting
interactively for it.

## Risk register

| Risk | Impact | Mitigation and retirement evidence |
| --- | --- | --- |
| Moon browser async remains moonrun-specific | Cannot offer native-looking `async fn` in browsers | M1 continuation spike; stable release requires a documented host runtime interface |
| Runtime assets are not propagated | Ordinary `moon build` omits the bridge | M3 `moonx` fallback; upstream declarative-asset proposal; M5 clean ordinary build |
| Preview bundler has build-machine authority | A compromised helper could read/write outside the intended output or run another process | precompiled exact-version asset, no shell, fixed child argv, path containment, pinned URLs, disclosed trust boundary, and least-privilege `moonrun` policy when supported |
| wasm-bindgen browser glue changes | Loader/bridge mismatch | pin exact tool version and ship JS+Wasm as one hashed artifact set |
| Two memories make copies expensive | Poor large-object throughput | M2 chunked bulk-copy budget; later streaming; no per-byte production path |
| Cancellation cannot abort host promise | wasted I/O after logical cancel | promise-level abort probes; claim logical cancellation only until stronger evidence |
| OPFS is Window-specific in OpenDAL 0.58.1 | Worker/Node claims would be wrong | support matrix and Window-only artifact metadata |
| S3 credentials leak in browser | security incident | temporary credentials/presign/broker; redaction cases; no long-lived-key quickstart |
| Wasm size grows with services | slow download/startup | service-minimal profiles and recorded compressed/uncompressed budgets |
| Dependency cache layout changes | loader cannot find asset | never expose Moon cache paths; bundler copies to application-owned `dist` |
| Native and Wasm versions drift | facade/bridge ABI failure | one binding version, exact manifest pins, bootstrap rejection |
| WIT/component work expands scope | delays useful OPFS release | keep it off the critical path until M7 |

## Feasibility assessment

| Goal | Assessment | Main uncertainty |
| --- | ---: | --- |
| Two-module MoonBit/OpenDAL Wasm binding | 8/10 | production async and bulk transfer, not basic interoperability |
| Mooncake preview with official bundler | 8/10 | engineering work is local and does not require Moon linker changes |
| OPFS after async lifecycle | 7/10 | browser runtime, quota/permission, Window restriction |
| Browser S3 | 6/10 | CORS, credentials, Fetch cancellation |
| Stable MoonBit `async fn` in ordinary browsers | 6/10 | current Moon Wasm1 async runtime portability |
| `moon add` plus plain `moon build` only | 5/10 today, higher with upstream support | declarative dependency runtime assets |

The overall project remains feasible and worth pursuing. The binding boundary
is already demonstrated in code. The critical unknown is a stable MoonBit
browser continuation contract; the main distribution gap is a first-class
runtime-asset concept in Moon.

## Pull-request sequence

Keep changes reviewable and preserve the native baseline:

1. design and Mooncake delivery contract (PR #58);
2. memory core-module canary (PR #59);
3. task ABI and delayed-memory async engine;
4. browser callback/continuation adapter;
5. bulk-transfer ABI;
6. artifact manifest, resolver, and `moonx` bundler;
7. clean Mooncake preview consumer;
8. OPFS `browser-local` vertical slice;
9. stable API/support matrix and release workflow;
10. optional browser S3 profile;
11. streaming and WIT/component evaluation.

The task-ABI, bundler skeleton, and upstream proposals can begin in parallel.
OPFS implementation begins only after a forced-pending memory operation
completes through the same public MoonBit path.

## Immediate next actions

1. Freeze a draft task ABI v1 and error/completion ownership table.
2. Replace poll-once with a delayed-memory task driven by the browser event
   loop.
3. Build the Moon continuation spike and decide stable async versus
   preview-only callback surface.
4. Replace per-byte transfer with a bounded bulk-copy experiment.
5. Define `wasm/artifacts-browser.json` and the `browser-local` archive layout.
6. Scaffold the exact-version `cmd/wasm-bundle` package.
7. Create the clean consumer acceptance fixture with Cargo/rustup/npm removed.
8. Draft the two Moon upstream proposals: target-aware prebuild and
   declarative runtime assets.
9. Record canonical bridge imports, exports, compressed size, and startup
   measurements.
10. Only then add the OPFS service and persistence cases.

This ordering makes "Mooncake-installed OpenDAL for browser Wasm" the
acceptance criterion at every step, instead of leaving packaging until after
the storage API is built.
