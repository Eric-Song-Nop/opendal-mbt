///|
fn odm1_write_u32(output : Array[Byte], offset : Int, value : UInt) -> Unit {
  output[offset] = value.to_byte()
  output[offset + 1] = (value >> 8).to_byte()
  output[offset + 2] = (value >> 16).to_byte()
  output[offset + 3] = (value >> 24).to_byte()
}

///|
fn odm1_write_u64(output : Array[Byte], offset : Int, value : UInt64) -> Unit {
  for index = 0; index < 8; index = index + 1 {
    output[offset + index] = (value >> (index * 8)).to_byte()
  }
}

///|
fn odm1_append(output : Array[Byte], value : Bytes) -> Unit {
  for byte in value {
    output.push(byte)
  }
}

///|
fn odm1_zero_array() -> Array[Byte] {
  let output = Array::make(METADATA_SNAPSHOT_HEADER_LENGTH, b'\x00')
  output[0] = b'O'
  output[1] = b'D'
  output[2] = b'M'
  output[3] = b'1'
  odm1_write_u32(output, 4, 1U)
  output
}

///|
fn odm1_parse_for_test(snapshot : Bytes) -> Metadata raise OpenDalError {
  parse_metadata_snapshot(snapshot, error_context(Stat))
}

///|
fn assert_malformed_odm1(snapshot : Array[Byte]) -> Unit raise {
  let error_kind = try odm1_parse_for_test(Bytes::from_array(snapshot)) catch {
    error => Some(error.kind())
  } noraise {
    _ => None
  }
  assert_eq(error_kind, Some(AbiMismatch))
}

///|
test "ODM1 decodes the complete native-shaped metadata contract" {
  let snapshot = odm1_zero_array()
  odm1_write_u64(snapshot, 8, 0x01ffUL)
  odm1_write_u32(snapshot, 16, 1U)
  odm1_write_u32(snapshot, 20, 0U)
  odm1_write_u32(snapshot, 24, 1U)
  odm1_write_u64(snapshot, 32, 18446744073709551615UL)
  odm1_write_u64(snapshot, 40, 123UL)
  odm1_write_u32(snapshot, 48, 456789U)
  for index, length in [5U, 11U, 8U, 3U, 4U, 4U, 7U] {
    odm1_write_u32(snapshot, 56 + index * 4, length)
  }
  odm1_append(snapshot, b"cachedispositionencodingmd5typeetagversion")

  assert_eq(odm1_parse_for_test(Bytes::from_array(snapshot)), {
    mode: File,
    content_length: 18446744073709551615UL,
    is_current: Some(false),
    is_deleted: true,
    cache_control: Some("cache"),
    content_disposition: Some("disposition"),
    content_encoding: Some("encoding"),
    content_md5: Some("md5"),
    content_type: Some("type"),
    etag: Some("etag"),
    last_modified: Some({ unix_seconds: 123L, nanoseconds: 456789 }),
    version: Some("version"),
  })
}

///|
test "ODM1 preserves canonical absence, pre-epoch time, and present empty text" {
  let zero = odm1_parse_for_test(Bytes::from_array(odm1_zero_array()))
  assert_eq(zero, {
    mode: Unknown,
    content_length: 0UL,
    is_current: None,
    is_deleted: false,
    cache_control: None,
    content_disposition: None,
    content_encoding: None,
    content_md5: None,
    content_type: None,
    etag: None,
    last_modified: None,
    version: None,
  })

  let pre_epoch = odm1_zero_array()
  odm1_write_u64(pre_epoch, 8, 2UL)
  odm1_write_u64(pre_epoch, 40, (-5L).reinterpret_as_uint64())
  odm1_write_u32(pre_epoch, 48, 999999999U)
  assert_eq(
    odm1_parse_for_test(Bytes::from_array(pre_epoch)).last_modified,
    Some({ unix_seconds: -5L, nanoseconds: 999999999 }),
  )

  let present_empty = odm1_zero_array()
  odm1_write_u64(present_empty, 8, 1UL << 2)
  assert_eq(
    odm1_parse_for_test(Bytes::from_array(present_empty)).cache_control,
    Some(""),
  )
}

///|
test "ODM1 rejects malformed header, scalar, and canonical-absence fields" {
  let bad_magic = odm1_zero_array()
  bad_magic[0] = b'X'
  assert_malformed_odm1(bad_magic)

  let bad_schema = odm1_zero_array()
  odm1_write_u32(bad_schema, 4, 2U)
  assert_malformed_odm1(bad_schema)

  let unknown_bits = odm1_zero_array()
  odm1_write_u64(unknown_bits, 8, 1UL << 9)
  assert_malformed_odm1(unknown_bits)

  let bad_mode = odm1_zero_array()
  odm1_write_u32(bad_mode, 16, 3U)
  assert_malformed_odm1(bad_mode)

  for offset in [20, 24] {
    let noncanonical_bool = odm1_zero_array()
    odm1_write_u32(noncanonical_bool, offset, 2U)
    assert_malformed_odm1(noncanonical_bool)
  }

  let absent_current = odm1_zero_array()
  odm1_write_u32(absent_current, 20, 1U)
  assert_malformed_odm1(absent_current)

  for offset in [28, 52] {
    let reserved = odm1_zero_array()
    odm1_write_u32(reserved, offset, 1U)
    assert_malformed_odm1(reserved)
  }

  let bad_nanoseconds = odm1_zero_array()
  odm1_write_u64(bad_nanoseconds, 8, 2UL)
  odm1_write_u32(bad_nanoseconds, 48, 1000000000U)
  assert_malformed_odm1(bad_nanoseconds)

  let absent_timestamp_seconds = odm1_zero_array()
  odm1_write_u64(absent_timestamp_seconds, 40, 1UL)
  assert_malformed_odm1(absent_timestamp_seconds)

  let absent_timestamp_nanoseconds = odm1_zero_array()
  odm1_write_u32(absent_timestamp_nanoseconds, 48, 1U)
  assert_malformed_odm1(absent_timestamp_nanoseconds)
}

///|
test "ODM1 rejects malformed lengths, trailing bytes, and UTF-8" {
  let absent_text = odm1_zero_array()
  odm1_write_u32(absent_text, 56, 1U)
  absent_text.push(b'x')
  assert_malformed_odm1(absent_text)

  let truncated = odm1_zero_array()
  odm1_write_u64(truncated, 8, 1UL << 2)
  odm1_write_u32(truncated, 56, 2U)
  truncated.push(b'x')
  assert_malformed_odm1(truncated)

  let trailing = odm1_zero_array()
  trailing.push(b'\x00')
  assert_malformed_odm1(trailing)

  let invalid_utf8 = odm1_zero_array()
  odm1_write_u64(invalid_utf8, 8, 1UL << 2)
  odm1_write_u32(invalid_utf8, 56, 1U)
  invalid_utf8.push(b'\xff')
  assert_malformed_odm1(invalid_utf8)

  let overlong_utf8 = odm1_zero_array()
  odm1_write_u64(overlong_utf8, 8, 1UL << 2)
  odm1_write_u32(overlong_utf8, 56, 2U)
  overlong_utf8.push(b'\xc0')
  overlong_utf8.push(b'\xaf')
  assert_malformed_odm1(overlong_utf8)

  let oversized_text = odm1_zero_array()
  odm1_write_u32(oversized_text, 56, 67108865U)
  assert_malformed_odm1(oversized_text)
}
