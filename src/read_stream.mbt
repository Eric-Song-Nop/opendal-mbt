///|
/// A bounded sequential cursor over one object or byte range.
///
/// Each successful `next` returns an owned chunk no larger than the
/// `chunk_size` selected when the stream was opened. `None` is a stable
/// end-of-stream marker.
pub struct ReadStream {
  priv native : NativeReadStream
  priv path : String
}

///|
/// Open a bounded sequential read stream.
///
/// `chunk_size` is fixed for the lifetime of the stream and defaults to one
/// MiB. It must be positive and fit the native output ceiling. The returned
/// stream owns its cursor and may outlive this Operator.
pub fn Operator::open_read_stream(
  self : Operator,
  path : StringView,
  range? : ByteRange = Full,
  chunk_size? : Int = 1024 * 1024,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> ReadStream raise OpenDalError {
  let path = path.to_owned()
  if chunk_size <= 0 {
    raise binding_error(
      InvalidArgument,
      Read,
      "read stream chunk_size must be positive",
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
  let result = native_operator_read_stream(
    self, path, range_kind, range_offset, range_length, chunk_size, has_version,
    version, has_if_match, if_match, has_if_none_match, if_none_match,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  let native = native_result_take_read_stream(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  native_result_release(result)
  { native, path }
}

///|
/// Return the next owned chunk, `None` at end-of-stream, or raise on failure.
///
/// End-of-stream is stable. A stream failure is terminal, and later calls
/// raise `ResourceClosed`.
pub fn ReadStream::next(self : ReadStream) -> Bytes? raise OpenDalError {
  let result = native_read_stream_next(self.native)
  match native_result_status(result) {
    0U => {
      let bytes = native_result_take_bytes(result)
      if native_result_status(result) != 0U {
        self.close()
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
      self.close()
      raise error_from_native_result(result, Read, Some(self.path), None)
    }
  }
}

///|
/// Release the cursor early. Closing more than once is harmless.
pub fn ReadStream::close(self : ReadStream) -> Unit {
  native_read_stream_close(self.native)
}
