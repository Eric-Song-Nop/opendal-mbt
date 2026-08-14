///|
/// An asynchronous view of an Operator.
///
/// The view shares the configured operator but exposes only operations backed
/// by OpenDAL's asynchronous API. Creating a view performs no I/O.
pub struct AsyncOperator {
  priv native : Operator
}

///|
/// An output-bounded asynchronous cursor over one object or byte range.
///
/// A stream admits one operation at a time. Cancelling an in-flight `next`
/// makes the stream terminal, because the cursor position may have advanced.
pub struct AsyncReadStream {
  priv native : NativeAsyncReadStream
  priv path : String
}

///|
/// An asynchronous chunked writer.
///
/// A writer admits one operation at a time. Cancelling an in-flight `write`,
/// `finish`, or `abort` makes it terminal because remote commit status may be
/// unknown.
pub struct AsyncWriter {
  priv native : NativeAsyncWriter
  priv path : String
}

///|
/// Create a lightweight asynchronous view that shares this Operator.
pub fn Operator::as_async(self : Operator) -> AsyncOperator {
  { native: self }
}

///|
/// Wait for one native operation through a public async pipe.
///
/// The start callback must copy all MoonBit inputs and duplicate the write fd
/// before returning. The Tokio worker writes one byte after publishing its
/// owned result; it never calls MoonBit and never retains a MoonBit reference.
async fn await_native_result(
  start : (Int) -> NativeResult,
  operation : Operation,
  path : String?,
  destination_path? : String,
) -> NativeResult {
  let (wake_read, wake_write) = @pipe.pipe() catch {
    _ =>
      raise binding_error(
        Unexpected,
        operation,
        "could not create async completion pipe",
        path,
        destination_path,
      )
  }
  defer wake_read.close()
  let started = start(wake_write.fd())
  wake_write.close()
  if native_result_status(started) != 0U {
    raise error_from_native_result(started, operation, path, destination_path)
  }
  let task = native_result_take_async_task(started)
  if native_result_status(started) != 0U {
    raise error_from_native_result(started, operation, path, destination_path)
  }
  native_result_release(started)
  // This defer is the cancellation bridge. After a successful take it is a
  // harmless no-op; on task cancellation it requests native cancellation.
  defer native_async_task_cancel(task)
  let wake = wake_read.read_some(max_len=1) catch {
    error if @async.is_being_cancelled() => raise error
    _ =>
      raise binding_error(
        AbiMismatch,
        operation,
        "native async completion channel failed",
        path,
        destination_path,
      )
  }
  match wake {
    Some(bytes) if bytes.length() == 1 => native_async_task_take_result(task)
    _ =>
      raise binding_error(
        AbiMismatch,
        operation,
        "native async task ended without a completion signal",
        path,
        destination_path,
      )
  }
}

///|
/// Verify that the configured backend can perform its native check operation
/// without blocking the MoonBit scheduler.
pub async fn AsyncOperator::check(self : AsyncOperator) -> Unit {
  let result = await_native_result(
    completion_fd => {
      native_async_operator_check_start(self.native, completion_fd)
    },
    Check,
    None,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Check, None, None)
  }
  native_result_release(result)
}

///|
/// Check whether a path exists. Only NotFound becomes false; all other
/// failures remain errors.
pub async fn AsyncOperator::exists(
  self : AsyncOperator,
  path : StringView,
) -> Bool {
  let path = path.to_owned()
  let result = await_native_result(
    completion_fd => {
      native_async_operator_exists_start(self.native, path, completion_fd)
    },
    Exists,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Exists, Some(path), None)
  }
  let exists = native_result_take_bool(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Exists, Some(path), None)
  }
  native_result_release(result)
  exists
}

///|
/// Fetch metadata for one path without blocking the MoonBit scheduler.
pub async fn AsyncOperator::stat(
  self : AsyncOperator,
  path : StringView,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Metadata {
  let path = path.to_owned()
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let result = await_native_result(
    completion_fd => {
      native_async_operator_stat_start(
        self.native,
        path,
        has_version,
        version,
        has_if_match,
        if_match,
        has_if_none_match,
        if_none_match,
        completion_fd,
      )
    },
    Stat,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Stat, Some(path), None)
  }
  let metadata = native_result_take_metadata(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Stat, Some(path), None)
  }
  native_result_release(result)
  metadata_from_native(metadata, Stat, Some(path))
}

