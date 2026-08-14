///|
/// An independent byte range for a whole-object read.
///
/// `Range` uses a byte count for `length`, not an inclusive end offset.
/// `Suffix` requires the selected backend to advertise native suffix support.
pub(all) enum ByteRange {
  Full
  From(offset~ : UInt64)
  Range(offset~ : UInt64, length~ : UInt64)
  Suffix(length~ : UInt64)
} derive(Eq, @debug.Debug)

///|
/// OpenDAL's retry classification for an operation failure.
///
/// A temporary or persistent classification is diagnostic information; it
/// does not by itself make an operation safe to retry.
pub enum ErrorStatus {
  Permanent
  Temporary
  Persistent
  UnknownStatus(Int)
} derive(Eq, @debug.Debug)

///|
/// Stable error categories exposed by the browser binding.
///
/// `Cancelled` represents cancellation requested through the JavaScript
/// `AbortSignal`; unknown bridge values are preserved instead of being folded
/// into `Unexpected`.
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
  ResourceBusy
  Cancelled
  BufferTooLarge
  AbiMismatch
  UnknownKind(Int, String)
} derive(Eq, @debug.Debug)

///|
/// The public operation being attempted when an error occurred.
#warnings("-unused_constructor")
pub enum Operation {
  LoadRuntime
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
/// Immutable context attached to a browser OpenDAL failure.
///
/// The bridge supplies `kind`, `status`, and `message`; the MoonBit facade
/// attaches the initiating operation and paths at the call site.
pub struct ErrorInfo {
  kind : ErrorKind
  status : ErrorStatus
  message : String
  operation : Operation
  path : String?
  destination_path : String?
} derive(Eq, @debug.Debug)

///|
/// The checked error effect raised by fallible browser OpenDAL operations.
pub suberror OpenDalError {
  OpenDalError(ErrorInfo)
} derive(Eq, @debug.Debug)

///|
/// Package-internal call-site context used while translating bridge outcomes.
priv struct ErrorContext {
  operation : Operation
  path : String?
  destination_path : String?
}

///|
/// Identifies whether an entry is a file, directory, or an unknown kind.
pub enum EntryMode {
  File
  Directory
  Unknown
} derive(Eq, @debug.Debug)

///|
/// A UTC instant represented without imposing a calendar or time-zone package.
///
/// Values produced by this package normalize `nanoseconds` to the inclusive
/// range `0..999_999_999`, including instants before the Unix epoch.
pub struct Timestamp {
  unix_seconds : Int64
  nanoseconds : Int
} derive(Eq, @debug.Debug)

///|
/// An owned metadata snapshot returned by one browser OpenDAL operation.
///
/// Optional text fields distinguish an absent value from a present empty
/// string. `content_length` retains the complete unsigned 64-bit value and
/// `last_modified` retains nanosecond precision.
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
/// Return whether this metadata describes a regular file.
pub fn Metadata::is_file(self : Metadata) -> Bool {
  self.mode is File
}

///|
/// Return whether this metadata describes a directory.
pub fn Metadata::is_dir(self : Metadata) -> Bool {
  self.mode is Directory
}

///|
/// An owned path, basename, and metadata snapshot returned by a bounded list.
pub struct Entry {
  path : String
  name : String
  metadata : Metadata
} derive(Eq, @debug.Debug)

///|
/// Return whether this entry describes a regular file.
pub fn Entry::is_file(self : Entry) -> Bool {
  self.metadata.is_file()
}

///|
/// Return whether this entry describes a directory.
pub fn Entry::is_dir(self : Entry) -> Bool {
  self.metadata.is_dir()
}

///|
/// A portable capability snapshot for one configured OpenDAL operator.
///
/// The browser package constructs this value from versioned bridge words;
/// callers inspect it through the methods below instead of depending on bit
/// positions.
pub struct Capability {
  priv word0 : UInt64
  priv delete_max_size_value : UInt64
} derive(Eq, @debug.Debug)

///|
/// Return whether the capability bit at `bit` is set.
fn Capability::has(self : Capability, bit : Int) -> Bool {
  (self.word0 & (1UL << bit)) != 0UL
}

///|
/// Return whether the operator supports metadata lookup.
pub fn Capability::can_stat(self : Capability) -> Bool {
  self.has(0)
}

///|
/// Return whether the operator supports whole-object reads.
pub fn Capability::can_read(self : Capability) -> Bool {
  self.has(1)
}

///|
/// Return whether the operator supports whole-object writes.
pub fn Capability::can_write(self : Capability) -> Bool {
  self.has(2)
}

///|
/// Return whether the operator supports recursive directory creation.
pub fn Capability::can_create_dir(self : Capability) -> Bool {
  self.has(3)
}

///|
/// Return whether the operator supports deletion.
pub fn Capability::can_delete(self : Capability) -> Bool {
  self.has(4)
}

///|
/// Return whether the operator supports listing.
pub fn Capability::can_list(self : Capability) -> Bool {
  self.has(5)
}

///|
/// Return whether the operator supports server-side copy.
pub fn Capability::can_copy(self : Capability) -> Bool {
  self.has(6)
}

///|
/// Return whether the operator supports server-side rename.
pub fn Capability::can_rename(self : Capability) -> Bool {
  self.has(7)
}

///|
/// Return whether the backend has native suffix-range read support.
pub fn Capability::can_read_suffix(self : Capability) -> Bool {
  self.has(8)
}

///|
/// Return whether the operator accepts append writes.
pub fn Capability::can_write_append(self : Capability) -> Bool {
  self.has(9)
}

///|
/// Return whether list requests accept an entry limit.
pub fn Capability::can_list_with_limit(self : Capability) -> Bool {
  self.has(10)
}

///|
/// Return whether list requests accept a `start_after` cursor.
pub fn Capability::can_list_with_start_after(self : Capability) -> Bool {
  self.has(11)
}

///|
/// Return whether the operator supports recursive listing.
pub fn Capability::can_list_recursive(self : Capability) -> Bool {
  self.has(12)
}

///|
/// Return whether the operator supports presigned stat requests.
pub fn Capability::can_presign_stat(self : Capability) -> Bool {
  self.has(13)
}

///|
/// Return whether the operator supports presigned read requests.
pub fn Capability::can_presign_read(self : Capability) -> Bool {
  self.has(14)
}

///|
/// Return whether the operator supports presigned write requests.
pub fn Capability::can_presign_write(self : Capability) -> Bool {
  self.has(15)
}

///|
/// Return the backend's maximum batch-delete size when one is reported.
///
/// The portable MoonBit value is saturated at `Int::max_value()` when the
/// bridge reports a larger unsigned limit.
pub fn Capability::delete_max_size(self : Capability) -> Int? {
  if self.delete_max_size_value == 0UL {
    None
  } else if self.delete_max_size_value > 2147483647UL {
    Some(2147483647)
  } else {
    Some(self.delete_max_size_value.to_int())
  }
}

///|
/// Read-only service identity and effective capabilities for an operator.
pub struct OperatorInfo {
  scheme : String
  root : String
  name : String
  capability : Capability
} derive(Eq, @debug.Debug)
