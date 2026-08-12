# Apache OpenDAL for MoonBit

OpenDAL provides one storage API over different storage services. This native
binding presents an ordinary MoonBit package: behavior lives on resource types
such as `Operator`, `ReadStream`, and `Writer`; fallible synchronous methods use
checked errors; asynchronous I/O uses MoonBit `async fn`; Rust and the C ABI
stay private.

## Release and source status

The published `0.1.0` package and this pinned but unpublished `0.2.0` release
candidate are intentionally different until the release tag is cut:

| Track | Services and API | Native hosts |
| --- | --- | --- |
| Published `Eric-Song-Nop/opendal@0.1.0` | `local` profile: `memory`, `fs`, synchronous Phase 1-4 API | Apple silicon macOS 11+, x86-64 glibc Linux 2.35+ |
| Pinned `0.2.0` release candidate (not published) | `standard` profile: `memory`, `fs`, typed `s3`, complete Phase 5A-E API | Apple silicon macOS 11+, x86-64 and arm64 glibc Linux 2.35+ |

This source tree selects the fully pinned `native/artifacts-standard.json`
table for `0.2.0-r1`. The immutable `v0.1.0-r1` local records remain unchanged
in `native/artifacts.json`. The standard URLs name future `v0.2.0` release
assets and are not a publication claim: `moon add
Eric-Song-Nop/opendal@0.1.0` still provides only the released local API until
the tag workflow publishes `0.2.0`.

The current source facade implements:

- whole, ranged, random-access, and output-bounded sequential reads;
- whole and chunked writes with explicit `finish` or `abort`;
- metadata, existence, directories, listing, delete, copy, and rename;
- typed S3 construction with default-chain, static/session, unsigned, and
  assume-role authentication;
- owned presigned read/write/stat requests;
- immutable timeout, retry, and concurrency-limit layers, all opt-in;
- all-or-error `delete_many` and a managed same-Operator, one-object `Copier`;
- an initial async facade for read, bounded read streams, and chunked writers;
- typed `OpenDalError` values and capability inspection.

The generated public interface is
[`src/pkg.generated.mbti`](pkg.generated.mbti).

## Install a release

After the `v0.2.0` tag workflow publishes the standard release, add it like any
other Moon package:

```sh
moon add Eric-Song-Nop/opendal@0.2.0
```

Until that tag is published, the available registry baseline remains:

```sh
moon add Eric-Song-Nop/opendal@0.1.0
```

Import it from a native package:

```moonbit nocheck
supported_targets = "native"

import {
  "Eric-Song-Nop/opendal",
}
```

No OpenDAL-specific linker flags are required. On the first native build, the
package downloads and verifies the release artifact for the current host;
later builds reuse Moon's shared cache. The current prebuild hook requires
Node.js 18 or newer and `tar` at build time. Consumers do not need Rust, Cargo,
or this repository.

Contributors testing the Phase 5 source stack use the checked-out repository:

```sh
make moon-deps
make test-profile NATIVE_SERVICE_PROFILE=standard
```

That maintainer path builds and overlays a standard archive from source for
validation; it neither rewrites the committed `v0.2.0` standard pins nor
changes the immutable `v0.1.0` local table.

## First operation

`memory` is the quickest way to try the API because it needs no configuration:

```mbt check
///|
test "README: memory round trip" {
  let operator = @opendal.Operator::new("memory")
  operator.write("hello.txt", b"hello from MoonBit") |> ignore
  assert_eq(operator.read("hello.txt"), b"hello from MoonBit")
}
```

Methods that can fail use MoonBit's checked-error effect:

```mbt check
///|
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
  let operator = @opendal.Operator::new("memory")
  let async_operator = operator.as_async()
  let writer = async_operator.open_writer("async.txt")
  writer.write(b"hello ")
  writer.write(b"asynchronously")
  writer.finish() |> ignore
  assert_eq(async_operator.read("async.txt"), b"hello asynchronously")
}
```

## Guides

- [Getting started](getting-started.mbt.md) — install a published release or
  exercise the pinned `v0.2.0` candidate, then choose synchronous or async I/O.
- [Connecting](connecting.mbt.md) — understand profiles and construct memory,
  filesystem, and typed S3 operators.
- [Common tasks](tasks.mbt.md) — recipes for streams, abort, presign, layers,
  batch deletion, Copier, async I/O, and the blocking core.
- [Roadmap](../docs/roadmap.md) — completed Phase 5 slices and the remaining
  release gates.

## Deliberate limits

- The current source enables only `memory`, `fs`, and `s3`; GCS, Azure Blob,
  WebDAV, and other OpenDAL services are not compiled in.
- `delete_many` is all-or-error and can have partial remote effects; it does
  not fabricate ordered per-path results or promise atomicity.
- `Copier` copies one object between two paths on the same Operator. It is not
  recursive and cannot bridge two operators or services.
- `ReadStream.chunk_size` hard-bounds each returned value and binding copy, but
  OpenDAL or a backend may provide and retain one larger raw buffer internally.
- Suffix ranges require `capability.can_read_suffix()`; the binding does not
  emulate suffix reads with a preliminary `stat`.
- The first async slice covers read, bounded read streams, and chunked writers;
  async stat/list/delete/copy/presign and public task handles remain later
  work.
- Retry, timeout, and concurrency limits are never implicit. Logging, tracing,
  metrics exporters, and custom callback layers are not exposed.
- Standard artifacts are pinned for the `v0.2.0` release candidate but are not
  yet published. Intel macOS, Windows, and musl Linux are not advertised; a
  Rust-only build is not treated as MoonBit host support.

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
