///|
test "presigned snapshot preserves duplicate and binary headers" {
  let request = presigned_request_from_parts(
    b"GET",
    b"https://example.invalid/object",
    [b"x-opendal-value", b"x-opendal-value"],
    [b"first", [0xff, 0x00, 0x80]],
    Read,
    "object",
  )
  assert_eq(request.http_method, "GET")
  assert_eq(request.uri, "https://example.invalid/object")
  assert_eq(request.headers.length(), 2)
  assert_eq(request.headers[0].name, "x-opendal-value")
  assert_eq(request.headers[0].value, b"first")
  assert_eq(request.headers[1].name, "x-opendal-value")
  assert_eq(request.headers[1].value, ([0xff, 0x00, 0x80] : Bytes))
}

///|
test "presign validation keeps operation and path context" {
  try validate_presign_expiry(0UL, Stat, "object") catch {
    OpenDalError(info) => {
      assert_true(info.kind is InvalidArgument)
      assert_true(info.operation is Stat)
      assert_eq(info.path, Some("object"))
    }
  } noraise {
    _ => fail("expected zero expiry to raise")
  }
}

///|
test "presigned snapshot rejects malformed text and header arrays" {
  try
    presigned_request_from_parts(
      b"GET",
      b"https://example.invalid",
      [b"x-one"],
      [],
      Read,
      "object",
    )
  catch {
    OpenDalError(info) => assert_true(info.kind is AbiMismatch)
  } noraise {
    _ => fail("expected mismatched header arrays to raise")
  }
  try
    presigned_request_from_parts(
      ([0xff] : Bytes),
      b"https://example.invalid",
      [],
      [],
      Read,
      "object",
    )
  catch {
    OpenDalError(info) => assert_true(info.kind is AbiMismatch)
  } noraise {
    _ => fail("expected malformed method text to raise")
  }
}

///|
fn presign_public_contract(
  operator : Operator,
) -> (PresignedRequest, PresignedRequest, PresignedRequest) raise OpenDalError {
  let read = operator.presign_read(
    "object",
    expires_in_seconds=300UL,
    range=Range(offset=4UL, length=8UL),
    version="read-version",
    if_match="read-etag",
  )
  let write = operator.presign_write(
    "object",
    expires_in_seconds=300UL,
    append=false,
    content_type="application/octet-stream",
    content_disposition="inline",
    content_encoding="identity",
    cache_control="no-cache",
    if_none_match="*",
  )
  let stat = operator.presign_stat(
    "object",
    expires_in_seconds=300UL,
    version="stat-version",
    if_none_match="stat-etag",
  )
  (read, write, stat)
}

///|
test "presign public facade type-checks" {
  ignore(presign_public_contract)
}
