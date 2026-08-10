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
/// Temporary owned native entry snapshot.
priv type NativeEntry

///|
/// Safe lister wrapper retaining the prefix used for error context.
pub struct Lister {
  priv native : NativeLister
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
#borrow(scheme, keys, values)
extern "C" fn native_operator_new(
  scheme : String,
  keys : FixedArray[String],
  values : FixedArray[String],
) -> NativeResult = "moonbit_opendal_operator_new"

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
#borrow(operator, path)
extern "C" fn native_operator_read(
  operator : Operator,
  path : String,
) -> NativeResult = "moonbit_opendal_operator_read"

///|
#borrow(operator, path, data)
extern "C" fn native_operator_write(
  operator : Operator,
  path : String,
  data : Bytes,
) -> NativeResult = "moonbit_opendal_operator_write"

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
) -> String? raise OpenDalError {
  if present {
    Some(decode_abi_text(bytes, operation, path, None))
  } else {
    None
  }
}

///|
fn metadata_from_native(
  metadata : NativeMetadata,
  operation : Operation,
  path : String?,
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
    ),
    content_disposition: optional_metadata_text(
      metadata_bit(bits, 3),
      content_disposition_bytes,
      operation,
      path,
    ),
    content_encoding: optional_metadata_text(
      metadata_bit(bits, 4),
      content_encoding_bytes,
      operation,
      path,
    ),
    content_md5: optional_metadata_text(
      metadata_bit(bits, 5),
      content_md5_bytes,
      operation,
      path,
    ),
    content_type: optional_metadata_text(
      metadata_bit(bits, 6),
      content_type_bytes,
      operation,
      path,
    ),
    etag: optional_metadata_text(
      metadata_bit(bits, 7),
      etag_bytes,
      operation,
      path,
    ),
    last_modified,
    version: optional_metadata_text(
      metadata_bit(bits, 8),
      version_bytes,
      operation,
      path,
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

///|
pub fn StatOptions::new(
  version? : String,
  if_match? : String,
  if_none_match? : String,
) -> StatOptions {
  { version, if_match, if_none_match }
}

///|
pub fn ListOptions::new(
  recursive? : Bool,
  limit? : UInt64,
  start_after? : String,
) -> ListOptions {
  { recursive: recursive.unwrap_or(false), limit, start_after }
}

///|
pub fn DeleteOptions::new(
  version? : String,
  recursive? : Bool,
) -> DeleteOptions {
  { version, recursive: recursive.unwrap_or(false) }
}

///|
pub fn Operator::new(
  scheme : StringView,
  config : Map[String, String],
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
pub fn Operator::check(self : Operator) -> Unit raise OpenDalError {
  let result = native_operator_check(self)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Check, None, None)
  }
  native_result_release(result)
}

