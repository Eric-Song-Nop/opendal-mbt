///|
const EXPECTED_ABI_VERSION = 0x0001_0005U

///|
// The public facade requires the resource, task, generic-operator, mutation,
// listing, and bulk-transfer features. Memory and the poll-once adapter are
// fixture-only bridge capabilities, not requirements of the generic facade.
const REQUIRED_FEATURE_FLAGS = 0x0000_03fcU

///|
const ERROR_SNAPSHOT_HEADER_LENGTH = 24

///|
const MAX_TRANSFER_BUFFER_LENGTH = 67108864

///|
const MAX_TRANSFER_CHUNK_LENGTH = 262144

///|
/// This adapter is private because MoonBit's linear-Wasm `Bytes` layout is
/// pinned by this package's toolchain, not part of MoonBit's stable Wasm ABI.
#borrow(bytes)
extern "wasm" fn bytes_address(bytes : Bytes) -> UInt =
  #|(func (param i32) (result i32)
  #|  (local.get 0))

///|
fn bridge_error_kind(code : UInt, kind_name : String) -> ErrorKind {
  match code {
    1U => Unexpected
    2U => Unsupported
    3U => ConfigInvalid
    4U => NotFound
    5U => PermissionDenied
    6U => IsADirectory
    7U => NotADirectory
    8U => AlreadyExists
    9U => RateLimited
    10U => IsSameFile
    11U => ConditionNotMatch
    12U => RangeNotSatisfied
    0x1001U => InvalidArgument
    0x1002U => ResourceClosed
    0x1003U => BufferTooLarge
    0x1004U => AbiMismatch
    _ => UnknownKind(code.reinterpret_as_int(), kind_name)
  }
}

///|
fn bridge_error_status(status : UInt) -> ErrorStatus {
  match status {
    1U => Permanent
    2U => Temporary
    3U => Persistent
    _ => UnknownStatus(status.reinterpret_as_int())
  }
}

///|
fn snapshot_word(snapshot : Bytes, offset : Int) -> UInt? {
  if offset < 0 || offset + 4 > snapshot.length() {
    return None
  }
  Some(
    snapshot[offset].to_uint() |
    (snapshot[offset + 1].to_uint() << 8) |
    (snapshot[offset + 2].to_uint() << 16) |
    (snapshot[offset + 3].to_uint() << 24),
  )
}

///|
fn decode_snapshot_utf8(value : BytesView) -> String? {
  try @utf8.decode(value) catch {
    _ => None
  } noraise {
    value => Some(value)
  }
}

///|
fn parse_bridge_error_snapshot(snapshot : Bytes) -> BridgeErrorSnapshot? {
  if snapshot.length() < ERROR_SNAPSHOT_HEADER_LENGTH ||
    snapshot[0] != b'O' ||
    snapshot[1] != b'D' ||
    snapshot[2] != b'E' ||
    snapshot[3] != b'1' {
    return None
  }
  guard snapshot_word(snapshot, 4) is Some(1U) else { return None }
  guard snapshot_word(snapshot, 8) is Some(kind_code) else { return None }
  guard snapshot_word(snapshot, 12) is Some(status_code) else { return None }
  guard snapshot_word(snapshot, 16) is Some(kind_name_length_raw) else {
    return None
  }
  guard snapshot_word(snapshot, 20) is Some(message_length_raw) else {
    return None
  }
  let kind_name_length = kind_name_length_raw.reinterpret_as_int()
  let message_length = message_length_raw.reinterpret_as_int()
  if kind_name_length < 0 || message_length < 0 {
    return None
  }
  let kind_name_end = ERROR_SNAPSHOT_HEADER_LENGTH + kind_name_length
  let message_end = kind_name_end + message_length
  if kind_name_end < ERROR_SNAPSHOT_HEADER_LENGTH ||
    message_end < kind_name_end ||
    message_end != snapshot.length() {
    return None
  }
  guard decode_snapshot_utf8(
      snapshot[ERROR_SNAPSHOT_HEADER_LENGTH:kind_name_end],
    )
    is Some(kind_name) else {
    return None
  }
  guard decode_snapshot_utf8(snapshot[kind_name_end:message_end])
    is Some(message) else {
    return None
  }
  Some({
    kind: bridge_error_kind(kind_code, kind_name),
    status: bridge_error_status(status_code),
    message,
  })
}

///|
fn take_bridge_error(
  fallback_kind : ErrorKind,
  fallback_message : String,
  context : ErrorContext,
) -> OpenDalError {
  let error = bridge_last_error_take()
  if error == 0U {
    local_error(fallback_kind, fallback_message, context)
  } else {
    take_owned_bridge_error(error, fallback_kind, fallback_message, context)
  }
}

///|
fn take_owned_bridge_error(
  error : UInt,
  fallback_kind : ErrorKind,
  fallback_message : String,
  context : ErrorContext,
) -> OpenDalError {
  if error == 0U {
    return local_error(fallback_kind, fallback_message, context)
  }
  let snapshot_buffer = bridge_error_snapshot_take(error)
  if snapshot_buffer == 0U {
    ignore(bridge_error_release(error))
    ignore(bridge_last_error_clear())
    return local_error(
      Unexpected,
      "could not materialize the OpenDAL bridge error snapshot",
      context,
    )
  }
  let bytes = copy_buffer_from_bridge_without_error_snapshot(snapshot_buffer)
  ignore(bridge_buffer_release(snapshot_buffer))
  match bytes {
    None =>
      local_error(
        Unexpected,
        "could not transfer the OpenDAL bridge error snapshot",
        context,
      )
    Some(bytes) =>
      match parse_bridge_error_snapshot(bytes) {
        Some(snapshot) => error_from_snapshot(snapshot, context)
        None =>
          local_error(
            AbiMismatch,
            "OpenDAL bridge returned a malformed error snapshot",
            context,
          )
      }
  }
}

