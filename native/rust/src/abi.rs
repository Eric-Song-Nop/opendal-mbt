//! Exact Rust mirror of `native/include/opendal_mbt.h`.

pub(crate) type Status = u32;

pub(crate) const ABI_MAJOR: u32 = 1;
pub(crate) const ABI_MINOR: u32 = 0;
pub(crate) const ABI_PATCH: u32 = 0;
pub(crate) const STRUCT_VERSION: u32 = 1;

pub(crate) const STATUS_OK: Status = 0;
#[allow(dead_code)]
pub(crate) const STATUS_END: Status = 1;
pub(crate) const STATUS_ERROR: Status = 2;
pub(crate) const STATUS_BUFFER_TOO_SMALL: Status = 3;
pub(crate) const STATUS_ABI_MISMATCH: Status = 4;
pub(crate) const STATUS_PANIC: Status = 5;

pub(crate) const ERROR_UNEXPECTED: u32 = 1;
pub(crate) const ERROR_UNSUPPORTED: u32 = 2;
pub(crate) const ERROR_CONFIG_INVALID: u32 = 3;
pub(crate) const ERROR_NOT_FOUND: u32 = 4;
pub(crate) const ERROR_PERMISSION_DENIED: u32 = 5;
pub(crate) const ERROR_IS_A_DIRECTORY: u32 = 6;
pub(crate) const ERROR_NOT_A_DIRECTORY: u32 = 7;
pub(crate) const ERROR_ALREADY_EXISTS: u32 = 8;
pub(crate) const ERROR_RATE_LIMITED: u32 = 9;
pub(crate) const ERROR_IS_SAME_FILE: u32 = 10;
pub(crate) const ERROR_CONDITION_NOT_MATCH: u32 = 11;
pub(crate) const ERROR_RANGE_NOT_SATISFIED: u32 = 12;
pub(crate) const ERROR_INVALID_ARGUMENT: u32 = 0x1001;
#[allow(dead_code)]
pub(crate) const ERROR_RESOURCE_CLOSED: u32 = 0x1002;
pub(crate) const ERROR_BUFFER_TOO_LARGE: u32 = 0x1003;
#[allow(dead_code)]
pub(crate) const ERROR_ABI_MISMATCH: u32 = 0x1004;

pub(crate) const ERROR_STATUS_PERMANENT: u32 = 1;
pub(crate) const ERROR_STATUS_TEMPORARY: u32 = 2;
pub(crate) const ERROR_STATUS_PERSISTENT: u32 = 3;

pub(crate) const ENTRY_MODE_UNKNOWN: u32 = 0;
pub(crate) const ENTRY_MODE_FILE: u32 = 1;
pub(crate) const ENTRY_MODE_DIRECTORY: u32 = 2;

pub(crate) const RANGE_FULL: u32 = 0;
pub(crate) const RANGE_FROM: u32 = 1;
pub(crate) const RANGE_OFFSET_LENGTH: u32 = 2;
pub(crate) const RANGE_SUFFIX: u32 = 3;

pub(crate) const FEATURE_BASE: u64 = 1 << 0;
pub(crate) const FEATURE_WHOLE_OBJECT: u64 = 1 << 1;

pub(crate) const CAP_STAT: u64 = 1 << 0;
pub(crate) const CAP_READ: u64 = 1 << 1;
pub(crate) const CAP_WRITE: u64 = 1 << 2;
pub(crate) const CAP_CREATE_DIR: u64 = 1 << 3;
pub(crate) const CAP_DELETE: u64 = 1 << 4;
pub(crate) const CAP_LIST: u64 = 1 << 5;
pub(crate) const CAP_COPY: u64 = 1 << 6;
pub(crate) const CAP_RENAME: u64 = 1 << 7;
pub(crate) const CAP_READ_SUFFIX: u64 = 1 << 8;
pub(crate) const CAP_WRITE_APPEND: u64 = 1 << 9;
pub(crate) const CAP_LIST_LIMIT: u64 = 1 << 10;
pub(crate) const CAP_LIST_START_AFTER: u64 = 1 << 11;
pub(crate) const CAP_LIST_RECURSIVE: u64 = 1 << 12;

