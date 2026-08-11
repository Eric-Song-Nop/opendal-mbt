///|
fn normalize_optional_text(value : StringView?) -> (Bool, String) {
  match value {
    Some(value) => (true, value.to_owned())
    None => (false, "")
  }
}

///|
/// Construct an operator from a service scheme and configuration map.
/// The map is copied and is never mutated or retained by reference.
pub fn Operator::new(
  scheme : StringView,
  config? : Map[String, String] = Map([]),
) -> Operator raise OpenDalError {
  let keys : Array[String] = []
  let values : Array[String] = []
  for key, value in config {
    keys.push(key)
    values.push(value)
  }
  let result = native_operator_new(
    scheme.to_owned(),
    FixedArray::from_array(keys),
    FixedArray::from_array(values),
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, NewOperator, None, None)
  }
  let operator = native_result_take_operator(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, NewOperator, None, None)
  }
  native_result_release(result)
  operator
}

///|
/// Verify that the operator can perform OpenDAL's check operation.
pub fn Operator::check(self : Operator) -> Unit raise OpenDalError {
  let result = native_operator_check(self)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Check, None, None)
  }
  native_result_release(result)
}

///|
/// Return a read-only information and capability snapshot.
pub fn Operator::info(self : Operator) -> OperatorInfo {
  let scheme_bytes = native_operator_info_scheme(self)
  let root_bytes = native_operator_info_root(self)
  let name_bytes = native_operator_info_name(self)
  let capability = native_operator_info_capability(self)
  let scheme = try! @utf8.decode(scheme_bytes)
  let root = try! @utf8.decode(root_bytes)
  let name = try! @utf8.decode(name_bytes)
  { scheme, root, name, capability }
}

///|
/// Check whether a path exists. Only NotFound becomes false; all other
/// failures remain errors.
pub fn Operator::exists(
  self : Operator,
  path : StringView,
) -> Bool raise OpenDalError {
  let path = path.to_owned()
  let result = native_operator_exists(self, path)
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
/// Fetch metadata for a path. Optional conditions are copied at the native
/// call boundary.
pub fn Operator::stat(
  self : Operator,
  path : StringView,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Metadata raise OpenDalError {
  let path = path.to_owned()
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let result = native_operator_stat(
    self, path, has_version, version, has_if_match, if_match, has_if_none_match,
    if_none_match,
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
/// Read a path or byte range into one owned Bytes value.
pub fn Operator::read(
  self : Operator,
  path : StringView,
  range? : ByteRange = Full,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Bytes raise OpenDalError {
  let path = path.to_owned()
  let (range_kind, range_offset, range_length) = range.native_parts(path)
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  let result = native_operator_read(
    self, path, range_kind, range_offset, range_length, has_version, version, has_if_match,
    if_match, has_if_none_match, if_none_match,
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
/// Write an entire byte view and return the resulting metadata. Only the
/// selected view is copied across the native boundary.
pub fn Operator::write(
  self : Operator,
  path : StringView,
  data : BytesView,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> Metadata raise OpenDalError {
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
  let result = native_operator_write(
    self,
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
pub fn Operator::create_dir(
  self : Operator,
  path : StringView,
) -> Unit raise OpenDalError {
  let path = path.to_owned()
  let result = native_operator_create_dir(self, path)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, CreateDir, Some(path), None)
  }
  native_result_release(result)
}

///|
/// Delete a path. Deleting a missing path is successful.
pub fn Operator::delete(
  self : Operator,
  path : StringView,
  version? : StringView,
  recursive? : Bool = false,
) -> Unit raise OpenDalError {
  let path = path.to_owned()
  let (has_version, version) = normalize_optional_text(version)
  let result = native_operator_delete(
    self, path, has_version, version, recursive,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Delete, Some(path), None)
  }
  native_result_release(result)
}

///|
/// Copy one object with the backend's native copy operation and return the
/// destination metadata. This is never emulated with read plus write.
pub fn Operator::copy(
  self : Operator,
  source : StringView,
  destination : StringView,
) -> Metadata raise OpenDalError {
  let source = source.to_owned()
  let destination = destination.to_owned()
  let result = native_operator_copy(self, source, destination)
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
/// Rename one object with the backend's native rename operation. This is never
/// emulated with copy plus delete.
pub fn Operator::rename(
  self : Operator,
  source : StringView,
  destination : StringView,
) -> Unit raise OpenDalError {
  let source = source.to_owned()
  let destination = destination.to_owned()
  let result = native_operator_rename(self, source, destination)
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
