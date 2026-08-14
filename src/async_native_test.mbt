///|
async test "native async memory: bounded read and stream preserve ranges" {
  let operator = @opendal.Operator::new("memory")
  operator.write("async/read.bin", b"0123456789") |> ignore
  let async_operator = operator.as_async()
  assert_eq(
    async_operator.read("async/read.bin", range=Range(offset=2UL, length=5UL)),
    b"23456",
  )
  assert_eq(
    async_operator.read("async/read.bin", range=Suffix(length=3UL)),
    b"789",
  )
  let stream = async_operator.open_read_stream(
    "async/read.bin",
    range=From(offset=1UL),
    chunk_size=3,
  )
  let collected : Array[Byte] = []
  for ;; {
    match stream.next() {
      Some(chunk) => {
        assert_true(chunk.length() <= 3)
        for byte in chunk {
          collected.push(byte)
        }
      }
      None => break
    }
  }
  assert_eq(Bytes::from_array(collected), b"123456789")
  assert_eq(stream.next(), None)
  assert_eq(stream.next(), None)
  stream.close()
  stream.close()
}

///|
async test "native async memory: writer finish and abort are explicit" {
  let operator = @opendal.Operator::new("memory")
  let async_operator = operator.as_async()
  let writer = async_operator.open_writer(
    "async/writer.bin",
    content_type="application/octet-stream",
  )
  let first_storage = b"__moonbit-__"
  writer.write(first_storage[2:10])
  writer.write(b"writer")
  let metadata = writer.finish()
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, 14UL)
  assert_eq(metadata.content_type, Some("application/octet-stream"))
  assert_eq(operator.read("async/writer.bin"), b"moonbit-writer")

  let aborted_path = "async/aborted.bin"
  let aborted = async_operator.open_writer(aborted_path)
  aborted.write(b"discard-me")
  aborted.abort()
  aborted.abort()
  assert_false(operator.exists(aborted_path))
}

///|
fn async_has_entry(entries : Array[@opendal.Entry], path : String) -> Bool {
  entries.search_by(fn(entry) { entry.path == path }) is Some(_)
}

///|
async test "native async memory: core operations preserve snapshots and context" {
  let operator = @opendal.Operator::new("memory")
  let async_operator = operator.as_async()
  async_operator.check()
  assert_false(async_operator.exists("async/core/file.bin"))
  async_operator.create_dir("async/core/")
  let storage = b"__payload__"
  let metadata = async_operator.write(
    "async/core/file.bin",
    storage[2:9],
    content_type="application/octet-stream",
  )
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, 7UL)
  assert_eq(metadata.content_type, Some("application/octet-stream"))
  assert_true(async_operator.exists("async/core/file.bin"))
  let stat = async_operator.stat("async/core/file.bin")
  assert_true(stat.is_file())
  assert_eq(stat.content_length, 7UL)
  let entries = async_operator.list("async/core/", recursive=true)
  assert_true(async_has_entry(entries, "async/core/file.bin"))
  for entry in entries {
    if entry.path == "async/core/file.bin" {
      assert_eq(entry.name, "file.bin")
      assert_true(entry.is_file())
      assert_false(entry.is_dir())
      assert_eq(entry.metadata.content_length, 7UL)
    }
  }
  try
    async_operator.copy("async/core/file.bin", "async/core/copied.bin")
  catch {
    @opendal.OpenDalError(info) => {
      assert_true(info.kind is Unsupported)
      assert_true(info.operation is Copy)
      assert_eq(info.path, Some("async/core/file.bin"))
      assert_eq(info.destination_path, Some("async/core/copied.bin"))
    }
    _ => fail("expected an OpenDalError from native memory async copy")
  } noraise {
    _ => fail("expected native memory async copy to remain unsupported")
  }
  try
    async_operator.rename("async/core/file.bin", "async/core/renamed.bin")
  catch {
    @opendal.OpenDalError(info) => {
      assert_true(info.kind is Unsupported)
      assert_true(info.operation is Rename)
      assert_eq(info.path, Some("async/core/file.bin"))
      assert_eq(info.destination_path, Some("async/core/renamed.bin"))
    }
    _ => fail("expected an OpenDalError from native memory async rename")
  } noraise {
    _ => fail("expected native memory async rename to remain unsupported")
  }
  async_operator.delete("async/core/", recursive=true)
  assert_false(async_operator.exists("async/core/file.bin"))
}

///|
async test "native async fs: copy and rename use backend-native operations" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-mbt-async-fs-copy-rename-\{@env.now()}"
  let operator = @opendal.Operator::new("fs", config={ "root": root })
  let async_operator = operator.as_async()
  async_operator.create_dir("moves/")
  async_operator.write("moves/source.bin", b"copy-rename") |> ignore
  async_operator.copy("moves/source.bin", "moves/copied.bin") |> ignore
  assert_eq(async_operator.read("moves/copied.bin"), b"copy-rename")
  async_operator.rename("moves/copied.bin", "moves/renamed.bin")
  assert_false(async_operator.exists("moves/copied.bin"))
  assert_eq(async_operator.read("moves/renamed.bin"), b"copy-rename")
  try async_operator.copy("moves/missing.bin", "moves/target.bin") catch {
    @opendal.OpenDalError(info) => {
      assert_true(info.kind is NotFound)
      assert_true(info.operation is Copy)
      assert_eq(info.path, Some("moves/missing.bin"))
      assert_eq(info.destination_path, Some("moves/target.bin"))
    }
    _ => fail("expected an OpenDalError from missing async copy source")
  } noraise {
    _ => fail("expected missing async copy source to raise")
  }
  async_operator.delete("moves/", recursive=true)
}