pub(crate) const READ_VERSION_PRESENT: u64 = 1 << 0;
pub(crate) const READ_IF_MATCH_PRESENT: u64 = 1 << 1;
pub(crate) const READ_IF_NONE_MATCH_PRESENT: u64 = 1 << 2;

pub(crate) const WRITE_APPEND: u64 = 1 << 0;
pub(crate) const WRITE_CONTENT_TYPE_PRESENT: u64 = 1 << 0;
pub(crate) const WRITE_CONTENT_DISPOSITION_PRESENT: u64 = 1 << 1;
pub(crate) const WRITE_CONTENT_ENCODING_PRESENT: u64 = 1 << 2;
pub(crate) const WRITE_CACHE_CONTROL_PRESENT: u64 = 1 << 3;
pub(crate) const WRITE_IF_MATCH_PRESENT: u64 = 1 << 4;
pub(crate) const WRITE_IF_NONE_MATCH_PRESENT: u64 = 1 << 5;

pub(crate) const STAT_VERSION_PRESENT: u64 = 1 << 0;
pub(crate) const STAT_IF_MATCH_PRESENT: u64 = 1 << 1;
pub(crate) const STAT_IF_NONE_MATCH_PRESENT: u64 = 1 << 2;

pub(crate) const DELETE_RECURSIVE: u64 = 1 << 0;
pub(crate) const DELETE_VERSION_PRESENT: u64 = 1 << 0;

pub(crate) const METADATA_IS_CURRENT_PRESENT: u64 = 1 << 0;
pub(crate) const METADATA_LAST_MODIFIED_PRESENT: u64 = 1 << 1;
pub(crate) const METADATA_CACHE_CONTROL_PRESENT: u64 = 1 << 2;
pub(crate) const METADATA_CONTENT_DISPOSITION_PRESENT: u64 = 1 << 3;
pub(crate) const METADATA_CONTENT_ENCODING_PRESENT: u64 = 1 << 4;
pub(crate) const METADATA_CONTENT_MD5_PRESENT: u64 = 1 << 5;
pub(crate) const METADATA_CONTENT_TYPE_PRESENT: u64 = 1 << 6;
pub(crate) const METADATA_ETAG_PRESENT: u64 = 1 << 7;
pub(crate) const METADATA_VERSION_PRESENT: u64 = 1 << 8;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct BytesViewV1 {
    pub(crate) data: *const u8,
    pub(crate) len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct KvV1 {
    pub(crate) key: BytesViewV1,
    pub(crate) value: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ByteRangeV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) kind: u32,
    pub(crate) reserved0: u32,
    pub(crate) offset: u64,
    pub(crate) length: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ReadOptionsV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) present_bits: u64,
    pub(crate) range: ByteRangeV1,
    pub(crate) version: BytesViewV1,
    pub(crate) if_match: BytesViewV1,
    pub(crate) if_none_match: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ReaderOptionsV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) present_bits: u64,
    pub(crate) version: BytesViewV1,
    pub(crate) if_match: BytesViewV1,
    pub(crate) if_none_match: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct WriteOptionsV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) present_bits: u64,
    pub(crate) flags: u64,
    pub(crate) content_type: BytesViewV1,
    pub(crate) content_disposition: BytesViewV1,
    pub(crate) content_encoding: BytesViewV1,
    pub(crate) cache_control: BytesViewV1,
    pub(crate) if_match: BytesViewV1,
    pub(crate) if_none_match: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct StatOptionsV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) present_bits: u64,
    pub(crate) version: BytesViewV1,
    pub(crate) if_match: BytesViewV1,
    pub(crate) if_none_match: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ListOptionsV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) present_bits: u64,
    pub(crate) flags: u64,
    pub(crate) limit: u64,
    pub(crate) start_after: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct DeleteOptionsV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) present_bits: u64,
    pub(crate) flags: u64,
    pub(crate) version: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct TimestampV1 {
    pub(crate) unix_seconds: i64,
    pub(crate) nanoseconds: u32,
    pub(crate) reserved0: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct CapabilityV1 {
    pub(crate) words: [u64; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct MetadataViewV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) present_bits: u64,
    pub(crate) mode: u32,
    pub(crate) is_current: u32,
    pub(crate) is_deleted: u32,
    pub(crate) reserved0: u32,
    pub(crate) content_length: u64,
    pub(crate) last_modified: TimestampV1,
    pub(crate) cache_control: BytesViewV1,
    pub(crate) content_disposition: BytesViewV1,
    pub(crate) content_encoding: BytesViewV1,
    pub(crate) content_md5: BytesViewV1,
    pub(crate) content_type: BytesViewV1,
    pub(crate) etag: BytesViewV1,
    pub(crate) version: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct EntryViewV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) reserved0: u64,
    pub(crate) path: BytesViewV1,
    pub(crate) name: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct OperatorInfoViewV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) reserved0: u64,
    pub(crate) scheme: BytesViewV1,
    pub(crate) root: BytesViewV1,
    pub(crate) name: BytesViewV1,
    pub(crate) capability: CapabilityV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ErrorViewV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) kind: u32,
    pub(crate) status: u32,
    pub(crate) kind_name: BytesViewV1,
    pub(crate) message: BytesViewV1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct LibraryInfoViewV1 {
    pub(crate) struct_size: u32,
    pub(crate) struct_version: u32,
    pub(crate) reserved0: u64,
    pub(crate) binding_version: BytesViewV1,
    pub(crate) opendal_version: BytesViewV1,
    pub(crate) service_profile: BytesViewV1,
}

