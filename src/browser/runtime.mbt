///|
/// URLs for the version-matched JavaScript runtime, wasm-bindgen module, and
/// OpenDAL bridge WebAssembly binary.
///
/// `Runtime::load` imports these browser assets once. The embedded distribution
/// layer added by the next stack PR supplies these values automatically; this
/// explicit form remains useful to bundlers and custom CDNs.
pub struct BrowserAssets {
  runtime_module_url : String
  bridge_module_url : String
  bridge_wasm_url : String
} derive(Eq, @debug.Debug)

///|
/// Construct an explicit set of version-matched browser asset URLs.
pub fn BrowserAssets::new(
  runtime_module_url : String,
  bridge_module_url : String,
  bridge_wasm_url : String,
) -> BrowserAssets {
  { runtime_module_url, bridge_module_url, bridge_wasm_url }
}

///|
/// An initialized, single-memory OpenDAL browser runtime.
///
/// The runtime owns its WebAssembly instance. Close it after all operators are
/// no longer needed; closing more than once is harmless.
pub struct Runtime {
  priv js : JsRuntime
  priv mut closed : Bool
}

///|
/// A configured OpenDAL browser operator.
///
/// Configuration and service identity are copied during construction. Storage
/// operations are exposed through `as_async` because browsers cannot perform
/// synchronous OpenDAL I/O without blocking the JavaScript event loop.
pub struct Operator {
  priv js : JsOperator
  priv runtime : Runtime
  priv info_snapshot : OperatorInfo
  priv mut closed : Bool
}

///|
/// A lightweight asynchronous view that shares one browser operator.
pub struct AsyncOperator {
  priv native : Operator
}

///|
fn bridge_error(outcome : JsOutcome, context : ErrorContext) -> OpenDalError {
  let error = outcome.error()
  error_from_bridge(
    error.kind_code(),
    error.status_code(),
    error.kind_name(),
    error.message(),
    context,
  )
}

///|
fn[X] outcome_value(
  outcome : JsOutcome,
  get_value : (JsOutcome) -> X,
  context : ErrorContext,
) -> X raise OpenDalError {
  if outcome.is_ok() {
    get_value(outcome)
  } else {
    let error = outcome.error()
    raise error_from_bridge(
      error.kind_code(),
      error.status_code(),
      error.kind_name(),
      error.message(),
      context,
    )
  }
}

///|
fn outcome_unit(
  outcome : JsOutcome,
  context : ErrorContext,
) -> Unit raise OpenDalError {
  if !outcome.is_ok() {
    raise bridge_error(outcome, context)
  }
}

///|
fn metadata_from_js(metadata : JsMetadata) -> Metadata {
  {
    mode: match metadata.mode() {
      1 => File
      2 => Directory
      _ => Unknown
    },
    content_length: metadata.content_length(),
    is_current: if metadata.has_is_current() {
      Some(metadata.is_current())
    } else {
      None
    },
    is_deleted: metadata.is_deleted(),
    cache_control: if metadata.has_cache_control() {
      Some(metadata.cache_control())
    } else {
      None
    },
    content_disposition: if metadata.has_content_disposition() {
      Some(metadata.content_disposition())
    } else {
      None
    },
    content_encoding: if metadata.has_content_encoding() {
      Some(metadata.content_encoding())
    } else {
      None
    },
    content_md5: if metadata.has_content_md5() {
      Some(metadata.content_md5())
    } else {
      None
    },
    content_type: if metadata.has_content_type() {
      Some(metadata.content_type())
    } else {
      None
    },
    etag: if metadata.has_etag() {
      Some(metadata.etag())
    } else {
      None
    },
    last_modified: if metadata.has_last_modified() {
      Some({
        unix_seconds: metadata.last_modified_seconds(),
        nanoseconds: metadata.last_modified_nanoseconds(),
      })
    } else {
      None
    },
    version: if metadata.has_version() {
      Some(metadata.version())
    } else {
      None
    },
  }
}

