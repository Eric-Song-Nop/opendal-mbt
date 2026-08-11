///|
/// Open a chunked writer. The returned resource reports completion only after
/// `finish` succeeds; finalization never finishes an open writer implicitly.
pub fn Operator::open_writer(
  self : Operator,
  path : StringView,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Writer raise OpenDalError {
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
  let result = native_operator_writer(
    self, path, append, has_content_type, content_type, has_content_disposition,
    content_disposition, has_content_encoding, content_encoding, has_cache_control,
    cache_control, has_if_match, if_match, has_if_none_match, if_none_match,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(path), None)
  }
  let native = native_result_take_writer(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(path), None)
  }
  native_result_release(result)
  { native, path }
}

///|
/// Write one complete chunk. Any native write failure is terminal for this
/// Writer; later calls raise `ResourceClosed`.
pub fn Writer::write(
  self : Writer,
  chunk : BytesView,
) -> Unit raise OpenDalError {
  let result = native_writer_write(self.native, chunk.to_owned())
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Write, Some(self.path), None)
  }
  native_result_release(result)
}

///|
/// Commit all chunks and return the resulting metadata. The first finish
/// attempt is terminal whether it succeeds or raises.
pub fn Writer::finish(self : Writer) -> Metadata raise OpenDalError {
  let result = native_writer_close(self.native)
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
