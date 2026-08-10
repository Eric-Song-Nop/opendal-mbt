///|
test "native memory: binary whole-object roundtrip" {
  let operator = @opendal.Operator::new("memory", Map([]))
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
test "native memory: missing path preserves typed context" {
  let operator = @opendal.Operator::new("memory", Map([]))
  try operator.read("missing.bin") catch {
    OpenDalError(info) => {
      assert_eq(info.kind, NotFound)
      assert_eq(info.operation, Read)
      assert_eq(info.path, Some("missing.bin"))
      assert_eq(info.destination_path, None)
    }
  } noraise {
    _ => fail("expected missing read to raise")
  }
}

///|
test "native memory: empty content and Unicode path roundtrip" {
  let operator = @opendal.Operator::new("memory", Map([]))
  let path = "目录/空文件-🌙.bin"
  let metadata = operator.write(path, Bytes::new(0))
  assert_true(metadata.is_file())
  assert_eq(metadata.content_length, 0UL)
  assert_eq(operator.read(path), Bytes::new(0))
}

///|
test "native memory: deferred read options fail before the ABI call" {
  let operator = @opendal.Operator::new("memory", Map([]))
  let options = @opendal.ReadOptions::new(range=From(1UL))
  try operator.read("missing.bin", options~) catch {
    OpenDalError(info) => {
      assert_eq(info.kind, Unsupported)
      assert_eq(info.operation, Read)
      assert_eq(info.path, Some("missing.bin"))
    }
  } noraise {
    _ => fail("expected non-default first-slice options to raise")
  }
}

///|
test "native memory: non-empty config crosses the FixedArray FFI" {
  let config : Map[String, String] = { "root": "/configured/" }
  let operator = @opendal.Operator::new("memory", config)
  assert_eq(operator.info().root, "/configured/")
  operator.write("value.bin", b"configured") |> ignore
  assert_eq(operator.read("value.bin"), b"configured")
}

///|
test "native memory: blocking core operations preserve storage semantics" {
  let operator = @opendal.Operator::new("memory", Map([]))
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
  let options = @opendal.DeleteOptions::new(recursive=true)
  operator.delete("tree/", options~)
  assert_false(operator.exists("tree/branch/nested/one.bin"))
  assert_false(operator.exists("tree/branch/nested/two.bin"))
}

///|
test "native memory: stat missing path preserves typed context" {
  let operator = @opendal.Operator::new("memory", Map([]))
  try operator.stat("missing-stat.bin") catch {
    OpenDalError(info) => {
      assert_eq(info.kind, NotFound)
      assert_eq(info.operation, Stat)
      assert_eq(info.path, Some("missing-stat.bin"))
      assert_eq(info.destination_path, None)
    }
  } noraise {
    _ => fail("expected missing stat to raise")
  }
}

///|
test "native memory: stat and delete options cross the ABI" {
  let operator = @opendal.Operator::new("memory", Map([]))
  operator.write("option-value.bin", b"value") |> ignore
  let stat_options = @opendal.StatOptions::new(if_match="not-an-etag")
  try operator.stat("option-value.bin", options=stat_options) catch {
    OpenDalError(info) => {
      assert_eq(info.kind, Unsupported)
      assert_eq(info.operation, Stat)
      assert_eq(info.path, Some("option-value.bin"))
    }
  } noraise {
    _ => fail("expected unsupported stat option to raise")
  }
  let delete_options = @opendal.DeleteOptions::new(version="not-a-version")
  try operator.delete("option-value.bin", options=delete_options) catch {
    OpenDalError(info) => {
      assert_eq(info.kind, Unsupported)
      assert_eq(info.operation, Delete)
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
  let operator = @opendal.Operator::new("fs", config)
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
  let options = @opendal.DeleteOptions::new(recursive=true)
  operator.delete("目录/", options~)
  assert_false(operator.exists("目录/nested/value.bin"))
  operator.check()
}
