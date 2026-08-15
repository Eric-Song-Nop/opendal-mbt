//! Scalar WebAssembly core used by the browser JavaScript Promise facade.
//!
//! It owns OpenDAL resources behind generation-checked handles. JavaScript
//! translates the task ABI into promises; MoonBit never observes these raw
//! handles and does not need a second WebAssembly memory.

use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};

use futures_util::TryStreamExt;
use opendal::{
    Buffer, BufferStream, BytesRange, ErrorKind, Lister, Metadata, Operator, OperatorRegistry,
    Writer, options, services::Memory,
};

const ABI_VERSION: u32 = 0x0001_0007;
const FEATURE_MEMORY_SERVICE: u32 = 1 << 0;
const FEATURE_POLL_ONCE_CANARY: u32 = 1 << 1;
const FEATURE_GENERATION_HANDLES: u32 = 1 << 2;
const FEATURE_BINARY_BUFFERS: u32 = 1 << 3;
const FEATURE_TASK_ABI: u32 = 1 << 4;
const FEATURE_GENERIC_OPERATOR: u32 = 1 << 5;
const FEATURE_CORE_MUTATIONS: u32 = 1 << 6;
const FEATURE_BOUNDED_LIST: u32 = 1 << 7;
const FEATURE_BULK_TRANSFER: u32 = 1 << 8;
const FEATURE_STRUCTURED_ERRORS: u32 = 1 << 9;
const FEATURE_METADATA_OPTIONS: u32 = 1 << 10;
const FEATURE_COPY_RENAME: u32 = 1 << 11;
const FEATURE_STATEFUL_READER: u32 = 1 << 12;
const FEATURE_STATEFUL_WRITER: u32 = 1 << 13;
const FEATURE_STATEFUL_LISTER: u32 = 1 << 14;
const FEATURE_EXPLICIT_EOF: u32 = 1 << 15;
const MAX_SLOTS: usize = u16::MAX as usize;
const MAX_GENERATION: u16 = i16::MAX as u16;
const MAX_BUFFER_LENGTH: usize = 64 * 1024 * 1024;
const MAX_TRANSFER_CHUNK: usize = 256 * 1024;
const MAX_LIST_ENTRIES: usize = 65_536;
const MAX_LIST_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_CONFIG_ENTRIES: usize = 1_024;
const MAX_CONFIG_BYTES: usize = 1024 * 1024;

const STATUS_OK: u32 = 0;
const SCALAR_ERROR: i32 = -1;

const ERROR_UNEXPECTED: u32 = 1;
const ERROR_UNSUPPORTED: u32 = 2;
const ERROR_CONFIG_INVALID: u32 = 3;
const ERROR_NOT_FOUND: u32 = 4;
const ERROR_PERMISSION_DENIED: u32 = 5;
const ERROR_IS_A_DIRECTORY: u32 = 6;
const ERROR_NOT_A_DIRECTORY: u32 = 7;
const ERROR_ALREADY_EXISTS: u32 = 8;
const ERROR_RATE_LIMITED: u32 = 9;
const ERROR_IS_SAME_FILE: u32 = 10;
const ERROR_CONDITION_NOT_MATCH: u32 = 11;
const ERROR_RANGE_NOT_SATISFIED: u32 = 12;
const ERROR_INVALID_ARGUMENT: u32 = 0x1001;
const ERROR_RESOURCE_CLOSED: u32 = 0x1002;
const ERROR_BUFFER_TOO_LARGE: u32 = 0x1003;
const ERROR_ABI_MISMATCH: u32 = 0x1004;
const ERROR_RESOURCE_BUSY: u32 = 0x1006;

const ERROR_STATUS_PERMANENT: u32 = 1;
const ERROR_STATUS_TEMPORARY: u32 = 2;
const ERROR_STATUS_PERSISTENT: u32 = 3;

const ERROR_SNAPSHOT_MAGIC: [u8; 4] = *b"ODE1";
const ERROR_SNAPSHOT_SCHEMA: u32 = 1;
const ERROR_SNAPSHOT_HEADER_LENGTH: usize = 24;

const METADATA_SNAPSHOT_MAGIC: [u8; 4] = *b"ODM1";
const METADATA_SNAPSHOT_SCHEMA: u32 = 1;
const METADATA_SNAPSHOT_HEADER_LENGTH: usize = 84;
const METADATA_IS_CURRENT_PRESENT: u64 = 1 << 0;
const METADATA_LAST_MODIFIED_PRESENT: u64 = 1 << 1;
const METADATA_CACHE_CONTROL_PRESENT: u64 = 1 << 2;
const METADATA_CONTENT_DISPOSITION_PRESENT: u64 = 1 << 3;
const METADATA_CONTENT_ENCODING_PRESENT: u64 = 1 << 4;
const METADATA_CONTENT_MD5_PRESENT: u64 = 1 << 5;
const METADATA_CONTENT_TYPE_PRESENT: u64 = 1 << 6;
const METADATA_ETAG_PRESENT: u64 = 1 << 7;
const METADATA_VERSION_PRESENT: u64 = 1 << 8;

const RANGE_FULL: u32 = 0;
const RANGE_FROM: u32 = 1;
const RANGE_OFFSET_LENGTH: u32 = 2;
const RANGE_SUFFIX: u32 = 3;

const TASK_PENDING: u32 = 1;
const TASK_READY: u32 = 2;
const TASK_CANCELLED: u32 = 3;
const TASK_CONSUMED: u32 = 4;

const COMPLETION_WRITE: u32 = 1;
const COMPLETION_READ: u32 = 2;
const COMPLETION_STAT: u32 = 3;
const COMPLETION_CREATE_DIR: u32 = 4;
const COMPLETION_DELETE: u32 = 5;
const COMPLETION_LIST: u32 = 6;
const COMPLETION_COPY: u32 = 7;
const COMPLETION_RENAME: u32 = 8;
const COMPLETION_OPEN_READER: u32 = 9;
const COMPLETION_READER_NEXT: u32 = 10;
const COMPLETION_OPEN_WRITER: u32 = 11;
const COMPLETION_WRITER_WRITE: u32 = 12;
const COMPLETION_WRITER_FINISH: u32 = 13;
const COMPLETION_WRITER_ABORT: u32 = 14;
const COMPLETION_OPEN_LISTER: u32 = 15;
const COMPLETION_LISTER_NEXT: u32 = 16;

const ENTRY_MODE_UNKNOWN: u32 = 0;
const ENTRY_MODE_FILE: u32 = 1;
const ENTRY_MODE_DIRECTORY: u32 = 2;

const CAP_STAT: u64 = 1 << 0;
const CAP_READ: u64 = 1 << 1;
const CAP_WRITE: u64 = 1 << 2;
const CAP_CREATE_DIR: u64 = 1 << 3;
const CAP_DELETE: u64 = 1 << 4;
const CAP_LIST: u64 = 1 << 5;
const CAP_COPY: u64 = 1 << 6;
const CAP_RENAME: u64 = 1 << 7;
const CAP_READ_SUFFIX: u64 = 1 << 8;
const CAP_WRITE_APPEND: u64 = 1 << 9;
const CAP_LIST_LIMIT: u64 = 1 << 10;
const CAP_LIST_START_AFTER: u64 = 1 << 11;
const CAP_LIST_RECURSIVE: u64 = 1 << 12;
const CAP_PRESIGN_STAT: u64 = 1 << 13;
const CAP_PRESIGN_READ: u64 = 1 << 14;
const CAP_PRESIGN_WRITE: u64 = 1 << 15;

#[derive(Clone, Debug, thiserror::Error)]
enum BridgeError {
    #[error("invalid or stale handle {handle}")]
    InvalidHandle { handle: u32 },
    #[error("handle {handle} contains {actual}, expected {expected}")]
    WrongResourceType {
        handle: u32,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("buffer cannot grow beyond {MAX_BUFFER_LENGTH} bytes in ABI v1")]
    BufferTooLarge,
    #[error("buffer index {index} is out of bounds for length {length}")]
    IndexOutOfBounds { index: u32, length: usize },
    #[error("{value} is not an unsigned byte")]
    InvalidByte { value: u32 },
    #[error("path buffer is not valid UTF-8")]
    InvalidUtf8,
    #[error("OpenDAL operation failed: {message}")]
    OpenDal {
        kind: u32,
        status: u32,
        kind_name: String,
        message: String,
    },
    #[error("OpenDAL future returned Pending in the poll-once canary adapter")]
    AsyncPending,
    #[error("the bridge has reached its {MAX_SLOTS}-handle capacity")]
    HandleLimit,
    #[error("value cannot be represented by the scalar ABI")]
    LengthOverflow,
    #[error("task {handle} is not ready")]
    TaskNotReady { handle: u32 },
    #[error("task {handle} has already been consumed")]
    TaskConsumed { handle: u32 },
    #[error("the bridge instance has been torn down")]
    TornDown,
    #[error("resource {handle} is closed")]
    ResourceClosed { handle: u32 },
    #[error("resource {handle} already has an operation in flight")]
    ResourceBusy { handle: u32 },
    #[error(
        "list exceeds the binding limits of {MAX_LIST_ENTRIES} entries or {MAX_LIST_OUTPUT_BYTES} UTF-8 bytes"
    )]
    ListTooLarge,
    #[error("the bridge could not reserve memory for an owned result")]
    AllocationFailed,
    #[error("invalid scalar argument: {message}")]
    InvalidArgument { message: String },
}

impl BridgeError {
    /// Returns the legacy scalar error code retained by the v1 task ABI.
    fn code(&self) -> u32 {
        match self {
            Self::InvalidHandle { .. } => 1,
            Self::WrongResourceType { .. } => 2,
            Self::BufferTooLarge => 3,
            Self::IndexOutOfBounds { .. } => 4,
            Self::InvalidByte { .. } => 5,
            Self::InvalidUtf8 => 6,
            Self::OpenDal { kind, .. } if *kind == ERROR_NOT_FOUND => 7,
            Self::OpenDal { .. } => 8,
            Self::AsyncPending => 9,
            Self::HandleLimit => 10,
            Self::LengthOverflow => 11,
            Self::TaskNotReady { .. } => 12,
            Self::TaskConsumed { .. } => 13,
            Self::TornDown => 14,
            Self::ListTooLarge => 15,
            Self::AllocationFailed => 16,
            Self::InvalidArgument { .. } => 17,
            Self::ResourceClosed { .. } => 1,
            Self::ResourceBusy { .. } => 18,
        }
    }

    fn kind(&self) -> u32 {
        match self {
            Self::InvalidHandle { .. }
            | Self::TaskNotReady { .. }
            | Self::TaskConsumed { .. }
            | Self::TornDown
            | Self::ResourceClosed { .. } => ERROR_RESOURCE_CLOSED,
            Self::ResourceBusy { .. } => ERROR_RESOURCE_BUSY,
            Self::WrongResourceType { .. } => ERROR_ABI_MISMATCH,
            Self::IndexOutOfBounds { .. }
            | Self::InvalidByte { .. }
            | Self::InvalidUtf8
            | Self::InvalidArgument { .. } => ERROR_INVALID_ARGUMENT,
            Self::BufferTooLarge | Self::LengthOverflow | Self::ListTooLarge => {
                ERROR_BUFFER_TOO_LARGE
            }
            Self::OpenDal { kind, .. } => *kind,
            Self::AsyncPending => ERROR_UNEXPECTED,
            Self::HandleLimit | Self::AllocationFailed => ERROR_UNEXPECTED,
        }
    }

    fn error_status(&self) -> u32 {
        match self {
            Self::OpenDal { status, .. } => *status,
            _ => ERROR_STATUS_PERMANENT,
        }
    }

    fn kind_name(&self) -> &str {
        match self {
            Self::InvalidHandle { .. }
            | Self::TaskNotReady { .. }
            | Self::TaskConsumed { .. }
            | Self::TornDown
            | Self::ResourceClosed { .. } => "ResourceClosed",
            Self::ResourceBusy { .. } => "ResourceBusy",
            Self::WrongResourceType { .. } => "AbiMismatch",
            Self::IndexOutOfBounds { .. }
            | Self::InvalidByte { .. }
            | Self::InvalidUtf8
            | Self::InvalidArgument { .. } => "InvalidArgument",
            Self::BufferTooLarge | Self::LengthOverflow | Self::ListTooLarge => "BufferTooLarge",
            Self::OpenDal { kind_name, .. } => kind_name,
            Self::AsyncPending => "Unexpected",
            Self::HandleLimit | Self::AllocationFailed => "Unexpected",
        }
    }

    fn diagnostic_message(&self) -> Cow<'_, str> {
        match self {
            Self::OpenDal { message, .. } => Cow::Borrowed(message),
            _ => Cow::Owned(self.to_string()),
        }
    }

    fn from_construction_error(error: opendal::Error) -> Self {
        let mut snapshot = Self::from(error);
        if let Self::OpenDal { message, .. } = &mut snapshot {
            *message = "operator construction failed".to_owned();
        }
        snapshot
    }
}

impl From<opendal::Error> for BridgeError {
    fn from(error: opendal::Error) -> Self {
        let kind = error.kind();
        let kind_code = match kind {
            ErrorKind::Unexpected => ERROR_UNEXPECTED,
            ErrorKind::Unsupported => ERROR_UNSUPPORTED,
            ErrorKind::ConfigInvalid => ERROR_CONFIG_INVALID,
            ErrorKind::NotFound => ERROR_NOT_FOUND,
            ErrorKind::PermissionDenied => ERROR_PERMISSION_DENIED,
            ErrorKind::IsADirectory => ERROR_IS_A_DIRECTORY,
            ErrorKind::NotADirectory => ERROR_NOT_A_DIRECTORY,
            ErrorKind::AlreadyExists => ERROR_ALREADY_EXISTS,
            ErrorKind::RateLimited => ERROR_RATE_LIMITED,
            ErrorKind::IsSameFile => ERROR_IS_SAME_FILE,
            ErrorKind::ConditionNotMatch => ERROR_CONDITION_NOT_MATCH,
            ErrorKind::RangeNotSatisfied => ERROR_RANGE_NOT_SATISFIED,
            _ => ERROR_UNEXPECTED,
        };
        let status = if error.is_temporary() {
            ERROR_STATUS_TEMPORARY
        } else if error.is_persistent() {
            ERROR_STATUS_PERSISTENT
        } else {
            ERROR_STATUS_PERMANENT
        };
        Self::OpenDal {
            kind: kind_code,
            status,
            kind_name: kind.into_static().to_owned(),
            // `message()` excludes operation context that can contain backend
            // configuration values. The Moon facade owns call-site context.
            message: error.message().to_owned(),
        }
    }
}

fn error_snapshot_bytes(error: &BridgeError) -> Result<Vec<u8>, BridgeError> {
    let kind_name = error.kind_name().as_bytes();
    let message = error.diagnostic_message();
    let message = message.as_bytes();
    let kind_name_length =
        u32::try_from(kind_name.len()).map_err(|_| BridgeError::BufferTooLarge)?;
    let message_length = u32::try_from(message.len()).map_err(|_| BridgeError::BufferTooLarge)?;
    let total_length = ERROR_SNAPSHOT_HEADER_LENGTH
        .checked_add(kind_name.len())
        .and_then(|length| length.checked_add(message.len()))
        .filter(|length| *length <= MAX_BUFFER_LENGTH)
        .ok_or(BridgeError::BufferTooLarge)?;
    let mut snapshot = Vec::new();
    snapshot
        .try_reserve_exact(total_length)
        .map_err(|_| BridgeError::AllocationFailed)?;
    snapshot.extend_from_slice(&ERROR_SNAPSHOT_MAGIC);
    snapshot.extend_from_slice(&ERROR_SNAPSHOT_SCHEMA.to_le_bytes());
    snapshot.extend_from_slice(&error.kind().to_le_bytes());
    snapshot.extend_from_slice(&error.error_status().to_le_bytes());
    snapshot.extend_from_slice(&kind_name_length.to_le_bytes());
    snapshot.extend_from_slice(&message_length.to_le_bytes());
    snapshot.extend_from_slice(kind_name);
    snapshot.extend_from_slice(message);
    Ok(snapshot)
}

struct MetadataSnapshotView<'a> {
    present_bits: u64,
    mode: u32,
    is_current: u32,
    is_deleted: u32,
    content_length: u64,
    last_modified_seconds: i64,
    last_modified_nanoseconds: u32,
    strings: [Option<&'a str>; 7],
    lengths: [u32; 7],
    encoded_length: usize,
}

impl<'a> MetadataSnapshotView<'a> {
    fn new(metadata: &'a Metadata) -> Result<Self, BridgeError> {
        let mode = match metadata.mode() {
            opendal::EntryMode::FILE => ENTRY_MODE_FILE,
            opendal::EntryMode::DIR => ENTRY_MODE_DIRECTORY,
            opendal::EntryMode::Unknown => ENTRY_MODE_UNKNOWN,
        };
        let (is_current, current_present) = match metadata.is_current() {
            Some(value) => (u32::from(value), true),
            None => (0, false),
        };
        let (last_modified_seconds, last_modified_nanoseconds, modified_present) =
            match metadata.last_modified() {
                Some(value) => {
                    let value = value.into_inner();
                    let mut seconds = value.as_second();
                    let subseconds = value.subsec_nanosecond();
                    let nanoseconds = if subseconds < 0 {
                        seconds = seconds.checked_sub(1).ok_or(BridgeError::LengthOverflow)?;
                        u32::try_from(1_000_000_000_i32 + subseconds)
                            .map_err(|_| BridgeError::LengthOverflow)?
                    } else {
                        u32::try_from(subseconds).map_err(|_| BridgeError::LengthOverflow)?
                    };
                    (seconds, nanoseconds, true)
                }
                None => (0, 0, false),
            };
        let strings = [
            metadata.cache_control(),
            metadata.content_disposition(),
            metadata.content_encoding(),
            metadata.content_md5(),
            metadata.content_type(),
            metadata.etag(),
            metadata.version(),
        ];
        let mut present_bits = 0;
        if current_present {
            present_bits |= METADATA_IS_CURRENT_PRESENT;
        }
        if modified_present {
            present_bits |= METADATA_LAST_MODIFIED_PRESENT;
        }
        let string_bits = [
            METADATA_CACHE_CONTROL_PRESENT,
            METADATA_CONTENT_DISPOSITION_PRESENT,
            METADATA_CONTENT_ENCODING_PRESENT,
            METADATA_CONTENT_MD5_PRESENT,
            METADATA_CONTENT_TYPE_PRESENT,
            METADATA_ETAG_PRESENT,
            METADATA_VERSION_PRESENT,
        ];
        let mut lengths = [0; 7];
        let mut encoded_length = METADATA_SNAPSHOT_HEADER_LENGTH;
        for (index, value) in strings.iter().enumerate() {
            if let Some(value) = value {
                present_bits |= string_bits[index];
                lengths[index] =
                    u32::try_from(value.len()).map_err(|_| BridgeError::BufferTooLarge)?;
                encoded_length = encoded_length
                    .checked_add(value.len())
                    .filter(|length| *length <= MAX_BUFFER_LENGTH)
                    .ok_or(BridgeError::BufferTooLarge)?;
            }
        }
        Ok(Self {
            present_bits,
            mode,
            is_current,
            is_deleted: u32::from(metadata.is_deleted()),
            content_length: metadata.content_length(),
            last_modified_seconds,
            last_modified_nanoseconds,
            strings,
            lengths,
            encoded_length,
        })
    }

    fn encode(&self) -> Result<Vec<u8>, BridgeError> {
        let mut snapshot = Vec::new();
        snapshot
            .try_reserve_exact(self.encoded_length)
            .map_err(|_| BridgeError::AllocationFailed)?;
        snapshot.extend_from_slice(&METADATA_SNAPSHOT_MAGIC);
        snapshot.extend_from_slice(&METADATA_SNAPSHOT_SCHEMA.to_le_bytes());
        snapshot.extend_from_slice(&self.present_bits.to_le_bytes());
        snapshot.extend_from_slice(&self.mode.to_le_bytes());
        snapshot.extend_from_slice(&self.is_current.to_le_bytes());
        snapshot.extend_from_slice(&self.is_deleted.to_le_bytes());
        snapshot.extend_from_slice(&0_u32.to_le_bytes());
        snapshot.extend_from_slice(&self.content_length.to_le_bytes());
        snapshot.extend_from_slice(&self.last_modified_seconds.to_le_bytes());
        snapshot.extend_from_slice(&self.last_modified_nanoseconds.to_le_bytes());
        snapshot.extend_from_slice(&0_u32.to_le_bytes());
        for length in self.lengths {
            snapshot.extend_from_slice(&length.to_le_bytes());
        }
        debug_assert_eq!(snapshot.len(), METADATA_SNAPSHOT_HEADER_LENGTH);
        for value in self.strings.iter().flatten() {
            snapshot.extend_from_slice(value.as_bytes());
        }
        debug_assert_eq!(snapshot.len(), self.encoded_length);
        Ok(snapshot)
    }
}

fn metadata_snapshot_bytes(metadata: &Metadata) -> Result<Vec<u8>, BridgeError> {
    MetadataSnapshotView::new(metadata)?.encode()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Buffer,
    OperatorBuilder,
    Operator,
    Metadata,
    Error,
    Task,
    Completion,
    EntryList,
    ReadStream,
    Writer,
    Lister,
}

impl ResourceKind {
    fn name(self) -> &'static str {
        match self {
            Self::Buffer => "buffer",
            Self::OperatorBuilder => "operator builder",
            Self::Operator => "operator",
            Self::Metadata => "metadata",
            Self::Error => "error",
            Self::Task => "task",
            Self::Completion => "completion",
            Self::EntryList => "entry list",
            Self::ReadStream => "read stream",
            Self::Writer => "writer",
            Self::Lister => "lister",
        }
    }
}