///|
/// Write an entire byte view through OpenDAL's asynchronous whole-object API.
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
  let data = data.to_owned()
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
  let result = await_native_result(
    completion_fd => {
      native_async_operator_write_start(
        self.native,
        path,
        data,
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
        completion_fd,
      )
    },
    Write,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(path), None)
  }
  let metadata = native_result_take_metadata(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(path), None)
  }
  native_result_release(result)
  metadata_from_native(metadata, Write, Some(path))
}

///|
/// Create a directory recursively. A directory path must end with /.
pub async fn AsyncOperator::create_dir(
  self : AsyncOperator,
  path : StringView,
) -> Unit {
  let path = path.to_owned()
  let result = await_native_result(
    completion_fd => {
      native_async_operator_create_dir_start(self.native, path, completion_fd)
    },
    CreateDir,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, CreateDir, Some(path), None)
  }
  native_result_release(result)
}

///|
/// Delete a path asynchronously. Deleting a missing path is successful.
pub async fn AsyncOperator::delete(
  self : AsyncOperator,
  path : StringView,
  version? : StringView,
  recursive? : Bool = false,
) -> Unit {
  let path = path.to_owned()
  let (has_version, version) = normalize_optional_text(version)
  let result = await_native_result(
    completion_fd => {
      native_async_operator_delete_start(
        self.native,
        path,
        has_version,
        version,
        recursive,
        completion_fd,
      )
    },
    Delete,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Delete, Some(path), None)
  }
  native_result_release(result)
}

///|
/// Materialize one bounded asynchronous prefix listing. The optional limit is
/// a backend request hint rather than a total-result bound.
pub async fn AsyncOperator::list(
  self : AsyncOperator,
  path : StringView,
  recursive? : Bool = false,
  limit? : UInt64,
  start_after? : StringView,
) -> Array[Entry] {
  let path = path.to_owned()
  let has_limit = limit is Some(_)
  let limit = limit.unwrap_or(0UL)
  let (has_start_after, start_after) = normalize_optional_text(start_after)
  let result = await_native_result(
    completion_fd => {
      native_async_operator_list_start(
        self.native,
        path,
        recursive,
        has_limit,
        limit,
        has_start_after,
        start_after,
        completion_fd,
      )
    },
    List,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, List, Some(path), None)
  }
  let native = native_result_take_lister(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, List, Some(path), None)
  }
  native_result_release(result)
  let lister : Lister = { native, path }
  let entries : Array[Entry] = []
  try {
    for ;; {
      match lister.next() {
        Some(entry) => entries.push(entry)
        None => break
      }
    }
    lister.close()
    entries
  } catch {
    error => {
      lister.close()
      raise error
    }
  } noraise {
    value => value
  }
}

///|
/// Copy one object with the backend's native asynchronous copy operation and
/// return the destination metadata. This is never emulated with read/write.
pub async fn AsyncOperator::copy(
  self : AsyncOperator,
  source : StringView,
  destination : StringView,
) -> Metadata {
  let source = source.to_owned()
  let destination = destination.to_owned()
  let result = await_native_result(
    completion_fd => {
      native_async_operator_copy_start(
        self.native,
        source,
        destination,
        completion_fd,
      )
    },
    Copy,
    Some(source),
    destination_path=destination,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Copy,
      Some(source),
      Some(destination),
    )
  }
  let metadata = native_result_take_metadata(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Copy,
      Some(source),
      Some(destination),
    )
  }
  native_result_release(result)
  metadata_from_native(
    metadata,
    Copy,
    Some(source),
    destination_path=destination,
  )
}

///|
/// Rename one object with the backend's native asynchronous rename operation.
/// This is never emulated with copy/delete.
pub async fn AsyncOperator::rename(
  self : AsyncOperator,
  source : StringView,
  destination : StringView,
) -> Unit {
  let source = source.to_owned()
  let destination = destination.to_owned()
  let result = await_native_result(
    completion_fd => {
      native_async_operator_rename_start(
        self.native,
        source,
        destination,
        completion_fd,
      )
    },
    Rename,
    Some(source),
    destination_path=destination,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(
      result,
      Rename,
      Some(source),
      Some(destination),
    )
  }
  native_result_release(result)
}

