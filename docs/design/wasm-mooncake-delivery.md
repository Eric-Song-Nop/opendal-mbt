# Mooncake delivery plan for OpenDAL WebAssembly

Status: proposed execution plan; M0 and an initial M1 callback/task canary are
implemented, but no Wasm release milestone is complete

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
| Stable browser API | The facade is backend-neutral, async, cancellation-safe, and bulk-transfer capable | First stable Wasm product |

A release must not be described as Mooncake-installable merely because the
MoonBit facade is present in the source archive. The companion Rust module and
the runtime adapter are part of the package contract.

## Scope decision

The first stable target is:

- MoonBit linear-memory `wasm`, not `wasm-gc`;
- a secure-context browser `Window`;
- an async-first MoonBit API;
- a generic `Operator::new(scheme, config)` entry point backed by OpenDAL's
  compiled service registry;
- service-minimal artifacts whose enabled schemes are declared in their
  manifests and discoverable at runtime;
- two core Wasm modules connected by an official loader;
- the existing native package remaining source- and ABI-compatible.

The following are separate product tracks, not aliases for this target:

- Node continues to use the native binding;
- Browser Worker support depends on the enabled services and their host APIs;
- WASI needs its own target, network/filesystem contract, and artifact;
- `wasm-gc` needs a separate ABI and runtime investigation;
- credential-bearing browser services need separate security acceptance after
  the common binding contract is proven.

No service is the interoperability mechanism or a privileged public API.
Memory and OPFS are convenient credential-free fixtures; they prove only the
common operations exercised by their tests. The binding must first make an
actually pending OpenDAL future complete safely through the browser event loop,
then expose the same scheme-and-config construction path for every service
compiled into an artifact.

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
   not as a prerequisite for the core binding.
7. Publish Wasm artifacts through a trust chain independent of the native
   static-library manifests.
8. Make `memory` the deterministic async and distribution canary. Exercise
   other services through the same generic constructor without adding
   service-specific facade constructors.

## Current implementation state

The implementation in PR #59 and the current follow-up slice establish the
basic two-module shape:

```text
MoonBit application Wasm
  -> Eric-Song-Nop/opendal/wasm facade
  -> scalar imports
  -> repository-owned loader
  -> Rust OpenDAL bridge Wasm
  -> OpenDAL memory service
```

The scalar ABI is version `0x0001_0004`. Its current feature bitmap is
`0x0000_01ff`: memory service, poll-once synchronous canary, generation
handles, binary buffers, task ABI, generic operator construction through the
compiled OpenDAL service registry, common create-dir/delete mutations, and
bounded streaming list materialization, and bounded bulk transfer. In addition to the original
synchronous round trip, the bridge now has generation-checked task and
completion handles, per-completion OpenDAL errors, logical cancellation,
per-operator service identity and capabilities, and permanent instance
teardown. The MoonBit facade exposes backend-neutral `Operator::new(scheme,
config)` plus the experimental public `write_callback`, `read_callback`,
`stat_callback`, `create_dir_callback`, `delete_callback`, and `list_callback`
methods. Each callback returns an `Operation` whose observable states are
`Pending`, `Completed`, `Cancelled`, and `Closed`.

The repository has two acceptance commands with intentionally different
claims:

```sh
# Node.js: synchronous ABI/bootstrap/resource smoke only.
make wasm-canary

# Real headless Chrome/Chromium: forced-Pending callback lifecycle.
make wasm-browser-canary
```

The browser fixture forces create-dir, write, read, `stat`, bounded list, two
idempotent recursive deletes, and a missing read to return `Pending` on their
first Rust poll. It rejects synchronous completion, lets a previously queued
browser heartbeat run before readiness, completes the MoonBit callback chain,
suppresses a cancel-before-ready callback, and checks the live-handle baseline
plus `runtime.dispose()`. Its success payload records `pendingTasks: 8`,
`heartbeat: true`, `cancellation: "suppressed"`, and `diagnostics: "isolated"`.
Only this Chrome/Chromium run is evidence for `Pending -> Ready` and a
responsive browser event loop; the Node command is not.

This proves more than the original poll-once canary, but it does not finish M1
or the product design:

- ordinary-browser MoonBit `async fn` suspension is still constrained by the
  officially documented runtime; the currently usable public path is the
  explicitly experimental callback `Operation` API;
- cancellation is logical only: it suppresses callbacks and discards late
  results but does not abort the underlying OpenDAL future or browser I/O;
- the synchronous methods remain for smoke coverage and still return code `9`
  if their single no-op-waker poll sees `Pending`;
