///|
fn normalize_optional_text(value : StringView?) -> (Bool, String) {
  match value {
    Some(value) => (true, value.to_owned())
    None => (false, "")
  }
}

///|
fn AsyncOperator::ensure_open(
  self : AsyncOperator,
  context : ErrorContext,
) -> Unit raise OpenDalError {
  if self.native.closed || self.native.runtime.closed {
    raise local_error(ResourceClosed, "browser operator is closed", context)
  }
}

///|
fn ByteRange::js_parts(
  self : ByteRange,
  capability : Capability,
  context : ErrorContext,
) -> (Int, UInt64, UInt64) raise OpenDalError {
  match self {
    Full => (0, 0UL, 0UL)
    From(offset~) => (1, offset, 0UL)
    Range(offset~, length~) => {
      if length > 18446744073709551615UL - offset {
        raise local_error(
          InvalidArgument,
          "byte range offset plus length exceeds UInt64",
          context,
        )
      }
      (2, offset, length)
    }
    Suffix(length~) => {
      if !capability.can_read_suffix() {
        raise local_error(
          Unsupported,
          "suffix reads require backend range support",
          context,
        )
      }
      (3, 0UL, length)
    }
  }
}

///|
async fn[X] await_outcome(
  start : (@js_async.AbortSignal) -> @js_async.Promise[JsOutcome],
  get_value : (JsOutcome) -> X,
  context : ErrorContext,
) -> X {
  let outcome = @js_async.run_promise(signal => start(signal)) catch {
    error if @async.is_being_cancelled() => raise error
    _ =>
      raise local_error(
        AbiMismatch,
        "OpenDAL browser Promise rejected instead of returning an outcome",
        context,
      )
  }
  outcome_value(outcome, get_value, context)
}

///|
async fn await_unit_outcome(
  start : (@js_async.AbortSignal) -> @js_async.Promise[JsOutcome],
  context : ErrorContext,
) -> Unit {
  let outcome = @js_async.run_promise(signal => start(signal)) catch {
    error if @async.is_being_cancelled() => raise error
    _ =>
      raise local_error(
        AbiMismatch,
        "OpenDAL browser Promise rejected instead of returning an outcome",
        context,
      )
  }
  outcome_unit(outcome, context)
}

///|
/// Create a directory recursively. A directory path must end with `/`.
pub async fn AsyncOperator::create_dir(
  self : AsyncOperator,
  path : StringView,
) -> Unit {
  let path = path.to_owned()
  let context = error_context(CreateDir, path~)
  self.ensure_open(context)
  await_unit_outcome(signal => self.native.js.create_dir(path, signal), context)
}

///|
/// Write an entire byte view and return the resulting owned metadata.
///
/// The selected view is copied into the bridge before asynchronous work starts.
pub async fn AsyncOperator::write(
  self : AsyncOperator,
  path : StringView,
  data : BytesView,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Metadata {
  let path = path.to_owned()
  let context = error_context(Write, path~)
  self.ensure_open(context)
  let (has_content_type, content_type) = normalize_optional_text(content_type)
  let (has_content_disposition, content_disposition) = normalize_optional_text(
    content_disposition,
  )
  let (has_content_encoding, content_encoding) = normalize_optional_text(
    content_encoding,
  )
  let (has_cache_control, cache_control) = normalize_optional_text(
    cache_control,
  )
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let metadata = await_outcome(
    signal => {
      self.native.js.write(
        path,
        data.to_owned(),
        append,
        has_content_type,
        content_type,
        has_content_disposition,
        content_disposition,
        has_content_encoding,
        content_encoding,
        has_cache_control,
        cache_control,
        has_if_match,
        if_match,
        has_if_none_match,
        if_none_match,
        signal,
      )
    },
    outcome => outcome.metadata_value(),
    context,
  )
  metadata_from_js(metadata)
}

///|
/// Read one path or byte range into an owned `Bytes` value.
///
/// Cancellation activates the JavaScript `AbortSignal`. Storage work already
/// dispatched by a backend may still complete, but its late result is discarded.
pub async fn AsyncOperator::read(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Bytes {
  let path = path.to_owned()
  let context = error_context(Read, path~)
  self.ensure_open(context)
  let (range_kind, range_offset, range_length) = range.js_parts(
    self.native.info_snapshot.capability,
    context,
  )
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  await_outcome(
    signal => {
      self.native.js.read(
        path, range_kind, range_offset, range_length, has_version, version, has_if_match,
        if_match, has_if_none_match, if_none_match, signal,
      )
    },
    outcome => outcome.bytes_value(),
    context,
  )
}

///|
/// Fetch a complete metadata snapshot with optional version conditions.
pub async fn AsyncOperator::stat(
  self : AsyncOperator,
  path : StringView,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Metadata {
  let path = path.to_owned()
  let context = error_context(Stat, path~)
  self.ensure_open(context)
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let metadata = await_outcome(
    signal => {
      self.native.js.stat(
        path, has_version, version, has_if_match, if_match, has_if_none_match, if_none_match,
        signal,
      )
    },
    outcome => outcome.metadata_value(),
    context,
  )
  metadata_from_js(metadata)
}

///|
/// Return whether a path exists. Only `NotFound` becomes `false`.
pub async fn AsyncOperator::exists(
  self : AsyncOperator,
  path : StringView,
) -> Bool {
  let path = path.to_owned()
  let context = error_context(Exists, path~)
  self.ensure_open(context)
  try
    await_outcome(
      signal => {
        self.native.js.stat(path, false, "", false, "", false, "", signal)
      },
      outcome => outcome.metadata_value(),
      context,
    )
  catch {
    OpenDalError(info) if info.kind is NotFound => false
    error => raise error
  } noraise {
    _ => true
  }
}

///|
/// Delete a path. Deleting a missing path is successful when the backend
/// follows OpenDAL delete semantics.
pub async fn AsyncOperator::delete(
  self : AsyncOperator,
  path : StringView,
  version? : StringView,
  recursive? : Bool = false,
) -> Unit {
  let path = path.to_owned()
  let context = error_context(Delete, path~)
  self.ensure_open(context)
  let (has_version, version) = normalize_optional_text(version)
  await_unit_outcome(
    signal => {
      self.native.js.delete(path, has_version, version, recursive, signal)
    },
    context,
  )
}

///|
/// Materialize one bounded prefix listing into owned MoonBit entries.
///
/// The browser bridge enforces hard entry and UTF-8 byte limits. `limit` is a
/// backend request hint rather than a total-result guarantee.
pub async fn AsyncOperator::list(
  self : AsyncOperator,
  path : StringView,
  recursive? : Bool = false,
  limit? : UInt64,
  start_after? : StringView,
) -> Array[Entry] {
  let path = path.to_owned()
  let context = error_context(List, path~)
  self.ensure_open(context)
  let (has_limit, limit) = match limit {
    Some(limit) => (true, limit)
    None => (false, 0UL)
  }
  let (has_start_after, start_after) = normalize_optional_text(start_after)
  let entries = await_outcome(
    signal => {
      self.native.js.list(
        path, recursive, has_limit, limit, has_start_after, start_after, signal,
      )
    },
    outcome => outcome.entries_value(),
    context,
  )
  entries.map(entry_from_js)
}