///|
/// Read one path or byte range into an owned Bytes value without blocking the
/// MoonBit scheduler. Cancellation requests native cancellation; storage side
/// effects already performed by the backend are not rolled back. Suffix ranges
/// require the underlying Operator's native suffix capability.
pub async fn AsyncOperator::read(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Bytes {
  let path = path.to_owned()
  let (range_kind, range_offset, range_length) = range.native_parts(path)
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let result = await_native_result(
    completion_fd => {
      native_async_operator_read_start(
        self.native,
        path,
        range_kind,
        range_offset,
        range_length,
        has_version,
        version,
        has_if_match,
        if_match,
        has_if_none_match,
        if_none_match,
        completion_fd,
      )
    },
    Read,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  let bytes = native_result_take_bytes(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  native_result_release(result)
  bytes
}

///|
/// Open an output-bounded asynchronous sequential read stream.
/// `chunk_size` bounds each returned value and binding copy, not a raw backend
/// buffer. Suffix ranges require the underlying Operator's native capability.
pub async fn AsyncOperator::open_read_stream(
  self : AsyncOperator,
  path : StringView,
  range? : ByteRange = Full,
  chunk_size? : Int = 1024 * 1024,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> AsyncReadStream {
  let path = path.to_owned()
  if chunk_size <= 0 {
    raise binding_error(
      InvalidArgument,
      Read,
      "async read stream chunk_size must be positive",
      Some(path),
      None,
    )
  }
  let (range_kind, range_offset, range_length) = range.native_parts(path)
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let result = await_native_result(
    completion_fd => {
      native_async_operator_read_stream_start(
        self.native,
        path,
        range_kind,
        range_offset,
        range_length,
        chunk_size,
        has_version,
        version,
        has_if_match,
        if_match,
        has_if_none_match,
        if_none_match,
        completion_fd,
      )
    },
    Read,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  let native = native_result_take_async_read_stream(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  native_result_release(result)
  { native, path }
}

///|
/// Return the next owned chunk, or `None` after a stable end-of-stream.
pub async fn AsyncReadStream::next(self : AsyncReadStream) -> Bytes? {
  let result = await_native_result(
    completion_fd => {
      native_async_read_stream_next_start(self.native, completion_fd)
    },
    Read,
    Some(self.path),
  )
  match native_result_status(result) {
    0U => {
      let bytes = native_result_take_bytes(result)
      if native_result_status(result) != 0U {
        native_async_read_stream_close(self.native)
        raise error_from_native_result(result, Read, Some(self.path), None)
      }
      native_result_release(result)
      Some(bytes)
    }
    1U => {
      native_result_release(result)
      None
    }
    _ => {
      native_async_read_stream_close(self.native)
      raise error_from_native_result(result, Read, Some(self.path), None)
    }
  }
}

///|
/// Release the cursor early. Closing more than once is harmless.
pub fn AsyncReadStream::close(self : AsyncReadStream) -> Unit {
  native_async_read_stream_close(self.native)
}

///|
/// Open an asynchronous chunked writer. Finalization never commits an open
/// writer implicitly.
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
  let result = await_native_result(
    completion_fd => {
      native_async_operator_writer_start(
        self.native,
        path,
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
        completion_fd,
      )
    },
    Write,
    Some(path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(path), None)
  }
  let native = native_result_take_async_writer(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(path), None)
  }
  native_result_release(result)
  { native, path }
}

///|
/// Write one complete chunk. The selected byte view is copied before native
/// work begins.
pub async fn AsyncWriter::write(self : AsyncWriter, chunk : BytesView) -> Unit {
  let chunk = chunk.to_owned()
  let result = await_native_result(
    completion_fd => {
      native_async_writer_write_start(self.native, chunk, completion_fd)
    },
    Write,
    Some(self.path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(self.path), None)
  }
  native_result_release(result)
}

///|
/// Commit all chunks and return the resulting metadata. The first finish
/// attempt is terminal whether it succeeds, raises, or is cancelled.
pub async fn AsyncWriter::finish(self : AsyncWriter) -> Metadata {
  let result = await_native_result(
    completion_fd => {
      native_async_writer_finish_start(self.native, completion_fd)
    },
    Write,
    Some(self.path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(self.path), None)
  }
  let metadata = native_result_take_metadata(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(self.path), None)
  }
  native_result_release(result)
  metadata_from_native(metadata, Write, Some(self.path))
}

///|
/// Discard an unfinished write. A successful abort is idempotent.
pub async fn AsyncWriter::abort(self : AsyncWriter) -> Unit {
  let result = await_native_result(
    completion_fd => native_async_writer_abort_start(self.native, completion_fd),
    Write,
    Some(self.path),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(self.path), None)
  }
  native_result_release(result)
}
