# Browser Runtime and Wasm ABI

Status: browser ABI 1.7 released in `0.2.0`

This document defines the private runtime contract behind the MoonBit
JavaScript target. The Rust bridge happens to compile to WebAssembly inside
that implementation; this does not add support for MoonBit's `wasm`,
`wasm-gc`, or other targets. It is a maintainer contract, not a public API:
applications use the target-specific MoonBit surface described in the
[browser API reference](../reference/browser-api.md), never the scalar exports
or JavaScript facade directly.

The browser path is deliberately separate from the native C ABI even though
both currently use the version label 1.7:

```text
MoonBit async API
  -> JavaScript Promise facade
  -> scalar browser Wasm ABI
  -> Rust OpenDAL bridge
  -> memory, OPFS, or S3
```

Each embedded `Runtime::new` evaluates private wasm-bindgen glue and owns one
bridge WebAssembly instance with one linear memory. JavaScript owns opaque
scalar handles into that instance's Rust arena and copies data through the same
memory; there is no second WebAssembly memory and no raw Rust reference crosses
the boundary.

The advanced `Runtime::load` path has a stricter ownership rule. Browser ESM
imports cache a module by URL, and wasm-bindgen's default initializer reuses the
module's initialized exports. A page must therefore call `Runtime::load` at
most once for a given `bridge_module_url`, for the lifetime of that page, and
share the returned `Runtime` between operators. Closing it permanently tears
down that bridge instance; loading the same module URL again is not supported.
Use the embedded constructor when independently owned runtimes are needed.

## Version and feature negotiation

The implemented packed version is `0x0001_0007` (`major << 16 | minor`). The
Promise runtime requires an exact version match and production feature mask
`0x0000_fffc`. An exact match is intentional for this private, embedded
contract: the runtime, wasm-bindgen glue, and bridge are shipped as one source
package snapshot rather than independently upgraded components.

ABI 1.7 defines the following production groups:

| Bit | Contract |
| ---: | --- |
| 2 | Generation-checked handles |
| 3 | Binary buffers in bridge memory |
| 4 | Asynchronous task and completion handles |
| 5 | Generic service/operator construction |
| 6 | Core mutations |
| 7 | Bounded eager listing |
| 8 | Chunked bulk transfer |
| 9 | Structured errors |
| 10 | Metadata and operation options |
| 11 | Copy and rename |
| 12 | Stateful read streams |
| 13 | Stateful writers |
| 14 | Stateful listers |
| 15 | Explicit end-of-stream/listing state |

Bits 0 and 1 identify the memory-service and deterministic pending-poll canary
features. They are not part of the production required mask. Applications
discover the actual compiled services through `Runtime::available_schemes()`;
the current artifact registers `memory`, `opfs`, and `s3`.

Generic operator construction calls OpenDAL's facade-level default installer,
not only the service registry initializer. This installs the browser-compatible
default HTTP transport required when S3 first performs network I/O.