#[repr(C)]
pub(crate) struct OperatorV1 {
    pub(crate) inner: opendal::blocking::Operator,
}

#[repr(C)]
pub(crate) struct BufferV1 {
    pub(crate) bytes: Vec<u8>,
}

#[repr(C)]
pub(crate) struct ErrorV1 {
    pub(crate) kind: u32,
    pub(crate) status: u32,
    pub(crate) kind_name: String,
    pub(crate) message: String,
}

#[repr(C)]
pub(crate) struct MetadataV1 {
    pub(crate) metadata: opendal::Metadata,
}

#[repr(C)]
pub(crate) struct EntryV1 {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) metadata: opendal::Metadata,
}

#[repr(C)]
pub(crate) struct OperatorInfoV1 {
    pub(crate) scheme: String,
    pub(crate) root: String,
    pub(crate) name: String,
    pub(crate) capability: CapabilityV1,
}

#[repr(C)]
pub(crate) struct ListerV1 {
    _private: [u8; 0],
}

#[repr(C)]
pub(crate) struct ReaderV1 {
    _private: [u8; 0],
}

#[repr(C)]
pub(crate) struct WriterV1 {
    _private: [u8; 0],
}

pub(crate) type LibraryInfoFn = unsafe extern "C" fn(*mut LibraryInfoViewV1) -> Status;
pub(crate) type ErrorViewFn = unsafe extern "C" fn(*const ErrorV1, *mut ErrorViewV1) -> Status;
pub(crate) type ErrorFreeFn = unsafe extern "C" fn(*mut ErrorV1);
pub(crate) type BufferLenFn = unsafe extern "C" fn(*const BufferV1, *mut u64) -> Status;
pub(crate) type BufferCopyFn =
    unsafe extern "C" fn(*const BufferV1, *mut u8, u64, *mut u64) -> Status;
pub(crate) type BufferFreeFn = unsafe extern "C" fn(*mut BufferV1);
pub(crate) type MetadataViewFn =
    unsafe extern "C" fn(*const MetadataV1, *mut MetadataViewV1) -> Status;
