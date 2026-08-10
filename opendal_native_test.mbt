///|
fn has_entry(entries : Array[@opendal.Entry], path : String) -> Bool {
  entries.search_by(fn(entry) { entry.path == path }) is Some(_)
}

///|
test "native memory: binary whole-object roundtrip" {
  let operator = @opendal.Operator::new("memory")
  let payload : Bytes = [0x00, 0x01, 0x7f, 0x80, 0xff, 0x00, 0x42]
  let metadata = operator.write("binary/embedded-nul.bin", payload)
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, payload.length().to_uint64())
  assert_eq(operator.read("binary/embedded-nul.bin"), payload)
  let info = operator.info()
  assert_eq(info.scheme, "memory")
  assert_true(info.capability.can_read())
  assert_true(info.capability.can_write())
}

///|
test "native memory: StringView and BytesView use only the selected slice" {
  let operator = @opendal.Operator::new("memory")
  let path_storage = "__slice.bin__"
  let path : StringView = path_storage[2:11]
  let data_storage = b"__payload__"
  let data : BytesView = data_storage[2:9]
  let metadata = operator.write(path, data)
  assert_eq(metadata.content_length, 7UL)
  assert_eq(operator.read(path), b"payload")
}

///|
test "native memory: missing path preserves typed context" {
  let operator = @opendal.Operator::new("memory")
  try operator.read("missing.bin") catch {
    error => {
      let info = error.info()
      assert_true(info.kind is NotFound)
      assert_true(info.operation is Read)
      assert_eq(info.path, Some("missing.bin"))
      assert_eq(info.destination_path, None)
      assert_true(error.kind() is NotFound)
      assert_true(error.status() is Permanent)
      assert_false(error.is_temporary())
      assert_false(error.is_persistent())
      assert_true("\{error}".has_prefix("OpenDAL NotFound during Read:"))
    }
  } noraise {
    _ => fail("expected missing read to raise")
  }
}

///|
test "native memory: empty content and Unicode path roundtrip" {
  let operator = @opendal.Operator::new("memory")
  let path = "目录/空文件-🌙.bin"
  let metadata = operator.write(path, Bytes::new(0))
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, 0UL)
  assert_eq(operator.read(path), Bytes::new(0))
}

///|
test "native memory: non-empty config crosses the FixedArray FFI" {
  let config : Map[String, String] = { "root": "/configured/" }
  let operator = @opendal.Operator::new("memory", config~)
  assert_eq(operator.info().root, "/configured/")
  operator.write("value.bin", b"configured") |> ignore
  assert_eq(operator.read("value.bin"), b"configured")
}

///|
test "native memory: blocking core operations preserve storage semantics" {
  let operator = @opendal.Operator::new("memory")
  operator.check()
  assert_false(operator.exists("tree/branch/value.bin"))
  operator.create_dir("tree/branch/")
  operator.create_dir("tree/branch/")
  let directory = operator.stat("tree/branch/")
  assert_true(directory.is_dir())
  let payload = b"blocking-core"
  operator.write("tree/branch/value.bin", payload) |> ignore
  assert_true(operator.exists("tree/branch/value.bin"))
  let metadata = operator.stat("tree/branch/value.bin")
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, payload.length().to_uint64())
  operator.delete("tree/branch/value.bin")
  operator.delete("tree/branch/value.bin")
  assert_false(operator.exists("tree/branch/value.bin"))
  operator.write("tree/branch/nested/one.bin", b"one") |> ignore
  operator.write("tree/branch/nested/two.bin", b"two") |> ignore
  operator.delete("tree/", recursive=true)
  assert_false(operator.exists("tree/branch/nested/one.bin"))
  assert_false(operator.exists("tree/branch/nested/two.bin"))
}

///|
test "native memory: stat missing path preserves typed context" {
  let operator = @opendal.Operator::new("memory")
  try operator.stat("missing-stat.bin") catch {
    OpenDalError(info) => {
      assert_true(info.kind is NotFound)
      assert_true(info.operation is Stat)
      assert_eq(info.path, Some("missing-stat.bin"))
      assert_eq(info.destination_path, None)
    }
  } noraise {
    _ => fail("expected missing stat to raise")
  }
}

