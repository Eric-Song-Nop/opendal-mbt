///|
/// Return the bridge ABI version expected by this facade.
pub fn bridge_version() -> UInt {
  bridge_abi_version()
}

///|
/// Return the feature bitmap implemented by the loaded bridge.
pub fn bridge_features() -> UInt {
  bridge_feature_flags()
}

///|
/// Return the number of currently live bridge handles.
///
/// This is a canary leak oracle and not a stable production metric.
pub fn live_handle_count() -> UInt {
  bridge_live_handle_count()
}

///|
/// Return how many asynchronous canary tasks reached an actual pending poll.
///
/// This is a canary oracle and not a stable production metric.
pub fn canary_forced_pending_poll_count() -> UInt {
  bridge_canary_forced_pending_poll_count()
}

///|
/// Create an OpenDAL in-memory operator.
pub fn Operator::memory() -> Operator raise WasmError {
  ignore(bridge_last_error_clear())
  let actual = bridge_abi_version()
  if actual != EXPECTED_ABI_VERSION {
    raise WasmError(
      0,
      "OpenDAL Wasm ABI mismatch: expected \{EXPECTED_ABI_VERSION}, found \{actual}",
    )
  }
  let features = bridge_feature_flags()
  if (features & REQUIRED_FEATURE_FLAGS) != REQUIRED_FEATURE_FLAGS {
    raise WasmError(0, "OpenDAL Wasm bridge is missing required features")
  }
  let handle = bridge_operator_new_memory()
  if handle == 0U {
    raise_last_bridge_error("could not create an OpenDAL memory operator")
  }
  { handle, }
}

///|
/// Write an owned byte snapshot to one path.
pub fn Operator::write(
  self : Operator,
  path : StringView,
  data : BytesView,
) -> Unit raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let data_handle = copy_bytes_to_bridge(data) catch {
    error => {
      ignore(bridge_buffer_release(path_handle))
      raise error
    }
  }
  let status = bridge_operator_write(self.handle, path_handle, data_handle)
  let error = if status == 0U {
    None
  } else {
    Some(
      take_bridge_error(
        status.reinterpret_as_int(),
        "write failed with bridge status \{status}",
      ),
    )
  }
  ignore(bridge_buffer_release(data_handle))
  ignore(bridge_buffer_release(path_handle))
  match error {
    Some(error) => raise error
    None => ()
  }
}

///|
/// Read one complete object into a MoonBit-owned `Bytes` value.
pub fn Operator::read(
  self : Operator,
  path : StringView,
) -> Bytes raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let result = bridge_operator_read(self.handle, path_handle)
  if result == 0U {
    let error = take_bridge_error(
      bridge_last_error_code().reinterpret_as_int(),
      "read failed",
    )
    ignore(bridge_buffer_release(path_handle))
    raise error
  }
  ignore(bridge_buffer_release(path_handle))
  let bytes = copy_buffer_from_bridge(result) catch {
    error => {
      ignore(bridge_buffer_release(result))
      raise error
    }
  }
  ignore(bridge_buffer_release(result))
  bytes
}

///|
/// Return a small immutable metadata snapshot for one object.
pub fn Operator::stat(
  self : Operator,
  path : StringView,
) -> Metadata raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let path_handle = copy_path_to_bridge(path)
  let metadata = bridge_operator_stat(self.handle, path_handle)
  if metadata == 0U {
    let error = take_bridge_error(
      bridge_last_error_code().reinterpret_as_int(),
      "stat failed",
    )
    ignore(bridge_buffer_release(path_handle))
    raise error
  }
  ignore(bridge_buffer_release(path_handle))
  let is_file = bridge_metadata_is_file(metadata)
  if is_file < 0 {
    let error = take_bridge_error(-1, "could not inspect metadata")
    ignore(bridge_metadata_release(metadata))
    raise error
  }
  ignore(bridge_last_error_clear())
  let content_length_low = bridge_metadata_content_length_low(metadata)
  if bridge_last_error_code() != 0U {
    let error = take_bridge_error(-1, "could not read metadata content length")
    ignore(bridge_metadata_release(metadata))
    raise error
  }
  let content_length_high = bridge_metadata_content_length_high(metadata)
  if bridge_last_error_code() != 0U {
    let error = take_bridge_error(-1, "could not read metadata content length")
    ignore(bridge_metadata_release(metadata))
    raise error
  }
  let content_length = combine_u64(content_length_low, content_length_high)
  ignore(bridge_metadata_release(metadata))
  { content_length, is_file: is_file != 0 }
}

///|
/// Release bridge resources early. Closing more than once is harmless.
pub fn Operator::close(self : Operator) -> Unit {
  if self.handle != 0U {
    ignore(bridge_operator_release(self.handle))
    self.handle = 0U
  }
}
