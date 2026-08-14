# Common storage tasks

The blocking core recipes also apply to the published `0.1.0` local profile.
Recipes for S3, presign, layers, batch/Copier, and async target the pinned but
unpublished `0.2.0` standard candidate and require a source standard build until
the tag publishes its assets and package. Examples use `memory` or `fs` when
behavior can be hermetic.

## Write and read a complete object

`write` accepts a `BytesView` and returns a metadata snapshot. `read` returns
an owned `Bytes` value.

```mbt check
///|
#cfg(target="native")
test "tasks: whole-object write and read" {
  let operator = @opendal.Operator::new("memory")
  let storage = b"__hello, OpenDAL__"
  let payload : BytesView = storage[2:16]

  let metadata = operator.write(
    "documents/hello.txt",
    payload,
    content_type="text/plain",
  )
  assert_eq(metadata.content_length, 14UL)
  assert_eq(operator.read("documents/hello.txt"), b"hello, OpenDAL")
}
```

The whole-object read must fit in one MoonBit `Bytes` value. Use a `ReadStream`
when the object should be consumed with a fixed per-returned-chunk output and
copy bound.

## Read byte ranges

`ByteRange` makes range semantics explicit:

```mbt check
///|
#cfg(target="native")
test "tasks: independent byte ranges" {
  let operator = @opendal.Operator::new("memory")
  operator.write("numbers.bin", b"0123456789") |> ignore

  assert_eq(
    operator.read("numbers.bin", range=Range(offset=2UL, length=4UL)),
    b"2345",
  )
  assert_eq(operator.read("numbers.bin", range=From(offset=6UL)), b"6789")
  assert_eq(operator.read("numbers.bin", range=Suffix(length=3UL)), b"789")
}
```

`Range(offset, length)` uses a byte count, not an end offset. `Suffix` requires
`operator.info().capability.can_read_suffix()`; the binding does not simulate
it with a preliminary metadata request.

## Read sequentially in bounded chunks

`open_read_stream` owns a sequential cursor. Its `chunk_size` is fixed when the
stream opens, and every returned chunk is an owned `Bytes` value no larger than
that bound. OpenDAL or the backend can still supply one larger raw buffer,
which the binding splits locally without polling upstream again until the
remainder is delivered.

```mbt check
///|
#cfg(target="native")
test "tasks: bounded sequential read" {
  let operator = @opendal.Operator::new("memory")
  operator.write("video.bin", b"0123456789") |> ignore

  let stream = operator.open_read_stream("video.bin", chunk_size=4)
  let mut bytes_read = 0
  for ;; {
    match stream.next() {
      Some(chunk) => {
        assert_true(chunk.length() <= 4)
        bytes_read += chunk.length()
      }
      None => break
    }
  }
  assert_eq(bytes_read, 10)
  assert_eq(stream.next(), None)
  stream.close()
}
```

`None` remains stable until close. Closing is idempotent; a read error is
terminal. `range`, `version`, `if_match`, and `if_none_match` use the same
semantics as whole-object reads. Some backends open lazily, so missing-object,
condition, or range errors can first appear from `next`.

## Reuse a random-access reader

An opened `Reader` does not keep a hidden cursor. Every call supplies an
independent range. Close it when it is no longer needed.

```mbt check
///|
#cfg(target="native")
test "tasks: random-access reader" {
  let operator = @opendal.Operator::new("memory")
  operator.write("archive.bin", b"abcdefghij") |> ignore

  let reader = operator.open_reader("archive.bin")
  assert_eq(reader.read(Range(offset=3UL, length=3UL)), b"def")
  assert_eq(reader.read(Full), b"abcdefghij")
  assert_eq(reader.read(Suffix(length=2UL)), b"ij")
  reader.close()
}
```

Each `Reader::read` still materializes its selected range into `Bytes`. This is
random access, not the Node.js binding's stream interface. As with whole reads,
`Suffix` requires the originating Operator's native suffix capability.

## Write in chunks

Use `open_writer` when data arrives in pieces. A write is committed only when
`finish` succeeds. Call `abort` to discard an unfinished write explicitly;
finalization does not silently finish or abort it.

