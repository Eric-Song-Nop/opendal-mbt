///|
fn require_bridge_contract() -> Unit raise WasmError {
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
}

///|
fn take_utf8_buffer(handle : UInt, fallback : String) -> String raise WasmError {
  if handle == 0U {
    raise_last_bridge_error(fallback)
  }
  let bytes = copy_buffer_from_bridge(handle) catch {
    error => {
      ignore(bridge_buffer_release(handle))
      raise error
    }
  }
  ignore(bridge_buffer_release(handle))
  @utf8.decode_lossy(bytes)
}

///|
fn set_builder_config(
  builder : UInt,
  key : String,
  value : String,
) -> Unit raise WasmError {
  let key_handle = copy_path_to_bridge(key)
  let value_handle = copy_path_to_bridge(value) catch {
    error => {
      ignore(bridge_buffer_release(key_handle))
      raise error
    }
  }
  let status = bridge_operator_builder_set(builder, key_handle, value_handle)
  let error = if status == 0U {
    None
  } else {
    Some(
      take_bridge_error(
        status.reinterpret_as_int(),
        "could not copy operator configuration",
      ),
    )
  }
  ignore(bridge_buffer_release(value_handle))
  ignore(bridge_buffer_release(key_handle))
  match error {
    Some(error) => raise error
    None => ()
  }
}

///|
/// Return the sorted schemes compiled into the loaded bridge artifact.
pub fn available_schemes() -> Array[String] raise WasmError {
  require_bridge_contract()
  ignore(bridge_last_error_clear())
  let encoded = take_utf8_buffer(
    bridge_registered_schemes(),
    "could not query registered OpenDAL schemes",
  )
  if encoded.is_empty() {
    []
  } else {
    encoded.split("\n").map(view => "\{view}").to_array()
  }
}

///|
/// Construct an operator from a service scheme and configuration map.
///
/// Only services compiled into the selected bridge artifact are available.
/// Use `available_schemes()` to inspect that exact runtime set.
pub fn Operator::new(
  scheme : StringView,
  config? : Map[String, String] = Map([]),
) -> Operator raise WasmError {
  require_bridge_contract()
  ignore(bridge_last_error_clear())
  let scheme_handle = copy_path_to_bridge(scheme)
  let builder = bridge_operator_builder_new(scheme_handle)
  ignore(bridge_buffer_release(scheme_handle))
  if builder == 0U {
    raise_last_bridge_error("could not create an OpenDAL operator builder")
  }
  for key, value in config {
    set_builder_config(builder, key, value) catch {
      error => {
        ignore(bridge_operator_builder_release(builder))
        raise error
      }
    }
  }
  let handle = bridge_operator_builder_build(builder)
  let error = if handle == 0U {
    Some(
      take_bridge_error(
        bridge_last_error_code().reinterpret_as_int(),
        "could not build an OpenDAL operator",
      ),
    )
  } else {
    None
  }
  ignore(bridge_operator_builder_release(builder))
  match error {
    Some(error) => raise error
    None => { handle, }
  }
}

///|
/// Return the service identity and effective capabilities for this operator.
pub fn Operator::info(self : Operator) -> OperatorInfo raise WasmError {
  if self.handle == 0U {
    raise WasmError(1, "operator is closed")
  }
  ignore(bridge_last_error_clear())
  let scheme = take_utf8_buffer(
    bridge_operator_info_scheme(self.handle),
    "could not query the operator scheme",
  )
  let root = take_utf8_buffer(
    bridge_operator_info_root(self.handle),
    "could not query the operator root",
  )
  let name = take_utf8_buffer(
    bridge_operator_info_name(self.handle),
    "could not query the operator name",
  )
  ignore(bridge_last_error_clear())
  let word0 = bridge_operator_info_capability_word(self.handle, 0U)
  let delete_max_size_value = bridge_operator_info_capability_word(
    self.handle,
    1U,
  )
  if bridge_last_error_code() != 0U {
    raise_last_bridge_error("could not query operator capabilities")
  }
  { scheme, root, name, capability: { word0, delete_max_size_value } }
}

///|
/// Release bridge resources early. Closing more than once is harmless.
pub fn Operator::close(self : Operator) -> Unit {
  if self.handle != 0U {
    ignore(bridge_operator_release(self.handle))
    self.handle = 0U
  }
}