///|
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
pub fn Operator::stat(
  self : Operator,
  path : StringView,
  options? : StatOptions,
) -> Metadata raise OpenDalError {
  let path = path.to_owned()
  let (
    has_version,
    version,
    has_if_match,
    if_match,
    has_if_none_match,
    if_none_match,
  ) = match options {
    None => (false, "", false, "", false, "")
    Some(value) =>
      (
        value.version is Some(_),
        value.version.unwrap_or(""),
        value.if_match is Some(_),
        value.if_match.unwrap_or(""),
        value.if_none_match is Some(_),
        value.if_none_match.unwrap_or(""),
      )
  }
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
pub fn Operator::read(
  self : Operator,
  path : StringView,
) -> Bytes raise OpenDalError {
  let path = path.to_owned()
  let result = native_operator_read(self, path)
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
pub fn Operator::write(
  self : Operator,
  path : StringView,
  data : Bytes,
) -> Metadata raise OpenDalError {
  let path = path.to_owned()
  let result = native_operator_write(self, path, data)
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
pub fn Operator::delete(
  self : Operator,
  path : StringView,
  options? : DeleteOptions,
) -> Unit raise OpenDalError {
  let path = path.to_owned()
  let (has_version, version, recursive) = match options {
    None => (false, "", false)
    Some(value) =>
      (value.version is Some(_), value.version.unwrap_or(""), value.recursive)
  }
  let result = native_operator_delete(
    self, path, has_version, version, recursive,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, Delete, Some(path), None)
  }
  native_result_release(result)
}

///|
pub fn Operator::lister(
  self : Operator,
  path : StringView,
  options? : ListOptions,
) -> Lister raise OpenDalError {
  let path = path.to_owned()
  let (recursive, has_limit, limit, has_start_after, start_after) = match
    options {
    None => (false, false, 0UL, false, "")
    Some(value) =>
      (
        value.recursive,
        value.limit is Some(_),
        value.limit.unwrap_or(0UL),
        value.start_after is Some(_),
        value.start_after.unwrap_or(""),
      )
  }
  let result = native_operator_lister(
    self, path, recursive, has_limit, limit, has_start_after, start_after,
  )
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, List, Some(path), None)
  }
  let native = native_result_take_lister(result)
  if native_result_status(result) != 0U {
    raise error_from_native_result(result, List, Some(path), None)
  }
  native_result_release(result)
  { native, path }
}

///|
pub fn Lister::next(self : Lister) -> Entry? raise OpenDalError {
  let result = native_lister_next(self.native)
  match native_result_status(result) {
    0U => {
      let entry = native_result_take_entry(result)
      if native_result_status(result) != 0U {
        self.close()
        raise error_from_native_result(result, List, Some(self.path), None)
      }
      native_result_release(result)
      let converted = try entry_from_native(entry, self.path) catch {
        error => {
          self.close()
          raise error
        }
      } noraise {
        value => value
      }
      Some(converted)
    }
    1U => {
      native_result_release(result)
      None
    }
    _ => {
      self.close()
      raise error_from_native_result(result, List, Some(self.path), None)
    }
  }
}

///|
pub fn Lister::close(self : Lister) -> Unit {
  native_lister_close(self.native)
}

///|
pub fn Operator::list(
  self : Operator,
  path : StringView,
  options? : ListOptions,
) -> Array[Entry] raise OpenDalError {
  let path = path.to_owned()
  let lister = self.lister(path, options?)
  let entries : Array[Entry] = []
  try {
    for ;; {
      match lister.next() {
        Some(entry) => {
          if entries.length() == 2147483647 {
            raise binding_error(
              BufferTooLarge,
              List,
              "list has too many entries to materialize in a MoonBit Array",
              Some(path),
              None,
            )
          }
          entries.push(entry)
        }
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
pub fn Metadata::is_file(self : Metadata) -> Bool {
  self.mode is File
}

///|
pub fn Metadata::is_dir(self : Metadata) -> Bool {
  self.mode is Directory
}

///|
fn Capability::has(self : Capability, bit : Int) -> Bool {
  let word = bit / 64
  let offset = bit % 64
  (native_capability_word(self, word) & (1UL << offset)) != 0UL
}

///|
pub fn Capability::can_stat(self : Capability) -> Bool {
  self.has(0)
}

///|
pub fn Capability::can_read(self : Capability) -> Bool {
  self.has(1)
}

///|
pub fn Capability::can_write(self : Capability) -> Bool {
  self.has(2)
}

///|
pub fn Capability::can_create_dir(self : Capability) -> Bool {
  self.has(3)
}

///|
pub fn Capability::can_delete(self : Capability) -> Bool {
  self.has(4)
}

///|
pub fn Capability::can_list(self : Capability) -> Bool {
  self.has(5)
}

///|
pub fn Capability::can_list_with_limit(self : Capability) -> Bool {
  self.has(10)
}

///|
pub fn Capability::can_list_with_start_after(self : Capability) -> Bool {
  self.has(11)
}

///|
pub fn Capability::can_list_recursive(self : Capability) -> Bool {
  self.has(12)
}