struct OperatorBuilder {
    scheme: String,
    config: Vec<(String, String)>,
    config_bytes: usize,
}

struct EntrySnapshot {
    path: String,
    name: String,
    mode: u32,
    content_length: u64,
    metadata_snapshot: Vec<u8>,
}

struct ReadStreamCursor {
    stream: BufferStream,
    pending: Option<Buffer>,
    max_chunk: usize,
}

enum ReadStreamState {
    Open(Box<ReadStreamCursor>, u64),
    Busy(u64),
    End,
    Failed,
}

enum WriterState {
    Open(Box<Writer>, u64),
    Busy(u64),
    Finished(Box<Metadata>),
    Aborted,
    Failed,
}

enum ListerState {
    Open(Box<Lister>, u64),
    Busy(u64),
    End,
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ResourceLease {
    handle: u32,
    kind: ResourceKind,
    // A handle generation identifies the resource, while this token identifies
    // one operation on that resource. An older ready task must not be able to
    // terminalize a newer in-flight operation on the same handle.
    operation: u64,
}

#[derive(Clone, Copy)]
struct ListBounds {
    max_entries: usize,
    max_bytes: usize,
}

const PRODUCTION_LIST_BOUNDS: ListBounds = ListBounds {
    max_entries: MAX_LIST_ENTRIES,
    max_bytes: MAX_LIST_OUTPUT_BYTES,
};

enum Resource {
    Buffer(Vec<u8>),
    OperatorBuilder(OperatorBuilder),
    Operator(Operator),
    Metadata(Metadata),
    Error(BridgeError),
    Task(Task),
    Completion(Completion),
    EntryList(Vec<EntrySnapshot>),
    ReadStream(ReadStreamState),
    Writer(WriterState),
    Lister(ListerState),
}

impl Resource {
    fn kind(&self) -> ResourceKind {
        match self {
            Self::Buffer(_) => ResourceKind::Buffer,
            Self::OperatorBuilder(_) => ResourceKind::OperatorBuilder,
            Self::Operator(_) => ResourceKind::Operator,
            Self::Metadata(_) => ResourceKind::Metadata,
            Self::Error(_) => ResourceKind::Error,
            Self::Task(_) => ResourceKind::Task,
            Self::Completion(_) => ResourceKind::Completion,
            Self::EntryList(_) => ResourceKind::EntryList,
            Self::ReadStream(_) => ResourceKind::ReadStream,
            Self::Writer(_) => ResourceKind::Writer,
            Self::Lister(_) => ResourceKind::Lister,
        }
    }
}

enum Task {
    Pending(Option<ResourceLease>),
    Ready(Box<Completion>, Option<ResourceLease>),
    Cancelled,
    Consumed,
}

enum Completion {
    Write(Result<Box<Metadata>, BridgeError>),
    Read(Result<Vec<u8>, BridgeError>),
    Stat(Result<Box<Metadata>, BridgeError>),
    CreateDir(Result<(), BridgeError>),
    Delete(Result<(), BridgeError>),
    List(Result<Vec<EntrySnapshot>, BridgeError>),
    Copy(Result<Box<Metadata>, BridgeError>),
    Rename(Result<(), BridgeError>),
    OpenReadStream(Result<Box<ReadStreamCursor>, BridgeError>),
    ReadStreamNext(Result<Option<Vec<u8>>, BridgeError>),
    OpenWriter(Result<Box<Writer>, BridgeError>),
    WriterWrite(Result<(), BridgeError>),
    WriterFinish(Result<Box<Metadata>, BridgeError>),
    WriterAbort(Result<(), BridgeError>),
    OpenLister(Result<Box<Lister>, BridgeError>),
    ListerNext(Result<Option<EntrySnapshot>, BridgeError>),
}

impl Completion {
    fn kind(&self) -> u32 {
        match self {
            Self::Write(_) => COMPLETION_WRITE,
            Self::Read(_) => COMPLETION_READ,
            Self::Stat(_) => COMPLETION_STAT,
            Self::CreateDir(_) => COMPLETION_CREATE_DIR,
            Self::Delete(_) => COMPLETION_DELETE,
            Self::List(_) => COMPLETION_LIST,
            Self::Copy(_) => COMPLETION_COPY,
            Self::Rename(_) => COMPLETION_RENAME,
            Self::OpenReadStream(_) => COMPLETION_OPEN_READER,
            Self::ReadStreamNext(_) => COMPLETION_READER_NEXT,
            Self::OpenWriter(_) => COMPLETION_OPEN_WRITER,
            Self::WriterWrite(_) => COMPLETION_WRITER_WRITE,
            Self::WriterFinish(_) => COMPLETION_WRITER_FINISH,
            Self::WriterAbort(_) => COMPLETION_WRITER_ABORT,
            Self::OpenLister(_) => COMPLETION_OPEN_LISTER,
            Self::ListerNext(_) => COMPLETION_LISTER_NEXT,
        }
    }

    fn error(&self) -> Option<&BridgeError> {
        match self {
            Self::Write(Err(error))
            | Self::Read(Err(error))
            | Self::Stat(Err(error))
            | Self::CreateDir(Err(error))
            | Self::Delete(Err(error))
            | Self::List(Err(error)) => Some(error),
            Self::Copy(Err(error))
            | Self::Rename(Err(error))
            | Self::OpenReadStream(Err(error))
            | Self::ReadStreamNext(Err(error))
            | Self::OpenWriter(Err(error))
            | Self::WriterWrite(Err(error))
            | Self::WriterFinish(Err(error))
            | Self::WriterAbort(Err(error))
            | Self::OpenLister(Err(error))
            | Self::ListerNext(Err(error)) => Some(error),
            Self::Write(Ok(_))
            | Self::Read(Ok(_))
            | Self::Stat(Ok(_))
            | Self::CreateDir(Ok(()))
            | Self::Delete(Ok(()))
            | Self::List(Ok(_)) => None,
            Self::Copy(Ok(_))
            | Self::Rename(Ok(()))
            | Self::OpenReadStream(Ok(_))
            | Self::ReadStreamNext(Ok(_))
            | Self::OpenWriter(Ok(_))
            | Self::WriterWrite(Ok(()))
            | Self::WriterFinish(Ok(_))
            | Self::WriterAbort(Ok(()))
            | Self::OpenLister(Ok(_))
            | Self::ListerNext(Ok(_)) => None,
        }
    }
}

struct Slot {
    generation: u16,
    resource: Option<Resource>,
}

#[derive(Default)]
struct Arena {
    slots: Vec<Slot>,
    live: u32,
}

impl Arena {
    fn can_insert(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.resource.is_none() && slot.generation != 0)
            || self.slots.len() < MAX_SLOTS
    }

    fn insert(&mut self, resource: Resource) -> Result<u32, BridgeError> {
        if let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.resource.is_none() && slot.generation != 0)
        {
            slot.resource = Some(resource);
            self.live += 1;
            return Ok(encode_handle(index, slot.generation));
        }

        if self.slots.len() == MAX_SLOTS {
            return Err(BridgeError::HandleLimit);
        }

        let index = self.slots.len();
        self.slots.push(Slot {
            generation: 1,
            resource: Some(resource),
        });
        self.live += 1;
        Ok(encode_handle(index, 1))
    }

    fn resource(&self, handle: u32) -> Result<&Resource, BridgeError> {
        let (index, generation) = decode_handle(handle)?;
        let slot = self
            .slots
            .get(index)
            .ok_or(BridgeError::InvalidHandle { handle })?;
        if slot.generation != generation {
            return Err(BridgeError::InvalidHandle { handle });
        }
        slot.resource
            .as_ref()
            .ok_or(BridgeError::InvalidHandle { handle })
    }

    fn resource_mut(&mut self, handle: u32) -> Result<&mut Resource, BridgeError> {
        let (index, generation) = decode_handle(handle)?;
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(BridgeError::InvalidHandle { handle })?;
        if slot.generation != generation {
            return Err(BridgeError::InvalidHandle { handle });
        }
        slot.resource
            .as_mut()
            .ok_or(BridgeError::InvalidHandle { handle })
    }

    fn buffer(&self, handle: u32) -> Result<&[u8], BridgeError> {
        match self.resource(handle)? {
            Resource::Buffer(buffer) => Ok(buffer),
            resource => Err(wrong_resource_type(handle, ResourceKind::Buffer, resource)),
        }
    }

    fn buffer_mut(&mut self, handle: u32) -> Result<&mut Vec<u8>, BridgeError> {
        match self.resource_mut(handle)? {
            Resource::Buffer(buffer) => Ok(buffer),
            resource => Err(wrong_resource_type(handle, ResourceKind::Buffer, resource)),
        }
    }

    fn operator_builder(&self, handle: u32) -> Result<&OperatorBuilder, BridgeError> {
        match self.resource(handle)? {
            Resource::OperatorBuilder(builder) => Ok(builder),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::OperatorBuilder,
                resource,
            )),
        }
    }

    fn operator_builder_mut(&mut self, handle: u32) -> Result<&mut OperatorBuilder, BridgeError> {
        match self.resource_mut(handle)? {
            Resource::OperatorBuilder(builder) => Ok(builder),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::OperatorBuilder,
                resource,
            )),
        }
    }

    fn operator(&self, handle: u32) -> Result<&Operator, BridgeError> {
        match self.resource(handle)? {
            Resource::Operator(operator) => Ok(operator),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::Operator,
                resource,
            )),
        }
    }

    fn metadata(&self, handle: u32) -> Result<&Metadata, BridgeError> {
        match self.resource(handle)? {
            Resource::Metadata(metadata) => Ok(metadata),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::Metadata,
                resource,
            )),
        }
    }

    fn error(&self, handle: u32) -> Result<&BridgeError, BridgeError> {
        match self.resource(handle)? {
            Resource::Error(error) => Ok(error),
            resource => Err(wrong_resource_type(handle, ResourceKind::Error, resource)),
        }
    }

    fn task(&self, handle: u32) -> Result<&Task, BridgeError> {
        match self.resource(handle)? {
            Resource::Task(task) => Ok(task),
            resource => Err(wrong_resource_type(handle, ResourceKind::Task, resource)),
        }
    }

    fn task_mut(&mut self, handle: u32) -> Result<&mut Task, BridgeError> {
        match self.resource_mut(handle)? {
            Resource::Task(task) => Ok(task),
            resource => Err(wrong_resource_type(handle, ResourceKind::Task, resource)),
        }
    }

    fn completion(&self, handle: u32) -> Result<&Completion, BridgeError> {
        match self.resource(handle)? {
            Resource::Completion(completion) => Ok(completion),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::Completion,
                resource,
            )),
        }
    }

    fn entry_list(&self, handle: u32) -> Result<&[EntrySnapshot], BridgeError> {
        match self.resource(handle)? {
            Resource::EntryList(entries) => Ok(entries),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::EntryList,
                resource,
            )),
        }
    }

    fn entry_list_mut(&mut self, handle: u32) -> Result<&mut [EntrySnapshot], BridgeError> {
        match self.resource_mut(handle)? {
            Resource::EntryList(entries) => Ok(entries),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::EntryList,
                resource,
            )),
        }
    }

    fn read_stream_mut(&mut self, handle: u32) -> Result<&mut ReadStreamState, BridgeError> {
        match self.resource_mut(handle)? {
            Resource::ReadStream(stream) => Ok(stream),
            resource => Err(wrong_resource_type(
                handle,
                ResourceKind::ReadStream,
                resource,
            )),
        }
    }

    fn writer_mut(&mut self, handle: u32) -> Result<&mut WriterState, BridgeError> {
        match self.resource_mut(handle)? {
            Resource::Writer(writer) => Ok(writer),
            resource => Err(wrong_resource_type(handle, ResourceKind::Writer, resource)),
        }
    }

    fn lister_mut(&mut self, handle: u32) -> Result<&mut ListerState, BridgeError> {
        match self.resource_mut(handle)? {
            Resource::Lister(lister) => Ok(lister),
            resource => Err(wrong_resource_type(handle, ResourceKind::Lister, resource)),
        }
    }

    fn ensure_insert_capacity_after_take(
        &self,
        handle: u32,
        expected: ResourceKind,
    ) -> Result<(), BridgeError> {
        let (index, generation) = decode_handle(handle)?;
        let resource = self
            .slots
            .get(index)
            .and_then(|slot| {
                (slot.generation == generation)
                    .then_some(slot)
                    .and_then(|slot| slot.resource.as_ref())
            })
            .ok_or(BridgeError::InvalidHandle { handle })?;
        if resource.kind() != expected {
            return Err(wrong_resource_type(handle, expected, resource));
        }
        if generation == MAX_GENERATION && !self.can_insert() {
            return Err(BridgeError::HandleLimit);
        }
        Ok(())
    }

    fn release(&mut self, handle: u32, expected: ResourceKind) -> Result<(), BridgeError> {
        self.take(handle, expected).map(drop)
    }

    fn take(&mut self, handle: u32, expected: ResourceKind) -> Result<Resource, BridgeError> {
        let (index, generation) = decode_handle(handle)?;
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(BridgeError::InvalidHandle { handle })?;
        if slot.generation != generation {
            return Err(BridgeError::InvalidHandle { handle });
        }

        let resource = slot
            .resource
            .as_ref()
            .ok_or(BridgeError::InvalidHandle { handle })?;
        if resource.kind() != expected {
            return Err(wrong_resource_type(handle, expected, resource));
        }

        let resource = slot.resource.take().expect("resource checked above");
        slot.generation = next_generation(slot.generation);
        self.live -= 1;
        Ok(resource)
    }

    fn clear(&mut self) {
        self.slots.clear();
        self.live = 0;
    }
}

#[derive(Default)]
struct State {
    arena: Arena,
    last_error: Option<BridgeError>,
    next_resource_operation: u64,
    forced_pending_poll_count: u32,
    force_pending_for_canary: bool,
    torn_down: bool,
}

fn allocate_resource_operation(state: &mut State) -> Result<u64, BridgeError> {
    // Never wrap: reusing a token could make a very old task authoritative.
    let operation = state
        .next_resource_operation
        .checked_add(1)
        .ok_or(BridgeError::LengthOverflow)?;
    state.next_resource_operation = operation;
    Ok(operation)
}

thread_local! {
    // Core Wasm without threads has one execution thread. Keeping the arena
    // thread-local avoids exposing a lock or pointer through the scalar ABI.
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn encode_handle(index: usize, generation: u16) -> u32 {
    (u32::from(generation) << 16) | (index as u32 + 1)
}

fn decode_handle(handle: u32) -> Result<(usize, u16), BridgeError> {
    let slot = (handle & u32::from(u16::MAX)) as u16;
    let generation = (handle >> 16) as u16;
    if slot == 0 || generation == 0 {
        return Err(BridgeError::InvalidHandle { handle });
    }
    Ok((usize::from(slot - 1), generation))
}

fn next_generation(generation: u16) -> u16 {
    if generation == MAX_GENERATION {
        // Retire the slot instead of allowing an old handle to become valid
        // again after generation wraparound.
        0
    } else {
        generation + 1
    }
}

fn wrong_resource_type(handle: u32, expected: ResourceKind, actual: &Resource) -> BridgeError {
    BridgeError::WrongResourceType {
        handle,
        expected: expected.name(),
        actual: actual.kind().name(),
    }
}

fn record_error(error: BridgeError) -> u32 {
    let code = error.code();
    STATE.with(|state| state.borrow_mut().last_error = Some(error));
    code
}

fn status(result: Result<(), BridgeError>) -> u32 {
    match result {
        Ok(()) => STATUS_OK,
        Err(error) => record_error(error),
    }
}

struct ForcePending<F> {
    inner: Pin<Box<F>>,
    delay: gloo_timers::future::TimeoutFuture,
    pending_seen: bool,
}

impl<F> ForcePending<F> {
    fn new(inner: F) -> Self {
        Self {
            inner: Box::pin(inner),
            delay: gloo_timers::future::TimeoutFuture::new(0),
            pending_seen: false,
        }
    }
}

impl<F: Future> Future for ForcePending<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.delay.poll_unpin(context).is_pending() {
            if !self.pending_seen {
                self.pending_seen = true;
                STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.forced_pending_poll_count =
                        state.forced_pending_poll_count.saturating_add(1);
                });
            }
            return Poll::Pending;
        }
        self.inner.as_mut().poll(context)
    }
}

trait PollUnpin: Future + Unpin {
    fn poll_unpin(&mut self, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self).poll(context)
    }
}

impl<F: Future + Unpin> PollUnpin for F {}

fn poll_once<F>(future: F) -> Result<F::Output, BridgeError>
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(BridgeError::AsyncPending),
    }
}

fn start_task(future: impl Future<Output = Completion> + 'static) -> Result<u32, BridgeError> {
    let handle = insert_resource(Resource::Task(Task::Pending(None)))?;
    let force_pending = STATE.with(|state| state.borrow().force_pending_for_canary);
    wasm_bindgen_futures::spawn_local(async move {
        let completion = if force_pending {
            ForcePending::new(future).await
        } else {
            future.await
        };
        publish_task(handle, completion);
    });
    Ok(handle)
}

fn publish_task(handle: u32, completion: Completion) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let Ok(task) = state.arena.task_mut(handle)
            && matches!(task, Task::Pending(None))
        {
            *task = Task::Ready(Box::new(completion), None);
        }
    });
}

enum ResourceUpdate {
    ReadStream(ReadStreamUpdate),
    Writer(WriterUpdate),
    Lister(ListerUpdate),
}

enum ReadStreamUpdate {
    Open(Box<ReadStreamCursor>),
    End,
    Failed,
}

enum WriterUpdate {
    Open(Box<Writer>),
    Finished(Box<Metadata>),
    Aborted,
    Failed,
}

enum ListerUpdate {
    Open(Box<Lister>),
    End,
    Failed,
}

impl ResourceUpdate {
    fn kind(&self) -> ResourceKind {
        match self {
            Self::ReadStream(_) => ResourceKind::ReadStream,
            Self::Writer(_) => ResourceKind::Writer,
            Self::Lister(_) => ResourceKind::Lister,
        }
    }
}

fn spawn_resource_task(
    handle: u32,
    lease: ResourceLease,
    future: impl Future<Output = (ResourceUpdate, Completion)> + 'static,
) {
    let force_pending = STATE.with(|state| state.borrow().force_pending_for_canary);
    wasm_bindgen_futures::spawn_local(async move {
        let (update, completion) = if force_pending {
            ForcePending::new(future).await
        } else {
            future.await
        };
        publish_resource_task(handle, lease, update, completion);
    });
}

fn publish_resource_task(
    task_handle: u32,
    lease: ResourceLease,
    update: ResourceUpdate,
    completion: Completion,
) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let task_is_pending = matches!(
            state.arena.task(task_handle),
            Ok(Task::Pending(Some(current))) if *current == lease
        );
        if !task_is_pending || update.kind() != lease.kind {
            return;
        }

        match update {
            ResourceUpdate::ReadStream(update) => {
                if let Ok(current) = state.arena.read_stream_mut(lease.handle)
                    && matches!(current, ReadStreamState::Busy(operation) if *operation == lease.operation)
                {
                    *current = match update {
                        ReadStreamUpdate::Open(cursor) => {
                            ReadStreamState::Open(cursor, lease.operation)
                        }
                        ReadStreamUpdate::End => ReadStreamState::End,
                        ReadStreamUpdate::Failed => ReadStreamState::Failed,
                    };
                }
            }
            ResourceUpdate::Writer(update) => {
                if let Ok(current) = state.arena.writer_mut(lease.handle)
                    && matches!(current, WriterState::Busy(operation) if *operation == lease.operation)
                {
                    *current = match update {
                        WriterUpdate::Open(writer) => WriterState::Open(writer, lease.operation),
                        WriterUpdate::Finished(metadata) => WriterState::Finished(metadata),
                        WriterUpdate::Aborted => WriterState::Aborted,
                        WriterUpdate::Failed => WriterState::Failed,
                    };
                }
            }
            ResourceUpdate::Lister(update) => {
                if let Ok(current) = state.arena.lister_mut(lease.handle)
                    && matches!(current, ListerState::Busy(operation) if *operation == lease.operation)
                {
                    *current = match update {
                        ListerUpdate::Open(lister) => {
                            ListerState::Open(lister, lease.operation)
                        }
                        ListerUpdate::End => ListerState::End,
                        ListerUpdate::Failed => ListerState::Failed,
                    };
                }
            }
        }

        if let Ok(task) = state.arena.task_mut(task_handle)
            && matches!(task, Task::Pending(Some(current)) if *current == lease)
        {
            *task = Task::Ready(Box::new(completion), Some(lease));
        }
    });
}

fn terminalize_lease(arena: &mut Arena, lease: ResourceLease) {
    match lease.kind {
        ResourceKind::ReadStream => {
            if let Ok(stream) = arena.read_stream_mut(lease.handle)
                && matches!(
                    stream,
                    ReadStreamState::Open(_, operation) | ReadStreamState::Busy(operation)
                        if *operation == lease.operation
                )
            {
                *stream = ReadStreamState::Failed;
            }
        }
        ResourceKind::Writer => {
            if let Ok(writer) = arena.writer_mut(lease.handle)
                && matches!(
                    writer,
                    WriterState::Open(_, operation) | WriterState::Busy(operation)
                        if *operation == lease.operation
                )
            {
                *writer = WriterState::Failed;
            }
        }
        ResourceKind::Lister => {
            if let Ok(lister) = arena.lister_mut(lease.handle)
                && matches!(
                    lister,
                    ListerState::Open(_, operation) | ListerState::Busy(operation)
                        if *operation == lease.operation
                )
            {
                *lister = ListerState::Failed;
            }
        }
        _ => {}
    }
}

