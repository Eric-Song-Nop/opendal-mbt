# Getting started

This guide follows the same path as OpenDAL's other language bindings: install
the package, start with an in-memory operator, then switch to a real backend
without changing the storage operations.

## 1. Add the package

From a MoonBit module:

```sh
moon add Eric-Song-Nop/opendal
```

The package is native-only, so the consuming package must select the native
target and import OpenDAL:

```moonbit nocheck
supported_targets = "native"

import {
  "Eric-Song-Nop/opendal",
}
```

The first build needs network access to download a pinned native artifact.
After that artifact is verified, it is kept in Moon's shared cache and can be
reused offline.

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
constructed memory operators must not be treated as the same persistent store.

## 3. Use it from a command

A minimal `cmd/main/main.mbt` can use the same API:

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

Storage methods such as `read` and `write` can raise `OpenDalError`. A `main
raise` function lets the error reach Moon's top-level error handling; library
code will usually catch it or declare the same checked error.

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

## Synchronous API today

The current binding deliberately exposes only OpenDAL's blocking operations.
There is no MoonBit async variant yet, so a storage call occupies the calling
thread until it completes. This is usually fine for `memory` and local `fs`,
but it is one reason cloud/network services are not enabled in the current
profile.

Continue with [Connecting](connecting.mbt.md), then use the recipes in
[Common tasks](tasks.mbt.md).
