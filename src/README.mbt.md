# Apache OpenDAL for MoonBit

OpenDAL provides one storage API over different storage services. This binding
presents one ordinary MoonBit package on native and JavaScript targets:
behavior lives on resource types such as `Operator`, `AsyncReadStream`, and
`AsyncWriter`; asynchronous I/O uses MoonBit `async fn`; the native ABI and the
embedded browser runtime stay private.

## Release and source status

Version `0.2.0` is the current release line. Version `0.1.0` remains an
immutable compatibility baseline for applications that only need the original
native `memory`/`fs` surface:

| Track | Services and API | Native hosts |
| --- | --- | --- |
| Current `Eric-Song-Nop/opendal@0.2.0` | `standard` profile: `memory`, `fs`, typed `s3`, complete Phase 5A-E API, and Browser JS | Apple silicon macOS 11+, x86-64 and arm64 glibc Linux 2.35+ |
| Legacy `Eric-Song-Nop/opendal@0.1.0` | `local` profile: `memory`, `fs`, synchronous Phase 1-4 API | Apple silicon macOS 11+, x86-64 glibc Linux 2.35+ |

This source tree selects the fully pinned `native/artifacts-standard.json`
table for `0.2.0-r2`. The immutable `v0.1.0-r1` local records remain unchanged
in `native/artifacts.json`.

The current source facade implements:

- whole, ranged, random-access, and output-bounded sequential reads;
- whole and chunked writes with explicit `finish` or `abort`;
- metadata, existence, directories, listing, delete, copy, and rename;
- typed S3 construction with default-chain, static/session, unsigned, and
  assume-role authentication;
- owned presigned read/write/stat requests;
- immutable timeout, retry, and concurrency-limit layers, all opt-in;
- all-or-error `delete_many` and a managed same-Operator, one-object `Copier`;
- one root-package async facade for whole writes and reads, bounded read
  streams, and explicit streaming writers on native and browser JS targets;
- an embedded browser OpenDAL Wasm runtime with additional async operations
  and listers;
- typed `OpenDalError` values and capability inspection.

The checked-in native public interface is
[`src/pkg.generated.mbti`](pkg.generated.mbti). Use `moon ide doc --target js
'@opendal'` for the target-specific browser surface.

## Install a release

Add the standard release like any other Moon package:

```sh
moon add Eric-Song-Nop/opendal@0.2.0
```

Use the legacy release only when an application intentionally targets its
smaller native-only compatibility surface:

```sh
moon add Eric-Song-Nop/opendal@0.1.0
```

Import the same root package from native and JavaScript packages:

```moonbit nocheck
supported_targets = "+native+js"

import {
  "Eric-Song-Nop/opendal",
}
```

No OpenDAL-specific linker flags are required. Native remains the default
target; pass `--target js` for a browser build. On the first native build, the
package downloads and verifies the release artifact for the current host;
later builds reuse Moon's shared cache. JavaScript builds use the embedded
browser runtime and do not download a native archive. The current prebuild hook
requires Node.js 18 or newer; native builds also require `tar`. Consumers do
not need Rust, Cargo, or this repository.

Only Moon's `native` and `js` targets are supported. `wasm` and `wasm-gc` are
not browser aliases for this binding; browser applications compile to the JS
target, which hosts the embedded OpenDAL core Wasm runtime. Portable native
async operations run as Rust Tokio tasks and wake Moon through a nonblocking
completion pipe; browser operations use Promises and `AbortSignal`.

Contributors validating changes from a source checkout use:

```sh
make moon-deps
make test-profile NATIVE_SERVICE_PROFILE=standard
```

That maintainer path builds and overlays a standard archive from source for
validation; it neither rewrites the committed `v0.2.0` standard pins nor
changes the immutable `v0.1.0` local table.

## Browser quickstart

From either this repository root or the root of the unpacked Moon package, one
Moon command compiles the published demo and runs its OpenDAL round trip in a
real headless Chrome or Chromium:

```sh
moon run --target js --release src/browser_demo
```

The command needs MoonBit, Node.js 18 or newer, and an installed Chrome or
Chromium (`CHROME_BIN` can select it). It needs no Rust toolchain, npm package,
JavaScript bundler, CDN, or separately served Wasm asset.

Browser consumers use the same root import as native consumers:

```moonbit nocheck
supported_targets = "js"

import {
  "Eric-Song-Nop/opendal",
  "moonbitlang/async",
}
```

Inside an async context, `AsyncOperator::new` initializes the target-specific
backend. On JavaScript it lazily boots the version-matched runtime embedded in
the Moon package:

```moonbit nocheck
///|
async fn browser_round_trip() {
  let storage = @opendal.AsyncOperator::new("memory")
  defer storage.close()
  storage.write("hello.txt", b"hello from Chrome") |> ignore
  assert_eq(storage.read("hello.txt"), b"hello from Chrome")
}
```