- the current browser fixture does not yet cover the full ready/cancel race,
  double-take/release, multi-operator concurrency, or all teardown orderings;
- bytes still cross the boundary one byte per Wasm call;
- the Mooncake archive does not yet contain or automatically acquire the
  version-matched Rust bridge, and canonical import/export/size evidence is
  still outstanding.

A compiler check or successful Rust target build alone does not satisfy these
runtime gates. M0 remains an engineering proof, and the implemented M1 slice
is not a release.

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
async fn use_storage() -> Unit {
  let op = @opendal.Operator::new("memory")
  defer op.close()

  op.write("notes/hello.txt", b"hello from MoonBit")
  let bytes = op.read("notes/hello.txt")
  let metadata = op.stat("notes/hello.txt")
  // Use MoonBit values; no Promise, pointer, or JS loader value is visible.
}
```

The current canary takes the fallback branch of this design and exposes the
following callback shape publicly but experimentally:

```moonbit
let operation = op.read_callback("notes/hello.txt", result => {
  match result {
    Ok(bytes) => consume(bytes)
    Err(error) => report(error)
  }
})

// Both calls are idempotent. Cancellation is logical, not an I/O abort.
operation.cancel()
operation.close()
```

`write_callback` and `stat_callback` follow the same pattern. The task and
completion handles remain hidden behind `Operation`, but this callback surface
is not renamed or described as stable MoonBit async/await. The stable `/wasm`
API is not declared complete until async functions can suspend and resume
through a documented ordinary-browser runtime interface, or the project makes
an explicit reviewed decision to retain callbacks as the stable contract.

## Runtime architecture

### Target module graph

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
  -> services compiled into the selected OpenDAL artifact
```

JavaScript is a runtime adapter, not a storage implementation. Paths,
capabilities, error mapping, service configuration, and storage operations
remain implemented by MoonBit facade code and Rust OpenDAL.

The implemented memory canary already has the task/resource imports,
repository-owned scheduling, `wasm_bindgen_futures` executor, and OpenDAL
memory service in this graph. It currently exposes a callback facade rather
than portable MoonBit `async fn`. Public reads and writes use the bounded
bulk-copy step shown above; per-byte exports remain canary-only oracles.

### Target bootstrap

The current facade checks bridge ABI `0x0001_0004` and required feature bits
`0x0000_01ff` before creating an operator. The artifact-manifest and
service-profile checks below remain product work.

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

The callback path now implements an owned task arena while retaining the
poll-once synchronous calls for the Node smoke test. The implemented task ABI
is:

```text
operator_write_start(operator, path, data) -> task_handle
operator_read_start(operator, path)         -> task_handle
operator_stat_start(operator, path)         -> task_handle

task_state(task_handle) -> state
task_take(task_handle) -> completion_handle
task_cancel(task_handle) -> status
task_release(task_handle) -> status

completion_kind(completion_handle) -> kind
completion_status(completion_handle) -> status
completion_take_buffer(completion_handle) -> buffer_handle
completion_take_metadata(completion_handle) -> metadata_handle
completion_take_error(completion_handle) -> error_handle
completion_release(completion_handle) -> status

host wait_task(task_handle, callback)
teardown() -> status
```

Every operation copies its inputs into Rust-owned storage before `start`
returns and clones the operator for the in-flight future. Rust drives the
future with `wasm_bindgen_futures::spawn_local`. The memory canary adds a
zero-delay timer wrapper to force at least one pending poll; this is a
deterministic scheduling oracle, not a claim that every production service
needs an artificial delay.

The implemented bridge task states are scalar `1` pending, `2` ready, `3`
cancelled, and `4` consumed:

```text
Pending
  -> Ready(Completion)
  -> Consumed
  -> task handle released

Pending or Ready
  -> Cancelled
  -> task handle released
```

`task_take` moves the result exactly once into an independently owned
completion. Completion kinds are write (`1`), read (`2`), stat (`3`),
create-dir (`4`), delete (`5`), and list (`6`); successful read/stat/list takes
and failed-error takes consume that completion, while successful unit
completions are released.
The local MoonBit `Operation` maps the lifecycle to `Pending`, `Completed`,
`Cancelled`, and `Closed` without exposing either handle class.

The current slice implements these required semantics:

- each completion is delivered at most once;
- each result is taken at most once;
- callback delivery begins from a later microtask/timer turn to avoid
  re-entering the initiating Rust or MoonBit frame;
