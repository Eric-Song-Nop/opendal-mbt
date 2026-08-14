///|
const METADATA_SNAPSHOT_HEADER_LENGTH = 84

///|
const METADATA_SNAPSHOT_PRESENT_BITS = 0x01ffUL

///|
fn malformed_metadata_snapshot(
  context : ErrorContext,
  detail : String,
) -> OpenDalError {
  local_error(
    AbiMismatch,
    "OpenDAL bridge returned a malformed metadata snapshot: \{detail}",
    context,
  )
}

///|
fn metadata_snapshot_u64(
  snapshot : Bytes,
  offset : Int,
  context : ErrorContext,
) -> UInt64 raise OpenDalError {
  guard snapshot_word(snapshot, offset) is Some(low) else {
    raise malformed_metadata_snapshot(context, "truncated UInt64 field")
  }
  guard snapshot_word(snapshot, offset + 4) is Some(high) else {
    raise malformed_metadata_snapshot(context, "truncated UInt64 field")
  }
  (high.to_uint64() << 32) | low.to_uint64()
}

///|
fn metadata_snapshot_length(
  snapshot : Bytes,
  offset : Int,
  context : ErrorContext,
) -> Int raise OpenDalError {
  guard snapshot_word(snapshot, offset) is Some(length) else {
    raise malformed_metadata_snapshot(context, "truncated string length")
  }
  if length > MAX_TRANSFER_BUFFER_LENGTH.reinterpret_as_uint() {
    raise malformed_metadata_snapshot(context, "string length exceeds 64 MiB")
  }
  length.reinterpret_as_int()
}

///|
fn decode_metadata_text(
  snapshot : Bytes,
  present_bits : UInt64,
  present_bit : UInt64,
  length : Int,
  cursor : Int,
  context : ErrorContext,
) -> (String?, Int) raise OpenDalError {
  let present = (present_bits & present_bit) != 0UL
  if !present {
    if length != 0 {
      raise malformed_metadata_snapshot(
        context, "absent string has a non-zero length",
      )
    }
    return (None, cursor)
  }
  let end = cursor + length
  let value = @utf8.decode(snapshot[cursor:end]) catch {
    _ => raise malformed_metadata_snapshot(context, "string is not UTF-8")
  }
  (Some(value), end)
}