///|
test "native memory: stat and delete options cross the ABI" {
  let operator = @opendal.Operator::new("memory")
  operator.write("option-value.bin", b"value") |> ignore
  let condition_storage = "__not-an-etag__"
  let condition : StringView = condition_storage[2:13]
  try operator.stat("option-value.bin", if_match=condition) catch {
    OpenDalError(info) => {
      assert_true(info.kind is Unsupported)
      assert_true(info.operation is Stat)
      assert_eq(info.path, Some("option-value.bin"))
    }
  } noraise {
    _ => fail("expected unsupported stat option to raise")
  }
  try operator.delete("option-value.bin", version="not-a-version") catch {
    OpenDalError(info) => {
      assert_true(info.kind is Unsupported)
      assert_true(info.operation is Delete)
      assert_eq(info.path, Some("option-value.bin"))
    }
  } noraise {
    _ => fail("expected unsupported delete option to raise")
  }
  assert_true(operator.exists("option-value.bin"))
}

///|
test "native fs: blocking core works against an isolated filesystem root" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-mbt-fs-core-\{@env.now()}"
  let config : Map[String, String] = { "root": root }
  let operator = @opendal.Operator::new("fs", config~)
  operator.check()
  let info = operator.info()
  assert_eq(info.scheme, "fs")
  assert_true(info.capability.can_stat())
  assert_true(info.capability.can_create_dir())
  assert_true(info.capability.can_delete())
  operator.create_dir("目录/nested/")
  assert_true(operator.stat("目录/nested/").is_dir())
  let payload : Bytes = [0x00, 0x7f, 0x80, 0xff]
  operator.write("目录/nested/value.bin", payload) |> ignore
  assert_true(operator.exists("目录/nested/value.bin"))
  let metadata = operator.stat("目录/nested/value.bin")
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, payload.length().to_uint64())
  operator.delete("目录/nested/missing.bin")
  operator.delete("目录/", recursive=true)
  assert_false(operator.exists("目录/nested/value.bin"))
  operator.check()
}

///|
test "native memory: eager and streaming listing return entry snapshots" {
  let operator = @opendal.Operator::new("memory")
  let capability = operator.info().capability
  assert_true(capability.can_list())
  operator.write("listing/root.txt", b"root") |> ignore
  operator.write("listing/目录/one.bin", b"one") |> ignore
  operator.write("listing/目录/two.bin", b"two") |> ignore
  let shallow = operator.list("listing/")
  assert_true(has_entry(shallow, "listing/root.txt"))
  assert_true(has_entry(shallow, "listing/目录/"))
  let recursive = operator.list("listing/", recursive=true)
  assert_true(has_entry(recursive, "listing/root.txt"))
  assert_true(has_entry(recursive, "listing/目录/one.bin"))
  assert_true(has_entry(recursive, "listing/目录/two.bin"))
  let start_after_storage = "__listing/root.txt__"
  let start_after : StringView = start_after_storage[2:18]
  try operator.list("listing/", recursive=true, limit=1UL, start_after~) catch {
    OpenDalError(info) => {
      assert_true(info.kind is Unsupported)
      assert_true(
        !capability.can_list_with_limit() ||
        !capability.can_list_with_start_after(),
      )
    }
  } noraise {
    entries =>
      if capability.can_list_with_start_after() {
        assert_false(has_entry(entries, "listing/root.txt"))
      }
  }
  let lister = operator.open_lister("listing/", recursive=true)
  let streamed : Array[@opendal.Entry] = []
  for ;; {
    match lister.next() {
      Some(entry) => streamed.push(entry)
      None => break
    }
  }
  assert_eq(lister.next(), None)
  lister.close()
  assert_true(has_entry(streamed, "listing/目录/one.bin"))
  for entry in streamed {
    if entry.path == "listing/目录/one.bin" {
      assert_eq(entry.name, "one.bin")
      assert_true(entry.metadata.is_file())
    }
  }
}

///|
test "native memory: closing a lister is idempotent and preserves context" {
  let operator = @opendal.Operator::new("memory")
  let lister = operator.open_lister("closed-prefix/")
  lister.close()
  lister.close()
  try lister.next() catch {
    OpenDalError(info) => {
      assert_true(info.kind is ResourceClosed)
      assert_true(info.operation is List)
      assert_eq(info.path, Some("closed-prefix/"))
      assert_eq(info.destination_path, None)
    }
  } noraise {
    _ => fail("expected next after close to raise")
  }
}

///|
test "native fs: recursive listing works against an isolated filesystem root" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-mbt-fs-listing-\{@env.now()}"
  let operator = @opendal.Operator::new("fs", config={ "root": root })
  operator.write("列表/root.txt", b"root") |> ignore
  operator.write("列表/nested/value.bin", b"value") |> ignore
  let entries = operator.list("列表/", recursive=true)
  assert_true(has_entry(entries, "列表/root.txt"))
  assert_true(has_entry(entries, "列表/nested/value.bin"))
  operator.delete("列表/", recursive=true)
}