```mbt check
///|
#cfg(target="native")
test "tasks: chunked writer" {
  let operator = @opendal.Operator::new("memory")
  let writer = operator.open_writer(
    "uploads/value.bin",
    content_type="application/octet-stream",
  )
  writer.write(b"first-")
  writer.write(b"second")
  let metadata = writer.finish()

  assert_eq(metadata.content_length, 12UL)
  assert_eq(operator.read("uploads/value.bin"), b"first-second")
}
```

Aborting is idempotent after its first success, and the writer rejects later
writes and finishes. In the memory example below no committed object remains.
Across backends, successful abort means OpenDAL reported cleanup success; it
does not promise rollback of remote effects already made visible.

```mbt check
///|
#cfg(target="native")
test "tasks: abort chunked writer" {
  let operator = @opendal.Operator::new("memory")
  let writer = operator.open_writer("uploads/discarded.bin")
  writer.write(b"temporary")
  writer.abort()
  writer.abort()

  assert_false(operator.exists("uploads/discarded.bin"))
}
```

A failed write or abort is also terminal. Any later operation then raises
`ResourceClosed`; `abort` after `finish` does the same.

## Check existence and metadata

`exists` returns `false` only for `NotFound`; other backend failures remain
errors. `stat` returns an immutable snapshot.

```mbt check
///|
#cfg(target="native")
test "tasks: exists and stat" {
  let operator = @opendal.Operator::new("memory")
  assert_false(operator.exists("images/logo.bin"))

  operator.write("images/logo.bin", b"\x00\x01\x02") |> ignore
  let metadata = operator.stat("images/logo.bin")
  assert_true(metadata.is_file())
  assert_false(metadata.is_dir())
  assert_eq(metadata.content_length, 3UL)
  assert_true(operator.exists("images/logo.bin"))
}
```

Optional metadata such as `etag`, `content_type`, `last_modified`, and
`version` can be absent. Do not invent a default value when a field is `None`.

## List a prefix eagerly

`list` materializes all returned entries into an array:

```mbt check
///|
#cfg(target="native")
test "tasks: eager recursive listing" {
  let operator = @opendal.Operator::new("memory")
  operator.write("logs/one.txt", b"one") |> ignore
  operator.write("logs/archive/two.txt", b"two") |> ignore

  let entries = operator.list("logs/", recursive=true)
  assert_true(entries.length() >= 2)
  assert_true(entries[0].path.has_prefix("logs/"))
}
```

`limit` is a backend request hint, not a binding-enforced total-result bound.
`start_after` and recursive listing must be guarded with the corresponding
capability methods when code moves between services.

## Stream a listing

`open_lister` avoids materializing the complete result set. `None` is a stable
end-of-stream marker; an I/O error is terminal.

```mbt check
///|
#cfg(target="native")
test "tasks: listing cursor" {
  let operator = @opendal.Operator::new("memory")
  operator.write("queue/a.bin", b"a") |> ignore
  operator.write("queue/b.bin", b"b") |> ignore

  let lister = operator.open_lister("queue/", recursive=true)
  let mut count = 0
  for ;; {
    match lister.next() {
      Some(entry) => {
        assert_true(entry.path.has_prefix("queue/"))
        count = count + 1
      }
      None => break
    }
  }
  lister.close()
  assert_eq(count, 2)
}
```

## Create directories and delete data

Directory paths end with a slash. `create_dir` creates parents recursively.
Deleting a missing path succeeds.

```mbt check
///|
#cfg(target="native")
test "tasks: directories and recursive delete" {
  let operator = @opendal.Operator::new("memory")
  operator.create_dir("workspace/nested/")
  assert_true(operator.stat("workspace/nested/").is_dir())

  operator.write("workspace/nested/value.bin", b"value") |> ignore
  operator.delete("workspace/", recursive=true)
  assert_false(operator.exists("workspace/nested/value.bin"))
  operator.delete("workspace/missing.bin")
}
```

Use `delete_many` when OpenDAL should drive a set through its high-level
deleter. The binding validates and copies all paths before starting; an empty
batch succeeds.

