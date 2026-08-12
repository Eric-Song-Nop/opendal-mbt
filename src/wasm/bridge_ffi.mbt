///|
fn bridge_abi_version() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_abi_version"

///|
fn bridge_feature_flags() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_feature_flags"

///|
fn bridge_live_handle_count() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_live_handle_count"

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
fn bridge_operator_new_memory() -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_new_memory"

///|
fn bridge_operator_write(operator : UInt, path : UInt, data : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_write"

///|
fn bridge_operator_read(operator : UInt, path : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_read"

///|
fn bridge_operator_stat(operator : UInt, path : UInt) -> UInt = "opendal_mbt_bridge" "opendal_mbt_wasm_operator_stat"

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
