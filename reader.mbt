///|
fn ByteRange::native_parts(
  self : ByteRange,
  path : String,
) -> (UInt, UInt64, UInt64) raise OpenDalError {
  match self {
    Full => (0U, 0UL, 0UL)
    From(offset~) => (1U, offset, 0UL)
    Range(offset~, length~) => {
      if length > 18446744073709551615UL - offset {
        raise binding_error(
          InvalidArgument,
          Read,
          "byte range offset plus length exceeds UInt64",
          Some(path),
          None,
        )
      }
      (2U, offset, length)
    }
    Suffix(length~) => (3U, 0UL, length)
  }
}

///|
/// Open a random-access reader. Reads on the returned value are independent;
/// it does not maintain an implicit cursor.
pub fn Operator::open_reader(
  self : Operator,
  path : StringView,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Reader raise OpenDalError {
  let path = path.to_owned()
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let result = native_operator_reader(
    self, path, has_version, version, has_if_match, if_match, has_if_none_match,
    if_none_match,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  let native = native_result_take_reader(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(path), None)
  }
  native_result_release(result)
  { native, path }
}

///|
/// Read one independent range into an owned `Bytes` value.
/// Ordinary read errors leave the Reader open for another range.
pub fn Reader::read(
  self : Reader,
  range : ByteRange,
) -> Bytes raise OpenDalError {
  let (range_kind, range_offset, range_length) = range.native_parts(self.path)
  let result = native_reader_read(
    self.native,
    range_kind,
    range_offset,
    range_length,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(self.path), None)
  }
  let bytes = native_result_take_bytes(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Read, Some(self.path), None)
  }
  native_result_release(result)
  bytes
}

///|
/// Release reader resources early. Closing more than once is harmless.
pub fn Reader::close(self : Reader) -> Unit {
  native_reader_close(self.native)
}