fn task_lease(task: &Task) -> Option<ResourceLease> {
    match task {
        Task::Pending(lease) | Task::Ready(_, lease) => *lease,
        Task::Cancelled | Task::Consumed => None,
    }
}

fn path_and_operator(
    operator_handle: u32,
    path_handle: u32,
) -> Result<(Operator, String), BridgeError> {
    STATE.with(|state| {
        let state = state.borrow();
        let operator = state.arena.operator(operator_handle)?.clone();
        let path = std::str::from_utf8(state.arena.buffer(path_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)?;
        Ok((operator, path))
    })
}

fn owned_utf8_buffer(handle: u32) -> Result<String, BridgeError> {
    STATE.with(|state| {
        let state = state.borrow();
        std::str::from_utf8(state.arena.buffer(handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)
    })
}

fn optional_owned_utf8_buffer(handle: u32) -> Result<Option<String>, BridgeError> {
    if handle == 0 {
        Ok(None)
    } else {
        owned_utf8_buffer(handle).map(Some)
    }
}

fn scalar_bool(value: u32, name: &'static str) -> Result<bool, BridgeError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(BridgeError::InvalidArgument {
            message: format!("{name} must be 0 or 1, found {value}"),
        }),
    }
}

fn optional_list_limit(has_limit: u32, limit: u64) -> Result<Option<usize>, BridgeError> {
    if !scalar_bool(has_limit, "has_limit")? {
        return Ok(None);
    }
    let limit = u32::try_from(limit).map_err(|_| BridgeError::LengthOverflow)?;
    Ok(Some(limit as usize))
}

fn byte_range(kind: u32, offset: u64, length: u64) -> Result<BytesRange, BridgeError> {
    match kind {
        RANGE_FULL if offset == 0 && length == 0 => Ok(BytesRange::default()),
        RANGE_FROM if length == 0 => Ok(BytesRange::new(offset, None)),
        RANGE_OFFSET_LENGTH if offset.checked_add(length).is_some() => {
            Ok(BytesRange::new(offset, Some(length)))
        }
        RANGE_SUFFIX if offset == 0 => Ok(BytesRange::suffix(length)),
        _ => Err(BridgeError::InvalidArgument {
            message: "invalid byte-range scalar encoding".to_owned(),
        }),
    }
}

fn read_options_from_scalars(
    range_kind: u32,
    range_offset: u64,
    range_length: u64,
    version_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> Result<options::ReadOptions, BridgeError> {
    Ok(options::ReadOptions {
        range: byte_range(range_kind, range_offset, range_length)?,
        version: optional_owned_utf8_buffer(version_handle)?,
        if_match: optional_owned_utf8_buffer(if_match_handle)?,
        if_none_match: optional_owned_utf8_buffer(if_none_match_handle)?,
        ..Default::default()
    })
}

fn stat_options_from_scalars(
    version_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> Result<options::StatOptions, BridgeError> {
    Ok(options::StatOptions {
        version: optional_owned_utf8_buffer(version_handle)?,
        if_match: optional_owned_utf8_buffer(if_match_handle)?,
        if_none_match: optional_owned_utf8_buffer(if_none_match_handle)?,
        ..Default::default()
    })
}

fn write_options_from_scalars(
    append: u32,
    content_type_handle: u32,
    content_disposition_handle: u32,
    content_encoding_handle: u32,
    cache_control_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> Result<options::WriteOptions, BridgeError> {
    Ok(options::WriteOptions {
        append: scalar_bool(append, "append")?,
        content_type: optional_owned_utf8_buffer(content_type_handle)?,
        content_disposition: optional_owned_utf8_buffer(content_disposition_handle)?,
        content_encoding: optional_owned_utf8_buffer(content_encoding_handle)?,
        cache_control: optional_owned_utf8_buffer(cache_control_handle)?,
        if_match: optional_owned_utf8_buffer(if_match_handle)?,
        if_none_match: optional_owned_utf8_buffer(if_none_match_handle)?,
        ..Default::default()
    })
}

fn read_stream_options_from_scalars(
    chunk_size: u32,
    range_kind: u32,
    range_offset: u64,
    range_length: u64,
    version_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> Result<(options::ReaderOptions, BytesRange, usize), BridgeError> {
    let chunk_size = usize::try_from(chunk_size).map_err(|_| BridgeError::LengthOverflow)?;
    if !(1..=MAX_TRANSFER_CHUNK).contains(&chunk_size) {
        return Err(BridgeError::InvalidArgument {
            message: format!(
                "read stream chunk size must be 1..={MAX_TRANSFER_CHUNK}, found {chunk_size}"
            ),
        });
    }
    Ok((
        options::ReaderOptions {
            version: optional_owned_utf8_buffer(version_handle)?,
            if_match: optional_owned_utf8_buffer(if_match_handle)?,
            if_none_match: optional_owned_utf8_buffer(if_none_match_handle)?,
            chunk: Some(chunk_size),
            ..Default::default()
        },
        byte_range(range_kind, range_offset, range_length)?,
        chunk_size,
    ))
}

fn ensure_suffix_is_native(operator: &Operator, range: &BytesRange) -> Result<(), BridgeError> {
    if range.is_suffix() && !operator.base_service().capability_dyn().read_with_suffix {
        return Err(BridgeError::from(opendal::Error::new(
            ErrorKind::Unsupported,
            "suffix reads require native backend range support",
        )));
    }
    Ok(())
}

fn start_read_task(
    operator_handle: u32,
    path_handle: u32,
    read_options: options::ReadOptions,
) -> Result<u32, BridgeError> {
    let (operator, path) = path_and_operator(operator_handle, path_handle)?;
    ensure_suffix_is_native(&operator, &read_options.range)?;
    start_task(async move {
        Completion::Read(match operator.read_options(&path, read_options).await {
            Ok(buffer) => try_owned_opendal_buffer(buffer),
            Err(error) => Err(BridgeError::from(error)),
        })
    })
}

fn start_stat_task(
    operator_handle: u32,
    path_handle: u32,
    stat_options: options::StatOptions,
) -> Result<u32, BridgeError> {
    let (operator, path) = path_and_operator(operator_handle, path_handle)?;
    start_task(async move {
        Completion::Stat(
            operator
                .stat_options(&path, stat_options)
                .await
                .map(Box::new)
                .map_err(BridgeError::from),
        )
    })
}

fn start_write_task(
    operator_handle: u32,
    path_handle: u32,
    data_handle: u32,
    write_options: options::WriteOptions,
) -> Result<u32, BridgeError> {
    let (operator, path, data) = STATE.with(|state| -> Result<_, BridgeError> {
        let state = state.borrow();
        let operator = state.arena.operator(operator_handle)?.clone();
        let path = std::str::from_utf8(state.arena.buffer(path_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)?;
        let data = try_owned_bytes(state.arena.buffer(data_handle)?)?;
        Ok((operator, path, data))
    })?;
    start_task(async move {
        Completion::Write(
            operator
                .write_options(&path, data, write_options)
                .await
                .map(Box::new)
                .map_err(BridgeError::from),
        )
    })
}

fn start_copy_task(
    operator_handle: u32,
    from_handle: u32,
    to_handle: u32,
) -> Result<u32, BridgeError> {
    let (operator, from, to) = STATE.with(|state| -> Result<_, BridgeError> {
        let state = state.borrow();
        let operator = state.arena.operator(operator_handle)?.clone();
        let from = std::str::from_utf8(state.arena.buffer(from_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)?;
        let to = std::str::from_utf8(state.arena.buffer(to_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)?;
        Ok((operator, from, to))
    })?;
    start_task(async move {
        Completion::Copy(
            operator
                .copy(&from, &to)
                .await
                .map(Box::new)
                .map_err(BridgeError::from),
        )
    })
}

fn start_rename_task(
    operator_handle: u32,
    from_handle: u32,
    to_handle: u32,
) -> Result<u32, BridgeError> {
    let (operator, from, to) = STATE.with(|state| -> Result<_, BridgeError> {
        let state = state.borrow();
        let operator = state.arena.operator(operator_handle)?.clone();
        let from = std::str::from_utf8(state.arena.buffer(from_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)?;
        let to = std::str::from_utf8(state.arena.buffer(to_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)?;
        Ok((operator, from, to))
    })?;
    start_task(async move {
        Completion::Rename(operator.rename(&from, &to).await.map_err(BridgeError::from))
    })
}

fn copy_stream_chunk(
    buffer: Buffer,
    max_chunk: usize,
) -> Result<(Vec<u8>, Option<Buffer>), BridgeError> {
    let mut buffer = buffer;
    let length = buffer.len().min(max_chunk);
    let chunk = buffer.split_to(length);
    let remainder = (!buffer.is_empty()).then_some(buffer);
    try_owned_opendal_buffer(chunk).map(|chunk| (chunk, remainder))
}

async fn read_stream_next(mut cursor: Box<ReadStreamCursor>) -> (ResourceUpdate, Completion) {
    let next = match cursor.pending.take() {
        Some(buffer) => Ok(Some(buffer)),
        None => cursor.stream.try_next().await.map_err(BridgeError::from),
    };
    match next {
        Ok(Some(buffer)) => match copy_stream_chunk(buffer, cursor.max_chunk) {
            Ok((chunk, remainder)) => {
                cursor.pending = remainder;
                (
                    ResourceUpdate::ReadStream(ReadStreamUpdate::Open(cursor)),
                    Completion::ReadStreamNext(Ok(Some(chunk))),
                )
            }
            Err(error) => (
                ResourceUpdate::ReadStream(ReadStreamUpdate::Failed),
                Completion::ReadStreamNext(Err(error)),
            ),
        },
        Ok(None) => (
            ResourceUpdate::ReadStream(ReadStreamUpdate::End),
            Completion::ReadStreamNext(Ok(None)),
        ),
        Err(error) => (
            ResourceUpdate::ReadStream(ReadStreamUpdate::Failed),
            Completion::ReadStreamNext(Err(error)),
        ),
    }
}

fn reserve_read_stream_next(
    handle: u32,
) -> Result<(u32, ResourceLease, Box<ReadStreamCursor>), BridgeError> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.torn_down {
            return Err(BridgeError::TornDown);
        }
        if !state.arena.can_insert() {
            return Err(BridgeError::HandleLimit);
        }
        let operation = allocate_resource_operation(&mut state)?;
        let stream = state.arena.read_stream_mut(handle)?;
        let (cursor, previous_operation) =
            match std::mem::replace(stream, ReadStreamState::Busy(operation)) {
                ReadStreamState::Open(cursor, previous_operation) => (cursor, previous_operation),
                ReadStreamState::Busy(active_operation) => {
                    *stream = ReadStreamState::Busy(active_operation);
                    return Err(BridgeError::ResourceBusy { handle });
                }
                terminal @ (ReadStreamState::End | ReadStreamState::Failed) => {
                    *stream = terminal;
                    return Err(BridgeError::ResourceClosed { handle });
                }
            };
        let lease = ResourceLease {
            handle,
            kind: ResourceKind::ReadStream,
            operation,
        };
        match state
            .arena
            .insert(Resource::Task(Task::Pending(Some(lease))))
        {
            Ok(task) => Ok((task, lease, cursor)),
            Err(error) => {
                *state.arena.read_stream_mut(handle)? =
                    ReadStreamState::Open(cursor, previous_operation);
                Err(error)
            }
        }
    })
}

fn reserve_writer_call(handle: u32) -> Result<(u32, ResourceLease, Box<Writer>), BridgeError> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.torn_down {
            return Err(BridgeError::TornDown);
        }
        if !state.arena.can_insert() {
            return Err(BridgeError::HandleLimit);
        }
        let operation = allocate_resource_operation(&mut state)?;
        let writer = state.arena.writer_mut(handle)?;
        let (inner, previous_operation) =
            match std::mem::replace(writer, WriterState::Busy(operation)) {
                WriterState::Open(inner, previous_operation) => (inner, previous_operation),
                WriterState::Busy(active_operation) => {
                    *writer = WriterState::Busy(active_operation);
                    return Err(BridgeError::ResourceBusy { handle });
                }
                terminal @ (WriterState::Finished(_)
                | WriterState::Aborted
                | WriterState::Failed) => {
                    *writer = terminal;
                    return Err(BridgeError::ResourceClosed { handle });
                }
            };
        let lease = ResourceLease {
            handle,
            kind: ResourceKind::Writer,
            operation,
        };
        match state
            .arena
            .insert(Resource::Task(Task::Pending(Some(lease))))
        {
            Ok(task) => Ok((task, lease, inner)),
            Err(error) => {
                *state.arena.writer_mut(handle)? = WriterState::Open(inner, previous_operation);
                Err(error)
            }
        }
    })
}

fn reserve_lister_next(handle: u32) -> Result<(u32, ResourceLease, Box<Lister>), BridgeError> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.torn_down {
            return Err(BridgeError::TornDown);
        }
        if !state.arena.can_insert() {
            return Err(BridgeError::HandleLimit);
        }
        let operation = allocate_resource_operation(&mut state)?;
        let lister = state.arena.lister_mut(handle)?;
        let (inner, previous_operation) =
            match std::mem::replace(lister, ListerState::Busy(operation)) {
                ListerState::Open(inner, previous_operation) => (inner, previous_operation),
                ListerState::Busy(active_operation) => {
                    *lister = ListerState::Busy(active_operation);
                    return Err(BridgeError::ResourceBusy { handle });
                }
                terminal @ (ListerState::End | ListerState::Failed) => {
                    *lister = terminal;
                    return Err(BridgeError::ResourceClosed { handle });
                }
            };
        let lease = ResourceLease {
            handle,
            kind: ResourceKind::Lister,
            operation,
        };
        match state
            .arena
            .insert(Resource::Task(Task::Pending(Some(lease))))
        {
            Ok(task) => Ok((task, lease, inner)),
            Err(error) => {
                *state.arena.lister_mut(handle)? = ListerState::Open(inner, previous_operation);
                Err(error)
            }
        }
    })
}

fn try_owned_string(value: &str) -> Result<String, BridgeError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| BridgeError::AllocationFailed)?;
    owned.push_str(value);
    Ok(owned)
}

fn try_owned_bytes(value: &[u8]) -> Result<Vec<u8>, BridgeError> {
    if value.len() > MAX_BUFFER_LENGTH {
        return Err(BridgeError::BufferTooLarge);
    }
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| BridgeError::AllocationFailed)?;
    owned.extend_from_slice(value);
    Ok(owned)
}

fn try_owned_opendal_buffer(buffer: opendal::Buffer) -> Result<Vec<u8>, BridgeError> {
    let length = buffer.len();
    if length > MAX_BUFFER_LENGTH {
        return Err(BridgeError::BufferTooLarge);
    }
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(length)
        .map_err(|_| BridgeError::AllocationFailed)?;
    for chunk in buffer {
        owned.extend_from_slice(&chunk);
    }
    Ok(owned)
}

fn owned_writer_chunk(handle: u32) -> Result<Vec<u8>, BridgeError> {
    STATE.with(|state| {
        let state = state.borrow();
        let data = state.arena.buffer(handle)?;
        if data.len() > MAX_TRANSFER_CHUNK {
            return Err(BridgeError::BufferTooLarge);
        }
        try_owned_bytes(data)
    })
}

fn checked_buffer_window(
    buffer_length: usize,
    offset: u32,
    length: u32,
) -> Result<std::ops::Range<usize>, BridgeError> {
    let offset = usize::try_from(offset).map_err(|_| BridgeError::LengthOverflow)?;
    let length = usize::try_from(length).map_err(|_| BridgeError::LengthOverflow)?;
    if length == 0 || length > MAX_TRANSFER_CHUNK {
        return Err(BridgeError::InvalidArgument {
            message: format!("transfer length must be 1..={MAX_TRANSFER_CHUNK}, found {length}"),
        });
    }
    let end = offset
        .checked_add(length)
        .ok_or(BridgeError::IndexOutOfBounds {
            index: u32::MAX,
            length: buffer_length,
        })?;
    if end > buffer_length {
        return Err(BridgeError::IndexOutOfBounds {
            index: offset as u32,
            length: buffer_length,
        });
    }
    Ok(offset..end)
}

fn zeroed_buffer(length: u32) -> Result<Vec<u8>, BridgeError> {
    let length = usize::try_from(length).map_err(|_| BridgeError::LengthOverflow)?;
    if length > MAX_BUFFER_LENGTH {
        return Err(BridgeError::BufferTooLarge);
    }
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(length)
        .map_err(|_| BridgeError::AllocationFailed)?;
    buffer.resize(length, 0);
    Ok(buffer)
}

fn push_entry_snapshot(
    entries: &mut Vec<EntrySnapshot>,
    total_bytes: &mut usize,
    entry: EntrySnapshot,
    bounds: ListBounds,
) -> Result<(), BridgeError> {
    let metadata_length = entry.metadata_snapshot.len();
    let next_bytes = total_bytes
        .checked_add(entry.path.len())
        .and_then(|value| value.checked_add(entry.name.len()))
        .and_then(|value| value.checked_add(metadata_length))
        .ok_or(BridgeError::ListTooLarge)?;
    if entries.len() >= bounds.max_entries || next_bytes > bounds.max_bytes {
        return Err(BridgeError::ListTooLarge);
    }
    entries
        .try_reserve(1)
        .map_err(|_| BridgeError::AllocationFailed)?;
    entries.push(entry);
    *total_bytes = next_bytes;
    Ok(())
}

fn entry_snapshot(entry: opendal::Entry) -> Result<EntrySnapshot, BridgeError> {
    let name = try_owned_string(entry.name())?;
    let (path, metadata) = entry.into_parts();
    let mode = match metadata.mode() {
        opendal::EntryMode::FILE => ENTRY_MODE_FILE,
        opendal::EntryMode::DIR => ENTRY_MODE_DIRECTORY,
        opendal::EntryMode::Unknown => ENTRY_MODE_UNKNOWN,
    };
    let content_length = metadata.content_length();
    let metadata_snapshot = metadata_snapshot_bytes(&metadata)?;
    let _encoded_length = path
        .len()
        .checked_add(name.len())
        .and_then(|length| length.checked_add(metadata_snapshot.len()))
        .filter(|length| *length <= MAX_LIST_OUTPUT_BYTES)
        .ok_or(BridgeError::ListTooLarge)?;
    Ok(EntrySnapshot {
        path,
        name,
        mode,
        content_length,
        metadata_snapshot,
    })
}

async fn collect_list(
    operator: Operator,
    path: String,
    recursive: bool,
    limit: Option<usize>,
    start_after: Option<String>,
) -> Result<Vec<EntrySnapshot>, BridgeError> {
    let mut request = operator.lister_with(&path).recursive(recursive);
    if let Some(limit) = limit {
        request = request.limit(limit);
    }
    if let Some(start_after) = &start_after {
        request = request.start_after(start_after);
    }
    let mut lister = request.await.map_err(BridgeError::from)?;
    let mut entries = Vec::new();
    let mut total_bytes = 0;
    while let Some(entry) = lister.try_next().await.map_err(BridgeError::from)? {
        push_entry_snapshot(
            &mut entries,
            &mut total_bytes,
            entry_snapshot(entry)?,
            PRODUCTION_LIST_BOUNDS,
        )?;
    }
    Ok(entries)
}

fn insert_buffer(buffer: impl Into<Vec<u8>>) -> Result<u32, BridgeError> {
    let buffer = buffer.into();
    if buffer.len() > MAX_BUFFER_LENGTH {
        return Err(BridgeError::BufferTooLarge);
    }
    insert_resource(Resource::Buffer(buffer))
}

fn try_clone_operator_builder(builder: &OperatorBuilder) -> Result<OperatorBuilder, BridgeError> {
    let mut config = Vec::new();
    config
        .try_reserve_exact(builder.config.len())
        .map_err(|_| BridgeError::AllocationFailed)?;
    for (key, value) in &builder.config {
        config.push((try_owned_string(key)?, try_owned_string(value)?));
    }
    Ok(OperatorBuilder {
        scheme: try_owned_string(&builder.scheme)?,
        config,
        config_bytes: builder.config_bytes,
    })
}

fn push_operator_config(
    builder: &mut OperatorBuilder,
    key: String,
    value: String,
) -> Result<(), BridgeError> {
    if builder
        .config
        .iter()
        .any(|(existing, _)| existing.eq_ignore_ascii_case(&key))
    {
        return Err(BridgeError::InvalidArgument {
            message: "duplicate config key".to_owned(),
        });
    }
    if builder.config.len() >= MAX_CONFIG_ENTRIES {
        return Err(BridgeError::InvalidArgument {
            message: format!("config exceeds the {MAX_CONFIG_ENTRIES}-entry limit"),
        });
    }
    let config_bytes = builder
        .config_bytes
        .checked_add(key.len())
        .and_then(|length| length.checked_add(value.len()))
        .filter(|length| *length <= MAX_CONFIG_BYTES)
        .ok_or_else(|| BridgeError::InvalidArgument {
            message: format!("config exceeds the {MAX_CONFIG_BYTES}-byte limit"),
        })?;
    builder
        .config
        .try_reserve(1)
        .map_err(|_| BridgeError::AllocationFailed)?;
    builder.config.push((key, value));
    builder.config_bytes = config_bytes;
    Ok(())
}

fn build_operator(builder: OperatorBuilder) -> Result<Operator, BridgeError> {
    opendal::install_default();
    Operator::via_iter(&builder.scheme, builder.config)
        .map_err(BridgeError::from_construction_error)
}

