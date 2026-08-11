# Connecting to storage

OpenDAL separates *what* your code does from *where* the data lives. Construct
an `Operator` with a service scheme and a map of service-specific string
configuration, then use the same operator methods for every supported service.

```moonbit nocheck
///|
let operator = @opendal.Operator::new(scheme, config={ "key": "value" })
```

The configuration map is copied during construction. The binding does not
retain or mutate the caller's `Map`.

## Services available in v0.1.0

| Scheme | Purpose | Required configuration |
| --- | --- | --- |
| `memory` | Ephemeral process-local object storage | none |
| `fs` | Local filesystem rooted at one directory | `root` |

This is the complete `local` profile currently shipped in the prebuilt native
artifacts. A scheme string is not a plugin name: using `s3`, `gcs`, `azblob`,
`webdav`, or another service cannot load that backend at runtime.

## Memory

```mbt check
///|
test "connecting: memory operator" {
  let operator = @opendal.Operator::new("memory")
  operator.check()

  let info = operator.info()
  assert_eq(info.scheme, "memory")
  assert_true(info.capability.can_read())
  assert_true(info.capability.can_write())
  assert_true(info.capability.can_list())
}
```

Use memory storage for unit tests, temporary transformations, and examples.
Do not use it when data must outlive the operator or process.

## Filesystem

The `root` is the boundary beneath which OpenDAL resolves operation paths:

```mbt check
///|
test "connecting: filesystem operator" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-doc-connecting-\{@env.now()}"
  let operator = @opendal.Operator::new("fs", config={ "root": root })
  operator.check()

  let info = operator.info()
  assert_eq(info.scheme, "fs")
  assert_true(info.capability.can_read())
  assert_true(info.capability.can_write())

  operator.write("nested/value.bin", b"value") |> ignore
  assert_eq(operator.read("nested/value.bin"), b"value")
  operator.delete("nested/", recursive=true)
}
```

Application code should choose and validate the root before constructing the
operator. Keep untrusted values in operation paths rather than allowing them
to select an arbitrary host directory. OpenDAL paths use forward slashes and
are relative to the configured root.

## One operator per storage boundary

An operator's service and configuration are fixed. Construct separate
operators when an application needs separate roots:

```mbt nocheck
///|
fn open_app_stores(data_root : String, cache_root : String) raise {
  let data = @opendal.Operator::new("fs", config={ "root": data_root })
  let cache = @opendal.Operator::new("fs", config={ "root": cache_root })
  ignore(data)
  ignore(cache)
}
```

`Operator::info()` returns a snapshot containing `scheme`, normalized `root`,
backend `name`, and its `Capability` set. Check capabilities when code uses an
operation that varies by backend:

```mbt check
///|
test "connecting: inspect capabilities" {
  let operator = @opendal.Operator::new("memory")
  let capability = operator.info().capability
  assert_true(capability.can_stat())
  assert_true(capability.can_read())
  assert_true(capability.can_write())
  assert_false(capability.can_copy())
  assert_false(capability.can_rename())
}
```

## Configuration and credentials not implemented yet

The Node.js binding's connecting guide covers cloud credentials and service
options. This MoonBit release does not yet ship cloud/network services, so it
does not currently expose working S3 endpoint/access-key examples, environment
credential chains, HTTP clients, or per-service typed builders.

There is also no connection-URI constructor. Use
`Operator::new(scheme, config=...)` with the two supported schemes above.
Unknown services, invalid configuration, and unsupported options raise a typed
`OpenDalError` during construction or operation.

Cloud service profiles are tracked as deferred work in
[the roadmap](../docs/roadmap.md#phase-5-deferred-capabilities).
