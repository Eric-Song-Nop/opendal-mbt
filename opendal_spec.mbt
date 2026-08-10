///|
/// Identifies whether an entry is a file, directory, or unknown object kind.
pub(all) enum EntryMode {
  File
  Directory
  Unknown
} derive(Eq, @debug.Debug)

///|
/// OpenDAL's retry classification for an operation failure.
pub(all) enum ErrorStatus {
  Permanent
  Temporary
  Persistent
  UnknownStatus(Int)
} derive(Eq, @debug.Debug)

///|
/// Stable error categories exposed by the MoonBit binding.
pub(all) enum ErrorKind {
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
pub(all) enum Operation {
  NewOperator
  Check
  Info
  CreateDir
  Read
  Write
  Copy
  Rename
  Stat
  Delete
  List
  UnknownOperation(String)
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
} derive(Eq, @debug.Debug)

///|
/// The checked error effect raised by all fallible OpenDAL operations.
pub(all) suberror OpenDalError {
  OpenDalError(ErrorInfo)
} derive(Eq, @debug.Debug)

///|
/// A byte range. `Range(offset, length)` uses a length, not an end offset.
pub(all) enum ByteRange {
  Full
  From(UInt64)
  Range(UInt64, UInt64)
  Suffix(UInt64)
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
/// Effective or native capabilities captured from an operator.
#declaration_only
pub type Capability

///|
/// Read-only information about one configured operator.
pub struct OperatorInfo {
  scheme : String
  root : String
  name : String
  native_capability : Capability
  full_capability : Capability
}

///|
/// Immutable options for a whole-object read.
pub struct ReadOptions {
  range : ByteRange?
  version : String?
  if_match : String?
  if_none_match : String?
}

///|
/// Immutable options for creating a random-access reader.
pub struct ReaderOptions {
  version : String?
  if_match : String?
  if_none_match : String?
}

///|
/// Immutable options for a write or writer.
pub struct WriteOptions {
  append : Bool
  content_type : String?
  content_disposition : String?
  content_encoding : String?
  cache_control : String?
  if_match : String?
  if_none_match : String?
}

///|
/// Immutable options for a metadata request.
pub struct StatOptions {
  version : String?
  if_match : String?
  if_none_match : String?
}

///|
/// Immutable options for a prefix listing.
pub struct ListOptions {
  recursive : Bool
  limit : UInt64?
  start_after : String?
}

///|
/// Immutable options for deletion.
pub struct DeleteOptions {
  version : String?
  recursive : Bool
}

///|
/// An immutable, shareable storage operator.
#declaration_only
pub type Operator

///|
/// A stateful, fallible prefix-listing cursor.
#declaration_only
pub type Lister

///|
/// A random-access reader. It has no implicit cursor.
#declaration_only
pub type Reader

///|
/// A stateful writer that commits only after a successful explicit close.
#declaration_only
pub type Writer

///|
/// Construct a whole-object read options value.
#declaration_only
pub fn ReadOptions::new(
  range? : ByteRange,
  version? : String,
  if_match? : String,
  if_none_match? : String,
) -> ReadOptions {
  ...
}

///|
/// Construct random-access reader options.
#declaration_only
pub fn ReaderOptions::new(
  version? : String,
  if_match? : String,
  if_none_match? : String,
) -> ReaderOptions {
  ...
}

///|
/// Construct write options. All supplied strings become binding-owned values.
#declaration_only
pub fn WriteOptions::new(
  append? : Bool,
  content_type? : String,
  content_disposition? : String,
  content_encoding? : String,
  cache_control? : String,
  if_match? : String,
  if_none_match? : String,
) -> WriteOptions {
  ...
}

///|
/// Construct stat options.
#declaration_only
pub fn StatOptions::new(
  version? : String,
  if_match? : String,
  if_none_match? : String,
) -> StatOptions {
  ...
}

///|
/// Construct list options. `limit` is a backend page/request hint, not a
/// guaranteed maximum number of entries produced by a lister.
#declaration_only
pub fn ListOptions::new(
  recursive? : Bool,
  limit? : UInt64,
  start_after? : String,
) -> ListOptions {
  ...
}

///|
/// Construct delete options. Recursive deletion is requested here instead of
/// through a separate `remove_all` operation.
#declaration_only
pub fn DeleteOptions::new(
  version? : String,
  recursive? : Bool,
) -> DeleteOptions {
  ...
}

///|
/// Construct an operator from a service scheme and configuration map.
/// The map is copied and is never mutated or retained by reference.
#declaration_only
pub fn Operator::new(
  scheme : StringView,
  config : Map[String, String],
) -> Operator raise OpenDalError {
  ...
}

///|
/// Verify that the operator can perform OpenDAL's check operation.
#declaration_only
pub fn Operator::check(self : Operator) -> Unit raise OpenDalError {
  ...
}

///|
/// Return a read-only information and capability snapshot.
#declaration_only
pub fn Operator::info(self : Operator) -> OperatorInfo {
  ...
}

///|
/// Check whether a path exists. Only `NotFound` becomes false; all other
/// failures remain errors.
#declaration_only
pub fn Operator::exists(
  self : Operator,
  path : StringView,
) -> Bool raise OpenDalError {
  ...
}

///|
/// Fetch metadata for a path.
#declaration_only
pub fn Operator::stat(
  self : Operator,
  path : StringView,
  options? : StatOptions,
) -> Metadata raise OpenDalError {
  ...
}

///|
/// Read an entire path into one MoonBit Bytes value.
#declaration_only
pub fn Operator::read(
  self : Operator,
  path : StringView,
  options? : ReadOptions,
) -> Bytes raise OpenDalError {
  ...
}

///|
/// Write an entire Bytes value and return the resulting metadata.
#declaration_only
pub fn Operator::write(
  self : Operator,
  path : StringView,
  data : Bytes,
  options? : WriteOptions,
) -> Metadata raise OpenDalError {
  ...
}

///|
/// Create a directory recursively. A directory path must end with `/`.
#declaration_only
pub fn Operator::create_dir(
  self : Operator,
  path : StringView,
) -> Unit raise OpenDalError {
  ...
}

///|
/// Delete a path. Deleting a missing path is successful.
#declaration_only
pub fn Operator::delete(
  self : Operator,
  path : StringView,
  options? : DeleteOptions,
) -> Unit raise OpenDalError {
  ...
}

///|
/// Materialize a prefix listing into memory.
#declaration_only
pub fn Operator::list(
  self : Operator,
  path : StringView,
  options? : ListOptions,
) -> Array[Entry] raise OpenDalError {
  ...
}

///|
/// Create a streaming prefix lister.
#declaration_only
pub fn Operator::lister(
  self : Operator,
  path : StringView,
  options? : ListOptions,
) -> Lister raise OpenDalError {
  ...
}

///|
/// Create a random-access reader with no implicit position.
#declaration_only
pub fn Operator::reader(
  self : Operator,
  path : StringView,
  options? : ReaderOptions,
) -> Reader raise OpenDalError {
  ...
}

///|
/// Create a chunked writer. The caller must call `Writer::close` to commit.
#declaration_only
pub fn Operator::writer(
  self : Operator,
  path : StringView,
  options? : WriteOptions,
) -> Writer raise OpenDalError {
  ...
}

///|
/// Copy one file and return destination metadata.
#declaration_only
pub fn Operator::copy(
  self : Operator,
  from : StringView,
  to : StringView,
) -> Metadata raise OpenDalError {
  ...
}

///|
/// Rename one file without emulating the operation as copy plus delete.
#declaration_only
pub fn Operator::rename(
  self : Operator,
  from : StringView,
  to : StringView,
) -> Unit raise OpenDalError {
  ...
}

///|
/// Return one entry, `None` at end of listing, or raise on failure. After an
/// error the lister is exhausted.
#declaration_only
pub fn Lister::next(self : Lister) -> Entry? raise OpenDalError {
  ...
}

///|
/// Release listing resources early. Closing more than once is harmless.
#declaration_only
pub fn Lister::close(self : Lister) -> Unit {
  ...
}

///|
/// Read one range without changing hidden cursor state. A short result means
/// the requested range reached the end of the object.
#declaration_only
pub fn Reader::read(
  self : Reader,
  range : ByteRange,
) -> Bytes raise OpenDalError {
  ...
}

///|
/// Release reader resources early. Closing more than once is harmless.
#declaration_only
pub fn Reader::close(self : Reader) -> Unit {
  ...
}

///|
/// Append one chunk to an open writer.
#declaration_only
pub fn Writer::write(self : Writer, data : Bytes) -> Unit raise OpenDalError {
  ...
}

///|
/// Commit all data and return metadata. The first close attempt is terminal;
/// dropping a writer without a successful close never counts as a commit.
#declaration_only
pub fn Writer::close(self : Writer) -> Metadata raise OpenDalError {
  ...
}

///|
/// Return true when metadata describes a file.
#declaration_only
pub fn Metadata::is_file(self : Metadata) -> Bool {
  ...
}

///|
/// Return true when metadata describes a directory.
#declaration_only
pub fn Metadata::is_dir(self : Metadata) -> Bool {
  ...
}

///|
/// Basic operation capabilities.
#declaration_only
pub fn Capability::can_stat(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_read(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_write(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_create_dir(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_delete(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_list(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_copy(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_rename(self : Capability) -> Bool {
  ...
}

///|
/// Selected option capabilities. More accessors can be added compatibly.
#declaration_only
pub fn Capability::can_read_range(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_write_append(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_list_with_limit(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_list_with_start_after(self : Capability) -> Bool {
  ...
}

///|
#declaration_only
pub fn Capability::can_list_recursive(self : Capability) -> Bool {
  ...
}