fn capability_word(operator: &Operator, word: u32) -> Result<u64, BridgeError> {
    let capability = operator.info().capability();
    let native_read_with_suffix = operator.base_service().capability_dyn().read_with_suffix;
    match word {
        0 => {
            let mut value = 0;
            value |= u64::from(capability.stat) * CAP_STAT;
            value |= u64::from(capability.read) * CAP_READ;
            value |= u64::from(capability.write) * CAP_WRITE;
            value |= u64::from(capability.create_dir) * CAP_CREATE_DIR;
            value |= u64::from(capability.delete) * CAP_DELETE;
            value |= u64::from(capability.list) * CAP_LIST;
            value |= u64::from(capability.copy) * CAP_COPY;
            value |= u64::from(capability.rename) * CAP_RENAME;
            value |= u64::from(native_read_with_suffix) * CAP_READ_SUFFIX;
            value |= u64::from(capability.write_can_append) * CAP_WRITE_APPEND;
            value |= u64::from(capability.list_with_limit) * CAP_LIST_LIMIT;
            value |= u64::from(capability.list_with_start_after) * CAP_LIST_START_AFTER;
            value |= u64::from(capability.list_with_recursive) * CAP_LIST_RECURSIVE;
            value |= u64::from(capability.presign_stat) * CAP_PRESIGN_STAT;
            value |= u64::from(capability.presign_read) * CAP_PRESIGN_READ;
            value |= u64::from(capability.presign_write) * CAP_PRESIGN_WRITE;
            Ok(value)
        }
        1 => capability.delete_max_size.map_or(Ok(0), |value| {
            u64::try_from(value).map_err(|_| BridgeError::LengthOverflow)
        }),
        2 | 3 => Ok(0),
        _ => Err(BridgeError::IndexOutOfBounds {
            index: word,
            length: 4,
        }),
    }
}

fn insert_resource(resource: Resource) -> Result<u32, BridgeError> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.torn_down {
            return Err(BridgeError::TornDown);
        }
        state.arena.insert(resource)
    })
}

fn release_resource(handle: u32, kind: ResourceKind) -> u32 {
    let result = STATE.with(|state| state.borrow_mut().arena.release(handle, kind));
    status(result)
}

// Each exported function uses only Wasm scalar parameters/results and contains
// no unsafe block. Edition 2024 requires the `unsafe(no_mangle)` spelling
// because duplicate linker symbols would violate the program's invariants.

/// Returns the packed bridge ABI version (`major << 16 | minor`).
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_abi_version() -> u32 {
    ABI_VERSION
}

/// Returns the feature bitset supported by this bridge instance.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_feature_flags() -> u32 {
    FEATURE_MEMORY_SERVICE
        | FEATURE_POLL_ONCE_CANARY
        | FEATURE_GENERATION_HANDLES
        | FEATURE_BINARY_BUFFERS
        | FEATURE_TASK_ABI
        | FEATURE_GENERIC_OPERATOR
        | FEATURE_CORE_MUTATIONS
        | FEATURE_BOUNDED_LIST
        | FEATURE_BULK_TRANSFER
        | FEATURE_STRUCTURED_ERRORS
        | FEATURE_METADATA_OPTIONS
        | FEATURE_COPY_RENAME
        | FEATURE_STATEFUL_READER
        | FEATURE_STATEFUL_WRITER
        | FEATURE_STATEFUL_LISTER
        | FEATURE_EXPLICIT_EOF
}

/// Returns how many forced-delay tasks reached an actual pending poll.
///
/// This export is a bridge-level acceptance probe, not a storage operation.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_canary_forced_pending_poll_count() -> u32 {
    STATE.with(|state| state.borrow().forced_pending_poll_count)
}

/// Enables or disables the forced-delay wrapper for tasks started afterwards.
///
/// This is a deterministic browser-test hook. Production tasks leave it
/// disabled and await the OpenDAL future directly on `spawn_local`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_canary_set_force_pending(enabled: u32) -> u32 {
    let result = scalar_bool(enabled, "force_pending").map(|enabled| {
        STATE.with(|state| state.borrow_mut().force_pending_for_canary = enabled);
    });
    status(result)
}

/// Returns the number of resource handles that have not been released.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_live_handle_count() -> u32 {
    STATE.with(|state| state.borrow().arena.live)
}

/// Tears down this bridge instance and releases every owned resource.
///
/// Teardown is idempotent and permanent. Late task completion is ignored.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_teardown() -> u32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.torn_down = true;
        state.arena.clear();
        state.last_error = None;
    });
    STATUS_OK
}

/// Returns `1` when a released handle is rejected after its slot is reused.
///
/// This export is a bridge-level acceptance probe, not a storage operation.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_canary_stale_handle_rejected() -> u32 {
    let _ = opendal_mbt_wasm_last_error_clear();
    let stale = opendal_mbt_wasm_buffer_new();
    if stale == 0 || opendal_mbt_wasm_buffer_release(stale) != STATUS_OK {
        return 0;
    }
    let current = opendal_mbt_wasm_buffer_new();
    if current == 0 {
        return 0;
    }
    let stale_result = opendal_mbt_wasm_buffer_len(stale);
    let error_code = opendal_mbt_wasm_last_error_code();
    let _ = opendal_mbt_wasm_last_error_clear();
    let released = opendal_mbt_wasm_buffer_release(current) == STATUS_OK;
    u32::from(stale_result == SCALAR_ERROR && error_code == 1 && current != stale && released)
}

/// Allocates an empty byte buffer and returns its handle, or `0` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_buffer_new() -> u32 {
    handle_or_record_error(insert_buffer(Vec::new()))
}

/// Allocates a zeroed byte buffer with a fixed length, or returns `0`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_buffer_new_sized(length: u32) -> u32 {
    handle_or_record_error(zeroed_buffer(length).and_then(insert_buffer))
}

/// Appends one byte and returns a status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_buffer_push(handle: u32, value: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let buffer = state.arena.buffer_mut(handle)?;
        if buffer.len() == MAX_BUFFER_LENGTH {
            return Err(BridgeError::BufferTooLarge);
        }
        let byte = u8::try_from(value).map_err(|_| BridgeError::InvalidByte { value })?;
        buffer.push(byte);
        Ok(())
    });
    status(result)
}

/// Returns a buffer's length, or `-1` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_buffer_len(handle: u32) -> i32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        i32::try_from(state.arena.buffer(handle)?.len()).map_err(|_| BridgeError::LengthOverflow)
    });
    match result {
        Ok(length) => length,
        Err(error) => {
            record_error(error);
            SCALAR_ERROR
        }
    }
}

/// Returns the byte at `index` as `0..=255`, or `-1` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_buffer_get(handle: u32, index: u32) -> i32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        let buffer = state.arena.buffer(handle)?;
        let index_usize = usize::try_from(index).map_err(|_| BridgeError::LengthOverflow)?;
        buffer
            .get(index_usize)
            .copied()
            .map(i32::from)
            .ok_or(BridgeError::IndexOutOfBounds {
                index,
                length: buffer.len(),
            })
    });
    match result {
        Ok(byte) => byte,
        Err(error) => {
            record_error(error);
            SCALAR_ERROR
        }
    }
}

/// Returns a checked mutable buffer window pointer, or `0` on failure.
///
/// The pointer remains valid only until the next bridge call that can mutate
/// or release this buffer. A window is limited to `MAX_TRANSFER_CHUNK` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_buffer_data_ptr(handle: u32, offset: u32, length: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let buffer = state.arena.buffer_mut(handle)?;
        let range = checked_buffer_window(buffer.len(), offset, length)?;
        u32::try_from(buffer[range].as_mut_ptr() as usize).map_err(|_| BridgeError::LengthOverflow)
    });
    handle_or_record_error(result)
}

/// Releases a buffer handle and returns a status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_buffer_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Buffer)
}

/// Creates an OpenDAL memory operator and returns its handle, or `0` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_new_memory() -> u32 {
    let result = Operator::new(Memory::default())
        .map_err(BridgeError::from)
        .and_then(|operator| insert_resource(Resource::Operator(operator)));
    match result {
        Ok(handle) => handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Returns the registered service schemes as a sorted newline-delimited buffer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_registered_schemes() -> u32 {
    opendal::init_default_registry();
    let mut schemes: Vec<_> = OperatorRegistry::get().schemes().into_iter().collect();
    schemes.sort_unstable();
    handle_or_record_error(insert_buffer(schemes.join("\n").into_bytes()))
}

/// Creates a generic operator builder from a UTF-8 service scheme.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_builder_new(scheme_handle: u32) -> u32 {
    let result = owned_utf8_buffer(scheme_handle).and_then(|scheme| {
        if scheme.is_empty() {
            return Err(BridgeError::InvalidArgument {
                message: "service scheme cannot be empty".to_owned(),
            });
        }
        insert_resource(Resource::OperatorBuilder(OperatorBuilder {
            scheme,
            config: Vec::new(),
            config_bytes: 0,
        }))
    });
    handle_or_record_error(result)
}

/// Copies one UTF-8 configuration key/value into a generic operator builder.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_builder_set(
    builder_handle: u32,
    key_handle: u32,
    value_handle: u32,
) -> u32 {
    let result = owned_utf8_buffer(key_handle).and_then(|key| {
        let value = owned_utf8_buffer(value_handle)?;
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            push_operator_config(
                state.arena.operator_builder_mut(builder_handle)?,
                key,
                value,
            )
        })
    });
    status(result)
}

/// Builds an operator through OpenDAL's compiled service registry.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_builder_build(builder_handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        try_clone_operator_builder(state.arena.operator_builder(builder_handle)?)
    });
    handle_or_record_error(result.and_then(|builder| {
        build_operator(builder).and_then(|operator| insert_resource(Resource::Operator(operator)))
    }))
}

/// Releases a generic operator builder.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_builder_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::OperatorBuilder)
}

fn operator_info_buffer(handle: u32, select: impl FnOnce(opendal::OperatorInfo) -> String) -> u32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(select(state.arena.operator(handle)?.info()).into_bytes())
    });
    handle_or_record_error(result.and_then(insert_buffer))
}

/// Copies an operator's registered service scheme into an owned buffer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_info_scheme(handle: u32) -> u32 {
    operator_info_buffer(handle, |info| info.scheme().to_owned())
}

/// Copies an operator's normalized root into an owned buffer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_info_root(handle: u32) -> u32 {
    operator_info_buffer(handle, |info| info.root())
}

/// Copies an operator's namespace name into an owned buffer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_info_name(handle: u32) -> u32 {
    operator_info_buffer(handle, |info| info.name())
}

/// Returns one word of the portable capability snapshot.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_info_capability_word(handle: u32, word: u32) -> u64 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        capability_word(state.arena.operator(handle)?, word)
    });
    match result {
        Ok(value) => value,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Writes a path/data buffer pair through an OpenDAL memory operator.
///
/// The OpenDAL future is polled exactly once. A pending future returns status
/// code `9`; this function is not a general asynchronous executor.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_write(
    operator_handle: u32,
    path_handle: u32,
    data_handle: u32,
) -> u32 {
    let inputs = STATE.with(|state| {
        let state = state.borrow();
        let operator = state.arena.operator(operator_handle)?.clone();
        let path = std::str::from_utf8(state.arena.buffer(path_handle)?)
            .map_err(|_| BridgeError::InvalidUtf8)
            .and_then(try_owned_string)?;
        let data = try_owned_bytes(state.arena.buffer(data_handle)?)?;
        Ok((operator, path, data))
    });

    let result = inputs.and_then(|(operator, path, data)| {
        poll_once(operator.write(&path, data))?.map_err(BridgeError::from)?;
        Ok(())
    });
    status(result)
}

/// Reads a complete object into a new buffer handle, or returns `0` on failure.
///
/// The OpenDAL future is polled exactly once. A pending future records status
/// code `9`; this function is not a general asynchronous executor.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_read(operator_handle: u32, path_handle: u32) -> u32 {
    let result = path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
        let buffer = poll_once(operator.read(&path))?.map_err(BridgeError::from)?;
        try_owned_opendal_buffer(buffer).and_then(insert_buffer)
    });
    match result {
        Ok(handle) => handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Stats an object and returns a metadata handle, or `0` on failure.
///
/// The OpenDAL future is polled exactly once. A pending future records status
/// code `9`; this function is not a general asynchronous executor.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_stat(operator_handle: u32, path_handle: u32) -> u32 {
    let result = path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
        let metadata = poll_once(operator.stat(&path))?.map_err(BridgeError::from)?;
        insert_resource(Resource::Metadata(metadata))
    });
    match result {
        Ok(handle) => handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Starts an asynchronous write and returns an owned task handle.
///
/// Inputs are copied before this function returns. `spawn_local` first polls
/// the OpenDAL future after the initiating call returns; the browser canary
/// can additionally enable a deterministic forced-pending timer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_write_start(
    operator_handle: u32,
    path_handle: u32,
    data_handle: u32,
) -> u32 {
    handle_or_record_error(start_write_task(
        operator_handle,
        path_handle,
        data_handle,
        options::WriteOptions::default(),
    ))
}

/// Starts an asynchronous write with the complete MoonBit v1 option subset.
#[allow(clippy::too_many_arguments, reason = "mirrors the frozen scalar ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_write_options_start_v1(
    operator_handle: u32,
    path_handle: u32,
    data_handle: u32,
    append: u32,
    content_type_handle: u32,
    content_disposition_handle: u32,
    content_encoding_handle: u32,
    cache_control_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> u32 {
    let write_options = write_options_from_scalars(
        append,
        content_type_handle,
        content_disposition_handle,
        content_encoding_handle,
        cache_control_handle,
        if_match_handle,
        if_none_match_handle,
    );
    handle_or_record_error(write_options.and_then(|write_options| {
        start_write_task(operator_handle, path_handle, data_handle, write_options)
    }))
}

/// Starts an asynchronous whole-object read and returns an owned task handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_read_start(
    operator_handle: u32,
    path_handle: u32,
) -> u32 {
    handle_or_record_error(start_read_task(
        operator_handle,
        path_handle,
        options::ReadOptions::default(),
    ))
}

/// Starts an asynchronous read with range and conditional options.
#[allow(clippy::too_many_arguments, reason = "mirrors the frozen scalar ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_read_options_start_v1(
    operator_handle: u32,
    path_handle: u32,
    range_kind: u32,
    range_offset: u64,
    range_length: u64,
    version_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> u32 {
    let read_options = read_options_from_scalars(
        range_kind,
        range_offset,
        range_length,
        version_handle,
        if_match_handle,
        if_none_match_handle,
    );
    handle_or_record_error(
        read_options
            .and_then(|read_options| start_read_task(operator_handle, path_handle, read_options)),
    )
}

/// Starts an asynchronous metadata lookup and returns an owned task handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_stat_start(
    operator_handle: u32,
    path_handle: u32,
) -> u32 {
    handle_or_record_error(start_stat_task(
        operator_handle,
        path_handle,
        options::StatOptions::default(),
    ))
}

/// Starts an asynchronous stat with version and conditional options.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_stat_options_start_v1(
    operator_handle: u32,
    path_handle: u32,
    version_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> u32 {
    let stat_options =
        stat_options_from_scalars(version_handle, if_match_handle, if_none_match_handle);
    handle_or_record_error(
        stat_options
            .and_then(|stat_options| start_stat_task(operator_handle, path_handle, stat_options)),
    )
}

/// Starts an asynchronous recursive directory creation.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_create_dir_start(
    operator_handle: u32,
    path_handle: u32,
) -> u32 {
    let result = path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
        start_task(async move {
            Completion::CreateDir(
                operator
                    .create_dir(&path)
                    .await
                    .map(|_| ())
                    .map_err(BridgeError::from),
            )
        })
    });
    handle_or_record_error(result)
}

/// Starts an asynchronous delete with optional version and recursive mode.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_delete_start(
    operator_handle: u32,
    path_handle: u32,
    version_handle: u32,
    recursive: u32,
) -> u32 {
    let inputs = scalar_bool(recursive, "recursive").and_then(|recursive| {
        path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
            let version = optional_owned_utf8_buffer(version_handle)?;
            Ok((operator, path, version, recursive))
        })
    });
    let result = inputs.and_then(|(operator, path, version, recursive)| {
        start_task(async move {
            let options = opendal::options::DeleteOptions { version, recursive };
            Completion::Delete(
                operator
                    .delete_options(&path, options)
                    .await
                    .map(|_| ())
                    .map_err(BridgeError::from),
            )
        })
    });
    handle_or_record_error(result)
}

/// Starts a bounded asynchronous prefix listing.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_list_start(
    operator_handle: u32,
    path_handle: u32,
    recursive: u32,
    has_limit: u32,
    limit: u64,
    start_after_handle: u32,
) -> u32 {
    let options = scalar_bool(recursive, "recursive").and_then(|recursive| {
        optional_list_limit(has_limit, limit).map(|limit| (recursive, limit))
    });
    let inputs = options.and_then(|(recursive, limit)| {
        path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
            let start_after = optional_owned_utf8_buffer(start_after_handle)?;
            Ok((operator, path, recursive, limit, start_after))
        })
    });
    let result = inputs.and_then(|(operator, path, recursive, limit, start_after)| {
        start_task(async move {
            Completion::List(collect_list(operator, path, recursive, limit, start_after).await)
        })
    });
    handle_or_record_error(result)
}

/// Starts an asynchronous server-side copy and returns destination metadata.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_copy_start(
    operator_handle: u32,
    from_handle: u32,
    to_handle: u32,
) -> u32 {
    handle_or_record_error(start_copy_task(operator_handle, from_handle, to_handle))
}

/// Starts an asynchronous server-side rename.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_rename_start(
    operator_handle: u32,
    from_handle: u32,
    to_handle: u32,
) -> u32 {
    handle_or_record_error(start_rename_task(operator_handle, from_handle, to_handle))
}

/// Opens a stateful, bounded read stream through an asynchronous task.
#[allow(clippy::too_many_arguments, reason = "mirrors the frozen scalar ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_read_stream_start_v1(
    operator_handle: u32,
    path_handle: u32,
    chunk_size: u32,
    range_kind: u32,
    range_offset: u64,
    range_length: u64,
    version_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> u32 {
    let options = read_stream_options_from_scalars(
        chunk_size,
        range_kind,
        range_offset,
        range_length,
        version_handle,
        if_match_handle,
        if_none_match_handle,
    );
    let result = options.and_then(|(reader_options, range, max_chunk)| {
        let (operator, path) = path_and_operator(operator_handle, path_handle)?;
        ensure_suffix_is_native(&operator, &range)?;
        start_task(async move {
            let result = async {
                let reader = operator
                    .reader_options(&path, reader_options)
                    .await
                    .map_err(BridgeError::from)?;
                let stream = reader.into_stream(range).await.map_err(BridgeError::from)?;
                Ok(Box::new(ReadStreamCursor {
                    stream,
                    pending: None,
                    max_chunk,
                }))
            }
            .await;
            Completion::OpenReadStream(result)
        })
    });
    handle_or_record_error(result)
}

/// Starts one bounded read from a stateful read stream.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_read_stream_next_start(handle: u32) -> u32 {
    let result = reserve_read_stream_next(handle).map(|(task, lease, cursor)| {
        spawn_resource_task(task, lease, read_stream_next(cursor));
        task
    });
    handle_or_record_error(result)
}

/// Releases a read stream. A pending operation can never restore it afterwards.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_read_stream_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::ReadStream)
}

/// Opens a stateful writer through an asynchronous task.
#[allow(clippy::too_many_arguments, reason = "mirrors the frozen scalar ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_writer_start_v1(
    operator_handle: u32,
    path_handle: u32,
    append: u32,
    content_type_handle: u32,
    content_disposition_handle: u32,
    content_encoding_handle: u32,
    cache_control_handle: u32,
    if_match_handle: u32,
    if_none_match_handle: u32,
) -> u32 {
    let options = write_options_from_scalars(
        append,
        content_type_handle,
        content_disposition_handle,
        content_encoding_handle,
        cache_control_handle,
        if_match_handle,
        if_none_match_handle,
    );
    let result = options.and_then(|write_options| {
        let (operator, path) = path_and_operator(operator_handle, path_handle)?;
        start_task(async move {
            Completion::OpenWriter(
                operator
                    .writer_options(&path, write_options)
                    .await
                    .map(Box::new)
                    .map_err(BridgeError::from),
            )
        })
    });
    handle_or_record_error(result)
}

/// Writes one synchronously copied, bounded chunk through a stateful writer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_writer_write_start(writer_handle: u32, data_handle: u32) -> u32 {
    let data = owned_writer_chunk(data_handle);
    let result = data.and_then(|data| {
        reserve_writer_call(writer_handle).map(|(task, lease, mut writer)| {
            spawn_resource_task(task, lease, async move {
                match writer.write(Buffer::from(data)).await {
                    Ok(()) => (
                        ResourceUpdate::Writer(WriterUpdate::Open(writer)),
                        Completion::WriterWrite(Ok(())),
                    ),
                    Err(error) => (
                        ResourceUpdate::Writer(WriterUpdate::Failed),
                        Completion::WriterWrite(Err(BridgeError::from(error))),
                    ),
                }
            });
            task
        })
    });
    handle_or_record_error(result)
}

/// Commits a stateful writer and returns destination metadata.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_writer_finish_start(writer_handle: u32) -> u32 {
    let finished = STATE.with(|state| {
        let mut state = state.borrow_mut();
        match state.arena.writer_mut(writer_handle)? {
            WriterState::Finished(metadata) => Ok(Some(metadata.clone())),
            WriterState::Open(_, _) => Ok(None),
            WriterState::Busy(_) => Err(BridgeError::ResourceBusy {
                handle: writer_handle,
            }),
            WriterState::Aborted | WriterState::Failed => Err(BridgeError::ResourceClosed {
                handle: writer_handle,
            }),
        }
    });
    let result = finished.and_then(|finished| match finished {
        Some(metadata) => start_task(async move { Completion::WriterFinish(Ok(metadata)) }),
        None => reserve_writer_call(writer_handle).map(|(task, lease, mut writer)| {
            spawn_resource_task(task, lease, async move {
                match writer.close().await {
                    Ok(metadata) => {
                        let metadata = Box::new(metadata);
                        (
                            ResourceUpdate::Writer(WriterUpdate::Finished(metadata.clone())),
                            Completion::WriterFinish(Ok(metadata)),
                        )
                    }
                    Err(error) => (
                        ResourceUpdate::Writer(WriterUpdate::Failed),
                        Completion::WriterFinish(Err(BridgeError::from(error))),
                    ),
                }
            });
            task
        }),
    });
    handle_or_record_error(result)
}