///|
fn copy_buffer_from_bridge_without_error_snapshot(handle : UInt) -> Bytes? {
  let length = bridge_buffer_len(handle)
  if length < 0 || length > MAX_TRANSFER_BUFFER_LENGTH {
    ignore(bridge_last_error_clear())
    return None
  }
  let output = Bytes::make(length, b'\x00')
  if length == 0 {
    return Some(output)
  }
  let base = bytes_address(output)
  let mut offset = 0
  while offset < length {
    let remaining = length - offset
    let chunk = if remaining > MAX_TRANSFER_CHUNK_LENGTH {
      MAX_TRANSFER_CHUNK_LENGTH
    } else {
      remaining
    }
    let status = host_copy_bridge_to_moon(
      handle,
      offset.reinterpret_as_uint(),
      base,
      length.reinterpret_as_uint(),
      offset.reinterpret_as_uint(),
      chunk.reinterpret_as_uint(),
    )
    if status != 0U {
      ignore(bridge_last_error_clear())
      return None
    }
    offset = offset + chunk
  }
  Some(output)
}

///|
fn raise_last_bridge_error(
  fallback_kind : ErrorKind,
  fallback_message : String,
  context : ErrorContext,
) -> Unit raise OpenDalError {
  raise take_bridge_error(fallback_kind, fallback_message, context)
}

///|
fn copy_bytes_to_bridge(
  bytes : BytesView,
  context : ErrorContext,
) -> UInt raise OpenDalError {
  let length = bytes.length()
  if length < 0 || length > MAX_TRANSFER_BUFFER_LENGTH {
    raise local_error(
      BufferTooLarge,
      "buffer exceeds the 64 MiB Wasm transfer limit",
      context,
    )
  }
  let handle = bridge_buffer_new_sized(length.reinterpret_as_uint())
  if handle == 0U {
    raise_last_bridge_error(
      Unexpected,
      "could not allocate a bridge buffer",
      context,
    )
  }
  if length == 0 {
    return handle
  }
  let data = bytes.data()
  let base = bytes_address(data)
  let data_length = data.length()
  let view_offset = bytes.start_offset()
  let mut offset = 0
  while offset < length {
    let remaining = length - offset
    let chunk = if remaining > MAX_TRANSFER_CHUNK_LENGTH {
      MAX_TRANSFER_CHUNK_LENGTH
    } else {
      remaining
    }
    let status = host_copy_moon_to_bridge(
      handle,
      offset.reinterpret_as_uint(),
      base,
      data_length.reinterpret_as_uint(),
      (view_offset + offset).reinterpret_as_uint(),
      chunk.reinterpret_as_uint(),
    )
    if status != 0U {
      let error = take_bridge_error(
        Unexpected,
        "could not copy a MoonBit byte window into the bridge",
        context,
      )
      ignore(bridge_buffer_release(handle))
      raise error
    }
    offset = offset + chunk
  }
  handle
}

///|
fn copy_buffer_from_bridge(
  handle : UInt,
  context : ErrorContext,
) -> Bytes raise OpenDalError {
  let length = bridge_buffer_len(handle)
  if length < 0 {
    raise_last_bridge_error(
      Unexpected,
      "could not query a bridge buffer length",
      context,
    )
  }
  if length > MAX_TRANSFER_BUFFER_LENGTH {
    raise local_error(
      BufferTooLarge,
      "buffer exceeds the 64 MiB Wasm transfer limit",
      context,
    )
  }
  let output = Bytes::make(length, b'\x00')
  if length == 0 {
    return output
  }
  let base = bytes_address(output)
  let mut offset = 0
  while offset < length {
    let remaining = length - offset
    let chunk = if remaining > MAX_TRANSFER_CHUNK_LENGTH {
      MAX_TRANSFER_CHUNK_LENGTH
    } else {
      remaining
    }
    let status = host_copy_bridge_to_moon(
      handle,
      offset.reinterpret_as_uint(),
      base,
      length.reinterpret_as_uint(),
      offset.reinterpret_as_uint(),
      chunk.reinterpret_as_uint(),
    )
    if status != 0U {
      raise take_bridge_error(
        Unexpected,
        "could not copy a bridge byte window into MoonBit",
        context,
      )
    }
    offset = offset + chunk
  }
  output
}

///|
fn copy_path_to_bridge(
  path : StringView,
  context : ErrorContext,
) -> UInt raise OpenDalError {
  let mut expect_low_surrogate = false
  for code_unit in path.code_units() {
    let value = code_unit.to_int()
    if expect_low_surrogate {
      if value < 0xdc00 || value > 0xdfff {
        raise local_error(
          InvalidArgument,
          "path contains invalid UTF-16",
          context,
        )
      }
      expect_low_surrogate = false
    } else if value >= 0xd800 && value <= 0xdbff {
      expect_low_surrogate = true
    } else if value >= 0xdc00 && value <= 0xdfff {
      raise local_error(
        InvalidArgument,
        "path contains invalid UTF-16",
        context,
      )
    }
  }
  if expect_low_surrogate {
    raise local_error(InvalidArgument, "path contains invalid UTF-16", context)
  }
  copy_bytes_to_bridge(@utf8.encode(path), context)
}

///|
fn combine_u64(low : UInt, high : UInt) -> UInt64 {
  (high.to_uint64() << 32) | low.to_uint64()
}
