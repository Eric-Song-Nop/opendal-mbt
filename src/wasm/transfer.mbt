///|
const EXPECTED_ABI_VERSION = 0x0001_0003U

///|
const REQUIRED_FEATURE_FLAGS = 0x0000_00ffU

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
  if length < 0 {
    return b""
  }
  Bytes::makei(length, index => {
    let value = bridge_buffer_get(handle, index.reinterpret_as_uint())
    if value < 0 {
      b'\x00'
    } else {
      value.to_byte()
    }
  })
}

///|
fn raise_last_bridge_error(fallback_message : String) -> Unit raise WasmError {
  let code = bridge_last_error_code().reinterpret_as_int()
  raise take_bridge_error(code, fallback_message)
}

///|
fn copy_bytes_to_bridge(bytes : BytesView) -> UInt raise WasmError {
  let handle = bridge_buffer_new()
  if handle == 0U {
    raise_last_bridge_error("could not allocate a bridge buffer")
  }
  for byte in bytes {
    let status = bridge_buffer_push(handle, byte.to_int().reinterpret_as_uint())
    if status != 0U {
      let error = take_bridge_error(
        status.reinterpret_as_int(),
        "could not copy a byte into the bridge",
      )
      ignore(bridge_buffer_release(handle))
      raise error
    }
  }
  handle
}

///|
fn copy_buffer_from_bridge(handle : UInt) -> Bytes raise WasmError {
  let length = bridge_buffer_len(handle)
  if length < 0 {
    raise_last_bridge_error("could not query a bridge buffer length")
  }
  Bytes::makei(length, index => {
    let value = bridge_buffer_get(handle, index.reinterpret_as_uint())
    if value < 0 {
      raise_last_bridge_error("could not copy a byte from the bridge")
    }
    value.to_byte()
  })
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
