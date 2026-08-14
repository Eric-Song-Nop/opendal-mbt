///|
fn require_bridge_contract() -> Unit raise OpenDalError {
  let context = error_context(NewOperator)
  ignore(bridge_last_error_clear())
  let actual = bridge_abi_version()
  if actual != EXPECTED_ABI_VERSION {
    raise local_error(
      AbiMismatch,
      "OpenDAL Wasm ABI mismatch: expected \{EXPECTED_ABI_VERSION}, found \{actual}",
      context,
    )
  }
  let features = bridge_feature_flags()
  if (features & REQUIRED_FEATURE_FLAGS) != REQUIRED_FEATURE_FLAGS {
    raise local_error(
      AbiMismatch,
      "OpenDAL Wasm bridge is missing required features",
      context,
    )
  }
}

///|
fn take_utf8_buffer(
  handle : UInt,
  fallback : String,
  context : ErrorContext,
) -> String raise OpenDalError {
  if handle == 0U {
    raise_last_bridge_error(Unexpected, fallback, context)
  }
  let bytes = copy_buffer_from_bridge(handle, context) catch {
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
  context : ErrorContext,
) -> Unit raise OpenDalError {
  let key_handle = copy_path_to_bridge(key, context)
  let value_handle = copy_path_to_bridge(value, context) catch {
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
        InvalidArgument,
        "could not copy operator configuration",
        context,
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
pub fn available_schemes() -> Array[String] raise OpenDalError {
  require_bridge_contract()
  let context = error_context(NewOperator)
  ignore(bridge_last_error_clear())
  let encoded = take_utf8_buffer(
    bridge_registered_schemes(),
    "could not query registered OpenDAL schemes",
    context,
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
) -> Operator raise OpenDalError {
  require_bridge_contract()
  let context = error_context(NewOperator)
  ignore(bridge_last_error_clear())
  let scheme_handle = copy_path_to_bridge(scheme, context)
  let builder = bridge_operator_builder_new(scheme_handle)
  ignore(bridge_buffer_release(scheme_handle))
  if builder == 0U {
    raise_last_bridge_error(
      InvalidArgument,
      "could not create an OpenDAL operator builder",
      context,
    )
  }
  for key, value in config {
    set_builder_config(builder, key, value, context) catch {
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
        ConfigInvalid,
        "could not build an OpenDAL operator",
        context,
      ),
    )
  } else {
    None
  }
  ignore(bridge_operator_builder_release(builder))
  match error {
    Some(error) => raise error
    None => {
      let info_snapshot = materialize_operator_info(handle, context) catch {
        error => {
          ignore(bridge_operator_release(handle))
          raise error
        }
      }
      { handle, info_snapshot }
    }
  }
}

///|
fn materialize_operator_info(
  handle : UInt,
  context : ErrorContext,
) -> OperatorInfo raise OpenDalError {
  ignore(bridge_last_error_clear())
  let scheme = take_utf8_buffer(
    bridge_operator_info_scheme(handle),
    "could not query the operator scheme",
    context,
  )
  let root = take_utf8_buffer(
    bridge_operator_info_root(handle),
    "could not query the operator root",
    context,
  )
  let name = take_utf8_buffer(
    bridge_operator_info_name(handle),
    "could not query the operator name",
    context,
  )
  ignore(bridge_last_error_clear())
  let word0 = bridge_operator_info_capability_word(handle, 0U)
  let delete_max_size_value = bridge_operator_info_capability_word(handle, 1U)
  let error = bridge_last_error_take()
  if error != 0U {
    raise take_owned_bridge_error(
      error,
      Unexpected,
      "could not query operator capabilities",
      context,
    )
  }
  { scheme, root, name, capability: { word0, delete_max_size_value } }
}

///|
/// Return the service identity and effective capabilities for this operator.
pub fn Operator::info(self : Operator) -> OperatorInfo {
  self.info_snapshot
}

///|
/// Create a lightweight asynchronous view that shares this Operator.
pub fn Operator::as_async(self : Operator) -> AsyncOperator {
  { native: self }
}

///|
/// Release the shared operator through this asynchronous view.
///
/// This explicit lifecycle extension is specific to the Wasm binding.
pub fn AsyncOperator::close(self : AsyncOperator) -> Unit {
  self.native.close()
}

///|
/// Release bridge resources early. Closing more than once is harmless.
pub fn Operator::close(self : Operator) -> Unit {
  if self.handle != 0U {
    ignore(bridge_operator_release(self.handle))
    self.handle = 0U
  }
}
