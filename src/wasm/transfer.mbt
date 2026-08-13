///|
const EXPECTED_ABI_VERSION = 0x0001_0004U

///|
const REQUIRED_FEATURE_FLAGS = 0x0000_01ffU

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
fn take_bridge_error(
  fallback_code : Int,
  fallback_message : String,
) -> WasmError {
  let error = bridge_last_error_take()
  if error == 0U {
    WasmError(fallback_code, fallback_message)
  } else {
    take_owned_bridge_error(error, fallback_code, fallback_message)
  }
}

///|
fn take_owned_bridge_error(
  error : UInt,
  fallback_code : Int,
  fallback_message : String,
) -> WasmError {
  if error == 0U {
    return WasmError(fallback_code, fallback_message)
  }
  let code = bridge_error_code(error).reinterpret_as_int()
  let message_buffer = bridge_error_message(error)
  let message = if message_buffer == 0U {
    fallback_message
  } else {
    let bytes = copy_buffer_from_bridge_without_error_snapshot(message_buffer)
    ignore(bridge_buffer_release(message_buffer))
    @utf8.decode_lossy(bytes)
  }
  ignore(bridge_error_release(error))
  WasmError(code, message)
}

///|
fn copy_buffer_from_bridge_without_error_snapshot(handle : UInt) -> Bytes {
  let length = bridge_buffer_len(handle)
  if length < 0 || length > MAX_TRANSFER_BUFFER_LENGTH {
    ignore(bridge_last_error_clear())
    return b""
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
      ignore(bridge_last_error_clear())
      return b""
    }
    offset = offset + chunk
  }
  output
}

///|
fn raise_last_bridge_error(fallback_message : String) -> Unit raise WasmError {
  let code = bridge_last_error_code().reinterpret_as_int()
  raise take_bridge_error(code, fallback_message)
}

///|
fn copy_bytes_to_bridge(bytes : BytesView) -> UInt raise WasmError {
  let length = bytes.length()
  if length < 0 || length > MAX_TRANSFER_BUFFER_LENGTH {
    raise WasmError(3, "buffer exceeds the 64 MiB Wasm transfer limit")
  }
  let handle = bridge_buffer_new_sized(length.reinterpret_as_uint())
  if handle == 0U {
    raise_last_bridge_error("could not allocate a bridge buffer")
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
        status.reinterpret_as_int(),
        "could not copy a MoonBit byte window into the bridge",
      )
      ignore(bridge_buffer_release(handle))
      raise error
    }
    offset = offset + chunk
  }
  handle
}

///|
fn copy_buffer_from_bridge(handle : UInt) -> Bytes raise WasmError {
  let length = bridge_buffer_len(handle)
  if length < 0 {
    raise_last_bridge_error("could not query a bridge buffer length")
  }
  if length > MAX_TRANSFER_BUFFER_LENGTH {
    raise WasmError(3, "buffer exceeds the 64 MiB Wasm transfer limit")
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
        status.reinterpret_as_int(),
        "could not copy a bridge byte window into MoonBit",
      )
    }
    offset = offset + chunk
  }
  output
}

///|
fn copy_path_to_bridge(path : StringView) -> UInt raise WasmError {
  let mut expect_low_surrogate = false
  for code_unit in path.code_units() {
    let value = code_unit.to_int()
    if expect_low_surrogate {
      if value < 0xdc00 || value > 0xdfff {
        raise WasmError(6, "path contains invalid UTF-16")
      }
      expect_low_surrogate = false
    } else if value >= 0xd800 && value <= 0xdbff {
      expect_low_surrogate = true
    } else if value >= 0xdc00 && value <= 0xdfff {
      raise WasmError(6, "path contains invalid UTF-16")
    }
  }
  if expect_low_surrogate {
    raise WasmError(6, "path contains invalid UTF-16")
  }
  copy_bytes_to_bridge(@utf8.encode(path))
}

///|
fn combine_u64(low : UInt, high : UInt) -> UInt64 {
  (high.to_uint64() << 32) | low.to_uint64()
}