```mbt check
///|
#cfg(target="native")
test "tasks: all-or-error batch delete" {
  let operator = @opendal.Operator::new("memory")
  operator.write("batch/one.bin", b"one") |> ignore
  operator.write("batch/two.bin", b"two") |> ignore

  operator.delete_many([
    "batch/one.bin", "batch/missing.bin", "batch/one.bin", "batch/two.bin",
  ])
  assert_false(operator.exists("batch/one.bin"))
  assert_false(operator.exists("batch/two.bin"))
  operator.delete_many([])
}
```

`delete_many` returns `Unit`: success covers the complete request, while an
error may follow partial remote effects. It does not promise atomicity,
ordering, uniqueness, or per-path outcomes. `Capability::delete_max_size()` is
an optional backend hint, not a universal binding batch size.

## Copy and rename

Copy and rename call the backend operations directly. The binding never
emulates rename as copy followed by delete.

```mbt check
///|
#cfg(target="native")
test "tasks: filesystem copy and rename" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-doc-copy-rename-\{@env.now()}"
  let operator = @opendal.Operator::new("fs", config={ "root": root })
  let capability = operator.info().capability
  assert_true(capability.can_copy())
  assert_true(capability.can_rename())

  operator.write("source.bin", b"value") |> ignore
  operator.copy("source.bin", "copied.bin") |> ignore
  operator.rename("copied.bin", "renamed.bin")
  assert_eq(operator.read("renamed.bin"), b"value")
  operator.delete("source.bin")
  operator.delete("renamed.bin")
}
```

The memory service currently reports `can_copy() == false` and
`can_rename() == false`. Check capabilities before writing portable code.

Use a `Copier` when a backend exposes incremental progress for one object. The
source and destination are paths on the same Operator:

```mbt check
///|
#cfg(target="native")
test "tasks: managed same-Operator Copier" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-doc-copier-\{@env.now()}"
  let operator = @opendal.Operator::new("fs", config={ "root": root })
  operator.write("source.bin", b"copy through a managed resource") |> ignore

  let copier = operator.open_copier("source.bin", "destination.bin")
  for ;; {
    match copier.next() {
      Some(delta) => ignore(delta)
      None => break
    }
  }
  copier.finish() |> ignore
  let metadata = operator.stat("destination.bin")
  assert_true(metadata.is_file())
  assert_eq(
    operator.read("destination.bin"),
    b"copy through a managed resource",
  )
  operator.delete_many(["source.bin", "destination.bin"])
}
```

`None` is stable until `finish` consumes the retained destination metadata.
`abort` is explicit and idempotent after a successful abort, but it cannot
roll back remote effects already visible. A `Copier` can outlive the Operator
that opened it. It is not recursive and cannot copy between two operators or
services; memory reports it as `Unsupported` instead of emulating it.

## Create presigned HTTP requests

The standard S3 profile can return owned request snapshots for read, write,
and stat. Generating a request performs no object I/O; the application chooses
an HTTP client and sends the method, URI, and every header exactly.

```mbt check
///|
#cfg(target="native")
test "tasks: create an owned presigned read request" {
  let operator = @opendal.Operator::s3(
    "example-bucket",
    region="us-east-1",
    endpoint="http://127.0.0.1:9000",
    auth=@opendal.S3Auth::unsigned(),
  )
  let request = operator.presign_read(
    "manual/object.bin",
    expires_in_seconds=60UL,
    range=Range(offset=0UL, length=16UL),
  )

  assert_eq(request.http_method, "GET")
  assert_true(request.uri.length() > 0)
}
```

`PresignedHeader.value` is `Bytes`, not assumed UTF-8. Duplicate header names
remain meaningful and must not be collapsed accidentally. Signed URLs and
headers can contain credentials: do not print, snapshot, or attach them to
ordinary error reports. Expiry must be positive. Check
`can_presign_read()`, `can_presign_write()`, or `can_presign_stat()` before
using a portable operator.

## Add explicit operational layers

Layer methods return new Operators and leave the original untouched. No layer
is installed by default. Compose the supported policies as timeout, retry,
then the outermost concurrency limit:

