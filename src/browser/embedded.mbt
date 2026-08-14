///|
/// Initialize the version-matched OpenDAL runtime embedded in this Moon
/// package.
///
/// The embedded runtime needs no JavaScript bundler, CDN, or separately served
/// Wasm asset. It is decoded lazily and owns one OpenDAL WebAssembly instance.
/// Browsers must support `WebAssembly` and `DecompressionStream`. Moon async
/// scheduler cancellation propagates unchanged. The host page's Content
/// Security Policy must permit WebAssembly compilation, normally with
/// `script-src 'wasm-unsafe-eval'`.
pub async fn Runtime::new() -> Runtime {
  let js = await_runtime_bootstrap(
    load_embedded_js_runtime(embedded_runtime_parts()),
    "could not initialize the embedded OpenDAL browser runtime",
  )
  { js, closed: false }
}
