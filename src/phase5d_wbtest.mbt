///|
test "batch delete validation counts exact UTF-8 bytes" {
  assert_eq(delete_path_utf8_length("plain"), 5UL)
  assert_eq(delete_path_utf8_length("é"), 2UL)
  assert_eq(delete_path_utf8_length("中"), 3UL)
  assert_eq(delete_path_utf8_length("🦀"), 4UL)
}

///|
test "batch delete validation copies array membership" {
  let input = ["first", "second"]
  let copied = prepare_delete_many_paths(input)
  input[0] = "changed"
  assert_eq(copied.length(), 2)
  assert_eq(copied[0], "first")
  assert_eq(copied[1], "second")
}

///|
test "batch delete validation accepts empty input" {
  let copied = prepare_delete_many_paths([])
  assert_eq(copied.length(), 0)
}

///|
test "batch delete validation rejects an empty path before native work" {
  try prepare_delete_many_paths(["one", ""]) catch {
    OpenDalError(info) => {
      assert_true(info.kind is InvalidArgument)
      assert_true(info.operation is Delete)
      assert_eq(info.path, Some(""))
    }
  } noraise {
    _ => fail("expected empty-path validation to fail")
  }
}

///|
test "capability batch size hint preserves absence and positive Int values" {
  assert_eq(capability_size_hint(0UL), None)
  assert_eq(capability_size_hint(37UL), Some(37))
  assert_eq(capability_size_hint(2147483648UL), Some(2147483647))
}