/// Aborts a writer. Abort is terminal and repeated aborts are successful.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_writer_abort_start(writer_handle: u32) -> u32 {
    let repeated = STATE.with(|state| {
        let mut state = state.borrow_mut();
        match state.arena.writer_mut(writer_handle)? {
            WriterState::Aborted => Ok(true),
            WriterState::Open(_, _) => Ok(false),
            WriterState::Busy(_) => Err(BridgeError::ResourceBusy {
                handle: writer_handle,
            }),
            WriterState::Finished(_) | WriterState::Failed => Err(BridgeError::ResourceClosed {
                handle: writer_handle,
            }),
        }
    });
    let result = repeated.and_then(|repeated| {
        if repeated {
            return start_task(async { Completion::WriterAbort(Ok(())) });
        }
        reserve_writer_call(writer_handle).map(|(task, lease, mut writer)| {
            spawn_resource_task(task, lease, async move {
                match writer.abort().await {
                    Ok(()) => (
                        ResourceUpdate::Writer(WriterUpdate::Aborted),
                        Completion::WriterAbort(Ok(())),
                    ),
                    Err(error) => (
                        ResourceUpdate::Writer(WriterUpdate::Failed),
                        Completion::WriterAbort(Err(BridgeError::from(error))),
                    ),
                }
            });
            task
        })
    });
    handle_or_record_error(result)
}

/// Releases a writer and makes any pending operation terminal.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_writer_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Writer)
}

/// Opens a stateful lister through an asynchronous task.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_lister_start(
    operator_handle: u32,
    path_handle: u32,
    recursive: u32,
    has_limit: u32,
    limit: u64,
    start_after_handle: u32,
) -> u32 {
    let options = scalar_bool(recursive, "recursive").and_then(|recursive| {
        optional_list_limit(has_limit, limit).map(|limit| (recursive, limit))
    });
    let inputs = options.and_then(|(recursive, limit)| {
        path_and_operator(operator_handle, path_handle).and_then(|(operator, path)| {
            let start_after = optional_owned_utf8_buffer(start_after_handle)?;
            Ok((operator, path, recursive, limit, start_after))
        })
    });
    let result = inputs.and_then(|(operator, path, recursive, limit, start_after)| {
        start_task(async move {
            let result = async {
                let mut request = operator.lister_with(&path).recursive(recursive);
                if let Some(limit) = limit {
                    request = request.limit(limit);
                }
                if let Some(start_after) = &start_after {
                    request = request.start_after(start_after);
                }
                request.await.map(Box::new).map_err(BridgeError::from)
            }
            .await;
            Completion::OpenLister(result)
        })
    });
    handle_or_record_error(result)
}

/// Starts one stateful lister step, returning one entry or explicit EOF.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_lister_next_start(handle: u32) -> u32 {
    let result = reserve_lister_next(handle).map(|(task, lease, mut lister)| {
        spawn_resource_task(task, lease, async move {
            match lister.try_next().await.map_err(BridgeError::from) {
                Ok(Some(entry)) => match entry_snapshot(entry) {
                    Ok(entry) => (
                        ResourceUpdate::Lister(ListerUpdate::Open(lister)),
                        Completion::ListerNext(Ok(Some(entry))),
                    ),
                    Err(error) => (
                        ResourceUpdate::Lister(ListerUpdate::Failed),
                        Completion::ListerNext(Err(error)),
                    ),
                },
                Ok(None) => (
                    ResourceUpdate::Lister(ListerUpdate::End),
                    Completion::ListerNext(Ok(None)),
                ),
                Err(error) => (
                    ResourceUpdate::Lister(ListerUpdate::Failed),
                    Completion::ListerNext(Err(error)),
                ),
            }
        });
        task
    });
    handle_or_record_error(result)
}

/// Releases a lister. A pending operation can never restore it afterwards.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_lister_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Lister)
}

fn handle_or_record_error(result: Result<u32, BridgeError>) -> u32 {
    match result {
        Ok(handle) => handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Returns one of the `TASK_*` scalar states, or `0` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_task_state(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(match state.arena.task(handle)? {
            Task::Pending(_) => TASK_PENDING,
            Task::Ready(_, _) => TASK_READY,
            Task::Cancelled => TASK_CANCELLED,
            Task::Consumed => TASK_CONSUMED,
        })
    });
    match result {
        Ok(task_state) => task_state,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Moves a ready task result into a separately owned completion handle.
///
/// A result can be taken exactly once. Returns `0` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_task_take(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.arena.can_insert() {
            return Err(BridgeError::HandleLimit);
        }
        let task = state.arena.task_mut(handle)?;
        match std::mem::replace(task, Task::Consumed) {
            Task::Ready(completion, _) => state.arena.insert(Resource::Completion(*completion)),
            Task::Pending(lease) => {
                *task = Task::Pending(lease);
                Err(BridgeError::TaskNotReady { handle })
            }
            Task::Cancelled => {
                *task = Task::Cancelled;
                Err(BridgeError::TaskNotReady { handle })
            }
            Task::Consumed => Err(BridgeError::TaskConsumed { handle }),
        }
    });
    handle_or_record_error(result)
}

/// Requests logical cancellation.
///
/// Cancelling a pending task makes its late completion inert. Cancelling a
/// ready task wins the race and drops the unconsumed result.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_task_cancel(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let lease = task_lease(state.arena.task(handle)?);
        if let Some(lease) = lease {
            terminalize_lease(&mut state.arena, lease);
        }
        let task = state.arena.task_mut(handle)?;
        match task {
            Task::Pending(_) | Task::Ready(_, _) => *task = Task::Cancelled,
            Task::Cancelled | Task::Consumed => {}
        }
        Ok(())
    });
    status(result)
}

/// Releases a task and any unconsumed completion it still owns.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_task_release(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let lease = task_lease(state.arena.task(handle)?);
        if let Some(lease) = lease {
            terminalize_lease(&mut state.arena, lease);
        }
        state.arena.release(handle, ResourceKind::Task)
    });
    status(result)
}

/// Returns the completion operation kind, or `0` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_kind(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(state.arena.completion(handle)?.kind())
    });
    match result {
        Ok(kind) => kind,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Returns the completion's stable status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_status(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(match state.arena.completion(handle)? {
            Completion::Write(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::Read(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::Stat(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::CreateDir(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::Delete(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::List(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::Copy(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::Rename(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::OpenReadStream(result) => {
                result.as_ref().map_or_else(BridgeError::code, |_| 0)
            }
            Completion::ReadStreamNext(result) => {
                result.as_ref().map_or_else(BridgeError::code, |_| 0)
            }
            Completion::OpenWriter(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::WriterWrite(result) => {
                result.as_ref().map_or_else(BridgeError::code, |_| 0)
            }
            Completion::WriterFinish(result) => {
                result.as_ref().map_or_else(BridgeError::code, |_| 0)
            }
            Completion::WriterAbort(result) => {
                result.as_ref().map_or_else(BridgeError::code, |_| 0)
            }
            Completion::OpenLister(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
            Completion::ListerNext(result) => result.as_ref().map_or_else(BridgeError::code, |_| 0),
        })
    });
    match result {
        Ok(completion_status) => completion_status,
        Err(error) => record_error(error),
    }
}

fn completion_is_end_value(handle: u32) -> Result<bool, BridgeError> {
    STATE.with(|state| {
        let state = state.borrow();
        match state.arena.completion(handle)? {
            Completion::ReadStreamNext(Ok(value)) => Ok(value.is_none()),
            Completion::ListerNext(Ok(value)) => Ok(value.is_none()),
            Completion::ReadStreamNext(Err(error)) | Completion::ListerNext(Err(error)) => {
                Err(error.clone())
            }
            completion => Err(BridgeError::WrongResourceType {
                handle,
                expected: "next completion",
                actual: match completion.kind() {
                    COMPLETION_READ => "read completion",
                    COMPLETION_LIST => "list completion",
                    _ => "non-next completion",
                },
            }),
        }
    })
}

/// Inspects a next completion without consuming it (`1` EOF, `0` value, `-1` error).
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_is_end(handle: u32) -> i32 {
    match completion_is_end_value(handle) {
        Ok(is_end) => i32::from(is_end),
        Err(error) => {
            record_error(error);
            SCALAR_ERROR
        }
    }
}

/// Moves a successful open-read-stream result into a read-stream handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_read_stream(handle: u32) -> u32 {
    let result =
        take_successful_completion(handle, COMPLETION_OPEN_READER, true).and_then(|completion| {
            let Completion::OpenReadStream(Ok(cursor)) = completion else {
                unreachable!("completion kind and status checked before removal")
            };
            insert_resource(Resource::ReadStream(ReadStreamState::Open(cursor, 0)))
        });
    handle_or_record_error(result)
}

/// Moves a successful open-writer result into a writer handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_writer(handle: u32) -> u32 {
    let result =
        take_successful_completion(handle, COMPLETION_OPEN_WRITER, true).and_then(|completion| {
            let Completion::OpenWriter(Ok(writer)) = completion else {
                unreachable!("completion kind and status checked before removal")
            };
            insert_resource(Resource::Writer(WriterState::Open(writer, 0)))
        });
    handle_or_record_error(result)
}

/// Moves a successful open-lister result into a lister handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_lister(handle: u32) -> u32 {
    let result =
        take_successful_completion(handle, COMPLETION_OPEN_LISTER, true).and_then(|completion| {
            let Completion::OpenLister(Ok(lister)) = completion else {
                unreachable!("completion kind and status checked before removal")
            };
            insert_resource(Resource::Lister(ListerState::Open(lister, 0)))
        });
    handle_or_record_error(result)
}

/// Moves a successful read result into a buffer handle, or returns `0`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_buffer(handle: u32) -> u32 {
    let kind = STATE.with(|state| {
        let state = state.borrow();
        state.arena.completion(handle).map(Completion::kind)
    });
    let result = kind.and_then(|kind| match kind {
        COMPLETION_READ => {
            take_successful_completion(handle, COMPLETION_READ, true).and_then(|completion| {
                let Completion::Read(Ok(buffer)) = completion else {
                    unreachable!("completion kind and status checked before removal")
                };
                insert_buffer(buffer)
            })
        }
        COMPLETION_READER_NEXT => {
            let is_end = completion_is_end_value(handle)?;
            take_successful_completion(handle, COMPLETION_READER_NEXT, !is_end).and_then(
                |completion| {
                    let Completion::ReadStreamNext(Ok(buffer)) = completion else {
                        unreachable!("completion kind and status checked before removal")
                    };
                    buffer.map_or(Ok(0), insert_buffer)
                },
            )
        }
        actual => Err(wrong_completion_type(handle, COMPLETION_READ, actual)),
    });
    handle_or_record_error(result)
}

/// Moves a successful stat result into a metadata handle, or returns `0`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_metadata(handle: u32) -> u32 {
    let kind = STATE.with(|state| {
        let state = state.borrow();
        state.arena.completion(handle).map(Completion::kind)
    });
    let result = kind.and_then(|kind| {
        if !matches!(
            kind,
            COMPLETION_STAT | COMPLETION_COPY | COMPLETION_WRITER_FINISH
        ) {
            return Err(wrong_completion_type(handle, COMPLETION_STAT, kind));
        }
        take_successful_completion(handle, kind, true).and_then(|completion| {
            let metadata = match completion {
                Completion::Stat(Ok(metadata))
                | Completion::Copy(Ok(metadata))
                | Completion::WriterFinish(Ok(metadata)) => metadata,
                _ => unreachable!("metadata completion kind checked before removal"),
            };
            STATE.with(|state| {
                state
                    .borrow_mut()
                    .arena
                    .insert(Resource::Metadata(*metadata))
            })
        })
    });
    handle_or_record_error(result)
}

/// Atomically replaces a successful stat/write completion with an ODM1 buffer.
///
/// Any encoding or replacement failure leaves the completion valid.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_metadata_snapshot(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let completion = state.arena.completion(handle)?;
        let snapshot = match completion {
            Completion::Write(Ok(metadata))
            | Completion::Stat(Ok(metadata))
            | Completion::Copy(Ok(metadata))
            | Completion::WriterFinish(Ok(metadata)) => metadata_snapshot_bytes(metadata)?,
            Completion::Write(Err(error))
            | Completion::Stat(Err(error))
            | Completion::Copy(Err(error))
            | Completion::WriterFinish(Err(error)) => {
                return Err(error.clone());
            }
            completion => {
                return Err(BridgeError::WrongResourceType {
                    handle,
                    expected: "successful metadata completion",
                    actual: match completion.kind() {
                        COMPLETION_READ => "read completion",
                        COMPLETION_CREATE_DIR => "create-dir completion",
                        COMPLETION_DELETE => "delete completion",
                        COMPLETION_LIST => "list completion",
                        COMPLETION_RENAME => "rename completion",
                        COMPLETION_OPEN_READER => "open-read-stream completion",
                        COMPLETION_READER_NEXT => "read-stream-next completion",
                        COMPLETION_OPEN_WRITER => "open-writer completion",
                        COMPLETION_WRITER_WRITE => "writer-write completion",
                        COMPLETION_WRITER_ABORT => "writer-abort completion",
                        COMPLETION_OPEN_LISTER => "open-lister completion",
                        COMPLETION_LISTER_NEXT => "lister-next completion",
                        _ => "completion",
                    },
                });
            }
        };
        state
            .arena
            .ensure_insert_capacity_after_take(handle, ResourceKind::Completion)?;
        let resource = state.arena.take(handle, ResourceKind::Completion)?;
        match resource {
            Resource::Completion(
                Completion::Write(Ok(_))
                | Completion::Stat(Ok(_))
                | Completion::Copy(Ok(_))
                | Completion::WriterFinish(Ok(_)),
            ) => {}
            _ => unreachable!("metadata completion was checked before removal"),
        }
        match state.arena.insert(Resource::Buffer(snapshot)) {
            Ok(snapshot_handle) => Ok(snapshot_handle),
            Err(_) => {
                unreachable!("replacement capacity was checked before consuming the completion")
            }
        }
    });
    handle_or_record_error(result)
}

/// Moves a successful list result into an owned entry-list handle.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_entry_list(handle: u32) -> u32 {
    let kind = STATE.with(|state| {
        let state = state.borrow();
        state.arena.completion(handle).map(Completion::kind)
    });
    let result = kind.and_then(|kind| match kind {
        COMPLETION_LIST => {
            take_successful_completion(handle, COMPLETION_LIST, true).and_then(|completion| {
                let Completion::List(Ok(entries)) = completion else {
                    unreachable!("completion kind and status checked before removal")
                };
                STATE.with(|state| {
                    state
                        .borrow_mut()
                        .arena
                        .insert(Resource::EntryList(entries))
                })
            })
        }
        COMPLETION_LISTER_NEXT => {
            let is_end = completion_is_end_value(handle)?;
            take_successful_completion(handle, COMPLETION_LISTER_NEXT, !is_end).and_then(
                |completion| {
                    let Completion::ListerNext(Ok(entry)) = completion else {
                        unreachable!("completion kind and status checked before removal")
                    };
                    entry.map_or(Ok(0), |entry| {
                        STATE.with(|state| {
                            state
                                .borrow_mut()
                                .arena
                                .insert(Resource::EntryList(vec![entry]))
                        })
                    })
                },
            )
        }
        actual => Err(wrong_completion_type(handle, COMPLETION_LIST, actual)),
    });
    handle_or_record_error(result)
}

fn take_successful_completion(
    handle: u32,
    expected_kind: u32,
    needs_replacement: bool,
) -> Result<Completion, BridgeError> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let completion = state.arena.completion(handle)?;
        let actual_kind = completion.kind();
        if actual_kind != expected_kind {
            return Err(wrong_completion_type(handle, expected_kind, actual_kind));
        }
        if let Some(error) = completion.error() {
            return Err(error.clone());
        }
        if let Completion::Read(Ok(buffer)) = completion
            && buffer.len() > MAX_BUFFER_LENGTH
        {
            return Err(BridgeError::BufferTooLarge);
        }
        if let Completion::ReadStreamNext(Ok(Some(buffer))) = completion
            && buffer.len() > MAX_TRANSFER_CHUNK
        {
            return Err(BridgeError::BufferTooLarge);
        }
        if needs_replacement {
            state
                .arena
                .ensure_insert_capacity_after_take(handle, ResourceKind::Completion)?;
        }
        let resource = state.arena.take(handle, ResourceKind::Completion)?;
        let Resource::Completion(completion) = resource else {
            unreachable!("arena checked completion resource type")
        };
        Ok(completion)
    })
}

fn wrong_completion_type(handle: u32, expected: u32, actual: u32) -> BridgeError {
    let name = |kind| match kind {
        COMPLETION_WRITE => "write completion",
        COMPLETION_READ => "read completion",
        COMPLETION_STAT => "stat completion",
        COMPLETION_CREATE_DIR => "create-dir completion",
        COMPLETION_DELETE => "delete completion",
        COMPLETION_LIST => "list completion",
        COMPLETION_COPY => "copy completion",
        COMPLETION_RENAME => "rename completion",
        COMPLETION_OPEN_READER => "open-read-stream completion",
        COMPLETION_READER_NEXT => "read-stream-next completion",
        COMPLETION_OPEN_WRITER => "open-writer completion",
        COMPLETION_WRITER_WRITE => "writer-write completion",
        COMPLETION_WRITER_FINISH => "writer-finish completion",
        COMPLETION_WRITER_ABORT => "writer-abort completion",
        COMPLETION_OPEN_LISTER => "open-lister completion",
        COMPLETION_LISTER_NEXT => "lister-next completion",
        _ => "completion",
    };
    BridgeError::WrongResourceType {
        handle,
        expected: name(expected),
        actual: name(actual),
    }
}

/// Moves a failed completion into an owned error handle, or returns `0`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_take_error(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let completion = state.arena.completion(handle)?;
        let is_error = match completion {
            Completion::Write(result) => result.is_err(),
            Completion::Read(result) => result.is_err(),
            Completion::Stat(result) => result.is_err(),
            Completion::CreateDir(result) => result.is_err(),
            Completion::Delete(result) => result.is_err(),
            Completion::List(result) => result.is_err(),
            Completion::Copy(result) => result.is_err(),
            Completion::Rename(result) => result.is_err(),
            Completion::OpenReadStream(result) => result.is_err(),
            Completion::ReadStreamNext(result) => result.is_err(),
            Completion::OpenWriter(result) => result.is_err(),
            Completion::WriterWrite(result) => result.is_err(),
            Completion::WriterFinish(result) => result.is_err(),
            Completion::WriterAbort(result) => result.is_err(),
            Completion::OpenLister(result) => result.is_err(),
            Completion::ListerNext(result) => result.is_err(),
        };
        if !is_error {
            return Err(BridgeError::WrongResourceType {
                handle,
                expected: "failed completion",
                actual: "successful completion",
            });
        }
        state
            .arena
            .ensure_insert_capacity_after_take(handle, ResourceKind::Completion)?;
        let resource = state.arena.take(handle, ResourceKind::Completion)?;
        let Resource::Completion(completion) = resource else {
            unreachable!("arena checked completion resource type")
        };
        let error = match completion {
            Completion::Write(Err(error))
            | Completion::Read(Err(error))
            | Completion::Stat(Err(error))
            | Completion::CreateDir(Err(error))
            | Completion::Delete(Err(error))
            | Completion::List(Err(error))
            | Completion::Copy(Err(error))
            | Completion::Rename(Err(error))
            | Completion::OpenReadStream(Err(error))
            | Completion::ReadStreamNext(Err(error))
            | Completion::OpenWriter(Err(error))
            | Completion::WriterWrite(Err(error))
            | Completion::WriterFinish(Err(error))
            | Completion::WriterAbort(Err(error))
            | Completion::OpenLister(Err(error))
            | Completion::ListerNext(Err(error)) => error,
            Completion::Write(Ok(_))
            | Completion::Read(Ok(_))
            | Completion::Stat(Ok(_))
            | Completion::CreateDir(Ok(()))
            | Completion::Delete(Ok(()))
            | Completion::List(Ok(_))
            | Completion::Copy(Ok(_))
            | Completion::Rename(Ok(()))
            | Completion::OpenReadStream(Ok(_))
            | Completion::ReadStreamNext(Ok(_))
            | Completion::OpenWriter(Ok(_))
            | Completion::WriterWrite(Ok(()))
            | Completion::WriterFinish(Ok(_))
            | Completion::WriterAbort(Ok(()))
            | Completion::OpenLister(Ok(_))
            | Completion::ListerNext(Ok(_)) => unreachable!("success checked before removal"),
        };
        state.arena.insert(Resource::Error(error))
    });
    handle_or_record_error(result)
}

/// Releases a completion and any unconsumed result it still owns.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_completion_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Completion)
}

fn entry_list_value<T>(
    handle: u32,
    index: u32,
    select: impl FnOnce(&EntrySnapshot) -> T,
) -> Result<T, BridgeError> {
    STATE.with(|state| {
        let state = state.borrow();
        let entries = state.arena.entry_list(handle)?;
        let index_usize = usize::try_from(index).map_err(|_| BridgeError::LengthOverflow)?;
        let entry = entries
            .get(index_usize)
            .ok_or(BridgeError::IndexOutOfBounds {
                index,
                length: entries.len(),
            })?;
        Ok(select(entry))
    })
}

