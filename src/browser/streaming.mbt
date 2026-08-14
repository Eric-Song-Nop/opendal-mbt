///|
/// An output-bounded asynchronous cursor over one object or byte range.
///
/// A stream admits one operation at a time. End-of-stream and cancellation
/// are terminal, and `close` is idempotent.
pub struct AsyncReadStream {
  priv js : JsReadStream
  priv operator : Operator
  priv path : String
  priv mut closed : Bool
  priv mut ended : Bool
}

///|
priv enum WriterLifecycle {
  WriterOpen
  WriterAborted
  WriterTerminal
}

///|
/// An asynchronous chunked writer backed by OpenDAL's streaming writer.
///
/// Call `finish` to commit or `abort` to discard. Either operation, any failed
/// state transition, and cancellation make the writer terminal.
pub struct AsyncWriter {
  priv js : JsWriter
  priv operator : Operator
  priv path : String
  priv mut lifecycle : WriterLifecycle
}

///|
/// An asynchronous, constant-binding-memory cursor over a prefix listing.
///
/// Entries are decoded one at a time. End-of-list and cancellation are
/// terminal, and `close` is idempotent.
pub struct AsyncLister {
  priv js : JsLister
  priv operator : Operator
  priv path : String
  priv mut closed : Bool
  priv mut ended : Bool
}

///|
fn terminal_resource_error(
  outcome : JsOutcome,
  context : ErrorContext,
) -> OpenDalError {
  bridge_error(outcome, context)
}

///|
fn AsyncReadStream::ensure_open(
  self : AsyncReadStream,
) -> Unit raise OpenDalError {
  let context = error_context(Read, path=self.path)
  if self.closed || self.operator.closed || self.operator.runtime.closed {
    raise local_error(ResourceClosed, "browser read stream is closed", context)
  }
}

///|
fn AsyncWriter::ensure_open(self : AsyncWriter) -> Unit raise OpenDalError {
  let context = error_context(Write, path=self.path)
  if !(self.lifecycle is WriterOpen) ||
    self.operator.closed ||
    self.operator.runtime.closed {
    raise local_error(ResourceClosed, "browser writer is closed", context)
  }
}

///|
fn AsyncLister::ensure_open(self : AsyncLister) -> Unit raise OpenDalError {
  let context = error_context(List, path=self.path)
  if self.closed || self.operator.closed || self.operator.runtime.closed {
    raise local_error(ResourceClosed, "browser lister is closed", context)
  }
}