pub(crate) type MetadataFreeFn = unsafe extern "C" fn(*mut MetadataV1);
pub(crate) type EntryViewFn = unsafe extern "C" fn(*const EntryV1, *mut EntryViewV1) -> Status;
pub(crate) type EntryMetadataViewFn =
    unsafe extern "C" fn(*const EntryV1, *mut MetadataViewV1) -> Status;
pub(crate) type EntryFreeFn = unsafe extern "C" fn(*mut EntryV1);
pub(crate) type OperatorInfoViewFn =
    unsafe extern "C" fn(*const OperatorInfoV1, *mut OperatorInfoViewV1) -> Status;
pub(crate) type OperatorInfoFreeFn = unsafe extern "C" fn(*mut OperatorInfoV1);
pub(crate) type OperatorNewFn = unsafe extern "C" fn(
    *const BytesViewV1,
    *const KvV1,
    u64,
    *mut *mut OperatorV1,
    *mut *mut OperatorInfoV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorFreeFn = unsafe extern "C" fn(*mut OperatorV1);
pub(crate) type OperatorCheckFn =
    unsafe extern "C" fn(*mut OperatorV1, *mut *mut ErrorV1) -> Status;
pub(crate) type OperatorExistsFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *mut u32,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorStatFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const StatOptionsV1,
    *mut *mut MetadataV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorReadFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const ReadOptionsV1,
    u64,
    *mut *mut BufferV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorWriteFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const BytesViewV1,
    *const WriteOptionsV1,
    *mut *mut MetadataV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorCreateDirFn =
    unsafe extern "C" fn(*mut OperatorV1, *const BytesViewV1, *mut *mut ErrorV1) -> Status;
pub(crate) type OperatorDeleteFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const DeleteOptionsV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorCopyFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const BytesViewV1,
    *mut *mut MetadataV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorRenameFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const BytesViewV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type OperatorListerFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const ListOptionsV1,
    *mut *mut ListerV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type ListerNextFn =
    unsafe extern "C" fn(*mut ListerV1, *mut *mut EntryV1, *mut *mut ErrorV1) -> Status;
pub(crate) type ListerCloseFn = unsafe extern "C" fn(*mut ListerV1);
pub(crate) type ListerFreeFn = unsafe extern "C" fn(*mut ListerV1);
pub(crate) type OperatorReaderFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const ReaderOptionsV1,
    *mut *mut ReaderV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type ReaderReadFn = unsafe extern "C" fn(
    *mut ReaderV1,
    *const ByteRangeV1,
    u64,
    *mut *mut BufferV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type ReaderCloseFn = unsafe extern "C" fn(*mut ReaderV1);
pub(crate) type ReaderFreeFn = unsafe extern "C" fn(*mut ReaderV1);
pub(crate) type OperatorWriterFn = unsafe extern "C" fn(
    *mut OperatorV1,
    *const BytesViewV1,
    *const WriteOptionsV1,
    *mut *mut WriterV1,
    *mut *mut ErrorV1,
) -> Status;
pub(crate) type WriterWriteFn =
    unsafe extern "C" fn(*mut WriterV1, *const BytesViewV1, *mut *mut ErrorV1) -> Status;
pub(crate) type WriterCloseFn =
    unsafe extern "C" fn(*mut WriterV1, *mut *mut MetadataV1, *mut *mut ErrorV1) -> Status;
pub(crate) type WriterFreeFn = unsafe extern "C" fn(*mut WriterV1);

#[repr(C)]
pub(crate) struct ApiV1 {
    pub(crate) struct_size: u32,
    pub(crate) requested_major: u32,
    pub(crate) library_struct_size: u32,
    pub(crate) library_minor: u32,
    pub(crate) library_patch: u32,
    pub(crate) reserved0: u32,
    pub(crate) feature_bits: u64,
    pub(crate) max_output_bytes: u64,
    pub(crate) library_info: Option<LibraryInfoFn>,
    pub(crate) error_view: Option<ErrorViewFn>,
    pub(crate) error_free: Option<ErrorFreeFn>,
    pub(crate) buffer_len: Option<BufferLenFn>,
    pub(crate) buffer_copy: Option<BufferCopyFn>,
    pub(crate) buffer_free: Option<BufferFreeFn>,
    pub(crate) metadata_view: Option<MetadataViewFn>,
    pub(crate) metadata_free: Option<MetadataFreeFn>,
    pub(crate) entry_view: Option<EntryViewFn>,
    pub(crate) entry_metadata_view: Option<EntryMetadataViewFn>,
    pub(crate) entry_free: Option<EntryFreeFn>,
    pub(crate) operator_info_view: Option<OperatorInfoViewFn>,
    pub(crate) operator_info_free: Option<OperatorInfoFreeFn>,
    pub(crate) operator_new: Option<OperatorNewFn>,
    pub(crate) operator_free: Option<OperatorFreeFn>,
    pub(crate) operator_check: Option<OperatorCheckFn>,
    pub(crate) operator_exists: Option<OperatorExistsFn>,
    pub(crate) operator_stat: Option<OperatorStatFn>,
    pub(crate) operator_read: Option<OperatorReadFn>,
    pub(crate) operator_write: Option<OperatorWriteFn>,
    pub(crate) operator_create_dir: Option<OperatorCreateDirFn>,
    pub(crate) operator_delete: Option<OperatorDeleteFn>,
    pub(crate) operator_copy: Option<OperatorCopyFn>,
    pub(crate) operator_rename: Option<OperatorRenameFn>,
    pub(crate) operator_lister: Option<OperatorListerFn>,
    pub(crate) lister_next: Option<ListerNextFn>,
    pub(crate) lister_close: Option<ListerCloseFn>,
    pub(crate) lister_free: Option<ListerFreeFn>,
    pub(crate) operator_reader: Option<OperatorReaderFn>,
    pub(crate) reader_read: Option<ReaderReadFn>,
    pub(crate) reader_close: Option<ReaderCloseFn>,
    pub(crate) reader_free: Option<ReaderFreeFn>,
    pub(crate) operator_writer: Option<OperatorWriterFn>,
    pub(crate) writer_write: Option<WriterWriteFn>,
    pub(crate) writer_close: Option<WriterCloseFn>,
    pub(crate) writer_free: Option<WriterFreeFn>,
}

pub(crate) const API_INPUT_SIZE: usize =
    core::mem::offset_of!(ApiV1, requested_major) + core::mem::size_of::<u32>();
pub(crate) const API_PREFIX_SIZE: usize =
    core::mem::offset_of!(ApiV1, max_output_bytes) + core::mem::size_of::<u64>();

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<BytesViewV1>() == 16);
    assert!(core::mem::align_of::<BytesViewV1>() == 8);
    assert!(core::mem::size_of::<KvV1>() == 32);
    assert!(core::mem::size_of::<ByteRangeV1>() == 32);
    assert!(core::mem::size_of::<ReadOptionsV1>() == 96);
    assert!(core::mem::size_of::<ReaderOptionsV1>() == 64);
    assert!(core::mem::size_of::<WriteOptionsV1>() == 120);
    assert!(core::mem::size_of::<StatOptionsV1>() == 64);
    assert!(core::mem::size_of::<ListOptionsV1>() == 48);
    assert!(core::mem::size_of::<DeleteOptionsV1>() == 40);
    assert!(core::mem::size_of::<MetadataViewV1>() == 168);
    assert!(core::mem::size_of::<EntryViewV1>() == 48);
    assert!(core::mem::size_of::<OperatorInfoViewV1>() == 96);
    assert!(core::mem::size_of::<ErrorViewV1>() == 48);
    assert!(core::mem::size_of::<LibraryInfoViewV1>() == 64);
    assert!(API_INPUT_SIZE == 8);
    assert!(API_PREFIX_SIZE == 40);
};
