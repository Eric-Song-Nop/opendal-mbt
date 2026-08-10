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
