# Getting started

Start with an in-memory operator, then switch the constructor while keeping the
same storage methods. The published `0.1.0` package is the local `memory`/`fs`
release. This checkout is the pinned but unpublished `0.2.0` release candidate,
containing typed S3 and the first async facade.

## 1. Add a published package

After the `v0.2.0` tag workflow publishes the standard release:

```sh
moon add Eric-Song-Nop/opendal@0.2.0
```

Until then, the registry baseline is the local release:

```sh
moon add Eric-Song-Nop/opendal@0.1.0
```

The package is native-only, so the consuming package selects the native target
and imports OpenDAL:

```moonbit nocheck
supported_targets = "native"

import {
  "Eric-Song-Nop/opendal",
}
```

The first build needs network access to download a pinned native artifact.
After verification, the artifact remains in Moon's shared cache and can be
reused offline. The published `0.1.0` matrix is Apple silicon macOS and x86-64
glibc Linux; it does not contain S3 or the Phase 5 symbols.

To exercise the pinned `0.2.0` release candidate before publication, clone this
repository and run:

```sh
make moon-deps
make test-profile NATIVE_SERVICE_PROFILE=standard
```

The maintainer path builds Rust locally. It is not an implicit fallback for a
registry consumer.

## 2. Try the memory service

An `Operator` is an accessor for one configured storage service. The `memory`
service needs no credentials and is ideal for tests and first experiments.

```mbt check
///|
test "getting started: write, stat, read, and delete" {
  let operator = @opendal.Operator::new("memory")

  let written = operator.write(
    "notes/hello.txt",
    b"Hello, OpenDAL!",
    content_type="text/plain",
  )
  assert_eq(written.content_length, 15UL)

  let metadata = operator.stat("notes/hello.txt")
  assert_true(metadata.is_file())
  assert_eq(metadata.content_type, Some("text/plain"))
  assert_eq(operator.read("notes/hello.txt"), b"Hello, OpenDAL!")

  operator.delete("notes/hello.txt")
  assert_false(operator.exists("notes/hello.txt"))
}
```

All data in a memory operator disappears with that operator. Two separately
constructed memory operators are independent stores.

## 3. Use it from a command

A minimal `cmd/main/main.mbt` can propagate checked storage errors:

```mbt nocheck
///|
fn main raise {
  let operator = @opendal.Operator::new("memory")
  operator.write("hello.txt", b"hello from MoonBit") |> ignore
  println(operator.read("hello.txt"))
}
```

Run it with the ordinary Moon command:

```sh
moon run --target native cmd/main
```

Library functions may declare `raise OpenDalError` or handle the error with
`try`/`catch`; no marker is needed at each propagating call site.

## 4. Switch to the filesystem

Storage operations stay the same; only operator construction changes:

```mbt check
///|
test "getting started: filesystem round trip" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-doc-getting-started-\{@env.now()}"
  let operator = @opendal.Operator::new("fs", config={ "root": root })

  operator.write("hello.txt", b"persisted on disk") |> ignore
  assert_eq(operator.read("hello.txt"), b"persisted on disk")
  operator.delete("hello.txt")
}
```

Paths passed to operations are relative to the configured filesystem `root`.
See [Connecting](connecting.mbt.md) before accepting a root from untrusted
input.

## 5. Choose blocking or async I/O

The existing `Operator` methods remain synchronous. In the `v0.2.0`
release-candidate facade, `as_async()` creates a lightweight view over the same
configured operator. Async methods are called normally from an `async fn` or
`async test`—MoonBit does not use an `await` keyword.

```mbt check
///|
async test "getting started: bounded async stream" {
  let operator = @opendal.Operator::new("memory")
  operator.write("async/data.bin", b"0123456789") |> ignore

  let stream = operator
    .as_async()
    .open_read_stream("async/data.bin", chunk_size=4)
  let mut total = 0
  for ;; {
    match stream.next() {
      Some(chunk) => {
        assert_true(chunk.length() <= 4)
        total += chunk.length()
      }
      None => break
    }
  }
  stream.close()
  assert_eq(total, 10)
}
```

The first async slice covers whole/ranged reads, bounded read streams, and
chunked writers with explicit finish/abort. It does not mirror every blocking
method yet. Cancellation of an in-flight stateful stream or writer operation
makes that resource terminal when progress or commit status may be unknown.

Continue with [Connecting](connecting.mbt.md) for profile and typed S3 setup,
then use the recipes in [Common tasks](tasks.mbt.md).