```mbt check
///|
#cfg(target="native")
test "tasks: compose immutable operational layers" {
  let base = @opendal.Operator::new("memory")
  base.write("layers/value.bin", b"value") |> ignore

  let tuned = base
    .with_timeout(operation_timeout_millis=5000UL, io_timeout_millis=2000UL)
    .with_retry(
      max_retries=2U,
      min_delay_millis=10UL,
      max_delay_millis=100UL,
      jitter=false,
    )
    .with_concurrency_limit(operation_limit=8U, http_request_limit=4U)

  assert_eq(tuned.read("layers/value.bin"), b"value")
  assert_eq(base.info().scheme, tuned.info().scheme)
}
```

Duplicate layers and reverse composition raise `InvalidArgument`.
`max_retries` counts attempts after the initial request. Retry is opt-in and
does not promise exactly-once stateful writes or appends after an uncertain
remote commit. Operation permits remain held for body-style resource
lifetimes; the optional HTTP permit remains held until a response body drops.

## Use the portable async facade

`AsyncOperator::new` is the shared native/browser constructor. Call async
methods directly from an async context—there is no `await` keyword:

```mbt check
///|
async test "tasks: async writer and bounded reader" {
  let async_operator = @opendal.AsyncOperator::new("memory")
  defer async_operator.close()
  async_operator.write("async/whole.bin", b"whole") |> ignore
  assert_eq(async_operator.read("async/whole.bin"), b"whole")
  let writer = async_operator.open_writer(
    "async/value.bin",
    content_type="application/octet-stream",
  )
  writer.write(b"first-")
  writer.write(b"second")
  let metadata = writer.finish()
  assert_eq(metadata.content_length, 12UL)

  let stream = async_operator.open_read_stream("async/value.bin", chunk_size=5)
  let collected : Array[Byte] = []
  for ;; {
    match stream.next() {
      Some(chunk) => {
        assert_true(chunk.length() <= 5)
        for byte in chunk {
          collected.push(byte)
        }
      }
      None => break
    }
  }
  assert_eq(Bytes::from_array(collected), b"first-second")
  stream.close()
}
```

Native callers that need synchronous configuration or layers can still call
`Operator::as_async()` to share that configured operator. The portable slice
includes whole write, whole/ranged read, read streams and writer chunks bounded
to 256 KiB, and explicit `finish`/`abort`; whole-object buffers are bounded to
64 MiB. Streams and writers allow one in-flight operation. Cancellation of a
stateful operation makes the resource terminal when cursor or commit progress
may be unknown; remote effects are not rolled back. Resource `close` is
synchronous, idempotent, and non-raising.

## Handle typed errors

`OpenDalError` exposes a stable kind, retry-status classification, operation,
path, and optional destination path:

```mbt check
///|
#cfg(target="native")
test "tasks: inspect a typed error" {
  let operator = @opendal.Operator::new("memory")
  try operator.read("missing.bin") catch {
    error => {
      let info = error.info()
      assert_true(info.kind is NotFound)
      assert_true(info.operation is Read)
      assert_eq(info.path, Some("missing.bin"))
      assert_eq(info.destination_path, None)
      assert_false(error.is_temporary())
    }
  } noraise {
    _ => fail("expected read to raise")
  }
}
```

Temporary or persistent classification alone does not make an operation safe
to retry. The binding never installs a retry layer implicitly; callers choose
`with_retry` and accept its replay contract explicitly.

## Tasks from the Node.js guide that are not available

The following upstream recipes cannot yet be translated faithfully:

- portable native async stat, list/lister, delete, copy/Copier, presign, and
  public task-handle APIs beyond the first async slice (the JavaScript target
  already exposes capability-checked Promise extensions for several of these);
- callback adapters or Node stream compatibility;
- presigned delete and other methods beyond read/write/stat;
- ordered per-path batch results or transactional batch rollback;
- recursive or cross-Operator/cross-service Copier tasks;
- logging, tracing, metrics, custom retry observers, and other callback layers;
- recipes requiring services beyond native `memory`/`fs`/`s3` or browser
  `memory`/`opfs`/`s3`;
- recipes requiring Intel macOS, Windows, or musl release artifacts.

They are deliberate scope boundaries, not hidden APIs. The published `0.1.0`
local profile lacks all Phase 5 methods until `0.2.0` is published; this source
tree already pins the exact standard candidate artifacts.
