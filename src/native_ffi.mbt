///|
/// Native operator storage is a MoonBit external object finalized by the C
/// stub. Its payload contains only project-owned C ABI handles.
pub type Operator

///|
/// Capability is an immutable external snapshot of the ABI capability words.
pub type Capability

///|
/// Native lister storage is a MoonBit external object finalized by the C stub.
priv type NativeLister

///|
/// Native random-access reader storage finalized by the C stub.
priv type NativeReader

///|
/// Native sequential read-stream storage finalized by the C stub.
priv type NativeReadStream

///|
/// Native chunked writer storage finalized by the C stub without finishing.
priv type NativeWriter

///|
/// Temporary owned native entry snapshot.
priv type NativeEntry

///|
/// Safe lister wrapper retaining the prefix used for error context.
pub struct Lister {
  priv native : NativeLister
  priv path : String
}

///|
/// Safe random-access reader retaining the path used for error context.
pub struct Reader {
  priv native : NativeReader
  priv path : String
}

///|
/// Safe chunked writer retaining the path used for error context.
pub struct Writer {
  priv native : NativeWriter
  priv path : String
}

///|
/// Call-scoped native result. The C finalizer releases every untaken Rust
/// handle, snapshot, error, or buffer.
priv type NativeResult

///|
/// Temporary Rust metadata snapshot owned through a MoonBit external object.
priv type NativeMetadata

///|
/// Temporary owned native presigned-request snapshot.
priv type NativePresignedRequest

///|
#borrow(scheme, keys, values)
extern "C" fn native_operator_new(
  scheme : String,
  keys : FixedArray[String],
  values : FixedArray[String],
) -> NativeResult = "moonbit_opendal_operator_new"

///|
#borrow(bucket, region, root, endpoint, access_key_id, secret_access_key, session_token, role_arn, external_id, role_session_name)
extern "C" fn native_operator_s3(
  bucket : String,
  region : String,
  has_root : Bool,
  root : String,
  has_endpoint : Bool,
  endpoint : String,
  auth_kind : UInt,
  source_kind : UInt,
  disable_ec2_metadata : Bool,
  access_key_id : String,
  secret_access_key : String,
  has_session_token : Bool,
  session_token : String,
  role_arn : String,
  has_external_id : Bool,
  external_id : String,
  has_role_session_name : Bool,
  role_session_name : String,
  has_duration_seconds : Bool,
  duration_seconds : UInt,
  virtual_host_style : Bool,
) -> NativeResult = "moonbit_opendal_operator_s3"

///|
#borrow(operator)
extern "C" fn native_operator_check(operator : Operator) -> NativeResult = "moonbit_opendal_operator_check"

///|
#borrow(operator, path)
extern "C" fn native_operator_exists(
  operator : Operator,
  path : String,
) -> NativeResult = "moonbit_opendal_operator_exists"

