///|
/// An independent byte range for whole-object reads.
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
#warnings("-unused_constructor")
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
priv struct ErrorContext {
  operation : Operation
  path : String?
  destination_path : String?
}

///|
priv struct BridgeErrorSnapshot {
  kind : ErrorKind
  status : ErrorStatus
  message : String
}

///|
/// Identifies whether a listed entry is a file, directory, or unknown kind.
pub enum EntryMode {
  File
  Directory
  Unknown
} derive(Eq, Debug)

///|
/// A UTC instant represented without imposing a calendar/time-zone package.
/// `nanoseconds` is always in the range 0 through 999,999,999 for values
/// produced by this package.
pub struct Timestamp {
  unix_seconds : Int64
  nanoseconds : Int
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
pub fn Metadata::is_file(self : Metadata) -> Bool {
  self.mode is File
}

///|
pub fn Metadata::is_dir(self : Metadata) -> Bool {
  self.mode is Directory
}

///|
/// An owned path and metadata snapshot returned by a bounded list operation.
pub struct Entry {
  path : String
  name : String
  metadata : Metadata
} derive(Eq, Debug)

///|
pub fn Entry::is_file(self : Entry) -> Bool {
  self.metadata.is_file()
}

///|
pub fn Entry::is_dir(self : Entry) -> Bool {
  self.metadata.is_dir()
}

///|
/// A portable snapshot of operations supported by one configured backend.
pub struct Capability {
  priv word0 : UInt64
  priv delete_max_size_value : UInt64
} derive(Eq, Debug)

///|
fn Capability::has(self : Capability, bit : Int) -> Bool {
  (self.word0 & (1UL << bit)) != 0UL
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
pub fn Capability::can_copy(self : Capability) -> Bool {
  self.has(6)
}

///|
pub fn Capability::can_rename(self : Capability) -> Bool {
  self.has(7)
}

///|
pub fn Capability::can_read_suffix(self : Capability) -> Bool {
  self.has(8)
}

///|
pub fn Capability::can_write_append(self : Capability) -> Bool {
  self.has(9)
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

///|
pub fn Capability::can_presign_stat(self : Capability) -> Bool {
  self.has(13)
}

///|
pub fn Capability::can_presign_read(self : Capability) -> Bool {
  self.has(14)
}

///|
pub fn Capability::can_presign_write(self : Capability) -> Bool {
  self.has(15)
}

///|
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
/// Read-only service identity and effective capability snapshot.
pub struct OperatorInfo {
  scheme : String
  root : String
  name : String
  capability : Capability
} derive(Eq, Debug)

///|
/// A generation-checked operator handle owned by the Rust Wasm bridge.
pub struct Operator {
  priv mut handle : UInt
  priv info_snapshot : OperatorInfo
}

///|
/// A callback-based asynchronous view of an Operator.
///
/// The view shares the configured operator and performs no I/O when created.
pub struct AsyncOperator {
  priv native : Operator
}

///|
/// Observable state of an experimental callback task.
pub enum TaskState {
  Pending
  Completed
  Cancelled
  Closed
} derive(Eq, Debug)

///|
/// A cancellable browser task driven by the official loader.
///
/// This callback surface is experimental until MoonBit exposes a documented
/// continuation API for ordinary browser Wasm hosts.
pub struct Task {
  priv mut handle : UInt
  priv mut state : TaskState
}
