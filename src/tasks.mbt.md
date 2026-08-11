# Common storage tasks

These recipes use the synchronous API shipped in `0.1.0`. Unless a recipe is
specifically about filesystem behavior, it uses `memory` so the example is
hermetic.

## Write and read a complete object

`write` accepts a `BytesView` and returns a metadata snapshot. `read` returns
an owned `Bytes` value.

```mbt check
///|
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
when the object should be consumed with a fixed per-chunk memory bound.

## Read byte ranges

`ByteRange` makes range semantics explicit:

```mbt check
///|
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

`Range(offset, length)` uses a byte count, not an end offset.

## Read sequentially in bounded chunks

`open_read_stream` owns a sequential cursor. Its `chunk_size` is fixed when the
stream opens, and every returned chunk is an owned `Bytes` value no larger than
that bound.

```mbt check
///|
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
semantics as whole-object reads.

## Reuse a random-access reader

An opened `Reader` does not keep a hidden cursor. Every call supplies an
independent range. Close it when it is no longer needed.

```mbt check
///|
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
random access, not the Node.js binding's stream interface.

## Write in chunks

Use `open_writer` when data arrives in pieces. A write is committed only when
`finish` succeeds. Call `abort` to discard an unfinished write explicitly;
finalization does not silently finish or abort it.

```mbt check
///|
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

Aborting is idempotent after its first success. It leaves no committed object,
and the writer rejects later writes and finishes.

```mbt check
///|
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

The binding has no batch-delete API. Delete multiple independent paths with
ordinary MoonBit control flow and decide explicitly how partial failure should
be handled.

## Copy and rename

Copy and rename call the backend operations directly. The binding never
emulates rename as copy followed by delete.

```mbt check
///|
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

## Handle typed errors

`OpenDalError` exposes a stable kind, retry-status classification, operation,
path, and optional destination path:

```mbt check
///|
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
to retry. The current binding does not install a retry layer.

## Tasks from the Node.js guide that are not available

The following upstream recipes cannot yet be translated faithfully:

- async/Promise operations, callback integration, and cancellation;
- Node stream adapters and a sequential large-object reader;
- presigned read, write, and stat requests;
- batch deletion;
- recursive copier/task APIs;
- layers for retry, timeout, tracing, metrics, and concurrency limits;
- recipes that require cloud/network services not present in the `local`
  profile.

They are intentional roadmap gaps, not hidden or undocumented APIs.