The machine-readable
[`contract.json`](https://github.com/Eric-Song-Nop/opendal-mbt/blob/v0.2.0/wasm/browser-runtime/contract.json) is the release and
canary mirror for values owned elsewhere. Keep these sources aligned:

- packed version and required features: Rust bridge, Promise runtime,
  generated header, and `contract.json`;
- limits and snapshot magic: Rust bridge, Promise decoder, and
  `contract.json`;
- service set: bridge Cargo features, registered schemes, and `contract.json`;
- binding version: Moon module, bridge crate, lockfile, and `contract.json`;
- wasm-bindgen version: Rust lockfile, Makefile/generator pin, generated header,
  and `contract.json`.

The Chrome canary reads selected contract values and compares the ABI version,
required feature mask, transfer-chunk limit, and structured local error codes
with the live bridge. The release checklist separately reviews the remaining
pins and mirrors listed above.

## Handle and ownership model

Every non-scalar bridge value is stored in a Rust arena and referenced by a
nonzero `u32` handle. A handle contains a slot index and generation; releasing
a value increments its slot generation so stale aliases cannot access a newer
resource. The arena permits at most 65,535 slots.

Ownership transfers are explicit:

- constructors and `*_start` exports return newly owned handles;
- `task_take` moves a ready task's result into a completion handle exactly
  once;
- `completion_take_*`, error snapshot, and list accessors move or copy results
  according to their named contract;
- every owned buffer, task, completion, error, operator, stream, writer,
  lister, and list has a matching release operation;
- JavaScript wrappers release temporary input buffers in `finally` blocks and
  track operator children and active tasks;
- `Runtime::close` closes operators, permanently tears down the bridge arena,
  and makes late task completions inert. Repeated close is harmless.

The runtime refuses to attach to a bridge with pre-existing live handles. This
detects an already active owner, but an empty cached wasm-bindgen module is not
distinguishable from a fresh module. That check is defense in depth, not a
replacement for the one-`Runtime::load`-per-bridge-module-URL rule above.

Stateful read streams, writers, and listers allow one in-flight operation per
handle. The bridge marks the resource `Busy` and attaches a monotonically
increasing operation token to the task. A second operation reports
`ResourceBusy`; a completion carrying an older token cannot change a newer
resource state.

## Promise and task lifecycle

Starting an operation copies its inputs, creates a `Pending` task handle, and
spawns the OpenDAL future on the browser's local executor. The Promise facade
polls only the scalar state:

```text
Pending --future completes--> Ready --task_take--> Consumed
    |                         |
    +------ cancel ----------+
                 |
              Cancelled
```

The four scalar states are stable for ABI 1.7:

- `Pending` (`1`): the future has not published a completion;
- `Ready` (`2`): the task exclusively owns one completion;
- `Cancelled` (`3`): cancellation won and any late result is ignored;
- `Consumed` (`4`): the completion was moved out exactly once.

When ready, JavaScript takes the completion, validates its operation kind,
turns it into an owned value or structured failure, and releases both handles.
A task release also drops any unconsumed completion it owns.

For a leased stream, writer, or lister operation, cancellation or task release
terminalizes the resource. Publication checks both the task handle and lease
token before applying a state update, so an already-running future cannot
resurrect a cancelled, closed, or reused resource.

## Cancellation boundary

The Promise facade accepts an `AbortSignal` and MoonBit's Promise adapter aborts
that signal when scheduler cancellation occurs. Cancellation is logical:

- a pending task becomes `Cancelled` and its late completion is inert;
- a ready-but-unconsumed task drops its completion;
- a stateful resource with unknown cursor or commit progress becomes terminal;
- an operation-level AbortSignal outcome is representable as structured
  `Cancelled` error kind `0x1005`.

MoonBit scheduler cancellation itself propagates as the scheduler's general
cancellation error after activating the signal; the wrapper does not relabel it
as `OpenDalError`. A structured `Cancelled` outcome received from the bridge
remains inspectable through `OpenDalError::is_cancelled()`.

Cancellation does not prove that an OpenDAL future or already-dispatched
remote request stopped at the storage service. It never rolls back bytes or
multipart state that may already be visible. Applications must design retries
of non-idempotent operations around that uncertainty.

Closing a JavaScript operator currently closes its child wrappers and cancels
tracked tasks. That is a target implementation detail, not a portable promise:
portable callers must finish or close children and await in-flight operations
before closing their parent `AsyncOperator`.

## Bounds and copying

ABI 1.7 applies hard bounds before allocation or cross-memory copy:

| Resource | Limit |
| --- | ---: |
| One owned buffer or whole-object result | 64 MiB |
| One transfer window, read-stream output, or Writer input | 256 KiB |
| Materialized list entries | 65,536 |
| Encoded materialized listing output | 16 MiB |
| Operator configuration entries | 1,024 |
| Combined UTF-8 operator configuration | 1 MiB |

The encoded listing budget includes paths, names, and metadata snapshots; the
`listUtf8Bytes` field name in `contract.json` is retained as the ABI 1.7
machine-contract key. A backend `limit` remains a request hint and cannot
raise these binding limits.

Bulk buffers are copied in windows no larger than 256 KiB. Text inputs must be
well-formed JavaScript strings and valid UTF-8 after encoding. Length
arithmetic is checked before conversion to the scalar carriers.

## Structured errors and snapshots

Storage and validation failures are ordinary resolved Promise outcomes:

```text
{ ok: true, value }
{ ok: false, error: { kind, kindCode, status, statusCode, message, ...context } }
```

Promise rejection is reserved for bootstrap and contract corruption such as a
module load failure, missing export, incompatible ABI, malformed snapshot, or
unexpected exception in the facade. The MoonBit layer translates those
failures to `AbiMismatch`; it preserves Moon scheduler cancellation.

The bridge owns two versioned binary snapshot formats:

- `ODE1` schema 1 carries error kind, retry status, kind name, and message;
- `ODM1` schema 1 carries metadata presence bits, mode, scalars, timestamp,
  and optional UTF-8 fields.

The JavaScript decoder rejects bad magic, unsupported schema, invalid presence
bits, non-canonical scalars, invalid UTF-8, truncated data, and trailing bytes.
It then attaches the initiating operation and caller paths, which do not cross
the Rust boundary. Unknown numeric kinds and statuses remain representable so
diagnostics are not silently collapsed.

Scalar ABI misuse reports failure through the bridge's owned last-error slot;
the Promise facade immediately takes or clears that error. Ordinary OpenDAL
operation errors travel in their task completion instead. Configuration
values and credentials must never be echoed in either channel.

## Embedded distribution

Normal consumers use `Runtime::new()` or `AsyncOperator::new()`. The published
Moon package contains
[`embedded_runtime.generated.mbt`](../../src/browser/embedded_runtime.generated.mbt),
which embeds three version-matched pieces:

1. the gzip-compressed Rust bridge Wasm;
2. wasm-bindgen `no-modules` glue;
3. the Promise runtime with module exports removed for inline loading.

At runtime the generated loader decodes the payload with browser
`DecompressionStream`, initializes wasm-bindgen from the bytes, validates the
ABI, and returns one runtime. No Rust, npm, bundler, CDN, or separately served
`.wasm` file is required. The host still needs `WebAssembly`,
`DecompressionStream`, and a Content Security Policy that permits Wasm
compilation, normally `script-src 'wasm-unsafe-eval'`.

The checked-in file is generated by
[`generate-browser-embed.mjs`](https://github.com/Eric-Song-Nop/opendal-mbt/blob/v0.2.0/scripts/generate-browser-embed.mjs) and
must never be edited by hand. The generator records:

- wasm-bindgen version;
- ABI version and required features extracted from the Promise runtime;
- SHA-256 values for glue, Wasm, runtime, and the normalized source set;
- a canonical gzip header and deterministic MoonBit source layout.

Rust/LLVM or zlib can produce different but valid bytes on different
maintainer hosts. `make browser-embed-check` therefore proves the checked-in
source fingerprint and generated layout, decompresses its payload, and
compares the public Wasm interface with a current rebuild. Real Chrome tests
execute both the separately built web-module form and the packaged embedded
form.

The exact wasm-bindgen CLI is pinned to `0.2.127`, matching the Rust dependency
resolved in `Cargo.lock`. Both `browser-bridge` and `browser-embed-bridge`
reject any other CLI version. Changing the bridge, Promise runtime, generator,
its declared source inputs, Rust lockfile, or tool pin requires regeneration:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.127
make browser-embed-generate
make browser-embed-check
```

Review the generated header hashes and payload diff; do not accept a stale
snapshot by removing a source from the fingerprint set.

## Validation gates

The browser contract is complete only while all of these pass:

```sh
make moon-browser-check
make moon-browser-test
make browser-rust-check
make browser-rust-test
make browser-js-canary RUST_PROFILE=release
make browser-embed-check
make browser-demo
make portable-async-example-browser
make packaged-browser
```

`browser-js-canary` serves the module-form bridge and runs task, cancellation,
snapshot, bounds, and ownership assertions in real Chrome. `browser-demo` runs
the embedded MoonBit consumer in Chrome. `portable-async-example-browser` runs
the same application-facing MoonBit code used by native, then uses an
independent cross-origin S3 fixture to require CORS and SigV4-shaped request
headers and prove that a Moon heartbeat resumes before the delayed read
completes. `packaged-browser` repeats the demo from a freshly packed module
while hiding Cargo, Rust, wasm-bindgen, npm, and common bundlers.

See the [Wasm maintainer index](https://github.com/Eric-Song-Nop/opendal-mbt/blob/v0.2.0/wasm/README.md) for file ownership and the
[release procedure](../releasing.md) for the pre-tag checklist.