The compatibility `Eric-Song-Nop/opendal/browser` import and explicit
`Runtime::load(BrowserAssets)` remain available for applications that manage
version-matched runtime, glue, and Wasm URLs themselves. Maintainers refresh
the checked-in embedded source with `make browser-embed-generate` and validate
it with `make browser-embed-check`; the check verifies the compressed payload,
its source fingerprint, exported Wasm interface, generated glue, and Promise
runtime. CI then executes both the freshly rebuilt bridge and the packaged
embedded bridge in real Chrome. A browser host page's Content Security Policy
must allow WebAssembly compilation, normally by including
`script-src 'wasm-unsafe-eval'`; the published demo sets this policy itself.
See [Using OpenDAL in a browser](browser-guide.mbt.md) for OPFS and browser S3
configuration, CORS and hosting requirements, JS-only operations, runtime
ownership, cancellation, and hard bridge limits. The
[`examples/browser`](../examples/browser/README.md) program runs shared
portable async code on native and in real Chrome. Its Browser command and
native verification target also run the same delayed S3 heartbeat proof that
pending OpenDAL I/O does not block MoonBit's scheduler.

## First operation

`memory` is the quickest way to try the API because it needs no configuration:

```mbt check
///|
#cfg(target="native")
test "README: memory round trip" {
  let operator = @opendal.Operator::new("memory")
  operator.write("hello.txt", b"hello from MoonBit") |> ignore
  assert_eq(operator.read("hello.txt"), b"hello from MoonBit")
}
```

Methods that can fail use MoonBit's checked-error effect:

```mbt check
///|
#cfg(target="native")
test "README: typed storage error" {
  let operator = @opendal.Operator::new("memory")
  try operator.read("missing.txt") catch {
    error => {
      assert_true(error.kind() is NotFound)
      assert_true(error.info().operation is Read)
      assert_eq(error.info().path, Some("missing.txt"))
    }
  } noraise {
    _ => fail("expected a missing-object error")
  }
}
```

Async methods are called directly inside an async context; MoonBit has no
`await` keyword:

```mbt check
///|
async test "README: async memory round trip" {
  let async_operator = @opendal.AsyncOperator::new("memory")
  defer async_operator.close()
  async_operator.write("whole.txt", b"whole object") |> ignore
  assert_eq(async_operator.read("whole.txt"), b"whole object")
  let writer = async_operator.open_writer("async.txt")
  writer.write(b"hello ")
  writer.write(b"asynchronously")
  writer.finish() |> ignore
  assert_eq(async_operator.read("async.txt"), b"hello asynchronously")
}
```

MoonBit async catch clauses receive the target-neutral `Error` type. Use
`OpenDalError::from_error(error)` to recover the structured category, status,
operation, and path on either target.

## Guides

- [Documentation index](../docs/README.md) — user guides, target-specific API
  reference, architecture contracts, release procedure, and examples.
- [Getting started](getting-started.mbt.md) — install `v0.2.0` or validate a
  source checkout, then choose synchronous or async I/O.
- [Connecting](connecting.mbt.md) — understand profiles and construct memory,
  filesystem, and typed S3 operators.
- [Using OpenDAL in a browser](browser-guide.mbt.md) — select the JS target,
  use the embedded runtime, configure OPFS or S3, and deploy it safely.
- [Common tasks](tasks.mbt.md) — recipes for streams, abort, presign, layers,
  batch deletion, Copier, async I/O, and the blocking core.
- [Roadmap](../docs/roadmap.md) — completed Phase 5 slices and the remaining
  release gates.

## Deliberate limits

- Native source builds enable `memory`, `fs`, and `s3`; browser JS builds
  enable `memory`, `opfs`, and `s3`. GCS, Azure Blob, WebDAV, and other OpenDAL
  services are not compiled in.
- `delete_many` is all-or-error and can have partial remote effects; it does
  not fabricate ordered per-path results or promise atomicity.
- `Copier` copies one object between two paths on the same Operator. It is not
  recursive and cannot bridge two operators or services.
- `ReadStream.chunk_size` hard-bounds each returned value and binding copy, but
  OpenDAL or a backend may provide and retain one larger raw buffer internally.
- Suffix ranges require `capability.can_read_suffix()`; the binding does not
  emulate suffix reads with a preliminary `stat`.
- The portable async slice covers whole writes and reads, bounded read streams,
  and chunked writers. Whole-object buffers are limited to 64 MiB and stream
  and writer chunks to 256 KiB on both targets. Native async
  stat/list/delete/copy/presign and public task handles remain later work; the
  JavaScript target additionally exposes its capability-checked Promise
  operations and stateful listers.
- Retry, timeout, and concurrency limits are never implicit. Logging, tracing,
  metrics exporters, and custom callback layers are not exposed.
- Standard artifacts are published for `v0.2.0`. Intel macOS, Windows, and
  musl Linux are not advertised; a Rust-only build is not treated as MoonBit
  host support.

Passing an arbitrary scheme does not install a backend. ABI feature presence,
selected-profile services, and a backend's operation capabilities are separate
checks; unsupported services and operations raise `OpenDalError` instead of
silently falling back.

## Upstream references

The organization of these guides follows Apache OpenDAL's official Node.js
binding documentation while keeping the smaller MoonBit contracts explicit:

- [Node.js binding overview](https://opendal.apache.org/docs/bindings/nodejs/)
- [Getting started](https://opendal.apache.org/docs/bindings/nodejs/getting-started/)
- [Connecting](https://opendal.apache.org/docs/bindings/nodejs/connecting/)
- [Common tasks](https://opendal.apache.org/docs/bindings/nodejs/tasks/)
