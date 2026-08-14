///|
fn browser_metadata_contract(metadata : Metadata, entry : Entry) -> Unit {
  ignore(metadata.mode)
  ignore(metadata.content_length)
  ignore(metadata.is_current)
  ignore(metadata.cache_control)
  ignore(metadata.last_modified)
  ignore(metadata.version)
  ignore(metadata.is_file())
  ignore(metadata.is_dir())
  ignore(entry.path)
  ignore(entry.name)
  ignore(entry.metadata)
  ignore(entry.is_file())
  ignore(entry.is_dir())
}

///|
async fn browser_async_api_contract(
  runtime : Runtime,
  operator : Operator,
  asynchronous : AsyncOperator,
) -> Unit {
  ignore(runtime.available_schemes())
  ignore(runtime.operator("memory"))
  ignore(operator.info())
  ignore(operator.as_async())
  asynchronous.create_dir("contract/")
  ignore(asynchronous.write("contract/value.bin", b"value"))
  ignore(asynchronous.read("contract/value.bin"))
  ignore(asynchronous.stat("contract/value.bin"))
  ignore(asynchronous.exists("contract/value.bin"))
  asynchronous.delete("contract/value.bin")
  ignore(asynchronous.list("contract/"))
  asynchronous.close()
  runtime.close()
}

///|
test "browser public surface type-checks from a black-box test" {
  ignore(browser_metadata_contract)
  ignore(browser_async_api_contract)
  let assets = BrowserAssets::new(
    "https://example.invalid/runtime.mjs", "https://example.invalid/bridge.mjs",
    "https://example.invalid/bridge.wasm",
  )
  assert_eq(assets.runtime_module_url, "https://example.invalid/runtime.mjs")
  ignore(Runtime::load)
}