/// Returns the number of entries in a bounded list, or `-1` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_entry_list_len(handle: u32) -> i32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        i32::try_from(state.arena.entry_list(handle)?.len())
            .map_err(|_| BridgeError::LengthOverflow)
    });
    match result {
        Ok(length) => length,
        Err(error) => {
            record_error(error);
            SCALAR_ERROR
        }
    }
}

fn entry_list_text(handle: u32, index: u32, path: bool) -> u32 {
    let result = entry_list_value(handle, index, |entry| {
        try_owned_bytes(if path {
            entry.path.as_bytes()
        } else {
            entry.name.as_bytes()
        })
    })
    .and_then(std::convert::identity);
    handle_or_record_error(result.and_then(insert_buffer))
}

/// Copies an entry path into a newly owned buffer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_entry_list_path(handle: u32, index: u32) -> u32 {
    entry_list_text(handle, index, true)
}

/// Copies an entry basename into a newly owned buffer.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_entry_list_name(handle: u32, index: u32) -> u32 {
    entry_list_text(handle, index, false)
}

/// Returns an entry mode (`0` unknown, `1` file, `2` directory), or `-1`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_entry_list_mode(handle: u32, index: u32) -> i32 {
    match entry_list_value(handle, index, |entry| entry.mode) {
        Ok(mode) => i32::try_from(mode).expect("entry mode fits i32"),
        Err(error) => {
            record_error(error);
            SCALAR_ERROR
        }
    }
}

/// Returns an entry's cached content length, recording failure separately.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_entry_list_content_length(handle: u32, index: u32) -> u64 {
    match entry_list_value(handle, index, |entry| entry.content_length) {
        Ok(length) => length,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Moves one pre-encoded ODM1 entry metadata snapshot into a buffer handle.
///
/// Each entry snapshot can be taken once. Capacity failure leaves it available.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_entry_list_metadata_snapshot(handle: u32, index: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let index = usize::try_from(index).map_err(|_| BridgeError::LengthOverflow)?;
        {
            let entries = state.arena.entry_list(handle)?;
            let entry = entries.get(index).ok_or(BridgeError::IndexOutOfBounds {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                length: entries.len(),
            })?;
            if entry.metadata_snapshot.is_empty() {
                return Err(BridgeError::InvalidArgument {
                    message: "entry metadata snapshot has already been taken".to_owned(),
                });
            }
        }
        if !state.arena.can_insert() {
            return Err(BridgeError::HandleLimit);
        }
        let snapshot = state
            .arena
            .entry_list_mut(handle)?
            .get_mut(index)
            .map(|entry| std::mem::take(&mut entry.metadata_snapshot))
            .filter(|snapshot| !snapshot.is_empty())
            .ok_or_else(|| BridgeError::InvalidArgument {
                message: "entry metadata snapshot has already been taken".to_owned(),
            })?;
        match state.arena.insert(Resource::Buffer(snapshot)) {
            Ok(snapshot_handle) => Ok(snapshot_handle),
            Err(_) => unreachable!("buffer capacity was checked before moving entry metadata"),
        }
    });
    handle_or_record_error(result)
}

/// Releases an entry list and all of its owned snapshots.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_entry_list_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::EntryList)
}

/// Releases an operator handle and returns a status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_operator_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Operator)
}

/// Returns the low 32 bits of a metadata handle's content length.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_metadata_content_length_low(handle: u32) -> u32 {
    metadata_content_length(handle, |length| length as u32)
}

/// Returns the high 32 bits of a metadata handle's content length.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_metadata_content_length_high(handle: u32) -> u32 {
    metadata_content_length(handle, |length| (length >> 32) as u32)
}

fn metadata_content_length(handle: u32, part: impl FnOnce(u64) -> u32) -> u32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(part(state.arena.metadata(handle)?.content_length()))
    });
    match result {
        Ok(value) => value,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Returns `1` for a file, `0` for a non-file entry, or `-1` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_metadata_is_file(handle: u32) -> i32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(i32::from(state.arena.metadata(handle)?.is_file()))
    });
    match result {
        Ok(value) => value,
        Err(error) => {
            record_error(error);
            SCALAR_ERROR
        }
    }
}

/// Returns a metadata mode (`0` unknown, `1` file, `2` directory), or `-1`.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_metadata_mode(handle: u32) -> i32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(match state.arena.metadata(handle)?.mode() {
            opendal::EntryMode::FILE => ENTRY_MODE_FILE,
            opendal::EntryMode::DIR => ENTRY_MODE_DIRECTORY,
            opendal::EntryMode::Unknown => ENTRY_MODE_UNKNOWN,
        })
    });
    match result {
        Ok(mode) => i32::try_from(mode).expect("entry mode fits i32"),
        Err(error) => {
            record_error(error);
            SCALAR_ERROR
        }
    }
}

/// Releases a metadata handle and returns a status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_metadata_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Metadata)
}

/// Returns the sticky last-error code, or `0` when no error is recorded.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_last_error_code() -> u32 {
    STATE.with(|state| {
        state
            .borrow()
            .last_error
            .as_ref()
            .map_or(STATUS_OK, BridgeError::code)
    })
}

/// Moves the sticky last error into an owned error handle.
///
/// Returns `0` if no last error exists or the error handle cannot be allocated.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_last_error_take() -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.torn_down {
            state.last_error = None;
            return Ok(0);
        }
        let Some(error) = state.last_error.clone() else {
            return Ok(0);
        };
        let handle = state.arena.insert(Resource::Error(error))?;
        state.last_error = None;
        Ok(handle)
    });
    match result {
        Ok(handle) => handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Clears the sticky last error and returns success.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_last_error_clear() -> u32 {
    STATE.with(|state| state.borrow_mut().last_error = None);
    STATUS_OK
}

/// Returns an owned error handle's stable error code, or `0` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_error_code(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let state = state.borrow();
        Ok(state.arena.error(handle)?.code())
    });
    match result {
        Ok(code) => code,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Copies an owned error's UTF-8 message into a new buffer handle.
///
/// Returns `0` on failure. The returned buffer must be released separately.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_error_message(handle: u32) -> u32 {
    let result = STATE
        .with(|state| {
            let state = state.borrow();
            Ok(state.arena.error(handle)?.to_string().into_bytes())
        })
        .and_then(insert_buffer);
    match result {
        Ok(message_handle) => message_handle,
        Err(error) => {
            record_error(error);
            0
        }
    }
}

/// Atomically replaces an owned error with its versioned binary snapshot.
///
/// The little-endian `ODE1` schema contains six 32-bit header words followed
/// by the UTF-8 kind name and message. Success consumes the error handle and
/// returns a buffer handle. Any failure leaves the error handle valid.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_error_snapshot_take(handle: u32) -> u32 {
    let result = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let snapshot = error_snapshot_bytes(state.arena.error(handle)?)?;
        state
            .arena
            .ensure_insert_capacity_after_take(handle, ResourceKind::Error)?;
        let resource = state.arena.take(handle, ResourceKind::Error)?;
        let Resource::Error(_) = resource else {
            unreachable!("arena checked error resource type")
        };
        match state.arena.insert(Resource::Buffer(snapshot)) {
            Ok(snapshot_handle) => Ok(snapshot_handle),
            Err(_) => unreachable!("replacement capacity was checked before consuming the error"),
        }
    });
    handle_or_record_error(result)
}

