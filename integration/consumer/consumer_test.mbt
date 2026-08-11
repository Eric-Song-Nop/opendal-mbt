///|
fn contains_path(entries : Array[@opendal.Entry], path : String) -> Bool {
  entries.any(fn(entry) { entry.path == path })
}

///|
test "downstream consumer executes every published operator workflow" {
  let operator = @opendal.Operator::new("memory")
  operator.check()
  let info = operator.info()
  assert_eq(info.scheme, "memory")
  assert_true(info.capability.can_stat())
  assert_true(info.capability.can_read())
  ignore(info.capability.can_read_suffix())
  assert_true(info.capability.can_write())
  ignore(info.capability.can_write_append())
  assert_true(info.capability.can_create_dir())
  assert_true(info.capability.can_delete())
  assert_true(info.capability.can_list())
  ignore(info.capability.can_list_with_limit())
  ignore(info.capability.can_list_with_start_after())
  ignore(info.capability.can_list_recursive())

  let path_storage = "__consumer/value.bin__"
  let path : StringView = path_storage[2:20]
  let data_storage = b"__downstream__"
  let data : BytesView = data_storage[2:12]
  operator.create_dir("consumer/")
  let written = operator.write(path, data)
  assert_true(written.is_file())
  assert_true(operator.exists(path))
  assert_true(operator.stat(path).is_file())
  assert_eq(operator.read(path), b"downstream")
  assert_eq(operator.read(path, range=Range(offset=4UL, length=6UL)), b"stream")
  let reader = operator.open_reader(path)
  assert_eq(reader.read(From(offset=4UL)), b"stream")
  assert_eq(reader.read(Suffix(length=4UL)), b"ream")
  reader.close()
  reader.close()

  let writer = operator.open_writer(
    "consumer/chunked.bin",
    content_type="application/octet-stream",
  )
  writer.write(b"chunked-")
  writer.write(b"consumer")
  let chunked = writer.finish()
  assert_eq(chunked.content_length, 16UL)
  assert_eq(operator.read("consumer/chunked.bin"), b"chunked-consumer")

  operator.write("consumer/nested/one.bin", b"one") |> ignore
  let entries = operator.list("consumer/", recursive=true)
  assert_true(contains_path(entries, "consumer/value.bin"))
  assert_true(contains_path(entries, "consumer/nested/one.bin"))

  let lister = operator.open_lister("consumer/", recursive=true)
  let mut saw_entry = false
  for ;; {
    match lister.next() {
      Some(_) => saw_entry = true
      None => break
    }
  }
  assert_true(saw_entry)
  assert_eq(lister.next(), None)
  lister.close()
  lister.close()

  try operator.read("consumer/missing.bin") catch {
    error => {
      let error_info = error.info()
      assert_true(error.kind() is NotFound)
      assert_true(error.status() is Permanent)
      assert_false(error.is_temporary())
      assert_false(error.is_persistent())
      assert_eq(error_info.path, Some("consumer/missing.bin"))
      assert_true("\{error}".contains("OpenDAL NotFound"))
    }
  } noraise {
    _ => fail("expected missing read to raise")
  }

  operator.delete("consumer/", recursive=true)
  assert_false(operator.exists(path))
}

///|
test "downstream consumer uses the filesystem service profile" {
  guard @env.current_dir() is Some(cwd) else {
    fail("current working directory is unavailable")
  }
  let root = "\{cwd}/target/opendal-mbt-consumer-fs-\{@env.now()}"
  let operator = @opendal.Operator::new("fs", config={ "root": root })
  operator.check()
  operator.write("consumer/nested/value.bin", b"filesystem") |> ignore
  assert_eq(operator.read("consumer/nested/value.bin"), b"filesystem")
  assert_true(
    contains_path(
      operator.list("consumer/", recursive=true),
      "consumer/nested/value.bin",
    ),
  )
  operator.delete("consumer/", recursive=true)
  assert_false(operator.exists("consumer/nested/value.bin"))
}
