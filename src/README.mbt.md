# Apache OpenDAL for MoonBit

OpenDAL provides one storage API over different storage services. This package
is the native MoonBit binding: it presents an ordinary MoonBit package surface,
uses checked MoonBit errors, and keeps the Rust and C ABI behind the package.

## Status

`Eric-Song-Nop/opendal@0.1.0` is published on mooncakes.io. The current
`local` service profile is synchronous and supports:

- in-memory (`memory`) and local-filesystem (`fs`) operators;
- whole and ranged reads, plus reusable random-access readers;
- whole and chunked writes;
- metadata, existence checks, directories, deletion, and listing;
- backend-native copy and rename when the selected service reports support;
- typed `OpenDalError` values and capability inspection.

The public API is generated in [`src/pkg.generated.mbti`](pkg.generated.mbti).

## Installation

Add the dependency like any other Moon package:

```sh
moon add Eric-Song-Nop/opendal
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
later builds reuse Moon's shared cache.

The initial binary matrix supports:

- Apple silicon macOS 11 or newer;
- x86-64 Linux with glibc 2.35 or newer.

The current Moon native prebuild hook also requires Node.js 18 or newer and
`tar` at build time. Consumers do not need Rust, Cargo, or this repository.

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

## Guides

- [Getting started](getting-started.mbt.md) — install the package and run the
  first memory and filesystem programs.
- [Connecting](connecting.mbt.md) — construct and validate operators for the
  service profiles available today.
- [Common tasks](tasks.mbt.md) — recipes for reads, writes, metadata, listing,
  directories, copy, rename, and errors.

## Current limitations

The upstream Node.js binding documents a broader OpenDAL surface. Version
`0.1.0` of this MoonBit binding does **not** yet provide:

- asynchronous APIs, callbacks, cancellation, or concurrent task handles;
- S3, GCS, Azure Blob, WebDAV, or other cloud/network service profiles;
- presigned requests;
- layers such as retry, logging, metrics, timeout, or concurrency limiting;
- batch delete or long-running recursive copier APIs;
- a streaming read cursor for objects larger than one MoonBit `Bytes` value;
- a public Writer abort method;
- Windows or additional CPU/libc release artifacts.

Passing a service name does not dynamically install it: available services are
fixed by the native artifact's compiled profile. Unsupported services and
options raise `OpenDalError` instead of silently falling back.

These gaps are recorded in [the project roadmap](../docs/roadmap.md#phase-5-deferred-capabilities).

## Upstream references

The organization of these guides follows Apache OpenDAL's official Node.js
binding documentation:

- [Node.js binding overview](https://opendal.apache.org/docs/bindings/nodejs/)
- [Getting started](https://opendal.apache.org/docs/bindings/nodejs/getting-started/)
- [Connecting](https://opendal.apache.org/docs/bindings/nodejs/connecting/)
- [Common tasks](https://opendal.apache.org/docs/bindings/nodejs/tasks/)

The examples and support claims here are intentionally rewritten for the
smaller MoonBit `0.1.0` surface.