/// Releases an owned error handle and returns a status code.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_mbt_wasm_error_release(handle: u32) -> u32 {
    release_resource(handle, ResourceKind::Error)
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use std::io::{Read, Write};
    #[cfg(not(target_arch = "wasm32"))]
    use std::net::TcpListener;
    #[cfg(not(target_arch = "wasm32"))]
    use std::thread;
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{Duration, Instant};

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct DecodedErrorSnapshot<'a> {
        kind: u32,
        status: u32,
        kind_name: &'a str,
        message: &'a str,
    }

    fn snapshot_word(snapshot: &[u8], offset: usize) -> Result<u32, &'static str> {
        let bytes = snapshot
            .get(offset..offset + 4)
            .ok_or("truncated snapshot header")?;
        let bytes: [u8; 4] = bytes
            .try_into()
            .map_err(|_| "snapshot word has the wrong length")?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn snapshot_u64(snapshot: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(snapshot[offset..offset + 8].try_into().unwrap())
    }

    fn snapshot_i64(snapshot: &[u8], offset: usize) -> i64 {
        i64::from_le_bytes(snapshot[offset..offset + 8].try_into().unwrap())
    }

    fn decode_error_snapshot(snapshot: &[u8]) -> Result<DecodedErrorSnapshot<'_>, &'static str> {
        if snapshot.get(0..4) != Some(ERROR_SNAPSHOT_MAGIC.as_slice()) {
            return Err("invalid snapshot magic");
        }
        if snapshot_word(snapshot, 4)? != ERROR_SNAPSHOT_SCHEMA {
            return Err("unsupported snapshot schema");
        }
        let kind_name_length =
            usize::try_from(snapshot_word(snapshot, 16)?).map_err(|_| "kind name too long")?;
        let message_length =
            usize::try_from(snapshot_word(snapshot, 20)?).map_err(|_| "message too long")?;
        let kind_name_end = ERROR_SNAPSHOT_HEADER_LENGTH
            .checked_add(kind_name_length)
            .ok_or("kind name length overflow")?;
        let message_end = kind_name_end
            .checked_add(message_length)
            .ok_or("message length overflow")?;
        if message_end != snapshot.len() {
            return Err("snapshot payload length mismatch");
        }
        let kind_name = std::str::from_utf8(
            snapshot
                .get(ERROR_SNAPSHOT_HEADER_LENGTH..kind_name_end)
                .ok_or("truncated kind name")?,
        )
        .map_err(|_| "kind name is not UTF-8")?;
        let message = std::str::from_utf8(
            snapshot
                .get(kind_name_end..message_end)
                .ok_or("truncated message")?,
        )
        .map_err(|_| "message is not UTF-8")?;
        Ok(DecodedErrorSnapshot {
            kind: snapshot_word(snapshot, 8)?,
            status: snapshot_word(snapshot, 12)?,
            kind_name,
            message,
        })
    }

    fn reset_state() {
        STATE.with(|state| *state.borrow_mut() = State::default());
    }

    fn insert_test_resource(resource: Resource) -> u32 {
        STATE.with(|state| state.borrow_mut().arena.insert(resource).unwrap())
    }

    fn insert_test_buffer(value: &[u8]) -> u32 {
        insert_test_resource(Resource::Buffer(value.to_vec()))
    }

    fn take_test_buffer(handle: u32) -> Vec<u8> {
        let value = STATE.with(|state| state.borrow().arena.buffer(handle).unwrap().to_vec());
        assert_eq!(opendal_mbt_wasm_buffer_release(handle), STATUS_OK);
        value
    }

    fn test_entry_snapshot(path: &str, name: &str, content_length: u64) -> EntrySnapshot {
        let metadata = Metadata::new(opendal::EntryMode::FILE).with_content_length(content_length);
        EntrySnapshot {
            path: path.to_owned(),
            name: name.to_owned(),
            mode: ENTRY_MODE_FILE,
            content_length,
            metadata_snapshot: metadata_snapshot_bytes(&metadata).unwrap(),
        }
    }

    #[test]
    fn opendal_error_kinds_use_the_native_binding_codes() {
        for (kind, expected_code, expected_name) in [
            (ErrorKind::Unexpected, ERROR_UNEXPECTED, "Unexpected"),
            (ErrorKind::Unsupported, ERROR_UNSUPPORTED, "Unsupported"),
            (
                ErrorKind::ConfigInvalid,
                ERROR_CONFIG_INVALID,
                "ConfigInvalid",
            ),
            (ErrorKind::NotFound, ERROR_NOT_FOUND, "NotFound"),
            (
                ErrorKind::PermissionDenied,
                ERROR_PERMISSION_DENIED,
                "PermissionDenied",
            ),
            (
                ErrorKind::IsADirectory,
                ERROR_IS_A_DIRECTORY,
                "IsADirectory",
            ),
            (
                ErrorKind::NotADirectory,
                ERROR_NOT_A_DIRECTORY,
                "NotADirectory",
            ),
            (
                ErrorKind::AlreadyExists,
                ERROR_ALREADY_EXISTS,
                "AlreadyExists",
            ),
            (ErrorKind::RateLimited, ERROR_RATE_LIMITED, "RateLimited"),
            (ErrorKind::IsSameFile, ERROR_IS_SAME_FILE, "IsSameFile"),
            (
                ErrorKind::ConditionNotMatch,
                ERROR_CONDITION_NOT_MATCH,
                "ConditionNotMatch",
            ),
            (
                ErrorKind::RangeNotSatisfied,
                ERROR_RANGE_NOT_SATISFIED,
                "RangeNotSatisfied",
            ),
        ] {
            let snapshot = error_snapshot_bytes(&BridgeError::from(opendal::Error::new(
                kind,
                "stable message",
            )))
            .unwrap();
            let decoded = decode_error_snapshot(&snapshot).unwrap();
            assert_eq!(
                decoded,
                DecodedErrorSnapshot {
                    kind: expected_code,
                    status: ERROR_STATUS_PERMANENT,
                    kind_name: expected_name,
                    message: "stable message",
                }
            );
        }
    }

    #[test]
    fn opendal_error_status_preserves_temporary_and_persistent() {
        for (error, expected_status) in [
            (
                opendal::Error::new(ErrorKind::Unexpected, "temporary").set_temporary(),
                ERROR_STATUS_TEMPORARY,
            ),
            (
                opendal::Error::new(ErrorKind::Unexpected, "persistent").set_persistent(),
                ERROR_STATUS_PERSISTENT,
            ),
        ] {
            let snapshot = error_snapshot_bytes(&BridgeError::from(error)).unwrap();
            assert_eq!(
                decode_error_snapshot(&snapshot).unwrap().status,
                expected_status
            );
        }
    }

    #[test]
    fn binding_errors_use_the_native_binding_codes() {
        for (error, expected_code, expected_name) in [
            (
                BridgeError::InvalidArgument {
                    message: "invalid".to_owned(),
                },
                ERROR_INVALID_ARGUMENT,
                "InvalidArgument",
            ),
            (
                BridgeError::InvalidHandle { handle: 1 },
                ERROR_RESOURCE_CLOSED,
                "ResourceClosed",
            ),
            (
                BridgeError::BufferTooLarge,
                ERROR_BUFFER_TOO_LARGE,
                "BufferTooLarge",
            ),
            (
                BridgeError::WrongResourceType {
                    handle: 1,
                    expected: "buffer",
                    actual: "operator",
                },
                ERROR_ABI_MISMATCH,
                "AbiMismatch",
            ),
        ] {
            let snapshot = error_snapshot_bytes(&error).unwrap();
            let decoded = decode_error_snapshot(&snapshot).unwrap();
            assert_eq!(
                (decoded.kind, decoded.kind_name),
                (expected_code, expected_name)
            );
        }
    }

    #[test]
    fn construction_error_snapshot_redacts_backend_diagnostics() {
        let error = BridgeError::from_construction_error(opendal::Error::new(
            ErrorKind::ConfigInvalid,
            "secret_access_key=do-not-leak",
        ));
        let snapshot = error_snapshot_bytes(&error).unwrap();

        assert_eq!(
            decode_error_snapshot(&snapshot).unwrap().message,
            "operator construction failed"
        );
    }

    #[test]
    fn error_snapshot_take_consumes_error_and_publishes_one_buffer() {
        reset_state();
        let error = insert_test_resource(Resource::Error(BridgeError::from(opendal::Error::new(
            ErrorKind::NotFound,
            "missing object",
        ))));

        let snapshot = opendal_mbt_wasm_error_snapshot_take(error);

        assert_ne!(snapshot, 0);
        assert!(STATE.with(|state| state.borrow().arena.error(error).is_err()));
        assert_eq!(
            decode_error_snapshot(&take_test_buffer(snapshot)).unwrap(),
            DecodedErrorSnapshot {
                kind: ERROR_NOT_FOUND,
                status: ERROR_STATUS_PERMANENT,
                kind_name: "NotFound",
                message: "missing object",
            }
        );
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn error_snapshot_take_preserves_error_when_replacement_has_no_capacity() {
        reset_state();
        let error = encode_handle(0, MAX_GENERATION);
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.arena.slots = (0..MAX_SLOTS)
                .map(|index| Slot {
                    generation: if index == 0 { MAX_GENERATION } else { 0 },
                    resource: (index == 0).then(|| {
                        Resource::Error(BridgeError::InvalidArgument {
                            message: "preserve me".to_owned(),
                        })
                    }),
                })
                .collect();
            state.arena.live = 1;
        });

        assert_eq!(opendal_mbt_wasm_error_snapshot_take(error), 0);
        assert!(STATE.with(|state| state.borrow().arena.error(error).is_ok()));
        reset_state();
    }

    #[test]
    fn error_snapshot_decoder_rejects_malformed_or_truncated_data() {
        let valid = error_snapshot_bytes(&BridgeError::InvalidArgument {
            message: "message".to_owned(),
        })
        .unwrap();
        let mut malformed = valid.clone();
        malformed[0] = b'X';

        assert!(decode_error_snapshot(&malformed).is_err());
        assert!(decode_error_snapshot(&valid[..valid.len() - 1]).is_err());
    }

    #[test]
    fn error_snapshot_decoder_rejects_invalid_utf8() {
        let mut snapshot = error_snapshot_bytes(&BridgeError::InvalidArgument {
            message: "message".to_owned(),
        })
        .unwrap();
        snapshot[ERROR_SNAPSHOT_HEADER_LENGTH] = 0xff;

        assert_eq!(
            decode_error_snapshot(&snapshot),
            Err("kind name is not UTF-8")
        );
    }

    #[test]
    fn metadata_snapshot_encodes_the_complete_native_shaped_contract() {
        let mut metadata = Metadata::new(opendal::EntryMode::FILE)
            .with_is_current(Some(false))
            .with_is_deleted(true)
            .with_content_length(u64::MAX)
            .with_last_modified(opendal::raw::Timestamp::new(123, 456_789).unwrap())
            .with_cache_control("cache".to_owned())
            .with_content_disposition("disposition".to_owned())
            .with_content_md5("md5".to_owned())
            .with_content_type("type".to_owned())
            .with_etag("etag".to_owned())
            .with_version("version".to_owned());
        metadata.set_content_encoding("encoding");

        let snapshot = metadata_snapshot_bytes(&metadata).unwrap();

        assert_eq!(&snapshot[0..4], METADATA_SNAPSHOT_MAGIC.as_slice());
        assert_eq!(snapshot_word(&snapshot, 4), Ok(METADATA_SNAPSHOT_SCHEMA));
        assert_eq!(snapshot_u64(&snapshot, 8), (1 << 9) - 1);
        assert_eq!(snapshot_word(&snapshot, 16), Ok(ENTRY_MODE_FILE));
        assert_eq!(snapshot_word(&snapshot, 20), Ok(0));
        assert_eq!(snapshot_word(&snapshot, 24), Ok(1));
        assert_eq!(snapshot_word(&snapshot, 28), Ok(0));
        assert_eq!(snapshot_u64(&snapshot, 32), u64::MAX);
        assert_eq!(snapshot_i64(&snapshot, 40), 123);
        assert_eq!(snapshot_word(&snapshot, 48), Ok(456_789));
        assert_eq!(snapshot_word(&snapshot, 52), Ok(0));
        assert_eq!(
            (0..7)
                .map(|index| snapshot_word(&snapshot, 56 + index * 4).unwrap())
                .collect::<Vec<_>>(),
            vec![5, 11, 8, 3, 4, 4, 7]
        );
        assert_eq!(
            &snapshot[METADATA_SNAPSHOT_HEADER_LENGTH..],
            b"cachedispositionencodingmd5typeetagversion"
        );
    }

    #[test]
    fn metadata_snapshot_uses_canonical_zeroes_for_absent_values() {
        let snapshot = metadata_snapshot_bytes(&Metadata::default()).unwrap();

        assert_eq!(snapshot.len(), METADATA_SNAPSHOT_HEADER_LENGTH);
        assert_eq!(snapshot_u64(&snapshot, 8), 0);
        assert!(snapshot[16..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn metadata_snapshot_canonicalizes_pre_epoch_fractional_timestamps() {
        let metadata =
            Metadata::default().with_last_modified(opendal::raw::Timestamp::new(-4, -1).unwrap());

        let snapshot = metadata_snapshot_bytes(&metadata).unwrap();

        assert_eq!(snapshot_u64(&snapshot, 8), METADATA_LAST_MODIFIED_PRESENT);
        assert_eq!(snapshot_i64(&snapshot, 40), -5);
        assert_eq!(snapshot_word(&snapshot, 48), Ok(999_999_999));
    }

    #[test]
    fn metadata_completion_snapshot_take_supports_write_and_stat() {
        for completion in [
            Completion::Write(Ok(Box::new(
                Metadata::new(opendal::EntryMode::FILE).with_content_length(3),
            ))),
            Completion::Stat(Ok(Box::new(
                Metadata::new(opendal::EntryMode::DIR).with_content_length(4),
            ))),
        ] {
            reset_state();
            let completion = insert_test_resource(Resource::Completion(completion));

            let snapshot = opendal_mbt_wasm_completion_take_metadata_snapshot(completion);

            assert_ne!(snapshot, 0);
            assert!(STATE.with(|state| state.borrow().arena.completion(completion).is_err()));
            assert_eq!(
                &take_test_buffer(snapshot)[0..4],
                METADATA_SNAPSHOT_MAGIC.as_slice()
            );
            assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        }
        reset_state();
    }

    #[test]
    fn metadata_completion_snapshot_take_is_failure_atomic_without_capacity() {
        reset_state();
        let completion = encode_handle(0, MAX_GENERATION);
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.arena.slots = (0..MAX_SLOTS)
                .map(|index| Slot {
                    generation: if index == 0 { MAX_GENERATION } else { 0 },
                    resource: (index == 0).then(|| {
                        Resource::Completion(Completion::Stat(Ok(Box::new(Metadata::new(
                            opendal::EntryMode::FILE,
                        )))))
                    }),
                })
                .collect();
            state.arena.live = 1;
        });

        assert_eq!(
            opendal_mbt_wasm_completion_take_metadata_snapshot(completion),
            0
        );
        assert_eq!(
            opendal_mbt_wasm_completion_kind(completion),
            COMPLETION_STAT
        );
        reset_state();
    }

    #[test]
    fn operation_options_preserve_empty_values_and_validate_scalars() {
        reset_state();
        let empty = insert_test_buffer(b"");
        let value = insert_test_buffer(b"value");

        let read = read_options_from_scalars(RANGE_OFFSET_LENGTH, 2, 3, empty, value, 0).unwrap();
        assert_eq!(read.range, BytesRange::new(2, Some(3)));
        assert_eq!(read.version.as_deref(), Some(""));
        assert_eq!(read.if_match.as_deref(), Some("value"));
        assert_eq!(read.if_none_match, None);

        let stat = stat_options_from_scalars(0, empty, value).unwrap();
        assert_eq!(stat.version, None);
        assert_eq!(stat.if_match.as_deref(), Some(""));
        assert_eq!(stat.if_none_match.as_deref(), Some("value"));

        let write = write_options_from_scalars(1, empty, value, 0, empty, value, 0).unwrap();
        assert!(write.append);
        assert_eq!(write.content_type.as_deref(), Some(""));
        assert_eq!(write.content_disposition.as_deref(), Some("value"));
        assert_eq!(write.content_encoding, None);
        assert_eq!(write.cache_control.as_deref(), Some(""));
        assert_eq!(write.if_match.as_deref(), Some("value"));
        assert_eq!(write.if_none_match, None);

        assert!(matches!(
            byte_range(RANGE_FULL, 1, 0),
            Err(BridgeError::InvalidArgument { .. })
        ));
        assert!(matches!(
            byte_range(RANGE_OFFSET_LENGTH, u64::MAX, 1),
            Err(BridgeError::InvalidArgument { .. })
        ));
        assert!(matches!(
            write_options_from_scalars(2, 0, 0, 0, 0, 0, 0),
            Err(BridgeError::InvalidArgument { .. })
        ));

        assert_eq!(opendal_mbt_wasm_buffer_release(empty), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(value), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn sized_buffers_are_zeroed_and_bounded() {
        reset_state();
        let buffer = opendal_mbt_wasm_buffer_new_sized(4);
        assert_ne!(buffer, 0);
        assert_eq!(opendal_mbt_wasm_buffer_len(buffer), 4);
        for index in 0..4 {
            assert_eq!(opendal_mbt_wasm_buffer_get(buffer, index), 0);
        }
        assert_eq!(opendal_mbt_wasm_buffer_release(buffer), STATUS_OK);

        assert_eq!(
            opendal_mbt_wasm_buffer_new_sized((MAX_BUFFER_LENGTH + 1) as u32),
            0
        );
        assert_eq!(opendal_mbt_wasm_last_error_code(), 3);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn transfer_windows_are_non_empty_bounded_and_in_range() {
        assert_eq!(
            checked_buffer_window(MAX_TRANSFER_CHUNK, 0, MAX_TRANSFER_CHUNK as u32).unwrap(),
            0..MAX_TRANSFER_CHUNK
        );
        assert!(matches!(
            checked_buffer_window(1, 0, 0),
            Err(BridgeError::InvalidArgument { .. })
        ));
        assert!(matches!(
            checked_buffer_window(MAX_TRANSFER_CHUNK + 1, 0, (MAX_TRANSFER_CHUNK + 1) as u32,),
            Err(BridgeError::InvalidArgument { .. })
        ));
        assert!(matches!(
            checked_buffer_window(8, 8, 1),
            Err(BridgeError::IndexOutOfBounds { .. })
        ));
        assert!(matches!(
            checked_buffer_window(8, u32::MAX, 1),
            Err(BridgeError::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn forced_pending_is_an_explicit_canary_setting() {
        reset_state();
        assert!(!STATE.with(|state| state.borrow().force_pending_for_canary));
        assert_eq!(opendal_mbt_wasm_canary_set_force_pending(1), STATUS_OK);
        assert!(STATE.with(|state| state.borrow().force_pending_for_canary));
        assert_eq!(opendal_mbt_wasm_canary_set_force_pending(0), STATUS_OK);
        assert!(!STATE.with(|state| state.borrow().force_pending_for_canary));
        assert_eq!(opendal_mbt_wasm_canary_set_force_pending(2), 17);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 17);
        reset_state();
    }

    #[test]
    fn generic_builder_constructs_registered_memory_service() {
        reset_state();
        let schemes = opendal_mbt_wasm_registered_schemes();
        assert_ne!(schemes, 0);
        let schemes = take_test_buffer(schemes);
        let schemes = std::str::from_utf8(&schemes)
            .expect("the registry serializes service schemes as UTF-8")
            .split('\n')
            .collect::<Vec<_>>();
        assert!(schemes.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(schemes.contains(&"memory"));
        assert!(schemes.contains(&"s3"));
        #[cfg(target_arch = "wasm32")]
        assert!(schemes.contains(&"opfs"));

        let scheme = insert_test_buffer(b"memory");
        let builder = opendal_mbt_wasm_operator_builder_new(scheme);
        assert_ne!(builder, 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(scheme), STATUS_OK);

        let key = insert_test_buffer(b"root");
        let value = insert_test_buffer(b"generic-prefix");
        assert_eq!(
            opendal_mbt_wasm_operator_builder_set(builder, key, value),
            STATUS_OK
        );
        assert_eq!(opendal_mbt_wasm_buffer_release(key), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(value), STATUS_OK);

        let operator = opendal_mbt_wasm_operator_builder_build(builder);
        assert_ne!(operator, 0);
        assert_eq!(
            opendal_mbt_wasm_operator_builder_release(builder),
            STATUS_OK
        );
        assert_eq!(
            take_test_buffer(opendal_mbt_wasm_operator_info_scheme(operator)),
            b"memory"
        );
        assert_eq!(
            take_test_buffer(opendal_mbt_wasm_operator_info_root(operator)),
            b"/generic-prefix/"
        );
        let capabilities = opendal_mbt_wasm_operator_info_capability_word(operator, 0);
        assert_eq!(capabilities & (CAP_STAT | CAP_READ | CAP_WRITE), 7);
        assert_eq!(opendal_mbt_wasm_operator_release(operator), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn generic_builder_installs_default_http_transport_for_s3_requests() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "S3 request never reached the default HTTP transport"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("local S3 fixture could not accept a request: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let mut chunk = [0_u8; 1024];
                let length = stream.read(&mut chunk).unwrap();
                assert_ne!(length, 0, "S3 request ended before its headers");
                request.extend_from_slice(&chunk[..length]);
                assert!(
                    request.len() <= 16 * 1024,
                    "S3 request headers are too large"
                );
            }
            let request = std::str::from_utf8(&request).unwrap();
            assert!(
                request.starts_with("GET /probe-bucket/delayed-missing.txt HTTP/1.1\r\n"),
                "unexpected S3 request: {request}"
            );
            let body = concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                "<Error><Code>NoSuchKey</Code><Message>missing probe object</Message>",
                "<Key>delayed-missing.txt</Key><RequestId>probe-request</RequestId>",
                "<HostId>probe-host</HostId></Error>"
            );
            write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nx-amz-request-id: probe-request\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let operator = build_operator(OperatorBuilder {
            scheme: "s3".to_owned(),
            config: vec![
                ("bucket".to_owned(), "probe-bucket".to_owned()),
                ("region".to_owned(), "us-east-1".to_owned()),
                ("endpoint".to_owned(), format!("http://{address}")),
                ("access_key_id".to_owned(), "probe-access-key".to_owned()),
                (
                    "secret_access_key".to_owned(),
                    "probe-secret-key".to_owned(),
                ),
                ("disable_config_load".to_owned(), "true".to_owned()),
                ("disable_ec2_metadata".to_owned(), "true".to_owned()),
            ],
            config_bytes: 0,
        })
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = runtime.block_on(operator.read("delayed-missing.txt"));

        server.join().unwrap();
        assert!(
            matches!(result, Err(ref error) if error.kind() == ErrorKind::NotFound),
            "S3 read did not use the installed default HTTP transport: {result:?}"
        );
    }

    #[test]
    fn generic_builder_redacts_unregistered_scheme_diagnostics_without_leaking() {
        reset_state();
        let scheme = insert_test_buffer(b"not-compiled-in");
        let builder = opendal_mbt_wasm_operator_builder_new(scheme);
        assert_ne!(builder, 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(scheme), STATUS_OK);

        assert_eq!(opendal_mbt_wasm_operator_builder_build(builder), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 8);
        let error = opendal_mbt_wasm_last_error_take();
        assert_ne!(error, 0);
        let snapshot = opendal_mbt_wasm_error_snapshot_take(error);
        assert_eq!(
            decode_error_snapshot(&take_test_buffer(snapshot))
                .unwrap()
                .message,
            "operator construction failed"
        );
        assert_eq!(
            opendal_mbt_wasm_operator_builder_release(builder),
            STATUS_OK
        );
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn suffix_capability_and_guard_use_the_native_memory_service() {
        reset_state();
        let masked_memory = Operator::new(Memory::default()).unwrap().layer(
            opendal::layers::CapabilityOverrideLayer::new(|mut capability| {
                capability.read_with_suffix = false;
                capability
            }),
        );
        let (context, service) = masked_memory.into_parts();
        let operator =
            Operator::from_parts(context, service).layer(opendal::layers::SimulateLayer::default());
        let operator_handle = insert_test_resource(Resource::Operator(operator.clone()));
        assert_ne!(operator_handle, 0);
        assert_eq!(
            opendal_mbt_wasm_operator_info_capability_word(operator_handle, 0) & CAP_READ_SUFFIX,
            0
        );

        assert!(operator.info().capability().read_with_suffix);
        assert!(!operator.base_service().capability_dyn().read_with_suffix);
        assert_eq!(
            ensure_suffix_is_native(&operator, &BytesRange::suffix(1))
                .unwrap_err()
                .kind(),
            ERROR_UNSUPPORTED
        );

        let path = insert_test_buffer(b"value");
        assert_eq!(
            opendal_mbt_wasm_operator_read_options_start_v1(
                operator_handle,
                path,
                RANGE_SUFFIX,
                0,
                1,
                0,
                0,
                0,
            ),
            0
        );
        assert_eq!(
            STATE.with(|state| state.borrow().last_error.as_ref().unwrap().kind()),
            ERROR_UNSUPPORTED
        );
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(path), STATUS_OK);
        assert_eq!(
            opendal_mbt_wasm_operator_release(operator_handle),
            STATUS_OK
        );
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn generic_builder_rejects_duplicate_and_unbounded_config() {
        reset_state();
        let scheme = insert_test_buffer(b"memory");
        let builder = opendal_mbt_wasm_operator_builder_new(scheme);
        assert_ne!(builder, 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(scheme), STATUS_OK);

        let upper_key = insert_test_buffer(b"Root");
        let first_value = insert_test_buffer(b"first");
        assert_eq!(
            opendal_mbt_wasm_operator_builder_set(builder, upper_key, first_value),
            STATUS_OK
        );
        assert_eq!(opendal_mbt_wasm_buffer_release(upper_key), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(first_value), STATUS_OK);

        let lower_key = insert_test_buffer(b"root");
        let second_value = insert_test_buffer(b"second");
        assert_eq!(
            opendal_mbt_wasm_operator_builder_set(builder, lower_key, second_value),
            17
        );
        assert_eq!(opendal_mbt_wasm_last_error_code(), 17);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(lower_key), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(second_value), STATUS_OK);
        STATE.with(|state| {
            let state = state.borrow();
            let builder = state.arena.operator_builder(builder).unwrap();
            assert_eq!(builder.config.len(), 1);
            assert_eq!(builder.config_bytes, 9);
        });
        assert_eq!(
            opendal_mbt_wasm_operator_builder_release(builder),
            STATUS_OK
        );

        let mut byte_limited = OperatorBuilder {
            scheme: "memory".to_owned(),
            config: Vec::new(),
            config_bytes: MAX_CONFIG_BYTES,
        };
        assert!(matches!(
            push_operator_config(&mut byte_limited, "x".to_owned(), String::new()),
            Err(BridgeError::InvalidArgument { .. })
        ));
        assert!(byte_limited.config.is_empty());
        assert_eq!(byte_limited.config_bytes, MAX_CONFIG_BYTES);

        let mut entry_limited = OperatorBuilder {
            scheme: "memory".to_owned(),
            config: vec![("existing".to_owned(), String::new()); MAX_CONFIG_ENTRIES],
            config_bytes: 0,
        };
        assert!(matches!(
            push_operator_config(&mut entry_limited, "new".to_owned(), String::new()),
            Err(BridgeError::InvalidArgument { .. })
        ));
        assert_eq!(entry_limited.config.len(), MAX_CONFIG_ENTRIES);
        reset_state();
    }

    #[test]
    fn unit_mutation_completions_are_typed_and_releasable() {
        reset_state();
        for (completion, expected_kind) in [
            (Completion::CreateDir(Ok(())), COMPLETION_CREATE_DIR),
            (Completion::Delete(Ok(())), COMPLETION_DELETE),
        ] {
            let task =
                insert_test_resource(Resource::Task(Task::Ready(Box::new(completion), None)));
            let completion = opendal_mbt_wasm_task_take(task);
            assert_ne!(completion, 0);
            assert_eq!(opendal_mbt_wasm_completion_kind(completion), expected_kind);
            assert_eq!(opendal_mbt_wasm_completion_status(completion), STATUS_OK);
            assert_eq!(opendal_mbt_wasm_completion_release(completion), STATUS_OK);
            assert_eq!(opendal_mbt_wasm_task_release(task), STATUS_OK);
        }
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn bounded_entry_snapshot_collection_is_failure_atomic() {
        let mut entries = Vec::new();
        let mut bytes = 0;
        let bounds = ListBounds {
            max_entries: 2,
            max_bytes: 172,
        };
        assert!(
            push_entry_snapshot(
                &mut entries,
                &mut bytes,
                test_entry_snapshot("a", "a", 1),
                bounds,
            )
            .is_ok()
        );
        assert!(
            push_entry_snapshot(
                &mut entries,
                &mut bytes,
                test_entry_snapshot("b", "b", 1),
                bounds,
            )
            .is_ok()
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(bytes, 172);

        assert!(matches!(
            push_entry_snapshot(
                &mut entries,
                &mut bytes,
                test_entry_snapshot("c", "c", 1),
                bounds,
            ),
            Err(BridgeError::ListTooLarge)
        ));
        assert_eq!(entries.len(), 2);
        assert_eq!(bytes, 172);

        let mut bytes_limited = Vec::new();
        let mut byte_count = 0;
        assert!(matches!(
            push_entry_snapshot(
                &mut bytes_limited,
                &mut byte_count,
                test_entry_snapshot("abc", "def", 3),
                ListBounds {
                    max_entries: 2,
                    max_bytes: 89,
                },
            ),
            Err(BridgeError::ListTooLarge)
        ));
        assert!(bytes_limited.is_empty());
        assert_eq!(byte_count, 0);
    }

    #[test]
    fn entry_list_take_and_index_access_are_owned_and_checked() {
        reset_state();
        let completion = insert_test_resource(Resource::Completion(Completion::List(Ok(vec![
            EntrySnapshot {
                path: "tree/value.bin".to_owned(),
                name: "value.bin".to_owned(),
                mode: ENTRY_MODE_FILE,
                content_length: 3,
                metadata_snapshot: metadata_snapshot_bytes(
                    &Metadata::new(opendal::EntryMode::FILE).with_content_length(3),
                )
                .unwrap(),
            },
        ]))));

        assert_eq!(opendal_mbt_wasm_completion_take_buffer(completion), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 2);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(
            opendal_mbt_wasm_completion_kind(completion),
            COMPLETION_LIST
        );

        let entries = opendal_mbt_wasm_completion_take_entry_list(completion);
        assert_ne!(entries, 0);
        assert_eq!(opendal_mbt_wasm_entry_list_len(entries), 1);
        assert_eq!(
            take_test_buffer(opendal_mbt_wasm_entry_list_path(entries, 0)),
            b"tree/value.bin"
        );
        assert_eq!(
            take_test_buffer(opendal_mbt_wasm_entry_list_name(entries, 0)),
            b"value.bin"
        );
        assert_eq!(
            opendal_mbt_wasm_entry_list_mode(entries, 0),
            ENTRY_MODE_FILE as i32
        );
        assert_eq!(opendal_mbt_wasm_entry_list_content_length(entries, 0), 3);
        let metadata = opendal_mbt_wasm_entry_list_metadata_snapshot(entries, 0);
        assert_ne!(metadata, 0);
        assert_eq!(
            &take_test_buffer(metadata)[0..4],
            METADATA_SNAPSHOT_MAGIC.as_slice()
        );
        assert_eq!(opendal_mbt_wasm_entry_list_metadata_snapshot(entries, 0), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 17);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_entry_list_content_length(entries, 0), 3);
        assert_eq!(opendal_mbt_wasm_entry_list_mode(entries, 1), SCALAR_ERROR);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 4);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_entry_list_release(entries), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn entry_metadata_snapshot_take_is_failure_atomic_and_single_use() {
        reset_state();
        let entries = encode_handle(0, MAX_GENERATION);
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.arena.slots = (0..MAX_SLOTS)
                .map(|index| Slot {
                    generation: if index == 0 { MAX_GENERATION } else { 0 },
                    resource: (index == 0).then(|| {
                        Resource::EntryList(vec![test_entry_snapshot("value.bin", "value.bin", 3)])
                    }),
                })
                .collect();
            state.arena.live = 1;
        });

        assert_eq!(opendal_mbt_wasm_entry_list_metadata_snapshot(entries, 0), 0);
        assert!(STATE.with(|state| {
            !state.borrow().arena.entry_list(entries).unwrap()[0]
                .metadata_snapshot
                .is_empty()
        }));
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);

        STATE.with(|state| state.borrow_mut().arena.slots[1].generation = 1);
        let snapshot = opendal_mbt_wasm_entry_list_metadata_snapshot(entries, 0);
        assert_ne!(snapshot, 0);
        assert_eq!(
            &take_test_buffer(snapshot)[0..4],
            METADATA_SNAPSHOT_MAGIC.as_slice()
        );
        assert_eq!(opendal_mbt_wasm_entry_list_metadata_snapshot(entries, 0), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 17);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_entry_list_release(entries), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn ready_task_moves_its_completion_exactly_once() {
        reset_state();
        let task = insert_test_resource(Resource::Task(Task::Ready(
            Box::new(Completion::Read(Ok(vec![0, 255, 42]))),
            None,
        )));

        let completion = opendal_mbt_wasm_task_take(task);
        assert_ne!(completion, 0);
        assert_eq!(opendal_mbt_wasm_task_state(task), TASK_CONSUMED);
        assert_eq!(opendal_mbt_wasm_task_take(task), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 13);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);

        let buffer = opendal_mbt_wasm_completion_take_buffer(completion);
        assert_ne!(buffer, 0);
        assert_eq!(opendal_mbt_wasm_buffer_len(buffer), 3);
        assert_eq!(opendal_mbt_wasm_buffer_get(buffer, 1), 255);
        assert_eq!(opendal_mbt_wasm_buffer_release(buffer), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_release(task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_release(task), 1);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn typed_take_failure_preserves_the_completion() {
        reset_state();
        let completion =
            insert_test_resource(Resource::Completion(Completion::Read(Ok(vec![7, 8, 9]))));

        assert_eq!(opendal_mbt_wasm_completion_take_metadata(completion), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 2);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(
            opendal_mbt_wasm_completion_kind(completion),
            COMPLETION_READ
        );

        let buffer = opendal_mbt_wasm_completion_take_buffer(completion);
        assert_ne!(buffer, 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(buffer), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn owned_completion_error_snapshot_is_isolated_from_later_sticky_errors() {
        reset_state();
        let completion = insert_test_resource(Resource::Completion(Completion::Read(Err(
            BridgeError::from(opendal::Error::new(ErrorKind::NotFound, "missing")),
        ))));

        assert_eq!(opendal_mbt_wasm_completion_take_buffer(completion), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 7);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);

        let error = opendal_mbt_wasm_completion_take_error(completion);
        assert_ne!(error, 0);
        assert_eq!(opendal_mbt_wasm_error_code(error), 7);
        assert_eq!(opendal_mbt_wasm_buffer_get(0, 0), SCALAR_ERROR);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 1);

        let snapshot = opendal_mbt_wasm_error_snapshot_take(error);
        assert_eq!(
            decode_error_snapshot(&take_test_buffer(snapshot)).unwrap(),
            DecodedErrorSnapshot {
                kind: ERROR_NOT_FOUND,
                status: ERROR_STATUS_PERMANENT,
                kind_name: "NotFound",
                message: "missing",
            }
        );
        assert_eq!(opendal_mbt_wasm_last_error_code(), 1);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn cancellation_and_release_make_late_completion_inert() {
        reset_state();
        let stale = insert_test_resource(Resource::Task(Task::Pending(None)));

        assert_eq!(opendal_mbt_wasm_task_cancel(stale), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_state(stale), TASK_CANCELLED);
        publish_task(
            stale,
            Completion::Write(Ok(Box::new(Metadata::new(opendal::EntryMode::FILE)))),
        );
        assert_eq!(opendal_mbt_wasm_task_state(stale), TASK_CANCELLED);
        assert_eq!(opendal_mbt_wasm_task_release(stale), STATUS_OK);

        let current = opendal_mbt_wasm_buffer_new();
        assert_ne!(current, 0);
        assert_ne!(current, stale);
        publish_task(
            stale,
            Completion::Write(Ok(Box::new(Metadata::new(opendal::EntryMode::FILE)))),
        );
        assert_eq!(opendal_mbt_wasm_buffer_len(current), 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(current), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn cancellation_wins_over_an_unclaimed_ready_result() {
        reset_state();
        let task = insert_test_resource(Resource::Task(Task::Ready(
            Box::new(Completion::Write(Ok(Box::new(Metadata::new(
                opendal::EntryMode::FILE,
            ))))),
            None,
        )));

        assert_eq!(opendal_mbt_wasm_task_cancel(task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_state(task), TASK_CANCELLED);
        assert_eq!(opendal_mbt_wasm_task_take(task), 0);
        assert_eq!(opendal_mbt_wasm_task_release(task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn teardown_is_permanent_and_late_completion_cannot_revive_state() {
        reset_state();
        let task = insert_test_resource(Resource::Task(Task::Pending(None)));

        assert_eq!(opendal_mbt_wasm_teardown(), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_teardown(), STATUS_OK);
        publish_task(
            task,
            Completion::Write(Ok(Box::new(Metadata::new(opendal::EntryMode::FILE)))),
        );
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);

        assert_eq!(opendal_mbt_wasm_buffer_new(), 0);
        assert_eq!(opendal_mbt_wasm_last_error_code(), 14);
        assert_eq!(opendal_mbt_wasm_last_error_take(), 0);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn streaming_feature_bits_and_completion_kinds_are_stable() {
        assert_eq!(ABI_VERSION, 0x0001_0007);
        let features = opendal_mbt_wasm_feature_flags();
        assert_ne!(features & FEATURE_COPY_RENAME, 0);
        assert_ne!(features & FEATURE_STATEFUL_READER, 0);
        assert_ne!(features & FEATURE_STATEFUL_WRITER, 0);
        assert_ne!(features & FEATURE_STATEFUL_LISTER, 0);
        assert_ne!(features & FEATURE_EXPLICIT_EOF, 0);
        assert_eq!(
            Completion::Copy(Ok(Box::new(Metadata::new(opendal::EntryMode::FILE)))).kind(),
            7
        );
        assert_eq!(Completion::Rename(Ok(())).kind(), 8);
        assert_eq!(Completion::ReadStreamNext(Ok(None)).kind(), 10);
        assert_eq!(Completion::WriterWrite(Ok(())).kind(), 12);
        assert_eq!(
            Completion::WriterFinish(Ok(Box::new(Metadata::new(opendal::EntryMode::FILE)))).kind(),
            13
        );
        assert_eq!(Completion::WriterAbort(Ok(())).kind(), 14);
        assert_eq!(Completion::ListerNext(Ok(None)).kind(), 16);
    }

    #[test]
    fn stream_chunk_limits_accept_the_boundary_and_reject_one_more() {
        reset_state();
        assert!(
            read_stream_options_from_scalars(MAX_TRANSFER_CHUNK as u32, RANGE_FULL, 0, 0, 0, 0, 0,)
                .is_ok()
        );
        assert!(matches!(
            read_stream_options_from_scalars(0, RANGE_FULL, 0, 0, 0, 0, 0),
            Err(BridgeError::InvalidArgument { .. })
        ));
        assert!(matches!(
            read_stream_options_from_scalars(
                MAX_TRANSFER_CHUNK as u32 + 1,
                RANGE_FULL,
                0,
                0,
                0,
                0,
                0,
            ),
            Err(BridgeError::InvalidArgument { .. })
        ));

        let accepted = insert_test_resource(Resource::Buffer(vec![0; MAX_TRANSFER_CHUNK]));
        let rejected = insert_test_resource(Resource::Buffer(vec![0; MAX_TRANSFER_CHUNK + 1]));
        assert_eq!(
            owned_writer_chunk(accepted).unwrap().len(),
            MAX_TRANSFER_CHUNK
        );
        assert!(matches!(
            owned_writer_chunk(rejected),
            Err(BridgeError::BufferTooLarge)
        ));
        assert_eq!(opendal_mbt_wasm_buffer_release(accepted), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(rejected), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn explicit_eof_is_non_consuming_and_distinguishes_empty_values() {
        reset_state();
        let eof = insert_test_resource(Resource::Completion(Completion::ReadStreamNext(Ok(None))));
        assert_eq!(opendal_mbt_wasm_completion_is_end(eof), 1);
        assert_eq!(opendal_mbt_wasm_completion_is_end(eof), 1);
        assert_eq!(
            opendal_mbt_wasm_completion_kind(eof),
            COMPLETION_READER_NEXT
        );
        assert_eq!(opendal_mbt_wasm_completion_take_buffer(eof), 0);

        let empty = insert_test_resource(Resource::Completion(Completion::ReadStreamNext(Ok(
            Some(Vec::new()),
        ))));
        assert_eq!(opendal_mbt_wasm_completion_is_end(empty), 0);
        assert_eq!(opendal_mbt_wasm_completion_is_end(empty), 0);
        let buffer = opendal_mbt_wasm_completion_take_buffer(empty);
        assert_ne!(buffer, 0);
        assert_eq!(opendal_mbt_wasm_buffer_len(buffer), 0);
        assert_eq!(opendal_mbt_wasm_buffer_release(buffer), STATUS_OK);

        let lister_eof =
            insert_test_resource(Resource::Completion(Completion::ListerNext(Ok(None))));
        assert_eq!(opendal_mbt_wasm_completion_is_end(lister_eof), 1);
        assert_eq!(opendal_mbt_wasm_completion_is_end(lister_eof), 1);
        assert_eq!(opendal_mbt_wasm_completion_take_entry_list(lister_eof), 0);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn copy_rename_and_metadata_completions_match_opendal() {
        reset_state();
        let operator = Operator::new(Memory::default()).unwrap();
        poll_once(operator.write("source", b"payload".to_vec()))
            .unwrap()
            .unwrap();
        assert!(matches!(
            poll_once(operator.copy("source", "copy")).unwrap(),
            Err(error) if error.kind() == ErrorKind::Unsupported
        ));
        assert!(matches!(
            poll_once(operator.rename("source", "renamed")).unwrap(),
            Err(error) if error.kind() == ErrorKind::Unsupported
        ));

        let metadata = Metadata::new(opendal::EntryMode::FILE).with_content_length(7);
        let completion = insert_test_resource(Resource::Completion(Completion::Copy(Ok(
            Box::new(metadata),
        ))));
        assert_eq!(
            opendal_mbt_wasm_completion_kind(completion),
            COMPLETION_COPY
        );
        let snapshot = opendal_mbt_wasm_completion_take_metadata_snapshot(completion);
        assert_ne!(snapshot, 0);
        assert_eq!(snapshot_u64(&take_test_buffer(snapshot), 32), 7);

        let completion = insert_test_resource(Resource::Completion(Completion::Rename(Ok(()))));
        assert_eq!(
            opendal_mbt_wasm_completion_kind(completion),
            COMPLETION_RENAME
        );
        assert_eq!(opendal_mbt_wasm_completion_status(completion), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_completion_release(completion), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn actual_reader_is_bounded_and_reaches_explicit_eof() {
        reset_state();
        let operator = Operator::new(Memory::default()).unwrap();
        let data = vec![9; MAX_TRANSFER_CHUNK * 2 + 17];
        poll_once(operator.write("large", data.clone()))
            .unwrap()
            .unwrap();
        let reader = poll_once(operator.reader_options(
            "large",
            options::ReaderOptions {
                chunk: Some(MAX_TRANSFER_CHUNK),
                ..Default::default()
            },
        ))
        .unwrap()
        .unwrap();
        let stream = poll_once(reader.into_stream(BytesRange::default()))
            .unwrap()
            .unwrap();
        let mut cursor = Box::new(ReadStreamCursor {
            stream,
            pending: None,
            max_chunk: MAX_TRANSFER_CHUNK,
        });
        let mut collected = Vec::new();
        loop {
            let (update, completion) = poll_once(read_stream_next(cursor)).unwrap();
            match completion {
                Completion::ReadStreamNext(Ok(Some(chunk))) => {
                    assert!(!chunk.is_empty());
                    assert!(chunk.len() <= MAX_TRANSFER_CHUNK);
                    collected.extend_from_slice(&chunk);
                    let ResourceUpdate::ReadStream(ReadStreamUpdate::Open(next)) = update else {
                        panic!("successful chunk must reopen the stream")
                    };
                    cursor = next;
                }
                Completion::ReadStreamNext(Ok(None)) => {
                    assert!(matches!(
                        update,
                        ResourceUpdate::ReadStream(ReadStreamUpdate::End)
                    ));
                    break;
                }
                Completion::ReadStreamNext(Err(error)) => panic!("unexpected read error: {error}"),
                _ => panic!("unexpected completion kind"),
            }
        }
        assert_eq!(collected, data);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn actual_writer_and_lister_use_stateful_opendal_resources() {
        reset_state();
        let operator = Operator::new(Memory::default()).unwrap();
        let mut writer = poll_once(operator.writer("tree/value")).unwrap().unwrap();
        poll_once(writer.write(vec![3; MAX_TRANSFER_CHUNK]))
            .unwrap()
            .unwrap();
        let metadata = poll_once(writer.close()).unwrap().unwrap();
        assert_eq!(metadata.content_length(), MAX_TRANSFER_CHUNK as u64);

        let mut lister = poll_once(operator.lister("tree/")).unwrap().unwrap();
        let entry = poll_once(lister.try_next()).unwrap().unwrap().unwrap();
        let entry = entry_snapshot(entry).unwrap();
        assert_eq!(entry.path, "tree/value");
        assert_eq!(entry.content_length, MAX_TRANSFER_CHUNK as u64);
        assert!(poll_once(lister.try_next()).unwrap().unwrap().is_none());

        let mut aborted = poll_once(operator.writer("aborted")).unwrap().unwrap();
        poll_once(aborted.write(b"uncommitted".to_vec()))
            .unwrap()
            .unwrap();
        poll_once(aborted.abort()).unwrap().unwrap();
        assert!(matches!(
            poll_once(operator.stat("aborted")).unwrap(),
            Err(error) if error.kind() == ErrorKind::NotFound
        ));
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn resource_busy_closed_and_error_states_are_explicit() {
        reset_state();
        let reader = insert_test_resource(Resource::ReadStream(ReadStreamState::Busy(1)));
        let writer = insert_test_resource(Resource::Writer(WriterState::Busy(2)));
        let lister = insert_test_resource(Resource::Lister(ListerState::Busy(3)));
        assert!(matches!(
            reserve_read_stream_next(reader),
            Err(BridgeError::ResourceBusy { handle }) if handle == reader
        ));
        assert!(matches!(
            reserve_writer_call(writer),
            Err(BridgeError::ResourceBusy { handle }) if handle == writer
        ));
        assert!(matches!(
            reserve_lister_next(lister),
            Err(BridgeError::ResourceBusy { handle }) if handle == lister
        ));
        assert_eq!(
            BridgeError::ResourceBusy { handle: writer }.kind(),
            ERROR_RESOURCE_BUSY
        );

        STATE.with(|state| {
            let mut state = state.borrow_mut();
            *state.arena.read_stream_mut(reader).unwrap() = ReadStreamState::End;
            *state.arena.writer_mut(writer).unwrap() = WriterState::Aborted;
            *state.arena.lister_mut(lister).unwrap() = ListerState::Failed;
        });
        assert!(matches!(
            reserve_read_stream_next(reader),
            Err(BridgeError::ResourceClosed { .. })
        ));
        assert!(matches!(
            reserve_writer_call(writer),
            Err(BridgeError::ResourceClosed { .. })
        ));
        assert!(matches!(
            reserve_lister_next(lister),
            Err(BridgeError::ResourceClosed { .. })
        ));
        assert_eq!(opendal_mbt_wasm_read_stream_release(reader), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_writer_release(writer), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_lister_release(lister), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn cancellation_and_release_terminalize_leased_resources_without_resurrection() {
        reset_state();
        let operator = Operator::new(Memory::default()).unwrap();
        let writer = poll_once(operator.writer("cancelled")).unwrap().unwrap();
        let writer_handle = insert_test_resource(Resource::Writer(WriterState::Busy(1)));
        let lease = ResourceLease {
            handle: writer_handle,
            kind: ResourceKind::Writer,
            operation: 1,
        };
        let task = insert_test_resource(Resource::Task(Task::Pending(Some(lease))));
        assert_eq!(opendal_mbt_wasm_task_cancel(task), STATUS_OK);
        assert!(STATE.with(|state| matches!(
            state.borrow_mut().arena.writer_mut(writer_handle).unwrap(),
            WriterState::Failed
        )));
        publish_resource_task(
            task,
            lease,
            ResourceUpdate::Writer(WriterUpdate::Open(Box::new(writer))),
            Completion::WriterWrite(Ok(())),
        );
        assert_eq!(opendal_mbt_wasm_task_state(task), TASK_CANCELLED);
        assert!(STATE.with(|state| matches!(
            state.borrow_mut().arena.writer_mut(writer_handle).unwrap(),
            WriterState::Failed
        )));
        assert_eq!(opendal_mbt_wasm_task_release(task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_writer_release(writer_handle), STATUS_OK);

        let late_writer = poll_once(operator.writer("late")).unwrap().unwrap();
        let stale = insert_test_resource(Resource::Writer(WriterState::Busy(2)));
        let stale_lease = ResourceLease {
            handle: stale,
            kind: ResourceKind::Writer,
            operation: 2,
        };
        let late_task = insert_test_resource(Resource::Task(Task::Pending(Some(stale_lease))));
        assert_eq!(opendal_mbt_wasm_writer_release(stale), STATUS_OK);
        let current = opendal_mbt_wasm_buffer_new();
        assert_ne!(current, stale);
        publish_resource_task(
            late_task,
            stale_lease,
            ResourceUpdate::Writer(WriterUpdate::Open(Box::new(late_writer))),
            Completion::WriterWrite(Ok(())),
        );
        assert_eq!(opendal_mbt_wasm_buffer_len(current), 0);
        assert_eq!(opendal_mbt_wasm_task_state(late_task), TASK_READY);
        let completion = opendal_mbt_wasm_task_take(late_task);
        assert_eq!(
            opendal_mbt_wasm_completion_kind(completion),
            COMPLETION_WRITER_WRITE
        );
        assert_eq!(opendal_mbt_wasm_completion_release(completion), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_release(late_task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_buffer_release(current), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn old_ready_task_cannot_terminalize_a_newer_writer_operation() {
        for cancel_old_task in [true, false] {
            reset_state();
            let operator = Operator::new(Memory::default()).unwrap();
            let writer = poll_once(operator.writer("operation-token"))
                .unwrap()
                .unwrap();
            let writer_handle =
                insert_test_resource(Resource::Writer(WriterState::Open(Box::new(writer), 0)));

            let (task_a, lease_a, writer_a) = reserve_writer_call(writer_handle).unwrap();
            publish_resource_task(
                task_a,
                lease_a,
                ResourceUpdate::Writer(WriterUpdate::Open(writer_a)),
                Completion::WriterWrite(Ok(())),
            );
            assert_eq!(opendal_mbt_wasm_task_state(task_a), TASK_READY);

            let (task_b, lease_b, writer_b) = reserve_writer_call(writer_handle).unwrap();
            assert_ne!(lease_a.operation, lease_b.operation);
            assert!(STATE.with(|state| matches!(
                state.borrow_mut().arena.writer_mut(writer_handle).unwrap(),
                WriterState::Busy(operation) if *operation == lease_b.operation
            )));

            if cancel_old_task {
                assert_eq!(opendal_mbt_wasm_task_cancel(task_a), STATUS_OK);
            } else {
                assert_eq!(opendal_mbt_wasm_task_release(task_a), STATUS_OK);
            }
            assert!(STATE.with(|state| matches!(
                state.borrow_mut().arena.writer_mut(writer_handle).unwrap(),
                WriterState::Busy(operation) if *operation == lease_b.operation
            )));

            publish_resource_task(
                task_b,
                lease_b,
                ResourceUpdate::Writer(WriterUpdate::Open(writer_b)),
                Completion::WriterWrite(Ok(())),
            );
            assert!(STATE.with(|state| matches!(
                state.borrow_mut().arena.writer_mut(writer_handle).unwrap(),
                WriterState::Open(_, operation) if *operation == lease_b.operation
            )));
            let completion_b = opendal_mbt_wasm_task_take(task_b);
            assert_eq!(
                opendal_mbt_wasm_completion_kind(completion_b),
                COMPLETION_WRITER_WRITE
            );
            assert_eq!(opendal_mbt_wasm_completion_release(completion_b), STATUS_OK);
            if cancel_old_task {
                assert_eq!(opendal_mbt_wasm_task_release(task_a), STATUS_OK);
            }
            assert_eq!(opendal_mbt_wasm_task_release(task_b), STATUS_OK);
            assert_eq!(opendal_mbt_wasm_writer_release(writer_handle), STATUS_OK);
            assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        }
        reset_state();
    }

    #[test]
    fn failed_resource_completion_is_owned_and_terminal() {
        reset_state();
        let writer = insert_test_resource(Resource::Writer(WriterState::Busy(1)));
        let lease = ResourceLease {
            handle: writer,
            kind: ResourceKind::Writer,
            operation: 1,
        };
        let task = insert_test_resource(Resource::Task(Task::Pending(Some(lease))));
        publish_resource_task(
            task,
            lease,
            ResourceUpdate::Writer(WriterUpdate::Failed),
            Completion::WriterWrite(Err(BridgeError::from(opendal::Error::new(
                ErrorKind::Unexpected,
                "write failed",
            )))),
        );
        assert!(STATE.with(|state| matches!(
            state.borrow_mut().arena.writer_mut(writer).unwrap(),
            WriterState::Failed
        )));
        let completion = opendal_mbt_wasm_task_take(task);
        assert_eq!(opendal_mbt_wasm_completion_status(completion), 8);
        let error = opendal_mbt_wasm_completion_take_error(completion);
        assert_ne!(error, 0);
        assert_eq!(opendal_mbt_wasm_error_code(error), 8);
        assert_eq!(opendal_mbt_wasm_error_release(error), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_release(task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_writer_release(writer), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn typed_open_completions_transfer_each_opendal_resource_once() {
        reset_state();
        let operator = Operator::new(Memory::default()).unwrap();
        poll_once(operator.write("tree/value", b"value".to_vec()))
            .unwrap()
            .unwrap();

        let reader = poll_once(operator.reader("tree/value")).unwrap().unwrap();
        let stream = poll_once(reader.into_stream(BytesRange::default()))
            .unwrap()
            .unwrap();
        let read_completion = insert_test_resource(Resource::Completion(
            Completion::OpenReadStream(Ok(Box::new(ReadStreamCursor {
                stream,
                pending: None,
                max_chunk: 16,
            }))),
        ));
        let read_stream = opendal_mbt_wasm_completion_take_read_stream(read_completion);
        assert_ne!(read_stream, 0);
        assert_eq!(opendal_mbt_wasm_completion_kind(read_completion), 0);
        assert_eq!(opendal_mbt_wasm_last_error_clear(), STATUS_OK);

        let writer = poll_once(operator.writer("tree/other")).unwrap().unwrap();
        let writer_completion = insert_test_resource(Resource::Completion(Completion::OpenWriter(
            Ok(Box::new(writer)),
        )));
        let writer = opendal_mbt_wasm_completion_take_writer(writer_completion);
        assert_ne!(writer, 0);

        let lister = poll_once(operator.lister("tree/")).unwrap().unwrap();
        let lister_completion = insert_test_resource(Resource::Completion(Completion::OpenLister(
            Ok(Box::new(lister)),
        )));
        let lister = opendal_mbt_wasm_completion_take_lister(lister_completion);
        assert_ne!(lister, 0);

        assert_eq!(opendal_mbt_wasm_read_stream_release(read_stream), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_writer_release(writer), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_lister_release(lister), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }

    #[test]
    fn cancellation_terminalizes_reader_and_lister_leases() {
        reset_state();
        let reader = insert_test_resource(Resource::ReadStream(ReadStreamState::Busy(1)));
        let reader_lease = ResourceLease {
            handle: reader,
            kind: ResourceKind::ReadStream,
            operation: 1,
        };
        let reader_task = insert_test_resource(Resource::Task(Task::Pending(Some(reader_lease))));
        assert_eq!(opendal_mbt_wasm_task_cancel(reader_task), STATUS_OK);
        assert!(STATE.with(|state| matches!(
            state.borrow_mut().arena.read_stream_mut(reader).unwrap(),
            ReadStreamState::Failed
        )));

        let lister = insert_test_resource(Resource::Lister(ListerState::Busy(2)));
        let lister_lease = ResourceLease {
            handle: lister,
            kind: ResourceKind::Lister,
            operation: 2,
        };
        let lister_task = insert_test_resource(Resource::Task(Task::Pending(Some(lister_lease))));
        assert_eq!(opendal_mbt_wasm_task_cancel(lister_task), STATUS_OK);
        assert!(STATE.with(|state| matches!(
            state.borrow_mut().arena.lister_mut(lister).unwrap(),
            ListerState::Failed
        )));

        assert_eq!(opendal_mbt_wasm_task_release(reader_task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_read_stream_release(reader), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_task_release(lister_task), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_lister_release(lister), STATUS_OK);
        assert_eq!(opendal_mbt_wasm_live_handle_count(), 0);
        reset_state();
    }
}