///|
fn parse_metadata_snapshot(
  snapshot : Bytes,
  context : ErrorContext,
) -> Metadata raise OpenDalError {
  if snapshot.length() < METADATA_SNAPSHOT_HEADER_LENGTH ||
    snapshot.length() > MAX_TRANSFER_BUFFER_LENGTH ||
    snapshot[0] != b'O' ||
    snapshot[1] != b'D' ||
    snapshot[2] != b'M' ||
    snapshot[3] != b'1' {
    raise malformed_metadata_snapshot(context, "invalid header")
  }
  guard snapshot_word(snapshot, 4) is Some(1U) else {
    raise malformed_metadata_snapshot(context, "unsupported schema")
  }
  let present_bits = metadata_snapshot_u64(snapshot, 8, context)
  if present_bits > METADATA_SNAPSHOT_PRESENT_BITS {
    raise malformed_metadata_snapshot(context, "unknown presence bits")
  }
  guard snapshot_word(snapshot, 16) is Some(mode_raw) else {
    raise malformed_metadata_snapshot(context, "truncated mode")
  }
  if mode_raw > 2U {
    raise malformed_metadata_snapshot(context, "invalid entry mode")
  }
  guard snapshot_word(snapshot, 20) is Some(is_current_raw) else {
    raise malformed_metadata_snapshot(context, "truncated current flag")
  }
  guard snapshot_word(snapshot, 24) is Some(is_deleted_raw) else {
    raise malformed_metadata_snapshot(context, "truncated deleted flag")
  }
  guard snapshot_word(snapshot, 28) is Some(0U) else {
    raise malformed_metadata_snapshot(context, "reserved word is non-zero")
  }
  if is_current_raw > 1U || is_deleted_raw > 1U {
    raise malformed_metadata_snapshot(context, "non-canonical boolean")
  }
  let current_present = (present_bits & 1UL) != 0UL
  if !current_present && is_current_raw != 0U {
    raise malformed_metadata_snapshot(
      context, "absent current flag is non-zero",
    )
  }
  let content_length = metadata_snapshot_u64(snapshot, 32, context)
  let modified_seconds = metadata_snapshot_u64(snapshot, 40, context).reinterpret_as_int64()
  guard snapshot_word(snapshot, 48) is Some(modified_nanoseconds_raw) else {
    raise malformed_metadata_snapshot(context, "truncated timestamp")
  }
  guard snapshot_word(snapshot, 52) is Some(0U) else {
    raise malformed_metadata_snapshot(context, "reserved word is non-zero")
  }
  if modified_nanoseconds_raw >= 1000000000U {
    raise malformed_metadata_snapshot(context, "invalid timestamp nanoseconds")
  }
  let modified_present = (present_bits & 2UL) != 0UL
  if !modified_present &&
    (modified_seconds != 0L || modified_nanoseconds_raw != 0U) {
    raise malformed_metadata_snapshot(context, "absent timestamp is non-zero")
  }
  let cache_control_length = metadata_snapshot_length(snapshot, 56, context)
  let content_disposition_length = metadata_snapshot_length(
    snapshot, 60, context,
  )
  let content_encoding_length = metadata_snapshot_length(snapshot, 64, context)
  let content_md5_length = metadata_snapshot_length(snapshot, 68, context)
  let content_type_length = metadata_snapshot_length(snapshot, 72, context)
  let etag_length = metadata_snapshot_length(snapshot, 76, context)
  let version_length = metadata_snapshot_length(snapshot, 80, context)
  let payload_length = cache_control_length +
    content_disposition_length +
    content_encoding_length +
    content_md5_length +
    content_type_length +
    etag_length +
    version_length
  if METADATA_SNAPSHOT_HEADER_LENGTH + payload_length != snapshot.length() {
    raise malformed_metadata_snapshot(context, "payload length mismatch")
  }
  let mut cursor = METADATA_SNAPSHOT_HEADER_LENGTH
  let (cache_control, next) = decode_metadata_text(
    snapshot,
    present_bits,
    1UL << 2,
    cache_control_length,
    cursor,
    context,
  )
  cursor = next
  let (content_disposition, next) = decode_metadata_text(
    snapshot,
    present_bits,
    1UL << 3,
    content_disposition_length,
    cursor,
    context,
  )
  cursor = next
  let (content_encoding, next) = decode_metadata_text(
    snapshot,
    present_bits,
    1UL << 4,
    content_encoding_length,
    cursor,
    context,
  )
  cursor = next
  let (content_md5, next) = decode_metadata_text(
    snapshot,
    present_bits,
    1UL << 5,
    content_md5_length,
    cursor,
    context,
  )
  cursor = next
  let (content_type, next) = decode_metadata_text(
    snapshot,
    present_bits,
    1UL << 6,
    content_type_length,
    cursor,
    context,
  )
  cursor = next
  let (etag, next) = decode_metadata_text(
    snapshot,
    present_bits,
    1UL << 7,
    etag_length,
    cursor,
    context,
  )
  cursor = next
  let (version, end) = decode_metadata_text(
    snapshot,
    present_bits,
    1UL << 8,
    version_length,
    cursor,
    context,
  )
  if end != snapshot.length() {
    raise malformed_metadata_snapshot(context, "trailing payload")
  }
  {
    mode: entry_mode_from_bridge_unchecked(mode_raw.reinterpret_as_int()),
    content_length,
    is_current: if current_present {
      Some(is_current_raw == 1U)
    } else {
      None
    },
    is_deleted: is_deleted_raw == 1U,
    cache_control,
    content_disposition,
    content_encoding,
    content_md5,
    content_type,
    etag,
    last_modified: if modified_present {
      Some({
        unix_seconds: modified_seconds,
        nanoseconds: modified_nanoseconds_raw.reinterpret_as_int(),
      })
    } else {
      None
    },
    version,
  }
}