- cancelling a task prevents later delivery to MoonBit;
- cancellation unregisters the loader wait before releasing the task, so an
  orphan scheduler poll cannot overwrite an unrelated sticky diagnostic;
- cancellation is logical; true cancellation of an underlying Fetch or OPFS
  promise is not claimed;
- an in-flight task owns an OpenDAL operator clone, so closing the MoonBit
  wrapper cannot cause use-after-close;
- `runtime.dispose()` unregisters loader polling, clears the bridge arena, and
  makes late completions inert;
- task, completion, error, buffer, metadata, and operator handles use
  generation checks and explicit release;
- errors belong to the completion, not to one global sticky error slot.

Streaming reader/writer handles, true host-operation abort, and the complete
ready/cancel/concurrency race matrix remain later acceptance work.

### MoonBit continuation gate

Rust can run a browser-local future today. The unresolved product question is
how a MoonBit `async fn` suspends and resumes in an ordinary browser host using
an officially supported runtime contract.

MoonBit documents Wasm closure imports through `externref` and the
`moonbit:ffi.make_closure` host import, which is useful for callback delivery:

- [MoonBit Wasm FFI and closures](https://docs.moonbitlang.com/en/latest/language/ffi.html)

MoonBit also documents Wasm1 async as experimental and currently tied to the
latest `moonrun` rather than arbitrary Wasm hosts:

- [MoonBit experimental async support](https://docs.moonbitlang.com/en/latest/language/async-experimental.html)

The current implementation uses documented Wasm closure delivery plus an
explicit experimental callback `Operation` API. It proves the host scheduler
and Rust task ABI without claiming that callbacks are native MoonBit
async/await. The next continuation work must still find and validate a
documented browser-compatible Moon async suspension interface, or make an
explicit API decision to keep the callback form preview-only.

Undocumented compiler intrinsics may be used for a short-lived experiment but
not as the stable package contract. A synchronous busy loop, `Atomics.wait`,
POSIX pipe, or UI-thread blocking facade is rejected.

### Bulk data transfer

The per-byte `buffer_push` and `buffer_get` calls remain useful as canary
oracles but are not imported by production read/write paths.

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
| Generic core | `Operator::new(scheme, config)` | write, read, stat, create_dir, list, delete | available schemes come from the artifact's compiled registry; memory is the deterministic fixture |
| Service profiles | same generic constructor | capability-gated common operations | service tests validate behavior, not new public constructors |
| Streaming | same constructor | Reader, Writer, abort, bounded chunks | requires backpressure |
| Convergence | same scheme/config model | portable common subset | compare native and Wasm interfaces |

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

Use service-minimal artifacts. Profiles are packaging choices, not distinct
MoonBit APIs:

| Profile | OpenDAL features | Purpose |
| --- | --- | --- |
| `memory-canary` | memory only | repository interoperability checks; never the stable default |
| `browser-memory-preview` | memory only | async, bulk-copy, and Mooncake distribution preview |
| `browser-local-test` | memory + OPFS | persistence test fixture |
| `browser-object` | memory + selected HTTP object services + browser transport | credential/CORS acceptance profile |
| `browser-standard` | selected browser-compatible services | considered only after size measurements |

Memory is built into OpenDAL core; other services are explicit Cargo features.
One "all services" Wasm is not the default because target compatibility,
service code, browser glue, and attack surface affect size and startup. The
manifest records the exact registered schemes, and the facade exposes them for
diagnostics and capability-driven application code.

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

## Service-neutral binding and fixtures

The public boundary accepts a service scheme and configuration map, matching
the native binding. Rust initializes OpenDAL's registry and rejects schemes
that are absent from the selected artifact with an owned error that also names
the available schemes. Operator information and capabilities are queried from
the constructed OpenDAL operator; they are not inferred from a facade method
or hard-coded service table.

Every service remains responsible for its own target and host requirements.
Adding a service to an artifact is a build-profile and acceptance-test change,
not a new MoonBit constructor.

### Memory

Memory is not the product storage backend; it is the deterministic test of
task lifecycle, bulk bytes, cancellation, error ownership, and packaging.

The decisive memory test must force at least one operation through
`Pending -> Ready`. An immediately ready future does not validate browser
scheduling.

### OPFS fixture

OPFS is an optional persistence fixture after the common async and distribution
gates. It receives no privileged facade API.
OpenDAL `0.58.1` includes a browser-oriented OPFS Wasm example:

- [OpenDAL OPFS Wasm example](https://github.com/apache/opendal/tree/v0.58.1/core/edge/opfs_wasm32)

An artifact that compiles OPFS is restricted to a secure-context browser
`Window`. Its service-specific acceptance covers:

- operator construction through `navigator.storage.getDirectory()`;
- create directory, write, read, stat, list, and delete;
- persistence after page reload;
- isolation between origins;
- missing-path mapping;
- quota and permission failures;
- cancellation during pending I/O;
- instance teardown with work in flight;
- zero live bridge resources after cleanup.

Copy, rename, presign, Node, Worker, and WASI are not inferred from this
fixture. Capability values come from OpenDAL and report only the constructed
operator's operations.

### Credential-bearing object-service fixtures

S3 is one useful network fixture and uses OpenDAL's browser-compatible HTTP
transport:

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

- extend the implemented memory task arena/browser executor through the full
  race and concurrency matrix;
- keep OpenDAL operation errors in the implemented per-completion ownership
  path and remove remaining sticky-error dependencies from concurrent paths;
- harden the implemented completion, logical-cancellation, and teardown states;
- add bulk buffer primitives;
- add feature-minimal service profiles without changing the facade API;
- keep the generated wasm-bindgen browser glue pinned and reproducible;
- preserve generation-checked resource ownership;
- record imports, exports, sizes, and features.

### B. MoonBit facade

- retain the implemented callback `Operation` surface as experimental while
  pursuing the intended async-first API;
- prototype stable browser continuation integration;
- extend the implemented error/value conversion;
- continue hiding task, completion, callback-host, and loader handles;
- add explicit close/abort and capability queries;
- share pure value/validation semantics with native without sharing handles;
- pin a generated public-interface contract for every preview.

### C. Loader and bundler

- retain the implemented Rust-before-Moon instantiation order;
- validate manifest/ABI/features before exposing the application;
- harden the implemented later-turn callback delivery and teardown;
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
- publish pre-release versions until the browser async and generic-service
  gates close.

### E. Upstream Moon work

- propose target-aware prebuild;
- propose declarative runtime assets;
- track a stable browser async/Promise continuation interface;
- evaluate WIT component composition after the core-module product works.

## Milestones and exit criteria

Effort ranges are order-of-magnitude estimates for one engineer, not release
date promises. Toolchain collaboration can proceed concurrently. Recalibrate
M1–M4 after the task-runner, precompiled-`moonx`, and generic registry spikes;
those three probes contain most of the schedule uncertainty.

### M0 — Core-module memory canary

Status: synchronous round-trip and resource evidence is implemented and
exercised by `make wasm-canary`; exact import/export and artifact-size evidence
is still pending.

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

The first two exit bullets are covered by the Node runner, which executes the
lifecycle twice and verifies teardown reaches zero live bridge handles. The
third remains open. M0 is not a release, and its Node runner is not evidence of
browser suspension.

### M1 — Actually asynchronous memory engine

Estimated size: large, roughly 5–8 engineering days.

Status: partial. `make wasm-browser-canary` now supplies real Chrome/Chromium
evidence for the initial task/callback slice, but the full exit matrix and the
stable MoonBit continuation decision remain open.

Deliverables:

- task ABI v1;
- forced-delayed memory operation that reaches `Pending`;
- event-loop callback/continuation driver;
- per-completion errors;
- logical cancellation and instance teardown;
- concurrent operations on multiple operators.

Implemented evidence:

- task ABI and per-completion errors for callback
  create-dir/write/read/stat/list/delete;
- eight forced `Pending -> Ready` operations, including bounded list,
  idempotent recursive delete, and owned `NotFound`;
- later-turn callback delivery and a browser heartbeat that runs before
  readiness;
- cancel-before-ready callback suppression, scheduler diagnostic isolation,
  and live-handle restoration;
- operator close while a cloned task is pending, plus idempotent instance
  teardown.

Still required for M1 exit:

- ready-versus-cancel, double-take, double-release, late-completion teardown,
  and multi-operator concurrency cases with deterministic outcomes;
- a documented ordinary-browser MoonBit continuation, or an explicit decision
  that release remains callback-based and experimental.

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

### M4 — Generic service profiles and browser fixtures

Estimated size: large, roughly 5–8 engineering days after M1–M3.

Deliverables:

- generic `Operator::new(scheme, config)` backed by the compiled OpenDAL
  registry;
- available-scheme, operator-info, and capability inspection;
- create_dir/read/write/stat/list/delete through the common task ABI;
- at least one non-memory Browser Window integration fixture;
- service-specific reload, quota/permission or credential/CORS cases as
  applicable.

Exit:

- every enabled scheme is constructible through the same facade entry point;
- capability values match the implemented OpenDAL backend;
- all expected failures become owned MoonBit errors;
- the UI event loop remains responsive while storage is pending;
- teardown leaves no tasks, callbacks, buffers, or operators;
- the clean Mooncake consumer deploys without Rust/npm.

M4 proves that the preview is not a memory-specific binding. It does not make
one tested backend representative of every OpenDAL service.

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
  retains browser async and service-fixture behavior with measurable evidence.

## Validation matrix

Future validation is evidence for milestone gates; it is not a reason to make
an interactive development session wait idly.

| Layer | Evidence |
| --- | --- |
| Static ABI | exact Moon imports versus Rust exports, manifest schema, API snapshot |
| Rust state model | task/cancel/late-completion/resource property cases |
| Moon facade | type/error conversion and compile-contract fixtures |
| Node core-Wasm smoke | `make wasm-canary`: synchronous ABI/bootstrap, repeated lifecycle, teardown |
| Core Wasm | import/export inspection and size record; no WASI/POSIX/thread surprises |
| Browser runtime | `make wasm-browser-canary`: real Chrome/Chromium, eight forced-pending tasks, heartbeat, logical cancel suppression, cleanup |
| Binary transfer | NUL/non-UTF-8, 16 MiB chunks, limits, memory growth |
| Service fixtures | generic construction, reported capabilities, and each service's relevant persistence/security failures |
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
| Some services are target- or host-specific | A generic constructor could be mistaken for universal runtime support | manifest exact compiled schemes; service-specific host matrix and acceptance fixtures |
| S3 credentials leak in browser | security incident | temporary credentials/presign/broker; redaction cases; no long-lived-key quickstart |
| Wasm size grows with services | slow download/startup | service-minimal profiles and recorded compressed/uncompressed budgets |
| Dependency cache layout changes | loader cannot find asset | never expose Moon cache paths; bundler copies to application-owned `dist` |
| Native and Wasm versions drift | facade/bridge ABI failure | one binding version, exact manifest pins, bootstrap rejection |
| WIT/component work expands scope | delays the useful generic binding | keep it off the critical path until M7 |

## Feasibility assessment

| Goal | Assessment | Main uncertainty |
| --- | ---: | --- |
| Two-module MoonBit/OpenDAL Wasm binding | 8/10 | production async and bulk transfer, not basic interoperability |
| Mooncake preview with official bundler | 8/10 | engineering work is local and does not require Moon linker changes |
| Generic service registry and capability surface | 8/10 | profile selection and service-specific host requirements |
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
3. task ABI and delayed-memory async engine (initial canary implemented;
   race/concurrency hardening remains);
4. browser callback adapter (experimental `Operation` and Chrome proof
   implemented; stable continuation decision remains);
5. bulk-transfer ABI;
6. artifact manifest, resolver, and `moonx` bundler;
7. clean Mooncake preview consumer;
8. generic service construction/capability surface and non-memory fixtures;
9. stable API/support matrix and release workflow;
10. optional browser S3 profile;
11. streaming and WIT/component evaluation.

The forced-pending memory operation now completes through the public MoonBit
callback path in Chrome. Task-ABI hardening, the generic constructor, the
bundler skeleton, and upstream proposals can proceed in parallel. Additional
service fixtures still wait for the remaining M1 race matrix, M2 transfer path,
and M3 distribution gates; the Chrome memory result alone does not authorize
claims about other services.

## Immediate next actions

1. Review and freeze the implemented draft task ABI v1, status values, and
   error/completion ownership table.
2. Add ready/cancel races, double-take/release, late teardown, and concurrent
   multi-operator browser cases.
3. Continue the documented Moon continuation spike; keep the current callback
   `Operation` API explicitly experimental meanwhile.
4. Replace per-byte transfer with a bounded bulk-copy experiment.
5. Define `wasm/artifacts-browser.json` and its exact compiled-scheme profile.
6. Scaffold the exact-version `cmd/wasm-bundle` package.
7. Create the clean consumer acceptance fixture with Cargo/rustup/npm removed.
8. Draft the two Moon upstream proposals: target-aware prebuild and
   declarative runtime assets.
9. Record canonical bridge imports, exports, compressed size, and startup
   measurements.
10. Add non-memory service fixtures through the same generic constructor only
    after the common gates pass.

This ordering makes "Mooncake-installed OpenDAL for browser Wasm" the
acceptance criterion at every step, instead of leaving packaging until after
the storage API is built.
