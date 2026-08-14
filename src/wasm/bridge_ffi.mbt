///|
fn bridge_abi_version() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_abi_version"

///|
fn bridge_feature_flags() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_feature_flags"

///|
fn bridge_buffer_new_sized(length : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_buffer_new_sized"

///|
fn bridge_buffer_len(handle : UInt) -> Int = "opendal_mbt_bridge" "opendal_mbt_wasm_buffer_len"

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
fn bridge_operator_list_start(
  operator : UInt,
  path : UInt,
  recursive : Bool,
  has_limit : Bool,
  limit : UInt64,
  start_after : UInt,
) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_list_start"

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
fn bridge_completion_take_entry_list(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_take_entry_list"

///|
fn bridge_completion_take_error(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_take_error"

///|
fn bridge_completion_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_completion_release"

///|
fn bridge_entry_list_len(handle : UInt) -> Int = "opendal_mbt_bridge" "opendal_mbt_wasm_entry_list_len"

///|
fn bridge_entry_list_path(handle : UInt, index : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_entry_list_path"

///|
fn bridge_entry_list_name(handle : UInt, index : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_entry_list_name"

///|
fn bridge_entry_list_mode(handle : UInt, index : UInt) -> Int = "opendal_mbt_bridge" "opendal_mbt_wasm_entry_list_mode"

///|
fn bridge_entry_list_content_length(handle : UInt, index : UInt) -> UInt64 = "opendal_mbt_bridge" "opendal_mbt_wasm_entry_list_content_length"

///|
fn bridge_entry_list_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_entry_list_release"

///|
fn host_wait_task(handle : UInt, callback : (UInt) -> Unit) -> Unit = "opendal_mbt_host" "wait_task"

///|
fn host_cancel_wait(handle : UInt) -> Unit = "opendal_mbt_host" "cancel_wait"

///|
fn host_copy_moon_to_bridge(
  handle : UInt,
  bridge_offset : UInt,
  moon_base : UInt,
  moon_data_length : UInt,
  moon_offset : UInt,
  length : UInt,
) -> UInt = "opendal_mbt_host" "copy_moon_to_bridge"

///|
fn host_copy_bridge_to_moon(
  handle : UInt,
  bridge_offset : UInt,
  moon_base : UInt,
  moon_data_length : UInt,
  moon_offset : UInt,
  length : UInt,
) -> UInt = "opendal_mbt_host" "copy_bridge_to_moon"

///|
fn bridge_operator_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_release"

///|
fn bridge_metadata_content_length_low(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_content_length_low"

///|
fn bridge_metadata_content_length_high(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_content_length_high"

///|
fn bridge_metadata_mode(handle : UInt) -> Int = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_mode"

///|
fn bridge_metadata_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_metadata_release"

///|
fn bridge_last_error_take() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_last_error_take"

///|
fn bridge_last_error_clear() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_last_error_clear"

///|
fn bridge_error_snapshot_take(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_error_snapshot_take"

///|
fn bridge_error_release(handle : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_error_release"