///|
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_operator_stat(
  operator : Operator,
  path : String,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_stat"

///|
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_operator_presign_stat(
  operator : Operator,
  path : String,
  expires_in_seconds : UInt64,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_presign_stat"

///|
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_operator_read(
  operator : Operator,
  path : String,
  range_kind : UInt,
  range_offset : UInt64,
  range_length : UInt64,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_read"

///|
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_operator_presign_read(
  operator : Operator,
  path : String,
  expires_in_seconds : UInt64,
  range_kind : UInt,
  range_offset : UInt64,
  range_length : UInt64,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_presign_read"

///|
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_operator_reader(
  operator : Operator,
  path : String,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_reader"

///|
#borrow(reader)
extern "C" fn native_reader_read(
  reader : NativeReader,
  range_kind : UInt,
  range_offset : UInt64,
  range_length : UInt64,
) -> NativeResult = "moonbit_opendal_reader_read"

///|
#borrow(reader)
extern "C" fn native_reader_close(reader : NativeReader) -> Unit = "moonbit_opendal_reader_close"

///|
#borrow(operator, path, version, if_match, if_none_match)
extern "C" fn native_operator_read_stream(
  operator : Operator,
  path : String,
  range_kind : UInt,
  range_offset : UInt64,
  range_length : UInt64,
  chunk_size : Int,
  has_version : Bool,
  version : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_read_stream"

///|
#borrow(stream)
extern "C" fn native_read_stream_next(
  stream : NativeReadStream,
) -> NativeResult = "moonbit_opendal_read_stream_next"

///|
#borrow(stream)
extern "C" fn native_read_stream_close(stream : NativeReadStream) -> Unit = "moonbit_opendal_read_stream_close"

///|
#borrow(operator, path, data, content_type, content_disposition, content_encoding, cache_control, if_match, if_none_match)
extern "C" fn native_operator_write(
  operator : Operator,
  path : String,
  data : Bytes,
  append : Bool,
  has_content_type : Bool,
  content_type : String,
  has_content_disposition : Bool,
  content_disposition : String,
  has_content_encoding : Bool,
  content_encoding : String,
  has_cache_control : Bool,
  cache_control : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_write"

///|
#borrow(operator, path, content_type, content_disposition, content_encoding, cache_control, if_match, if_none_match)
extern "C" fn native_operator_presign_write(
  operator : Operator,
  path : String,
  expires_in_seconds : UInt64,
  append : Bool,
  has_content_type : Bool,
  content_type : String,
  has_content_disposition : Bool,
  content_disposition : String,
  has_content_encoding : Bool,
  content_encoding : String,
  has_cache_control : Bool,
  cache_control : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_presign_write"

///|
#borrow(operator, path, content_type, content_disposition, content_encoding, cache_control, if_match, if_none_match)
extern "C" fn native_operator_writer(
  operator : Operator,
  path : String,
  append : Bool,
  has_content_type : Bool,
  content_type : String,
  has_content_disposition : Bool,
  content_disposition : String,
  has_content_encoding : Bool,
  content_encoding : String,
  has_cache_control : Bool,
  cache_control : String,
  has_if_match : Bool,
  if_match : String,
  has_if_none_match : Bool,
  if_none_match : String,
) -> NativeResult = "moonbit_opendal_operator_writer"

///|
#borrow(writer, data)
extern "C" fn native_writer_write(
  writer : NativeWriter,
  data : Bytes,
) -> NativeResult = "moonbit_opendal_writer_write"

///|
#borrow(writer)
extern "C" fn native_writer_close(writer : NativeWriter) -> NativeResult = "moonbit_opendal_writer_close"

///|
#borrow(writer)
extern "C" fn native_writer_abort(writer : NativeWriter) -> NativeResult = "moonbit_opendal_writer_abort"

///|
#borrow(operator, source, destination)
extern "C" fn native_operator_copy(
  operator : Operator,
  source : String,
  destination : String,
) -> NativeResult = "moonbit_opendal_operator_copy"

///|
#borrow(operator, source, destination)
extern "C" fn native_operator_rename(
  operator : Operator,
  source : String,
  destination : String,
) -> NativeResult = "moonbit_opendal_operator_rename"

///|
#borrow(operator, path)
extern "C" fn native_operator_create_dir(
  operator : Operator,
  path : String,
) -> NativeResult = "moonbit_opendal_operator_create_dir"

///|
#borrow(operator, path, version)
extern "C" fn native_operator_delete(
  operator : Operator,
  path : String,
  has_version : Bool,
  version : String,
  recursive : Bool,
) -> NativeResult = "moonbit_opendal_operator_delete"

///|
#borrow(operator, path, start_after)
extern "C" fn native_operator_lister(
  operator : Operator,
  path : String,
  recursive : Bool,
  has_limit : Bool,
  limit : UInt64,
  has_start_after : Bool,
  start_after : String,
) -> NativeResult = "moonbit_opendal_operator_lister"

///|
#borrow(lister)
extern "C" fn native_lister_next(lister : NativeLister) -> NativeResult = "moonbit_opendal_lister_next"

///|
#borrow(lister)
extern "C" fn native_lister_close(lister : NativeLister) -> Unit = "moonbit_opendal_lister_close"

///|
#borrow(result)
extern "C" fn native_result_status(result : NativeResult) -> UInt = "moonbit_opendal_result_status"

///|
#borrow(result)
extern "C" fn native_result_error_kind(result : NativeResult) -> UInt = "moonbit_opendal_result_error_kind"

///|
#borrow(result)
extern "C" fn native_result_error_status(result : NativeResult) -> UInt = "moonbit_opendal_result_error_status"

///|
#borrow(result)
extern "C" fn native_result_error_kind_name(result : NativeResult) -> Bytes = "moonbit_opendal_result_error_kind_name"

///|
#borrow(result)
extern "C" fn native_result_error_message(result : NativeResult) -> Bytes = "moonbit_opendal_result_error_message"

///|
#borrow(result)
extern "C" fn native_result_take_operator(result : NativeResult) -> Operator = "moonbit_opendal_result_take_operator"

///|
#borrow(result)
extern "C" fn native_result_take_lister(result : NativeResult) -> NativeLister = "moonbit_opendal_result_take_lister"

///|
#borrow(result)
extern "C" fn native_result_take_reader(result : NativeResult) -> NativeReader = "moonbit_opendal_result_take_reader"

///|
#borrow(result)
extern "C" fn native_result_take_read_stream(
  result : NativeResult,
) -> NativeReadStream = "moonbit_opendal_result_take_read_stream"

///|
#borrow(result)
extern "C" fn native_result_take_writer(result : NativeResult) -> NativeWriter = "moonbit_opendal_result_take_writer"

///|
#borrow(result)
extern "C" fn native_result_take_entry(result : NativeResult) -> NativeEntry = "moonbit_opendal_result_take_entry"

///|
#borrow(result)
extern "C" fn native_result_take_bool(result : NativeResult) -> Bool = "moonbit_opendal_result_take_bool"

///|
#borrow(result)
extern "C" fn native_result_take_bytes(result : NativeResult) -> Bytes = "moonbit_opendal_result_take_bytes"

///|
#borrow(result)
extern "C" fn native_result_take_metadata(
  result : NativeResult,
) -> NativeMetadata = "moonbit_opendal_result_take_metadata"

///|
#borrow(result)
extern "C" fn native_result_take_presigned_request(
  result : NativeResult,
) -> NativePresignedRequest = "moonbit_opendal_result_take_presigned_request"

///|
#borrow(result)
extern "C" fn native_result_release(result : NativeResult) -> Unit = "moonbit_opendal_result_release"

///|
#borrow(entry)
extern "C" fn native_entry_path(entry : NativeEntry) -> Bytes = "moonbit_opendal_entry_path"

///|
#borrow(entry)
extern "C" fn native_entry_name(entry : NativeEntry) -> Bytes = "moonbit_opendal_entry_name"

///|
#borrow(entry)
extern "C" fn native_entry_take_metadata(entry : NativeEntry) -> NativeMetadata = "moonbit_opendal_entry_take_metadata"

///|
#borrow(entry)
extern "C" fn native_entry_release(entry : NativeEntry) -> Unit = "moonbit_opendal_entry_release"

///|
#borrow(operator)
extern "C" fn native_operator_info_scheme(operator : Operator) -> Bytes = "moonbit_opendal_operator_info_scheme"

///|
#borrow(operator)
extern "C" fn native_operator_info_root(operator : Operator) -> Bytes = "moonbit_opendal_operator_info_root"

///|
#borrow(operator)
extern "C" fn native_operator_info_name(operator : Operator) -> Bytes = "moonbit_opendal_operator_info_name"

///|
#borrow(operator)
extern "C" fn native_operator_info_capability(
  operator : Operator,
) -> Capability = "moonbit_opendal_operator_info_capability"

///|
#borrow(capability)
extern "C" fn native_capability_word(
  capability : Capability,
  index : Int,
) -> UInt64 = "moonbit_opendal_capability_word"

///|
#borrow(metadata)
extern "C" fn native_metadata_present_bits(metadata : NativeMetadata) -> UInt64 = "moonbit_opendal_metadata_present_bits"

///|
#borrow(metadata)
extern "C" fn native_metadata_mode(metadata : NativeMetadata) -> UInt = "moonbit_opendal_metadata_mode"

///|
#borrow(metadata)
extern "C" fn native_metadata_content_length(
  metadata : NativeMetadata,
) -> UInt64 = "moonbit_opendal_metadata_content_length"

///|
#borrow(metadata)
extern "C" fn native_metadata_is_current(metadata : NativeMetadata) -> Bool = "moonbit_opendal_metadata_is_current"

///|
#borrow(metadata)
extern "C" fn native_metadata_is_deleted(metadata : NativeMetadata) -> Bool = "moonbit_opendal_metadata_is_deleted"

///|
#borrow(metadata)
extern "C" fn native_metadata_last_modified_seconds(
  metadata : NativeMetadata,
) -> Int64 = "moonbit_opendal_metadata_last_modified_seconds"

///|
#borrow(metadata)
extern "C" fn native_metadata_last_modified_nanoseconds(
  metadata : NativeMetadata,
) -> UInt = "moonbit_opendal_metadata_last_modified_nanoseconds"

///|
#borrow(metadata)
extern "C" fn native_metadata_cache_control(metadata : NativeMetadata) -> Bytes = "moonbit_opendal_metadata_cache_control"

///|
#borrow(metadata)
extern "C" fn native_metadata_content_disposition(
  metadata : NativeMetadata,
) -> Bytes = "moonbit_opendal_metadata_content_disposition"

///|
#borrow(metadata)
extern "C" fn native_metadata_content_encoding(
  metadata : NativeMetadata,
) -> Bytes = "moonbit_opendal_metadata_content_encoding"

///|
#borrow(metadata)
extern "C" fn native_metadata_content_md5(metadata : NativeMetadata) -> Bytes = "moonbit_opendal_metadata_content_md5"

///|
#borrow(metadata)
extern "C" fn native_metadata_content_type(metadata : NativeMetadata) -> Bytes = "moonbit_opendal_metadata_content_type"

///|
#borrow(metadata)
extern "C" fn native_metadata_etag(metadata : NativeMetadata) -> Bytes = "moonbit_opendal_metadata_etag"

///|
#borrow(metadata)
extern "C" fn native_metadata_version(metadata : NativeMetadata) -> Bytes = "moonbit_opendal_metadata_version"

///|
#borrow(metadata)
extern "C" fn native_metadata_release(metadata : NativeMetadata) -> Unit = "moonbit_opendal_metadata_release"

///|
#borrow(request)
extern "C" fn native_presigned_request_method(
  request : NativePresignedRequest,
) -> Bytes = "moonbit_opendal_presigned_request_method"

///|
#borrow(request)
extern "C" fn native_presigned_request_uri(
  request : NativePresignedRequest,
) -> Bytes = "moonbit_opendal_presigned_request_uri"

///|
#borrow(request)
extern "C" fn native_presigned_request_header_count(
  request : NativePresignedRequest,
) -> UInt64 = "moonbit_opendal_presigned_request_header_count"

///|
#borrow(request)
extern "C" fn native_presigned_request_header_name(
  request : NativePresignedRequest,
  index : Int,
) -> Bytes = "moonbit_opendal_presigned_request_header_name"

///|
#borrow(request)
extern "C" fn native_presigned_request_header_value(
  request : NativePresignedRequest,
  index : Int,
) -> Bytes = "moonbit_opendal_presigned_request_header_value"

///|
#borrow(request)
extern "C" fn native_presigned_request_release(
  request : NativePresignedRequest,
) -> Unit = "moonbit_opendal_presigned_request_release"

///|
fn abi_text_error(
  operation : Operation,
  path : String?,
  destination_path : String?,
) -> OpenDalError {
  OpenDalError({
    kind: AbiMismatch,
    status: Permanent,
    message: "native ABI returned malformed UTF-8",
    operation,
    path,
    destination_path,
  })
}

///|
fn decode_abi_text(
  bytes : Bytes,
  operation : Operation,
  path : String?,
  destination_path : String?,
) -> String raise OpenDalError {
  try @utf8.decode(bytes) catch {
    _ => raise abi_text_error(operation, path, destination_path)
  } noraise {
    text => text
  }
}

///|
fn binding_error(
  kind : ErrorKind,
  operation : Operation,
  message : String,
  path : String?,
  destination_path : String?,
) -> OpenDalError {
  OpenDalError({
    kind,
    status: Permanent,
    message,
    operation,
    path,
    destination_path,
  })
}

///|
fn native_error_kind(code : UInt, name : String) -> ErrorKind {
  match code {
    1U => Unexpected
    2U => Unsupported
    3U => ConfigInvalid
    4U => NotFound
    5U => PermissionDenied
    6U => IsADirectory
    7U => NotADirectory
    8U => AlreadyExists
    9U => RateLimited
    10U => IsSameFile
    11U => ConditionNotMatch
    12U => RangeNotSatisfied
    0x1001U => InvalidArgument
    0x1002U => ResourceClosed
    0x1003U => BufferTooLarge
    0x1004U => AbiMismatch
    other => UnknownKind(other.reinterpret_as_int(), name)
  }
}

///|
fn native_error_status(code : UInt) -> ErrorStatus {
  match code {
    1U => Permanent
    2U => Temporary
    3U => Persistent
    other => UnknownStatus(other.reinterpret_as_int())
  }
}

///|
fn error_from_native_result(
  result : NativeResult,
  operation : Operation,
  path : String?,
  destination_path : String?,
) -> OpenDalError raise OpenDalError {
  let kind_code = native_result_error_kind(result)
  let status_code = native_result_error_status(result)
  let kind_name_bytes = native_result_error_kind_name(result)
  let message_bytes = native_result_error_message(result)
  native_result_release(result)
  let kind_name = decode_abi_text(
    kind_name_bytes, operation, path, destination_path,
  )
  let message = decode_abi_text(
    message_bytes, operation, path, destination_path,
  )
  OpenDalError({
    kind: native_error_kind(kind_code, kind_name),
    status: native_error_status(status_code),
    message,
    operation,
    path,
    destination_path,
  })
}

///|
fn metadata_bit(bits : UInt64, index : Int) -> Bool {
  (bits & (1UL << index)) != 0UL
}

///|
fn optional_metadata_text(
  present : Bool,
  bytes : Bytes,
  operation : Operation,
  path : String?,
  destination_path : String?,
) -> String? raise OpenDalError {
  if present {
    Some(decode_abi_text(bytes, operation, path, destination_path))
  } else {
    None
  }
}

///|
fn metadata_from_native(
  metadata : NativeMetadata,
  operation : Operation,
  path : String?,
  destination_path? : String,
) -> Metadata raise OpenDalError {
  let bits = native_metadata_present_bits(metadata)
  let mode_code = native_metadata_mode(metadata)
  let content_length = native_metadata_content_length(metadata)
  let is_current_value = native_metadata_is_current(metadata)
  let is_deleted = native_metadata_is_deleted(metadata)
  let modified_seconds = native_metadata_last_modified_seconds(metadata)
  let modified_nanoseconds = native_metadata_last_modified_nanoseconds(metadata)
  let cache_control_bytes = native_metadata_cache_control(metadata)
  let content_disposition_bytes = native_metadata_content_disposition(metadata)
  let content_encoding_bytes = native_metadata_content_encoding(metadata)
  let content_md5_bytes = native_metadata_content_md5(metadata)
  let content_type_bytes = native_metadata_content_type(metadata)
  let etag_bytes = native_metadata_etag(metadata)
  let version_bytes = native_metadata_version(metadata)
  native_metadata_release(metadata)
  let mode = match mode_code {
    1U => File
    2U => Directory
    _ => Unknown
  }
  let is_current = if metadata_bit(bits, 0) {
    Some(is_current_value)
  } else {
    None
  }
  let last_modified = if metadata_bit(bits, 1) {
    Some({
      unix_seconds: modified_seconds,
      nanoseconds: modified_nanoseconds.reinterpret_as_int(),
    })
  } else {
    None
  }
  {
    mode,
    content_length,
    is_current,
    is_deleted,
    cache_control: optional_metadata_text(
      metadata_bit(bits, 2),
      cache_control_bytes,
      operation,
      path,
      destination_path,
    ),
    content_disposition: optional_metadata_text(
      metadata_bit(bits, 3),
      content_disposition_bytes,
      operation,
      path,
      destination_path,
    ),
    content_encoding: optional_metadata_text(
      metadata_bit(bits, 4),
      content_encoding_bytes,
      operation,
      path,
      destination_path,
    ),
    content_md5: optional_metadata_text(
      metadata_bit(bits, 5),
      content_md5_bytes,
      operation,
      path,
      destination_path,
    ),
    content_type: optional_metadata_text(
      metadata_bit(bits, 6),
      content_type_bytes,
      operation,
      path,
      destination_path,
    ),
    etag: optional_metadata_text(
      metadata_bit(bits, 7),
      etag_bytes,
      operation,
      path,
      destination_path,
    ),
    last_modified,
    version: optional_metadata_text(
      metadata_bit(bits, 8),
      version_bytes,
      operation,
      path,
      destination_path,
    ),
  }
}

///|
fn entry_from_native(
  entry : NativeEntry,
  listing_path : String,
) -> Entry raise OpenDalError {
  let path_bytes = native_entry_path(entry)
  let name_bytes = native_entry_name(entry)
  let metadata = native_entry_take_metadata(entry)
  native_entry_release(entry)
  let path = decode_abi_text(path_bytes, List, Some(listing_path), None)
  let name = decode_abi_text(name_bytes, List, Some(listing_path), None)
  { path, name, metadata: metadata_from_native(metadata, List, Some(path)) }
}
