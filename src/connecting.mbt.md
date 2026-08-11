# Connecting to storage

OpenDAL separates *what* code does from *where* data lives. Construct one
`Operator` for a storage boundary, inspect its capabilities, then use the same
resource-oriented methods wherever the selected backend supports them.

## Profiles are compile-time distribution choices

A scheme name is not a plugin loader. The native archive fixes the available
services before a MoonBit program starts:

| Profile | Services | Availability |
| --- | --- | --- |
| `local` | `memory`, `fs` | Published and selected by `0.1.0` |
| `standard` | `memory`, `fs`, `s3` | Pinned by the unpublished `0.2.0` release candidate; tag and registry publication pending |

There is no public runtime profile selector. A package release selects exactly
one compatible artifact table. Passing `"s3"` to the published `0.1.0` local
archive cannot download or enable S3 dynamically.

The generic constructor remains useful for compiled services and advanced
string options:

```moonbit nocheck
///|
let operator = @opendal.Operator::new(scheme, config={ "key": "value" })
```

The configuration map is copied during construction. For S3, prefer the typed
constructor so credentials remain inside opaque values and invalid combinations
are rejected before crossing the native boundary.

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

Use memory storage for unit tests and temporary transformations. Its data does
not outlive the operator or process.

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

Application code chooses and validates the root before constructing the
operator. Keep untrusted values in operation paths rather than allowing them
to select an arbitrary host directory. OpenDAL paths use forward slashes and
are relative to the configured root.

## Typed S3 in the standard `v0.2.0` release candidate

`Operator::s3` requires a bucket and region. Root, endpoint, authentication,
and virtual-host style are labelled options. All strings and credential bytes
are copied while constructing the operator.

The following checked example constructs a path-style operator for a local
S3-compatible endpoint but performs no network request:

```mbt check
///|
test "connecting: typed S3 operator" {
  let auth = @opendal.S3Auth::static_credentials(
    access_key_id="example-access-key",
    secret_access_key="example-secret-key",
  )
  let operator = @opendal.Operator::s3(
    "example-bucket",
    region="us-east-1",
    root="moonbit/",
    endpoint="http://127.0.0.1:9000",
    auth~,
  )

  let info = operator.info()
  assert_eq(info.scheme, "s3")
  assert_true(info.capability.can_read())
}
```

For AWS, omitting `auth` selects the standard credential chain. The other
policies are explicit and opaque—none derives `Debug` or `Show`:

```mbt check
///|
test "connecting: typed S3 authentication policies" {
  let default_chain = @opendal.S3Auth::default_chain(disable_ec2_metadata=true)
  let session = @opendal.S3Auth::static_credentials(
    access_key_id="example-access-key",
    secret_access_key="example-secret-key",
    session_token="example-session-token",
  )
  let source = @opendal.S3CredentialSource::static_credentials(
    access_key_id="source-access-key",
    secret_access_key="source-secret-key",
  )
  let assumed = @opendal.S3Auth::assume_role(
    role_arn="arn:aws:iam::123456789012:role/example",
    source~,
    role_session_name="opendal-moonbit",
    duration_seconds=900UL,
  )
  let unsigned = @opendal.S3Auth::unsigned()

  ignore(default_chain)
  ignore(session)
  ignore(assumed)
  ignore(unsigned)
}
```

Use unsigned mode only for a public bucket or an endpoint deliberately
configured for anonymous access. The first typed API has no connection-URI
parser or named-profile string; named profile/environment precedence remains
inside the native default credential chain.

`operator.check()` performs backend I/O. The repository's integration suite
runs it against a pinned MinIO image with ephemeral credentials; documentation
examples do not assume a local S3 server.

## One operator per storage boundary

An operator's service and configuration are immutable. Construct separate
operators for separate roots or buckets. Layer methods return new operators;
they do not mutate the original or resources already opened from it.

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
backend `name`, and its `Capability` set. Check capabilities for operations
that vary by backend or configuration:

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
  assert_false(capability.can_presign_read())
}
```

ABI feature presence and backend capability are distinct. For example, the
standard ABI can expose `open_copier` while memory correctly raises
`Unsupported` for that operation.

## Host and service limits

The published local release supports Apple silicon macOS and x86-64 glibc
Linux. The pinned but unpublished `v0.2.0` standard candidate adds
target-native Linux arm64. Intel macOS is not advertised because the current
MoonBit installer has no matching CLI; Windows and musl need separate build,
link, and clean-consumer work. GCS, Azure Blob, WebDAV, and other OpenDAL
services are not part of the standard profile.

See [Common tasks](tasks.mbt.md) for presigned requests, explicit layers,
batch deletion, Copier, and async I/O.
