///|
test "structured error snapshot decodes little-endian fields" {
  let snapshot : Bytes = [
    b'O', b'D', b'E', b'1', 1, 0, 0, 0, 4, 0, 0, 0, 2, 0, 0, 0, 8, 0, 0, 0, 7, 0,
    0, 0, b'N', b'o', b't', b'F', b'o', b'u', b'n', b'd', b'm', b'i', b's', b's',
    b'i', b'n', b'g',
  ]

  guard parse_bridge_error_snapshot(snapshot) is Some(decoded) else {
    fail("expected a decoded snapshot")
  }
  assert_eq(decoded.kind, NotFound)
  assert_eq(decoded.status, Temporary)
  assert_eq(decoded.message, "missing")
}

///|
test "structured error snapshot preserves unknown kind and status" {
  let snapshot : Bytes = [
    b'O', b'D', b'E', b'1', 1, 0, 0, 0, 1, 32, 0, 0, 99, 0, 0, 0, 10, 0, 0, 0, 1,
    0, 0, 0, b'F', b'u', b't', b'u', b'r', b'e', b'K', b'i', b'n', b'd', b'x',
  ]

  guard parse_bridge_error_snapshot(snapshot) is Some(decoded) else {
    fail("expected a decoded snapshot")
  }
  assert_eq(decoded.kind, UnknownKind(8193, "FutureKind"))
  assert_eq(decoded.status, UnknownStatus(99))
}

///|
test "structured error snapshot rejects malformed schema and truncation" {
  let wrong_schema : Bytes = [
    b'O', b'D', b'E', b'1', 2, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0,
  ]
  let truncated : Bytes = [b'O', b'D', b'E', b'1', 1, 0, 0]

  assert_true(parse_bridge_error_snapshot(wrong_schema) is None)
  assert_true(parse_bridge_error_snapshot(truncated) is None)
}

///|
test "structured error snapshot rejects invalid UTF-8 and trailing bytes" {
  let invalid_utf8 : Bytes = [
    b'O', b'D', b'E', b'1', 1, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0,
    0, 0, 255,
  ]
  let trailing : Bytes = [
    b'O', b'D', b'E', b'1', 1, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0,
  ]

  assert_true(parse_bridge_error_snapshot(invalid_utf8) is None)
  assert_true(parse_bridge_error_snapshot(trailing) is None)
}
