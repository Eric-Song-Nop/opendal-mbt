///|
/// Identifies whether an entry is a file, directory, or unknown object kind.
pub enum EntryMode {
  File
  Directory
  Unknown
} derive(Eq, @debug.Debug)

///|
/// An independent byte range for whole-object or random-access reads.
/// The `length` field is a byte count, not an end offset.
/// `Suffix` requires the selected backend's native suffix capability.
pub(all) enum ByteRange {
  Full
  From(offset~ : UInt64)
  Range(offset~ : UInt64, length~ : UInt64)
  Suffix(length~ : UInt64)
} derive(Eq, @debug.Debug)

///|
/// OpenDAL's retry classification for an operation failure.
pub enum ErrorStatus {
  Permanent
  Temporary
  Persistent
  UnknownStatus(Int)
} derive(Eq, @debug.Debug)

///|
/// Stable error categories exposed by the MoonBit binding.
pub enum ErrorKind {
  Unexpected
  Unsupported
  ConfigInvalid
  NotFound
  PermissionDenied
  IsADirectory
  NotADirectory
  AlreadyExists
  RateLimited
  IsSameFile
  ConditionNotMatch
  RangeNotSatisfied
  InvalidArgument
  ResourceClosed
  BufferTooLarge
  AbiMismatch
  UnknownKind(Int, String)
} derive(Eq, @debug.Debug)

///|
/// The operation being attempted when an error occurred.
pub enum Operation {
  NewOperator
  Check
  Exists
  CreateDir
  Read
  Write
  Stat
  Delete
  List
  Copy
  Rename
} derive(Eq, @debug.Debug)

///|
/// A UTC instant represented without imposing a calendar/time-zone package.
/// `nanoseconds` is always in the range 0 through 999,999,999 for values
/// produced by this package.
pub struct Timestamp {
  unix_seconds : Int64
  nanoseconds : Int
} derive(Eq, @debug.Debug)

///|
/// Structured context attached to a storage failure.
pub struct ErrorInfo {
  kind : ErrorKind
  status : ErrorStatus
  message : String
  operation : Operation
  path : String?
  destination_path : String?
} derive(Eq, @debug.Debug)

///|
/// The checked error effect raised by all fallible OpenDAL operations.
pub suberror OpenDalError {
  OpenDalError(ErrorInfo)
} derive(Eq, @debug.Debug)

///|
/// Read-only metadata snapshot associated with one operation context.
pub struct Metadata {
  mode : EntryMode
  content_length : UInt64
  is_current : Bool?
  is_deleted : Bool
  cache_control : String?
  content_disposition : String?
  content_encoding : String?
  content_md5 : String?
  content_type : String?
  etag : String?
  last_modified : Timestamp?
  version : String?
} derive(Eq, @debug.Debug)

///|
/// A path and its possibly-partial metadata returned by a list operation.
pub struct Entry {
  path : String
  name : String
  metadata : Metadata
} derive(Eq, @debug.Debug)

///|
/// Read-only information about one configured operator.
pub struct OperatorInfo {
  scheme : String
  root : String
  name : String
  capability : Capability
}

///|
/// One owned HTTP header from a presigned request.
///
/// Header values remain bytes because a valid HTTP field value is not
/// required to be UTF-8. This type deliberately has no Debug or Show
/// implementation because signing material can appear in header values.
pub struct PresignedHeader {
  name : String
  value : Bytes
} derive(Eq)

///|
/// A fully owned HTTP request snapshot produced by an OpenDAL presign
/// operation.
///
/// Header order and duplicate names are preserved. The URI and headers can
/// contain bearer-equivalent signing material, so this type deliberately has
/// no Debug or Show implementation.
pub struct PresignedRequest {
  http_method : String
  uri : String
  headers : Array[PresignedHeader]
} derive(Eq)