///|
/// Open an output-bounded sequential read stream.
///
/// `chunk_size` bounds each returned MoonBit value and bridge copy. It must be
/// positive and no larger than the bridge's 256 KiB transfer limit.
pub async fn AsyncOperator::open_read_stream(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  chunk_size? : Int = 256 * 1024,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> AsyncReadStream {
  let path = path.to_owned()
  let context = error_context(Read, path~)
  self.ensure_open(context)
  if chunk_size <= 0 || chunk_size > 256 * 1024 {
    raise local_error(
      InvalidArgument,
      "browser read stream chunk_size must be between 1 and 256 KiB",
      context,
    )
  }
  let (range_kind, range_offset, range_length) = range.js_parts(
    self.native.info_snapshot.capability,
    context,
  )
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let js = await_outcome(
    signal => {
      self.native.js.open_read_stream(
        path, range_kind, range_offset, range_length, chunk_size, has_version, version,
        has_if_match, if_match, has_if_none_match, if_none_match, signal,
      )
    },
    outcome => outcome.read_stream_value(),
    context,
  )
  { js, operator: self.native, path, closed: false, ended: false }
}

///|
/// Return the next owned chunk, or `None` after a stable end-of-stream.
pub async fn AsyncReadStream::next(self : AsyncReadStream) -> Bytes? {
  if self.ended {
    return None
  }
  self.ensure_open()
  let context = error_context(Read, path=self.path)
  let outcome = @js_async.run_promise(signal => self.js.next(signal)) catch {
    error if @async.is_being_cancelled() => {
      self.closed = true
      ignore(self.js.close())
      raise error
    }
    _ => {
      self.closed = true
      ignore(self.js.close())
      raise local_error(
        AbiMismatch,
        "OpenDAL browser stream Promise rejected instead of returning an outcome",
        context,
      )
    }
  }
  if !outcome.is_ok() {
    self.closed = true
    ignore(self.js.close())
    raise terminal_resource_error(outcome, context)
  }
  if outcome.has_value() {
    Some(outcome.bytes_value())
  } else {
    self.closed = true
    self.ended = true
    ignore(self.js.close())
    None
  }
}

///|
/// Release the cursor early. Closing more than once is harmless.
pub fn AsyncReadStream::close(
  self : AsyncReadStream,
) -> Unit raise OpenDalError {
  if !self.closed {
    self.closed = true
    outcome_unit(self.js.close(), error_context(Read, path=self.path))
  }
}

///|
/// Open an asynchronous chunked writer. Dropping or closing an unfinished
/// writer never commits it implicitly.
pub async fn AsyncOperator::open_writer(
  self : AsyncOperator,
  path : StringView,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> AsyncWriter {
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
  let js = await_outcome(
    signal => {
      self.native.js.open_writer(
        path, append, has_content_type, content_type, has_content_disposition, content_disposition,
        has_content_encoding, content_encoding, has_cache_control, cache_control,
        has_if_match, if_match, has_if_none_match, if_none_match, signal,
      )
    },
    outcome => outcome.writer_value(),
    context,
  )
  { js, operator: self.native, path, lifecycle: WriterOpen }
}

///|
/// Write one complete chunk of at most 256 KiB. The selected byte view is
/// copied before the asynchronous bridge call starts.
pub async fn AsyncWriter::write(self : AsyncWriter, chunk : BytesView) -> Unit {
  self.ensure_open()
  let context = error_context(Write, path=self.path)
  if chunk.length() > 256 * 1024 {
    raise local_error(
      BufferTooLarge,
      "browser writer chunk must not exceed 256 KiB",
      context,
    )
  }
  let outcome = @js_async.run_promise(signal => {
    self.js.write(chunk.to_owned(), signal)
  }) catch {
    error if @async.is_being_cancelled() => {
      self.lifecycle = WriterTerminal
      ignore(self.js.close())
      raise error
    }
    _ => {
      self.lifecycle = WriterTerminal
      ignore(self.js.close())
      raise local_error(
        AbiMismatch,
        "OpenDAL browser writer Promise rejected instead of returning an outcome",
        context,
      )
    }
  }
  if !outcome.is_ok() {
    self.lifecycle = WriterTerminal
    ignore(self.js.close())
    raise terminal_resource_error(outcome, context)
  }
}

///|
/// Commit all chunks and return destination metadata. The first finish
/// attempt is terminal whether it succeeds, raises, or is cancelled.
pub async fn AsyncWriter::finish(self : AsyncWriter) -> Metadata {
  self.ensure_open()
  self.lifecycle = WriterTerminal
  let context = error_context(Write, path=self.path)
  let outcome = @js_async.run_promise(signal => self.js.finish(signal)) catch {
    error if @async.is_being_cancelled() => {
      ignore(self.js.close())
      raise error
    }
    _ => {
      ignore(self.js.close())
      raise local_error(
        AbiMismatch,
        "OpenDAL browser writer Promise rejected instead of returning an outcome",
        context,
      )
    }
  }
  let metadata = outcome_value(
    outcome,
    value => value.metadata_value(),
    context,
  )
  metadata_from_js(metadata)
}

///|
/// Discard an unfinished write. A completed abort is idempotent.
pub async fn AsyncWriter::abort(self : AsyncWriter) -> Unit {
  if self.lifecycle is WriterAborted {
    return
  }
  self.ensure_open()
  self.lifecycle = WriterTerminal
  let context = error_context(Write, path=self.path)
  let outcome = @js_async.run_promise(signal => self.js.abort(signal)) catch {
    error if @async.is_being_cancelled() => {
      ignore(self.js.close())
      raise error
    }
    _ => {
      ignore(self.js.close())
      raise local_error(
        AbiMismatch,
        "OpenDAL browser writer Promise rejected instead of returning an outcome",
        context,
      )
    }
  }
  outcome_unit(outcome, context)
  self.lifecycle = WriterAborted
}

///|
/// Release an unfinished writer without committing it. Prefer `abort` when
/// cleanup completion must be observed.
pub fn AsyncWriter::close(self : AsyncWriter) -> Unit raise OpenDalError {
  if self.lifecycle is WriterOpen {
    self.lifecycle = WriterTerminal
    outcome_unit(self.js.close(), error_context(Write, path=self.path))
  }
}

///|
/// Open a streaming prefix lister whose entries are decoded one at a time.
/// `limit` remains a backend request hint rather than a total-result bound.
pub async fn AsyncOperator::open_lister(
  self : AsyncOperator,
  path : StringView,
  recursive? : Bool = false,
  limit? : UInt64,
  start_after? : StringView,
) -> AsyncLister {
  let path = path.to_owned()
  let context = error_context(List, path~)
  self.ensure_open(context)
  let (has_limit, limit) = match limit {
    Some(limit) => (true, limit)
    None => (false, 0UL)
  }
  let (has_start_after, start_after) = normalize_optional_text(start_after)
  let js = await_outcome(
    signal => {
      self.native.js.open_lister(
        path, recursive, has_limit, limit, has_start_after, start_after, signal,
      )
    },
    outcome => outcome.lister_value(),
    context,
  )
  { js, operator: self.native, path, closed: false, ended: false }
}

///|
/// Return the next owned entry, or `None` after a stable end-of-list.
pub async fn AsyncLister::next(self : AsyncLister) -> Entry? {
  if self.ended {
    return None
  }
  self.ensure_open()
  let context = error_context(List, path=self.path)
  let outcome = @js_async.run_promise(signal => self.js.next(signal)) catch {
    error if @async.is_being_cancelled() => {
      self.closed = true
      ignore(self.js.close())
      raise error
    }
    _ => {
      self.closed = true
      ignore(self.js.close())
      raise local_error(
        AbiMismatch,
        "OpenDAL browser lister Promise rejected instead of returning an outcome",
        context,
      )
    }
  }
  if !outcome.is_ok() {
    self.closed = true
    ignore(self.js.close())
    raise terminal_resource_error(outcome, context)
  }
  if outcome.has_value() {
    Some(entry_from_js(outcome.entry_value()))
  } else {
    self.closed = true
    self.ended = true
    ignore(self.js.close())
    None
  }
}

///|
/// Release the listing cursor early. Closing more than once is harmless.
pub fn AsyncLister::close(self : AsyncLister) -> Unit raise OpenDalError {
  if !self.closed {
    self.closed = true
    outcome_unit(self.js.close(), error_context(List, path=self.path))
  }
}
