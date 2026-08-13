///|
fn bridge_abi_version() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_abi_version"

///|
fn bridge_feature_flags() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_feature_flags"

///|
fn bridge_live_handle_count() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_live_handle_count"

///|
fn bridge_canary_forced_pending_poll_count() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_canary_forced_pending_poll_count"

///|
fn bridge_buffer_new() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_buffer_new"

///|
fn bridge_buffer_push(handle : UInt, byte : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_buffer_push"

///|
fn bridge_buffer_len(handle : UInt) -> Int = "opendal_mbt_bridge" "opendal_mbt_wasm_buffer_len"

///|
fn bridge_buffer_get(handle : UInt, index : UInt) -> Int = "opendal_mbt_bridge" "opendal_mbt_wasm_buffer_get"

///|
fn bridge_buffer_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_buffer_release"

///|
fn bridge_registered_schemes() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_registered_schemes"

///|
fn bridge_operator_builder_new(scheme : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_builder_new"

///|
fn bridge_operator_builder_set(
  builder : UInt,
  key : UInt,
  value : UInt,
) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_builder_set"

///|
fn bridge_operator_builder_build(builder : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_builder_build"

///|
fn bridge_operator_builder_release(builder : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_builder_release"

///|
fn bridge_operator_info_scheme(operator : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_info_scheme"

///|
fn bridge_operator_info_root(operator : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_info_root"

///|
fn bridge_operator_info_name(operator : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_info_name"

///|
fn bridge_operator_info_capability_word(operator : UInt, word : UInt) -> UInt64 = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_info_capability_word"

///|
fn bridge_operator_write(operator : UInt, path : UInt, data : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_write"

///|
fn bridge_operator_read(operator : UInt, path : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_read"

///|
fn bridge_operator_stat(operator : UInt, path : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_stat"

///|
fn bridge_operator_write_start(
  operator : UInt,
  path : UInt,
  data : UInt,
) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_write_start"

///|
fn bridge_operator_read_start(operator : UInt, path : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_read_start"

///|
fn bridge_operator_stat_start(operator : UInt, path : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_stat_start"

///|
fn bridge_operator_create_dir_start(operator : UInt, path : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_create_dir_start"

///|
fn bridge_operator_delete_start(
  operator : UInt,
  path : UInt,
  version : UInt,
  recursive : Bool,
) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_delete_start"

///|
fn bridge_task_state(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_task_state"

///|
fn bridge_task_take(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_task_take"

///|
fn bridge_task_cancel(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_task_cancel"

///|
fn bridge_task_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_task_release"

///|
fn bridge_completion_kind(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_kind"

///|
fn bridge_completion_status(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_status"

///|
fn bridge_completion_take_buffer(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_take_buffer"

///|
fn bridge_completion_take_metadata(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_take_metadata"

///|
fn bridge_completion_take_error(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_take_error"

///|
fn bridge_completion_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_release"

///|
fn host_wait_task(handle : UInt, callback : (UInt) -> Unit) -> Unit = "opendal_mbt_host" "wait_task"

///|
fn bridge_operator_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_release"

///|
fn bridge_metadata_content_length_low(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_content_length_low"

///|
fn bridge_metadata_content_length_high(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_content_length_high"

///|
fn bridge_metadata_is_file(handle : UInt) -> Int = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_is_file"

///|
fn bridge_metadata_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_release"

///|
fn bridge_last_error_code() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_last_error_code"

///|
fn bridge_last_error_take() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_last_error_take"

///|
fn bridge_last_error_clear() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_last_error_clear"

///|
fn bridge_error_code(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_error_code"

///|
fn bridge_error_message(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_error_message"

///|
fn bridge_error_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_error_release"
