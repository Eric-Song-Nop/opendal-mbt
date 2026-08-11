///|
let max_presigned_header_count : UInt64 = 2147483647UL

///|
fn validate_presign_expiry(
  expires_in_seconds : UInt64,
  operation : Operation,
  path : String,
) -> Unit raise OpenDalError {
  if expires_in_seconds == 0UL {
    raise binding_error(
      InvalidArgument,
      operation,
      "expires_in_seconds must be greater than zero",
      Some(path),
      None,
    )
  }
}

///|
fn presigned_request_from_parts(
  method_bytes : Bytes,
  uri_bytes : Bytes,
  header_names : Array[Bytes],
  header_values : Array[Bytes],
  operation : Operation,
  path : String,
) -> PresignedRequest raise OpenDalError {
  if header_names.length() != header_values.length() {
    raise binding_error(
      AbiMismatch,
      operation,
      "native ABI returned mismatched presigned header arrays",
      Some(path),
      None,
    )
  }
  let http_method = decode_abi_text(method_bytes, operation, Some(path), None)
  let uri = decode_abi_text(uri_bytes, operation, Some(path), None)
  let headers : Array[PresignedHeader] = []
  for index, name_bytes in header_names {
    headers.push({
      name: decode_abi_text(name_bytes, operation, Some(path), None),
      value: header_values[index],
    })
  }
  { http_method, uri, headers }
}

///|
fn presigned_request_from_native(
  request : NativePresignedRequest,
  operation : Operation,
  path : String,
) -> PresignedRequest raise OpenDalError {
  let method_bytes = native_presigned_request_method(request)
  let uri = native_presigned_request_uri(request)
  let count = native_presigned_request_header_count(request)
  if count > max_presigned_header_count {
    native_presigned_request_release(request)
    raise binding_error(
      AbiMismatch,
      operation,
      "native presigned header count exceeds the MoonBit collection limit",
      Some(path),
      None,
    )
  }
  let header_names : Array[Bytes] = []
  let header_values : Array[Bytes] = []
  for index in 0..<count.to_int() {
    header_names.push(native_presigned_request_header_name(request, index))
    header_values.push(native_presigned_request_header_value(request, index))
  }
  native_presigned_request_release(request)
  presigned_request_from_parts(
    method_bytes, uri, header_names, header_values, operation, path,
  )
}

///|
fn take_presigned_request(
  result : NativeResult,
  operation : Operation,
  path : String,
) -> PresignedRequest raise OpenDalError {
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, operation, Some(path), None)
  }
  let request = native_result_take_presigned_request(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, operation, Some(path), None)
  }
  native_result_release(result)
  presigned_request_from_native(request, operation, path)
}

///|
/// Create a presigned HTTP read request.
///
/// The returned URI and headers can contain bearer-equivalent signing
/// material. Avoid logging or snapshotting them. Header order, duplicate
/// names, and binary values are preserved.
pub fn Operator::presign_read(
  self : Operator,
  path : StringView,
  expires_in_seconds~ : UInt64,
  range? : ByteRange = Full,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> PresignedRequest raise OpenDalError {
  let path = path.to_owned()
  validate_presign_expiry(expires_in_seconds, Read, path)
  let (range_kind, range_offset, range_length) = range.native_parts(path)
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  native_operator_presign_read(
    self, path, expires_in_seconds, range_kind, range_offset, range_length, has_version,
    version, has_if_match, if_match, has_if_none_match, if_none_match,
  )
  |> take_presigned_request(Read, path)
}

///|
/// Create a presigned HTTP write request.
///
/// The returned URI and headers can contain bearer-equivalent signing
/// material. Avoid logging or snapshotting them. The caller supplies the
/// request body when executing the snapshot.
pub fn Operator::presign_write(
  self : Operator,
  path : StringView,
  expires_in_seconds~ : UInt64,
  append? : Bool = false,
  content_type? : StringView,
  content_disposition? : StringView,
  content_encoding? : StringView,
  cache_control? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> PresignedRequest raise OpenDalError {
  let path = path.to_owned()
  validate_presign_expiry(expires_in_seconds, Write, path)
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
  native_operator_presign_write(
    self, path, expires_in_seconds, append, has_content_type, content_type, has_content_disposition,
    content_disposition, has_content_encoding, content_encoding, has_cache_control,
    cache_control, has_if_match, if_match, has_if_none_match, if_none_match,
  )
  |> take_presigned_request(Write, path)
}

///|
/// Create a presigned HTTP metadata request.
///
/// The returned URI and headers can contain bearer-equivalent signing
/// material. Avoid logging or snapshotting them.
pub fn Operator::presign_stat(
  self : Operator,
  path : StringView,
  expires_in_seconds~ : UInt64,
  version? : StringView,
  if_match? : StringView,
  if_none_match? : StringView,
) -> PresignedRequest raise OpenDalError {
  let path = path.to_owned()
  validate_presign_expiry(expires_in_seconds, Stat, path)
  let (has_version, version) = normalize_optional_text(version)
  let (has_if_match, if_match) = normalize_optional_text(if_match)
  let (has_if_none_match, if_none_match) = normalize_optional_text(
    if_none_match,
  )
  native_operator_presign_stat(
    self, path, expires_in_seconds, has_version, version, has_if_match, if_match,
    has_if_none_match, if_none_match,
  )
  |> take_presigned_request(Stat, path)
}