///|
fn entry_from_js(entry : JsEntry) -> Entry {
  {
    path: entry.path(),
    name: entry.name(),
    metadata: metadata_from_js(entry.metadata()),
  }
}

///|
fn operator_info_from_js(info : JsOperatorInfo) -> OperatorInfo {
  let capability = info.capability()
  {
    scheme: info.scheme(),
    root: info.root(),
    name: info.name(),
    capability: {
      word0: capability.word0(),
      delete_max_size_value: if capability.has_delete_max_size() {
        capability.delete_max_size()
      } else {
        0UL
      },
    },
  }
}

///|
async fn await_runtime_bootstrap(
  promise : @js_async.Promise[JsRuntime],
  failure_message : String,
) -> JsRuntime {
  promise.wait() catch {
    error if @async.is_being_cancelled() => raise error
    _ =>
      raise local_error(
        AbiMismatch,
        failure_message,
        error_context(LoadRuntime),
      )
  }
}

///|
/// Load and validate a version-matched OpenDAL browser runtime.
///
/// Module loading, ABI mismatch, or malformed bootstrap state raises
/// `AbiMismatch`. Storage failures after initialization retain their structured
/// OpenDAL category and retry status. Moon async scheduler cancellation
/// propagates unchanged.
pub async fn Runtime::load(assets : BrowserAssets) -> Runtime {
  let js = await_runtime_bootstrap(
    load_js_runtime(
      assets.runtime_module_url,
      assets.bridge_module_url,
      assets.bridge_wasm_url,
    ),
    "could not load or validate the OpenDAL browser runtime",
  )
  { js, closed: false }
}

///|
/// Return the service schemes compiled into this exact browser artifact.
pub fn Runtime::available_schemes(
  self : Runtime,
) -> Array[String] raise OpenDalError {
  outcome_value(
    self.js.available_schemes(),
    outcome => outcome.schemes_value(),
    error_context(Check),
  )
}

///|
/// Construct an operator from a service scheme and copied configuration map.
pub fn Runtime::operator(
  self : Runtime,
  scheme : StringView,
  config? : Map[String, String] = Map([]),
) -> Operator raise OpenDalError {
  let context = error_context(NewOperator)
  if self.closed {
    raise local_error(ResourceClosed, "browser runtime is closed", context)
  }
  let keys : Array[String] = []
  let values : Array[String] = []
  for key, value in config {
    keys.push(key)
    values.push(value)
  }
  let js = outcome_value(
    self.js.operator(scheme.to_owned(), keys, values),
    outcome => outcome.operator_value(),
    context,
  )
  let info_snapshot = operator_info_from_js(
    outcome_value(js.info(), outcome => outcome.operator_info_value(), context),
  )
  { js, runtime: self, info_snapshot, closed: false }
}

///|
/// Release every operator and bridge resource owned by this runtime.
///
/// Closing more than once is harmless. A late task completion cannot revive a
/// resource after this method returns.
pub fn Runtime::close(self : Runtime) -> Unit raise OpenDalError {
  if !self.closed {
    self.closed = true
    outcome_unit(self.js.close(), error_context(Check))
  }
}

///|
/// Return the immutable service identity and capability snapshot.
pub fn Operator::info(self : Operator) -> OperatorInfo {
  self.info_snapshot
}

///|
/// Create a lightweight asynchronous view without performing I/O.
pub fn Operator::as_async(self : Operator) -> AsyncOperator {
  { native: self }
}

///|
/// Release this operator early. Closing more than once is harmless.
pub fn Operator::close(self : Operator) -> Unit raise OpenDalError {
  if !self.closed {
    self.closed = true
    outcome_unit(self.js.close(), error_context(Check))
  }
}

///|
/// Release the shared operator through its asynchronous view.
pub fn AsyncOperator::close(self : AsyncOperator) -> Unit raise OpenDalError {
  self.native.close()
}
